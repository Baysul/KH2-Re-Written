//! Melee hits and an enemy's hit reactions, translated from the PC
//! executable: hit-shape tests FUN_1403c94c0, swept attack spheres in
//! FUN_14040f610, damage FUN_1403c12f0, reaction classes FUN_1403ce980,
//! grounded flinch FUN_1403d43b0/FUN_1403d4960 and blow setup.
//!
//! Positions here are in the simulation's Y-up space. The enemy's movement
//! after a flinch or blow (decay of the slide speed and blow velocity) was
//! not traced; the braking below uses the SSTM blow-brake values as a
//! labelled test-room choice.

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

/// Enemy motion slots (direct slot = motion id for this set).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct EnemyMotions {
    pub idle: usize,
    /// Front-high, front-low, back-high, back-low flinches (10..13).
    pub flinch: [usize; 4],
    /// Blow front/back and their landings (16/17, 18/19).
    pub blow: [usize; 2],
    pub blow_land: [usize; 2],
    pub appear: usize,
    pub leave: usize,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum EnemyState {
    Appear,
    Idle,
    Flinch,
    Blow { back: bool },
    BlowLand,
    Leave,
    Gone { ticks: u32 },
}

/// Test-room choices for movement the trace did not cover.
pub const SLIDE_BRAKE: f32 = 0.8;
pub const BLOW_BRAKE_XZ: f32 = 0.8;
pub const BLOW_MIN_XZ: f32 = 0.1;
pub const RESPAWN_TICKS: u32 = 120;

#[derive(Clone, Debug, PartialEq)]
pub struct Enemy {
    pub position: Vec3,
    pub spawn: Vec3,
    pub yaw: f32,
    pub hp: i32,
    pub stats: TargetStats,
    pub state: EnemyState,
    /// Current motion slot and ticks since it started.
    pub motion: usize,
    pub motion_tick: u32,
    pub velocity: Vec3,
    slide: Vec3,
    /// Attack instances that already hit this enemy.
    hit_by: Vec<u32>,
}

/// Clip lengths in ticks, by slot.
pub trait ClipLengths {
    fn length(&self, slot: usize) -> u32;
}

impl Enemy {
    pub fn new(position: Vec3, yaw: f32, stats: TargetStats, motions: &EnemyMotions) -> Self {
        Enemy {
            position,
            spawn: position,
            yaw,
            hp: stats.max_hp,
            stats,
            state: EnemyState::Appear,
            motion: motions.appear,
            motion_tick: 0,
            velocity: [0.; 3],
            slide: [0.; 3],
            hit_by: Vec::new(),
        }
    }

    pub fn alive(&self) -> bool {
        !matches!(self.state, EnemyState::Leave | EnemyState::Gone { .. })
    }

    fn play(&mut self, slot: usize) {
        self.motion = slot;
        self.motion_tick = 0;
    }

    /// Applies one hit from attack instance `instance` travelling along
    /// `direction` (horizontal, from attacker to target). Returns the damage,
    /// or None when this instance already hit (one hit per window).
    pub fn hit(
        &mut self,
        instance: u32,
        direction: Vec3,
        high: bool,
        attack: &AttackParams,
        attacker: &AttackerStats,
        motions: &EnemyMotions,
    ) -> Option<i32> {
        if !self.alive() || self.hit_by.contains(&instance) {
            return None;
        }
        self.hit_by.push(instance);
        let dmg = damage(attacker, attack, &self.stats);
        self.hp = (self.hp - dmg).max(0);
        let forward = [self.yaw.sin(), 0., self.yaw.cos()];
        let back = dot(forward, direction) >= 0.;
        match reaction_class(attack.reaction) {
            4 => {
                let [up, speed] = attack.knockback.map(|v| v as f32);
                self.velocity = [direction[0] * speed, up, direction[2] * speed];
                self.state = EnemyState::Blow { back };
                self.play(motions.blow[back as usize]);
            }
            _ => {
                // Slide distance per frame (KB1 + stats 0x1c8) x 0.1.
                let speed = attack.knockback[0] as f32 * 0.1;
                self.slide = direction.map(|c| c * speed);
                self.state = EnemyState::Flinch;
                let index = (back as usize) * 2 + (!high) as usize;
                self.play(motions.flinch[index]);
            }
        }
        Some(dmg)
    }

    /// One 60 Hz tick. `ground` returns the floor height under a point.
    pub fn tick(
        &mut self,
        motions: &EnemyMotions,
        clips: &dyn ClipLengths,
        gravity: f32,
        face: Vec3,
        ground: &dyn Fn(Vec3) -> Option<f32>,
    ) {
        self.motion_tick += 1;
        let finished = self.motion_tick >= clips.length(self.motion);
        match self.state {
            EnemyState::Appear | EnemyState::Flinch | EnemyState::BlowLand if finished => {
                if self.hp <= 0 {
                    self.state = EnemyState::Leave;
                    self.play(motions.leave);
                } else {
                    self.state = EnemyState::Idle;
                    self.play(motions.idle);
                }
            }
            EnemyState::Idle => {
                if finished {
                    self.motion_tick = 0;
                }
                // Test-room choice: idle Shadows turn to face Sora.
                let d = sub(face, self.position);
                if d[0] * d[0] + d[2] * d[2] > 1. {
                    self.yaw = d[0].atan2(d[2]);
                }
            }
            EnemyState::Leave if finished => {
                self.state = EnemyState::Gone { ticks: 0 };
            }
            EnemyState::Gone { ticks } => {
                if ticks >= RESPAWN_TICKS {
                    self.position = self.spawn;
                    self.hp = self.stats.max_hp;
                    self.hit_by.clear();
                    self.state = EnemyState::Appear;
                    self.play(motions.appear);
                } else {
                    self.state = EnemyState::Gone { ticks: ticks + 1 };
                }
                return;
            }
            _ => {}
        }
        // Flinch slide.
        if self.slide.iter().any(|v| v.abs() > 1e-3) {
            self.position = [0, 1, 2].map(|k| self.position[k] + self.slide[k]);
            self.slide = self.slide.map(|v| v * SLIDE_BRAKE);
        }
        // Blow flight.
        if let EnemyState::Blow { back } = self.state {
            self.position = [0, 1, 2].map(|k| self.position[k] + self.velocity[k]);
            self.velocity[0] *= BLOW_BRAKE_XZ;
            self.velocity[2] *= BLOW_BRAKE_XZ;
            if self.velocity[0].hypot(self.velocity[2]) < BLOW_MIN_XZ {
                self.velocity[0] = 0.;
                self.velocity[2] = 0.;
            }
            self.velocity[1] -= gravity;
            if let Some(floor) = ground(self.position)
                && self.position[1] <= floor
                && self.velocity[1] <= 0.
            {
                self.position[1] = floor;
                self.velocity = [0.; 3];
                self.state = EnemyState::BlowLand;
                self.play(motions.blow_land[back as usize]);
            }
        } else if let Some(floor) = ground(self.position) {
            self.position[1] = floor;
        }
    }

    /// Clears the per-window hit record (a new swing may hit again).
    pub fn forget(&mut self, instance: u32) {
        self.hit_by.retain(|&i| i != instance);
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
