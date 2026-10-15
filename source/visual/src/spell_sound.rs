//! Spell sounds: what a cast and a release request (research magicsfx-01).
//!
//! The original has no per-spell bank and no play-sound trigger in the cast
//! motions. Each spell's sounds are sound ids of its effect elements, which
//! the spell's script starts at the cast (motion start) and at the release
//! frame. They are plain request ids: below 1000 the system bank, 999xxx
//! the second resident bank.
//!
//! Not modelled here (unresolved or tied to unported parts): hit sounds,
//! Magnet's looping hold and its end, Cure on other party members, the
//! per-bolt timing of Thunder's Master and Final forms, and Blizzard's
//! alternating Final-form volleys.

use player::magic::{BLIZZARD, CURE, FIRE, MAGNET, REFLECT, THUNDER};

/// `form` is the player character's form byte as `magic::variant_index`
/// reads it: 2 Wisdom, 4 Master, 5 Final.
const WISDOM: u8 = 2;
const MASTER: u8 = 4;
const FINAL: u8 = 5;

fn level(levels: [u32; 3], level: u8) -> u32 {
    levels[(level.clamp(1, 3) - 1) as usize]
}

/// The cast sound at the cast motion's start.
pub fn cast_sound(magic_type: u8, form: u8) -> Option<u32> {
    match magic_type {
        // Skipped in Wisdom and Final.
        FIRE => (form != WISDOM && form != FINAL).then_some(116),
        BLIZZARD => Some(120),
        THUNDER => Some(126),
        CURE => Some(129),
        MAGNET => Some(133),
        _ => None,
    }
}

/// The sounds at the release frame of a spell of `level` (1..=3).
pub fn release_sounds(
    magic_type: u8,
    level_number: u8,
    finisher: bool,
    airborne: bool,
    form: u8,
) -> Vec<u32> {
    let _ = airborne; // the standing and jump elements share their sounds
    match magic_type {
        FIRE => {
            let mut out = vec![level([117, 140, 156], level_number)];
            // A second ball.
            if (form == WISDOM && finisher) || form == MASTER || (form == FINAL && finisher) {
                out.push(999016);
            }
            // Final form's finisher element.
            if form == FINAL && finisher {
                out.push(level([141, 142, 146], level_number));
            }
            out
        }
        BLIZZARD => vec![level([121, 143, 159], level_number)],
        // One bolt; Master and Final spawn more, on their own schedule.
        THUNDER => vec![level([127, 148, 164], level_number)],
        // The chosen target (the caster here).
        CURE => vec![level([130, 150, 166], level_number)],
        // The sphere's spawn.
        MAGNET => vec![134],
        REFLECT => vec![level([138, 175, 177], level_number)],
        _ => Vec::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fire_casts_except_in_wisdom_and_final() {
        assert_eq!(cast_sound(FIRE, 0), Some(116));
        assert_eq!(cast_sound(FIRE, MASTER), Some(116));
        assert_eq!(cast_sound(FIRE, WISDOM), None);
        assert_eq!(cast_sound(FIRE, FINAL), None);
    }

    #[test]
    fn every_other_spell_has_one_cast_sound_and_reflect_none() {
        assert_eq!(cast_sound(BLIZZARD, 0), Some(120));
        assert_eq!(cast_sound(THUNDER, 0), Some(126));
        assert_eq!(cast_sound(CURE, 0), Some(129));
        assert_eq!(cast_sound(MAGNET, 0), Some(133));
        assert_eq!(cast_sound(REFLECT, 0), None);
    }

    #[test]
    fn fire_release_by_level_and_second_ball() {
        assert_eq!(release_sounds(FIRE, 1, false, false, 0), vec![117]);
        assert_eq!(release_sounds(FIRE, 3, false, true, 0), vec![156]);
        assert_eq!(
            release_sounds(FIRE, 2, false, false, MASTER),
            vec![140, 999016]
        );
        assert_eq!(
            release_sounds(FIRE, 1, true, false, WISDOM),
            vec![117, 999016]
        );
        assert_eq!(release_sounds(FIRE, 1, false, false, WISDOM), vec![117]);
        assert_eq!(
            release_sounds(FIRE, 2, true, false, FINAL),
            vec![140, 999016, 142]
        );
    }

    #[test]
    fn other_spells_release_by_level() {
        assert_eq!(release_sounds(BLIZZARD, 2, false, false, 0), vec![143]);
        assert_eq!(release_sounds(THUNDER, 3, false, false, 0), vec![164]);
        assert_eq!(release_sounds(CURE, 1, false, false, 0), vec![130]);
        assert_eq!(release_sounds(MAGNET, 3, false, false, 0), vec![134]);
        assert_eq!(release_sounds(REFLECT, 2, false, false, 0), vec![175]);
    }

    #[test]
    fn unknown_type_and_out_of_range_levels_are_safe() {
        assert!(release_sounds(9, 1, false, false, 0).is_empty());
        assert_eq!(release_sounds(FIRE, 0, false, false, 0), vec![117]);
        assert_eq!(release_sounds(FIRE, 9, false, false, 0), vec![156]);
    }
}
