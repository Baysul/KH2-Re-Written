//! The engine objects scripts act on, and the machinery that runs them.
//!
//! A [`World`] owns every script-driven object ([`Object`]) and the engine
//! state they share (random generator, frame time, the player). A trap runs
//! with a [`Ctx`]: the object whose script called it plus the world.

use crate::act;
use crate::env::Env;
use crate::script::{self, Callback, Exit, Fault, Host, Instance, Memory, Slot, Thread};
use crate::sysobj::Sysobj;
use std::collections::HashMap;

pub type ObjId = u32;

/// The scripts' shared random generator (DAT_1407535c0): an LCG with
/// multiplier 0x10dcd, initial value 1.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Lcg(pub u32);

impl Default for Lcg {
    fn default() -> Self {
        Lcg(1)
    }
}

impl Lcg {
    pub fn step(&mut self) -> u32 {
        self.0 = self.0.wrapping_mul(0x10dcd);
        self.0
    }
}

/// A script-driven object: engine body plus script.
pub struct Object {
    pub id: ObjId,
    pub sys: Sysobj,
    pub acts: act::Manager,
    pub script: ScriptSpace,
    /// Callbacks traps queued for the interpreter to run (see [`Host::take_callbacks`]).
    pub pending: Vec<Callback>,
}

impl Object {
    /// An object with an empty script (tests and tools).
    pub fn bare(id: ObjId) -> Self {
        Self {
            id,
            sys: Sysobj::bare(),
            acts: act::Manager::default(),
            pending: Vec::new(),
            script: ScriptSpace {
                instance: Instance::placeholder(),
                stack_bytes: 0,
                temp_bytes: 0,
                main: None,
                main_ended: false,
                act_thread: None,
                act_generation: 0,
                faults: Vec::new(),
            },
        }
    }
}

/// The object's script instance and its threads.
pub struct ScriptSpace {
    pub instance: Instance,
    pub stack_bytes: u32,
    pub temp_bytes: u32,
    /// The persistent main thread (OBJ_MAIN) and whether it has ended.
    pub main: Option<Thread>,
    pub main_ended: bool,
    /// The running act thread.
    pub act_thread: Option<Thread>,
    /// Counts act thread replacements, so a run can tell its thread was
    /// discarded while it executed.
    pub act_generation: u32,
    /// Faults the interpreter reported (`pc` of the thread, fault), newest last.
    pub faults: Vec<(i32, Fault)>,
}

/// Side effects the presentation layer or other systems must carry out.
#[derive(Clone, Debug, PartialEq)]
pub enum Event {
    EffectStart {
        object: ObjId,
        effect: i32,
        arguments: [i32; 2],
        handle: u32,
        /// Where the script placed it (`obj_effect_start_pos`); otherwise it
        /// follows the object.
        position: Option<[f32; 3]>,
        /// Bound to the object (`obj_effect_start_bind`).
        bound: bool,
        /// The object it is bound to when that is not the caller.
        other: Option<ObjId>,
    },
    /// A presentation request this build does not play (lights, screen fades,
    /// camera shots, command overrides): the trap's name and its operand words.
    Presentation {
        name: &'static str,
        words: [u32; 3],
    },
    ScatterPrize {
        object: ObjId,
        count: i32,
    },
    /// A command on an effect handle: the first operand slots, and the vector
    /// the first one points at when it is readable.
    EffectCommand {
        handle: u32,
        words: [u32; 2],
        vector: Option<[f32; 3]>,
    },
    /// A motion event whose engine routine another subsystem owns (sound,
    /// effects, camera, physics modes).
    MotionEvent {
        object: ObjId,
        range: bool,
        kind: u8,
        params: Vec<u16>,
    },
    /// Frame event 1: an effect or sound with this id started.
    MotionEffect {
        object: ObjId,
        id: u16,
        handle: u32,
    },
    EffectDeadBlock {
        object: ObjId,
        handle: u32,
    },
    /// `magic_start_thread`: an enemy spell thread the magic system starts.
    MagicStart {
        object: ObjId,
        id: i32,
        arguments: [i32; 4],
        handle: u32,
    },
    /// `attack_strike`: a script applied an attack to a target directly.
    AttackStrike {
        attack: u32,
        target: u32,
        arguments: [i32; 2],
    },
    /// `obj_dead`: the object finished dying and is removed.
    ObjectDead {
        object: ObjId,
    },
    /// `obj_voice`: the object plays a voice line.
    Voice {
        object: ObjId,
        id: i32,
        arg: i32,
    },
    /// A service the executable provides that this build does not translate yet.
    Unsupported(&'static str),
}

/// Everything shared between objects.
pub struct World {
    /// `DAT_140717480`: 1.0 at 60 fps and normal speed.
    pub frame_time: f32,
    pub rng: Lcg,
    pub objects: Vec<Option<Box<Object>>>,
    /// Static result cells traps hand scripts addresses of (the executable's
    /// per-trap `static` buffers); allocated on first use.
    statics: HashMap<(u16, u16, u16), u32>,
    static_words: Vec<u32>,
    /// Traps a script called that nothing implements yet.
    pub missing: Vec<(u16, u16)>,
    pub events: Vec<Event>,
    /// Keep running when a script calls a trap with no translation (the call
    /// returns zero and is listed in `missing`). Development aid.
    pub tolerate_missing: bool,
    next_effect: u32,
    /// Copies of target records (`trap_target_dup`), by slot.
    pub target_dups: Vec<Option<crate::sysobj::Target>>,
    /// `teamwork_alloc` blocks by key (DAT_142ae75d0).
    pub teamwork: HashMap<i32, u32>,
    next_magic: u32,
    /// The stand-in traps reach when a script names Sora (see [`PLAYER_OBJECT`]).
    pub player_proxy: Option<Box<Object>>,
    /// The attack objects scripts created (`attack_new`), by slot.
    pub attacks: Vec<Option<crate::attack::Attack>>,
    /// `attack_new_combo_group`'s counter (DAT_142a24afc).
    pub combo_group: u32,
    /// `status_set_lockon_ratio`: the lock-on ratio scripts last set.
    pub lockon_ratio: f32,
}

impl Default for World {
    fn default() -> Self {
        Self {
            frame_time: 1.0,
            rng: Lcg::default(),
            objects: Vec::new(),
            statics: HashMap::new(),
            static_words: Vec::new(),
            missing: Vec::new(),
            events: Vec::new(),
            tolerate_missing: false,
            next_effect: 0,
            target_dups: Vec::new(),
            teamwork: HashMap::new(),
            next_magic: 0,
            player_proxy: Some(Box::new(Object::bare(PLAYER_OBJECT))),
            attacks: Vec::new(),
            combo_group: 0,
            lockon_ratio: 0.,
        }
    }
}

impl World {
    /// `trap_memory_alloc`: a zeroed block in the shared static area.
    pub fn alloc(&mut self, bytes: u32) -> u32 {
        let words = bytes.div_ceil(4).max(1) as usize;
        let at = self.static_words.len() as u32 * 4;
        self.static_words.extend(std::iter::repeat_n(0, words));
        script::EXTERN_SEGMENT + STATIC_BASE + at
    }

    /// A fresh handle for an effect the host will create.
    pub fn next_effect_handle(&mut self) -> u32 {
        self.next_effect += 1;
        EFFECT_HANDLES + self.next_effect * 16
    }

    /// A fresh handle for a magic thread the host will create.
    pub fn next_magic_handle(&mut self) -> u32 {
        self.next_magic += 1;
        MAGIC_HANDLES + self.next_magic * 16
    }

    /// The static cell if some trap already allocated it.
    pub fn static_lookup(&self, table: u16, index: u16, slot: u16) -> Option<u32> {
        self.statics
            .get(&(table, index, slot))
            .map(|at| script::EXTERN_SEGMENT + STATIC_BASE + at)
    }

    /// The handle of a four-word static cell owned by `(table, index, slot)`;
    /// the first use allocates it zeroed.
    pub fn static_cell(&mut self, table: u16, index: u16, slot: u16) -> u32 {
        let words = &mut self.static_words;
        let at = *self.statics.entry((table, index, slot)).or_insert_with(|| {
            let at = words.len() as u32 * 4;
            words.extend([0; 4]);
            at
        });
        script::EXTERN_SEGMENT + STATIC_BASE + at
    }

    pub fn static_read(&self, handle: u32) -> Option<u32> {
        let at = handle.checked_sub(script::EXTERN_SEGMENT + STATIC_BASE)?;
        self.static_words.get((at / 4) as usize).copied()
    }

    pub fn static_write(&mut self, handle: u32, value: u32) -> bool {
        let Some(at) = handle.checked_sub(script::EXTERN_SEGMENT + STATIC_BASE) else {
            return false;
        };
        match self.static_words.get_mut((at / 4) as usize) {
            Some(w) => {
                *w = value;
                true
            }
            None => false,
        }
    }
}

/// Engine handles: statics at 0x6000_0000, objects at 0x6100_0000 + id.
const STATIC_BASE: u32 = 0;
pub const OBJECT_HANDLES: u32 = script::EXTERN_SEGMENT + 0x0100_0000;

/// Sora's handle (the player is not one of the script-driven objects).
pub const PLAYER_HANDLE: u32 = script::EXTERN_SEGMENT + 0x00f0_0000;
/// Effect handles scripts keep, and copies of target records.
pub const EFFECT_HANDLES: u32 = script::EXTERN_SEGMENT + 0x0200_0000;
pub const ATTACK_HANDLES: u32 = script::EXTERN_SEGMENT + 0x0700_0000;
pub const MAGIC_HANDLES: u32 = script::EXTERN_SEGMENT + 0x0500_0000;
pub const TARGET_DUP_HANDLES: u32 = script::EXTERN_SEGMENT + 0x0300_0000;
/// The engine object's target record (sys+0xbf8) is this far into its handle range.
pub const TARGET_RECORD_OFFSET: u32 = 0x800;

/// The id the player proxy answers to. Sora is not a script-driven object, but
/// scripts call the same traps on her as on any object; those reach a stand-in
/// whose position and heading follow the host's player.
pub const PLAYER_OBJECT: ObjId = 0x00ff_ffff;

pub fn object_handle(id: ObjId) -> u32 {
    if id == PLAYER_OBJECT {
        return PLAYER_HANDLE;
    }
    OBJECT_HANDLES + id * 0x1000
}

/// A word inside an object's handle range: the object and its field offset.
pub fn object_field(address: u32) -> Option<(ObjId, u32)> {
    let o = address.checked_sub(OBJECT_HANDLES)?;
    (o < 0x0100_0000).then_some((o / 0x1000, o % 0x1000))
}

pub fn object_from_handle(handle: u32) -> Option<ObjId> {
    let o = handle.checked_sub(OBJECT_HANDLES)?;
    (o % 0x1000 == 0 && o < 0x0100_0000).then_some(o / 0x1000)
}

/// A word of a target record (sys+0xbf8): +0 and +8 hold the target's engine
/// handle (0 = nobody), +4 is zero and +0xc the search kind.
fn target_record_word(t: crate::sysobj::Target, offset: u32) -> u32 {
    match offset {
        0 | 8 => match t.id {
            0 => 0,
            crate::traps::target::PLAYER_TARGET => PLAYER_HANDLE,
            id => object_handle(id),
        },
        0xc => t.kind as u32,
        _ => 0,
    }
}

/// What a trap sees: the calling object and the shared world.
pub struct Ctx<'a> {
    pub world: &'a mut World,
    pub me: &'a mut Object,
    pub env: &'a dyn Env,
}

impl Ctx<'_> {
    /// The object with this id: the calling object or another in the world.
    pub fn object(&mut self, id: ObjId) -> Result<&mut Object, Fault> {
        if id == self.me.id {
            return Ok(self.me);
        }
        if id == PLAYER_OBJECT {
            let proxy = self
                .world
                .player_proxy
                .get_or_insert_with(|| Box::new(Object::bare(PLAYER_OBJECT)));
            proxy.sys.position = self.env.player_position();
            proxy.sys.yaw = self.env.player_yaw();
            return Ok(proxy);
        }
        self.world
            .objects
            .get_mut(id as usize)
            .and_then(|o| o.as_deref_mut())
            .ok_or(Fault::Memory(object_handle(id)))
    }

    /// An engine-object argument (the trap received the handle itself).
    pub fn sys_of(&self, handle: u32) -> Result<ObjId, Fault> {
        if handle == PLAYER_HANDLE {
            return Ok(PLAYER_OBJECT);
        }
        object_from_handle(handle).ok_or(Fault::Memory(handle))
    }

    /// An AI-object argument that may name Sora: the word at +4 is either an
    /// engine object or the player.
    pub fn sys_ref_ai(&self, memory: &Memory<'_>, ai: u32) -> Result<SysRef, Fault> {
        let handle = self.read_u32(memory, ai + 4).ok_or(Fault::Memory(ai))?;
        if handle == PLAYER_HANDLE {
            return Ok(SysRef::Player);
        }
        self.sys_of(handle).map(SysRef::Object)
    }

    /// An AI-object argument: the engine object handle is the word at +4.
    pub fn sys_of_ai(&self, memory: &Memory<'_>, ai: u32) -> Result<ObjId, Fault> {
        let handle = self.read_u32(memory, ai + 4).ok_or(Fault::Memory(ai))?;
        self.sys_of(handle)
    }

    /// Read a 32-bit cell a script handle names: script memory first, then the
    /// static result cells.
    pub fn read_u32(&self, memory: &Memory<'_>, handle: u32) -> Option<u32> {
        memory
            .read_u32(handle)
            .or_else(|| self.world.static_read(handle))
    }

    pub fn read_f32(&self, memory: &Memory<'_>, handle: u32) -> Option<f32> {
        self.read_u32(memory, handle).map(f32::from_bits)
    }

    pub fn write_u32(&mut self, memory: &mut Memory<'_>, handle: u32, value: u32) -> bool {
        memory.write_u32(handle, value) || self.world.static_write(handle, value)
    }

    pub fn write_f32(&mut self, memory: &mut Memory<'_>, handle: u32, value: f32) -> bool {
        self.write_u32(memory, handle, value.to_bits())
    }
}

/// What a script-held object reference names.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SysRef {
    Player,
    Object(ObjId),
}

pub type TrapFn = fn(&mut Ctx<'_>, &mut Memory<'_>, &mut [Slot; 16]) -> Result<(), Fault>;

/// The script [`Host`] for one object's thread: resolves traps through the
/// registry in [`crate::traps`].
pub struct Machine<'a> {
    pub ctx: Ctx<'a>,
    /// Keep going when a trap has no implementation (development).
    pub tolerate_missing: bool,
}

impl Host for Machine<'_> {
    fn frame_time(&self) -> f32 {
        self.ctx.world.frame_time
    }

    fn take_callbacks(&mut self) -> Vec<Callback> {
        std::mem::take(&mut self.ctx.me.pending)
    }

    fn patch_gate(&self) -> f32 {
        self.ctx
            .world
            .static_lookup(1, 0, 1)
            .and_then(|cell| self.ctx.world.static_read(cell + 12))
            .map_or(0.0, f32::from_bits)
    }

    fn peek(&self, address: u32) -> Option<u32> {
        if let Some(offset) = address.checked_sub(PLAYER_HANDLE).filter(|o| *o < 0x1000) {
            return self.ctx.world.player_proxy.as_ref()?.sys.peek_word(offset);
        }
        if let Some((id, offset)) = object_field(address) {
            if (TARGET_RECORD_OFFSET..TARGET_RECORD_OFFSET + 16).contains(&offset) {
                let sys = if id == self.ctx.me.id {
                    &self.ctx.me.sys
                } else {
                    &self.ctx.world.objects.get(id as usize)?.as_deref()?.sys
                };
                return Some(target_record_word(
                    sys.target,
                    offset - TARGET_RECORD_OFFSET,
                ));
            }
            if id == self.ctx.me.id {
                return self.ctx.me.sys.peek_word(offset);
            }
            return self
                .ctx
                .world
                .objects
                .get(id as usize)?
                .as_deref()?
                .sys
                .peek_word(offset);
        }
        self.ctx.world.static_read(address)
    }

    fn poke(&mut self, address: u32, value: u32) -> bool {
        if let Some(offset) = address.checked_sub(PLAYER_HANDLE).filter(|o| *o < 0x1000) {
            return self
                .ctx
                .world
                .player_proxy
                .as_mut()
                .is_some_and(|p| p.sys.poke_word(offset, value));
        }
        if let Some((id, offset)) = object_field(address) {
            if id == self.ctx.me.id {
                return self.ctx.me.sys.poke_word(offset, value);
            }
            return match self.ctx.world.objects.get_mut(id as usize) {
                Some(Some(o)) => o.sys.poke_word(offset, value),
                _ => false,
            };
        }
        self.ctx.world.static_write(address, value)
    }

    fn trap_flags(&self, table: u16, index: u16) -> Option<u32> {
        crate::trap_table::lookup(table, index).map(|t| t.flags)
    }

    fn trap(
        &mut self,
        memory: &mut Memory<'_>,
        table: u16,
        index: u16,
        args: &mut [Slot; 16],
    ) -> Result<(), Fault> {
        match crate::traps::lookup(table, index) {
            Some(f) => f(&mut self.ctx, memory, args),
            None => {
                self.ctx.world.missing.push((table, index));
                if self.tolerate_missing {
                    // Arguments are consumed; a returned value reads as zero.
                    args[0] = Slot::int(0);
                    Ok(())
                } else {
                    Err(Fault::Trap { table, index })
                }
            }
        }
    }
}

/// Run one thread of `obj`'s script to its next yield.
pub fn run_thread(world: &mut World, env: &dyn Env, obj: &mut Object, thread: &mut Thread) -> Exit {
    let mut instance = std::mem::replace(&mut obj.script.instance, Instance::placeholder());
    let world_tolerates = world.tolerate_missing;
    let exit = {
        let mut machine = Machine {
            ctx: Ctx {
                world,
                me: obj,
                env,
            },
            tolerate_missing: world_tolerates,
        };
        script::tick_tree(&mut instance, thread, &mut machine, 1_000_000)
    };
    obj.script.instance = instance;
    if let Exit::Fault(f) = exit {
        obj.script.faults.push((thread.pc, f));
    }
    exit
}
