//! A party limit's scene: the program of the limit file run as the limit
//! object's thread (FUN_1403d7fa0 / FUN_1403d96d0: trigger 22 with the
//! limit object as its argument), the threads it starts, the action threads
//! it hooks on the participants (`obj_hook`, FUN_14042ca20 / FUN_1403d76d0:
//! obj+0xd48, run by the participant's own update, FUN_1403d7280), its
//! follow-ups (FUN_1403d8b40's other branch: trigger 23 with the command,
//! run once) and the gauge time (FUN_1403d9130).
//!
//! The participants are engine objects here: Sora is the player stand-in,
//! the friends are objects of the world. The host reads back where they are
//! and what they play, and carries out the events (motions of the limit
//! file, effects, sounds, attacks, reaction commands).

use crate::env::Env;
use crate::script::{Child, Slot, Thread};
use crate::sysobj::Sysobj;
use crate::traps::limit::LimitState;
use crate::world::{Event, ObjId, Object, PLAYER_OBJECT, World, object_handle, run_thread};

/// Trigger keys of a limit program.
pub const START: i32 = 22;
pub const FOLLOW_UP: i32 = 23;
/// The id the program's own object takes (it holds the program instance).
pub const PROGRAM: ObjId = 0x00ff_fffd;
/// Frame slots: the scene thread, the action threads.
const SCENE_SLOT: u32 = 0;
const ACTION_SLOT: u32 = 1;
/// Trinity Limit's command: its file brings a special command deck.
pub const TRINITY: u16 = 0x1e4;
/// obj+0x194 bit 0 (range event 0x2d): the gauge holds while it is set.
const HOLDS_TIME: u32 = 1;

/// One engine object taking part (a friend) or present (an enemy).
pub struct Participant {
    pub id: ObjId,
    pub sys: Sysobj,
}

/// How the scene ended.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Status {
    Running,
    /// The scene thread and its children are done and no limit motion plays.
    Over,
}

pub struct Scene {
    pub world: World,
    program: Object,
    scene: Thread,
    /// The action thread of each participant (obj+0xd48), by object.
    actions: Vec<(ObjId, Thread)>,
    /// The gauge time runs (the limit's gauge is up, FUN_1403fa140).
    pub gauge_shown: bool,
    /// The gauge's rate (Sora's ability record +0x64).
    pub time_rate: f32,
}

impl Scene {
    /// FUN_1403d7fa0 for a friend limit: the limit object with its friends,
    /// then the program's trigger 22 thread with it as the argument.
    pub fn start(
        env: &dyn Env,
        program: &[u8],
        command: u16,
        time: f32,
        player: Sysobj,
        friends: Vec<Participant>,
        others: Vec<Participant>,
    ) -> Result<Self, String> {
        let mut world = World::default();
        world.tolerate_missing = true;
        world.player_scripted = true;
        // Sora's side is not of the enemy class (+0x6c8 bit 4, which
        // `enemy_each` walks); the friends and the program neither.
        let not_enemy = |mut s: Sysobj| {
            s.class_flags &= !crate::sysobj::class_flag::ENEMY;
            s
        };
        let mut proxy = Object::bare(PLAYER_OBJECT);
        proxy.sys = not_enemy(player);
        let friends: Vec<Participant> = friends
            .into_iter()
            .map(|p| Participant {
                id: p.id,
                sys: not_enemy(p.sys),
            })
            .collect();
        world.player_proxy = Some(Box::new(proxy));
        let mut members = [None; 2];
        for (k, p) in friends.iter().enumerate().take(2) {
            members[k] = Some(p.id);
        }
        for p in friends.into_iter().chain(others) {
            let id = p.id as usize;
            if world.objects.len() <= id {
                world.objects.resize_with(id + 1, || None);
            }
            let mut o = Object::bare(p.id);
            o.sys = p.sys;
            world.objects[id] = Some(Box::new(o));
        }
        let mut state = LimitState::new(&mut world, command, members, time);
        // Trinity Limit's `comd` sheets (FUN_1403d96d0).
        if command == TRINITY {
            state.open_special();
        }
        let handle = state.handle;
        world.limit = Some(state);
        let program = Object::spawn(&mut world, env, PROGRAM, program, not_enemy(Sysobj::bare()))
            .map_err(|e| format!("limit program rejected: {e:?}"))?;
        let entry = program.script.instance.trigger(START);
        if entry <= 0 {
            return Err("limit program has no trigger 22".into());
        }
        let scene = Thread::with_sizes(
            program.script.stack_bytes,
            program.script.temp_bytes,
            SCENE_SLOT,
            entry,
            &[Slot::address(handle)],
        );
        Ok(Scene {
            world,
            program,
            scene,
            actions: Vec::new(),
            gauge_shown: true,
            time_rate: 1.0,
        })
    }

    /// FUN_140540fa0, the flare pool's task: Sora's push shapes where he
    /// stands, then each flare in use parts from the others and from them;
    /// the effects on moved flares follow.
    fn separate_flares(&mut self, env: &dyn Env) {
        let p = self.player().position;
        let pushers: Vec<_> = env
            .player_push_shapes()
            .into_iter()
            .map(|mut s| {
                let c = s.center;
                s.center = [p[0] + c[0], -p[1] + c[1], p[2] + c[2], 1.];
                s
            })
            .collect();
        let Some(pool) = self.world.flares.as_mut() else {
            return;
        };
        for i in pool.separate(&pushers) {
            crate::traps::flare::effect_follows(&mut self.world.events, &pool.flares[i]);
        }
    }

    /// FUN_1403b6950 for a command the scene offered: Sora is the user, the
    /// override record's object (a friend) the target.
    pub fn can_decide(&self, env: &dyn Env, command: u16, target: Option<ObjId>) -> bool {
        let them = target.and_then(|t| self.object(t).map(|o| (t, &o.sys)));
        crate::traps::state::decides(env, command, (PLAYER_OBJECT, self.player()), them)
    }

    /// The first effect handle the next tick's traps will hand out.
    pub fn effect_watermark(&self) -> u32 {
        self.world.effect_watermark()
    }

    /// The scene's world (its attack objects, for the host to land).
    pub fn world(&self) -> &World {
        &self.world
    }

    /// Record every trap the scene calls (a development aid).
    pub fn trace(&mut self) {
        self.world.trace.get_or_insert_with(Vec::new);
    }

    /// The traps called so far, with their counts, when tracing.
    pub fn trace_counts(&self) -> Vec<((u16, u16), usize)> {
        let mut out: Vec<((u16, u16), usize)> = Vec::new();
        for &t in self.world.trace.iter().flatten() {
            match out.iter_mut().find(|(k, _)| *k == t) {
                Some(e) => e.1 += 1,
                None => out.push((t, 1)),
            }
        }
        out
    }

    /// The host landed attack `slot` on a target (its `attack_is_hit`).
    pub fn struck(&mut self, slot: usize) {
        if let Some(Some(a)) = self.world.attacks.get_mut(slot) {
            a.mark_hit(crate::attack::flag::STRUCK);
        }
    }

    /// Hits that landed this tick, for the combo counter (+0xc0).
    pub fn count_hits(&mut self, n: i32) {
        if let Some(l) = self.world.limit.as_mut() {
            l.hits += n;
        }
    }

    pub fn limit(&self) -> &LimitState {
        self.world.limit.as_ref().expect("limit state")
    }

    pub fn player(&self) -> &Sysobj {
        &self.world.player_proxy.as_ref().expect("player").sys
    }

    pub fn player_mut(&mut self) -> &mut Sysobj {
        &mut self.world.player_proxy.as_mut().expect("player").sys
    }

    pub fn object(&self, id: ObjId) -> Option<&Object> {
        self.world.objects.get(id as usize)?.as_deref()
    }

    pub fn object_mut(&mut self, id: ObjId) -> Option<&mut Object> {
        self.world.objects.get_mut(id as usize)?.as_deref_mut()
    }

    /// A follow-up command (the reaction button on the command the scene
    /// offered): trigger 23 with the command, run until it yields.
    pub fn follow_up(&mut self, env: &dyn Env, command: u16) {
        self.program.fire(
            &mut self.world,
            env,
            FOLLOW_UP,
            &[Slot::int(i32::from(command))],
        );
        self.adopt(env);
    }

    /// The threads the program asked for since the last call: the scene's
    /// new children, and the action threads `obj_hook` started (each
    /// replaces the object's previous one).
    fn adopt(&mut self, _env: &dyn Env) {
        let (stack, temp) = (
            self.program.script.stack_bytes,
            self.program.script.temp_bytes,
        );
        if let Some(l) = self.world.limit.as_mut() {
            for r in std::mem::take(&mut l.threads) {
                let n = self.scene.children.len() as u32;
                self.scene.children.push(Child {
                    kind: 0,
                    tag: 0,
                    handle: crate::script::CHILD_THREADS + 0x8000 + n * 16,
                    thread: Thread::with_sizes(stack, temp, 3 + n % 12, r.entry, &[r.argument]),
                });
            }
        }
        let mut kept = Vec::new();
        for event in std::mem::take(&mut self.world.events) {
            match event {
                Event::MagicStart {
                    object,
                    id: entry,
                    hook: true,
                    other,
                    ..
                } => {
                    let mut args = vec![Slot::address(object_handle(object))];
                    if let Some(o) = other {
                        args.push(Slot::address(object_handle(o)));
                    }
                    let thread = Thread::with_sizes(stack, temp, ACTION_SLOT, entry, &args);
                    self.actions.retain(|(id, _)| *id != object);
                    self.actions.push((object, thread));
                }
                Event::MagicStop { .. } => {}
                other => kept.push(other),
            }
        }
        self.world.events = kept;
    }

    fn tick_participant(&mut self, env: &dyn Env, id: ObjId) {
        let mut object = if id == PLAYER_OBJECT {
            self.world.player_proxy.take()
        } else {
            self.world
                .objects
                .get_mut(id as usize)
                .and_then(Option::take)
        };
        let Some(o) = object.as_mut() else { return };
        let before = o.sys.motion.slot;
        o.sys.begin_tick(env);
        // FUN_1403c6740 starts a queued motion through FUN_1403c64a0, which
        // hands the object a new token (+0x184 = DAT_14074b980 - 1): scripts
        // waiting on the old one see their motion replaced.
        if o.sys.motion.slot != before
            && let Some(l) = self.world.limit.as_mut()
        {
            o.sys.limit_token = l.next_token();
        }
        o.carry_out_motion_effects(&mut self.world, env);
        let action = self.actions.iter().position(|(a, _)| *a == id);
        // Without an action thread nothing asks for movement this tick (the
        // participant's own control would; it is off during the limit) [I].
        if action.is_none() {
            o.sys.move_speed = 0.;
        }
        // Put the object back while its action thread runs: the thread's
        // traps reach it through the world.
        if id == PLAYER_OBJECT {
            self.world.player_proxy = object.take();
        } else {
            self.world.objects[id as usize] = object.take();
        }
        if let Some(k) = action {
            let mut thread =
                std::mem::replace(&mut self.actions[k].1, Thread::with_sizes(0, 0, 0, 0, &[]));
            run_thread(&mut self.world, env, &mut self.program, &mut thread);
            if thread.finished() {
                self.actions.retain(|(a, _)| *a != id);
            } else if let Some(slot) = self.actions.iter_mut().find(|(a, _)| *a == id) {
                slot.1 = thread;
            }
        }
        let object = if id == PLAYER_OBJECT {
            self.world.player_proxy.as_deref_mut()
        } else {
            self.world
                .objects
                .get_mut(id as usize)
                .and_then(|o| o.as_deref_mut())
        };
        if let Some(o) = object {
            o.sys.end_tick(env);
            if let Some(l) = self.world.limit.as_mut() {
                l.settle(&mut o.sys);
            }
        }
    }

    /// One tick: Sora and the friends (motion clock and events, their action
    /// threads, physics), the other objects' bodies, then the scene thread and
    /// its children, then the gauge time.
    pub fn tick(&mut self, env: &dyn Env) -> Status {
        let members = self.limit().members;
        self.tick_participant(env, PLAYER_OBJECT);
        for id in members.into_iter().flatten() {
            self.tick_participant(env, id);
        }
        // The other objects (weapons, the enemies' bodies here): their motion
        // clocks and physics.
        for k in 0..self.world.objects.len() {
            let id = k as ObjId;
            if members.contains(&Some(id)) {
                continue;
            }
            let Some(mut o) = self.world.objects[k].take() else {
                continue;
            };
            o.sys.begin_tick(env);
            o.carry_out_motion_effects(&mut self.world, env);
            o.sys.end_tick(env);
            if let Some(l) = self.world.limit.as_mut() {
                l.settle(&mut o.sys);
            }
            self.world.objects[k] = Some(o);
        }
        self.world.follow_attachments(env, None);
        run_thread(&mut self.world, env, &mut self.program, &mut self.scene);
        self.adopt(env);
        self.world.tick_attacks(env);
        self.separate_flares(env);
        crate::traps::trinity::tick(&mut self.world, env);
        // FUN_1403d9130: the gauge runs down while it is up and no
        // participant's motion holds it.
        let held = self.player().motion.flags.word3 & HOLDS_TIME != 0
            || members
                .into_iter()
                .flatten()
                .filter_map(|id| self.object(id))
                .any(|o| o.sys.motion.flags.word3 & HOLDS_TIME != 0);
        let dt = self.world.frame_time;
        let rate = self.time_rate;
        if self.gauge_shown
            && !held
            && let Some(l) = self.world.limit.as_mut()
        {
            l.time -= dt * rate;
        }
        let pending = self.limit().pending;
        if self.scene.finished() && pending <= 0 {
            if let Some(l) = self.world.limit.as_mut() {
                l.ending = true;
            }
            Status::Over
        } else {
            Status::Running
        }
    }

    pub fn take_events(&mut self) -> Vec<Event> {
        std::mem::take(&mut self.world.events)
    }

    /// Faults the program reported (program counter, fault).
    pub fn faults(&self) -> &[(i32, crate::script::Fault)] {
        &self.program.script.faults
    }

    /// Traps the scene called that have no translation.
    pub fn missing(&self) -> &[(u16, u16)] {
        &self.world.missing
    }
}
