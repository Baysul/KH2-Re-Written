//! The skateboard is Sora's equipped Keyblade. The model follows the game's
//! own chain from the equipped item to its weapon object: the item record's
//! +4 is the weapon's position in a WENT set of 03system (counting the set's
//! size word as 0), the set is the one Sora's
//! object entry names by its neo-moveset index, and the set entry is the
//! weapon's object id, whose model name is `obj/<name>.mdlx` (layouts per
//! OpenKH's 03system and 00objentry documentation; checked on the install by
//! the Kingdom Key resolving to the room's own weapon model).
use bevy::prelude::*;
use package_reader::{item_table, objentry::ObjectEntry};
use std::collections::HashMap;

/// Board length in scene units (test-room choice: a 1.4 m longboard, so
/// the rider's feet fit; the Kingdom Key is 116 units long as modelled).
const BOARD_LENGTH: f32 = 140.;

/// Item category of a Keyblade.
const KEYBLADE: u8 = 2;

/// Keyblade item id -> its weapon object's model name.
#[derive(Default)]
pub struct Keyblades {
    by_item: HashMap<u16, String>,
}

impl Keyblades {
    /// Reads 03system's WENT and ITEM entries (`went`, `items`) for the
    /// player object `player` among `objects`.
    pub fn read(
        went: &[u8],
        items: &[u8],
        objects: &[ObjectEntry],
        player: &str,
    ) -> Result<Self, Box<dyn std::error::Error>> {
        let player = objects
            .iter()
            .find(|o| o.model_name == player.as_bytes())
            .ok_or("player object entry absent")?;
        let set = went_set(went, player.neo_moveset as usize)?;
        let mut by_item = HashMap::new();
        for item in item_table::read(items)?
            .iter()
            .filter(|i| i.kind == KEYBLADE)
        {
            // The id counts words from the set's start, the size word being
            // 0 (on the install the Kingdom Key then resolves to W_EX010,
            // the room's own weapon model).
            let Some(&object_id) = (item.amount as usize)
                .checked_sub(1)
                .and_then(|i| set.get(i))
            else {
                continue;
            };
            if let Some(o) = objects.iter().find(|o| o.object_id == object_id) {
                by_item.insert(item.id, String::from_utf8_lossy(&o.model_name).into_owned());
            }
        }
        Ok(Self { by_item })
    }

    /// The model name of the Keyblade item `item`.
    pub fn model(&self, item: u16) -> Option<&str> {
        self.by_item.get(&item).map(String::as_str)
    }
}

fn word(bytes: &[u8], at: usize) -> Result<u32, Box<dyn std::error::Error>> {
    Ok(u32::from_le_bytes(
        bytes
            .get(at..at + 4)
            .ok_or("WENT entry out of range")?
            .try_into()?,
    ))
}

/// One WENT set: pointers (4-byte units) at the start, each to a set of
/// `size` words, the first being `size` itself.
fn went_set(went: &[u8], index: usize) -> Result<Vec<u32>, Box<dyn std::error::Error>> {
    let pointer = word(went, index * 4)? as usize;
    // The pointer table ends where the first set begins.
    let mut first = usize::MAX;
    let mut i = 0;
    while i < first && i < 4096 {
        match word(went, i * 4)? as usize {
            0 => {}
            p => first = first.min(p),
        }
        i += 1;
    }
    if pointer == 0 || index >= first {
        return Err("WENT set index outside the table".into());
    }
    let size = word(went, pointer * 4)? as usize;
    if size == 0 || size > 4096 {
        return Err("WENT set size out of range".into());
    }
    (1..size).map(|i| word(went, (pointer + i) * 4)).collect()
}

/// The Keyblade laid flat as a deck, its own pose `points`: the longest
/// extent along the board, the next across it, centred on the deck frame
/// and scaled to `BOARD_LENGTH`.
pub fn deck_transform(points: &[Vec3]) -> Transform {
    if points.is_empty() {
        return Transform::IDENTITY;
    }
    let (low, high) = points.iter().fold(
        (Vec3::splat(f32::MAX), Vec3::splat(f32::MIN)),
        |(l, h), &p| (l.min(p), h.max(p)),
    );
    let size = high - low;
    let mut axes = [(size.x, Vec3::X), (size.y, Vec3::Y), (size.z, Vec3::Z)];
    axes.sort_by(|a, b| b.0.total_cmp(&a.0));
    let [(long, long_axis), (_, mid_axis), _] = axes;
    // Columns: where the board's X (across), Y (up) and Z (along) come from.
    let up = long_axis.cross(mid_axis);
    let rotation = Quat::from_mat3(&Mat3::from_cols(mid_axis, up, long_axis).transpose());
    let scale = if long > 0. { BOARD_LENGTH / long } else { 1. };
    let centre = (low + high) / 2.;
    Transform {
        translation: rotation * (-centre * scale) + Vec3::Y * crate::skate::DECK_TOP,
        rotation,
        scale: Vec3::splat(scale),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn le(words: &[u32]) -> Vec<u8> {
        words.iter().flat_map(|w| w.to_le_bytes()).collect()
    }

    #[test]
    fn went_sets_follow_their_pointers() {
        // Two pointers (the second repeats the first), then one set of
        // size 3 holding object ids 103 and 104.
        let went = le(&[2, 2, 3, 103, 104]);
        assert_eq!(went_set(&went, 0).unwrap(), [103, 104]);
        assert_eq!(went_set(&went, 1).unwrap(), [103, 104]);
        assert!(went_set(&went, 2).is_err());
    }

    #[test]
    fn the_longest_extent_runs_along_the_board() {
        // A blade along +Y, wider in X than in Z.
        let points = [Vec3::new(-5., 0., -1.), Vec3::new(5., 100., 1.)];
        let t = deck_transform(&points);
        let tip = t.transform_point(Vec3::new(0., 100., 0.));
        let hilt = t.transform_point(Vec3::new(0., 0., 0.));
        assert!(((tip - hilt).length() - BOARD_LENGTH).abs() < 1e-3);
        assert!((tip - hilt).normalize().z.abs() > 0.999);
    }
}
