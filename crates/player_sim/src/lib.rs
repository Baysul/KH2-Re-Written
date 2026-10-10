//! Test-room player controller: a fixed-tick state machine for locomotion,
//! jumping and grounded/aerial attack combos. It owns position, facing and
//! the active action/motion frame; presentation only reads this state.
//!
//! Numeric inputs come from installed records where a public lead exists
//! (PRTY walk/run/jump/turn, SSTM fall max, PTYA combo motions, ANB range
//! triggers). Gravity, step height, body size, walk threshold and the
//! collision response are explicit test-room choices, not observed KH2 rules.

pub mod camera;
pub mod combat;
pub mod shadow;

pub type Vec3 = [f32; 3];

/// One update is one 60 Hz global frame, matching the public ANB note that
/// gameplay motion runs against a 60-per-second global frame rate.
pub const TICKS_PER_SECOND: u32 = 60;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Params {
    /// Units per tick.
    pub walk_speed: f32,
    pub run_speed: f32,
    /// Apex height in units.
    pub jump_height: f32,
    /// Radians per tick.
    pub turn_speed: f32,
    /// Maximum downward speed, units per tick.
    pub fall_max: f32,
    /// Test-room choice: downward acceleration, units per tick squared.
    pub gravity: f32,
    /// Test-room choice: highest ledge climbed without jumping.
    pub step_height: f32,
    /// Test-room choice: stick magnitude below which Sora walks.
    pub walk_threshold: f32,
    /// Test-room choice: body radius/height for wall contact.
    pub radius: f32,
    pub height: f32,
    /// Test-room choice: ticks without attacking before the weapon is put
    /// away and field locomotion resumes.
    pub disarm_ticks: u32,
}

impl Params {
    pub fn validate(&self) -> Result<(), &'static str> {
        let positive = [
            self.walk_speed,
            self.run_speed,
            self.jump_height,
            self.turn_speed,
            self.fall_max,
            self.gravity,
            self.radius,
            self.height,
        ];
        if positive.iter().any(|v| !v.is_finite() || *v <= 0.)
            || !self.step_height.is_finite()
            || self.step_height < 0.
            || !(0. ..1.).contains(&self.walk_threshold)
        {
            return Err("player parameters must be finite and positive");
        }
        Ok(())
    }
    /// Launch speed whose discrete apex (speed reduced by gravity before
    /// each position step) reaches `jump_height`.
    pub fn jump_speed(&self) -> f32 {
        launch_speed(self.gravity, self.jump_height)
    }
}

/// Motion timing in ticks relative to the motion's FrameStart, with the ANB
/// range triggers the controller honours. `end` is FrameEnd and
/// `loop_start` FrameReturn; a return at or past the end holds the last frame.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Clip {
    pub end: u32,
    pub loop_start: u32,
    /// Range trigger 11, "Allow combo".
    pub combo_windows: Vec<(u32, u32)>,
    /// Earliest range trigger 2, "Blend to idle".
    pub blend_to_idle: Option<u32>,
    /// Range trigger 3, "No gravity".
    pub no_gravity: Vec<(u32, u32)>,
}

impl Clip {
    pub fn looping(end: u32, loop_start: u32) -> Self {
        Self {
            end,
            loop_start,
            ..Self::default()
        }
    }
    fn within(ranges: &[(u32, u32)], frame: u32) -> bool {
        ranges.iter().any(|&(a, b)| (a..=b).contains(&frame))
    }
    /// Frame shown for a running counter: plays to FrameEnd, then loops
    /// from FrameReturn (or holds FrameEnd when there is no loop span).
    pub fn looped_frame(&self, frame: u32) -> u32 {
        if frame <= self.end || self.loop_start >= self.end {
            frame.min(self.end)
        } else {
            let span = self.end - self.loop_start;
            self.loop_start + (frame - self.loop_start) % span
        }
    }
    pub fn held_frame(&self, frame: u32) -> u32 {
        frame.min(self.end)
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct AttackStep {
    pub clip: Clip,
    /// PTYA "Jump" field, applied as a lift height when an aerial step starts
    /// (public label only; the installed use is unobserved).
    pub hop: f32,
    /// Root-position translation per tick in model space (+Z is facing).
    /// Its per-tick change moves the player; empty means no root motion.
    pub root: Vec<Vec3>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Clips {
    /// Battle-mode (weapon drawn) locomotion.
    pub idle: Clip,
    pub walk: Clip,
    pub run: Clip,
    /// Field-mode (weapon put away) locomotion.
    pub field_idle: Clip,
    pub field_walk: Clip,
    pub field_run: Clip,
    pub jump: Clip,
    pub fall: Clip,
    pub land: Clip,
    pub ground_combo: Vec<AttackStep>,
    pub air_combo: Vec<AttackStep>,
    /// Grounded damage reactions (front-high, front-low, back-high,
    /// back-low; motion ids 10..13). Empty when not loaded.
    pub hurt: Vec<Clip>,
    /// Guard (motion 173) and its success recoil (motion 42), when loaded.
    pub guard: Option<Clip>,
    pub guard_recoil: Option<Clip>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Action {
    Idle,
    Walk,
    Run,
    Jump,
    Fall,
    Land,
    Attack(usize),
    AirAttack(usize),
    /// A damage reaction (index into `Clips::hurt`).
    Hurt(usize),
    /// Guard (PTYA 0x0b, motion 173): one press, actionable from its
    /// range-2 tick.
    Guard,
    /// Guard success recoil (motion 42).
    GuardRecoil,
}

impl Action {
    pub fn airborne(self) -> bool {
        matches!(self, Self::Jump | Self::Fall | Self::AirAttack(_))
    }
}

/// Camera-relative stick already converted to a world XZ direction, with
/// magnitude 0..=1, plus button edges for this tick.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Input {
    pub direction: [f32; 2],
    pub jump: bool,
    pub attack: bool,
    pub guard: bool,
    /// XZ direction toward the current target, if any (Guard turns to it
    /// at its first tick: frame trigger 21, up to 360°).
    pub target: Option<[f32; 2]>,
}

/// Room collision as used by the controller.
pub trait World {
    /// Highest walkable surface at (x, z) whose height is at most `top`.
    fn floor(&self, x: f32, z: f32, top: f32) -> Option<f32>;
    /// Pushes a body circle out of walls overlapping the vertical span.
    fn resolve_walls(&self, position: [f32; 2], radius: f32, low: f32, high: f32) -> [f32; 2];
}

#[derive(Clone, Debug, PartialEq)]
pub struct Player {
    pub position: Vec3,
    /// Facing angle; facing direction is (sin yaw, cos yaw) in XZ.
    pub yaw: f32,
    pub vertical_speed: f32,
    pub action: Action,
    /// Ticks since the current action started.
    pub frame: u32,
    /// Battle mode: weapon drawn, battle locomotion clips.
    pub armed: bool,
    idle_ticks: u32,
    queued_attack: bool,
    air_combo_spent: bool,
    /// Knock-back slide per tick while hurt.
    slide: [f32; 2],
    /// Knock-slide (FUN_1403d4960/FUN_1403d4570): unit direction and speed.
    knock: Option<([f32; 2], f32)>,
}

/// Knock-back slide brake per tick [test-room choice; SSTM brake 0.8].
pub const HURT_SLIDE_BRAKE: f32 = 0.8;
/// Guard recoil knock strength (DAT_140585efc) and knock-slide decay
/// (DAT_1405aaa40).
pub const GUARD_KNOCK: f32 = 200.;
pub const KNOCK_DECAY: f32 = 0.9;

/// What changed during one tick, for logging and presentation.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Step {
    pub started: Option<Action>,
    pub landed: bool,
}

fn launch_speed(gravity: f32, height: f32) -> f32 {
    (2. * gravity * height).sqrt() + gravity * 0.5
}

fn wrap_angle(a: f32) -> f32 {
    let tau = std::f32::consts::TAU;
    (a + std::f32::consts::PI).rem_euclid(tau) - std::f32::consts::PI
}

impl Player {
    pub fn new(position: Vec3, yaw: f32) -> Self {
        Self {
            position,
            yaw,
            vertical_speed: 0.,
            action: Action::Idle,
            frame: 0,
            armed: false,
            idle_ticks: 0,
            queued_attack: false,
            air_combo_spent: false,
            slide: [0.; 2],
            knock: None,
        }
    }

    /// Starts a grounded damage reaction with a knock-back slide (per tick,
    /// decaying). Airborne actions only take the slide.
    pub fn hurt(&mut self, index: usize, slide: [f32; 2], clips: &Clips) -> Option<Action> {
        self.slide = slide;
        if self.action.airborne() || index >= clips.hurt.len() {
            return None;
        }
        self.action = Action::Hurt(index);
        self.frame = 0;
        self.queued_attack = false;
        self.arm();
        Some(self.action)
    }

    /// Guard success (FUN_1403d61d0, grounded): face the attacker, play the
    /// recoil and knock-slide away at 200 × 0.1 per tick (×0.9 per tick,
    /// stopping below 1). `away` is the XZ unit vector from the attacker.
    pub fn guard_recoil(&mut self, away: [f32; 2], clips: &Clips) -> Option<Action> {
        if self.action.airborne() || clips.guard_recoil.is_none() {
            return None;
        }
        self.yaw = (-away[0]).atan2(-away[1]);
        self.knock = Some((away, GUARD_KNOCK * 0.1));
        self.action = Action::GuardRecoil;
        self.frame = 0;
        self.queued_attack = false;
        self.arm();
        Some(self.action)
    }

    fn start(&mut self, action: Action, step: &mut Step) {
        self.action = action;
        self.frame = 0;
        self.queued_attack = false;
        step.started = Some(action);
    }

    pub fn clip<'a>(&self, clips: &'a Clips) -> &'a Clip {
        match (self.action, self.armed) {
            (Action::Idle, true) => &clips.idle,
            (Action::Walk, true) => &clips.walk,
            (Action::Run, true) => &clips.run,
            (Action::Idle, false) => &clips.field_idle,
            (Action::Walk, false) => &clips.field_walk,
            (Action::Run, false) => &clips.field_run,
            (Action::Jump, _) => &clips.jump,
            (Action::Fall, _) => &clips.fall,
            (Action::Land, _) => &clips.land,
            (Action::Attack(i), _) => &clips.ground_combo[i].clip,
            (Action::AirAttack(i), _) => &clips.air_combo[i].clip,
            (Action::Hurt(i), _) => &clips.hurt[i],
            (Action::Guard, _) => clips.guard.as_ref().unwrap_or(&clips.idle),
            (Action::GuardRecoil, _) => clips.guard_recoil.as_ref().unwrap_or(&clips.idle),
        }
    }

    /// Root-motion translation for the current attack frame, model space.
    pub fn root_offset(&self, clips: &Clips) -> Vec3 {
        let root = match self.action {
            Action::Attack(i) => &clips.ground_combo[i].root,
            Action::AirAttack(i) => &clips.air_combo[i].root,
            _ => return [0.; 3],
        };
        root.get(self.motion_frame(clips) as usize)
            .copied()
            .unwrap_or([0.; 3])
    }

    fn arm(&mut self) {
        self.armed = true;
        self.idle_ticks = 0;
    }

    /// Motion frame to sample for the current action.
    pub fn motion_frame(&self, clips: &Clips) -> u32 {
        let clip = self.clip(clips);
        match self.action {
            Action::Idle | Action::Walk | Action::Run | Action::Jump | Action::Fall => {
                clip.looped_frame(self.frame)
            }
            _ => clip.held_frame(self.frame),
        }
    }

    fn turn_toward(&mut self, direction: [f32; 2], rate: f32) {
        let target = direction[0].atan2(direction[1]);
        let delta = wrap_angle(target - self.yaw);
        self.yaw = wrap_angle(self.yaw + delta.clamp(-rate, rate));
    }

    fn lift(&mut self, height: f32, p: &Params) {
        if height.is_finite() && height > 0. {
            self.vertical_speed = self.vertical_speed.max(launch_speed(p.gravity, height));
        }
    }

    fn facing(&self) -> [f32; 2] {
        [self.yaw.sin(), self.yaw.cos()]
    }

    /// Moves horizontally, keeping the body out of walls and off ledges that
    /// are higher than a step. Returns whether ground remains underfoot.
    fn travel(&mut self, delta: [f32; 2], p: &Params, world: &impl World, grounded: bool) -> bool {
        let [x, y, z] = self.position;
        let wanted = [x + delta[0], z + delta[1]];
        let low = y + p.step_height;
        let moved = world.resolve_walls(wanted, p.radius, low, y + p.height);
        // A floor reachable from this height must exist under the new point.
        match world.floor(moved[0], moved[1], low) {
            Some(floor) => {
                self.position[0] = moved[0];
                self.position[2] = moved[1];
                if grounded {
                    if y - floor <= p.step_height {
                        self.position[1] = floor;
                        true
                    } else {
                        false
                    }
                } else {
                    true
                }
            }
            None => grounded,
        }
    }

    pub fn tick(&mut self, input: &Input, p: &Params, clips: &Clips, world: &impl World) -> Step {
        let mut step = Step::default();
        self.frame = self.frame.saturating_add(1);
        let magnitude = input.direction[0].hypot(input.direction[1]).min(1.);
        let moving = magnitude > 0.1;
        let direction = if moving {
            [
                input.direction[0] / magnitude,
                input.direction[1] / magnitude,
            ]
        } else {
            [0., 0.]
        };
        match self.action {
            Action::Idle | Action::Walk | Action::Run | Action::Land => {
                if input.guard && !input.attack && clips.guard.is_some() {
                    self.start_guard(input, &mut step);
                } else if input.attack && !clips.ground_combo.is_empty() {
                    if moving {
                        self.yaw = direction[0].atan2(direction[1]);
                    }
                    self.arm();
                    self.start(Action::Attack(0), &mut step);
                } else if input.jump {
                    self.vertical_speed = p.jump_speed();
                    self.air_combo_spent = false;
                    self.start(Action::Jump, &mut step);
                } else {
                    // Landing holds until its "Blend to idle" range starts.
                    let landing = self.action == Action::Land
                        && self.frame < clips.land.blend_to_idle.unwrap_or(clips.land.end);
                    let target = if !moving {
                        if landing { Action::Land } else { Action::Idle }
                    } else {
                        // Hysteresis keeps an analog stick near the threshold
                        // from flipping between walk and run every tick.
                        let band = if self.action == Action::Run {
                            -0.05
                        } else {
                            0.05
                        };
                        if magnitude < p.walk_threshold + band {
                            Action::Walk
                        } else {
                            Action::Run
                        }
                    };
                    self.idle_ticks = self.idle_ticks.saturating_add(1);
                    if self.armed && self.idle_ticks >= p.disarm_ticks && target != Action::Land {
                        // Weapon put away: restart the locomotion clip in field mode.
                        self.armed = false;
                        self.start(target, &mut step);
                    } else if target != self.action {
                        self.start(target, &mut step);
                    }
                    if moving {
                        self.turn_toward(direction, p.turn_speed);
                        let speed = if target == Action::Walk {
                            p.walk_speed
                        } else {
                            p.run_speed
                        };
                        let facing = self.facing();
                        if !self.travel([facing[0] * speed, facing[1] * speed], p, world, true) {
                            self.vertical_speed = 0.;
                            self.air_combo_spent = false;
                            self.start(Action::Fall, &mut step);
                        }
                    }
                }
            }
            Action::Attack(i) => {
                let clip = &clips.ground_combo[i].clip;
                if input.attack {
                    self.queued_attack = true;
                }
                // Root motion: the attack's root-curve change this tick.
                let root = &clips.ground_combo[i].root;
                let at = |f: u32| root.get(clip.held_frame(f) as usize).copied();
                if let (Some(now), Some(before)) = (at(self.frame), at(self.frame - 1)) {
                    let (s, c) = self.yaw.sin_cos();
                    let (dx, dz) = (now[0] - before[0], now[2] - before[2]);
                    let delta = [dx * c + dz * s, -dx * s + dz * c];
                    if (delta[0] != 0. || delta[1] != 0.) && !self.travel(delta, p, world, true) {
                        self.vertical_speed = 0.;
                        self.start(Action::Fall, &mut step);
                        return step;
                    }
                }
                let can_chain = Clip::within(&clip.combo_windows, self.frame);
                let recovery = self.frame >= clip.blend_to_idle.unwrap_or(clip.end);
                if self.queued_attack && can_chain && i + 1 < clips.ground_combo.len() {
                    if moving {
                        self.yaw = direction[0].atan2(direction[1]);
                    }
                    self.start(Action::Attack(i + 1), &mut step);
                } else if recovery && input.jump {
                    self.vertical_speed = p.jump_speed();
                    self.air_combo_spent = false;
                    self.start(Action::Jump, &mut step);
                } else if (recovery && moving) || self.frame >= clip.end {
                    self.start(Action::Idle, &mut step);
                }
            }
            Action::Jump | Action::Fall | Action::AirAttack(_) => {
                self.airborne(
                    input, moving, magnitude, direction, p, clips, world, &mut step,
                );
            }
            Action::Hurt(i) => {
                if self.frame >= clips.hurt[i].end {
                    self.start(Action::Idle, &mut step);
                }
            }
            Action::Guard | Action::GuardRecoil => {
                // Locked until the clip's range-2 tick, then input is taken
                // again (FUN_1401b15f0 actionable bits).
                let clip = self.clip(clips);
                let (end, ready) = (clip.end, clip.blend_to_idle.unwrap_or(clip.end));
                if self.frame >= ready {
                    if input.attack && !clips.ground_combo.is_empty() {
                        if moving {
                            self.yaw = direction[0].atan2(direction[1]);
                        }
                        self.start(Action::Attack(0), &mut step);
                    } else if input.guard && clips.guard.is_some() {
                        self.start_guard(input, &mut step);
                    } else if input.jump {
                        self.vertical_speed = p.jump_speed();
                        self.air_combo_spent = false;
                        self.start(Action::Jump, &mut step);
                    } else if moving || self.frame >= end {
                        self.start(Action::Idle, &mut step);
                    }
                }
            }
        }
        if let Some((dir, v)) = self.knock {
            if v < 1. {
                self.knock = None;
            } else {
                let grounded = !self.action.airborne();
                self.travel([dir[0] * v, dir[1] * v], p, world, grounded);
                self.knock = Some((dir, v * KNOCK_DECAY));
            }
        }
        if self.slide[0].hypot(self.slide[1]) > 1e-3 {
            let grounded = !self.action.airborne();
            let slide = self.slide;
            self.travel(slide, p, world, grounded);
            self.slide = slide.map(|v| v * HURT_SLIDE_BRAKE);
        }
        step
    }

    /// PTYA 0x0b: the guard motion, turning to the target at once.
    fn start_guard(&mut self, input: &Input, step: &mut Step) {
        if let Some(t) = input.target
            && t[0].hypot(t[1]) > 1e-4
        {
            self.yaw = t[0].atan2(t[1]);
        }
        self.arm();
        self.start(Action::Guard, step);
    }

    #[allow(clippy::too_many_arguments)]
    fn airborne(
        &mut self,
        input: &Input,
        moving: bool,
        magnitude: f32,
        direction: [f32; 2],
        p: &Params,
        clips: &Clips,
        world: &impl World,
        step: &mut Step,
    ) {
        if let Action::AirAttack(i) = self.action {
            let clip = &clips.air_combo[i].clip;
            if input.attack {
                self.queued_attack = true;
            }
            if self.queued_attack
                && Clip::within(&clip.combo_windows, self.frame)
                && i + 1 < clips.air_combo.len()
            {
                if moving {
                    self.yaw = direction[0].atan2(direction[1]);
                }
                self.start(Action::AirAttack(i + 1), step);
                self.lift(clips.air_combo[i + 1].hop, p);
            } else if self.frame >= clip.end {
                let [x, y, z] = self.position;
                // A swing that touched down finishes on the ground.
                if world
                    .floor(x, z, y + 1e-3)
                    .is_some_and(|f| (y - f).abs() < 1e-3)
                {
                    self.start(Action::Land, step);
                    step.landed = true;
                    return;
                }
                self.start(Action::Fall, step);
            }
        } else if input.attack && !self.air_combo_spent && !clips.air_combo.is_empty() {
            self.air_combo_spent = true;
            if moving {
                self.yaw = direction[0].atan2(direction[1]);
            }
            self.arm();
            self.start(Action::AirAttack(0), step);
            self.lift(clips.air_combo[0].hop, p);
        } else if self.action == Action::Jump && self.vertical_speed <= 0. {
            self.start(Action::Fall, step);
        }
        let floating = match self.action {
            Action::AirAttack(i) => Clip::within(&clips.air_combo[i].clip.no_gravity, self.frame),
            _ => false,
        };
        if floating {
            self.vertical_speed = 0.;
        } else {
            self.vertical_speed = (self.vertical_speed - p.gravity).max(-p.fall_max);
        }
        if moving && !matches!(self.action, Action::AirAttack(_)) {
            self.turn_toward(direction, p.turn_speed);
            let speed = p.run_speed * magnitude.max(p.walk_threshold);
            let facing = self.facing();
            self.travel([facing[0] * speed, facing[1] * speed], p, world, false);
        }
        let [x, y, z] = self.position;
        let next = y + self.vertical_speed;
        if self.vertical_speed <= 0.
            && let Some(floor) = world.floor(x, z, y + p.step_height)
            && next <= floor
        {
            self.position[1] = floor;
            self.vertical_speed = 0.;
            if matches!(self.action, Action::AirAttack(_)) {
                // Test-room choice: let the aerial swing play out on the ground.
                return;
            }
            step.landed = true;
            self.start(Action::Land, step);
            return;
        }
        self.position[1] = next;
    }
}

/// Room collision triangles split into walkable floors and walls.
pub struct TriangleWorld {
    floors: Vec<[Vec3; 3]>,
    walls: Vec<[Vec3; 3]>,
}

impl TriangleWorld {
    /// Triangles whose upward normal component is at least `min_floor_y`
    /// (after normalizing) are floors when `stand` is set; steeper ones are
    /// walls when `collide` is set. Winding is ignored because the selected
    /// collision data is not oriented here.
    pub fn new(
        triangles: impl IntoIterator<Item = ([Vec3; 3], bool, bool)>,
        min_floor_y: f32,
    ) -> Self {
        let mut floors = Vec::new();
        let mut walls = Vec::new();
        for (t, stand, collide) in triangles {
            let u = sub(t[1], t[0]);
            let v = sub(t[2], t[0]);
            let n = cross(u, v);
            let length = (n[0] * n[0] + n[1] * n[1] + n[2] * n[2]).sqrt();
            if length.is_nan() || length <= 1e-6 || t.iter().flatten().any(|c| !c.is_finite()) {
                continue;
            }
            if (n[1] / length).abs() >= min_floor_y {
                if stand {
                    floors.push(t);
                }
            } else if collide {
                walls.push(t);
            }
        }
        Self { floors, walls }
    }
    pub fn counts(&self) -> (usize, usize) {
        (self.floors.len(), self.walls.len())
    }
}

fn sub(a: Vec3, b: Vec3) -> Vec3 {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}
fn cross(a: Vec3, b: Vec3) -> Vec3 {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}

/// Height of triangle `t` at (x, z), if the point lies inside its XZ shadow.
fn height_at(t: &[Vec3; 3], x: f32, z: f32) -> Option<f32> {
    let [a, b, c] = t;
    let d = (b[2] - c[2]) * (a[0] - c[0]) + (c[0] - b[0]) * (a[2] - c[2]);
    if d.abs() < 1e-9 {
        return None;
    }
    let w0 = ((b[2] - c[2]) * (x - c[0]) + (c[0] - b[0]) * (z - c[2])) / d;
    let w1 = ((c[2] - a[2]) * (x - c[0]) + (a[0] - c[0]) * (z - c[2])) / d;
    let w2 = 1. - w0 - w1;
    let e = -1e-4;
    (w0 >= e && w1 >= e && w2 >= e).then(|| w0 * a[1] + w1 * b[1] + w2 * c[1])
}

fn closest_on_segment(p: [f32; 2], a: [f32; 2], b: [f32; 2]) -> [f32; 2] {
    let ab = [b[0] - a[0], b[1] - a[1]];
    let len = ab[0] * ab[0] + ab[1] * ab[1];
    let t = if len > 0. {
        (((p[0] - a[0]) * ab[0] + (p[1] - a[1]) * ab[1]) / len).clamp(0., 1.)
    } else {
        0.
    };
    [a[0] + ab[0] * t, a[1] + ab[1] * t]
}

impl World for TriangleWorld {
    fn floor(&self, x: f32, z: f32, top: f32) -> Option<f32> {
        self.floors
            .iter()
            .filter_map(|t| height_at(t, x, z))
            .filter(|&h| h <= top + 1e-3)
            .reduce(f32::max)
    }

    fn resolve_walls(&self, position: [f32; 2], radius: f32, low: f32, high: f32) -> [f32; 2] {
        let mut p = position;
        // A few relaxation passes let the body slide along corners.
        for _ in 0..4 {
            let mut pushed = false;
            for t in &self.walls {
                let (lo, hi) = t
                    .iter()
                    .fold((f32::MAX, f32::MIN), |(l, h), v| (l.min(v[1]), h.max(v[1])));
                if hi <= low || lo >= high {
                    continue;
                }
                for (a, b) in [(t[0], t[1]), (t[1], t[2]), (t[2], t[0])] {
                    let q = closest_on_segment(p, [a[0], a[2]], [b[0], b[2]]);
                    let d = [p[0] - q[0], p[1] - q[1]];
                    let distance = d[0].hypot(d[1]);
                    if distance < radius && distance > 1e-5 {
                        let push = (radius - distance) / distance;
                        p = [p[0] + d[0] * push, p[1] + d[1] * push];
                        pushed = true;
                    }
                }
            }
            if !pushed {
                break;
            }
        }
        p
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn params() -> Params {
        Params {
            walk_speed: 2.,
            run_speed: 8.,
            jump_height: 185.,
            turn_speed: 0.2618,
            fall_max: 16.,
            gravity: 1.,
            step_height: 20.,
            walk_threshold: 0.5,
            radius: 30.,
            height: 140.,
            disarm_ticks: 120,
        }
    }
    fn attack(end: u32, window: (u32, u32), hop: f32) -> AttackStep {
        AttackStep {
            clip: Clip {
                end,
                loop_start: 0,
                combo_windows: vec![window],
                blend_to_idle: Some(end - 10),
                no_gravity: vec![],
            },
            hop,
            root: Vec::new(),
        }
    }
    fn clips() -> Clips {
        Clips {
            idle: Clip::looping(100, 0),
            walk: Clip::looping(60, 0),
            run: Clip::looping(40, 0),
            field_idle: Clip::looping(90, 0),
            field_walk: Clip::looping(50, 0),
            field_run: Clip::looping(36, 0),
            jump: Clip::looping(30, 0),
            fall: Clip::looping(30, 0),
            land: Clip::looping(12, 0),
            ground_combo: vec![
                attack(40, (10, 25), 0.),
                attack(40, (10, 25), 0.),
                attack(50, (60, 60), 0.),
            ],
            air_combo: vec![attack(30, (8, 20), 20.), attack(30, (8, 20), 20.)],
            hurt: vec![
                Clip {
                    end: 24,
                    ..Clip::default()
                };
                4
            ],
            guard: Some(Clip {
                end: 60,
                blend_to_idle: Some(52),
                ..Clip::default()
            }),
            guard_recoil: Some(Clip {
                end: 80,
                blend_to_idle: Some(40),
                ..Clip::default()
            }),
        }
    }
    /// A flat 2000-unit square floor with one wall along x = 500.
    fn room() -> TriangleWorld {
        let f = [
            [-1000., 0., -1000.],
            [1000., 0., -1000.],
            [1000., 0., 1000.],
            [-1000., 0., 1000.],
        ];
        let w = [
            [500., 0., -1000.],
            [500., 300., -1000.],
            [500., 300., 1000.],
            [500., 0., 1000.],
        ];
        TriangleWorld::new(
            [
                [f[0], f[1], f[2]],
                [f[0], f[2], f[3]],
                [w[0], w[1], w[2]],
                [w[0], w[2], w[3]],
            ]
            .map(|t| (t, true, true)),
            0.5,
        )
    }
    fn hold(dir: [f32; 2]) -> Input {
        Input {
            direction: dir,
            ..Input::default()
        }
    }

    #[test]
    fn splits_floors_from_walls() {
        assert_eq!(room().counts(), (2, 2));
        assert_eq!(room().floor(0., 0., 10.), Some(0.));
        assert_eq!(room().floor(0., 0., -10.), None);
        assert_eq!(room().floor(5000., 0., 10.), None);
    }

    #[test]
    fn walks_and_runs_by_stick_magnitude_and_turns_at_rate_limit() {
        let (p, c, w) = (params(), clips(), room());
        let mut player = Player::new([0., 0., 0.], 0.);
        player.tick(&hold([0., 0.3]), &p, &c, &w);
        assert_eq!(player.action, Action::Walk);
        assert!((player.position[2] - 2.).abs() < 1e-5);
        player.tick(&hold([0., 1.]), &p, &c, &w);
        assert_eq!(player.action, Action::Run);
        assert!((player.position[2] - 10.).abs() < 1e-5);
        // Facing +X requires a quarter turn: limited per tick.
        player.tick(&hold([1., 0.]), &p, &c, &w);
        assert!((player.yaw - 0.2618).abs() < 1e-5);
        player.tick(&Input::default(), &p, &c, &w);
        assert_eq!(player.action, Action::Idle);
    }

    #[test]
    fn walls_stop_the_body_radius_away() {
        let (p, c, w) = (params(), clips(), room());
        let mut player = Player::new([400., 0., 0.], std::f32::consts::FRAC_PI_2);
        for _ in 0..60 {
            player.tick(&hold([1., 0.]), &p, &c, &w);
        }
        assert!((player.position[0] - (500. - p.radius)).abs() < 1e-3);
    }

    #[test]
    fn jump_reaches_parameter_height_then_lands() {
        let (p, c, w) = (params(), clips(), room());
        let mut player = Player::new([0., 0., 0.], 0.);
        let jump = Input {
            jump: true,
            ..Input::default()
        };
        assert_eq!(player.tick(&jump, &p, &c, &w).started, Some(Action::Jump));
        let mut apex: f32 = 0.;
        let mut landed = None;
        for t in 0..200 {
            let step = player.tick(&Input::default(), &p, &c, &w);
            apex = apex.max(player.position[1]);
            if step.landed {
                landed = Some(t);
                break;
            }
        }
        // Discrete integration lands slightly under the continuous apex.
        assert!(apex > 184. && apex <= 185., "apex {apex}");
        assert!(landed.is_some());
        assert_eq!(player.action, Action::Land);
        assert_eq!(player.position[1], 0.);
        for _ in 0..12 {
            player.tick(&Input::default(), &p, &c, &w);
        }
        assert_eq!(player.action, Action::Idle);
    }

    #[test]
    fn combo_chains_only_inside_allow_combo_windows() {
        let (p, c, w) = (params(), clips(), room());
        let mut player = Player::new([0., 0., 0.], 0.);
        let press = Input {
            attack: true,
            ..Input::default()
        };
        player.tick(&press, &p, &c, &w);
        assert_eq!(player.action, Action::Attack(0));
        // Pressing early queues; chaining waits for the window at frame 10.
        player.tick(&press, &p, &c, &w);
        for _ in 0..7 {
            player.tick(&Input::default(), &p, &c, &w);
        }
        player.tick(&Input::default(), &p, &c, &w);
        assert_eq!(player.action, Action::Attack(0));
        player.tick(&Input::default(), &p, &c, &w);
        assert_eq!(player.action, Action::Attack(1));
        // Without another press the step finishes and returns to idle.
        for _ in 0..40 {
            player.tick(&Input::default(), &p, &c, &w);
        }
        assert_eq!(player.action, Action::Idle);
    }

    #[test]
    fn recovery_frames_allow_movement_cancel() {
        let (p, c, w) = (params(), clips(), room());
        let mut player = Player::new([0., 0., 0.], 0.);
        player.tick(
            &Input {
                attack: true,
                ..Input::default()
            },
            &p,
            &c,
            &w,
        );
        for _ in 0..20 {
            player.tick(&hold([0., 1.]), &p, &c, &w);
        }
        assert_eq!(player.action, Action::Attack(0));
        assert_eq!(player.position, [0., 0., 0.]);
        for _ in 0..15 {
            player.tick(&hold([0., 1.]), &p, &c, &w);
        }
        assert!(matches!(player.action, Action::Run | Action::Idle));
    }

    #[test]
    fn one_air_combo_per_jump_with_hop() {
        let (p, c, w) = (params(), clips(), room());
        let mut player = Player::new([0., 0., 0.], 0.);
        let jump = Input {
            jump: true,
            ..Input::default()
        };
        let press = Input {
            attack: true,
            ..Input::default()
        };
        player.tick(&jump, &p, &c, &w);
        for _ in 0..25 {
            player.tick(&Input::default(), &p, &c, &w);
        }
        player.tick(&press, &p, &c, &w);
        assert_eq!(player.action, Action::AirAttack(0));
        assert!(player.vertical_speed > 0.);
        for _ in 0..40 {
            player.tick(&Input::default(), &p, &c, &w);
        }
        assert!(!matches!(player.action, Action::AirAttack(_)));
        if player.action == Action::Fall {
            player.tick(&press, &p, &c, &w);
            assert_eq!(player.action, Action::Fall);
        }
    }

    #[test]
    fn walking_off_a_ledge_falls() {
        let (p, c, _) = (params(), clips(), ());
        let high = [
            [-100., 100., -100.],
            [100., 100., -100.],
            [100., 100., 100.],
            [-100., 100., 100.],
        ];
        let low = [
            [-1000., 0., -1000.],
            [1000., 0., -1000.],
            [1000., 0., 1000.],
            [-1000., 0., 1000.],
        ];
        let w = TriangleWorld::new(
            [
                [high[0], high[1], high[2]],
                [high[0], high[2], high[3]],
                [low[0], low[1], low[2]],
                [low[0], low[2], low[3]],
            ]
            .map(|t| (t, true, true)),
            0.5,
        );
        let mut player = Player::new([0., 100., 0.], 0.);
        let mut fell = false;
        for _ in 0..80 {
            player.tick(&hold([0., 1.]), &p, &c, &w);
            fell |= player.action == Action::Fall;
        }
        assert!(fell);
        assert_eq!(player.position[1], 0.);
    }

    #[test]
    fn surfaces_without_flags_are_ignored() {
        let f = [[-10., 0., -10.], [10., 0., -10.], [10., 0., 10.]];
        let w = [[0., 0., -10.], [0., 99., -10.], [0., 99., 10.]];
        let world = TriangleWorld::new([(f, false, true), (w, true, false)], 0.5);
        assert_eq!(world.counts(), (0, 0));
    }

    #[test]
    fn attacking_draws_the_weapon_until_idle_timeout() {
        let (p, c, w) = (params(), clips(), room());
        let mut player = Player::new([0., 0., 0.], 0.);
        assert!(!player.armed);
        assert_eq!(player.clip(&c).end, 90);
        player.tick(
            &Input {
                attack: true,
                ..Input::default()
            },
            &p,
            &c,
            &w,
        );
        assert!(player.armed);
        for _ in 0..60 {
            player.tick(&Input::default(), &p, &c, &w);
        }
        assert_eq!(player.action, Action::Idle);
        assert!(player.armed);
        assert_eq!(player.clip(&c).end, 100);
        for _ in 0..p.disarm_ticks {
            player.tick(&Input::default(), &p, &c, &w);
        }
        assert!(!player.armed);
        assert_eq!(player.clip(&c).end, 90);
    }

    #[test]
    fn attack_root_motion_moves_along_facing() {
        let (p, mut c, w) = (params(), clips(), room());
        c.ground_combo[0].root = (0..40).map(|f| [0., 0., f as f32 * 2.]).collect();
        // Facing +X: model +Z root motion moves the player along +X.
        let mut player = Player::new([0., 0., 0.], std::f32::consts::FRAC_PI_2);
        player.tick(
            &Input {
                attack: true,
                ..Input::default()
            },
            &p,
            &c,
            &w,
        );
        for _ in 0..10 {
            player.tick(&Input::default(), &p, &c, &w);
        }
        assert!(
            (player.position[0] - 20.).abs() < 1e-3,
            "{:?}",
            player.position
        );
        assert!(player.position[2].abs() < 1e-3);
    }

    #[test]
    fn rejects_invalid_parameters() {
        let mut p = params();
        assert!(p.validate().is_ok());
        p.run_speed = f32::NAN;
        assert!(p.validate().is_err());
    }

    #[test]
    fn guard_turns_to_target_locks_and_recoils_with_a_knock_slide() {
        let (p, c, world) = (params(), clips(), room());
        let mut player = Player::new([0., 0., 0.], 0.);
        let press = Input {
            guard: true,
            target: Some([1., 0.]),
            ..Input::default()
        };
        let step = player.tick(&press, &p, &c, &world);
        assert_eq!(step.started, Some(Action::Guard));
        assert!((player.yaw - std::f32::consts::FRAC_PI_2).abs() < 1e-5);
        // Locked: moving does nothing until tick 52.
        let stick = Input {
            direction: [0., 1.],
            ..Input::default()
        };
        for _ in 1..52 {
            player.tick(&stick, &p, &c, &world);
        }
        assert_eq!(player.action, Action::Guard);
        player.tick(&stick, &p, &c, &world);
        assert_eq!(player.action, Action::Idle);
        // Recoil: face the attacker and slide away 20, ×0.9, until below 1.
        assert_eq!(
            player.guard_recoil([0., -1.], &c),
            Some(Action::GuardRecoil)
        );
        assert!(player.yaw.abs() < 1e-5);
        let start = player.position[2];
        for _ in 0..60 {
            player.tick(&Input::default(), &p, &c, &world);
        }
        let moved = start - player.position[2];
        let expected: f32 = (0..29).map(|k| 20. * 0.9f32.powi(k)).sum();
        assert!((moved - expected).abs() < 1e-2, "{moved} vs {expected}");
    }

    #[test]
    fn hurt_plays_its_clip_slides_back_and_returns_to_idle() {
        let c = clips();
        let world = room();
        let mut player = Player::new([0.; 3], 0.);
        assert_eq!(player.hurt(1, [0., -10.], &c), Some(Action::Hurt(1)));
        assert!(player.armed);
        let p = params();
        player.tick(&Input::default(), &p, &c, &world);
        assert!((player.position[2] + 10.).abs() < 1e-3);
        for _ in 0..30 {
            player.tick(&Input::default(), &p, &c, &world);
        }
        assert_eq!(player.action, Action::Idle);
    }
}
