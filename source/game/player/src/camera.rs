//! Field follow camera (default mode), reimplemented from observations of the
//! installed PC executable's camera update: the per-mode table (field of
//! view, radius, lock radius, radius limits, height curve), the yaw follow
//! that swings behind a moving player, the radius return, and the spring
//! that eases the current eye and look-at points toward their targets.
//!
//! Positions here are in the game's own world space: Y points down, so
//! "above the player" is negative Y. One `tick` is one 60 Hz frame (the
//! game's frame-time scale of 1). Camera collision uses the map's camera
//! collision mesh: the eye is pulled to the first hit on the segment from the
//! look-at point, 5 units off the surface.

pub type Vec3 = [f32; 3];

/// One row of the camera mode table.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Mode {
    /// Horizontal field of view in radians across a 640-unit-wide screen.
    pub fov: f32,
    pub radius: f32,
    pub lock_radius: f32,
    pub radius_min: f32,
    pub radius_max: f32,
    /// Eye height grows with the square of (radius - radius_min) times this.
    pub up_curve: f32,
}

/// Mode 0 (default/outdoor) row as stored in the executable's table.
pub const OUTDOOR: Mode = Mode {
    fov: 1.5,
    radius: 420.,
    lock_radius: 380.,
    radius_min: 250.,
    radius_max: 500.,
    up_curve: 0.008,
};

/// Look-at point height above the player's position.
pub const LOOK_AT_HEIGHT: f32 = 170.;
/// Eye target sits this far below the look-at point before the radius curve.
pub const EYE_DROP: f32 = 150.;
/// Yaw easing toward the yaw target, radians per frame.
pub const YAW_EASE: f32 = 0.174_532_9;
/// Behind-the-player follow speed, radians per frame, and its per-unit
/// change with the player's speed parameter (relative to 8).
pub const FOLLOW_SPEED: f32 = 0.004_363_32;
pub const FOLLOW_SPEED_PER_UNIT: f32 = 0.000_174_533;
pub const SPEED_REFERENCE: f32 = 8.;
/// Stick "toward the camera" component at or above which follow pauses.
pub const FOLLOW_STICK_LIMIT: f32 = 0.9;
/// Radius return toward the mode radius, units per frame.
pub const RADIUS_RETURN: f32 = 0.5;
/// Manual rotation, radians per frame at full input (default setting).
pub const MANUAL_TURN: f32 = 0.052_359_9;
/// Manual zoom, units per frame at full input.
pub const MANUAL_ZOOM: f32 = 8.;
/// Spring gains (default camera setting) and their per-unit speed change.
pub const SPRING_GAIN: f32 = 0.07;
pub const SPRING_GAIN_PER_UNIT: f32 = 0.0006;
pub const LOOK_AT_EXTRA_GAIN: f32 = 0.03;
pub const SPRING: f32 = 2.5;
pub const DAMPING: f32 = 0.5;
pub const REST_LENGTH: f32 = 1.;
/// Distance the eye is kept off a camera collision surface.
pub const COLLISION_OFFSET: f32 = 5.;

/// First hit on the segment `from` -> `to`: point and unit surface normal
/// facing `from`.
pub type Hit = Option<(Vec3, Vec3)>;

fn cross(a: Vec3, b: Vec3) -> Vec3 {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}

/// Nearest intersection of a segment with a triangle soup.
pub fn segment_hit(triangles: &[[Vec3; 3]], from: Vec3, to: Vec3) -> Hit {
    let dir = sub(to, from);
    let mut best: Option<(f32, Vec3)> = None;
    for [a, b, c] in triangles {
        let (e1, e2) = (sub(*b, *a), sub(*c, *a));
        let p = cross(dir, e2);
        let det = dot(e1, p);
        if det.abs() < 1e-8 {
            continue;
        }
        let inv = 1. / det;
        let t0 = sub(from, *a);
        let u = dot(t0, p) * inv;
        if !(0. ..=1.).contains(&u) {
            continue;
        }
        let q = cross(t0, e1);
        let v = dot(dir, q) * inv;
        if v < 0. || u + v > 1. {
            continue;
        }
        let t = dot(e2, q) * inv;
        if (0. ..=1.).contains(&t) && best.is_none_or(|(bt, _)| t < bt) {
            best = Some((t, cross(e1, e2)));
        }
    }
    let (t, n) = best?;
    let len = dot(n, n).sqrt();
    let mut n = n.map(|c| c / len);
    if dot(n, dir) > 0. {
        n = n.map(|c| -c);
    }
    Some(([0, 1, 2].map(|k| from[k] + dir[k] * t), n))
}

fn pushed_off((point, normal): (Vec3, Vec3)) -> Vec3 {
    [0, 1, 2].map(|k| point[k] + normal[k] * COLLISION_OFFSET)
}

/// Per-frame inputs. Stick axes follow the pad convention: x right, y
/// toward the player (down on the stick); `magnitude` is the stick's
/// analog strength.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct CameraInput {
    /// Native mode-2 camera anchor, in Y-down world coordinates.
    pub lock_target: Option<Vec3>,
    pub player_position: Vec3,
    /// Player facing as a horizontal direction in world space.
    pub player_forward: Vec3,
    /// The player's horizontal position changed this frame.
    pub moved: bool,
    pub stick: [f32; 2],
    pub magnitude: f32,
    /// Manual rotation (-1..1, positive increases yaw).
    pub turn: f32,
    /// Manual zoom (-1..1, positive moves away).
    pub zoom: f32,
    /// Swing directly behind the player and restore the mode radius.
    pub recenter: bool,
    /// The player's speed parameter (PRTY run speed in this test room).
    pub speed: f32,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FollowCamera {
    pub mode: Mode,
    pub yaw: f32,
    pub yaw_target: f32,
    pub radius: f32,
    pub eye: Vec3,
    pub look_at: Vec3,
    eye_target: Vec3,
    eye_target_prev: Vec3,
    look_at_target: Vec3,
    look_at_target_prev: Vec3,
}

/// The executable's angle wrap into [-pi, pi].
pub fn wrap(a: f32) -> f32 {
    use std::f32::consts::{PI, TAU};
    if a > 0. {
        (a + PI) % TAU - PI
    } else {
        PI - (PI - a) % TAU
    }
}

/// Moves `from` toward `to` by at most `step` along the shorter arc.
fn approach(from: f32, to: f32, step: f32) -> f32 {
    let d = wrap(to - from);
    wrap(from + d.clamp(-step, step))
}

fn sub(a: Vec3, b: Vec3) -> Vec3 {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}

fn dot(a: Vec3, b: Vec3) -> f32 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

/// Eases `current` toward `target`: a spring on the distance with damping
/// against the target's own motion (`target_prev` is last frame's target).
pub fn spring(current: Vec3, target: Vec3, target_prev: Vec3, gain: f32) -> Vec3 {
    let d = sub(current, target);
    let length = dot(d, d).sqrt();
    if length == 0. {
        return current;
    }
    let v = sub(target_prev, target).map(|c| c * gain);
    let f = dot(d, v);
    let scale = ((f / length) * DAMPING + (REST_LENGTH - length) * SPRING) * gain / length;
    let out = [
        current[0] + d[0] * scale,
        current[1] + d[1] * scale,
        current[2] + d[2] * scale,
    ];
    out.map(|c| if c.is_nan() { 0. } else { c })
}

/// Yaw that places the eye behind a player facing `forward`.
pub fn behind(forward: Vec3) -> f32 {
    (-forward[0]).atan2(-forward[2])
}

impl FollowCamera {
    /// A camera snapped behind the player at the mode radius.
    pub fn new(mode: Mode, input: &CameraInput) -> Self {
        let yaw = behind(input.player_forward);
        let mut camera = Self {
            mode,
            yaw,
            yaw_target: yaw,
            radius: mode.radius,
            eye: [0.; 3],
            look_at: [0.; 3],
            eye_target: [0.; 3],
            eye_target_prev: [0.; 3],
            look_at_target: [0.; 3],
            look_at_target_prev: [0.; 3],
        };
        camera.place_targets(input.player_position, true);
        camera
    }

    /// Vertical field of view for a 640x480 screen at the mode's horizontal
    /// field of view.
    pub fn vertical_fov(&self) -> f32 {
        let focal = 320. / (self.mode.fov * 0.5).tan();
        2. * (240. / focal).atan()
    }

    fn place_targets(&mut self, player: Vec3, snap: bool) {
        self.look_at_target = [player[0], player[1] - LOOK_AT_HEIGHT, player[2]];
        self.yaw = if snap {
            self.yaw_target
        } else {
            approach(self.yaw, self.yaw_target, YAW_EASE)
        };
        let at = self.look_at_target;
        let d = self.radius - self.mode.radius_min;
        let (s, c) = self.yaw.sin_cos();
        self.eye_target = [
            at[0] + self.radius * s,
            at[1] + EYE_DROP - d * d * self.mode.up_curve,
            at[2] + self.radius * c,
        ];
        if snap {
            self.eye = self.eye_target;
            self.look_at = self.look_at_target;
            self.eye_target_prev = self.eye_target;
            self.look_at_target_prev = self.look_at_target;
        }
    }

    /// One frame. `collide` returns the first camera-collision hit on a
    /// segment (see `segment_hit`).
    pub fn tick(&mut self, input: &CameraInput, collide: &dyn Fn(Vec3, Vec3) -> Hit) {
        self.look_at_target_prev = self.look_at_target;
        self.eye_target_prev = self.eye_target;
        let mode = self.mode;
        let mut snap = false;
        let locked = input.lock_target.is_some();
        if locked {
            if (self.radius - mode.lock_radius).abs() >= 1. {
                self.radius = if self.radius > mode.lock_radius {
                    (self.radius - 1.).max(mode.radius_min)
                } else {
                    (self.radius + 1.).min(mode.radius_max)
                };
            }
        } else if input.turn != 0. {
            self.yaw = wrap(self.yaw + input.turn * MANUAL_TURN);
            self.yaw_target = self.yaw;
        } else if input.recenter {
            self.radius = mode.radius;
            self.yaw_target = behind(input.player_forward);
            self.eye = self.eye_target;
            snap = input.magnitude != 0.;
        }
        if !locked && input.zoom > 0. {
            self.radius = (self.radius + input.zoom * MANUAL_ZOOM).min(mode.radius_max);
        } else if !locked && input.zoom < 0. {
            self.radius = (self.radius + input.zoom * MANUAL_ZOOM).max(mode.radius_min);
        }
        let speed_offset = input.speed - SPEED_REFERENCE;
        if !locked
            && !snap
            && input.moved
            && input.magnitude != 0.
            && input.turn == 0.
            && input.stick[1] < FOLLOW_STICK_LIMIT
        {
            let step = 2.
                * (input.stick[0].abs() + 1.)
                * (input.stick[1].abs() + 1.)
                * input.magnitude
                * (speed_offset * FOLLOW_SPEED_PER_UNIT + FOLLOW_SPEED);
            self.yaw = approach(self.yaw, behind(input.player_forward), step);
            self.yaw_target = self.yaw;
            if (self.radius - mode.radius).abs() >= 1. {
                self.radius = if self.radius <= mode.radius {
                    (self.radius + RADIUS_RETURN).min(mode.radius_max)
                } else {
                    (self.radius - RADIUS_RETURN).max(mode.radius_min)
                };
            }
        }
        self.place_targets(input.player_position, snap);
        // Native mode 2 preserves the player-anchored eye target. Only its
        // look target interpolates toward the selected actor's camera anchor.
        if let Some(target) = input.lock_target {
            let player = input.player_position;
            let delta = sub(target, [player[0], player[1] - 100., player[2]]);
            let distance = dot(delta, delta).sqrt();
            if !(distance > 300. && delta[1] / distance > 0.5) {
                self.look_at_target = [0, 1, 2]
                    .map(|i| self.look_at_target[i] + 0.3 * (target[i] - self.look_at_target[i]));
            }
        }
        let collision_origin = if locked {
            [
                input.player_position[0],
                input.player_position[1] - LOOK_AT_HEIGHT,
                input.player_position[2],
            ]
        } else {
            self.look_at_target
        };
        if let Some(hit) = collide(collision_origin, self.eye_target) {
            self.eye_target = pushed_off(hit);
        }
        // At 60 Hz the gain is halved; the look-at point adds its extra gain
        // afterwards.
        let gain = (SPRING_GAIN + speed_offset * SPRING_GAIN_PER_UNIT) * 0.5;
        self.look_at = spring(
            self.look_at,
            self.look_at_target,
            self.look_at_target_prev,
            gain + LOOK_AT_EXTRA_GAIN,
        );
        self.eye = spring(self.eye, self.eye_target, self.eye_target_prev, gain);
        if let Some(hit) = collide(self.look_at, self.eye) {
            self.eye = pushed_off(hit);
            self.eye_target = self.eye;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn input(position: Vec3, forward: Vec3) -> CameraInput {
        CameraInput {
            player_position: position,
            player_forward: forward,
            speed: 8.,
            ..Default::default()
        }
    }

    #[test]
    fn lock_looks_between_actors_but_keeps_the_eye_on_the_player_anchor() {
        let mut step = input([0.; 3], [0., 0., 1.]);
        let mut camera = FollowCamera::new(OUTDOOR, &step);
        step.lock_target = Some([100., -50., 200.]);
        camera.tick(&step, &|_, _| None);
        assert_eq!(camera.radius, 419.);
        assert_eq!(
            camera.look_at_target,
            [
                f32::from_bits(0x41f0_0001),
                -134.,
                f32::from_bits(0x4270_0001)
            ]
        );
        assert!(camera.eye_target[0].abs() < 0.001);
        assert!((camera.eye_target[2] + 419.).abs() < 0.001);
        for _ in 0..60 {
            camera.tick(&step, &|_, _| None);
        }
        assert_eq!(camera.radius, 380.);
    }

    #[test]
    fn a_distant_target_below_the_player_preserves_the_player_look_anchor() {
        let mut step = input([0.; 3], [0., 0., 1.]);
        let mut camera = FollowCamera::new(OUTDOOR, &step);
        step.lock_target = Some([0., 600., 100.]);
        camera.tick(&step, &|_, _| None);
        assert_eq!(camera.look_at_target, [0., -170., 0.]);
    }

    #[test]
    fn snapped_camera_sits_behind_and_above() {
        let camera = FollowCamera::new(OUTDOOR, &input([0., 0., 0.], [0., 0., 1.]));
        // Facing +Z puts the eye at -Z, 420 back; Y is down, so above is
        // negative: look-at 170 up, eye 20 + 170^2 * 0.008 up.
        assert!((camera.eye[2] + 420.).abs() < 1e-3);
        assert!(camera.eye[0].abs() < 1e-3);
        assert!((camera.look_at[1] + 170.).abs() < 1e-3);
        assert!((camera.eye[1] + (20. + 170. * 170. * 0.008)).abs() < 1e-2);
        assert!((camera.vertical_fov() - 1.2197).abs() < 1e-3);
    }

    #[test]
    fn wrap_matches_the_executable_branches() {
        use std::f32::consts::PI;
        assert!((wrap(PI + 0.5) - (-PI + 0.5)).abs() < 1e-5);
        assert!((wrap(-PI - 0.5) - (PI - 0.5)).abs() < 1e-5);
        assert_eq!(wrap(0.25), 0.25);
        assert!((approach(3.0, -3.0, 0.1) - 3.1).abs() < 1e-5);
    }

    #[test]
    fn spring_eases_toward_a_still_target() {
        let out = spring([100., 0., 0.], [0., 0., 0.], [0., 0., 0.], 0.035);
        // (1 - 100) * 2.5 * 0.035 = -8.6625 along the offset.
        assert!((out[0] - (100. - 8.6625)).abs() < 1e-3);
        assert_eq!(
            spring([1., 2., 3.], [1., 2., 3.], [0.; 3], 0.5),
            [1., 2., 3.]
        );
    }

    #[test]
    fn wall_between_look_at_and_eye_pulls_the_eye_in() {
        // A wall at z = -100 across the path behind a player facing +Z.
        let wall = [[
            [-1000., -1000., -100.],
            [1000., -1000., -100.],
            [0., 1000., -100.],
        ]];
        let mut camera = FollowCamera::new(OUTDOOR, &input([0., 0., 0.], [0., 0., 1.]));
        camera.tick(&input([0., 0., 0.], [0., 0., 1.]), &|a, b| {
            segment_hit(&wall, a, b)
        });
        assert!((camera.eye[2] - (-95.)).abs() < 1e-3);
        assert!(segment_hit(&wall, [0., 0., 0.], [0., 0., -50.]).is_none());
    }

    #[test]
    fn follow_swings_behind_a_moving_player_and_pauses_when_running_at_it() {
        let mut camera = FollowCamera::new(OUTDOOR, &input([0., 0., 0.], [0., 0., 1.]));
        let mut step = input([0., 0., 0.], [1., 0., 0.]);
        step.moved = true;
        step.magnitude = 1.;
        camera.tick(&step, &|_, _| None);
        // Behind +Z is -pi; behind +X is -pi/2. Full stick, no tilt turns
        // 2 * 1 * 1 * 1 * FOLLOW_SPEED per frame toward it.
        let expected = -std::f32::consts::PI + 2. * FOLLOW_SPEED;
        assert!((camera.yaw - expected).abs() < 1e-5);
        let before = camera.yaw;
        step.stick = [0., 1.];
        camera.tick(&step, &|_, _| None);
        assert_eq!(camera.yaw, before);
    }
}
