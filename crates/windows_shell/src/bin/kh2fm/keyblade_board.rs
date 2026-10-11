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

/// The ground below the board frame (scene units): Skate 3's deck frame
/// rides 8.7 units above the floor (measured on the test room's floor).
const GROUND: f32 = -8.7;
/// Clearance kept between the board's lowest point and the ground.
const CLEARANCE: f32 = 0.5;

/// Board length in scene units (test-room choice: a 1.4 m longboard, so
/// the rider's feet fit; the Kingdom Key is 116 units long as modelled).
const BOARD_LENGTH: f32 = 140.;

/// Item category of a Keyblade.
const KEYBLADE: u8 = 2;

/// Keyblade item id -> its weapon object's model name.
#[derive(Default)]
pub struct Keyblades {
    by_item: HashMap<u16, String>,
    /// Model name -> its object entry's motion set name.
    animations: HashMap<String, String>,
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
        let animations = by_item
            .values()
            .filter_map(|name| {
                let o = objects.iter().find(|o| o.model_name == name.as_bytes())?;
                (!o.animation_name.is_empty()).then(|| {
                    (
                        name.clone(),
                        String::from_utf8_lossy(&o.animation_name).into_owned(),
                    )
                })
            })
            .collect();
        Ok(Self {
            by_item,
            animations,
        })
    }

    /// The motion set named by the object entry of model `model`.
    pub fn animation(&self, model: &str) -> Option<&str> {
        self.animations.get(model).map(String::as_str)
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

/// The principal axes of `points` (largest spread first), by power
/// iteration on their covariance.
fn principal_axes(points: &[Vec3]) -> [Vec3; 3] {
    let n = points.len().max(1) as f32;
    let mean = points.iter().copied().sum::<Vec3>() / n;
    let mut c = Mat3::ZERO;
    for &p in points {
        let d = p - mean;
        c += Mat3::from_cols(d * d.x, d * d.y, d * d.z);
    }
    let dominant = |m: Mat3, seed: Vec3| {
        let mut v = seed;
        for _ in 0..64 {
            let next = m * v;
            if next.length_squared() < 1e-20 {
                break;
            }
            v = next.normalize();
        }
        v
    };
    let first = dominant(c, Vec3::new(0.577, 0.577, 0.577));
    // Deflate, then take the strongest direction left, kept orthogonal.
    let spread = (c * first).dot(first);
    let rest = c - Mat3::from_cols(first * first.x, first * first.y, first * first.z) * spread;
    let second = dominant(rest, first.any_orthonormal_vector());
    let second = (second - first * second.dot(first)).normalize_or(first.any_orthonormal_vector());
    [first, second, first.cross(second)]
}

/// The Keyblade laid flat as a deck, from its pose `points` in its grip
/// joint's space: its long (principal) axis along the board with the key
/// end (the end farthest from the grip) forward, its next axis across it,
/// scaled to `BOARD_LENGTH`, and its top over the middle of the board
/// (where the feet stand) at the deck's top, so the feet rest on it. Where
/// the Keyblade then reaches below the ground (a guard, the keychain), the
/// board is raised clear and the returned lift says how far the rider must
/// rise with it.
pub fn deck_transform(points: &[Vec3]) -> (Transform, f32) {
    if points.is_empty() {
        return (Transform::IDENTITY, 0.);
    }
    let [mut along, across, _] = principal_axes(points);
    // The key end: the point farthest from the grip lies on the forward side.
    let tip = points
        .iter()
        .copied()
        .max_by(|a, b| a.length_squared().total_cmp(&b.length_squared()))
        .unwrap_or(Vec3::ZERO);
    if tip.dot(along) < 0. {
        along = -along;
    }
    // Columns: where the board's X (across), Y (up) and Z (along) come from.
    let up = along.cross(across);
    let rotation = Quat::from_mat3(&Mat3::from_cols(across, up, along).transpose());
    let local: Vec<Vec3> = points.iter().map(|&p| rotation * p).collect();
    let (low, high) = local.iter().fold(
        (Vec3::splat(f32::MAX), Vec3::splat(f32::MIN)),
        |(l, h), &p| (l.min(p), h.max(p)),
    );
    let long = high.z - low.z;
    let scale = if long > 0. { BOARD_LENGTH / long } else { 1. };
    let centre = (low + high) / 2.;
    // The board's top across its middle 60 %.
    let half = long * 0.3;
    let top = local
        .iter()
        .filter(|p| (p.z - centre.z).abs() <= half)
        .map(|p| p.y)
        .fold(f32::MIN, f32::max);
    let top = if top.is_finite() { top } else { centre.y };
    let lowest = (low.y - top) * scale + crate::skate::DECK_TOP;
    let lift = (GROUND + CLEARANCE - lowest).max(0.);
    let transform = Transform {
        translation: Vec3::new(-centre.x, -top, -centre.z) * scale
            + Vec3::Y * (crate::skate::DECK_TOP + lift),
        rotation,
        scale: Vec3::splat(scale),
    };
    (transform, lift)
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
    fn the_key_end_points_forward_along_the_board() {
        // A blade from the grip (origin) to its key end, a little wider in
        // X than in Z; straight, then turned diagonally.
        for tip_y in [100., -100.] {
            let mut points = Vec::new();
            for i in 0..=20 {
                let y = tip_y * i as f32 / 20.;
                points.extend([Vec3::new(-5., y, -1.), Vec3::new(5., y, 1.)]);
            }
            points.push(Vec3::new(0., tip_y, 0.));
            for turn in [
                Quat::IDENTITY,
                Quat::from_rotation_z(0.6) * Quat::from_rotation_x(0.4),
            ] {
                let turned: Vec<Vec3> = points.iter().map(|&p| turn * p).collect();
                let (t, _) = deck_transform(&turned);
                let tip = t.transform_point(turn * Vec3::new(0., tip_y, 0.));
                let grip = t.transform_point(Vec3::ZERO);
                assert!(
                    ((tip - grip).length() - BOARD_LENGTH).abs() < 0.5,
                    "{tip} {grip}"
                );
                assert!((tip - grip).normalize().z > 0.999, "{tip} {grip}");
            }
        }
    }

    #[test]
    fn a_board_reaching_below_the_ground_is_raised_clear() {
        // A thin blade with a deep guard hanging below it at the grip.
        let mut points = Vec::new();
        for i in 0..=20 {
            let y = 100. * i as f32 / 20.;
            points.extend([Vec3::new(-5., y, -1.), Vec3::new(5., y, 1.)]);
        }
        points.extend([Vec3::new(0., 2., -40.), Vec3::new(0., 100., 0.)]);
        let (t, lift) = deck_transform(&points);
        let lowest = points
            .iter()
            .map(|&p| t.transform_point(p).y)
            .fold(f32::MAX, f32::min);
        assert!(lift > 0.);
        assert!((lowest - (GROUND + CLEARANCE)).abs() < 1e-3, "{lowest}");
    }
}
