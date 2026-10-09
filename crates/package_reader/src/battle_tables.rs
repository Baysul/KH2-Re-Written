//! 00battle.bin tables used by combat: ATKP (attack parameters), ENMP (enemy
//! parameters), LVPM (enemy level multipliers) and LVUP (player levels).
//! Layouts follow the public OpenKH 00battle.md
//! (7a3b945c538d32c6a285128c98aefba093f52ceb). Field meanings marked
//! "executable" were checked against the PC damage routine FUN_1403c12f0.
use std::io;

fn invalid(m: &str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, m.to_owned())
}
fn u16_at(b: &[u8], at: usize) -> io::Result<u16> {
    b.get(at..at + 2)
        .map(|w| u16::from_le_bytes([w[0], w[1]]))
        .ok_or_else(|| invalid("battle table truncated"))
}
fn i16_at(b: &[u8], at: usize) -> io::Result<i16> {
    u16_at(b, at).map(|v| v as i16)
}
fn u32_at(b: &[u8], at: usize) -> io::Result<u32> {
    b.get(at..at + 4)
        .map(|w| u32::from_le_bytes(w.try_into().unwrap()))
        .ok_or_else(|| invalid("battle table truncated"))
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Attack {
    pub id: u16,
    pub kind: u8,
    /// Attacker stat used (executable): 0 strength, 1 magic, else none.
    pub stat: u8,
    pub power: u16,
    pub element: u8,
    pub reaction: u8,
    pub knockback: [i16; 2],
    pub flags: u8,
    /// Sound id played on a hit when non-negative (executable: +0x1C read
    /// by the hit routine FUN_1403d12c0).
    pub hit_sound: i32,
    /// Hit effect number (+0xb, 0 = none) and random-pick count (+0x2d)
    /// (executable: FUN_1403d1b40).
    pub hit_effect: u8,
    pub hit_effect_random: u8,
}

/// The ATKP row for `id` at sub-table 0, hit step 0 (record bytes 0 and 1
/// zero), as the executable's search key ((id << 8) | sub) << 8 selects.
pub fn attack(atkp: &[u8], id: u16) -> io::Result<Attack> {
    let count = u32_at(atkp, 4)? as usize;
    for k in 0..count {
        let r = 8 + k * 48;
        let rec = atkp
            .get(r..r + 48)
            .ok_or_else(|| invalid("ATKP truncated"))?;
        if u16_at(rec, 2)? == id && rec[0] == 0 && rec[1] == 0 {
            return Ok(Attack {
                id,
                kind: rec[4],
                stat: rec[5],
                power: u16_at(rec, 6)?,
                element: rec[9],
                reaction: rec[10],
                knockback: [i16_at(rec, 12)?, i16_at(rec, 14)?],
                flags: rec[18],
                hit_sound: u32_at(rec, 0x1c)? as i32,
                hit_effect: rec[0xb],
                hit_effect_random: rec[0x2d],
            });
        }
    }
    Err(invalid("ATKP id absent"))
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Enemy {
    pub id: u16,
    /// 0 uses the world battle level.
    pub level: u16,
    /// Main HP pool, percent of the LVPM HP.
    pub hp_percent: u16,
    /// Damage cap and minimum, per 10000 of max HP (executable).
    pub damage_cap: u16,
    pub damage_min: u16,
    /// Physical, fire, blizzard, thunder, dark, light, general (percent).
    pub weakness: [u16; 7],
}

pub fn enemy(enmp: &[u8], id: u16) -> io::Result<Enemy> {
    let count = u32_at(enmp, 4)? as usize;
    for k in 0..count {
        let r = 8 + k * 0x5c;
        let rec = enmp
            .get(r..r + 0x5c)
            .ok_or_else(|| invalid("ENMP truncated"))?;
        if u16_at(rec, 0)? == id {
            let mut weakness = [0; 7];
            for (i, w) in weakness.iter_mut().enumerate() {
                *w = u16_at(rec, 72 + i * 2)?;
            }
            return Ok(Enemy {
                id,
                level: u16_at(rec, 2)?,
                hp_percent: u16_at(rec, 4)?,
                damage_cap: u16_at(rec, 68)?,
                damage_min: u16_at(rec, 70)?,
                weakness,
            });
        }
    }
    Err(invalid("ENMP id absent"))
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct LevelRow {
    pub hp: u16,
    pub strength: u16,
    pub defense: u16,
    pub max_strength: u16,
    pub min_strength: u16,
    pub exp: u16,
}

/// LVPM row for an enemy level 1..=99 (99 x 12 bytes, no header).
pub fn enemy_level(lvpm: &[u8], level: u16) -> io::Result<LevelRow> {
    if !(1..=99).contains(&level) {
        return Err(invalid("LVPM level outside 1..=99"));
    }
    let r = (level as usize - 1) * 12;
    Ok(LevelRow {
        hp: u16_at(lvpm, r)?,
        strength: u16_at(lvpm, r + 2)?,
        defense: u16_at(lvpm, r + 4)?,
        max_strength: u16_at(lvpm, r + 6)?,
        min_strength: u16_at(lvpm, r + 8)?,
        exp: u16_at(lvpm, r + 10)?,
    })
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PlayerLevel {
    pub strength: u8,
    pub magic: u8,
    pub defense: u8,
    pub ap: u8,
}

/// LVUP row for a character id and level: the character block is at the
/// u32 word (x4) in the pointer table at 8 + id*4; row L is at block + 4 +
/// (L - 1) * 16 (executable FUN_1403ed2e0).
pub fn player_level(lvup: &[u8], character: usize, level: usize) -> io::Result<PlayerLevel> {
    if !(1..=99).contains(&level) {
        return Err(invalid("LVUP level outside 1..=99"));
    }
    let block = u32_at(lvup, 8 + character * 4)? as usize * 4;
    let r = block + 4 + (level - 1) * 16;
    let row = lvup
        .get(r..r + 16)
        .ok_or_else(|| invalid("LVUP truncated"))?;
    Ok(PlayerLevel {
        strength: row[4],
        magic: row[5],
        defense: row[6],
        ap: row[7],
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_synthetic_rows() {
        let mut atkp = vec![6, 0, 0, 0, 1, 0, 0, 0];
        let mut rec = [0u8; 48];
        rec[2] = 126;
        rec[6] = 100;
        rec[10] = 3;
        rec[12] = 125;
        rec[0x1c..0x20].copy_from_slice(&(-1i32).to_le_bytes());
        atkp.extend(rec);
        let a = attack(&atkp, 126).unwrap();
        assert_eq!((a.power, a.reaction, a.knockback), (100, 3, [125, 0]));
        assert_eq!((a.hit_sound, a.hit_effect), (-1, 0));
        assert!(attack(&atkp, 127).is_err());

        let mut lvpm = vec![0u8; 24];
        lvpm[12..14].copy_from_slice(&21u16.to_le_bytes());
        assert_eq!(enemy_level(&lvpm, 2).unwrap().hp, 21);
        assert!(enemy_level(&lvpm, 0).is_err());

        let mut lvup = vec![0u8; 64];
        lvup[12..16].copy_from_slice(&8u32.to_le_bytes());
        lvup[32 + 4 + 4] = 2;
        assert_eq!(player_level(&lvup, 1, 1).unwrap().strength, 2);
    }
}
