//! Small state traps: setters and getters on fields of the engine object that
//! scripts use directly, and the services recorded as unsupported.

use crate::script::{Fault, Memory, Slot};
use crate::world::{Ctx, Event, TrapFn, object_handle};

type Args = [Slot; 16];

pub fn lookup(table: u16, index: u16) -> Option<TrapFn> {
    Some(match (table, index) {
        (1, 38) => set_team,
        (1, 53) => disable_collision,
        (1, 54) => reset_collision,
        (1, 97) => spec,
        (1, 109) => hide_part,
        (1, 114) => search_by_entry,
        (1, 177) => set_path,
        (1, 222) => get_entry_id,
        (2, 20) => magic_start_thread,
        (2, 21) => teamwork_alloc,
        (2, 76) => set_karma_limit,
        _ => return None,
    })
}

/// 1:38 FUN_14042bd30: +0x4dc.
fn set_team(ctx: &mut Ctx<'_>, m: &mut Memory<'_>, args: &mut Args) -> Result<(), Fault> {
    let id = ctx.sys_of_ai(m, args[0].value)?;
    ctx.object(id)?.sys.team = args[1].as_int();
    Ok(())
}

/// 1:53 FUN_14042ede0 -> FUN_1403f1f80.
fn disable_collision(ctx: &mut Ctx<'_>, m: &mut Memory<'_>, args: &mut Args) -> Result<(), Fault> {
    let id = ctx.sys_of_ai(m, args[0].value)?;
    ctx.object(id)?.sys.disable_collision(args[1].value);
    Ok(())
}

/// 1:54 FUN_14042ee10 -> FUN_1403f2000.
fn reset_collision(ctx: &mut Ctx<'_>, m: &mut Memory<'_>, args: &mut Args) -> Result<(), Fault> {
    let id = ctx.sys_of_ai(m, args[0].value)?;
    ctx.object(id)?.sys.reset_collision(args[1].value);
    Ok(())
}

/// 1:97 FUN_14042faa0: the object's spec block (+0x128). Installed scripts
/// only write its word 12 (+0x134, the turn rate).
fn spec(ctx: &mut Ctx<'_>, m: &mut Memory<'_>, args: &mut Args) -> Result<(), Fault> {
    let id = ctx.sys_of_ai(m, args[0].value)?;
    args[0] = Slot::address(object_handle(id) + 0x128);
    Ok(())
}

/// 1:109 FUN_14042fd40 -> FUN_1403cc9b0 (the object at +0x7b0). [U] Recorded
/// only.
fn hide_part(ctx: &mut Ctx<'_>, _: &mut Memory<'_>, _: &mut Args) -> Result<(), Fault> {
    ctx.world.events.push(Event::Unsupported("obj_hide_part"));
    Ok(())
}

/// 1:114 FUN_14042ddb0 -> FUN_1403b4110: bind the AI object to the engine
/// object found by entry id. [U] Recorded only; the binding is left alone.
fn search_by_entry(ctx: &mut Ctx<'_>, _: &mut Memory<'_>, _: &mut Args) -> Result<(), Fault> {
    ctx.world
        .events
        .push(Event::Unsupported("obj_search_by_entry"));
    Ok(())
}

/// 1:177 FUN_14042d8e0 -> FUN_1403b9de0: bind a path to the object. [U]
/// Recorded; reports that nothing was bound.
fn set_path(ctx: &mut Ctx<'_>, _: &mut Memory<'_>, args: &mut Args) -> Result<(), Fault> {
    ctx.world.events.push(Event::Unsupported("obj_set_path"));
    args[0] = Slot::int(0);
    Ok(())
}

/// 1:222 FUN_14042bb10 -> FUN_1403b5770: the first word of the object entry.
fn get_entry_id(ctx: &mut Ctx<'_>, m: &mut Memory<'_>, args: &mut Args) -> Result<(), Fault> {
    let id = ctx.sys_of_ai(m, args[0].value)?;
    let entry = ctx.object(id)?.sys.entry_id;
    args[0] = Slot::int(entry as i32);
    Ok(())
}

/// 2:76 FUN_140431e30: +0xd4c.
fn set_karma_limit(ctx: &mut Ctx<'_>, m: &mut Memory<'_>, args: &mut Args) -> Result<(), Fault> {
    let id = ctx.sys_of_ai(m, args[0].value)?;
    ctx.object(id)?.sys.karma_limit = args[1].as_int();
    Ok(())
}

/// 2:20 FUN_140432cd0 -> FUN_1403c5ec0: start a magic thread. [U] The magic
/// system is not translated; the start is recorded and the script gets a handle.
fn magic_start_thread(ctx: &mut Ctx<'_>, _: &mut Memory<'_>, args: &mut Args) -> Result<(), Fault> {
    let handle = ctx.world.next_magic_handle();
    let object = ctx.me.id;
    ctx.world.events.push(Event::MagicStart {
        object,
        id: args[0].as_int(),
        arguments: [
            args[1].as_int(),
            args[2].as_int(),
            args[3].as_int(),
            args[4].as_int(),
        ],
        handle,
    });
    args[0] = Slot::address(handle);
    Ok(())
}

/// 2:21 FUN_140432d70 -> FUN_1403f6df0: the zeroed block shared under a key.
/// A second request for the key sets the block's first byte.
fn teamwork_alloc(ctx: &mut Ctx<'_>, m: &mut Memory<'_>, args: &mut Args) -> Result<(), Fault> {
    let (key, size) = (args[0].as_int(), args[1].as_int().max(0) as u32);
    let handle = match ctx.world.teamwork.get(&key).copied() {
        Some(h) => {
            let first = ctx.read_u32(m, h).unwrap_or(0);
            ctx.write_u32(m, h, first & !0xff | 1);
            h
        }
        None => {
            let h = ctx.world.alloc(size + 1);
            ctx.world.teamwork.insert(key, h);
            h
        }
    };
    args[0] = Slot::address(handle);
    Ok(())
}

#[cfg(test)]
mod tests;
