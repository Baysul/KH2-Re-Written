//! pppRyjMegaBirth (type 201): a pooled sub-particle emitter
//! (update FUN_1404bc3b0, draw FUN_1404bbdb0, create FUN_1404bc5f0,
//! destroy FUN_1404bc670). Not yet translated: it records itself as
//! unsupported and draws nothing.
use crate::{Ctx, Part};
use package_reader::effect::Program;

#[derive(Debug, Default)]
pub(crate) struct Pool;

fn note(ctx: &mut Ctx) {
    if !ctx.unsupported.contains(&crate::programs::MEGA_BIRTH) {
        ctx.unsupported.push(crate::programs::MEGA_BIRTH);
    }
}

pub(crate) fn create(p: &Program, part: &mut Part, ctx: &mut Ctx) {
    part.mega.insert(p.use_val, Pool);
    note(ctx);
}

pub(crate) fn update(_p: &Program, _cursor: usize, _part: &mut Part, ctx: &mut Ctx) {
    note(ctx);
}

pub(crate) fn draw(_p: &Program, _cursor: usize, _part: &mut Part, ctx: &mut Ctx) {
    note(ctx);
}

pub(crate) fn destroy(_p: &Program, _part: &mut Part, _ctx: &mut Ctx) {}
