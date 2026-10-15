//! Containers around Skate 3's EA SNR (EAAC) streams, the SPLC patch tree,
//! PCM16 WAV files and the grain loop bands
//! (tools/asset_pipeline/audio_formats.py, audio_export.py's WAV helpers).
//!
//! - SNR header: u32 version(4) | codec(4) | channels-1(6) | sample rate(18);
//!   u32 type(2) | loop(1) | sample count(29); u32 loop start when looping.
//!   RAM streams (type 0) are followed by blocks: u8 flag (0x00, or 0x80 =
//!   last), u24 block size (incl. this 8-byte header), u32 samples in the block.
//! - SPLC `.bnk`: u32 stream count at 0x18, parameter records, then that many
//!   SNR streams back to back at unaligned offsets.
//! - `.grain`: u32 offset of the SNR stream, f32 duration in seconds, a seek
//!   table, then one SNR stream.
use crate::{Error, be16, be32};
use serde_json::{Map, Value, json};

/// The codec the disc's RAM streams use (EA-XMA).
pub const EA_XMA: u32 = 3;

fn bad(why: String) -> Error {
    Error::Io(std::io::Error::other(why))
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SnrStream {
    pub offset: usize,
    pub end: usize,
    pub codec: u32,
    pub channels: u32,
    pub sample_rate: u32,
    pub samples: u32,
    pub loop_start: Option<u32>,
}

impl SnrStream {
    pub fn seconds(&self) -> f64 {
        f64::from(self.samples) / f64::from(self.sample_rate)
    }
}

fn u32_at(data: &[u8], at: usize) -> u32 {
    u32::from_be_bytes(data[at..at + 4].try_into().unwrap())
}

/// The RAM SNR stream (EA-XMA) starting at `offset`, if its block chain is
/// consistent (`snr_at`).
pub fn snr_at(data: &[u8], offset: usize) -> Option<SnrStream> {
    if offset.checked_add(16)? > data.len() {
        return None;
    }
    let header = u32_at(data, offset);
    let (version, codec) = (header >> 28, (header >> 24) & 0xF);
    let (channels, rate) = (((header >> 18) & 0x3F) + 1, header & 0x3FFFF);
    if version > 1 || codec != EA_XMA || !(8000..=48000).contains(&rate) || channels > 8 {
        return None;
    }
    let flags = u32_at(data, offset + 4);
    let (kind, looped, samples) = (flags >> 30, (flags >> 29) & 1, flags & 0x1FFF_FFFF);
    if kind != 0 || samples == 0 {
        return None;
    }
    let loop_start = (looped == 1).then(|| u32_at(data, offset + 8));
    let mut cursor = offset + if looped == 1 { 12 } else { 8 };
    let mut decoded: u64 = 0;
    while decoded < u64::from(samples) {
        if cursor + 8 > data.len() {
            return None;
        }
        let flag = data[cursor];
        let size = (u32_at(data, cursor) & 0xFF_FFFF) as usize;
        let block_samples = u32_at(data, cursor + 4);
        if (flag != 0x00 && flag != 0x80)
            || size <= 8
            || cursor + size > data.len()
            || block_samples == 0
        {
            return None;
        }
        decoded += u64::from(block_samples);
        cursor += size;
        if flag == 0x80 {
            break;
        }
    }
    if decoded != u64::from(samples) {
        return None;
    }
    Some(SnrStream {
        offset,
        end: cursor,
        codec,
        channels,
        sample_rate: rate,
        samples,
        loop_start,
    })
}

/// Every consistent SNR stream in `data`, in file order (`scan_snr`).
pub fn scan_snr(data: &[u8], start: usize) -> Vec<SnrStream> {
    let mut streams = Vec::new();
    let mut offset = start;
    while offset + 16 <= data.len() {
        match snr_at(data, offset) {
            None => offset += 1,
            Some(stream) => {
                offset = stream.end;
                streams.push(stream);
            }
        }
    }
    streams
}

/// An SPLC bank's streams; their number must be the declared count.
pub fn splc_streams(data: &[u8]) -> Result<Vec<SnrStream>, Error> {
    if data.get(..4) != Some(b"SPLC".as_slice()) {
        return Err(Error::Format("not an SPLC bank"));
    }
    let expected = be32(data, 0x18)? as usize;
    let streams = scan_snr(data, 0);
    if streams.len() != expected {
        return Err(bad(format!(
            "SPLC bank declares {expected} streams; found {}",
            streams.len()
        )));
    }
    Ok(streams)
}

/// A `.grain` member's stored duration and stream (`grain`).
#[derive(Clone, Copy, Debug)]
pub struct Grain {
    pub duration: f64,
    pub stream: SnrStream,
}

pub fn grain(data: &[u8]) -> Result<Grain, Error> {
    if data.len() < 8 {
        return Err(Error::Format("truncated grain file"));
    }
    let offset = u32_at(data, 0) as usize;
    let duration = f64::from(f32::from_bits(u32_at(data, 4)));
    let stream = snr_at(data, offset)
        .ok_or_else(|| bad(format!("grain file has no SNR stream at {offset:#x}")))?;
    if (stream.seconds() - duration).abs() > 0.01 {
        return Err(bad(format!(
            "grain duration {duration:.3}s does not match its stream ({:.3}s)",
            stream.seconds()
        )));
    }
    Ok(Grain { duration, stream })
}

/// The stream as a standalone `.snr` file (header followed by its blocks).
pub fn standalone<'a>(data: &'a [u8], stream: &SnrStream) -> &'a [u8] {
    &data[stream.offset..stream.end]
}

/// A bank's streams: an SPLC bank's declared ones, else every SNR stream.
pub fn bank_streams(name: &str, data: &[u8]) -> Result<Vec<SnrStream>, Error> {
    let streams = if data.get(..4) == Some(b"SPLC".as_slice()) {
        splc_streams(data)?
    } else {
        scan_snr(data, 0)
    };
    if streams.is_empty() {
        return Err(bad(format!("{name} contains no decodable streams")));
    }
    Ok(streams)
}

fn f32_at(data: &[u8], at: usize) -> Result<f64, Error> {
    Ok(f64::from(f32::from_bits(be32(data, at)?)))
}

fn byte(data: &[u8], at: usize) -> Result<u8, Error> {
    data.get(at)
        .copied()
        .ok_or(Error::Format("truncated SPLC bank"))
}

/// The patch tree of an SPLC bank: what retail plays for each sound id
/// (`splc_patches`; layout in audio_formats.py). `{records, containers}`.
pub fn splc_patches(data: &[u8]) -> Result<Value, Error> {
    if data.get(..4) != Some(b"SPLC".as_slice()) {
        return Err(Error::Format("not an SPLC bank"));
    }
    let table = 60 + be32(data, 8)? as usize;
    let records = be32(data, 12)? as usize;
    let containers = be32(data, 16)? as usize;
    let samples = be32(data, 24)?;
    if be32(data, 20)? != 0 {
        return Err(Error::Format(
            "SPLC bank with 24-byte extras is not supported",
        ));
    }
    if records > data.len() / 36 || containers > data.len() / 72 {
        return Err(Error::Format("truncated SPLC bank"));
    }
    let container_base = 60 + 36 * records;
    let mut out_containers = Vec::with_capacity(containers);
    for c in 0..containers {
        let at = container_base + 72 * c;
        let count = byte(data, at + 68)? as usize;
        let ids = (0..count)
            .map(|k| be16(data, at + 4 + 2 * k).map(Value::from))
            .collect::<Result<Vec<_>, _>>()?;
        out_containers.push(Value::Array(ids));
    }
    let mut cursor = container_base + 72 * containers;
    let mut out_records = Vec::with_capacity(records);
    for r in 0..records {
        let at = 60 + 36 * r;
        let id = be16(data, at + 4)?;
        if usize::from(id) != r {
            return Err(bad(format!("SPLC record {r} has id {id}")));
        }
        let mut groups = Vec::new();
        for _ in 0..byte(data, at + 7)? {
            let (count, mode) = (byte(data, cursor + 8)?, byte(data, cursor + 9)?);
            cursor += 12;
            let mut members = Vec::with_capacity(count.into());
            for _ in 0..count {
                let sample = be16(data, cursor)?;
                if u32::from(sample) >= samples {
                    return Err(bad(format!(
                        "SPLC member names sample {sample} of {samples}"
                    )));
                }
                let gain = f32_at(data, cursor + 8)?;
                let pitch = f32_at(data, cursor + 44)?;
                let gain_range = f32_at(data, cursor + 48)?;
                let probability = f32_at(data, cursor + 64)?;
                members.push(json!([
                    sample,
                    float(py_round(gain, 4)),
                    float(py_round(gain_range, 4)),
                    float(py_round(pitch, 4)),
                    float(py_round(probability, 4)),
                ]));
                cursor += 72;
            }
            groups.push(json!({"mode": mode, "members": members}));
        }
        out_records.push(Value::Array(groups));
    }
    if cursor != table {
        return Err(bad(format!(
            "SPLC patch tree ends at {cursor:#x}, sample table at {table:#x}"
        )));
    }
    Ok(json!({"records": out_records, "containers": out_containers}))
}

/// The SPLC patch tree's length: everything before the sample table.
pub fn splc_tree_len(data: &[u8]) -> Result<usize, Error> {
    let tree = 60 + be32(data, 8)? as usize;
    if tree > data.len() {
        return Err(Error::Format("SPLC sample table is past the bank"));
    }
    Ok(tree)
}

/// Python's `round(x, digits)`: the decimal nearest to the double's exact
/// value, ties to even, read back as a double.
pub fn py_round(x: f64, digits: usize) -> f64 {
    if !x.is_finite() {
        return x;
    }
    format!("{x:.digits$}").parse().unwrap_or(x)
}

/// A double as JSON (NaN and infinities, which Python would write as bare
/// `NaN` / `Infinity`, become null).
pub fn float(x: f64) -> Value {
    serde_json::Number::from_f64(x).map_or(Value::Null, Value::Number)
}

/// A double as Python's `repr` writes it: the shortest round-trip digits,
/// fixed notation for decimal exponents -5 < e < 16 (with `.0` when whole),
/// else `d.ddde±XX`.
pub fn python_float(x: f64) -> String {
    if x.is_nan() {
        return "NaN".into();
    }
    if x.is_infinite() {
        return if x > 0.0 { "Infinity" } else { "-Infinity" }.into();
    }
    if x == 0.0 {
        return if x.is_sign_negative() { "-0.0" } else { "0.0" }.into();
    }
    // `{:e}` gives the shortest round-trip digits: "-1.2345e-5".
    let sci = format!("{x:e}");
    let (mantissa, exponent) = sci.split_once('e').unwrap();
    let exponent: i32 = exponent.parse().unwrap();
    let (sign, mantissa) = match mantissa.strip_prefix('-') {
        Some(m) => ("-", m),
        None => ("", mantissa),
    };
    let digits: String = mantissa.chars().filter(|c| *c != '.').collect();
    let decpt = exponent + 1;
    if (-4 < decpt) && (decpt <= 16) {
        let n = digits.len() as i32;
        let body = if decpt <= 0 {
            format!("0.{}{digits}", "0".repeat((-decpt) as usize))
        } else if decpt >= n {
            format!("{digits}{}.0", "0".repeat((decpt - n) as usize))
        } else {
            format!(
                "{}.{}",
                &digits[..decpt as usize],
                &digits[decpt as usize..]
            )
        };
        format!("{sign}{body}")
    } else {
        let mantissa = if digits.len() > 1 {
            format!("{}.{}", &digits[..1], &digits[1..])
        } else {
            digits
        };
        let esign = if exponent < 0 { '-' } else { '+' };
        format!("{sign}{mantissa}e{esign}{:02}", exponent.abs())
    }
}

fn python_string(s: &str, out: &mut String) {
    out.push('"');
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            '\u{8}' => out.push_str("\\b"),
            '\u{c}' => out.push_str("\\f"),
            c if (c as u32) < 0x20 || (c as u32) > 0x7E => {
                let mut units = [0u16; 2];
                for unit in c.encode_utf16(&mut units) {
                    out.push_str(&format!("\\u{unit:04x}"));
                }
            }
            c => out.push(c),
        }
    }
    out.push('"');
}

fn python_value(v: &Value, depth: usize, out: &mut String) {
    let indent = |out: &mut String, depth: usize| {
        out.push('\n');
        out.push_str(&" ".repeat(depth));
    };
    match v {
        Value::Null => out.push_str("null"),
        Value::Bool(b) => out.push_str(if *b { "true" } else { "false" }),
        Value::Number(n) => match (n.as_i64(), n.as_u64()) {
            (Some(i), _) => out.push_str(&i.to_string()),
            (_, Some(u)) => out.push_str(&u.to_string()),
            _ => out.push_str(&python_float(n.as_f64().unwrap_or(f64::NAN))),
        },
        Value::String(s) => python_string(s, out),
        Value::Array(items) if items.is_empty() => out.push_str("[]"),
        Value::Array(items) => {
            out.push('[');
            for (i, item) in items.iter().enumerate() {
                if i > 0 {
                    out.push(',');
                }
                indent(out, depth + 1);
                python_value(item, depth + 1, out);
            }
            indent(out, depth);
            out.push(']');
        }
        Value::Object(map) if map.is_empty() => out.push_str("{}"),
        Value::Object(map) => {
            out.push('{');
            for (i, (key, item)) in map.iter().enumerate() {
                if i > 0 {
                    out.push(',');
                }
                indent(out, depth + 1);
                python_string(key, out);
                out.push_str(": ");
                python_value(item, depth + 1, out);
            }
            indent(out, depth);
            out.push('}');
        }
    }
}

/// JSON as Python's `json.dumps(value, indent=1)` writes it (ASCII only,
/// Python float reprs); object keys in the map's order.
pub fn python_json(v: &Value) -> String {
    let mut out = String::new();
    python_value(v, 0, &mut out);
    out
}

/// A PCM WAV read the way Python's `wave` module reads it: the `fmt ` chunk,
/// then the first `data` chunk (frames = size / frame size).
pub struct Wav<'a> {
    pub channels: u16,
    pub sample_rate: u32,
    pub sample_width: u16,
    pub frames: usize,
    pub data: &'a [u8],
}

pub fn read_wav(bytes: &[u8]) -> Result<Wav<'_>, Error> {
    let bad_wav = Error::Format("not a PCM WAV file");
    if bytes.get(..4) != Some(b"RIFF".as_slice()) || bytes.get(8..12) != Some(b"WAVE".as_slice()) {
        return Err(bad_wav);
    }
    let le16 = |at: usize| {
        bytes
            .get(at..at + 2)
            .map(|b| u16::from_le_bytes([b[0], b[1]]))
    };
    let le32 = |at: usize| {
        bytes
            .get(at..at + 4)
            .map(|b| u32::from_le_bytes(b.try_into().unwrap()))
    };
    let mut cursor = 12;
    let mut format: Option<(u16, u32, u16)> = None;
    while cursor + 8 <= bytes.len() {
        let name = &bytes[cursor..cursor + 4];
        let size = le32(cursor + 4).ok_or(Error::Format("truncated WAV chunk"))? as usize;
        let body = cursor + 8;
        if name == b"fmt " {
            let tag = le16(body).ok_or(Error::Format("truncated WAV format"))?;
            if tag != 1 && tag != 0xFFFE {
                return Err(Error::Format("WAV is not PCM"));
            }
            let channels = le16(body + 2).ok_or(Error::Format("truncated WAV format"))?;
            let rate = le32(body + 4).ok_or(Error::Format("truncated WAV format"))?;
            let bits = le16(body + 14).ok_or(Error::Format("truncated WAV format"))?;
            format = Some((channels, rate, bits.div_ceil(8)));
        } else if name == b"data" {
            let (channels, sample_rate, sample_width) =
                format.ok_or(Error::Format("WAV data before its format"))?;
            let frame = usize::from(channels) * usize::from(sample_width);
            if frame == 0 || sample_rate == 0 {
                return Err(Error::Format("WAV with an empty format"));
            }
            // Frames count the declared size, as `wave` does; the data is
            // what the file holds of it.
            let frames = size / frame;
            let end = (body + frames * frame).min(bytes.len());
            return Ok(Wav {
                channels,
                sample_rate,
                sample_width,
                frames,
                data: &bytes[body..end],
            });
        }
        cursor = body + size + (size & 1);
    }
    Err(bad_wav)
}

/// A PCM16 WAV as Python's `wave` module writes it (44-byte header).
pub fn wav_bytes(channels: u16, sample_rate: u32, pcm: &[i16]) -> Vec<u8> {
    let data_len = (pcm.len() * 2) as u32;
    let mut out = Vec::with_capacity(44 + pcm.len() * 2);
    out.extend_from_slice(b"RIFF");
    out.extend_from_slice(&(36 + data_len).to_le_bytes());
    out.extend_from_slice(b"WAVEfmt ");
    out.extend_from_slice(&16u32.to_le_bytes());
    out.extend_from_slice(&1u16.to_le_bytes());
    out.extend_from_slice(&channels.to_le_bytes());
    out.extend_from_slice(&sample_rate.to_le_bytes());
    out.extend_from_slice(&(sample_rate * u32::from(channels) * 2).to_le_bytes());
    out.extend_from_slice(&(channels * 2).to_le_bytes());
    out.extend_from_slice(&16u16.to_le_bytes());
    out.extend_from_slice(b"data");
    out.extend_from_slice(&data_len.to_le_bytes());
    for s in pcm {
        out.extend_from_slice(&s.to_le_bytes());
    }
    out
}

/// `_wav_info`: channels, sample rate and seconds (rounded to 4 places).
pub fn wav_info(wav: &Wav<'_>) -> Map<String, Value> {
    let mut m = Map::new();
    m.insert("channels".into(), wav.channels.into());
    m.insert("sample_rate".into(), wav.sample_rate.into());
    let seconds = wav.frames as f64 / f64::from(wav.sample_rate);
    m.insert("seconds".into(), float(py_round(seconds, 4)));
    m
}

/// Mono PCM16 cut into `bands` equal parts, each made seamless as a loop
/// (`loop_bands`): a part's last `crossfade` samples are blended into its
/// first ones with linear weights, then dropped. Float32 math as numpy does
/// it (the weights are float64 `i / crossfade` rounded to float32).
pub fn loop_bands(samples: &[i16], bands: usize, crossfade: usize) -> Result<Vec<Vec<i16>>, Error> {
    let size = samples.len() / bands;
    if size <= 2 * crossfade {
        return Err(Error::Format("grain recording too short for its bands"));
    }
    let step = 1.0f64 / crossfade as f64;
    let fade: Vec<f32> = (0..crossfade).map(|i| (i as f64 * step) as f32).collect();
    let mut out = Vec::with_capacity(bands);
    for index in 0..bands {
        let mut part: Vec<f32> = samples[index * size..(index + 1) * size]
            .iter()
            .map(|&s| f32::from(s))
            .collect();
        let tail_at = size - crossfade;
        for (i, &w) in fade.iter().enumerate() {
            let (head, tail) = (part[i], part[tail_at + i]);
            part[i] = head * w + tail * (1.0f32 - w);
        }
        part.truncate(tail_at);
        out.push(part.iter().map(|&x| x.round_ties_even() as i16).collect());
    }
    Ok(out)
}
