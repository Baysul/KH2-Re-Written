//! The engine objects scripts act on, and the machinery that runs them.
//!
//! A [`World`] owns every script-driven object ([`Object`]) and the engine
//! state they share (random generator, frame time, the player). A trap runs
//! with a [`Ctx`]: the object whose script called it plus the world.

use crate::act;
use crate::script::{self, Exit, Fault, Host, Instance, Memory, Slot, Thread};
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
    pub fn next(&mut self) -> u32 {
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
}

/// The object's script instance and its threads.
pub struct ScriptSpace {
    pub instance: Instance,
    /// The persistent threads (OBJ_MAIN and friends), in creation order.
    pub threads: Vec<Thread>,
    /// The running act's thread.
    pub act_thread: Option<Thread>,
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
        }
    }
}

impl World {
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

pub fn object_handle(id: ObjId) -> u32 {
    OBJECT_HANDLES + id * 0x1000
}

pub fn object_from_handle(handle: u32) -> Option<ObjId> {
    let o = handle.checked_sub(OBJECT_HANDLES)?;
    (o % 0x1000 == 0 && o < 0x0100_0000).then_some(o / 0x1000)
}

/// What a trap sees: the calling object and the shared world.
pub struct Ctx<'a> {
    pub world: &'a mut World,
    pub me: &'a mut Object,
}

impl Ctx<'_> {
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

    fn peek(&self, address: u32) -> Option<u32> {
        self.ctx.world.static_read(address)
    }

    fn poke(&mut self, address: u32, value: u32) -> bool {
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
pub fn run_thread(world: &mut World, obj: &mut Object, thread: &mut Thread) -> Exit {
    let mut instance = std::mem::replace(&mut obj.script.instance, Instance::placeholder());
    let exit = {
        let mut machine = Machine {
            ctx: Ctx { world, me: obj },
            tolerate_missing: false,
        };
        script::run(&mut instance, thread, &mut machine, 1_000_000)
    };
    obj.script.instance = instance;
    exit
}
