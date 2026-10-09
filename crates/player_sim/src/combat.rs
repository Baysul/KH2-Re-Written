//! Melee hits and an enemy's hit reactions, translated from the PC
//! executable: hit-shape tests FUN_1403c94c0, swept attack spheres in
//! FUN_14040f610, damage FUN_1403c12f0, reaction classes FUN_1403ce980,
//! and the reaction classes used by the enemy (see `shadow`).
//!
//! Positions here are in the simulation's Y-up space.

pub type Vec3 = [f32; 3];

fn sub(a: Vec3, b: Vec3) -> Vec3 {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}
fn dot(a: Vec3, b: Vec3) -> f32 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

/// Strict sphere overlap: |c1 - c2|^2 < (r1 + r2)^2.
pub fn spheres_overlap(a: Vec3, ra: f32, b: Vec3, rb: f32) -> bool {
    let d = sub(a, b);
    dot(d, d) < (ra + rb) * (ra + rb)
}

/// Attack sphere centres for one frame, with extra copies filling the gap
/// back toward last frame's centre whenever it is farther than 1.5 radii
/// (copies every radius, 0x1405b22e0 = 1.5).
pub fn swept(current: Vec3, previous: Option<Vec3>, radius: f32) -> Vec<Vec3> {
    let mut out = vec![current];
    let Some(prev) = previous else { return out };
    let delta = sub(prev, current);
    let mut dist = dot(delta, delta).sqrt();
    if dist <= 0. || radius <= 0. {
        return out;
    }
    let dir = delta.map(|c| c / dist);
    let mut at = current;
    while dist > 1.5 * radius {
        at = [0, 1, 2].map(|k| at[k] + dir[k] * radius);
        out.push(at);
        dist -= radius;
    }
    out
}

/// Attack parameters used by the damage routine (from ATKP).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct AttackParams {
    pub power: u16,
    /// 0 strength, 1 magic, else no stat.
    pub stat: u8,
    pub element: u8,
    pub reaction: u8,
    pub knockback: [i16; 2],
}

/// Attacker stats (player: LVUP + PLRP + equipment).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct AttackerStats {
    pub strength: i32,
    pub magic: i32,
    /// Clamp defaults for a player stat struct: 99999 / 1.
    pub max: i32,
    pub min: i32,
}

/// Target stats (enemy: ENMP x LVPM).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TargetStats {
    pub max_hp: i32,
    pub defense: i32,
    /// (enmp cap x maxHP + 9999) / 10000 and the same for the minimum.
    pub cap: i32,
    pub min: i32,
    /// Weakness percents: physical..light, then general.
    pub weakness: [i32; 7],
}

impl TargetStats {
    /// Enemy stats: HP pool = (hp% x lvpm HP + 99) / 100; cap/minimum are
    /// per 10000 of max HP (FUN_1403c2240).
    pub fn enemy(
        hp_percent: u16,
        damage_cap: u16,
        damage_min: u16,
        weakness: [u16; 7],
        level_hp: u16,
        level_defense: u16,
    ) -> Self {
        let max_hp = (hp_percent as i32 * level_hp as i32 + 99) / 100;
        TargetStats {
            max_hp,
            defense: level_defense as i32,
            cap: (damage_cap as i32 * max_hp + 9999) / 10000,
            min: (damage_min as i32 * max_hp + 9999) / 10000,
            weakness: weakness.map(|w| w as i32),
        }
    }
}

/// Damage of a normal (non-cure) attack at Standard difficulty
/// (FUN_1403c12f0 default branch; the low-HP defence bonus, special modes
/// and ability boosts are zero here).
pub fn damage(attacker: &AttackerStats, attack: &AttackParams, target: &TargetStats) -> i32 {
    let mut s = match attack.stat {
        0 => attacker.strength,
        1 => attacker.magic,
        _ => 0,
    };
    s -= target.defense;
    s = s.min(attacker.max.min(target.cap));
    s = s.max(attacker.min.max(target.min));
    let weak = target.weakness[(attack.element as usize).min(5)] as i64;
    let general = target.weakness[6] as i64;
    ((weak * general * attack.power as i64 * s as i64 + 999_999) / 1_000_000) as i32
}

/// Reaction class (FUN_1403ce980): 3 flinch, 4 blow.
pub fn reaction_class(reaction: u8) -> u8 {
    match reaction {
        1 | 12 => 1,
        3 | 6 | 8 | 10 | 13 | 16 | 18 => 3,
        4 | 7 | 9 | 11 | 15 | 17 => 4,
        r => r,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn shadow(level_hp: u16, level_def: u16) -> TargetStats {
        TargetStats::enemy(50, 10000, 1000, [100; 7], level_hp, level_def)
    }

    #[test]
    fn damage_matches_the_traced_level_one_numbers() {
        let sora = AttackerStats {
            strength: 5,
            magic: 7,
            max: 99999,
            min: 1,
        };
        let t = shadow(20, 2);
        assert_eq!((t.max_hp, t.cap, t.min), (10, 10, 1));
        let hit = |power| AttackParams {
            power,
            stat: 0,
            element: 0,
            reaction: 3,
            knockback: [125, 0],
        };
        assert_eq!(damage(&sora, &hit(100), &t), 3);
        assert_eq!(damage(&sora, &hit(65), &t), 2);
        assert_eq!(damage(&sora, &hit(200), &t), 6);
        // Level 3: HP 12, defence 3; the 10% minimum floor gives 2.
        let t3 = shadow(23, 3);
        assert_eq!(t3.max_hp, 12);
        assert_eq!(damage(&sora, &hit(100), &t3), 2);
    }

    #[test]
    fn sweep_fills_large_moves_and_spheres_are_strict() {
        assert_eq!(swept([0.; 3], Some([100., 0., 0.]), 40.).len(), 2);
        assert_eq!(swept([0.; 3], Some([130., 0., 0.]), 40.).len(), 3);
        assert_eq!(swept([0.; 3], Some([50., 0., 0.]), 40.).len(), 1);
        assert!(spheres_overlap([0.; 3], 1., [1.9, 0., 0.], 1.));
        assert!(!spheres_overlap([0.; 3], 1., [2., 0., 0.], 1.));
        assert_eq!(reaction_class(7), 4);
        assert_eq!(reaction_class(3), 3);
    }
}
