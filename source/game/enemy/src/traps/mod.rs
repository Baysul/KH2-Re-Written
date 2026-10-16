//! Trap implementations, one module per group. A group module exports
//! `lookup(table, index) -> Option<TrapFn>`; [`lookup`] asks each in turn.
//! Add a trap by adding its arm to the owning group's `lookup` and writing the
//! function in that module, citing the executable address it translates.

use crate::world::TrapFn;

pub mod math;
pub mod object;
pub mod sheet;
pub mod state;
pub mod system;
pub mod target;

pub fn lookup(table: u16, index: u16) -> Option<TrapFn> {
    math::lookup(table, index)
        .or_else(|| object::lookup(table, index))
        .or_else(|| target::lookup(table, index))
        .or_else(|| system::lookup(table, index))
        .or_else(|| sheet::lookup(table, index))
        .or_else(|| state::lookup(table, index))
}
