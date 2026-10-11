//! Synthetic bytes only.
use super::big::{Big, refpack};
use super::*;

struct Bytes(Vec<u8>);

impl ReadAt for Bytes {
    fn len(&self) -> u64 {
        self.0.len() as u64
    }
    fn read_at(&mut self, offset: u64, len: usize) -> Result<Vec<u8>, Error> {
        self.0
            .get(offset as usize..offset as usize + len)
            .map(<[u8]>::to_vec)
            .ok_or(Error::Format("past end"))
    }
}

#[test]
fn refpack_literals_and_overlapping_backrefs() {
    // 4 literals, a 2-byte command copying 3 from distance 4, then a run of
    // 5 from distance 1 (overlapping), then stop.
    let stream = [0xE0, b'a', b'b', b'c', b'd', 0x00, 0x03, 0x08, 0x00, 0xFC];
    assert_eq!(refpack(&stream, None).unwrap(), b"abcdabcccccc");
    let mut headed = vec![0x10, 0xFB, 0, 0, 12];
    headed.extend_from_slice(&stream);
    assert_eq!(refpack(&headed, Some(12)).unwrap(), b"abcdabcccccc");
    assert!(refpack(&headed, Some(11)).is_err());
    assert!(refpack(&[0x00, 0x09, 0xFC], None).is_err());
}

fn big(entries: &[(&str, &str, &[u8], u8, u32)]) -> Vec<u8> {
    let count = entries.len();
    let index_size = 48 + (16 * count).div_ceil(16) * 16 + count.div_ceil(16) * 16;
    let name_record = 16usize;
    let dirs: Vec<&str> = {
        let mut d: Vec<&str> = entries.iter().map(|e| e.0).collect();
        d.dedup();
        d
    };
    let names_size = (name_record * count).div_ceil(16) * 16 + dirs.len() * 16;
    let mut out = vec![0u8; index_size + names_size];
    out[0..2].copy_from_slice(&0x4542u16.to_be_bytes());
    out[2..4].copy_from_slice(&3u16.to_be_bytes());
    out[4..8].copy_from_slice(&(count as u32).to_be_bytes());
    out[12..16].copy_from_slice(&(index_size as u32).to_be_bytes());
    out[16..20].copy_from_slice(&(names_size as u32).to_be_bytes());
    out[20] = name_record as u8;
    out[21] = 16;
    let compression_start = 48 + (16 * count).div_ceil(16) * 16;
    let dirs_start = index_size + (name_record * count).div_ceil(16) * 16;
    for (i, d) in dirs.iter().enumerate() {
        out[dirs_start + i * 16..][..d.len()].copy_from_slice(d.as_bytes());
    }
    for (i, (dir, file, payload, compression, unpacked)) in entries.iter().enumerate() {
        let offset = out.len();
        out.extend_from_slice(payload);
        let e = 48 + i * 16;
        out[e..e + 4].copy_from_slice(&(offset as u32).to_be_bytes());
        out[e + 4..e + 8].copy_from_slice(&(payload.len() as u32).to_be_bytes());
        out[e + 8..e + 12].copy_from_slice(&unpacked.to_be_bytes());
        out[compression_start + i] = *compression;
        let n = index_size + i * name_record;
        let d = dirs.iter().position(|x| x == dir).unwrap() as u16;
        out[n..n + 2].copy_from_slice(&d.to_be_bytes());
        out[n + 2..n + 2 + file.len()].copy_from_slice(file.as_bytes());
    }
    out
}

#[test]
fn big_entries_paths_and_payloads() {
    let packed: &[u8] = &[0xE0, b'a', b'b', b'c', b'd', 0x00, 0x03, 0xFC];
    let bytes = big(&[
        ("data/config", "input.cfg", b"raw", 0, 3),
        ("data\\anim", "x.abin", packed, 1, 7),
        (".", "top.txt", b"t", 0, 0),
    ]);
    let mut archive = Big::open(Bytes(bytes)).unwrap();
    let paths: Vec<_> = archive.entries.iter().map(|e| e.path.clone()).collect();
    assert_eq!(
        paths,
        ["data/config/input.cfg", "data/anim/x.abin", "top.txt"]
    );
    let e = archive.find("DATA/ANIM/X.ABIN").unwrap().clone();
    assert_eq!(archive.read(&e).unwrap(), b"abcdabc");
    let e = archive.entries[2].clone();
    assert_eq!(archive.read(&e).unwrap(), b"t");
    assert!(Big::open(Bytes(vec![0; 64])).is_err());
}

#[test]
fn unsafe_paths_are_refused() {
    for bad in ["../x", "/x", "a//b", "C:/x", "a/./b"] {
        assert!(safe_relative(bad).is_err(), "{bad}");
    }
    assert_eq!(safe_relative("a/b").unwrap(), Path::new("a").join("b"));
}

#[test]
fn skeleton_names_and_bones() {
    // One kind-5 record of 192 bytes with one bone.
    let mut data = vec![0u8; 48 + 192];
    let end = data.len() as u32;
    data[0..4].copy_from_slice(&end.to_be_bytes());
    data[4..8].copy_from_slice(&1u32.to_be_bytes());
    let at = 48;
    data[at..at + 4].copy_from_slice(&192u32.to_be_bytes());
    data[at + 4..at + 8].copy_from_slice(&5u32.to_be_bytes());
    // "AB": digits 11 and 12 in base 38, most significant first.
    let word = 11 * 38u32.pow(5) + 12 * 38u32.pow(4);
    data[at + 16..at + 20].copy_from_slice(&word.to_be_bytes());
    let payload = (at + 55) & !15;
    data[payload..payload + 4].copy_from_slice(&1u32.to_be_bytes());
    let bone = payload + 16;
    data[bone + 92..bone + 96].copy_from_slice(&(11 * 38u32.pow(5)).to_be_bytes());
    let v = skeleton::convert(&data).unwrap();
    assert_eq!(v["skeletons"][0]["name"], "AB");
    assert_eq!(v["skeletons"][0]["bones"][0]["name"], "A");
    assert_eq!(
        v["skeletons"][0]["bones"][0]["words"]
            .as_array()
            .unwrap()
            .len(),
        28
    );
}
