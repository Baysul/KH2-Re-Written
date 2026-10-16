//! Trap implementations, one module per group. A group module exports
//! `lookup(table, index) -> Option<TrapFn>`; [`lookup`] asks each in turn.
//! Add a trap by adding its arm to the owning group's `lookup` and writing the
//! function in that module, citing the executable address it translates.

use crate::world::TrapFn;

pub mod math;

pub fn lookup(table: u16, index: u16) -> Option<TrapFn> {
    math::lookup(table, index)
}
