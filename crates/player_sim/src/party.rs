//! Party members (Donald, Goofy) as the executable's `kn::Friend` objects:
//! the follow behaviour of FUN_1401b41a0 / FUN_1401b4820, the slot
//! formation, and the attack choice of FUN_1401b1d40 with its action gate
//! FUN_1401b2880, on the shared movement rules.
//!
//! The thresholds are the personality resource's header floats read by
//! FUN_1401cc890 / FUN_1401cc8c0; the formation distance is not decoded
//! (`DAT_14071d760` is filled at run time) and is a test-room value. The
//! friend AI keeps no combo chain: each decision picks one eligible record
//! at random, never the same one three times in a row.

use crate::{Body, Vec3, World};

/// Walk and run speeds per tick (PRTY words 0 and 1: 2.0 and 8.0).
pub const WALK_SPEED: f32 = 2.;
pub const RUN_SPEED: f32 = 8.;
/// Distances at which the gait steps up (from stand, from walk) and down
/// (to walk, to stand) [personality header pairs (0,200) (160,240)
/// (200,1000)].
const UP: [f32; 2] = [200., 240.];
const DOWN: [f32; 2] = [200., 160.];
/// Turn rate per tick (prty +0xc = 0.262).
const TURN: f32 = 0.262;
/// Slot yaw offset (the exe uses 0.349 × 2i − 0.349; widened here so the
/// two bodies do not overlap, as the formation table is not decoded) and
/// the formation distance behind the leader [test-room value].
const SLOT_YAW: f32 = 0.6;
const FORMATION: f32 = 150.;
/// Target search radius and the leash from the leader (personality +0x68,
/// +0x80).
pub const SEARCH_RADIUS: f32 = 800.;
pub const LEADER_LEASH: f32 = 1200.;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Gait {
    Stand,
    Walk,
    Run,
}

/// One action parameter entry (record of 0x80 bytes): the attack's motion
/// and its range window (FUN_1401b2880).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Record {
    pub id: u8,
    pub motion: u16,
    pub near: f32,
    pub far: f32,
    /// The target's height above the member must lie in
    /// `[low − r, high + r]` (the exe's Y-down window, negated).
    pub low: f32,
    pub high: f32,
    /// Air records jump first (+0x20 ≠ 0); not played yet.
    pub air: bool,
}

const fn ground(id: u8, motion: u16, far: f32) -> Record {
    Record {
        id,
        motion,
        near: 0.,
        far,
        low: -150.,
        high: 100.,
        air: false,
    }
}

/// Donald's ground attack records (resource at 0x14071d850).
pub const DONALD: [Record; 2] = [ground(0, 151, 200.), ground(1, 152, 180.)];
/// Goofy's ground attack records (personality 1).
pub const GOOFY: [Record; 2] = [ground(4, 151, 220.), ground(5, 152, 300.)];

/// The enemy a member fights: its centre and radius.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Target {
    pub position: Vec3,
    pub radius: f32,
}

/// An attack in progress.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Attack {
    pub record: u8,
    pub motion: u16,
    pub tick: u32,
}

/// A party member that follows the leader and fights.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Follower {
    pub position: Vec3,
    pub yaw: f32,
    pub gait: Gait,
    /// Party slot 0 or 1 (obj+0xdd0).
    pub slot: u8,
    /// Ticks in the current motion.
    pub tick: u32,
    pub attack: Option<Attack>,
    /// The last three chosen records (obj+0xed0..0xee0).
    history: [u8; 3],
    seed: u32,
}

fn wrap(a: f32) -> f32 {
    let tau = std::f32::consts::TAU;
    (a + std::f32::consts::PI).rem_euclid(tau) - std::f32::consts::PI
}

/// What a tick needs from the room.
pub struct Scene<'a, W: World> {
    pub leader: Vec3,
    pub leader_yaw: f32,
    /// The enemy to fight, when one can be hit and a fight is on.
    pub target: Option<Target>,
    pub world: &'a W,
    pub body: Body,
    /// Ticks of each attack motion (None: not loaded).
    pub length: &'a dyn Fn(u16) -> Option<u32>,
    /// Root-motion displacement of a motion at a tick (model space).
    pub root: &'a dyn Fn(u16, u32) -> Vec3,
}

impl Follower {
    /// FUN_1403c2fc0: at the leader's position, facing its direction.
    pub fn spawn(leader: Vec3, leader_yaw: f32, slot: u8) -> Self {
        Follower {
            position: leader,
            yaw: leader_yaw,
            gait: Gait::Stand,
            slot,
            tick: 0,
            attack: None,
            history: [255; 3],
            seed: 0x1234_5678 ^ (slot as u32 * 0x9e37_79b9),
        }
    }

    /// The motion id playing: an attack, or the gait (idle 0, walk 1,
    /// run 2).
    pub fn motion(&self) -> u16 {
        if let Some(a) = self.attack {
            return a.motion;
        }
        match self.gait {
            Gait::Stand => 0,
            Gait::Walk => 1,
            Gait::Run => 2,
        }
    }

    fn rand(&mut self, n: u32) -> u32 {
        // CRT-style LCG [the seed is unknown, U].
        self.seed = self.seed.wrapping_mul(214_013).wrapping_add(2_531_011);
        (self.seed >> 16) % n.max(1)
    }

    /// The goal point: behind the leader at the slot's yaw offset.
    pub fn goal(&self, leader: Vec3, leader_yaw: f32) -> Vec3 {
        let offset = SLOT_YAW * 2. * self.slot as f32 - SLOT_YAW;
        let a = leader_yaw + offset;
        [
            leader[0] - a.sin() * FORMATION,
            leader[1],
            leader[2] - a.cos() * FORMATION,
        ]
    }

    /// FUN_1401b2880's range test of one record against the target.
    pub fn eligible(&self, r: &Record, t: &Target) -> bool {
        if r.air {
            return false;
        }
        let dx = t.position[0] - self.position[0];
        let dz = t.position[2] - self.position[2];
        let d = dx.hypot(dz);
        let rise = t.position[1] - self.position[1];
        let front = self.yaw.sin() * dx + self.yaw.cos() * dz >= 0.;
        d >= r.near - t.radius
            && d <= r.far + t.radius
            && rise >= r.low - t.radius
            && rise <= r.high + t.radius
            && front
    }

    /// FUN_1401b1d40 over the attack records: the random pick among the
    /// eligible ones, skipping a record chosen three times running.
    fn choose(&mut self, records: &[Record], t: &Target) -> Option<Record> {
        let candidates: Vec<Record> = records
            .iter()
            .copied()
            .filter(|r| self.eligible(r, t))
            .collect();
        if candidates.is_empty() {
            return None;
        }
        let mut pick = candidates[self.rand(candidates.len() as u32) as usize];
        if self.history.iter().all(|&h| h == pick.id)
            && let Some(other) = candidates.iter().find(|r| r.id != pick.id)
        {
            pick = *other;
        }
        self.history = [self.history[1], self.history[2], pick.id];
        Some(pick)
    }

    /// One tick: finish or continue an attack, else choose one against the
    /// target, else approach the target or follow the leader.
    pub fn tick<W: World>(&mut self, records: &[Record], scene: &Scene<W>) {
        // An attack plays out its motion with root motion, then ends.
        if let Some(mut a) = self.attack {
            a.tick += 1;
            let (s, c) = self.yaw.sin_cos();
            let m = (scene.root)(a.motion, a.tick);
            let p = (scene.root)(a.motion, a.tick.saturating_sub(1));
            let (dx, dz) = (m[0] - p[0], m[2] - p[2]);
            let disp = [dx * c + dz * s, 0., -dx * s + dz * c];
            let moved = scene.world.collide(self.position, disp, false, &scene.body);
            self.position = moved.position;
            self.tick = a.tick;
            let end = (scene.length)(a.motion).unwrap_or(0);
            self.attack = (a.tick < end).then_some(a);
            if self.attack.is_none() {
                self.tick = 0;
            }
            return;
        }
        // Decide: an eligible attack starts at once (FUN_1401b1d40).
        if let Some(t) = scene.target
            && let Some(r) = self.choose(records, &t)
            && (scene.length)(r.motion).is_some()
        {
            let to = [
                t.position[0] - self.position[0],
                t.position[2] - self.position[2],
            ];
            if to[0].hypot(to[1]) > 1e-3 {
                self.yaw = to[0].atan2(to[1]);
            }
            self.attack = Some(Attack {
                record: r.id,
                motion: r.motion,
                tick: 0,
            });
            self.gait = Gait::Stand;
            self.tick = 0;
            return;
        }
        // Movement: toward the target in a fight, else behind the leader.
        let goal = match scene.target {
            Some(t) => t.position,
            None => self.goal(scene.leader, scene.leader_yaw),
        };
        let d = [goal[0] - self.position[0], goal[2] - self.position[2]];
        let dist = d[0].hypot(d[1]);
        let before = self.gait;
        self.gait = match self.gait {
            Gait::Stand if dist >= UP[0] => Gait::Walk,
            Gait::Walk if dist >= UP[1] => Gait::Run,
            Gait::Walk if dist <= DOWN[1] => Gait::Stand,
            Gait::Run if dist <= DOWN[0] => Gait::Walk,
            g => g,
        };
        // In a fight there is no standing at a distance: close in at a run.
        if scene.target.is_some() && self.gait != Gait::Run && dist > 60. {
            self.gait = Gait::Run;
        }
        if self.gait != before {
            self.tick = 0;
        } else {
            self.tick += 1;
        }
        let speed = match self.gait {
            Gait::Stand => 0.,
            Gait::Walk => WALK_SPEED,
            Gait::Run => RUN_SPEED,
        };
        if speed > 0. && dist > 1e-3 {
            let want = d[0].atan2(d[1]);
            let delta = wrap(want - self.yaw);
            self.yaw = wrap(self.yaw + delta.clamp(-TURN, TURN));
            let step = speed.min(dist);
            let disp = [self.yaw.sin() * step, 0., self.yaw.cos() * step];
            let moved = scene.world.collide(self.position, disp, false, &scene.body);
            self.position = moved.position;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Collided;

    struct Flat;
    impl World for Flat {
        fn floor(&self, _: f32, _: f32, _: f32) -> Option<f32> {
            Some(0.)
        }
        fn resolve_walls(&self, p: [f32; 2], _: f32, _: f32, _: f32) -> [f32; 2] {
            p
        }
        fn collide(&self, start: Vec3, d: Vec3, _: bool, _: &Body) -> Collided {
            Collided {
                position: [start[0] + d[0], 0., start[2] + d[2]],
                on_floor: true,
            }
        }
    }
    const BODY: Body = Body {
        radius: 30.,
        height: 140.,
        step: 24.,
    };
    fn length(m: u16) -> Option<u32> {
        matches!(m, 151 | 152).then_some(60)
    }
    fn no_root(_: u16, _: u32) -> Vec3 {
        [0.; 3]
    }
    fn scene(leader: Vec3, target: Option<Target>, world: &Flat) -> Scene<'_, Flat> {
        Scene {
            leader,
            leader_yaw: 0.,
            target,
            world,
            body: BODY,
            length: &length,
            root: &no_root,
        }
    }

    #[test]
    fn a_distant_member_walks_then_runs_and_settles_at_the_goal() {
        let mut f = Follower::spawn([0., 0., 0.], 0., 0);
        let leader = [0., 0., 600.];
        let mut gaits = Vec::new();
        for _ in 0..400 {
            f.tick(&DONALD, &scene(leader, None, &Flat));
            if gaits.last() != Some(&f.gait) {
                gaits.push(f.gait);
            }
        }
        assert_eq!(gaits, vec![Gait::Walk, Gait::Run, Gait::Walk, Gait::Stand]);
        let goal = f.goal(leader, 0.);
        let d = (goal[0] - f.position[0]).hypot(goal[2] - f.position[2]);
        assert!(d < 200., "{d}");
    }

    #[test]
    fn the_two_slots_stand_on_either_side_behind_the_leader() {
        let a = Follower::spawn([0.; 3], 0., 0).goal([0.; 3], 0.);
        let b = Follower::spawn([0.; 3], 0., 1).goal([0.; 3], 0.);
        assert!(a[0] > 0. && b[0] < 0. || a[0] < 0. && b[0] > 0.);
        assert!(a[2] < 0. && b[2] < 0.);
    }

    #[test]
    fn a_member_closes_in_on_the_enemy_and_attacks_in_range() {
        let mut f = Follower::spawn([0.; 3], 0., 0);
        let t = Target {
            position: [0., 0., 500.],
            radius: 40.,
        };
        let mut started = None;
        for tick in 0..200 {
            f.tick(&DONALD, &scene([0.; 3], Some(t), &Flat));
            if f.attack.is_some() && started.is_none() {
                started = Some((tick, f.position[2]));
            }
        }
        let (_, z) = started.expect("an attack starts");
        // Within the longest record's reach plus the target's radius.
        assert!(500. - z <= 200. + 40. + 1., "{z}");
    }

    #[test]
    fn attacks_play_to_their_end_and_never_repeat_three_times() {
        let mut f = Follower::spawn([0., 0., 400.], 0., 0);
        let t = Target {
            position: [0., 0., 480.],
            radius: 40.,
        };
        let mut chosen = Vec::new();
        for _ in 0..3000 {
            let before = f.attack;
            f.tick(&DONALD, &scene([0.; 3], Some(t), &Flat));
            if before.is_none()
                && let Some(a) = f.attack
            {
                chosen.push(a.record);
            }
        }
        assert!(chosen.len() > 10, "{}", chosen.len());
        assert!(
            chosen
                .windows(4)
                .all(|w| !(w[0] == w[1] && w[1] == w[2] && w[2] == w[3]))
        );
        assert!(chosen.contains(&0) && chosen.contains(&1));
    }
}
