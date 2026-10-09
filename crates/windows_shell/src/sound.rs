//! Sound banks as the exe registers them (sound-01): a request id selects
//! bank `id / 1000` and sound `id % 1000` in that bank's SCD. Banks come
//! from `se/seNNN.win32.scd` entries or from an object archive's
//! `se/obj/seNNN` replacement asset.
use audio_out::Clip;
use package_reader::{original, probe::ProbeSource, replacement, scd};
use std::collections::HashMap;

/// Reads a bank: the entry's original payload, or its named replacement.
pub fn load_bank(
    source: &ProbeSource,
    index: &str,
    ordinal: usize,
    name_hash: [u8; 16],
    replacement_name: Option<&str>,
) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
    let mut checked = source.open_index(index)?;
    let entry = checked
        .inventory
        .entries
        .get(ordinal)
        .filter(|e| e.name_hash == name_hash)
        .ok_or("sound bank entry mismatch")?;
    let bytes = match replacement_name {
        None => original::read_original(&mut checked.reader, entry)?.bytes,
        Some(name) => {
            replacement::read_all_replacements(&mut checked.reader, entry)?
                .into_iter()
                .find(|r| {
                    let n = &r.metadata.name_bytes;
                    let end = n.iter().position(|&b| b == 0).unwrap_or(n.len());
                    &n[..end] == name.as_bytes()
                })
                .ok_or("sound bank replacement absent")?
                .bytes
        }
    };
    checked.verify_unchanged()?;
    scd::read(&bytes)?;
    Ok(bytes)
}

/// One resolved request: its layers, the sound's volume and loop flag.
pub struct Request {
    pub layers: Vec<Clip>,
    pub volume: f32,
    pub looped: bool,
}

#[derive(Default)]
pub struct Banks {
    banks: HashMap<u16, Vec<u8>>,
    decoded: HashMap<(u16, usize), Option<Clip>>,
}

impl Banks {
    /// Registers a bank under the number in its own table header.
    pub fn insert(&mut self, bytes: Vec<u8>) -> Result<u16, Box<dyn std::error::Error>> {
        let number = scd::read(&bytes)?.number;
        self.banks.insert(number, bytes);
        Ok(number)
    }

    pub fn numbers(&self) -> Vec<u16> {
        let mut n: Vec<_> = self.banks.keys().copied().collect();
        n.sort_unstable();
        n
    }

    /// Resolves `id`; None when its bank is not loaded or the sound is
    /// outside it (the engine drops such requests).
    pub fn request(&mut self, id: u32) -> Result<Option<Request>, Box<dyn std::error::Error>> {
        let (number, no) = ((id / 1000) as u16, (id % 1000) as usize);
        let Some(bytes) = self.banks.get(&number) else {
            return Ok(None);
        };
        let bank = scd::read(bytes)?;
        let Some(sound) = bank.sound(no)? else {
            return Ok(None);
        };
        let mut layers = Vec::new();
        for &audio in &sound.audio {
            let decoded = match self.decoded.entry((number, audio as usize)) {
                std::collections::hash_map::Entry::Occupied(e) => e.into_mut(),
                std::collections::hash_map::Entry::Vacant(e) => {
                    e.insert(bank.decode(audio as usize)?.map(|pcm| Clip {
                        channels: pcm.channels,
                        rate: pcm.rate,
                        loop_frames: Some((pcm.loop_start, pcm.loop_end)),
                        samples: pcm.samples.into(),
                    }))
                }
            };
            if let Some(clip) = decoded {
                let mut clip = clip.clone();
                if !sound.looped {
                    clip.loop_frames = None;
                }
                layers.push(clip);
            }
        }
        Ok(Some(Request {
            layers,
            volume: sound.volume,
            looped: sound.looped,
        }))
    }
}
