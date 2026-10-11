//! Limit Form's four limits, translated from the form's BDX program
//! (`limitform`, P_EX100_KH1F.mdlx 'limi') and the limit object around it
//! (FUN_1403d8b40 start, FUN_1403d9130 tick, FUN_1403d83e0 end): Sonic
//! Rave, Last Arcanum, Strike Raid and Infinity, their reaction-command
//! follow-ups, Strike Raid's flying Keyblade and Infinity's bullets.
//!
//! The limit runs two threads a tick, in this order [I: the order is
//! inferred]: the player's action thread (the motions a follow-up starts)
//! and the limit thread (TR22: keep alive while the player plays the last
//! motion the script started or an Infinity motion, offer the follow-ups
//! in the script's windows). Space is the test room's (Y up).

use crate::{Clips, Input, Player, Vec3, World};

/// The four limits and their follow-ups (03system `cmd`).
pub const SONIC_RAVE: u16 = 0x2ba;
pub const RAVE: u16 = 0x2bb;
pub const FINISH: u16 = 0x2bc;
pub const LAST_ARCANUM: u16 = 0x2bd;
pub const BASH: u16 = 0x2be;
pub const THE_END: u16 = 0x2bf;
pub const STRIKE_RAID: u16 = 0x2c0;
pub const RAID: u16 = 0x2c1;
pub const JUDGMENT: u16 = 0x2c2;
pub const INFINITY: u16 = 0x2ab;
pub const SHOOT: u16 = 0x2ac;
pub const IMPACT: u16 = 0x2ad;
pub const GIGA_IMPACT: u16 = 0x2ae;
/// `DAT_14074ebe8` / `DAT_1405c9678`: the Limit submenu's rows and the L1
/// shortcuts' replacements, in order.
pub const LIMITS: [u16; 4] = [SONIC_RAVE, LAST_ARCANUM, STRIKE_RAID, INFINITY];
/// The "Limit" command of the Magic slot (submenu 12).
pub const LIMIT_COMMAND: u16 = 0x2c3;

/// colour_filter_on's colours (L4909): Sonic Rave, Last Arcanum, Strike
/// Raid, Infinity; world 13 gets grey.
fn tint_of(cmd: u16) -> [u8; 3] {
    match cmd {
        SONIC_RAVE => [64, 64, 80],
        LAST_ARCANUM => [80, 64, 64],
        STRIKE_RAID => [80, 64, 80],
        _ => [64, 80, 64],
    }
}
/// The tint ramps in over 16 ticks and back out over 30 (FUN_1403d83e0,
/// `DAT_140623bcc`).
pub const TINT_IN: f32 = 16.;
pub const TINT_OUT: f32 = 30.;

/// Infinity's PTYA records, requested by id (set 0x42).
const INFINITY_CHAIN: [u8; 5] = [0x34, 0x35, 0x36, 0x37, 0x38];
/// Strike Raid's thrown-Keyblade attacks (ATKP 1866 Raid r160, 1867
/// Judgment r190) and Infinity's bullets (ATKP 1862, r80).
pub const RAID_ATKP: [u16; 2] = [1866, 1867];
pub const RAID_RADIUS: [f32; 2] = [160., 190.];
pub const BULLET_ATKP: u16 = 1862;
pub const BULLET_RADIUS: f32 = 80.;
/// vector_homing's rate in the bullet and Keyblade threads: 5° a tick.
const HOMING: f32 = 0.087_266;

/// The limit-row colour (FUN_1403d88e0): 5 while a limit runs or MP
/// charges, 2 (orange: usable, empties MP into a charge) when MP is short,
/// else 0.
pub fn colour(cost: u8, running: bool, mp: i32, charging: bool) -> usize {
    if running {
        5
    } else if cost < 1 {
        0
    } else if charging {
        5
    } else if mp < cost as i32 {
        2
    } else {
        0
    }
}

/// The player action a follow-up started (obj+0xd48); a new one replaces
/// the old.
#[derive(Clone, Copy, Debug, PartialEq)]
enum Stage {
    None,
    /// sonic_open: 211, then restart 212 when it takes over.
    SonicOpen,
    /// raid_open: 231 holds until its end, then the throw.
    RaidOpen,
    /// raid_throw: the throw motion until the catch (234).
    RaidThrow {
        motion: u16,
        judgment: bool,
        thrown: bool,
    },
    /// infinity_chain: wait for the motion to end, then the next record.
    InfinityChain {
        next: usize,
    },
    /// The charge motion 246: the orb at its mark.
    InfinityCharge,
}

/// Strike Raid's flying Keyblade (the `fly` thread, L3125 / L4124).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Keyblade {
    pub pos: Vec3,
    pub dir: Vec3,
    /// The wobble offset the Keyblade is drawn at (Judgment).
    pub offset: Vec3,
    pub judgment: bool,
    pub key: u64,
    homing: bool,
    side: Vec3,
    phase: f32,
    previous: Option<Vec3>,
}

/// One Infinity bullet (L285).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Bullet {
    pub pos: Vec3,
    pub dir: Vec3,
    pub key: u64,
    /// One of the three that chase the lock-on target.
    lock_on: bool,
    wait: f32,
    life: f32,
    target: Option<usize>,
    visited: u32,
    previous: Option<Vec3>,
}

/// What the room offers the limit this tick.
pub struct Scene<'a> {
    /// The enemies' TARGET shape centres; the first is the lock-on target.
    pub targets: &'a [Vec3],
    /// Sora's body centre (bone code 0x4001) and Keyblade top (0x4003).
    pub body: Vec3,
    pub keyblade_top: Vec3,
}

/// What a limit tick produced.
#[derive(Default)]
pub struct Output {
    /// The reaction command offered this tick (slot 1, Triangle).
    pub offer: Option<u16>,
    /// Effects of the form's PAX to start (bound to Sora), and at a point.
    pub effects: Vec<u16>,
    pub effects_at: Vec<(u16, Vec3)>,
    /// The limit ended this tick.
    pub ended: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Limit {
    pub cmd: u16,
    /// W0: the last motion the script started.
    w0: u16,
    /// W28 / W32: the commands of the range-16 / 17 windows; W36 the
    /// follow-up counter; W16 the Bash count; W8 Infinity's bullet count.
    w28: u16,
    w32: u16,
    w36: i32,
    w16: u16,
    w8: u32,
    stage: Stage,
    /// The motion and frame the action thread saw last tick.
    seen: (u16, u32, u32),
    pub tint: [u8; 3],
    pub keyblade: Option<Keyblade>,
    pub bullets: Vec<Bullet>,
    orb: bool,
    serial: u64,
    started: bool,
}

/// Rotates `v` about the unit `axis` by `a` (FUN_1403b7560).
fn rotate(v: Vec3, axis: Vec3, a: f32) -> Vec3 {
    let (s, c) = a.sin_cos();
    let d = dot(axis, v);
    let x = cross(axis, v);
    std::array::from_fn(|i| v[i] * c + x[i] * s + axis[i] * d * (1. - c))
}
fn dot(a: Vec3, b: Vec3) -> f32 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}
fn cross(a: Vec3, b: Vec3) -> Vec3 {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}
fn sub(a: Vec3, b: Vec3) -> Vec3 {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}
fn len(v: Vec3) -> f32 {
    dot(v, v).sqrt()
}
fn unit(v: Vec3) -> Vec3 {
    let l = len(v);
    if l > 1e-6 { v.map(|c| c / l) } else { v }
}
/// FUN_1403b7200: turn `d` toward `to` by at most `rate`.
fn home(d: Vec3, to: Vec3, rate: f32) -> Vec3 {
    let (d, t) = (unit(d), unit(to));
    let angle = dot(d, t).clamp(-1., 1.).acos();
    if angle <= rate || angle < 1e-6 {
        return t;
    }
    let s = angle.sin();
    let (a, b) = (((angle - rate).sin()) / s, rate.sin() / s);
    unit(std::array::from_fn(|i| d[i] * a + t[i] * b))
}

fn draw(seed: &mut u32) -> u32 {
    *seed = seed.wrapping_mul(0x10dcd);
    *seed
}
fn getf(seed: &mut u32, x: f32) -> f32 {
    draw(seed) as f32 * 2.328_306_4e-10 * x
}

impl Limit {
    /// FUN_1403d8b40 for a limit: MP is spent (below the cost: 0 and a
    /// charge), the limit thread starts (TR22) and dispatches the command.
    pub fn start(cmd: u16, cost: u8, player: &mut Player, clips: &Clips) -> Self {
        player
            .mp
            .change(-(cost as i32), crate::magic::CHARGE_LENGTH);
        player.mp_charging = player.mp.charging();
        let mut l = Limit {
            cmd,
            w0: 0,
            w28: 0,
            w32: 0,
            w36: 0,
            w16: 0,
            w8: 0,
            stage: Stage::None,
            seen: (u16::MAX, 0, 0),
            tint: tint_of(cmd),
            keyblade: None,
            bullets: Vec::new(),
            orb: false,
            serial: 0,
            started: false,
        };
        l.dispatch(cmd, player, clips);
        l
    }

    /// start_motion (L1994): W0 = id.
    fn start_motion(&mut self, player: &mut Player, clips: &Clips, id: u16, blend: f32) {
        self.w0 = id;
        player.play_motion(id, blend, clips);
    }

    /// dispatch (L1263) and TR23 for a follow-up.
    fn dispatch(&mut self, cmd: u16, player: &mut Player, clips: &Clips) {
        self.stage = Stage::None;
        match cmd {
            INFINITY => {
                self.w8 = 0;
                self.start_motion(player, clips, 241, 0.);
                self.stage = Stage::InfinityChain { next: 0 };
            }
            SHOOT | IMPACT | GIGA_IMPACT => {
                let (n, strength, motion) = match cmd {
                    SHOOT => (10, 50., 247),
                    IMPACT => (17, 100., 247),
                    _ => (25, 200., 248),
                };
                player.recoil_back(strength);
                self.start_motion(player, clips, motion, 0.);
                player.queue_motion(4, 0.);
                self.w8 = n;
                self.orb = false;
            }
            SONIC_RAVE => {
                (self.w28, self.w32, self.w36) = (RAVE, FINISH, 5);
                self.start_motion(player, clips, 211, 0.);
                player.queue_motion(212, 0.);
                player.queue_motion(0, 0.);
                self.stage = Stage::SonicOpen;
            }
            RAVE | FINISH => {
                player.turn_round();
                self.start_motion(player, clips, if cmd == RAVE { 212 } else { 213 }, 8.);
                player.queue_motion(0, 0.);
            }
            STRIKE_RAID => {
                (self.w28, self.w32, self.w36) = (RAID, JUDGMENT, 3);
                self.start_motion(player, clips, 231, 0.);
                self.stage = Stage::RaidOpen;
            }
            RAID | JUDGMENT => self.raid_throw(player, clips, cmd == JUDGMENT),
            LAST_ARCANUM => {
                self.w16 = 0;
                self.start_motion(player, clips, 221, 0.);
                player.queue_motion(0, 0.);
            }
            BASH => {
                self.w16 += 1;
                self.start_motion(player, clips, 221 + self.w16, 0.);
                player.queue_motion(0, 0.);
            }
            THE_END => {
                self.start_motion(player, clips, 227, 0.);
                player.queue_motion(0, 0.);
            }
            _ => {}
        }
    }

    /// raid_throw (L2848): the Keyblade back in the hand, the throw, then
    /// the catch motion.
    fn raid_throw(&mut self, player: &mut Player, clips: &Clips, judgment: bool) {
        self.keyblade = None;
        let motion = if judgment { 233 } else { 232 };
        self.start_motion(player, clips, motion, 0.);
        player.queue_motion(234, 0.);
        self.stage = Stage::RaidThrow {
            motion,
            judgment,
            thrown: false,
        };
    }

    /// A follow-up pressed (limit_exec's non-limit branch): W36 −= 1, then
    /// the dispatch. Returns whether it gives Limit Form EXP.
    pub fn follow_up(&mut self, cmd: u16, player: &mut Player, clips: &Clips) -> bool {
        self.w36 -= 1;
        self.dispatch(cmd, player, clips);
        crate::forms::LIMIT_EXP_COMMANDS.contains(&cmd)
    }

    /// The player's motion and frame, and whether the last motion the
    /// action thread watched ran to its end.
    fn watch(&mut self, player: &Player, clips: &Clips) -> (u16, u16, bool) {
        let m = player.motion_id(clips);
        let (prev, last, end) = self.seen;
        let ended = m != prev && prev != u16::MAX && last + 1 >= end;
        self.seen = (m, player.frame, player.clip(clips).end);
        (m, prev, ended)
    }

    /// One tick, after the player's: the action thread, then the limit
    /// thread. `seed` is the shared battle RNG.
    #[allow(clippy::too_many_arguments)]
    pub fn tick(
        &mut self,
        player: &mut Player,
        clips: &Clips,
        input: &Input,
        scene: &Scene,
        world: &impl World,
        seed: &mut u32,
    ) -> Output {
        let mut out = Output::default();
        let (m, prev, ended) = self.watch(player, clips);
        let frame = player.frame;
        let clip = player.clip(clips).clone();
        let mark = clip.marks.contains(&frame);
        match self.stage {
            Stage::SonicOpen if m != 211 => {
                if m == 212 {
                    self.start_motion(player, clips, 212, 0.);
                    player.queue_motion(0, 0.);
                }
                self.stage = Stage::None;
            }
            Stage::RaidOpen => {
                if m != 231 {
                    self.stage = Stage::None;
                } else if frame >= clip.end {
                    self.raid_throw(player, clips, false);
                }
            }
            Stage::RaidThrow {
                motion,
                judgment,
                thrown,
            } => {
                if m == motion {
                    if mark && !thrown {
                        self.throw(player, judgment, scene);
                        self.stage = Stage::RaidThrow {
                            motion,
                            judgment,
                            thrown: true,
                        };
                    }
                } else {
                    if m == 234 {
                        self.keyblade_return(player, clips, &mut out);
                    }
                    self.stage = Stage::None;
                }
            }
            Stage::InfinityChain { next } => {
                // motion_continues: the watched motion ran to its end (241
                // holds its last frame; an attack hands over to idle).
                if m != prev && !ended {
                    self.stage = Stage::None;
                } else if m != prev || frame >= clip.end {
                    if player.ptya_attack(INFINITY_CHAIN[next], input, clips) {
                        let started = player.motion_id(clips);
                        if next < 4 {
                            self.w0 = started;
                        }
                        self.seen = (started, player.frame, player.clip(clips).end);
                        self.stage = if next + 1 < INFINITY_CHAIN.len() {
                            Stage::InfinityChain { next: next + 1 }
                        } else {
                            Stage::InfinityCharge
                        };
                    } else {
                        self.stage = Stage::None;
                    }
                }
            }
            Stage::InfinityCharge => {
                if m == 246 {
                    if mark && !self.orb {
                        self.orb = true;
                        out.effects.push(20);
                    }
                } else {
                    self.orb = false;
                    self.stage = Stage::None;
                }
            }
            _ => {}
        }
        // TR17 (frame trigger 7 (100, 0)): Infinity's burst.
        if clip.events.iter().any(|&(f, a, _)| f == frame && a == 100) {
            self.burst(player, scene, seed);
        }
        self.step_keyblade(scene, world);
        self.step_bullets(scene, seed);
        // TR22's loop: alive while the player plays W0 or an Infinity
        // motion; the windows offer the follow-ups.
        let m = player.motion_id(clips);
        let alive = m == self.w0 || (241..=248).contains(&m);
        if !self.started {
            self.started = true;
        } else if !alive {
            if self.cmd == STRIKE_RAID && m == 234 {
                self.keyblade = None;
            }
            out.ended = true;
            return out;
        }
        let active = |id: u8| {
            clip.windows
                .iter()
                .find(|w| w.2 == id && (w.0..=w.1).contains(&frame))
                .map(|w| w.3)
        };
        if self.w36 > 0 && active(16).is_some() {
            out.offer = Some(self.w28);
        } else if self.w36 == 0 && active(17).is_some() {
            out.offer = Some(self.w32);
        } else if let Some(cmd) = active(20) {
            // Range 20 offers its own command (FUN_14040d810 case 0x14).
            out.offer = Some(cmd as u16);
        }
        out
    }

    /// spawn_keyblade: the throw along the facing.
    fn throw(&mut self, player: &Player, judgment: bool, scene: &Scene) {
        self.serial += 1;
        let dir = [player.yaw.sin(), 0., player.yaw.cos()];
        let up = [0., 1., 0.];
        let side = if scene.targets.is_empty() {
            up
        } else {
            // A right-hand side vector for the wobble.
            unit(cross(dir, up))
        };
        self.keyblade = Some(Keyblade {
            pos: scene.keyblade_top,
            dir,
            offset: [0.; 3],
            judgment,
            key: (1 << 59) | self.serial,
            homing: true,
            side,
            phase: 0.,
            previous: None,
        });
    }

    /// keyblade_return (L4738): the warp flash where it was, back in hand.
    fn keyblade_return(&mut self, player: &mut Player, clips: &Clips, out: &mut Output) {
        if let Some(k) = self.keyblade.take() {
            out.effects_at.push((34, k.pos));
        }
        self.start_motion(player, clips, 234, 0.);
        player.queue_motion(0, 8.);
    }

    /// The `fly` thread (L3125 / L4124): speed 30, homing 5° a tick on the
    /// lock-on target until within 120, held 120 / 150 above the ground,
    /// Judgment's wobble.
    fn step_keyblade(&mut self, scene: &Scene, world: &impl World) {
        let Some(k) = self.keyblade.as_mut() else {
            return;
        };
        let speed = 30.;
        if let Some(&t) = scene.targets.first()
            && k.homing
        {
            if len(sub(t, k.pos)) < speed * 4. {
                k.homing = false;
            }
            k.dir = home(k.dir, sub(t, k.pos), HOMING);
        }
        let prev = k.pos;
        let d = unit(k.dir);
        k.pos = std::array::from_fn(|i| k.pos[i] + d[i] * speed);
        let (reach, hold) = if k.judgment {
            (250., 150.)
        } else {
            (220., 120.)
        };
        if let Some(g) = world.floor(k.pos[0], k.pos[2], k.pos[1] + 100.)
            && g >= k.pos[1] + 100. - reach
        {
            k.pos[1] = g + hold;
        }
        let moved = sub(k.pos, prev);
        k.dir = std::array::from_fn(|i| moved[i] + 0.8 * (k.dir[i] - moved[i]));
        if k.judgment {
            k.phase = (k.phase + std::f32::consts::TAU / 20.).rem_euclid(std::f32::consts::TAU);
            let w = 30. * k.phase.sin() - 15.;
            k.offset = k.side.map(|c| c * w);
        }
        k.previous = Some(prev);
    }

    /// TR17's burst (L47): `W8` bullets in a ring about the facing from
    /// the Keyblade top; exactly three chase the lock-on target.
    fn burst(&mut self, player: &Player, scene: &Scene, seed: &mut u32) {
        let n = self.w8;
        let axis = [player.yaw.sin(), 0., player.yaw.cos()];
        let mut special = 3.;
        for i in 0..n {
            let dir = rotate(
                [0., 1., 0.],
                axis,
                i as f32 * std::f32::consts::TAU / n as f32,
            );
            let lock_on = if getf(seed, 1.) < special / (n - i) as f32 {
                special -= 1.;
                true
            } else {
                false
            };
            self.serial += 1;
            self.bullets.push(Bullet {
                pos: scene.keyblade_top,
                dir,
                key: (1 << 58) | self.serial,
                lock_on,
                wait: 6.,
                life: 0.,
                target: None,
                visited: 0,
                previous: None,
            });
        }
    }

    /// The bullet thread (L285): 6 ticks' wait, then 16 a tick homing 5°
    /// toward a goal skewed sideways, 150–180 ticks, gone on a hit.
    fn step_bullets(&mut self, scene: &Scene, seed: &mut u32) {
        for b in &mut self.bullets {
            if b.wait > 0. {
                b.wait -= 1.;
                if b.wait <= 0. {
                    b.life = 150. + getf(seed, 30.);
                    b.target = if b.lock_on {
                        (!scene.targets.is_empty()).then_some(0)
                    } else {
                        let k = scene.targets.len().min(10) as u32;
                        (k > 0).then(|| {
                            draw(seed);
                            (*seed / (u32::MAX / k).max(1)).min(k - 1) as usize
                        })
                    };
                }
                continue;
            }
            if b.target.is_none() {
                // search_next: the nearest part not visited yet.
                b.target = (0..scene.targets.len())
                    .filter(|&i| b.visited >> i.min(31) & 1 == 0)
                    .min_by(|&x, &y| {
                        len(sub(scene.targets[x], b.pos))
                            .total_cmp(&len(sub(scene.targets[y], b.pos)))
                    });
            }
            let mix = 0.2 + 0.6 * getf(seed, 1.);
            let goal = match b.target.and_then(|t| scene.targets.get(t)) {
                Some(&t) => {
                    let g = sub(t, b.pos);
                    if len(g) < 64. {
                        b.visited |= 1 << b.target.unwrap_or(0).min(31);
                        b.target = None;
                    }
                    g
                }
                None => {
                    // bg_get_random_pos_air about the body centre [U].
                    let r = [
                        getf(seed, 200.) - 100.,
                        getf(seed, 200.) - 100.,
                        getf(seed, 200.) - 100.,
                    ];
                    sub(std::array::from_fn(|i| scene.body[i] + r[i]), b.pos)
                }
            };
            let g = unit(goal);
            let side = unit(cross(g, b.dir));
            let goal = std::array::from_fn(|i| g[i] * (1. - mix) + side[i] * mix);
            b.dir = home(b.dir, goal, HOMING);
            b.previous = Some(b.pos);
            b.pos = std::array::from_fn(|i| b.pos[i] + b.dir[i] * 16.);
            b.life -= 1.;
        }
        self.bullets.retain(|b| b.wait > 0. || b.life > 0.);
    }

    /// The attack volumes alive this tick: (key, ATKP, centre, radius,
    /// last centre, direction).
    pub fn volumes(&self) -> Vec<(u64, u16, Vec3, f32, Option<Vec3>, Vec3)> {
        let mut v = Vec::new();
        if let Some(k) = &self.keyblade {
            let j = k.judgment as usize;
            v.push((
                k.key,
                RAID_ATKP[j],
                k.pos,
                RAID_RADIUS[j],
                k.previous,
                k.dir,
            ));
        }
        for b in self.bullets.iter().filter(|b| b.wait <= 0.) {
            v.push((b.key, BULLET_ATKP, b.pos, BULLET_RADIUS, b.previous, b.dir));
        }
        v
    }

    /// An attack hit (attack_is_hit): a bullet ends; the Keyblade hits
    /// once per throw and keeps flying.
    pub fn hit(&mut self, key: u64) {
        self.bullets.retain(|b| b.key != key);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn limit_rows_grey_while_charging_or_running_and_orange_when_short() {
        assert_eq!(colour(60, false, 100, false), 0);
        assert_eq!(colour(60, false, 40, false), 2);
        assert_eq!(colour(60, false, 100, true), 5);
        assert_eq!(colour(60, true, 100, false), 5);
    }

    #[test]
    fn the_ring_turns_about_the_facing() {
        let up = rotate([0., 1., 0.], [0., 0., 1.], std::f32::consts::FRAC_PI_2);
        assert!((up[0].abs() - 1.).abs() < 1e-5 && up[1].abs() < 1e-5);
        let h = home([1., 0., 0.], [0., 1., 0.], HOMING);
        assert!((h[1] - HOMING.sin()).abs() < 1e-5);
    }
}
