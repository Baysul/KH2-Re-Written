//! The vault tuning sections of the audio manifest, read from the converted
//! skatercollections records (audio_export.py `grain_tuning`,
//! `player_tuning`, `collision_tuning`, `bus_tuning`, `frontend_sounds`).
//!
//! Each record is `{class, key, parent, fields: {name: {type, data, array?}}}`;
//! field roles and keys are copied from the Python setup as facts.
use super::formats::{float, py_round};
use crate::Error;
use serde_json::{Map, Value, json};
use skate_data::attrib_hash::hash as hash64;
use std::collections::{BTreeMap, HashMap};

// ---- Field readers (`_exact`, `_floats`, `_words`, `_scalar_or_list`, `_word`, `_value`).

fn ws(s: &str) -> String {
    s.split_whitespace().collect()
}

fn kind_of(field: &Value) -> &str {
    field.get("type").and_then(Value::as_str).unwrap_or("")
}

fn data_of(field: &Value) -> &str {
    field.get("data").and_then(Value::as_str).unwrap_or("")
}

fn has_array(field: &Value) -> bool {
    field.get("array").is_some()
}

fn items(field: &Value) -> Result<&Vec<Value>, Error> {
    field
        .get("array")
        .and_then(|a| a.get("items"))
        .and_then(Value::as_array)
        .ok_or(Error::Format("vault array without items"))
}

/// The field's items (arrays) or its data, as text.
fn item_texts(field: &Value) -> Result<Vec<&str>, Error> {
    if has_array(field) {
        Ok(items(field)?
            .iter()
            .map(|v| v.as_str().unwrap_or(""))
            .collect())
    } else {
        Ok(vec![data_of(field)])
    }
}

/// Python truthiness of an optional field record (a non-empty dict).
fn truthy(field: Option<&Value>) -> bool {
    field.is_some_and(|f| f.as_object().is_some_and(|o| !o.is_empty()))
}

fn hex_u32(hex: &str) -> Result<u32, Error> {
    if hex.len() != 8 {
        return Err(Error::Format("vault word is not four bytes"));
    }
    u32::from_str_radix(hex, 16).map_err(|_| Error::Format("vault word is not hex"))
}

fn be_f32(hex: &str) -> Result<f64, Error> {
    Ok(f64::from(f32::from_bits(hex_u32(hex)?)))
}

fn be_i32(hex: &str) -> Result<i64, Error> {
    Ok(i64::from(hex_u32(hex)? as i32))
}

fn hex_u64(hex: &str) -> Result<u64, Error> {
    if hex.is_empty() {
        return Err(Error::Format("empty vault key"));
    }
    u64::from_str_radix(hex, 16).map_err(|_| Error::Format("vault key is not hex"))
}

fn slice(s: &str, from: usize, to: usize) -> &str {
    let to = to.min(s.len());
    s.get(from.min(to)..to).unwrap_or("")
}

/// `_exact`: f32 exactly, Int32 words, text; arrays as lists.
pub(crate) fn exact(field: &Value) -> Result<Value, Error> {
    let kind = kind_of(field);
    if has_array(field) {
        return items(field)?
            .iter()
            .map(|item| exact(&json!({"type": kind, "data": item})))
            .collect::<Result<Vec<_>, _>>()
            .map(Value::Array);
    }
    let data = ws(data_of(field));
    if kind.ends_with("Text") {
        return field
            .get("data")
            .cloned()
            .ok_or(Error::Format("text field without data"));
    }
    if kind.ends_with("Float") {
        let floats = (0..data.len())
            .step_by(8)
            .map(|i| be_f32(slice(&data, i, i + 8)).map(float))
            .collect::<Result<Vec<_>, _>>()?;
        return Ok(if floats.len() == 1 {
            floats.into_iter().next().unwrap()
        } else {
            Value::Array(floats)
        });
    }
    if kind.ends_with("Int32") && data.len() == 8 {
        return be_i32(&data).map(Value::from);
    }
    Ok(Value::String(data))
}

/// `_floats`: every whole f32 of the field's data.
fn floats_of(data: &str) -> Result<Vec<f64>, Error> {
    let data = ws(data);
    (0..data.len().saturating_sub(7))
        .step_by(8)
        .map(|i| be_f32(slice(&data, i, i + 8)))
        .collect()
}

pub(crate) fn floats(field: &Value) -> Result<Vec<f64>, Error> {
    floats_of(data_of(field))
}

/// `_words`: the first signed word of each item (one byte when shorter).
pub(crate) fn words(field: &Value) -> Result<Vec<i64>, Error> {
    let mut out = Vec::new();
    for item in item_texts(field)? {
        let data = ws(item);
        if data.len() >= 8 {
            out.push(be_i32(slice(&data, 0, 8))?);
        } else if !data.is_empty() {
            out.push(
                i64::from_str_radix(slice(&data, 0, 2), 16)
                    .map_err(|_| Error::Format("vault byte is not hex"))?,
            );
        }
    }
    Ok(out)
}

fn first_word(field: &Value) -> Result<i64, Error> {
    words(field)?
        .first()
        .copied()
        .ok_or(Error::Format("vault field has no word"))
}

/// `_scalar_or_list`: floats for Float types, else signed words.
pub(crate) fn scalar_or_list(field: &Value) -> Result<Value, Error> {
    let values: Vec<Value> = if kind_of(field).ends_with("Float") {
        item_texts(field)?
            .into_iter()
            .map(|i| be_f32(slice(&ws(i), 0, 8)).map(float))
            .collect::<Result<_, _>>()?
    } else {
        words(field)?.into_iter().map(Value::from).collect()
    };
    Ok(if has_array(field) {
        Value::Array(values)
    } else {
        values.into_iter().next().unwrap_or(Value::from(0))
    })
}

/// `_word`: the first 32-bit word as a signed int (zero-padded; 0 if empty).
pub(crate) fn word_of(field: &Value) -> Result<i64, Error> {
    let data = ws(data_of(field));
    if data.is_empty() {
        return Ok(0);
    }
    let mut d = slice(&data, 0, 8).to_owned();
    while d.len() < 8 {
        d.push('0');
    }
    be_i32(&d)
}

fn hex_bytes(data: &str) -> Result<Vec<u8>, Error> {
    let data = ws(data);
    if !data.len().is_multiple_of(2) || !data.is_ascii() {
        return Err(Error::Format("vault data is not whole bytes"));
    }
    (0..data.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&data[i..i + 2], 16).map_err(|_| Error::Format("bad hex")))
        .collect()
}

/// `_value`: float (rounded to 6 places), int, text, or a list for arrays
/// (RefSpec items become the referenced record's u64 key).
pub(crate) fn value(field: &Value) -> Result<Value, Error> {
    let kind = field
        .get("type")
        .and_then(Value::as_str)
        .ok_or(Error::Format("vault field without a type"))?;
    if has_array(field) {
        return items(field)?
            .iter()
            .map(|item| {
                if kind.ends_with("RefSpec") {
                    hex_u64(slice(item.as_str().unwrap_or(""), 16, 32)).map(Value::from)
                } else {
                    value(&json!({"type": kind, "data": item}))
                }
            })
            .collect::<Result<Vec<_>, _>>()
            .map(Value::Array);
    }
    let data = field
        .get("data")
        .ok_or(Error::Format("vault field without data"))?;
    if kind.ends_with("Text") {
        return Ok(data.clone());
    }
    let data = data.as_str().unwrap_or("");
    if kind == "Attrib::RefSpec" {
        return hex_u64(slice(data, 16, 32)).map(Value::from);
    }
    if kind.contains("RefSpec") {
        return hex_u64(slice(data, 0, 16)).map(Value::from);
    }
    let raw = hex_bytes(data)?;
    let raw: [u8; 4] = raw
        .try_into()
        .map_err(|_| Error::Format("vault scalar is not four bytes"))?;
    if kind.ends_with("Float") {
        return Ok(float(py_round(f64::from(f32::from_be_bytes(raw)), 6)));
    }
    Ok(Value::from(i32::from_be_bytes(raw)))
}

// ---- Record indexes.

/// One class's records by key, in first-appearance order (a Python dict
/// built by assignment: later duplicates replace the value in place).
#[derive(Default)]
struct Records<'a> {
    order: Vec<&'a str>,
    map: HashMap<&'a str, &'a Value>,
}

impl<'a> Records<'a> {
    fn insert(&mut self, key: &'a str, record: &'a Value) {
        if self.map.insert(key, record).is_none() {
            self.order.push(key);
        }
    }
    fn get(&self, key: &str) -> Option<&'a Value> {
        self.map.get(key).copied()
    }
    fn contains(&self, key: &str) -> bool {
        self.map.contains_key(key)
    }
    /// The field from the record or its parents (32 steps at most).
    fn resolve(&self, key: &str, field: &str) -> Option<&'a Value> {
        let mut key = key;
        let mut seen = 0;
        while seen < 32 {
            let record = self.get(key)?;
            if let Some(f) = fields(record).get(field) {
                return Some(f);
            }
            key = parent(record);
            seen += 1;
        }
        None
    }
}

fn str_of<'a>(record: &'a Value, name: &str) -> &'a str {
    record.get(name).and_then(Value::as_str).unwrap_or("")
}

fn parent(record: &Value) -> &str {
    str_of(record, "parent")
}

static EMPTY: std::sync::LazyLock<Map<String, Value>> = std::sync::LazyLock::new(Map::new);

fn fields(record: &Value) -> &Map<String, Value> {
    record
        .get("fields")
        .and_then(Value::as_object)
        .unwrap_or(&EMPTY)
}

struct Classes<'a> {
    by_class: HashMap<&'a str, Records<'a>>,
}

impl<'a> Classes<'a> {
    fn new(collections: &'a [Value]) -> Self {
        let mut by_class: HashMap<&str, Records> = HashMap::new();
        for c in collections {
            by_class
                .entry(str_of(c, "class"))
                .or_default()
                .insert(str_of(c, "key"), c);
        }
        Classes { by_class }
    }
    fn class(&self, cls: &str) -> Option<&Records<'a>> {
        self.by_class.get(cls)
    }
    fn resolve(&self, cls: &str, key: &str, field: &str) -> Option<&'a Value> {
        self.class(cls)?.resolve(key, field)
    }
}

fn hash_key(k: u64) -> String {
    format!("Hash_{k:016X}")
}

/// A member path's stem (`Path(member).stem`).
pub(crate) fn path_stem(path: &str) -> &str {
    let name = path.rsplit(['/', '\\']).next().unwrap_or(path);
    match name.rfind('.') {
        Some(i) if i > 0 => &name[..i],
        _ => name,
    }
}

// ---- grain_tuning.

const GRAIN_CLASS: &str = "Hash_7AB23C11B6ADA2DE";
const GRAIN_OWNER_CLASS: &str = "Hash_6E878344774A7999";
const GRAIN_FIELDS: [(&str, &str); 25] = [
    ("Hash_2C073BF8BC45063B", "grain_file"),
    ("Hash_4890392C91829954", "max_kmh"),
    ("Hash_CEC749561306022A", "b_slope_gain"),
    ("Hash_D380D303C64CF6F8", "b_slope_ramp_kmh"),
    ("Hash_1F459FC797B2C6BA", "a_shift_per_slope_hz"),
    ("Hash_5C9AA28695C17004", "turn_cap"),
    ("Hash_145D8340A9440DA3", "b_base_shift_hz"),
    ("Hash_7FFF3A8AD44809EF", "b_shift_per_slope_hz"),
    ("Hash_281FF01081475899", "turn_rise_step"),
    ("Hash_63764B8C7EB9EC9B", "turn_fall_step"),
    ("Hash_6BDC44AE7C3C79D0", "special_gain"),
    ("Hash_F62BC5EBD8E5DDE8", "special_shift_hz"),
    ("Hash_2D751DEB89BB5E33", "push_ramp_kmh"),
    ("Hash_E239B03F0E890686", "push_scale_low"),
    ("Hash_B87ECDDAAB0F8404", "push_scale_high"),
    ("Hash_C658A7923FC7B99E", "push_shift_low_hz"),
    ("Hash_A15AD56E225ADBA6", "push_shift_high_hz"),
    ("Hash_B3D7468820AFC661", "push_scale_attack_ms"),
    ("Hash_DAC9DA910EF0316C", "push_scale_hold_ms"),
    ("Hash_0C3D5DBC262ED276", "push_scale_return_ms"),
    ("Hash_09A5CC79BA2178E7", "push_shift_attack_ms"),
    ("Hash_3206FD96427EA4D2", "push_shift_hold_ms"),
    ("Hash_DB597F672CA47138", "push_shift_return_ms"),
    ("Hash_57A78D3BE8D47BB3", "slope_down_divisor"),
    ("Hash_8DD4C3FC8DAF4059", "slope_up_divisor"),
];
const GRAIN_CURVE_FIELD: &str = "Hash_A985FBAA9326718D";
const GRAIN_PARAMS_FIELD: &str = "Hash_D18D1174735E5CDE";
const GRAIN_OWNER_FIELDS: [(&str, &str); 22] = [
    ("Hash_7508154FF73DDCED", "rocket_start_kmh"),
    ("Hash_1185E9A69919B051", "rocket_top_kmh"),
    ("Hash_9BC13FA19CC4DF00", "rocket_gain_word"),
    ("Hash_88AA96B08FD16914", "g1_level_start_kmh"),
    ("Hash_3FFB5107C82BA3E0", "g1_level_end_kmh"),
    ("Hash_D3E8894CA25A4F71", "g1_level_floor"),
    ("Hash_0D665393E2EDC605", "g3_send_start_kmh"),
    ("Hash_28E708782445747F", "g3_send_end_kmh"),
    ("Hash_D900C07BE7C5450F", "g3_send_max"),
    ("Hash_E64C04ED542DABC8", "g3_clip"),
    ("Hash_55BEB30353F244A9", "g3_shelf_hz"),
    ("Hash_45516395725ED16B", "g3_shelf_gain"),
    ("Hash_281A501B22B6CCDF", "wobble_start_kmh"),
    ("Hash_54CDE019E31FC04E", "wobble_end_kmh"),
    ("Hash_36AE41817640FE04", "wobble_a_ms_low"),
    ("Hash_71EE27313BD30F21", "wobble_a_ms_high"),
    ("Hash_437D128B53669C34", "wobble_a_low"),
    ("Hash_02885338DD5D7DCA", "wobble_a_high"),
    ("Hash_2055BBF39C152FA9", "wobble_b_ms_low"),
    ("Hash_F5240AFADA3B3FFC", "wobble_b_ms_high"),
    ("Hash_F916E153393C5F24", "wobble_b_low"),
    ("Hash_0A36F90732016D85", "wobble_b_high"),
];
const SURFACE_MAP: (&str, &str, &str) = (
    "Hash_C1831BDB6CB1B1EA",
    "Hash_C489459A0C07D154",
    "Hash_4CA607558B1CF440",
);

fn params_items(field: &Value) -> Result<Vec<&str>, Error> {
    if has_array(field) {
        Ok(items(field)?
            .iter()
            .map(|v| v.as_str().unwrap_or(""))
            .collect())
    } else {
        Ok(vec![
            field
                .get("data")
                .and_then(Value::as_str)
                .ok_or(Error::Format("vault field without data"))?,
        ])
    }
}

fn floats_value(values: Vec<f64>) -> Value {
    Value::Array(values.into_iter().map(float).collect())
}

/// `{surfaces: {member stem: tuning}, default, owner, surface_map}`.
pub fn grain_tuning(collections: &[Value]) -> Result<Value, Error> {
    let mut records = Records::default();
    for c in collections
        .iter()
        .filter(|c| str_of(c, "class") == GRAIN_CLASS)
    {
        records.insert(str_of(c, "key"), c);
    }
    let tuning = |key: &str| -> Result<Map<String, Value>, Error> {
        let mut out = Map::new();
        for (field, name) in GRAIN_FIELDS {
            if let Some(v) = records.resolve(key, field) {
                out.insert(name.into(), exact(v)?);
            }
        }
        if let Some(curve) = records.resolve(key, GRAIN_CURVE_FIELD) {
            let m = floats(curve)?;
            if m.len() == 16 {
                out.insert("bezier".into(), floats_value(vec![m[13], m[9], m[5], m[1]]));
            }
        }
        if let Some(params) = records.resolve(key, GRAIN_PARAMS_FIELD) {
            let rows = params_items(params)?
                .into_iter()
                .map(|item| floats_of(item).map(floats_value))
                .collect::<Result<Vec<_>, _>>()?;
            out.insert("params".into(), Value::Array(rows));
        }
        Ok(out)
    };
    let mut surfaces = Map::new();
    for key in &records.order {
        let mut entry = tuning(key)?;
        let member = entry.remove("grain_file");
        let member = match &member {
            Some(Value::String(s)) if !s.is_empty() => s.as_str(),
            Some(v) if truthy_value(v) => {
                return Err(Error::Format("grain_file is not text"));
            }
            _ => continue,
        };
        surfaces.insert(path_stem(member).to_owned(), Value::Object(entry));
    }
    let mut owner = Map::new();
    let mut owners = Records::default();
    for c in collections
        .iter()
        .filter(|c| str_of(c, "class") == GRAIN_OWNER_CLASS)
    {
        owners.insert(str_of(c, "key"), c);
    }
    let default_owner = owners.get("default").map(fields).unwrap_or(&EMPTY);
    for (field, name) in GRAIN_OWNER_FIELDS {
        if let Some(v) = default_owner.get(field) {
            owner.insert(name.into(), exact(v)?);
        }
    }
    if let Some(field) = default_owner.get(GRAIN_PARAMS_FIELD) {
        let first = *params_items(field)?
            .first()
            .ok_or(Error::Format("rocket params without items"))?;
        owner.insert("rocket_params".into(), floats_value(floats_of(first)?));
    }
    let mut surface_map = Vec::new();
    let holder = collections
        .iter()
        .find(|c| str_of(c, "class") == SURFACE_MAP.0 && str_of(c, "key") == SURFACE_MAP.1);
    if let Some(field) = holder.and_then(|h| fields(h).get(SURFACE_MAP.2)) {
        for item in items(field)? {
            let data = ws(item.as_str().unwrap_or(""));
            surface_map.push(Value::from(hex_u32(slice(&data, 8, 16))?));
        }
    }
    let mut default = tuning("default")?;
    default.remove("grain_file");
    Ok(
        json!({"surfaces": surfaces, "default": default, "owner": owner, "surface_map": surface_map}),
    )
}

/// Python truthiness of a JSON value.
fn truthy_value(v: &Value) -> bool {
    match v {
        Value::Null => false,
        Value::Bool(b) => *b,
        Value::Number(n) => n.as_f64().is_some_and(|x| x != 0.0),
        Value::String(s) => !s.is_empty(),
        Value::Array(a) => !a.is_empty(),
        Value::Object(o) => !o.is_empty(),
    }
}

// ---- player_tuning and collision_tuning.

const JITTER_CLASS: &str = "Hash_0AB9F005A2C8FBC7";
const SEAM_CLASS: &str = "Hash_7242F32831ED3332";
const SEAM_PATTERNS: [&str; 15] = [
    "spidercrack",
    "square_2_x_2",
    "square_4_x_4",
    "square_8_x_8",
    "square_12_x_12",
    "square_24_x_24",
    "irregular_small",
    "irregular_medium",
    "irregular_large",
    "slats",
    "sidewalk",
    "brick_tile_random_size",
    "mini_tile",
    "special_1",
    "special_2",
];
/// SEAM_FIELDS then SEAM_PATTERN_FIELDS (`{**SEAM_FIELDS, **SEAM_PATTERN_FIELDS}`).
const SEAM_ALL_FIELDS: [(&str, &str); 15] = [
    ("Hash_FA3A57801765A2F8", "gain_low"),
    ("Hash_32A9692F1B826274", "gain_high"),
    ("Hash_F713CB547B1DF920", "ms_low"),
    ("Hash_0608B3129FF81F12", "ms_high"),
    ("Hash_199170BB1C52EE64", "gain"),
    ("Hash_D18F436B5764F260", "angle"),
    ("Hash_534B0A719762E2E8", "grid_z"),
    ("Hash_107A78BA11A2B813", "grid_x"),
    ("Hash_5FAD918A2DE5459A", "class"),
    ("Hash_CA81764BF5A85E34", "mode"),
    ("Hash_DAE803A0CBD286D1", "min_frames"),
    ("Hash_F7BCA67F0A1FC92E", "speed_threshold"),
    ("Hash_A3BC1976039FA00A", "spacing"),
    ("Hash_2F29F40384863C8C", "level"),
    ("Hash_1911176187FB9B1F", "surface3_scale"),
];
const GRIND_CLASS: &str = "Hash_049861E8F9A8D16B";
const GRIND_SURFACE_KEYS: [u64; 14] = [
    0x72766EB54205429A,
    0x849358CD45882044,
    0x8DB0436F6B3405D5,
    0xA36240FD856EEBA2,
    0x8212F939B27CB441,
    0x8710D7DAA28B90E7,
    0x80AD71A0341B4AEB,
    0x29FC132BB19BE7C9,
    0xFD4F9F04AE61311B,
    0x7CBD549861AE837E,
    0xBDDCF1000271E0F7,
    0x0C4C7B1701C3EA1B,
    0x3696D249613957DD,
    0xEDB51E711C25AA58,
];
const GRIND_V: [&str; 4] = [
    "Hash_0ECECDAC28B2B979",
    "Hash_58070BF511809903",
    "Hash_72BA0780A8FA25D6",
    "Hash_721A50C80028AD69",
];
const GRIND_F: [&str; 4] = [
    "Hash_69969AF1BE6BB367",
    "Hash_0555484D6D4A3128",
    "Hash_C21983A2160ED3AC",
    "Hash_69A5FC53091ED1F8",
];
const MATERIAL_CLASS: &str = "Hash_D40CB4C0FFE45676";
const MATERIAL_FOOTSTEPS: &str = "Hash_B1CF62EA632CF13F";
const MATERIAL_STEP_GAIN: &str = "Hash_37320DF00471F91A";
const MATERIAL_STEP_LANDING_GAIN: &str = "Hash_9D6068AB16703650";
const LANDING_FLAG: &str = "Hash_1EBF9D2EB0DD56BA";
const WHEEL_CLASS: &str = "Hash_C26949FCB638A2CA";
const TRICK_CLASS: &str = "Hash_6918469984A8C596";
const TRICK_FIELD: &str = "Hash_8C3025DB4D1761AF";
const TRICK_FIELD_2: &str = "Hash_A2C5C22C5BE725F8";
const WHEEL_BUCKET: [(&str, &str); 2] = [
    ("Hash_6D3D91A9BA7ADCDC", "wheel_bucket_high"),
    ("Hash_2A70BB8A382574E4", "wheel_bucket_low"),
];
const MATERIAL_KINDS: [i8; 143] = [
    0, 0, 0, 0, 0, 0, 0, 0, 1, 0, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1,
    1, 1, 1, 1, 1, 1, 1, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
    0, 0, 1, 1, 1, 0, 0, 0, 1, 0, 0, 0, 0, 0, 0, 0, 0, 1, 0, 1, 1, 0, 0, 0, 1, 0, 1, 0, 0, 0, -1,
    0, 0, 0, 0, 0, 0, 0, 2, 2, 2, 2, 2, 0, 0, 0, 0, 0, 0, 0, 1, 1, 1, 1, 1, 1, 1, 1, 1, 0, 0, 0, 0,
    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
];
const MATERIAL_KEYS: [u64; 143] = [
    0x84EF7A70636ED529,
    0x9182CCAD63479434,
    0x49A5B1F124E53FDE,
    0x49F167E3FC3A008B,
    0xD7EFFA5849BB7B15,
    0x72B4ECCDC6CEE6E8,
    0xAF8EAB3B0227FD48,
    0x54D22F1336F8B796,
    0x42CD19124AE613EB,
    0x0613243A490D11EA,
    0xA4B6334D87D72CC7,
    0x3738E9708EB6C776,
    0x1BFF6E5CA982683A,
    0x5BBDF4492AC7FB47,
    0x22132549704E8733,
    0x1F2ACA29DEBE0729,
    0x500227DC11D198AC,
    0xB7DBFE813D021524,
    0x6EAACF19DDD07B5A,
    0x60EC16F48C0BE620,
    0x791C508A6E1205EA,
    0x42337BA2D862B622,
    0x10ED41289D47F052,
    0xBD556D5B135BBF8E,
    0xB74F55A0A96516A0,
    0x3B214D295DB514F7,
    0xA7AB65A3C044B3DA,
    0xE96D23CB66372699,
    0xB062804C25EDD8A7,
    0xA8EF0E67A39D62D9,
    0xF7141297FEB29B5D,
    0x0DEB3D61CAED9567,
    0x7AC77169773C0CE4,
    0x0193A08E3EDA269B,
    0xDBF3EC4C65A69F5D,
    0x3A43FEE3639BA301,
    0x309033B5E5872675,
    0xEF0C2293B7888864,
    0xD807FA29ADE65957,
    0x10017676688AA504,
    0xD8F132C02494531F,
    0x5D4D49C598C7C2E3,
    0xD3184F6BD8E629CC,
    0xA227D8D1EB22C467,
    0x7D5F7C90590CD76C,
    0xE351230372B8AA43,
    0xC65BAF14769DFCEB,
    0x13D19F33ABDEFDA4,
    0x6B170B796C325537,
    0x8640A18245382F66,
    0xC53FA0D15249C949,
    0x9988214E2D3F25C2,
    0x6A0D63DA93EF3459,
    0x99433514E821A363,
    0xF002CCA1F7FD6978,
    0x947A2EEC879CF0C4,
    0xC5C3FCD994414A11,
    0x45EF058BC984C7E8,
    0x03745EEB6C08A0B7,
    0xF3D78078D942C1F5,
    0x3E93846A0C0904EF,
    0x71D3FF58545A7240,
    0x36F1FB399593A42E,
    0xCA39C5BF26B5A8CE,
    0x98CD99EC1E257D5A,
    0x94BFC99C9D77FBE9,
    0x3F4E33C9A0B9B05E,
    0xDC7B09876FB19FFF,
    0x8BD87DE72E1A3157,
    0x0613243A490D11EA,
    0x0613243A490D11EA,
    0x0613243A490D11EA,
    0xBFBDAC657253E431,
    0x73235D7F8A764EA3,
    0x070B3A354E4B91E8,
    0x6C322F848110963C,
    0x07FB54E5A776E0DD,
    0x4EE7754C40081FFB,
    0x59BA34C2182DFAED,
    0x761C67A1D52C27D8,
    0x5AAA7F375D3FAFD7,
    0x7CAF97EA114D54A9,
    0xA029E9DED863C78E,
    0xB13FD71EA972B2E7,
    0x061A5C9D1EB44C60,
    0x0526D0B41AA7E7F3,
    0xD7A4B6C19268884E,
    0x0023C43AFE5425C2,
    0x0B56CDEC643B3C5C,
    0xA5C4BE35A6484D2A,
    0x44C45991DF83F528,
    0x954AB2622DDDCDAE,
    0xB2BD1F28DDE601B0,
    0x09A0A000A6C314BE,
    0x0000000000000000,
    0xF512E3A5DFD6C51C,
    0x27CDE290C32867A9,
    0x379CADECE541E342,
    0xFC139EC967F75F74,
    0xAB73644DAB95C51A,
    0x65AF5100E77A2982,
    0x0DB9BC878EC6B280,
    0xFB0158EFB8889A2A,
    0x87684DAC4583AED5,
    0xDC73596A59C3A8B9,
    0xF16BBF49DDEA88A9,
    0xC83E4BE38A953FAA,
    0xD2E3F99A952DC96B,
    0x66153E29A9E4F5F0,
    0xB025A0D5D09BAD53,
    0xC54DB3B091215CA0,
    0xF2E9CB8F28FAED2D,
    0x7410CAC4852F5168,
    0x76535DC1C13DC342,
    0x56AE4185FFB27F3B,
    0xBC46374D5FE945CC,
    0x4E3F3F3D686C1376,
    0xFD06F928C97B0C13,
    0x272430E460312991,
    0x8CDC59AF8D2FE8C0,
    0x3C39F714BDF14002,
    0x582E3694A7D7A81C,
    0xDAB71EE7131FB16F,
    0x5C7F8A8CD61ADC6A,
    0xD715DAADCCFE3707,
    0xD1104F0408894184,
    0x0180D422774A2E76,
    0x7EE4E115095A35FE,
    0xE6ACC17E8C6E545D,
    0x897D82A74C3DD55F,
    0x9322B76721A7E961,
    0x88E70FFEEB920FAC,
    0xE5E4636B15D921E1,
    0xB2B199D7B0CC6E56,
    0x361AF94AF4738607,
    0xBC8EB5FAC4FBFCAA,
    0xDE485796A9BF0518,
    0x4E988E099EB988FA,
    0xBFA1E3CB2AC6A29F,
    0xBA3A875662F7B1A3,
    0x90C5D36FD10D6A3C,
    0x93046C79A53E24D9,
    0x93610C96EB08CDAD,
];
const MATERIAL_IDS: [[&str; 7]; 3] = [
    [
        "Hash_9203DF6FD029B377",
        "Hash_BFABF634D2B1E45A",
        "Hash_9ABFC64574AB2F9F",
        "Hash_BCD5E888294F7B15",
        "Hash_EF9BD81F9CFF725F",
        "Hash_A3ADCA7B19287B5D",
        "Hash_C676C87F862C0490",
    ],
    [
        "Hash_F54277A83E0170FD",
        "Hash_66A95889604DED36",
        "Hash_595537EBA0196BE7",
        "Hash_79DD0E6659793D0E",
        "Hash_B722B88FE44B046E",
        "Hash_1411108A7E9CC74A",
        "Hash_50796F92F3DE449B",
    ],
    [
        "Hash_3EA2579C2F3BB23B",
        "Hash_137C683BFB506ECA",
        "Hash_2A2830137430BB02",
        "Hash_79BDE00B04DF51B9",
        "Hash_5432B35224E4B1C1",
        "Hash_3FCBE0407F833721",
        "Hash_53311C6761F135A1",
    ],
];
const MATERIAL_GAIN: &str = "Hash_875BA75341DC8391";
const MATERIAL_PITCH_FLAG: &str = "Hash_57E3031C18C769A1";
const MATERIAL_PITCH: &str = "Hash_C090F2C1F048F17B";
const MATERIAL_PITCH_ALT: &str = "Hash_3A3DD47E8DAFE796";
const MATERIAL_CATEGORY: &str = "Hash_D5EF686287A57AFE";
const MATERIAL_WINDOWS: &str = "Hash_82B1451A90152514";
const MATERIAL_BANDS: &str = "Hash_E228508FE0F53970";
const WINDOW_CLASS: &str = "Hash_13E20D398E385A56";
const WINDOW_FIELDS: [&str; 14] = [
    "Hash_036F313CEBC664FD",
    "Hash_8001982DA2E91D6A",
    "Hash_E24D9CB4000A53AC",
    "Hash_1A83AD0330976744",
    "Hash_24B725E05CEB027E",
    "Hash_CBBB19E302CE1A17",
    "Hash_75DC915E876A9DC9",
    "Hash_711F1F54903E76C9",
    "Hash_C8DED1BC20B9D6A5",
    "Hash_B8870D2001033E0F",
    // WINDOW_FIELDS_2
    "Hash_41F2E2456A97752A",
    "Hash_2638FF12C8FBBCCB",
    "Hash_166BAB2FD5B60560",
    "Hash_0D6EF57A39AF0C93",
];
const WINDOW_SCALE: &str = "Hash_1A5F7E8CCABBB0A2";
const BAND_CLASS: &str = "Hash_7DAFF70B3A91CD5D";
const BAND_FIELDS: [&str; 4] = [
    "Hash_C8DED1BC20B9D6A5",
    "Hash_B8870D2001033E0F",
    "Hash_7D8DEDD338D45482",
    "Hash_D660AC459139BDF4",
];
const CONTACTS_CLASS: &str = "Hash_6EBA5BCD3E38A98A";
const COLLISION_POSTERS: [(&str, &str, &str, &str); 23] = [
    (
        GRIND_CLASS,
        "default",
        "Hash_086B66C3D4FFEE8F",
        "grind_split",
    ),
    (
        GRIND_CLASS,
        "default",
        "Hash_B2ACAFDBCD963C93",
        "grind_high",
    ),
    (
        WHEEL_CLASS,
        "default",
        "Hash_6D68BC2D1A23C29A",
        "landing_air",
    ),
    (
        WHEEL_CLASS,
        "default",
        "Hash_85FDC8BF696BCA5C",
        "landing_board",
    ),
    (
        WHEEL_CLASS,
        "default",
        "Hash_3462CBB16DCA696E",
        "landing_split",
    ),
    (
        WHEEL_CLASS,
        "default",
        "Hash_590495E420B399E5",
        "landing_high",
    ),
    (
        WHEEL_CLASS,
        "default",
        "Hash_31DEEF8FA219950F",
        "landing_scale_a",
    ),
    (
        WHEEL_CLASS,
        "default",
        "Hash_0EC6EEF5366FEA85",
        "landing_scale_b",
    ),
    (
        CONTACTS_CLASS,
        "default",
        "Hash_27D3C5DC3282B59D",
        "deck_cooldown",
    ),
    (
        WHEEL_CLASS,
        "default",
        "Hash_823D59FB46324175",
        "scuff_speed",
    ),
    (WHEEL_CLASS, "default", "Hash_733C45DF5B638ECB", "scuff_ids"),
    (
        WHEEL_CLASS,
        "default",
        "Hash_F2866EE0540CDF05",
        "scuff_ids_soft",
    ),
    (TAP_CLASS, TAP_KEY, "Hash_6BFB6A3F22AAF797", "tap_off_ms"),
    (TAP_CLASS, TAP_KEY, "Hash_B63CC740D29AD118", "tap_speed"),
    (TAP_CLASS, TAP_KEY, "Hash_7D1CA200987AA109", "tap_mid"),
    (TAP_CLASS, TAP_KEY, "Hash_E1F87C4937E3CCA2", "tap_high"),
    (TAP_CLASS, TAP_KEY, "Hash_6F133132EF3F8063", "tap_first"),
    (TAP_CLASS, TAP_KEY, "Hash_E518088F4E61203A", "tap_second"),
    (TAP_CLASS, TAP_KEY, "Hash_4CFF36855CA04B6F", "tap_both"),
    (TAP_CLASS, TAP_KEY, "Hash_3AC5AA2D2010B49C", "tap_special"),
    (
        TAP_CLASS,
        TAP_KEY,
        "Hash_DB56952676D7122E",
        "tap_first_soft",
    ),
    (
        TAP_CLASS,
        TAP_KEY,
        "Hash_0ACFCB7E532B7A0B",
        "tap_second_soft",
    ),
    (
        TAP_CLASS,
        TAP_KEY,
        "Hash_7EA37827E4228614",
        "tap_other_soft",
    ),
];
const TAP_CLASS: &str = "Hash_923CCB46EF5BF5BA";
const TAP_KEY: &str = "Hash_F1647BFB782BE97F";
const PLANT_FIELDS: [&str; 5] = [
    "Hash_1A5D3CDBA6C160D0",
    "Hash_6C93C9BAD7B07C6B",
    "Hash_9CCABF46584CA16C",
    "Hash_57553DC3A33C9B38",
    "Hash_4F138972C957C8AF",
];
const LIFT_FIELDS: [&str; 5] = [
    "Hash_2D97D30AEA78BCE2",
    "Hash_73CB69882A79481B",
    "Hash_767F6CAEACB748C8",
    "Hash_A17BA1994B766B55",
    "Hash_8D8EF475983B33A8",
];
const EQCHAIN_CLASS: &str = "Hash_42AFE160E647167C";
const PLANT_EQ: &str = "Hash_748CBC9727A5347F";
const GRIND_CONTACT_EQ: &str = "Hash_D1A87641CCB98787";
const BODY_POSTERS: [(&str, &str); 11] = [
    ("Hash_6DD85F43C1B6E6AA", "body_cooldown"),
    ("Hash_D12003A60E987B9D", "body_110_head_low"),
    ("Hash_FA2AA5A0C0481D00", "body_110_head_high"),
    ("Hash_AFA4B5090F1BCF36", "body_110_torso_low"),
    ("Hash_00960FEDB3EFEE9C", "body_110_torso_high"),
    ("Hash_3695327CFB5E1AC3", "body_111_low"),
    ("Hash_35FEE8A95523D812", "body_111_high"),
    ("Hash_076E9081CA1759E9", "body_112_low0"),
    ("Hash_DF539915EB7E883E", "body_112_high0"),
    ("Hash_8E3025BAA686F721", "body_112_low1"),
    ("Hash_504D3B73505972D4", "body_112_high1"),
];
const BODY_SPEED_GRAPH: &str = "Hash_8B164823E008749C";
const GRIND_METAL: &str = "Hash_02BAC36BCC8A30DE";

struct GrindContact {
    name: &'static str,
    ids: [[&'static str; 4]; 2],
    gain: [&'static str; 4],
    level: [&'static str; 2],
    pitch: [&'static str; 2],
}

const GRIND_CONTACTS: [GrindContact; 2] = [
    GrindContact {
        name: "on",
        ids: [
            [
                "Hash_013C2E46521A9080",
                "Hash_71F2DBB8CA385137",
                "Hash_F9FED84DD3E93919",
                "Hash_2162F0EFBDDE72C6",
            ],
            [
                "Hash_F344EC2BC1BD4B6E",
                "Hash_4A76108CDB52FDD3",
                "Hash_8375C9A4AD144F7D",
                "Hash_991C563B70C40DED",
            ],
        ],
        gain: [
            "Hash_3D63FBC14F6A6CEA",
            "Hash_0BD0F347C3AC6D00",
            "Hash_18A94AE8DEED0D27",
            "Hash_AA81C2EC8304ADF0",
        ],
        level: ["Hash_60418B2EF89EFE2D", "Hash_FF11B09A395C5F7B"],
        pitch: ["Hash_AA380B47376A7415", "Hash_E5E8E505DDEE595B"],
    },
    GrindContact {
        name: "off",
        ids: [
            [
                "Hash_FDDA1C015F450486",
                "Hash_9086C01E46C97B05",
                "Hash_B2DB39923B3A2C3F",
                "Hash_FFD9CE934742D506",
            ],
            [
                "Hash_A7639B2F6ABFF8E8",
                "Hash_F314474F29EB8D64",
                "Hash_60470BE7D8CA45C4",
                "Hash_92258B0B80C10301",
            ],
        ],
        gain: [
            "Hash_958D61F2EC118BFF",
            "Hash_94E3254CD727BEA6",
            "Hash_751C968D0992C231",
            "Hash_624BCB9A663D35DB",
        ],
        level: ["Hash_D0BD38748095DCE3", "Hash_E02ABA94997FABE7"],
        pitch: ["Hash_9F866C1CA0A23F3C", "Hash_196D60DE7611D2D3"],
    },
];

/// `{surface_table, jitter, seam_wobbles, grind, grind_contact_eq,
/// wheel_bucket_high/low, jump_thresholds, audio_tricks(_2), collision,
/// landing_materials}` (`player_tuning` without the TU3 image).
pub fn player_tuning(collections: &[Value]) -> Result<Value, Error> {
    let classes = Classes::new(collections);
    let mut out = Map::new();
    let holder = classes
        .class(SURFACE_MAP.0)
        .and_then(|r| r.get(SURFACE_MAP.1));
    if let Some(field) = holder.and_then(|h| fields(h).get(SURFACE_MAP.2)) {
        let mut rows = Vec::new();
        for item in items(field)? {
            let data = ws(item.as_str().unwrap_or(""));
            let end = data.len().min(144);
            let mut row = (0..end.saturating_sub(7))
                .step_by(8)
                .map(|i| be_i32(slice(&data, i, i + 8)))
                .collect::<Result<Vec<_>, _>>()?;
            row.resize(row.len().max(18), 0);
            rows.push(json!(row));
        }
        out.insert("surface_table".into(), Value::Array(rows));
    }
    if let Some(jitter) = classes.class(JITTER_CLASS).filter(|j| !j.order.is_empty()) {
        let parents: std::collections::HashSet<&str> =
            jitter.map.values().map(|c| parent(c)).collect();
        let mut leaves: Vec<(u64, &str)> = Vec::new();
        for key in &jitter.order {
            if !parents.contains(key) && key.starts_with("Hash_") {
                leaves.push((hex_u64(&key[5..])?, key));
            }
        }
        leaves.sort_by_key(|l| l.0);
        let mut channels = Vec::new();
        for (_, key) in leaves {
            let enabled = jitter.resolve(key, "Hash_8F956FBAD301AE26");
            let ident = jitter.resolve(key, "Hash_E7D491E2EB228F54");
            let Some(params) = jitter.resolve(key, "Hash_B66AAD957873A8B3") else {
                continue;
            };
            channels.push(json!({
                "enabled": truthy(enabled) && data_of(enabled.unwrap()).starts_with("01"),
                "id": if truthy(ident) { exact(ident.unwrap())? } else { Value::from(0) },
                "params": floats_value(floats(params)?),
                "key": &key[5..],
            }));
        }
        out.insert("jitter".into(), Value::Array(channels));
    }
    if classes.class(SEAM_CLASS).is_some() {
        let mut wobbles = Vec::new();
        let keys = std::iter::once("default".to_owned())
            .chain(SEAM_PATTERNS.iter().map(|n| hash_key(hash64(n))));
        for key in keys {
            let mut entry = Map::new();
            for (field, name) in SEAM_ALL_FIELDS {
                if let Some(v) = classes.resolve(SEAM_CLASS, &key, field) {
                    let v = if name == "mode" {
                        Value::from(word_of(v)?)
                    } else {
                        exact(v)?
                    };
                    entry.insert(name.into(), v);
                }
            }
            wobbles.push(Value::Object(entry));
        }
        out.insert("seam_wobbles".into(), Value::Array(wobbles));
    }
    if classes.class(GRIND_CLASS).is_some() {
        let mut surfaces = Vec::new();
        let or_one = |x: Option<&Value>| -> Result<Value, Error> {
            if truthy(x) {
                exact(x.unwrap())
            } else {
                Ok(float(1.0))
            }
        };
        for k in GRIND_SURFACE_KEYS {
            let key = hash_key(k);
            let r = |f: &str| classes.resolve(GRIND_CLASS, &key, f);
            let mut entry = Map::new();
            entry.insert(
                "v".into(),
                Value::Array(
                    GRIND_V
                        .iter()
                        .map(|f| or_one(r(f)))
                        .collect::<Result<_, _>>()?,
                ),
            );
            entry.insert(
                "f".into(),
                Value::Array(
                    GRIND_F
                        .iter()
                        .map(|f| or_one(r(f)))
                        .collect::<Result<_, _>>()?,
                ),
            );
            let mut metal = false;
            if let Some(m) = r(GRIND_METAL) {
                let d = ws(data_of(m));
                metal = !matches!(slice(&d, 0, 2), "" | "00");
                entry.insert("metal".into(), metal.into());
            }
            for contact in &GRIND_CONTACTS {
                let ids: Vec<_> = contact.ids[usize::from(metal)]
                    .iter()
                    .map(|f| r(f))
                    .collect();
                let gain: Vec<_> = contact.gain.iter().map(|f| r(f)).collect();
                let level: Vec<_> = contact.level.iter().map(|f| r(f)).collect();
                let pitch: Vec<_> = contact.pitch.iter().map(|f| r(f)).collect();
                if ids.iter().chain(&level).chain(&pitch).any(Option::is_none) {
                    continue;
                }
                let ids = ids
                    .iter()
                    .map(|x| first_word(x.unwrap()).map(Value::from))
                    .collect::<Result<Vec<_>, _>>()?;
                let gain = gain
                    .iter()
                    .map(|x| or_one(*x))
                    .collect::<Result<Vec<_>, _>>()?;
                let level = level
                    .iter()
                    .map(|x| exact(x.unwrap()))
                    .collect::<Result<Vec<_>, _>>()?;
                let pitch = pitch
                    .iter()
                    .map(|x| exact(x.unwrap()))
                    .collect::<Result<Vec<_>, _>>()?;
                entry.insert(
                    contact.name.into(),
                    json!({"ids": ids, "gain": gain, "level": level, "pitch": pitch}),
                );
            }
            surfaces.push(Value::Object(entry));
        }
        out.insert("grind".into(), Value::Array(surfaces));
        let bus = classes
            .class(EQCHAIN_CLASS)
            .and_then(|r| r.get("default"))
            .and_then(|d| fields(d).get(GRIND_CONTACT_EQ));
        if let Some(bus) = bus {
            out.insert("grind_contact_eq".into(), first_word(bus)?.into());
        }
    }
    let wheel = classes.class(WHEEL_CLASS).and_then(|r| r.get("default"));
    if let Some(wheel) = wheel.filter(|w| w.as_object().is_some_and(|o| !o.is_empty())) {
        for (field, name) in WHEEL_BUCKET {
            if let Some(v) = fields(wheel).get(field) {
                out.insert(name.into(), exact(v)?);
            }
        }
    }
    if let Some(jump) = classes.resolve("Hash_A867FBE3454326FF", "default", "Hash_468752B0BEE65CDB")
    {
        let values = items(jump)?
            .iter()
            .map(|x| be_f32(x.as_str().unwrap_or("")))
            .collect::<Result<Vec<_>, _>>()?;
        if values.len() != 2 {
            return Err(Error::Format(
                "native jump-strength tuning must contain two thresholds",
            ));
        }
        out.insert("jump_thresholds".into(), floats_value(values));
    }
    if let Some(tricks_class) = classes.class(TRICK_CLASS) {
        for (name, field) in [
            ("audio_tricks", TRICK_FIELD),
            ("audio_tricks_2", TRICK_FIELD_2),
        ] {
            let mut tricks = Map::new();
            for key in &tricks_class.order {
                if !key.starts_with("Hash_") {
                    continue;
                }
                if let Some(v) = tricks_class.resolve(key, field) {
                    let d = ws(data_of(v));
                    if d.len() == 8 {
                        tricks.insert(key[5..].to_owned(), be_i32(&d)?.into());
                    }
                }
            }
            out.insert(name.into(), Value::Object(tricks));
        }
    }
    if classes.class(MATERIAL_CLASS).is_some() {
        let collision = collision_tuning(&classes)?;
        let landing: Vec<Value> = collision["materials"]
            .as_array()
            .into_iter()
            .flatten()
            .enumerate()
            .filter(|(_, row)| row["landing"] == true)
            .map(|(m, _)| m.into())
            .collect();
        out.insert("collision".into(), collision);
        out.insert("landing_materials".into(), Value::Array(landing));
    }
    Ok(Value::Object(out))
}

/// `{materials: [143 rows], posters}`: the collision manager's material
/// table and the vault values its posters read.
fn collision_tuning(classes: &Classes<'_>) -> Result<Value, Error> {
    // Name-keyed records also answer to their hash64 key (`setdefault`).
    let mut aliased: HashMap<&str, Records> = HashMap::new();
    let mut alias_keys: Vec<String> = Vec::new();
    for cls in [MATERIAL_CLASS, WINDOW_CLASS, BAND_CLASS] {
        if let Some(records) = classes.class(cls) {
            for name in &records.order {
                if !name.starts_with("Hash_") {
                    alias_keys.push(hash_key(hash64(name)));
                }
            }
        }
    }
    let mut alias_iter = alias_keys.iter();
    for cls in [MATERIAL_CLASS, WINDOW_CLASS, BAND_CLASS] {
        let mut records = Records::default();
        if let Some(original) = classes.class(cls) {
            for key in &original.order {
                records.insert(key, original.map[key]);
            }
            for name in &original.order {
                if !name.starts_with("Hash_") {
                    let alias = alias_iter.next().unwrap();
                    if !records.contains(alias) {
                        records.insert(alias.as_str(), original.map[name]);
                    }
                }
            }
        }
        aliased.insert(cls, records);
    }
    let resolve = |cls: &str, key: &str, field: &str| -> Option<&Value> {
        match aliased.get(cls) {
            Some(records) => records.resolve(key, field),
            None => classes.resolve(cls, key, field),
        }
    };
    let has = |cls: &str, key: &str| aliased.get(cls).is_some_and(|r| r.contains(key));
    let ref_key = |field: Option<&Value>| -> Option<String> {
        let data = field.map(|f| ws(data_of(f))).unwrap_or_default();
        (data.len() >= 32).then(|| format!("Hash_{}", slice(&data, 16, 32).to_uppercase()))
    };
    let word = |cls: &str, key: &str, f: &str, default: i64| -> Result<i64, Error> {
        match resolve(cls, key, f) {
            Some(v) => Ok(words(v)?.first().copied().unwrap_or(default)),
            None => Ok(default),
        }
    };
    let mut rows = Vec::with_capacity(143);
    for m in 0..143 {
        let (kind, key) = (MATERIAL_KINDS[m], hash_key(MATERIAL_KEYS[m]));
        let mut row = Map::new();
        row.insert("kind".into(), kind.into());
        if !has(MATERIAL_CLASS, &key) {
            row.insert("missing".into(), true.into());
        }
        if (0..3).contains(&kind) {
            let ids = MATERIAL_IDS[kind as usize]
                .iter()
                .map(|f| word(MATERIAL_CLASS, &key, f, 0).map(Value::from))
                .collect::<Result<Vec<_>, _>>()?;
            row.insert("ids".into(), Value::Array(ids));
        }
        let w = |f: &str, default: i64| word(MATERIAL_CLASS, &key, f, default);
        row.insert("gain".into(), w(MATERIAL_GAIN, 0)?.into());
        row.insert("pitch".into(), w(MATERIAL_PITCH, 4096)?.into());
        row.insert(
            "pitch_flag".into(),
            (w(MATERIAL_PITCH_FLAG, 0)? != 0).into(),
        );
        if let Some(alt) = resolve(MATERIAL_CLASS, &key, MATERIAL_PITCH_ALT) {
            row.insert("pitch_alt".into(), first_word(alt)?.into());
        }
        row.insert("category".into(), w(MATERIAL_CATEGORY, 0)?.into());
        row.insert("landing".into(), (w(LANDING_FLAG, 0)? != 0).into());
        row.insert("footsteps".into(), (w(MATERIAL_FOOTSTEPS, 0)? != 0).into());
        row.insert("step_gain".into(), w(MATERIAL_STEP_GAIN, 32767)?.into());
        row.insert(
            "step_landing_gain".into(),
            w(MATERIAL_STEP_LANDING_GAIN, 32767)?.into(),
        );
        let windows_key = ref_key(resolve(MATERIAL_CLASS, &key, MATERIAL_WINDOWS));
        if let Some(wk) = windows_key.filter(|wk| has(WINDOW_CLASS, wk)) {
            let windows = WINDOW_FIELDS
                .iter()
                .map(|f| word(WINDOW_CLASS, &wk, f, 0).map(Value::from))
                .collect::<Result<Vec<_>, _>>()?;
            row.insert("windows".into(), Value::Array(windows));
            let scale = match resolve(WINDOW_CLASS, &wk, WINDOW_SCALE) {
                Some(s) => scalar_or_list(s)?,
                None => float(0.0),
            };
            row.insert("scale".into(), scale);
        }
        let bands_key = ref_key(resolve(MATERIAL_CLASS, &key, MATERIAL_BANDS));
        if let Some(bk) = bands_key.filter(|bk| has(BAND_CLASS, bk)) {
            let bands = BAND_FIELDS
                .iter()
                .map(|f| match resolve(BAND_CLASS, &bk, f) {
                    Some(v) => scalar_or_list(v),
                    None => Ok(float(0.0)),
                })
                .collect::<Result<Vec<_>, _>>()?;
            row.insert("bands".into(), Value::Array(bands));
        }
        rows.push(Value::Object(row));
    }
    let mut posters = Map::new();
    for (cls, key, field, name) in COLLISION_POSTERS {
        if let Some(v) = resolve(cls, key, field) {
            posters.insert(name.into(), scalar_or_list(v)?);
        }
    }
    for (name, list) in [("plant_ids", PLANT_FIELDS), ("lift_ids", LIFT_FIELDS)] {
        let values: Vec<_> = list
            .iter()
            .map(|f| resolve(WHEEL_CLASS, "default", f))
            .collect();
        if values.iter().all(Option::is_some) {
            let ids = values
                .iter()
                .map(|v| first_word(v.unwrap()).map(Value::from))
                .collect::<Result<Vec<_>, _>>()?;
            posters.insert(name.into(), Value::Array(ids));
        }
    }
    if let Some(v) = resolve(EQCHAIN_CLASS, "default", PLANT_EQ) {
        posters.insert("plant_eq".into(), first_word(v)?.into());
    }
    for (field, name) in BODY_POSTERS {
        if let Some(v) = resolve(CONTACTS_CLASS, "default", field) {
            posters.insert(name.into(), scalar_or_list(v)?);
        }
    }
    if let Some(v) = resolve(CONTACTS_CLASS, "default", BODY_SPEED_GRAPH) {
        let f = floats(v)?;
        if f.len() >= 20 {
            posters.insert("body_speed_x".into(), floats_value(f[4..12].to_vec()));
            posters.insert("body_speed_y".into(), floats_value(f[12..20].to_vec()));
        }
    }
    Ok(json!({"materials": rows, "posters": posters}))
}

// ---- bus_tuning.

const REVERB_CLASS: &str = "Hash_204CAC1FD77088B8";
const REVERB_FIELDS: [&str; 44] = [
    "48376C6D695CDDBB",
    "4E7CDD2F6C41296F",
    "22FB428A30D3602C",
    "66588E403F254F64",
    "8968075E7CBACC4E",
    "FA81EA055905EB17",
    "2490987C531FB4E9",
    "5075CFC3C70ACB30",
    "1812369BC788F71F",
    "D548E99383500988",
    "4C50D4334042E446",
    "4ABF29854AA8167A",
    "3F9A78465CFFC183",
    "578AB8D6BF494236",
    "C8A4239F7CBBF072",
    "B3BB9F5129BBCFB5",
    "2EBE3CBF5E339625",
    "533ED00D3783EF02",
    "8249454FFE32EA2B",
    "DECBDDB58AC61519",
    "2DA3FDBA90F64924",
    "C76E3B36EC05F9E0",
    "27D28928F58346B1",
    "30DCBFB636929C6A",
    "103E1FD179ADA674",
    "F38B7E9560E0AA84",
    "C7F29E4229F528B6",
    "3E80A0539763F3DF",
    "EDA35434B3617A0A",
    "024737095437438A",
    "E9E26ECCA2D28D11",
    "81504B05719020A0",
    "433B43350289C6CE",
    "FC50736430148B30",
    "35B87D3598BAD8E5",
    "58C65800CA17ACD9",
    "9BAA927DA7AA8970",
    "7492F9438252B839",
    "843273C75A0984F5",
    "7B2416B80E3BA149",
    "D6812FA712AACD13",
    "796D8327E0995ECC",
    "B388DAFB738410FC",
    "EECADFA12642D9F3",
];
const EQ_BUS_CLASS: &str = "Hash_AA801D9FC0ADBBBF";
const FLANGE_CLASS: &str = "Hash_4CDD7CDC1A955D5C";
const FLANGE_KEYS: [&str; 2] = ["C6290FFCB9E84CD9", "DFDFFAD67CCBA322"];
const FLANGE_FIELDS: [&str; 9] = [
    "DE9BF5C10AB2C866",
    "113F78E17495296F",
    "3C0376079AD9A56C",
    "7D3294EC1C443CBD",
    "CD44B25FAD9E48D9",
    "EDEE41D561FA8CE5",
    "9C890EC62D3BC5C6",
    "D10D7F661896AC30",
    "9024FC56E2314F9F",
];
const EQ_BUS_KEYS: [&str; 8] = [
    "42DDFA3F5011BAB4",
    "DA24A51DDC51EDF3",
    "201C88714085E3C3",
    "8AF23914D89E4AF0",
    "F7CFA0A8D2F98BDE",
    "8AA93567C056736B",
    "87D5F21D9A71736A",
    "BFA2D9D8C138F925",
];
const EQ_BUS_ENABLE: &str = "Hash_C6DA68C12A3822D2";
const EQ_BUS_CLIP: &str = "Hash_2086A0CE99C39A86";
const EQ_BUS_RANGES: [(&str, &str); 6] = [
    ("5B48CBEB3EB440D2", "2B77D4F58AF71CCF"),
    ("7317CC02098BDFF6", "90A94DDDA5F804CB"),
    ("F30B7500D67E206B", "B4190B5C42432FA2"),
    ("3264F689300A522A", "1DA611DA4DE5A67A"),
    ("54868EDE952EF581", "25853859640A3796"),
    ("594D91B30B95483A", "CDF599CDEC9C7C64"),
];

/// bus_tuning's resolve: the parent chain, then the class's `default` record
/// unless the walk ended on a key named `default`.
fn resolve_or_default<'a>(records: &Records<'a>, key: &str, field: &str) -> Option<&'a Value> {
    let mut key = key;
    let mut seen = 0;
    while seen < 32 {
        let Some(record) = records.get(key) else {
            break;
        };
        if let Some(f) = fields(record).get(field) {
            return Some(f);
        }
        key = parent(record);
        seen += 1;
    }
    if key != "default"
        && let Some(d) = records.get("default")
    {
        return fields(d).get(field);
    }
    None
}

/// `{reverb: {key: [44 values]}, eq_buses: [8 x {enabled, clip, ranges}],
/// flange: [[9], [9]]}` (`bus_tuning`).
pub fn bus_tuning(collections: &[Value]) -> Result<Value, Error> {
    let classes = Classes::new(collections);
    let mut out = Map::new();
    if let Some(reverb) = classes.class(REVERB_CLASS) {
        let mut presets = Map::new();
        for key in &reverb.order {
            if !key.starts_with("Hash_") {
                continue;
            }
            let mut row = Vec::with_capacity(44);
            for (i, h) in REVERB_FIELDS.iter().enumerate() {
                let v = resolve_or_default(reverb, key, &format!("Hash_{h}"));
                row.push(match v {
                    None => Value::from(0),
                    Some(v) if i == 12 => Value::from(word_of(v)?),
                    Some(v) => exact(v)?,
                });
            }
            presets.insert(key[5..].to_owned(), Value::Array(row));
        }
        out.insert("reverb".into(), Value::Object(presets));
    }
    if let Some(eq) = classes.class(EQ_BUS_CLASS) {
        let mut buses = Vec::new();
        for key in EQ_BUS_KEYS {
            let k = format!("Hash_{key}");
            let enable = resolve_or_default(eq, &k, EQ_BUS_ENABLE);
            let clip = resolve_or_default(eq, &k, EQ_BUS_CLIP);
            let mut ranges = Vec::new();
            for (a, b) in EQ_BUS_RANGES {
                let va = resolve_or_default(eq, &k, &format!("Hash_{a}"));
                let vb = resolve_or_default(eq, &k, &format!("Hash_{b}"));
                let pick = |v: Option<&Value>| -> Result<Value, Error> {
                    if truthy(v) {
                        exact(v.unwrap())
                    } else {
                        Ok(float(0.0))
                    }
                };
                ranges.push(json!([pick(va)?, pick(vb)?]));
            }
            let enabled = truthy(enable) && ws(data_of(enable.unwrap())).starts_with("01");
            let clip = if truthy(clip) {
                exact(clip.unwrap())?
            } else {
                float(100.0)
            };
            buses.push(json!({"enabled": enabled, "clip": clip, "ranges": ranges}));
        }
        out.insert("eq_buses".into(), Value::Array(buses));
    }
    if let Some(flange) = classes.class(FLANGE_CLASS) {
        let mut returns = Vec::new();
        for key in FLANGE_KEYS {
            let k = format!("Hash_{key}");
            let values: Vec<_> = FLANGE_FIELDS
                .iter()
                .map(|f| resolve_or_default(flange, &k, &format!("Hash_{f}")))
                .collect();
            if values.iter().any(Option::is_none) {
                break;
            }
            returns.push(Value::Array(
                values
                    .iter()
                    .map(|v| exact(v.unwrap()))
                    .collect::<Result<_, _>>()?,
            ));
        }
        if returns.len() == 2 {
            out.insert("flange".into(), Value::Array(returns));
        }
    }
    Ok(Value::Object(out))
}

// ---- frontend_sounds.

const FE_CLASSES: [&str; 2] = ["fe", "Hash_5831CB95F3E90598"];
const FE_FIELDS: [(&str, &str); 7] = [
    ("Hash_8FCC7EF9B9208858", "id"),
    ("Hash_875BA75341DC8391", "level"),
    ("Hash_845A10052522A2FB", "hom"),
    ("Hash_A4080FB65880E3C2", "moment"),
    ("Hash_BF45D439FAC71A2E", "alt_bus"),
    ("Hash_942AB8AEE4B414ED", "name"),
    ("Name", "name"),
];
const FE_BANK: &str = "sk8_menu";

/// `_resolved`: records of one class by u64 key, inherited fields resolved
/// (16 steps at most), `wanted` field names renamed.
fn resolved(
    collections: &[Value],
    cls: &str,
    wanted: &[(&str, &str)],
) -> Result<BTreeMap<u64, Map<String, Value>>, Error> {
    let mut records = Records::default();
    for c in collections.iter().filter(|c| str_of(c, "class") == cls) {
        records.insert(str_of(c, "key"), c);
    }
    let mut out = BTreeMap::new();
    for key in &records.order {
        let mut chain = Vec::new();
        let mut cursor: &str = key;
        while let Some(record) = records.get(cursor) {
            if chain.len() >= 16 {
                break;
            }
            chain.push(record);
            cursor = parent(record);
        }
        let mut found = Map::new();
        for record in chain.iter().rev() {
            for (name, field) in fields(record) {
                if let Some((_, to)) = wanted.iter().find(|(from, _)| from == name) {
                    found.insert((*to).to_owned(), value(field)?);
                }
            }
        }
        let id = match key.strip_prefix("Hash_") {
            Some(hex) => hex_u64(hex)?,
            None => hash64(key),
        };
        out.insert(id, found);
    }
    Ok(out)
}

fn py_int(v: &Value) -> Result<i64, Error> {
    match v {
        Value::Bool(b) => Ok(i64::from(*b)),
        Value::Number(n) => n
            .as_i64()
            .or_else(|| n.as_f64().map(|x| x.trunc() as i64))
            .ok_or(Error::Format("vault number out of range")),
        Value::String(s) => s
            .trim()
            .parse()
            .map_err(|_| Error::Format("vault text is not an integer")),
        _ => Err(Error::Format("vault value is not an integer")),
    }
}

fn py_float(v: &Value) -> Result<f64, Error> {
    match v {
        Value::Bool(b) => Ok(f64::from(u8::from(*b))),
        Value::Number(n) => n.as_f64().ok_or(Error::Format("vault number out of range")),
        Value::String(s) => s
            .trim()
            .parse()
            .map_err(|_| Error::Format("vault text is not a number")),
        _ => Err(Error::Format("vault value is not a number")),
    }
}

/// `{bank, sounds: {key: {name, id, level, hom, moment, alt_bus}}}` for every
/// front-end record; `{}` when the database has no such class.
pub fn frontend_sounds(collections: &[Value]) -> Result<Value, Error> {
    let Some(cls) = FE_CLASSES
        .into_iter()
        .find(|c| collections.iter().any(|r| str_of(r, "class") == *c))
    else {
        return Ok(json!({}));
    };
    let mut sounds = Map::new();
    for (key, f) in resolved(collections, cls, &FE_FIELDS)? {
        let int = |name: &str| f.get(name).map_or(Ok(0), py_int);
        sounds.insert(
            format!("{key:016X}"),
            json!({
                "name": f.get("name").cloned().unwrap_or_else(|| Value::from("")),
                "id": int("id")?,
                "level": float(f.get("level").map_or(Ok(1.0), py_float)?),
                "hom": int("hom")?,
                "moment": int("moment")?,
                "alt_bus": f.get("alt_bus").is_some_and(truthy_value),
            }),
        );
    }
    Ok(json!({"bank": FE_BANK, "sounds": sounds}))
}

#[cfg(test)]
pub(super) mod probe {
    //! Field readers exposed to the tests.
    pub(crate) use super::{exact, floats, scalar_or_list, value, word_of, words};
}
