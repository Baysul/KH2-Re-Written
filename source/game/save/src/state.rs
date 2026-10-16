//! The part of the save image the rewrite models, read out of and written
//! into the 0x10fc0-byte image. Everything else in the image is left as it
//! was loaded, so a save from the original game keeps its other fields
//! when the rewrite saves over it.
//!
//! Offsets are `address - 0x1409a98b0` (the live structure, FUN_140152380);
//! each cites where the exe reads or writes it.

use crate::{data::SaveData, slots::field};

/// Program table `{map, btl, evt}` i16 per `world * 0x40 + area`
/// (`DAT_1409a98c0`, cleared by FUN_140152030 as 0x4c0 entries of 6 bytes).
pub const PROGRAMS: usize = 0x10;
pub const PROGRAM_ENTRIES: usize = 0x4c0;
/// Story flag words (`DAT_1409ab540`; 19 worlds of 8 words are saved, the
/// exe's flag test can index further, which only matters to the flags
/// outside the table).
pub const FLAGS: usize = 0x1c90;
pub const FLAG_WORDS: usize = 19 * 8;
/// Per-world arif set byte (FUN_1403ab470: `DAT_1409abc9c[world]`).
pub const ARIF: usize = 0x23ec;
/// Per-world record words (FUN_1403f1e20: `DAT_1409ad89c + world * 4`; the
/// map program uses bytes 1 and 2).
pub const WORLD_RECORDS: usize = 0x3fec;
/// Shared EXP (camp status "Experience", FUN_140354340).
pub const EXP: usize = 0x36e0;
/// Shortcut command ids (camp Customize, `DAT_1409acfa8`).
pub const SHORTCUTS: usize = 0x36f8;
/// Sora's party record (record 1): equipped weapon item `u16`, level at
/// +0xf, 80 ability slots `u16` at +0x54 (bit 15 equipped).
pub const SORA: usize = field::PARTY;
pub const ABILITY_SLOTS: usize = 80;
pub const ABILITIES: usize = SORA + 0x54;
/// Config bits (vibration bit 0, camera bits 1..2; FUN_1402e5000).
pub const CONFIG: usize = 0x41a4;

/// The rewrite's persistent state.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Persistent {
    pub world: u8,
    pub area: u8,
    pub entrance: u8,
    pub play_frames: u32,
    pub munny: u32,
    pub difficulty: u32,
    pub roxas: bool,
    pub level: u8,
    pub exp: u32,
    pub weapon: u16,
    pub abilities: [u16; ABILITY_SLOTS],
    pub shortcuts: [u16; 4],
    pub config: u32,
    pub programs: Vec<[i16; 3]>,
    pub flags: Vec<u32>,
    pub arif: [u8; 19],
    pub world_records: [[u8; 2]; 19],
}

impl Default for Persistent {
    fn default() -> Self {
        Self {
            world: 0,
            area: 0,
            entrance: 0,
            play_frames: 0,
            munny: 0,
            difficulty: 1,
            roxas: false,
            level: 1,
            exp: 0,
            weapon: 0,
            abilities: [0; ABILITY_SLOTS],
            shortcuts: [0; 4],
            config: 0,
            programs: vec![[0; 3]; PROGRAM_ENTRIES],
            flags: vec![0; FLAG_WORDS],
            arif: [0; 19],
            world_records: [[0; 2]; 19],
        }
    }
}

impl Persistent {
    pub fn read(data: &SaveData) -> Self {
        let mut abilities = [0u16; ABILITY_SLOTS];
        for (i, a) in abilities.iter_mut().enumerate() {
            *a = data.u16_at(ABILITIES + i * 2);
        }
        let mut shortcuts = [0u16; 4];
        for (i, s) in shortcuts.iter_mut().enumerate() {
            *s = data.u16_at(SHORTCUTS + i * 2);
        }
        let programs = (0..PROGRAM_ENTRIES)
            .map(|i| {
                let at = PROGRAMS + i * 6;
                [
                    data.u16_at(at) as i16,
                    data.u16_at(at + 2) as i16,
                    data.u16_at(at + 4) as i16,
                ]
            })
            .collect();
        let flags = (0..FLAG_WORDS)
            .map(|i| data.u32_at(FLAGS + i * 4))
            .collect();
        let mut arif = [0u8; 19];
        for (i, a) in arif.iter_mut().enumerate() {
            *a = data.u8_at(ARIF + i);
        }
        let mut world_records = [[0u8; 2]; 19];
        for (i, r) in world_records.iter_mut().enumerate() {
            *r = [
                data.u8_at(WORLD_RECORDS + i * 4 + 1),
                data.u8_at(WORLD_RECORDS + i * 4 + 2),
            ];
        }
        Self {
            world: data.u8_at(field::WORLD),
            area: data.u8_at(field::AREA),
            entrance: data.u8_at(field::ENTRANCE),
            play_frames: data.u32_at(field::PLAY_FRAMES),
            munny: data.u32_at(field::MUNNY),
            difficulty: data.u32_at(field::DIFFICULTY),
            roxas: data.u8_at(field::ROXAS) & 1 != 0,
            level: data.u8_at(SORA + field::RECORD_LEVEL),
            exp: data.u32_at(EXP),
            weapon: data.u16_at(SORA),
            abilities,
            shortcuts,
            config: data.u32_at(CONFIG),
            programs,
            flags,
            arif,
            world_records,
        }
    }

    /// Writes the modeled fields and seals the checksum.
    pub fn write(&self, data: &mut SaveData) {
        data.set_u8(field::WORLD, self.world);
        data.set_u8(field::AREA, self.area);
        data.set_u8(field::ENTRANCE, self.entrance);
        data.set_u32(field::PLAY_FRAMES, self.play_frames);
        data.set_u32(field::MUNNY, self.munny);
        data.set_u32(field::DIFFICULTY, self.difficulty);
        let roxas = data.u8_at(field::ROXAS) & !1 | u8::from(self.roxas);
        data.set_u8(field::ROXAS, roxas);
        data.set_u8(SORA + field::RECORD_LEVEL, self.level);
        data.set_u32(EXP, self.exp);
        data.set_u16(SORA, self.weapon);
        for (i, a) in self.abilities.iter().enumerate() {
            data.set_u16(ABILITIES + i * 2, *a);
        }
        for (i, s) in self.shortcuts.iter().enumerate() {
            data.set_u16(SHORTCUTS + i * 2, *s);
        }
        data.set_u32(CONFIG, self.config);
        for (i, p) in self.programs.iter().take(PROGRAM_ENTRIES).enumerate() {
            let at = PROGRAMS + i * 6;
            for (k, v) in p.iter().enumerate() {
                data.set_u16(at + k * 2, *v as u16);
            }
        }
        for (i, w) in self.flags.iter().take(FLAG_WORDS).enumerate() {
            data.set_u32(FLAGS + i * 4, *w);
        }
        for (i, a) in self.arif.iter().enumerate() {
            data.set_u8(ARIF + i, *a);
        }
        for (i, r) in self.world_records.iter().enumerate() {
            data.set_u8(WORLD_RECORDS + i * 4 + 1, r[0]);
            data.set_u8(WORLD_RECORDS + i * 4 + 2, r[1]);
        }
        data.seal();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn write_then_read_round_trips_and_keeps_other_bytes() {
        let mut image = SaveData::blank();
        // A byte this module does not model survives a write.
        image.set_u8(0x5000, 0x77);
        image.seal();
        let mut p = Persistent {
            world: 2,
            area: 8,
            entrance: 3,
            play_frames: 216_000 + 3600 * 7,
            munny: 4321,
            difficulty: 2,
            roxas: true,
            level: 14,
            exp: 9999,
            weapon: 0x29,
            shortcuts: [3, 0, 7, 9],
            config: 0b101,
            ..Persistent::default()
        };
        p.abilities[0] = 0x8000 | 0x62;
        p.abilities[79] = 5;
        p.programs[2 * 0x40 + 8] = [3, -1, 7];
        p.flags[2 * 8 + 1] = 0xdead_beef;
        p.arif[2] = 4;
        p.world_records[2] = [1, 9];
        p.write(&mut image);
        assert!(SaveData::from_bytes(image.bytes()).is_ok());
        assert_eq!(Persistent::read(&image), p);
        assert_eq!(image.u8_at(0x5000), 0x77);
    }

    #[test]
    fn writing_the_roxas_flag_keeps_the_other_bits() {
        let mut image = SaveData::blank();
        image.set_u8(field::ROXAS, 0b10);
        let mut p = Persistent::read(&image);
        p.roxas = true;
        p.write(&mut image);
        assert_eq!(image.u8_at(field::ROXAS), 0b11);
        p.roxas = false;
        p.write(&mut image);
        assert_eq!(image.u8_at(field::ROXAS), 0b10);
    }
}
