//! Party members (Donald, Goofy) as the executable's `kn::Friend` objects:
//! the follow behaviour of FUN_1401b41a0 / FUN_1401b4820 and the slot
//! formation, on the shared movement rules. The decision AI (attacks,
//! spells, items) builds on this state.
//!
//! The thresholds are the personality resource's header floats read by
//! FUN_1401cc890 / FUN_1401cc8c0 (200 / 240 up, 160 / 200 down); the
//! formation distance is not decoded (`DAT_14071d760` is filled at run
//! time) and is a test-room value.

use crate::{Body, Vec3, World};

/// Walk and run speeds per tick (PRTY words 0 and 1: 2.0 and 8.0).
pub const WALK_SPEED: f32 = 2.;
pub const RUN_SPEED: f32 = 8.;
/// Distances at which the gait steps up (from stand, from walk) and down
/// (to walk, to stand) [I from the personality header].
const UP: [f32; 2] = [200., 240.];
const DOWN: [f32; 2] = [200., 160.];
/// Turn rate per tick (`FUN_14040a440` default π/16).
const TURN: f32 = std::f32::consts::PI / 16.;
/// Slot yaw offset (the exe uses 0.349 × 2i − 0.349; widened here so the
/// two bodies do not overlap, as the formation table is not decoded) and the formation
/// distance behind the leader [test-room value].
const SLOT_YAW: f32 = 0.6;
const FORMATION: f32 = 150.;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Gait {
    Stand,
    Walk,
    Run,
}

/// A party member that follows the leader.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Follower {
    pub position: Vec3,
    pub yaw: f32,
    pub gait: Gait,
    /// Party slot 0 or 1 (obj+0xdd0).
    pub slot: u8,
    /// Ticks in the current gait's motion.
    pub tick: u32,
}

fn wrap(a: f32) -> f32 {
    let tau = std::f32::consts::TAU;
    (a + std::f32::consts::PI).rem_euclid(tau) - std::f32::consts::PI
}

impl Follower {
    /// FUN_1403c2fc0: at the leader's position, 100 below it before the
    /// floor snap, facing the leader's direction away.
    pub fn spawn(leader: Vec3, leader_yaw: f32, slot: u8) -> Self {
        Follower {
            position: leader,
            yaw: leader_yaw,
            gait: Gait::Stand,
            slot,
            tick: 0,
        }
    }

    /// The motion id of the gait: idle 0, walk 1, run 2.
    pub fn motion(&self) -> u16 {
        match self.gait {
            Gait::Stand => 0,
            Gait::Walk => 1,
            Gait::Run => 2,
        }
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

    /// One tick: choose the gait by the distance to the goal, turn toward
    /// it and move at the gait's speed through the world.
    pub fn tick(&mut self, leader: Vec3, leader_yaw: f32, world: &impl World, body: &Body) {
        let goal = self.goal(leader, leader_yaw);
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
            let moved = world.collide(self.position, disp, false, body);
            self.position = moved.position;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Collided, World};

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

    #[test]
    fn a_distant_member_walks_then_runs_and_settles_at_the_goal() {
        let mut f = Follower::spawn([0., 0., 0.], 0., 0);
        let leader = [0., 0., 600.];
        let mut gaits = Vec::new();
        for _ in 0..400 {
            f.tick(leader, 0., &Flat, &BODY);
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
}
