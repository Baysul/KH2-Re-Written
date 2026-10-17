//! The victim's own hit sound. `FUN_140410d60` (the damage routine, victim
//! side) plays `FUN_1403dadf0(victim, bank * 1000 + (n & 1) + class, position)`
//! once per hit unless the hit record's flags bit 2 is set, where `n` is the
//! victim's hit counter byte (`obj+0x26f*4`, incremented after each sound),
//! `class` follows the ATKP reaction through `FUN_1403ce980`, and `bank` is the
//! sound bank the victim's object was loaded with (the record behind
//! `obj+0xbb0`).
//!
//! [I] The bank. Its source is a load argument whose origin was not traced
//! (issue #100); the Shadow's effects name bank 201 (sounds 201016, 201017,
//! 201019) and sounds 201000 to 201003, the four this scheme produces, exist
//! in the install and in no other bank tried. The bank is therefore taken from
//! the victim's own effect sounds: the most common thousand among the
//! elements' sounds of 100000 and up.
use data::object_effect::Pax;
use std::collections::HashMap;

/// `FUN_1403ce980`: the reaction's class.
fn class_of(reaction: u8) -> u8 {
    match reaction {
        1 | 0xc => 1,
        3 | 6 | 8 | 10 | 0xd | 0x10 | 0x12 => 3,
        4 | 7 | 9 | 0xb | 0xf | 0x11 => 4,
        other => other,
    }
}

/// The id offset of a reaction: classes 1, 3 and 5 are 0, class 4 is 2, any
/// other reaction plays nothing.
pub fn offset(reaction: u8) -> Option<u32> {
    match class_of(reaction) {
        1 | 3 | 5 => Some(0),
        4 => Some(2),
        _ => None,
    }
}

/// The bank named by the victim's effect sounds.
pub fn bank(pax: &Pax) -> Option<u32> {
    let mut counts: HashMap<u32, u32> = HashMap::new();
    for e in &pax.elements {
        if e.sound >= 100_000 {
            *counts.entry(e.sound as u32 / 1000).or_default() += 1;
        }
    }
    counts
        .into_iter()
        .max_by_key(|&(bank, n)| (n, std::cmp::Reverse(bank)))
        .map(|(bank, _)| bank)
}

/// The sound id for one hit on victim `key`, advancing its counter.
pub fn sound(pax: &Pax, reaction: u8, hits: &mut HashMap<u64, u8>, key: u64) -> Option<u32> {
    let offset = offset(reaction)?;
    let bank = bank(pax)?;
    let n = hits.entry(key).or_insert(0);
    let id = bank * 1000 + (*n & 1) as u32 + offset;
    *n = n.wrapping_add(1);
    Some(id)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reactions_map_to_the_id_offset() {
        for (reaction, want) in [
            (1, Some(0)),
            (0xc, Some(0)),
            (3, Some(0)),
            (0x12, Some(0)),
            (5, Some(0)),
            (4, Some(2)),
            (7, Some(2)),
            (0x11, Some(2)),
            (0, None),
            (2, None),
            (0x20, None),
        ] {
            assert_eq!(offset(reaction), want, "reaction {reaction}");
        }
    }

    fn pax(sounds: &[i32]) -> Pax {
        use data::object_effect::{BonePos, Element};
        Pax {
            name: "test".into(),
            elements: sounds
                .iter()
                .enumerate()
                .map(|(i, &sound)| Element {
                    effect_number: i as u16,
                    id: i as u16,
                    group: 0,
                    fadeout_frame: 0,
                    bone: 0,
                    category: 0,
                    flags: 0,
                    start_wait: 0.,
                    sound,
                    translation: [0.; 3],
                    rotation: [0.; 3],
                    scale: [1.; 3],
                    bone_pos: BonePos::default(),
                })
                .collect(),
            effects: Vec::new(),
            dpds: Vec::new(),
        }
    }

    #[test]
    fn the_bank_comes_from_the_victims_own_sounds_and_the_id_alternates() {
        // Sounds below 100000 (the system bank, none) do not count.
        let shadow = pax(&[-1, 201019, 201016, 201017, 114]);
        assert_eq!(bank(&shadow), Some(201));
        assert_eq!(bank(&pax(&[-1, 5])), None);
        let mut hits = HashMap::new();
        let ids: Vec<_> = (0..4)
            .map(|_| sound(&shadow, 3, &mut hits, 9).unwrap())
            .collect();
        assert_eq!(ids, [201000, 201001, 201000, 201001]);
        // A blow takes the heavy pair; a victim of its own counts alone.
        assert_eq!(sound(&shadow, 7, &mut hits, 9), Some(201002));
        assert_eq!(sound(&shadow, 7, &mut hits, 10), Some(201002));
        assert_eq!(sound(&shadow, 7, &mut hits, 10), Some(201003));
        // A reaction without a hit sound leaves the counter alone.
        assert_eq!(sound(&shadow, 0, &mut hits, 10), None);
        assert_eq!(hits[&10], 2);
    }
}
