//! Synthetic bytes only.
use super::formats::*;
use super::tuning::{self, probe};
use super::*;
use crate::tests::{Bytes, big};

/// An EA-XMA RAM stream: `blocks` of (samples, payload bytes), the last
/// flagged 0x80.
fn snr(rate: u32, blocks: &[(u32, usize)], loop_start: Option<u32>) -> Vec<u8> {
    let samples: u32 = blocks.iter().map(|b| b.0).sum();
    let mut out = Vec::new();
    out.extend_from_slice(&((3u32 << 24) | rate).to_be_bytes());
    let looped = u32::from(loop_start.is_some()) << 29;
    out.extend_from_slice(&(looped | samples).to_be_bytes());
    if let Some(l) = loop_start {
        out.extend_from_slice(&l.to_be_bytes());
    }
    for (i, &(n, payload)) in blocks.iter().enumerate() {
        let flag = if i + 1 == blocks.len() { 0x80u32 } else { 0 };
        out.extend_from_slice(&((flag << 24) | (8 + payload) as u32).to_be_bytes());
        out.extend_from_slice(&n.to_be_bytes());
        out.extend(std::iter::repeat_n(0xAB, payload));
    }
    out
}

#[test]
fn python_rounding_and_reprs() {
    assert_eq!(py_round(0.03125, 4), 0.0312); // an exact tie goes to even
    assert_eq!(py_round(0.09375, 4), 0.0938);
    assert_eq!(py_round(2.675, 2), 2.67); // the double is below the tie
    assert_eq!(py_round(0.5, 0), 0.0);
    assert_eq!(py_round(1.5, 0), 2.0);
    for (x, text) in [
        (1.0, "1.0"),
        (1e-5, "1e-05"),
        (1e16, "1e+16"),
        (1234567890123456.0, "1234567890123456.0"),
        (f64::from(0.1f32), "0.10000000149011612"),
        (-0.00012, "-0.00012"),
        (0.0001, "0.0001"),
        (123456.5, "123456.5"),
        (-2.5e-7, "-2.5e-07"),
        (3.0e20, "3e+20"),
    ] {
        assert_eq!(python_float(x), text, "{x}");
    }
    let v = json!({"b": [1, 2.5, "\u{e9}"], "a": {}, "c": []});
    assert_eq!(
        python_json(&v),
        "{\n \"a\": {},\n \"b\": [\n  1,\n  2.5,\n  \"\\u00e9\"\n ],\n \"c\": []\n}"
    );
}

#[test]
fn snr_streams_are_found_by_their_block_chains() {
    let a = snr(32000, &[(100, 8), (200, 4)], None);
    let b = snr(48000, &[(50, 3)], Some(7));
    let mut data = vec![1, 2, 3];
    data.extend_from_slice(&a);
    data.extend_from_slice(&[0; 5]);
    data.extend_from_slice(&b);
    data.extend_from_slice(&[0; 20]);
    let streams = scan_snr(&data, 0);
    assert_eq!(streams.len(), 2);
    assert_eq!(streams[0].offset, 3);
    assert_eq!(streams[0].end, 3 + a.len());
    assert_eq!((streams[0].channels, streams[0].sample_rate), (1, 32000));
    assert_eq!(streams[0].samples, 300);
    assert_eq!(streams[1].loop_start, Some(7));
    assert_eq!(standalone(&data, &streams[1]), b.as_slice());
    // A block chain that does not add up is not a stream.
    let mut broken = snr(32000, &[(100, 8)], None);
    broken[7] = 101;
    assert!(snr_at(&broken, 0).is_none());
    // Codecs other than EA-XMA are skipped.
    let mut pcm = a.clone();
    pcm[0] = 0x04;
    assert!(snr_at(&pcm, 0).is_none());
}

#[test]
fn grain_members_check_their_stored_duration() {
    let stream = snr(8000, &[(4000, 8), (4000, 8)], None);
    let mut member = Vec::new();
    member.extend_from_slice(&16u32.to_be_bytes());
    member.extend_from_slice(&1.0f32.to_be_bytes());
    member.extend_from_slice(&[0; 8]); // seek table
    member.extend_from_slice(&stream);
    let g = grain(&member).unwrap();
    assert_eq!(g.duration, 1.0);
    assert_eq!(g.stream.offset, 16);
    member[4..8].copy_from_slice(&1.5f32.to_be_bytes());
    assert!(grain(&member).is_err());
}

/// One record, one container, one group with one member; two samples.
fn splc(sample_count: u32, streams: &[Vec<u8>]) -> Vec<u8> {
    let mut d = vec![0u8; 252];
    d[..4].copy_from_slice(b"SPLC");
    d[8..12].copy_from_slice(&(252u32 - 60).to_be_bytes());
    d[12..16].copy_from_slice(&1u32.to_be_bytes());
    d[16..20].copy_from_slice(&1u32.to_be_bytes());
    d[24..28].copy_from_slice(&sample_count.to_be_bytes());
    d[60 + 7] = 1; // record 0: one group
    d[96 + 4..96 + 6].copy_from_slice(&0u16.to_be_bytes());
    d[96 + 68] = 1;
    d[96 + 69] = 3;
    d[168 + 8] = 1; // group: one member, mode 2
    d[168 + 9] = 2;
    let m = 180;
    d[m..m + 2].copy_from_slice(&1u16.to_be_bytes());
    d[m + 8..m + 12].copy_from_slice(&0.5f32.to_be_bytes());
    d[m + 44..m + 48].copy_from_slice(&1.25f32.to_be_bytes());
    d[m + 48..m + 52].copy_from_slice(&0.1f32.to_be_bytes());
    d[m + 64..m + 68].copy_from_slice(&0.33333f32.to_be_bytes());
    let declared = streams.len() as u32;
    d[0x18..0x1C].copy_from_slice(&declared.max(sample_count).to_be_bytes());
    for s in streams {
        d.extend_from_slice(s);
    }
    d
}

#[test]
fn splc_patch_trees_and_streams() {
    let streams = [snr(32000, &[(10, 4)], None), snr(32000, &[(20, 4)], None)];
    let bank = splc(2, &streams);
    let tree = splc_patches(&bank).unwrap();
    assert_eq!(
        tree,
        json!({"records": [[{"mode": 2, "members": [[1, 0.5, 0.1, 1.25, 0.3333]]}]], "containers": [[0]]})
    );
    assert_eq!(splc_tree_len(&bank).unwrap(), 252);
    assert_eq!(splc_streams(&bank).unwrap().len(), 2);
    assert_eq!(bank_streams("x", &bank).unwrap().len(), 2);
    // A member naming a sample past the table is refused.
    let mut past = bank.clone();
    past[180..182].copy_from_slice(&2u16.to_be_bytes());
    assert!(splc_patches(&past).is_err());
    assert!(bank_streams("x", b"ABKC\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0").is_err());
}

#[test]
fn wav_files_round_trip_as_python_writes_them() {
    let bytes = wav_bytes(1, 32000, &[1, -2, 3]);
    assert_eq!(bytes.len(), 44 + 6);
    assert_eq!(&bytes[..4], b"RIFF");
    assert_eq!(u32::from_le_bytes(bytes[4..8].try_into().unwrap()), 42);
    let wav = read_wav(&bytes).unwrap();
    assert_eq!(
        (wav.channels, wav.sample_rate, wav.sample_width, wav.frames),
        (1, 32000, 2, 3)
    );
    assert_eq!(wav.data, &bytes[44..]);
    let info = wav_info(&wav);
    assert_eq!(info["seconds"], json!(0.0001)); // 3 / 32000 = 0.00009375
    // Chunks before `fmt ` / `data` (odd sizes padded) are skipped.
    let mut padded = bytes[..12].to_vec();
    padded.extend_from_slice(b"LIST\x03\0\0\0abc\0");
    padded.extend_from_slice(&bytes[12..]);
    assert_eq!(read_wav(&padded).unwrap().frames, 3);
}

#[test]
fn loop_bands_blend_tails_into_heads() {
    let samples: Vec<i16> = vec![10, 20, 30, 40, 51, 100, 200, 300, 400, 500, 7];
    let bands = loop_bands(&samples, 2, 2).unwrap();
    // fade = [0, 0.5]: s0*0 + s3*1, s1*0.5 + s4*0.5 = 35.5 -> 36 (ties to even).
    assert_eq!(bands, vec![vec![40, 36, 30], vec![400, 350, 300]]);
    assert!(loop_bands(&samples, 2, 3).is_err());
    // Weights are float64 i / n rounded to float32: 1/3 here.
    let samples: Vec<i16> = (0..14).map(|i| (i * 1000) as i16).collect();
    let bands = loop_bands(&samples, 2, 3).unwrap();
    let w = (1.0f64 / 3.0) as f32;
    let expect = (1000.0f32 * w + 5000.0f32 * (1.0 - w)).round_ties_even() as i16;
    assert_eq!(bands[0][1], expect);
}

fn field(kind: &str, data: &str) -> Value {
    json!({"type": kind, "data": data})
}

fn array(kind: &str, items: &[&str]) -> Value {
    json!({"type": kind, "data": "00", "array": {"items": items}})
}

#[test]
fn vault_field_readers_match_the_python_helpers() {
    let f = field("Float", "3F800000");
    assert_eq!(probe::exact(&f).unwrap(), json!(1.0));
    assert_eq!(
        probe::exact(&field("Float", "3F80000040000000")).unwrap(),
        json!([1.0, 2.0])
    );
    assert_eq!(
        probe::exact(&field("Int32", "FFFFFFFE")).unwrap(),
        json!(-2)
    );
    assert_eq!(probe::exact(&field("Bool", "01")).unwrap(), json!("01"));
    assert_eq!(
        probe::exact(&field("EA::Reflection::Text", "abc")).unwrap(),
        json!("abc")
    );
    assert_eq!(
        probe::exact(&array("Int32", &["00000001", "00000002"])).unwrap(),
        json!([1, 2])
    );
    assert_eq!(
        probe::floats(&field("Float", "3F80000040000000AB")).unwrap(),
        vec![1.0, 2.0]
    );
    assert_eq!(
        probe::words(&array("x", &["FFFFFFFF0000", "7F", ""])).unwrap(),
        vec![-1, 127]
    );
    assert_eq!(
        probe::scalar_or_list(&field("Float", "40400000")).unwrap(),
        json!(3.0)
    );
    assert_eq!(
        probe::scalar_or_list(&array("Int32", &["00000005"])).unwrap(),
        json!([5])
    );
    assert_eq!(probe::word_of(&field("Bool", "01")).unwrap(), 0x0100_0000);
    assert_eq!(probe::word_of(&field("Bool", "")).unwrap(), 0);
    // `_value`: floats rounded to 6 places, RefSpecs as keys.
    assert_eq!(
        probe::value(&field("Float", "3DCCCCCD")).unwrap(),
        json!(0.1)
    );
    assert_eq!(
        probe::value(&field(
            "Attrib::RefSpec",
            "0000000000000001000000000000002A"
        ))
        .unwrap(),
        json!(42)
    );
    assert_eq!(
        probe::value(&field("ClassRefSpec_x", "000000000000002B")).unwrap(),
        json!(43)
    );
    assert!(probe::value(&field("Bool", "01")).is_err());
}

fn record(class: &str, key: &str, parent: &str, fields: Value) -> Value {
    json!({"class": class, "key": key, "parent": parent, "fields": fields})
}

#[test]
fn grain_tuning_resolves_members_through_default() {
    let mut curve = String::new();
    for i in 0..16 {
        curve.push_str(&format!("{:08X}", (i as f32).to_bits()));
    }
    let c = vec![
        record(
            "Hash_7AB23C11B6ADA2DE",
            "default",
            "",
            json!({
                "Hash_4890392C91829954": field("Float", "41F00000"),
                "Hash_A985FBAA9326718D": field("Float", &curve),
                "Hash_D18D1174735E5CDE": array("x", &["3F800000", "40000000"]),
            }),
        ),
        record(
            "Hash_7AB23C11B6ADA2DE",
            "Hash_0000000000000001",
            "default",
            json!({"Hash_2C073BF8BC45063B": field("EA::Reflection::Text", "data/audio/concrete.grain"),
                   "Hash_5C9AA28695C17004": field("Float", "3F000000")}),
        ),
        record(
            "Hash_C1831BDB6CB1B1EA",
            "Hash_C489459A0C07D154",
            "",
            json!({"Hash_4CA607558B1CF440": array("x", &["0000000000000003", "0000000000000004"])}),
        ),
    ];
    let t = tuning::grain_tuning(&c).unwrap();
    let s = &t["surfaces"]["concrete"];
    assert_eq!(s["max_kmh"], json!(30.0));
    assert_eq!(s["turn_cap"], json!(0.5));
    assert_eq!(s["bezier"], json!([13.0, 9.0, 5.0, 1.0]));
    assert_eq!(s["params"], json!([[1.0], [2.0]]));
    assert!(t["default"].get("turn_cap").is_none());
    assert_eq!(t["surface_map"], json!([3, 4]));
    assert_eq!(t["owner"], json!({}));
}

#[test]
fn player_and_bus_tuning_sections() {
    let seam = format!("Hash_{:016X}", skate_data::attrib_hash::hash("slats"));
    let c = vec![
        record(
            "Hash_7242F32831ED3332",
            &seam,
            "",
            json!({"Hash_CA81764BF5A85E34": field("Int32", "00000002"),
                   "Hash_199170BB1C52EE64": field("Float", "3F800000")}),
        ),
        record(
            "Hash_D40CB4C0FFE45676",
            "head",
            "",
            json!({"Hash_1EBF9D2EB0DD56BA": field("Bool", "01000000"),
                   "Hash_875BA75341DC8391": field("Int32", "00000064")}),
        ),
        record(
            "Hash_204CAC1FD77088B8",
            "default",
            "",
            json!({"Hash_48376C6D695CDDBB": field("Float", "3F800000"),
                   "Hash_3F9A78465CFFC183": field("Int32", "00000007")}),
        ),
        record(
            "Hash_204CAC1FD77088B8",
            "Hash_00000000000000AA",
            "",
            json!({}),
        ),
        record(
            "fe",
            "Hash_0000000000000010",
            "",
            json!({"Hash_8FCC7EF9B9208858": field("Int32", "00000005"),
                   "Name": field("EA::Reflection::Text", "beep")}),
        ),
    ];
    let p = tuning::player_tuning(&c).unwrap();
    let wobbles = p["seam_wobbles"].as_array().unwrap();
    assert_eq!(wobbles.len(), 16);
    assert_eq!(wobbles[10], json!({"mode": 2, "gain": 1.0})); // `slats` is pattern 10
    let materials = p["collision"]["materials"].as_array().unwrap();
    assert_eq!(materials.len(), 143);
    // Material 0 is keyed by hash64("head"): the name-keyed record answers to it.
    let head = format!("Hash_{:016X}", skate_data::attrib_hash::hash("head"));
    let m = materials_with_key(&head);
    assert!(!m.is_empty());
    for &i in &m {
        assert_eq!(materials[i]["landing"], json!(true));
        assert_eq!(materials[i]["gain"], json!(100));
        assert!(materials[i].get("missing").is_none());
    }
    assert_eq!(p["landing_materials"], json!(m));
    let b = tuning::bus_tuning(&c).unwrap();
    let row = b["reverb"]["00000000000000AA"].as_array().unwrap();
    assert_eq!(row.len(), 44);
    assert_eq!(row[0], json!(1.0)); // from `default`
    assert_eq!(row[12], json!(7)); // the preset number, as a word
    assert_eq!(row[1], json!(0));
    let f = tuning::frontend_sounds(&c).unwrap();
    assert_eq!(f["bank"], json!("sk8_menu"));
    assert_eq!(
        f["sounds"]["0000000000000010"],
        json!({"name": "beep", "id": 5, "level": 1.0, "hom": 0, "moment": 0, "alt_bus": false})
    );
}

/// Indexes of the material table whose key is `key` (by the test's hash).
fn materials_with_key(key: &str) -> Vec<usize> {
    let p = tuning::player_tuning(&[record("Hash_D40CB4C0FFE45676", key, "", json!({}))]).unwrap();
    p["collision"]["materials"]
        .as_array()
        .unwrap()
        .iter()
        .enumerate()
        .filter(|(_, r)| r.get("missing").is_none())
        .map(|(i, _)| i)
        .collect()
}

/// A disc of named files (case-insensitive paths).
struct Disc(HashMap<String, Vec<u8>>);

impl DiscFiles for Disc {
    fn open(&mut self, path: &str) -> Result<Option<Box<dyn ReadAt + '_>>, Error> {
        Ok(self
            .0
            .get(&path.to_lowercase())
            .map(|b| Box::new(Bytes(b.clone())) as Box<dyn ReadAt>))
    }
    fn list(&mut self, _: &str) -> Result<Vec<String>, Error> {
        Ok(Vec::new())
    }
}

#[test]
fn prepares_the_manifest_layout_the_engine_reads() {
    let grain_stream = snr(8000, &[(8000, 16)], None);
    let mut member = 16u32.to_be_bytes().to_vec();
    member.extend_from_slice(&1.0f32.to_be_bytes());
    member.extend_from_slice(&[0; 8]);
    member.extend_from_slice(&grain_stream);
    let wheel = snr(32000, &[(320, 8)], None);
    let splc_bank = splc(
        2,
        &[snr(32000, &[(10, 4)], None), snr(32000, &[(20, 4)], None)],
    );
    let mut abk = b"ABKC".to_vec();
    abk.extend_from_slice(&[0; 12]);
    abk.extend_from_slice(&snr(22050, &[(30, 4)], None));
    let mut mixmap = vec![0u8; 24];
    mixmap[4..8].copy_from_slice(&2u32.to_be_bytes());
    mixmap[8..12].copy_from_slice(&16u32.to_be_bytes());
    // The BIG builder's name records hold 14 characters.
    let mut files = HashMap::new();
    files.insert(
        "data/audio/grains.big".to_owned(),
        big(&[
            ("grains", "concrete.grain", &member, 0, 0),
            ("grains", "x.txt", b"no", 0, 0),
        ]),
    );
    files.insert(
        "data/audio/wheels.big".to_owned(),
        big(&[("w", "Whl.snr", &wheel, 0, 0)]),
    );
    files.insert(
        "data/audio/audiofiles.big".to_owned(),
        big(&[
            ("data/audio", "Metal.bnk", &splc_bank, 0, 0),
            ("data/audio", "GRINDS.abk", &abk, 0, 0),
            ("data/audio", "Common.abk", &abk, 0, 0),
            ("data/audio", "Player.csi", b"csis", 0, 0),
        ]),
    );
    files.insert("data/audio/mixmapsk8.mxb".to_owned(), mixmap.clone());
    let mut disc = Disc(files);
    let root = std::env::temp_dir().join(format!("skate-disc-audio-test-{}", std::process::id()));
    let _ = fs::remove_dir_all(&root);
    // Stands in for vgmstream: one second of a mono ramp per stream.
    let calls = std::cell::RefCell::new(Vec::new());
    let mut decode = |dir: &Path, names: &[String]| -> Result<(), Error> {
        calls.borrow_mut().push(names.len());
        for name in names {
            let pcm: Vec<i16> = (0..8000).map(|i| (i % 300) as i16).collect();
            fs::write(dir.join(format!("{name}.wav")), wav_bytes(1, 8000, &pcm))?;
        }
        Ok(())
    };
    let mut messages = Vec::new();
    prepare_with(
        &mut disc,
        &[],
        &["Metal.bnk", "GRINDS.abk"],
        &mut decode,
        &root,
        &mut |m: &str| messages.push(m.to_owned()),
    )
    .unwrap();
    assert_eq!(*calls.borrow(), [1, 1, 2, 1]);
    assert!(!root.join("audio-work.tmp").exists());
    let audio = root.join("private/audio");
    let m: Value =
        serde_json::from_slice(&fs::read(audio.join("audio_manifest.json")).unwrap()).unwrap();
    assert_eq!(m["version"], json!(5));
    assert_eq!(m["ambience"], json!({}));
    let g = &m["grains"]["concrete"];
    assert_eq!(g["bands"].as_array().unwrap().len(), 6);
    assert_eq!(g["bands"][0]["file"], json!("grains/concrete/0.wav"));
    // 8000 / 6 = 1333 frames a band, less the 640-frame crossfade.
    assert_eq!(g["bands"][0]["seconds"], json!(py_round(693.0 / 8000.0, 4)));
    assert_eq!(g["file"], json!("grains/concrete.wav"));
    assert_eq!(g["grain"], json!("grains/concrete.grain"));
    assert_eq!(g["duration"], json!(1.0));
    assert_eq!(
        fs::read(audio.join("grains/concrete.grain")).unwrap(),
        member
    );
    assert_eq!(m["wheels"]["Whl"]["file"], json!("wheels/Whl.wav"));
    assert_eq!(m["banks"]["Metal"].as_array().unwrap().len(), 2);
    assert_eq!(
        m["banks"]["Metal"][1]["file"],
        json!("banks/Metal/0001.wav")
    );
    assert_eq!(m["banks"]["GRINDS"][0]["sample_rate"], json!(8000));
    assert_eq!(m["patches"]["Metal"]["containers"], json!([[0]]));
    assert!(m["patches"].get("GRINDS").is_none());
    assert_eq!(m["aems"]["projects"], json!(["aems/Player.csi"]));
    assert_eq!(
        m["aems"]["banks"],
        json!({"Common": "aems/Common.abk", "GRINDS": "aems/GRINDS.abk"})
    );
    assert_eq!(m["aems"]["splice"], json!({"Metal": "aems/Metal.splc"}));
    assert_eq!(m["aems"]["mixmap"], json!("aems/MixMapSK8.mxb"));
    assert_eq!(
        fs::read(audio.join("aems/Metal.splc")).unwrap(),
        splc_bank[..252]
    );
    assert_eq!(fs::read(audio.join("aems/MixMapSK8.mxb")).unwrap(), mixmap);
    assert_eq!(m["frontend"], json!({}));
    for path in [
        "grains/concrete/5.wav",
        "grains/concrete.wav",
        "wheels/Whl.wav",
        "banks/GRINDS/0000.wav",
    ] {
        assert!(audio.join(path).is_file(), "{path}");
    }
    assert_eq!(
        messages,
        [
            "Decoding grains",
            "Decoding wheels",
            "Decoding sound effect banks",
            "Copying the AEMS banks and projects"
        ]
    );
    // A missing bank fails the preparation and leaves no work folder.
    let err = prepare_with(
        &mut disc,
        &[],
        &["Nope.abk"],
        &mut decode,
        &root,
        &mut |_: &str| {},
    );
    assert!(matches!(err, Err(Error::Missing(_))));
    assert!(!root.join("audio-work.tmp").exists());
    fs::remove_dir_all(&root).unwrap();
}
