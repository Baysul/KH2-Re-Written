//! Table 0 traps for vectors, numbers and the shared random generator.

use crate::script::{Fault, Memory, Slot};
use crate::world::{Ctx, TrapFn};

pub fn lookup(_table: u16, _index: u16) -> Option<TrapFn> {
    None
}

#[allow(dead_code)]
type Args = [Slot; 16];
#[allow(dead_code)]
fn _signature(_: &mut Ctx<'_>, _: &mut Memory<'_>, _: &mut Args) -> Result<(), Fault> {
    Ok(())
}
