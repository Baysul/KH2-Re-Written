//! Object lifecycle and the per-tick order of an enemy: creation and its
//! script triggers (FUN_1403b44d0), the script tick (FUN_1403b4460: the main
//! thread, then the act manager when the main thread has not ended) and the
//! act manager loop (FUN_1403ca550).

use crate::act::{self, Action};
use crate::env::Env;
use crate::script::{Callback, Instance, Slot, Thread};
use crate::sysobj::Sysobj;
use crate::world::{ObjId, Object, ScriptSpace, World, object_handle, run_thread};

/// Script trigger keys (OBJ_INIT, OBJ_MAIN, OBJ_DAMAGE, OBJ_FALL ...).
pub const INIT: i32 = 0;
pub const MAIN: i32 = 1;
pub const DAMAGE: i32 = 6;
pub const FALL: i32 = 13;
pub const LAND: i32 = 14;
pub const ATTACK: i32 = 15;

/// Frame slots of the object's threads (see [`crate::script`]).
const MAIN_SLOT: u32 = 0;
const ACT_SLOT: u32 = 1;
const EVENT_SLOT: u32 = 2;

#[derive(Debug)]
pub enum SpawnError {
    Program,
}

impl Object {
    /// FUN_1403b44d0: build the object around a script. OBJ_INIT runs to its
    /// first yield on a throw-away thread with the object as its argument; then
    /// the persistent OBJ_MAIN thread is created.
    pub fn spawn(
        world: &mut World,
        env: &dyn Env,
        id: ObjId,
        payload: &[u8],
        sys: Sysobj,
    ) -> Result<Object, SpawnError> {
        let instance = Instance::new(id, payload).ok_or(SpawnError::Program)?;
        let init = instance.trigger(INIT);
        let main = instance.trigger(MAIN);
        let (stack_bytes, temp_bytes) = (instance.stack_bytes, instance.temp_bytes);
        let mut obj = Object::bare(id);
        obj.sys = sys;
        obj.script = ScriptSpace {
            instance,
            stack_bytes,
            temp_bytes,
            main: None,
            main_ended: false,
            act_thread: None,
            act_generation: 0,
            faults: Vec::new(),
        };
        if init > 0 {
            let mut t = Thread::with_sizes(
                stack_bytes,
                temp_bytes,
                EVENT_SLOT,
                init,
                &[Slot::address(object_handle(id))],
            );
            run_thread(world, env, &mut obj, &mut t);
        }
        if main > 0 {
            obj.script.main = Some(Thread::with_sizes(
                stack_bytes,
                temp_bytes,
                MAIN_SLOT,
                main,
                &[],
            ));
        } else {
            obj.script.main_ended = true;
        }
        Ok(obj)
    }

    /// FUN_1403e1840: fire a trigger on a throw-away thread, run once.
    pub fn fire(&mut self, world: &mut World, env: &dyn Env, key: i32, args: &[Slot]) {
        let entry = self.script.instance.trigger(key);
        if entry <= 0 {
            return;
        }
        let mut t = Thread::with_sizes(
            self.script.stack_bytes,
            self.script.temp_bytes,
            EVENT_SLOT,
            entry,
            args,
        );
        run_thread(world, env, self, &mut t);
    }

    /// Turn the act manager's requests into thread changes. Inside the
    /// interpreter callbacks are queued (it runs them after the trap); outside
    /// they run now.
    pub fn apply_actions(&mut self, world: Option<(&mut World, &dyn Env)>, actions: Vec<Action>) {
        let mut later = Vec::new();
        for action in actions {
            match action {
                Action::Kill => {
                    self.script.act_thread = None;
                    self.script.act_generation = self.script.act_generation.wrapping_add(1);
                }
                Action::Spawn { entry } => {
                    self.script.act_thread = Some(Thread::with_sizes(
                        self.script.stack_bytes,
                        self.script.temp_bytes,
                        ACT_SLOT,
                        i32::from(entry),
                        &[Slot::address(self.acts.ai_object)],
                    ));
                    self.script.act_generation = self.script.act_generation.wrapping_add(1);
                }
                Action::Callback { word, .. } => later.push(Callback {
                    entry: word,
                    args: vec![Slot::address(self.acts.ai_object)],
                }),
            }
        }
        match world {
            None => self.pending.extend(later),
            Some((world, env)) => {
                for cb in later {
                    let mut t = Thread::with_sizes(
                        self.script.stack_bytes,
                        self.script.temp_bytes,
                        EVENT_SLOT,
                        i32::from(cb.entry),
                        &cb.args,
                    );
                    run_thread(world, env, self, &mut t);
                }
            }
        }
    }

    /// One frame of this object's script (FUN_1403b4460).
    pub fn tick_script(&mut self, world: &mut World, env: &dyn Env) {
        let Some(mut main) = self.script.main.take() else {
            return;
        };
        run_thread(world, env, self, &mut main);
        if main.finished() {
            self.script.main_ended = true;
        } else {
            self.script.main = Some(main);
            self.tick_acts(world, env);
        }
    }

    /// FUN_1403ca550.
    fn tick_acts(&mut self, world: &mut World, env: &dyn Env) {
        if !self.acts.has_table {
            return;
        }
        self.acts.flags |= act::JUST_STARTED;
        let mut highest = 0u16;
        loop {
            if !self.acts.locked() && self.sys.act_paused {
                return;
            }
            let mut actions = Vec::new();
            let started = self.acts.pop(&mut actions);
            self.apply_actions(Some((world, env)), actions);
            if !started && self.acts.flags & act::JUST_STARTED == 0 {
                return;
            }
            let flags = self.acts.flags;
            self.acts.flags &= !act::JUST_STARTED;
            if self.script.act_thread.is_none() {
                continue;
            }
            if flags & act::GATED != 0 {
                if self.sys.air_blocked() || self.sys.no_control(env) {
                    return;
                }
                self.acts.flags &= !act::GATED;
            }
            highest = highest.max(self.acts.priority);
            if highest < 300 && self.sys.hit_stunned() {
                return;
            }
            let generation = self.script.act_generation;
            let mut thread = self.script.act_thread.take().expect("act thread");
            run_thread(world, env, self, &mut thread);
            let replaced = generation != self.script.act_generation;
            let ended = thread.finished();
            if !replaced && !ended {
                self.script.act_thread = Some(thread);
            }
            if ended && self.acts.flags & act::JUST_STARTED == 0 {
                let mut actions = Vec::new();
                self.acts.thread_ended(&mut actions);
                self.apply_actions(Some((world, env)), actions);
            }
        }
    }

    /// One frame: the script, then the engine's motion and physics.
    pub fn tick(&mut self, world: &mut World, env: &dyn Env) {
        self.tick_script(world, env);
        let no_control = self.sys.no_control(env);
        self.sys.tick(env, no_control);
    }
}
