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
//!
//! Donald's four spells are records too (57-60, `FUN_1401cc650` /
//! `FUN_1401cc1a0`): they need the think loop's random MP-scaled permit
//! (`FUN_1401b53b0`) and an MP bar that is not charging, and play the act of
//! `party_spell`.

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

/// The spells of Donald's `p_ex020` acts (`fire`, `blizzard`, `thunder`,
/// `cure`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Spell {
    Fire,
    Blizzard,
    Thunder,
    Cure,
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
    /// MP cost (+8); 255 starts an MP Charge.
    pub cost: u8,
    /// The act this record starts instead of a weapon motion.
    pub spell: Option<Spell>,
    /// Cure: a member is healed while its HP is below this share of its
    /// maximum (+0x3c).
    pub heal_below: f32,
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
        cost: 0,
        spell: None,
        heal_below: 0.,
    }
}

/// A spell record: the act's motion, its range window (distances and the
/// Y-down height window negated into `low`/`high`) and MP cost.
const fn spell(id: u8, spell: Spell, motion: u16, window: [f32; 4], cost: u8) -> Record {
    Record {
        id,
        motion,
        near: window[0],
        far: window[1],
        low: window[2],
        high: window[3],
        air: false,
        cost,
        spell: Some(spell),
        heal_below: 0.,
    }
}

/// Donald's records (resource at 0x14071d850): the two ground attacks and
/// the spells 57-60 (Fire 0-350, Blizzard 250-750, Thunder 0-500 with
/// their height windows; Cure costs 255 and heals below 0.4 of the HP).
/// The act motions are 192 (Fire, Thunder, Cure) and 191 (Blizzard).
pub const DONALD: [Record; 6] = [
    ground(0, 151, 200.),
    ground(1, 152, 180.),
    spell(57, Spell::Fire, 192, [0., 350., -200., 100.], 10),
    spell(58, Spell::Blizzard, 191, [250., 750., -150., 100.], 10),
    spell(59, Spell::Thunder, 192, [0., 500., -1000., 500.], 10),
    Record {
        heal_below: 0.4,
        ..spell(60, Spell::Cure, 192, [0., 0., 0., 0.], 255)
    },
];
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
    /// The spell this attack is the cast of.
    pub spell: Option<Spell>,
    /// Cure: the party slot healed (0 Sora, 1 Donald, 2 Goofy).
    pub heal: u8,
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
    /// The think loop's spell permit (obj+0xe18 bit 5), rolled each
    /// decision.
    pub permit: bool,
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
    /// A spell act is still running (the act manager's lock, obj+0x4a8):
    /// no new decision.
    pub busy: bool,
    /// The AI may choose spells (the tour orders its casts instead).
    pub spells: bool,
    /// This member's MP: current, maximum and whether an MP Charge runs.
    pub mp: Option<(i32, i32, bool)>,
    /// HP and maximum HP of Sora, Donald and Goofy (None: absent).
    pub hp: [Option<(i32, i32)>; 3],
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
            permit: false,
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
        // Cure's record reads 0-0 as a range, which the original's check
        // would turn into "touching the enemy"; the cast waits on the
        // heal target instead [U: the record's flags were not decoded].
        if r.spell == Some(Spell::Cure) {
            return true;
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

    /// Partner-in-need (obj+0xf78, FUN_1401b53b0): Sora when his HP is at
    /// most a quarter of the maximum, else the other member under it.
    fn partner_in_need(&self, hp: &[Option<(i32, i32)>; 3]) -> Option<usize> {
        let own = self.slot as usize + 1;
        [0, 3 - own]
            .into_iter()
            .find(|&i| hp[i].is_some_and(|(cur, max)| cur * 4 <= max))
    }

    /// FUN_1401cc1a0: the member Cure heals: the partner-in-need, else
    /// the first of itself, Sora and the other member whose HP is under
    /// `max × ratio` (FUN_1401b3310 truncates the product).
    pub fn heal_target(&self, ratio: f32, hp: &[Option<(i32, i32)>; 3]) -> Option<usize> {
        if let Some(i) = self.partner_in_need(hp) {
            return Some(i);
        }
        let own = self.slot as usize + 1;
        [own, 0, 3 - own]
            .into_iter()
            .find(|&i| hp[i].is_some_and(|(cur, max)| cur < (max as f32 * ratio) as i32))
    }

    /// The spell gate of FUN_1401b2880: a record with a cost needs the
    /// permit and an MP bar that is not charging; Cure also a member to
    /// heal.
    fn spell_ready<W: World>(&self, r: &Record, scene: &Scene<W>) -> bool {
        let Some(spell) = r.spell else { return true };
        if !scene.spells {
            return false;
        }
        if let Some((_, _, charging)) = scene.mp
            && r.cost >= 1
            && !(self.permit && !charging)
        {
            return false;
        }
        spell != Spell::Cure || self.heal_target(r.heal_below, &scene.hp).is_some()
    }

    /// FUN_1401b1d40 over the attack records: the random pick among the
    /// eligible ones, skipping a record chosen three times running.
    fn choose<W: World>(
        &mut self,
        records: &[Record],
        t: &Target,
        scene: &Scene<W>,
    ) -> Option<Record> {
        let candidates: Vec<Record> = records
            .iter()
            .copied()
            .filter(|r| {
                (scene.length)(r.motion).is_some()
                    && self.eligible(r, t)
                    && self.spell_ready(r, scene)
            })
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

    /// Starts a record's motion toward the target (FUN_1401b7d30 for an
    /// attack, FUN_1403caa90 for a spell act).
    pub fn begin(&mut self, r: &Record, t: &Target, heal: u8) {
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
            spell: r.spell,
            heal,
        });
        self.gait = Gait::Stand;
        self.tick = 0;
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
        // A running spell act holds the member where it stands.
        if scene.busy {
            self.gait = Gait::Stand;
            self.tick += 1;
            return;
        }
        // The think loop's spell permit: MP share scaled (FUN_1401b53b0).
        if let Some((cur, max, _)) = scene.mp {
            let p = (cur as f32 / max.max(1) as f32) * 0.75 + 0.25;
            self.permit = self.rand(20001) as f32 / 20000. <= p;
        }
        // Decide: an eligible attack starts at once (FUN_1401b1d40).
        if let Some(t) = scene.target
            && let Some(r) = self.choose(records, &t, scene)
            && (scene.length)(r.motion).is_some()
        {
            let heal = if r.spell == Some(Spell::Cure) {
                self.heal_target(r.heal_below, &scene.hp).unwrap_or(0) as u8
            } else {
                0
            };
            self.begin(&r, &t, heal);
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
            busy: false,
            spells: false,
            mp: None,
            hp: [None; 3],
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

    fn spell_length(m: u16) -> Option<u32> {
        matches!(m, 151 | 152 | 191 | 192).then_some(60)
    }

    fn spell_scene<'a>(
        t: Option<Target>,
        world: &'a Flat,
        mp: (i32, i32, bool),
        hp: [Option<(i32, i32)>; 3],
    ) -> Scene<'a, Flat> {
        Scene {
            leader: [0.; 3],
            leader_yaw: 0.,
            target: t,
            world,
            body: BODY,
            length: &spell_length,
            root: &no_root,
            busy: false,
            spells: true,
            mp: Some(mp),
            hp,
        }
    }

    #[test]
    fn spells_need_the_permit_and_an_mp_bar_that_is_not_charging() {
        let t = Target {
            position: [0., 0., 300.],
            radius: 40.,
        };
        // Full MP: the permit odds are 1; the member casts something.
        let mut f = Follower::spawn([0., 0., 100.], 0., 0);
        let mut spells = 0;
        for _ in 0..4000 {
            f.tick(
                &DONALD,
                &spell_scene(Some(t), &Flat, (100, 100, false), [Some((20, 20)); 3]),
            );
            if f.attack.is_some_and(|a| a.spell.is_some() && a.tick == 0) {
                spells += 1;
            }
        }
        assert!(spells > 5, "{spells}");
        // An MP Charge running: no spell is ever chosen.
        let mut f = Follower::spawn([0., 0., 100.], 0., 0);
        for _ in 0..4000 {
            f.tick(
                &DONALD,
                &spell_scene(Some(t), &Flat, (0, 100, true), [Some((20, 20)); 3]),
            );
            assert!(f.attack.is_none_or(|a| a.spell.is_none()));
        }
    }

    #[test]
    fn blizzard_waits_for_distance_and_fire_for_the_enemy_to_be_near() {
        // At 600 only Blizzard (250-750) is in range.
        let mut f = Follower::spawn([0., 0., 0.], 0., 0);
        f.permit = true;
        let t = Target {
            position: [0., 0., 600.],
            radius: 40.,
        };
        let scene = spell_scene(Some(t), &Flat, (100, 100, false), [Some((20, 20)); 3]);
        let ready: Vec<u8> = DONALD
            .iter()
            .filter(|r| f.eligible(r, &t) && f.spell_ready(r, &scene))
            .map(|r| r.id)
            .collect();
        assert_eq!(ready, vec![58]);
    }

    #[test]
    fn cure_heals_the_partner_in_need_then_itself_then_sora_then_the_other_member() {
        let f = Follower::spawn([0.; 3], 0., 0); // Donald: party slot 1
        // Sora at a quarter of his HP is the partner in need, whoever else
        // is hurt.
        assert_eq!(
            f.heal_target(0.4, &[Some((5, 20)), Some((1, 20)), Some((1, 20))]),
            Some(0)
        );
        // Else Donald himself under 0.4 (8 of 20).
        assert_eq!(
            f.heal_target(0.4, &[Some((20, 20)), Some((7, 20)), Some((20, 20))]),
            Some(1)
        );
        // Goofy at a quarter is the partner in need before Donald's 0.4.
        assert_eq!(
            f.heal_target(0.4, &[Some((20, 20)), Some((7, 20)), Some((5, 20))]),
            Some(2)
        );
        // Nobody under 0.4: no heal.
        assert_eq!(
            f.heal_target(0.4, &[Some((20, 20)), Some((20, 20)), Some((20, 20))]),
            None
        );
        // Sora alone under 0.4 but above a quarter (7 of 20).
        assert_eq!(
            f.heal_target(0.4, &[Some((7, 20)), Some((20, 20)), Some((20, 20))]),
            Some(0)
        );
    }
}
