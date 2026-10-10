//! Test-room player controller: a fixed-tick state machine for locomotion,
//! jumping and grounded/aerial attack combos. It owns position, facing and
//! the active action/motion frame; presentation only reads this state.
//!
//! Movement parameters follow the executable's object init
//! (`physics::ObjParams`: PRTY walk/run/jump/turn, SSTM fall max) and its
//! gravity (`physics::G`); PTYA supplies the combo motions and ANB range
//! triggers the windows. Step height, body size and the collision response
//! are explicit test-room choices, not observed KH2 rules.

pub mod abilities;
pub mod camera;
pub mod combat;
pub mod physics;
pub mod ptya;
pub mod shadow;
pub mod targeting;

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
    /// PRTY words 10/11 (record +0x28/+0x2c): inertia and drag of mode 0's
    /// range-42 movement (FUN_140422350 → FUN_1403b7ad0).
    pub land_inertia: f32,
    pub land_drag: f32,
    /// Test-room choice: highest ledge climbed without jumping.
    pub step_height: f32,
    /// Stick magnitude from which Sora runs (`DAT_140585c7c`, 0.8).
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
    /// Range trigger 50: a finisher may chain (obj+0x194 bit 5).
    pub finisher_windows: Vec<(u32, u32)>,
    /// Earliest range trigger 2, "Blend to idle".
    pub blend_to_idle: Option<u32>,
    /// Range trigger 3, "No gravity".
    pub no_gravity: Vec<(u32, u32)>,
    /// Range triggers 0, 1 and 2: commands are taken while one is active
    /// (obj+0x18c & 0x1c, FUN_14040d810 / FUN_1401b15f0).
    pub actionable: Vec<(u32, u32)>,
    /// Range trigger 42: mode 0 moves inertially (obj+0x190 bit 0,
    /// FUN_140422350 → FUN_1403b7ad0 with PRTY words 10/11).
    pub inertia: Vec<(u32, u32)>,
    /// Range trigger 28: turn toward the target by up to `deg` per tick.
    pub turn_to_target: Vec<(u32, u32, f32)>,
    /// Frame trigger 21: turn toward the target by up to `deg` at a tick.
    pub face_target: Vec<(u32, f32)>,
    /// Frame trigger 0: the pending physics mode starts (jump take-off,
    /// FUN_14040dd70).
    pub takeoff: Option<u32>,
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
    /// PTYA +0xc "Jump": hop height when the step starts with no target
    /// (FUN_1403d78d0).
    pub hop: f32,
    /// PTYA +0x1c (speed maximum): horizontal speed of a hop without a
    /// target, and the cap toward one (FUN_1403d78d0).
    pub hop_speed: f32,
    /// PTYA +0x10 / +0x14: hop height limits toward a target.
    pub hop_max: f32,
    pub hop_min: f32,
    /// PTYA +0x18: minimum hop speed toward a target.
    pub hop_speed_min: f32,
    /// PTYA +0x3c: distance adjustment toward a target.
    pub hop_reach: f32,
    /// PTYA +0x38: blend into the next motion when this one ends
    /// (FUN_1403d78d0 → FUN_1403c8310).
    pub blend: f32,
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
    /// The battler's PTYA records (obj+0xd50) and, at the same index,
    /// each record's motion as an attack step.
    pub records: Vec<ptya::Record>,
    pub attacks: Vec<AttackStep>,
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
    /// A grounded attack (index into `Clips::records` / `attacks`).
    Attack(usize),
    /// An attack in the air, including ground records that hop.
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
    /// Target height above the player (Y-up) and the two bodies' summed
    /// radii, for aerial hops toward it (FUN_1403d78d0 / FUN_1403d7520).
    pub target_rise: f32,
    pub target_reach: f32,
    /// The current target's TARGET shape (the attack chooser's "target").
    pub target_shape: Option<ptya::Target>,
    /// Every attackable object's TARGET shapes near the player.
    pub nearby: [Option<ptya::Target>; 4],
    /// Grounded probe ahead hit the background (FUN_1403ba750) [U].
    pub probe_blocked: bool,
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
    /// Last tick's vertical displacement (Y-up units per tick).
    pub vertical_speed: f32,
    /// Jump/fall curve while airborne (obj+0xf0..+0xfc).
    pub arc: physics::Arc,
    /// Horizontal velocity while airborne (obj+0x20 x/z), units per tick.
    pub air_velocity: [f32; 2],
    /// Physics-mode velocity sampling (FUN_1403b89a0, obj+0xba8/+0xb98).
    pub sampler: physics::Sampler,
    /// A jump waiting for its take-off trigger (obj+0x648 = 1): height and
    /// take-off velocity (+0xe0).
    pub pending_jump: Option<(f32, [f32; 2])>,
    pub action: Action,
    /// Ticks since the current action started.
    pub frame: u32,
    /// Battle mode: weapon drawn, battle locomotion clips.
    pub armed: bool,
    /// Blend, in ticks, from the previous motion into the current one.
    pub blend: f32,
    idle_ticks: u32,
    /// Combo counters obj+0xd74 / +0xd75.
    pub combo: ptya::Combo,
    /// obj+0x120 bit 25: the last attack connected (set by a hit or a
    /// clash, cleared when an attack starts).
    pub connected: bool,
    /// Battle stats +0x1d0.
    pub abilities: abilities::Abilities,
    /// Stats +0x1c0 > 0 (MP charge).
    pub mp_charging: bool,
    /// Knock-slide (FUN_1403d4960 / FUN_1403d4570), for hits and the guard
    /// recoil alike.
    knock: Option<physics::Knock>,
}

/// Guard recoil knock strength (DAT_140585efc).
pub const GUARD_KNOCK: f32 = 200.;

/// What changed during one tick, for logging and presentation.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Step {
    pub started: Option<Action>,
    pub landed: bool,
}

/// Motion blend, in ticks, when an action's motion starts: locomotion and
/// the fall motion blend over 8 (`DAT_140623bac`: FUN_140422520,
/// FUN_1403cb610); jump (FUN_140422630), land (FUN_1403cb940), attacks
/// (FUN_1403d78d0), damage (FUN_1403d43b0), guard (FUN_1403a9c70) and its
/// recoil (FUN_1403d61d0) start with blend 0.
pub fn blend_for(action: Action) -> f32 {
    match action {
        Action::Idle | Action::Walk | Action::Run | Action::Fall => physics::MOTION_BLEND,
        _ => 0.,
    }
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
            arc: physics::Arc::default(),
            air_velocity: [0.; 2],
            sampler: physics::Sampler::default(),
            pending_jump: None,
            action: Action::Idle,
            frame: 0,
            armed: false,
            blend: 0.,
            idle_ticks: 0,
            combo: ptya::Combo::default(),
            connected: false,
            abilities: abilities::Abilities::default(),
            mp_charging: false,
            knock: None,
        }
    }

    /// Starts a grounded damage reaction with a knock-slide away along
    /// `direction` (XZ) at `strength` × 0.1 per tick, ×0.9 per tick until
    /// below 1 (FUN_1403d4960 / FUN_1403d4570). Airborne actions only take
    /// the slide.
    pub fn hurt(
        &mut self,
        index: usize,
        direction: [f32; 2],
        strength: f32,
        clips: &Clips,
    ) -> Option<Action> {
        self.knock = physics::Knock::new([direction[0], 0., direction[1]], strength, 0.);
        if self.in_air() || index >= clips.hurt.len() {
            return None;
        }
        self.pending_jump = None;
        self.action = Action::Hurt(index);
        self.frame = 0;
        self.blend = 0.;
        self.arm();
        Some(self.action)
    }

    /// Guard success (FUN_1403d61d0, grounded): face the attacker, play the
    /// recoil and knock-slide away at 200 × 0.1 per tick (×0.9 per tick,
    /// stopping below 1). `away` is the XZ unit vector from the attacker.
    pub fn guard_recoil(&mut self, away: [f32; 2], clips: &Clips) -> Option<Action> {
        if self.in_air() || clips.guard_recoil.is_none() {
            return None;
        }
        self.yaw = (-away[0]).atan2(-away[1]);
        self.knock = physics::Knock::new([away[0], 0., away[1]], GUARD_KNOCK, 0.);
        self.action = Action::GuardRecoil;
        self.frame = 0;
        self.blend = 0.;
        self.arm();
        Some(self.action)
    }

    fn start(&mut self, action: Action, step: &mut Step) {
        self.action = action;
        self.frame = 0;
        self.blend = blend_for(action);
        step.started = Some(action);
    }

    /// Mode 0's walk↔run switch (FUN_1403c6dc0): the new loop starts at the
    /// old one's phase, `fmod(len_new × (t + blend) / len_old, len_new)`,
    /// with the locomotion blend.
    fn switch_gait(&mut self, action: Action, clips: &Clips, step: &mut Step) {
        let old = self.clip(clips).end.max(1) as f32;
        let time = self.motion_frame(clips) as f32;
        self.start(action, step);
        let new = self.clip(clips).end.max(1) as f32;
        self.frame = ((new * ((time + physics::MOTION_BLEND) / old)) % new) as u32;
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
            (Action::Attack(i) | Action::AirAttack(i), _) => &clips.attacks[i].clip,
            (Action::Hurt(i), _) => &clips.hurt[i],
            (Action::Guard, _) => clips.guard.as_ref().unwrap_or(&clips.idle),
            (Action::GuardRecoil, _) => clips.guard_recoil.as_ref().unwrap_or(&clips.idle),
        }
    }

    /// Root-motion translation for the current attack frame, model space.
    pub fn root_offset(&self, clips: &Clips) -> Vec3 {
        let root = match self.action {
            Action::Attack(i) | Action::AirAttack(i) => &clips.attacks[i].root,
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

    /// In the jump/fall mode: airborne actions once any pending jump has
    /// taken off.
    pub fn in_air(&self) -> bool {
        self.action.airborne() && self.pending_jump.is_none()
    }

    /// An ANB range 0, 1 or 2 covers the current motion frame.
    fn actionable(&self, clips: &Clips) -> bool {
        Clip::within(&self.clip(clips).actionable, self.motion_frame(clips))
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

    /// PTYA attack start with a positive Jump field (FUN_1403d78d0), then
    /// the mode's jump slot (FUN_140409ad0: FUN_1403cbb00 + mode 1). With a
    /// target the height is its rise clamped to [+0x14, +0x10] and the speed
    /// (distance − reach + PTYA +0x3c) / √(2h/g) clamped to [+0x18, +0x1c]
    /// toward it; without one, PTYA +0xc along the facing at +0x1c.
    fn lift(&mut self, step: &AttackStep, input: &Input) {
        if !(step.hop.is_finite() && step.hop > 0.) {
            return;
        }
        let finite = |v: f32| if v.is_finite() { v } else { 0. };
        let (height, dir, speed) = match input.target {
            Some(t) if t[0].hypot(t[1]) > 1e-4 => {
                let (lo, hi) = (finite(step.hop_min), finite(step.hop_max));
                let rise = input.target_rise;
                let height = if rise < lo {
                    lo
                } else if rise > hi {
                    hi
                } else {
                    rise
                };
                let len = t[0].hypot(t[1]);
                let reach = len - input.target_reach + finite(step.hop_reach);
                let per_tick = reach / physics::rise_time(height);
                let (smin, smax) = (finite(step.hop_speed_min), finite(step.hop_speed));
                let speed = if smin <= per_tick {
                    per_tick.min(smax)
                } else {
                    smin
                };
                (height, [t[0] / len, t[1] / len], speed)
            }
            _ => (
                step.hop,
                [self.yaw.sin(), self.yaw.cos()],
                finite(step.hop_speed),
            ),
        };
        self.arc = physics::Arc::launch(self.position[1], height);
        self.air_velocity = [dir[0] * speed, dir[1] * speed];
        self.sampler.reset();
    }

    /// Jump (mode 0 slot +0x48, FUN_140422630 → FUN_1403cbb00): plays JUMP
    /// and queues the arc of the PRTY jump height with the stick velocity
    /// (obj+0xe0); the jump mode starts at JUMP's take-off trigger.
    fn jump(&mut self, p: &Params, velocity: [f32; 2], step: &mut Step) {
        self.pending_jump = Some((p.jump_height, velocity));
        self.vertical_speed = 0.;
        self.start(Action::Jump, step);
    }

    /// Leaving the ground without jumping (FUN_1403b4780 / FUN_1403b8150),
    /// keeping the ground velocity (obj+0xa38 → +0x20 [I: +0xa38 is the
    /// last ground velocity]).
    fn drop_off(&mut self, velocity: [f32; 2], step: &mut Step) {
        self.arc = physics::Arc::fall(self.position[1], 0.);
        self.pending_jump = None;
        self.air_velocity = velocity;
        self.sampler.reset();
        self.vertical_speed = 0.;
        self.start(Action::Fall, step);
    }

    /// FUN_1403a8fa0's move request: the stick direction at the walk speed
    /// below magnitude 0.8 and the run speed from it; zero without a stick.
    fn stick_velocity(direction: [f32; 2], magnitude: f32, p: &Params) -> [f32; 2] {
        if magnitude <= 0. {
            return [0.; 2];
        }
        let speed = if magnitude < p.walk_threshold {
            p.walk_speed
        } else {
            p.run_speed
        };
        [direction[0] * speed, direction[1] * speed]
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
        // Mode 0 move request this tick (obj+0x10 × +0x1c): the stick, but
        // only while an ANB range 0–2 is active (the same gate as commands);
        // LAND before tick 12, attacks, damage and guard leave it idle [I:
        // the stick feed's gate was not traced; the original locks movement
        // on landing].
        let mut ground_request = [0f32; 2];
        let was_in_air = self.in_air();
        let magnitude = input.direction[0].hypot(input.direction[1]).min(1.);
        // Any stick past the pad's dead zone moves (FUN_1403a8fa0: speed is
        // set whenever the stick magnitude is above 0).
        let moving = magnitude > 0.;
        let direction = if moving {
            [
                input.direction[0] / magnitude,
                input.direction[1] / magnitude,
            ]
        } else {
            [0., 0.]
        };
        let stick = Self::stick_velocity(direction, magnitude, p);
        match self.action {
            Action::Idle | Action::Walk | Action::Run | Action::Land => {
                // Commands need an active ANB range 0, 1 or 2 (obj+0x18c &
                // 0x1c, FUN_1401b15f0): always during locomotion, from
                // LAND's range-2 start (tick 12 installed) after a landing.
                let ready = self.actionable(clips);
                if ready && input.guard && !input.attack && clips.guard.is_some() {
                    self.start_guard(input, &mut step);
                } else if ready
                    && input.attack
                    && self.try_attack(input, p, clips, ready, moving, direction, &mut step)
                {
                } else if ready && input.jump {
                    self.jump(p, stick, &mut step);
                } else {
                    let landing = self.action == Action::Land;
                    // FUN_1403a8fa0: run speed from stick magnitude 0.8
                    // (DAT_140585c7c), walk speed below; FUN_140422520 then
                    // plays RUN when speed ≥ (walk + run) / 2, else WALK, but
                    // only while ANB range 0 or 2 is active (obj+0x18c bits
                    // 2/4); LAND otherwise plays to its end, then idles.
                    let target = if !moving {
                        if landing && self.frame < clips.land.end {
                            Action::Land
                        } else {
                            Action::Idle
                        }
                    } else if !ready {
                        self.action
                    } else if magnitude < p.walk_threshold {
                        Action::Walk
                    } else {
                        Action::Run
                    };
                    self.idle_ticks = self.idle_ticks.saturating_add(1);
                    if self.armed && self.idle_ticks >= p.disarm_ticks && target != Action::Land {
                        // Weapon put away: restart the locomotion clip in field mode.
                        self.armed = false;
                        self.start(target, &mut step);
                    } else if target != self.action {
                        if matches!(
                            (self.action, target),
                            (Action::Walk, Action::Run) | (Action::Run, Action::Walk)
                        ) {
                            self.switch_gait(target, clips, &mut step);
                        } else {
                            self.start(target, &mut step);
                        }
                    }
                    if moving && ready {
                        // FUN_1403b4780 turns the facing at +0x134 per tick;
                        // FUN_140422350 moves along the stick direction at
                        // the full walk/run speed (inertially under range 42).
                        self.turn_toward(direction, p.turn_speed);
                        ground_request = stick;
                    }
                }
            }
            Action::Jump if self.pending_jump.is_some() => {
                // The jump mode is pending (obj+0x648 = 1) until the JUMP
                // motion's frame trigger 0 (tick 4 installed) switches to it
                // (FUN_14040dd70); until then mode 0 keeps going, with JUMP's
                // range 1 (tick 8) not yet feeding the stick.
                if self.actionable(clips) {
                    ground_request = stick;
                }
                if self.frame >= clips.jump.takeoff.unwrap_or(0)
                    && let Some((height, velocity)) = self.pending_jump.take()
                {
                    // FUN_1403cbd30 (mode 1 enter): the arc's base is the
                    // take-off height; the stored +0xe0 velocity applies.
                    self.arc = physics::Arc::launch(self.position[1], height);
                    self.air_velocity = velocity;
                    self.sampler.reset();
                    self.airborne(
                        input, moving, magnitude, direction, p, clips, world, &mut step,
                    );
                }
            }
            Action::Attack(i) => {
                let clip = &clips.attacks[i].clip;
                self.face_target(clip, input);
                // Root motion: the attack's root-curve change this tick.
                let root = &clips.attacks[i].root;
                let at = |f: u32| root.get(clip.held_frame(f) as usize).copied();
                if let (Some(now), Some(before)) = (at(self.frame), at(self.frame - 1)) {
                    let (s, c) = self.yaw.sin_cos();
                    let (dx, dz) = (now[0] - before[0], now[2] - before[2]);
                    let delta = [dx * c + dz * s, -dx * s + dz * c];
                    if (delta[0] != 0. || delta[1] != 0.) && !self.travel(delta, p, world, true) {
                        self.drop_off(delta, &mut step);
                        return step;
                    }
                }
                let recovery = self.frame >= clip.blend_to_idle.unwrap_or(clip.end);
                let ready = self.actionable(clips);
                if input.attack
                    && self.try_attack(input, p, clips, ready, moving, direction, &mut step)
                {
                } else if recovery && input.jump {
                    self.jump(p, stick, &mut step);
                } else if recovery && moving {
                    self.start(Action::Idle, &mut step);
                } else if self.frame >= clip.end {
                    self.start(Action::Idle, &mut step);
                    self.blend = clips.attacks[i].blend;
                }
            }
            Action::Jump | Action::Fall | Action::AirAttack(_) => {
                self.airborne(
                    input, moving, magnitude, direction, p, clips, world, &mut step,
                );
            }
            Action::Hurt(_) | Action::Guard | Action::GuardRecoil => {
                // Locked until the clip's range-2 tick (hurt 50, guard 52,
                // recoil 40 installed), then input is taken again
                // (FUN_1401b15f0 actionable bits).
                let clip = self.clip(clips);
                let (end, ready) = (clip.end, clip.blend_to_idle.unwrap_or(clip.end));
                let actionable = self.frame >= ready;
                if input.attack
                    && self.try_attack(input, p, clips, actionable, moving, direction, &mut step)
                {
                } else if actionable {
                    if input.guard && clips.guard.is_some() {
                        self.start_guard(input, &mut step);
                    } else if input.jump {
                        self.jump(p, stick, &mut step);
                    } else if moving || self.frame >= end {
                        self.start(Action::Idle, &mut step);
                    }
                }
            }
        }
        if !was_in_air && !self.in_air() {
            // Mode 0 (FUN_140422350) sampled by FUN_1403b89a0 every second
            // frame. Under ANB range 42 (obj+0x190 bit 0, e.g. all of LAND)
            // the velocity eases toward the request through FUN_1403b7ad0
            // with PRTY words 10/11 as inertia and drag; otherwise it is the
            // request itself.
            let speed = ground_request[0].hypot(ground_request[1]);
            if self.sampler.due(speed) {
                let dt = physics::PHYSICS_STEP;
                let clip = self.clip(clips);
                if Clip::within(&clip.inertia, self.frame) {
                    let dir = if speed > 0. {
                        [ground_request[0] / speed, ground_request[1] / speed]
                    } else {
                        [0.; 2]
                    };
                    self.air_velocity =
                        physics::steer(self.air_velocity, dir, speed, p.land_inertia, p.land_drag);
                } else {
                    self.air_velocity = ground_request;
                }
                self.sampler
                    .store([self.air_velocity[0] * dt, 0., self.air_velocity[1] * dt]);
            }
            let [vx, _, vz] = self.sampler.displacement();
            if (vx != 0. || vz != 0.) && !self.travel([vx, vz], p, world, true) {
                self.drop_off([vx, vz], &mut step);
            }
        }
        if let Some(knock) = self.knock.as_mut() {
            match knock.step() {
                Some(d) => {
                    let grounded = !self.in_air();
                    self.travel([d[0], d[2]], p, world, grounded);
                }
                None => self.knock = None,
            }
        }
        step
    }

    /// The current motion id (obj+0x180) as far as the controller knows it.
    pub fn motion_id(&self, clips: &Clips) -> u16 {
        match self.action {
            Action::Attack(i) | Action::AirAttack(i) => clips.records[i].motion,
            Action::Idle => 0,
            Action::Walk => 1,
            Action::Run => 2,
            Action::Jump => 3,
            Action::Fall => 4,
            Action::Land => 5,
            Action::Hurt(i) => 10 + i as u16,
            Action::Guard => 173,
            Action::GuardRecoil => 42,
        }
    }

    /// The attack command (FUN_1403d6fc0 type 1 → FUN_1403d6ca0, then
    /// FUN_1403a8690 → FUN_1403d78d0). The press counts toward the combo
    /// only inside an "allow combo" (11) or finisher (50) range after an
    /// attack that connected (obj+0x120 bit 25) or with Combo Master;
    /// otherwise it needs an active range 0–2 and starts a new combo. The
    /// record comes from the PTYA chooser, or Counterguard (0x27) out of the
    /// guard recoil. Returns false when nothing starts. Requests last one
    /// frame (FUN_1403b2340 clears them), so there is no input buffer.
    #[allow(clippy::too_many_arguments)]
    fn try_attack(
        &mut self,
        input: &Input,
        p: &Params,
        clips: &Clips,
        actionable: bool,
        moving: bool,
        direction: [f32; 2],
        step: &mut Step,
    ) -> bool {
        if clips.records.is_empty() {
            return false;
        }
        let clip = self.clip(clips);
        let frame = self.motion_frame(clips);
        let state = ptya::State {
            abilities: &self.abilities,
            airborne: self.in_air(),
            mp_charging: self.mp_charging,
            combo_window: Clip::within(&clip.combo_windows, frame),
            finisher_window: Clip::within(&clip.finisher_windows, frame),
            actionable,
        };
        let chain = self.connected || self.abilities.has(abilities::bit::COMBO_MASTER);
        let mut combo = self.combo;
        if !ptya::advance(&mut combo, &state, chain) {
            return false;
        }
        let current = self.motion_id(clips);
        let counter = if current == 42 {
            ptya::find(&clips.records, 0x27)
                .filter(|&i| ptya::allowed(&clips.records[i], &state, combo))
        } else {
            None
        };
        // [test-room carry-over] The stick sets the facing an attack is
        // chosen and swung along.
        let facing_yaw = if moving {
            direction[0].atan2(direction[1])
        } else {
            self.yaw
        };
        let nearby: Vec<ptya::Target> = input.nearby.iter().flatten().copied().collect();
        let scene = ptya::Scene {
            origin: self.position,
            facing: [facing_yaw.sin(), facing_yaw.cos()],
            target: input.target_shape,
            others: &nearby,
            jump_height: p.jump_height,
            current_motion: current,
        };
        let Some(i) = counter
            .or_else(|| ptya::select(&clips.records, &state, combo, &scene, input.probe_blocked))
        else {
            return false;
        };
        let airborne = self.in_air();
        self.combo = combo;
        self.connected = false;
        self.yaw = facing_yaw;
        let record = clips.records[i];
        // FUN_1403d78d0: a hop from the ground starts a fresh count.
        if record.jump > 0. && !airborne {
            self.combo = ptya::Combo { step: 1, finish: 1 };
        }
        self.arm();
        self.pending_jump = None;
        let action = if airborne || record.leaves_ground() {
            Action::AirAttack(i)
        } else {
            Action::Attack(i)
        };
        self.start(action, step);
        self.face_target(&clips.attacks[i].clip, input);
        self.lift(&clips.attacks[i], input);
        true
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

    /// Attack facing toward the target: ANB range 28 turns by up to its
    /// parameter in degrees per tick (FUN_14040d810 → FUN_1403d4b10 →
    /// FUN_1403d3a90, `DAT_1405b15b4` = π/180) and frame trigger 21 by up to
    /// its parameter at that tick, both only beyond 50 units horizontally
    /// (`DAT_1405b532c`).
    fn face_target(&mut self, clip: &Clip, input: &Input) {
        let Some(t) = input.target else { return };
        if t[0].hypot(t[1]) <= 50. {
            return;
        }
        let frame = self.frame;
        let ranges = clip
            .turn_to_target
            .iter()
            .filter(|&&(a, b, _)| (a..=b).contains(&frame))
            .map(|&(_, _, deg)| deg);
        let ticks = clip
            .face_target
            .iter()
            .filter(|&&(f, _)| f == frame)
            .map(|&(_, deg)| deg);
        let limit = ranges.chain(ticks).fold(0f32, f32::max);
        if limit > 0. {
            self.turn_toward(t, limit * std::f32::consts::PI / 180.);
        }
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
            let clip = &clips.attacks[i].clip;
            self.face_target(clip, input);
            let ready = self.actionable(clips);
            if input.attack && self.try_attack(input, p, clips, ready, moving, direction, step) {
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
                self.blend = clips.attacks[i].blend;
            }
        } else if input.attack
            && self.actionable(clips)
            && self.try_attack(input, p, clips, true, moving, direction, step)
        {
            // Air attacks need an active ANB range 0–2 too: JUMP from its
            // range-1 start (tick 8 installed), FALL throughout.
        } else if self.action == Action::Jump && self.arc.falling_motion() {
            // FUN_1403cb680 → FUN_1403cb610: FALL (motion 4) blended over 8
            // once the apex is within 5 ticks.
            self.start(Action::Fall, step);
        }
        // ANB "No gravity" ranges (range 3 → obj+0x18c bit 5, FUN_14040d810)
        // hold the height while the curve's time keeps running
        // (FUN_1403cb500).
        let floating = Clip::within(&self.clip(clips).no_gravity, self.frame);
        // Mode 1 horizontal (FUN_1403cb360 → FUN_1403b7ad0 with 0.9, 0.95):
        // the velocity keeps 90% of itself and takes 10% of the stick
        // velocity, or decays ×0.95 without a stick. The stick only feeds it
        // under an ANB range 0–2: JUMP from tick 8, FALL throughout, never
        // during aerial attacks [I, as on the ground].
        let steering = moving && self.actionable(clips);
        if steering {
            self.turn_toward(direction, p.turn_speed);
        }
        let (dir, speed) = if !steering {
            ([0.; 2], 0.)
        } else {
            let v = Self::stick_velocity(direction, magnitude, p);
            (direction, v[0].hypot(v[1]))
        };
        // FUN_1403b89a0: the mode routine runs every second frame with a
        // two-frame step; its displacement is spread over those frames.
        if self.sampler.due(speed) {
            let dt = physics::PHYSICS_STEP;
            let dy = self.arc.step(self.position[1], p.fall_max, floating, dt);
            self.air_velocity = physics::steer(
                self.air_velocity,
                dir,
                speed,
                physics::AIR_INERTIA,
                physics::AIR_DRAG,
            );
            self.sampler
                .store([self.air_velocity[0] * dt, dy, self.air_velocity[1] * dt]);
        }
        let [vx, vy, vz] = self.sampler.displacement();
        self.vertical_speed = vy;
        if vx != 0. || vz != 0. {
            self.travel([vx, vz], p, world, false);
        }
        let [x, y, z] = self.position;
        let next = y + self.vertical_speed;
        if self.vertical_speed <= 0.
            && let Some(floor) = world.floor(x, z, y + p.step_height)
            && next <= floor
        {
            self.position[1] = floor;
            self.vertical_speed = 0.;
            // Landing switches to the ground mode and plays the land motion
            // (FUN_1403cb940: obj+0x11c = 5, which FUN_1403d78d0 also sets
            // for aerial attacks), cutting an aerial swing short. The air
            // velocity stays in obj+0x20 for LAND's inertial slide.
            self.sampler = physics::Sampler::default();
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
            land_inertia: 0.9,
            land_drag: 0.8,
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
                ..Clip::default()
            },
            hop,
            hop_speed: 0.,
            hop_max: 0.,
            hop_min: 0.,
            hop_speed_min: 0.,
            hop_reach: 0.,
            blend: 0.,
            root: Vec::new(),
        }
    }
    /// A cut of Sora's set: finisher 3, the basic swing 5 (alternate 6),
    /// the target dash 6, and the air swings 9 (alternate 10) and 10.
    fn records() -> Vec<ptya::Record> {
        use ptya::{flag, kind};
        let r = |id, kind, alternate, flags, motion, jump| ptya::Record {
            id,
            kind,
            alternate,
            flags,
            motion,
            jump,
            cos_min: 0.99996,
            cos_max: 1.,
            ..ptya::Record::default()
        };
        vec![
            r(3, kind::ALWAYS, -1, flag::FINISHER, 155, 0.),
            r(5, kind::ALWAYS, 6, 0, 151, 0.),
            r(6, kind::TARGET, -1, 0, 153, 0.),
            r(9, kind::ALWAYS, 10, flag::AIR, 181, 21.),
            r(10, kind::TARGET, -1, flag::AIR, 182, 21.),
        ]
    }
    /// A loop with ANB range 0 over its whole length (always actionable).
    fn open(end: u32) -> Clip {
        Clip {
            actionable: vec![(0, end)],
            ..Clip::looping(end, 0)
        }
    }
    fn clips() -> Clips {
        Clips {
            idle: open(100),
            walk: open(60),
            run: open(40),
            field_idle: open(90),
            field_walk: open(50),
            field_run: open(36),
            jump: open(30),
            fall: open(30),
            land: Clip::looping(12, 0),
            records: records(),
            attacks: vec![
                attack(50, (60, 60), 0.),
                attack(40, (10, 25), 0.),
                attack(40, (10, 25), 0.),
                attack(30, (8, 20), 20.),
                attack(30, (8, 20), 20.),
            ],
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
        // FUN_1403b89a0 samples the move request every second frame: the
        // run starts next frame, the walk velocity carries this one.
        player.tick(&hold([0., 1.]), &p, &c, &w);
        assert_eq!(player.action, Action::Run);
        assert!((player.position[2] - 4.).abs() < 1e-5);
        player.tick(&hold([0., 1.]), &p, &c, &w);
        assert!((player.position[2] - 12.).abs() < 1e-5);
        // Facing +X requires a quarter turn: limited per tick, while the
        // body moves along the stick (FUN_140422350) at the next sample.
        player.tick(&hold([1., 0.]), &p, &c, &w);
        assert!((player.yaw - 0.2618).abs() < 1e-5);
        assert!((player.position[2] - 20.).abs() < 1e-5);
        player.tick(&hold([1., 0.]), &p, &c, &w);
        assert!((player.position[0] - 8.).abs() < 1e-5);
        assert!((player.position[2] - 20.).abs() < 1e-5);
        player.tick(&Input::default(), &p, &c, &w);
        assert_eq!(player.action, Action::Idle);
    }

    #[test]
    fn walk_to_run_keeps_the_gait_phase_and_blends_eight() {
        let (p, c, w) = (params(), clips(), room());
        let mut player = Player::new([0., 0., 0.], 0.);
        for _ in 0..20 {
            player.tick(&hold([0., 0.3]), &p, &c, &w);
        }
        assert_eq!((player.action, player.motion_frame(&c)), (Action::Walk, 19));
        assert_eq!(player.blend, 8.);
        player.tick(&hold([0., 1.]), &p, &c, &w);
        // Field walk 50 → field run 36: 36 × (20 + 8) / 50 = 20.16.
        assert_eq!((player.action, player.frame), (Action::Run, 20));
        let jump = Input {
            jump: true,
            ..Input::default()
        };
        player.tick(&jump, &p, &c, &w);
        assert_eq!(player.blend, 0.);
    }

    #[test]
    fn running_jump_carries_momentum_then_drags() {
        let (p, c, w) = (params(), clips(), room());
        let mut player = Player::new([-900., 0., 0.], 0.);
        let run_jump = Input {
            direction: [0., 1.],
            jump: true,
            ..Input::default()
        };
        player.tick(&run_jump, &p, &c, &w);
        assert_eq!(player.pending_jump, Some((185., [0., 8.])));
        // Stick released: FUN_1403b7ad0 scales the velocity by 0.95 per
        // two-frame sample, moving 7.6 on each of those frames.
        let z = player.position[2];
        player.tick(&Input::default(), &p, &c, &w);
        player.tick(&Input::default(), &p, &c, &w);
        assert!((player.position[2] - z - 15.2).abs() < 1e-4);
        // Stick sideways: 10% of the stick velocity at the next sample.
        player.tick(&hold([1., 0.]), &p, &c, &w);
        assert!((player.air_velocity[0] - 0.8).abs() < 1e-5);
        assert!((player.air_velocity[1] - 7.6 * 0.9).abs() < 1e-4);
    }

    #[test]
    fn fall_motion_starts_five_ticks_before_the_apex() {
        let (p, c, w) = (params(), clips(), room());
        let mut player = Player::new([0., 0., 0.], 0.);
        let jump = Input {
            jump: true,
            ..Input::default()
        };
        player.tick(&jump, &p, &c, &w);
        let mut fell = None;
        for tick in 1..40 {
            let step = player.tick(&Input::default(), &p, &c, &w);
            if step.started == Some(Action::Fall) {
                fell = Some(tick);
                break;
            }
        }
        // rise √(2·185/g) = 30.1 ticks; FALL starts when rise ≤ t + 5. The
        // curve's t advances by 2 every second frame (t = 26 on tick 26).
        assert_eq!(fell, Some(26));
        assert_eq!(player.blend, 8.);
        assert!(player.position[1] < 185.);
    }

    /// The installed JUMP/LAND trigger layout: take-off at JUMP tick 4,
    /// commands from JUMP tick 8; LAND locked until tick 12 and inertial
    /// throughout (range 42).
    fn installed_jump_land(c: &mut Clips) {
        c.jump = Clip {
            actionable: vec![(8, 26)],
            takeoff: Some(4),
            ..Clip::looping(26, 0)
        };
        c.land = Clip {
            actionable: vec![(12, 26)],
            inertia: vec![(0, 26)],
            blend_to_idle: Some(12),
            ..Clip::looping(26, 0)
        };
    }

    #[test]
    fn jump_takes_off_at_the_jump_trigger() {
        let (p, mut c, w) = (params(), clips(), room());
        installed_jump_land(&mut c);
        let mut player = Player::new([0., 0., 0.], 0.);
        let jump = Input {
            jump: true,
            ..Input::default()
        };
        player.tick(&jump, &p, &c, &w);
        assert_eq!(player.action, Action::Jump);
        for _ in 1..4 {
            player.tick(&Input::default(), &p, &c, &w);
            assert_eq!(player.position[1], 0.);
            assert!(player.pending_jump.is_some());
        }
        // FUN_14040dd70: frame trigger 0 switches to the jump mode.
        player.tick(&Input::default(), &p, &c, &w);
        assert!(player.pending_jump.is_none());
        assert!(player.position[1] > 0.);
        // No air attack before JUMP's range-1 start (tick 8).
        let press = Input {
            attack: true,
            ..Input::default()
        };
        player.tick(&press, &p, &c, &w);
        assert_eq!(player.action, Action::Jump);
        for _ in 0..3 {
            player.tick(&Input::default(), &p, &c, &w);
        }
        player.tick(&press, &p, &c, &w);
        assert_eq!(player.action, Action::AirAttack(3));
    }

    #[test]
    fn landing_locks_commands_and_slides_inertially() {
        let (p, mut c, w) = (params(), clips(), room());
        installed_jump_land(&mut c);
        let mut player = Player::new([-900., 0., 0.], 0.);
        let run_jump = Input {
            direction: [0., 1.],
            jump: true,
            ..Input::default()
        };
        player.tick(&run_jump, &p, &c, &w);
        let mut landed = false;
        for _ in 0..120 {
            if player.tick(&hold([0., 1.]), &p, &c, &w).landed {
                landed = true;
                break;
            }
        }
        assert!(landed);
        assert_eq!(player.action, Action::Land);
        // Locked until LAND tick 12: no jump, no switch to running and no
        // stick; range 42 eases the carried velocity by the PRTY drag.
        let push = Input {
            direction: [0., 1.],
            jump: true,
            ..Input::default()
        };
        for _ in 0..10 {
            player.tick(&push, &p, &c, &w);
            assert_eq!(player.action, Action::Land);
        }
        assert!(player.air_velocity[1] > 0.);
        player.tick(&hold([0., 1.]), &p, &c, &w);
        player.tick(&hold([0., 1.]), &p, &c, &w);
        assert_eq!(player.action, Action::Run);
    }

    #[test]
    fn aerial_hop_heads_for_the_target() {
        let (p, mut c, w) = (params(), clips(), room());
        c.attacks[3].hop_max = 71.;
        c.attacks[3].hop_min = 21.;
        c.attacks[3].hop_speed_min = -8.;
        c.attacks[3].hop_speed = 8.;
        let mut player = Player::new([0., 0., 0.], 0.);
        let jump = Input {
            jump: true,
            ..Input::default()
        };
        player.tick(&jump, &p, &c, &w);
        for _ in 0..10 {
            player.tick(&Input::default(), &p, &c, &w);
        }
        let press = Input {
            attack: true,
            target: Some([400., 0.]),
            target_rise: -100.,
            ..Input::default()
        };
        player.tick(&press, &p, &c, &w);
        assert_eq!(player.action, Action::AirAttack(3));
        // Height clamps to the 21 minimum; speed 400 / √(2·21/g) > 8 caps
        // at 8 toward +X, then the first two-frame sample drags it ×0.95.
        assert!((player.arc.rise - physics::rise_time(21.)).abs() < 1e-4);
        assert!((player.air_velocity[0] - 7.6).abs() < 1e-4);
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
        // FUN_1403cb500: the double-sine rise reaches 185 at t ≈ 30.1 and
        // the parabola lands at t ≈ 60.2.
        assert!(apex > 184.9 && apex <= 185.001, "apex {apex}");
        assert_eq!(landed, Some(60));
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
        assert_eq!(player.action, Action::Attack(1));
        assert_eq!(player.combo, ptya::Combo { step: 1, finish: 1 });
        // Presses last one frame: before the window nothing happens, and a
        // swing that missed cannot chain inside it either.
        for _ in 0..9 {
            player.tick(&press, &p, &c, &w);
            assert_eq!(player.action, Action::Attack(1));
        }
        player.tick(&press, &p, &c, &w);
        assert_eq!(player.action, Action::Attack(1));
        // After a hit the press in the window chains: 5 would repeat 151,
        // so its alternate 6 (153) plays.
        player.connected = true;
        player.tick(&press, &p, &c, &w);
        assert_eq!(player.action, Action::Attack(2));
        assert_eq!(player.combo.step, 2);
        assert!(!player.connected);
        for _ in 0..10 {
            player.tick(&Input::default(), &p, &c, &w);
        }
        // Third step reaches the limit 3: the finisher.
        player.connected = true;
        player.tick(&press, &p, &c, &w);
        assert_eq!(player.action, Action::Attack(0));
        // Without another press the step finishes and returns to idle.
        for _ in 0..60 {
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
        assert_eq!(player.action, Action::Attack(1));
        assert_eq!(player.position, [0., 0., 0.]);
        for _ in 0..15 {
            player.tick(&hold([0., 1.]), &p, &c, &w);
        }
        assert!(matches!(player.action, Action::Run | Action::Idle));
    }

    #[test]
    fn air_attacks_hop_and_restart_from_the_fall() {
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
        assert_eq!(player.action, Action::AirAttack(3));
        assert!(player.vertical_speed > 0.);
        for _ in 0..40 {
            player.tick(&Input::default(), &p, &c, &w);
        }
        assert!(!matches!(player.action, Action::AirAttack(_)));
        // FALL is actionable (range 0): a new air combo starts from step 1
        // (FUN_1403d6ca0), so the exe allows another swing while airborne.
        if player.action == Action::Fall {
            player.tick(&press, &p, &c, &w);
            assert_eq!(player.action, Action::AirAttack(3));
            assert_eq!(player.combo.step, 1);
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
        c.attacks[1].root = (0..40).map(|f| [0., 0., f as f32 * 2.]).collect();
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
        assert_eq!(player.hurt(1, [0., -1.], 100., &c), Some(Action::Hurt(1)));
        assert!(player.armed);
        let p = params();
        player.tick(&Input::default(), &p, &c, &world);
        assert!((player.position[2] + 10.).abs() < 1e-3);
        player.tick(&Input::default(), &p, &c, &world);
        assert!((player.position[2] + 19.).abs() < 1e-3);
        for _ in 0..30 {
            player.tick(&Input::default(), &p, &c, &world);
        }
        assert_eq!(player.action, Action::Idle);
    }
}
