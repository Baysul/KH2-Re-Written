//! Small state traps: setters and getters on fields of the engine object that
//! scripts use directly, and the services recorded as unsupported.

use super::math::read_sanitized;
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
        (1, 18) => obj_attach,
        (1, 59) => is_fly,
        (1, 99) => float_height,
        (1, 100) => jump_height_to_uptime,
        (1, 107) => motion_speed,
        (1, 112) => obj_hook,
        (1, 140) => target_is_exist,
        (1, 179) => set_fall_motion,
        (1, 180) => set_land_motion,
        (1, 209) => obj_dead,
        (2, 45) => damage_attack_param_id,
        (1, 75) => motion_hook,
        (1, 77) => motion_is_hook,
        (1, 85) | (1, 86) => texanm,
        (1, 101) => motion_is_capture,
        (1, 108) => show_part,
        (1, 143) | (1, 210) => search_object,
        (1, 145) => touch_zone,
        (1, 148) => set_pos,
        (1, 161) => is_attach,
        (1, 162) => target_set_before_player,
        (1, 181) => light_create,
        (1, 182) | (1, 183) => light_set,
        (1, 213) => obj_part,
        (1, 218) => set_unit_arg,
        (1, 247) => bg_random_pos,
        (1, 250) => set_lockon_ratio,
        (1, 262) => voice,
        (4, 4) => mission_information,
        (4, 12) => mission_gauge_ratio,
        (5, 17) => check_rtn_flag,
        (5, 27) => set_rtn_flag,
        (5, 33) => set_rtn_time,
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

/// 1:59 FUN_14042ef50.
fn is_fly(ctx: &mut Ctx<'_>, m: &mut Memory<'_>, args: &mut Args) -> Result<(), Fault> {
    let id = ctx.sys_of_ai(m, args[0].value)?;
    let fly = ctx.object(id)?.sys.is_fly();
    args[0] = Slot::int(fly as i32);
    Ok(())
}

/// 1:75 FUN_14042f460 -> FUN_1403c7c10: look up the playing clip's hook. [U]
/// Recorded; no clip has a hook here.
fn motion_hook(ctx: &mut Ctx<'_>, _: &mut Memory<'_>, _: &mut Args) -> Result<(), Fault> {
    ctx.world.events.push(Event::Unsupported("obj_motion_hook"));
    Ok(())
}

/// 1:77 FUN_14042f4c0: a hook is set (+0x160). [U] Always 0.
fn motion_is_hook(_: &mut Ctx<'_>, _: &mut Memory<'_>, args: &mut Args) -> Result<(), Fault> {
    args[0] = Slot::int(0);
    Ok(())
}

/// 1:85 and 1:86 FUN_14042f820 / FUN_14042f850: texture animation start and
/// stop. [U] Recorded.
fn texanm(ctx: &mut Ctx<'_>, _: &mut Memory<'_>, _: &mut Args) -> Result<(), Fault> {
    ctx.world.events.push(Event::Unsupported("obj_texanm"));
    Ok(())
}

/// 1:101 FUN_14042d360: +0x18c bit 14 (a captured motion plays).
fn motion_is_capture(ctx: &mut Ctx<'_>, m: &mut Memory<'_>, args: &mut Args) -> Result<(), Fault> {
    let id = ctx.sys_of_ai(m, args[0].value)?;
    let bit = ctx.object(id)?.sys.motion.flags.word >> 14 & 1;
    args[0] = Slot::int(bit as i32);
    Ok(())
}

/// 1:108 FUN_14042fd10 -> FUN_1403cd810: show a model part. [U] Recorded.
fn show_part(ctx: &mut Ctx<'_>, _: &mut Memory<'_>, _: &mut Args) -> Result<(), Fault> {
    ctx.world.events.push(Event::Unsupported("obj_show_part"));
    Ok(())
}

/// 1:143 and 1:210 FUN_14042ddf0 / FUN_14042de30: bind the AI object to the
/// engine object with a serial or part id. [U] Recorded; the binding is kept.
fn search_object(ctx: &mut Ctx<'_>, _: &mut Memory<'_>, _: &mut Args) -> Result<(), Fault> {
    ctx.world.events.push(Event::Unsupported("obj_search"));
    Ok(())
}

/// 1:145 FUN_14042b6f0: +0xcf4.
fn touch_zone(ctx: &mut Ctx<'_>, m: &mut Memory<'_>, args: &mut Args) -> Result<(), Fault> {
    let id = ctx.sys_of_ai(m, args[0].value)?;
    let v = ctx.object(id)?.sys.rtn_time[7];
    args[0] = Slot::int(v as i32);
    Ok(())
}

/// 1:148 FUN_14042b800 -> FUN_1403b64d0: place the object (w = 1).
fn set_pos(ctx: &mut Ctx<'_>, m: &mut Memory<'_>, args: &mut Args) -> Result<(), Fault> {
    let v = read_sanitized(ctx, m, args[1].value)?;
    let id = ctx.sys_of_ai(m, args[0].value)?;
    ctx.object(id)?.sys.position = [v[0], v[1], v[2]];
    Ok(())
}

/// 1:161 FUN_14042b980: an attached object exists (+0x6a0). [U] Always 0.
fn is_attach(_: &mut Ctx<'_>, _: &mut Memory<'_>, args: &mut Args) -> Result<(), Fault> {
    args[0] = Slot::int(0);
    Ok(())
}

/// 1:162 FUN_1404304b0 -> FUN_1403bd410. [U] Recorded.
fn target_set_before_player(
    ctx: &mut Ctx<'_>,
    _: &mut Memory<'_>,
    _: &mut Args,
) -> Result<(), Fault> {
    ctx.world
        .events
        .push(Event::Unsupported("target_set_before_player"));
    Ok(())
}

/// 1:181 FUN_140430950: create a light. [U] Recorded; a placeholder handle.
fn light_create(ctx: &mut Ctx<'_>, _: &mut Memory<'_>, args: &mut Args) -> Result<(), Fault> {
    ctx.world.events.push(Event::Unsupported("light_create"));
    let handle = ctx.world.next_magic_handle();
    args[0] = Slot::address(handle);
    Ok(())
}

/// 1:182 and 1:183: light flag and colour. [U] Recorded.
fn light_set(ctx: &mut Ctx<'_>, _: &mut Memory<'_>, _: &mut Args) -> Result<(), Fault> {
    ctx.world.events.push(Event::Unsupported("light_set"));
    Ok(())
}

/// 1:213 FUN_14042bae0 -> FUN_1403b5b20: the entry's part id (+0x4c).
fn obj_part(ctx: &mut Ctx<'_>, m: &mut Memory<'_>, args: &mut Args) -> Result<(), Fault> {
    let id = ctx.sys_of_ai(m, args[0].value)?;
    let part = ctx.object(id)?.sys.entry_part;
    args[0] = Slot::int(i32::from(part));
    Ok(())
}

/// 1:218 FUN_14042db40 -> FUN_1403b65b0: spawn argument `index` (+0x9f4).
fn set_unit_arg(ctx: &mut Ctx<'_>, m: &mut Memory<'_>, args: &mut Args) -> Result<(), Fault> {
    let id = ctx.sys_of_ai(m, args[0].value)?;
    let (index, value) = (args[1].as_int(), args[2].as_int());
    if let Some(slot) = usize::try_from(index)
        .ok()
        .and_then(|i| ctx.object(id).ok()?.sys.spawn_args.get_mut(i))
    {
        *slot = value;
    }
    Ok(())
}

/// 1:247 FUN_14042ead0 -> FUN_1403f3fa0: a random point in the map. [U] The
/// map is not available to the engine; the shared vector reads zero.
fn bg_random_pos(ctx: &mut Ctx<'_>, _: &mut Memory<'_>, args: &mut Args) -> Result<(), Fault> {
    ctx.world
        .events
        .push(Event::Unsupported("bg_get_random_pos"));
    let cell = ctx.world.static_cell(1, 247, 0);
    args[0] = Slot::address(cell);
    Ok(())
}

/// 1:250 FUN_1404308d0: the lock-on ratio.
fn set_lockon_ratio(ctx: &mut Ctx<'_>, _: &mut Memory<'_>, args: &mut Args) -> Result<(), Fault> {
    ctx.world.lockon_ratio = args[0].as_float();
    Ok(())
}

/// 1:262 FUN_14042bde0: the object's voice (virtual call 0xa0 on its class).
fn voice(ctx: &mut Ctx<'_>, m: &mut Memory<'_>, args: &mut Args) -> Result<(), Fault> {
    let object = ctx.sys_of_ai(m, args[0].value)?;
    ctx.world.events.push(Event::Voice {
        object,
        id: args[1].as_int(),
        arg: args[2].as_int(),
    });
    Ok(())
}

/// 4:4 FUN_1404334b0: mission information on the HUD. [U] Recorded.
fn mission_information(ctx: &mut Ctx<'_>, _: &mut Memory<'_>, _: &mut Args) -> Result<(), Fault> {
    ctx.world
        .events
        .push(Event::Unsupported("mission_information"));
    Ok(())
}

/// 4:12 FUN_140433770: the mission gauge ratio. [U] Reads 0.
fn mission_gauge_ratio(
    ctx: &mut Ctx<'_>,
    _: &mut Memory<'_>,
    args: &mut Args,
) -> Result<(), Fault> {
    ctx.world
        .events
        .push(Event::Unsupported("mission_get_gauge_ratio"));
    args[0] = Slot::float(0.);
    Ok(())
}

/// 5:17 FUN_140557ed0 -> FUN_140417630.
fn check_rtn_flag(ctx: &mut Ctx<'_>, m: &mut Memory<'_>, args: &mut Args) -> Result<(), Fault> {
    let id = ctx.sys_of_ai(m, args[0].value)?;
    let set = ctx.object(id)?.sys.rtn_flag(args[1].value);
    args[0] = Slot::int(set as i32);
    Ok(())
}

/// 5:27 FUN_140557f10 -> FUN_140418800.
fn set_rtn_flag(ctx: &mut Ctx<'_>, m: &mut Memory<'_>, args: &mut Args) -> Result<(), Fault> {
    let id = ctx.sys_of_ai(m, args[0].value)?;
    ctx.object(id)?.sys.set_rtn_flag(args[1].value);
    Ok(())
}

/// 5:33 FUN_140557f40 -> FUN_140418820: copy nine words of time parameters.
fn set_rtn_time(ctx: &mut Ctx<'_>, m: &mut Memory<'_>, args: &mut Args) -> Result<(), Fault> {
    let id = ctx.sys_of_ai(m, args[0].value)?;
    let mut words = [0u32; 9];
    for (k, w) in words.iter_mut().enumerate() {
        *w = ctx
            .read_u32(m, args[1].value + 4 * k as u32)
            .ok_or(Fault::Memory(args[1].value))?;
    }
    ctx.object(id)?.sys.rtn_time = words;
    Ok(())
}

/// 1:99 FUN_14042fb50 -> FUN_1403b87e0: minus the depth of the object below
/// the ground at its position (0 above it).
fn float_height(ctx: &mut Ctx<'_>, m: &mut Memory<'_>, args: &mut Args) -> Result<(), Fault> {
    let id = ctx.sys_of_ai(m, args[0].value)?;
    let p = ctx.object(id)?.sys.position;
    let floor = ctx.env.floor(p[0], p[2], p[1]).unwrap_or(p[1]);
    let below = (floor - p[1]).max(0.);
    args[0] = Slot::float(-below);
    Ok(())
}

/// 1:100 FUN_14042fbc0 -> FUN_1403b73f0: the time a jump of this apex height
/// takes to reach it, sqrt(|2h / g|).
fn jump_height_to_uptime(
    _: &mut Ctx<'_>,
    _: &mut Memory<'_>,
    args: &mut Args,
) -> Result<(), Fault> {
    let h = args[0].as_float();
    args[0] = Slot::float(((h + h) / player::actor_physics::G).abs().sqrt());
    Ok(())
}

/// 1:107 FUN_14042fce0: +0x2a8.
fn motion_speed(ctx: &mut Ctx<'_>, m: &mut Memory<'_>, args: &mut Args) -> Result<(), Fault> {
    let id = ctx.sys_of_ai(m, args[0].value)?;
    ctx.object(id)?.sys.motion_speed = args[1].as_float();
    Ok(())
}

/// 1:179 and 1:180 FUN_14042ce00 / FUN_14042ce30: the fall and land motions
/// of the current jump (+0x118, +0x11c).
fn set_fall_motion(ctx: &mut Ctx<'_>, m: &mut Memory<'_>, args: &mut Args) -> Result<(), Fault> {
    let id = ctx.sys_of_ai(m, args[0].value)?;
    let slot = args[1].as_int().max(0) as usize;
    let motions = ctx.object(id)?.sys.jump_motions.get_or_insert((0, 0));
    motions.0 = slot;
    Ok(())
}

fn set_land_motion(ctx: &mut Ctx<'_>, m: &mut Memory<'_>, args: &mut Args) -> Result<(), Fault> {
    let id = ctx.sys_of_ai(m, args[0].value)?;
    let slot = args[1].as_int().max(0) as usize;
    let motions = ctx.object(id)?.sys.jump_motions.get_or_insert((0, 0));
    motions.1 = slot;
    Ok(())
}

/// 1:112 FUN_14042ca20: start a magic thread bound to the object as a hook.
/// [U] Recorded; the script gets nothing.
fn obj_hook(ctx: &mut Ctx<'_>, _: &mut Memory<'_>, _: &mut Args) -> Result<(), Fault> {
    ctx.world.events.push(Event::Unsupported("obj_hook"));
    Ok(())
}

/// 1:18 FUN_14042c550 -> FUN_1403d9fe0: attach one object to another (a grab).
/// [U] Recorded.
fn obj_attach(ctx: &mut Ctx<'_>, _: &mut Memory<'_>, _: &mut Args) -> Result<(), Fault> {
    ctx.world.events.push(Event::Unsupported("obj_attach"));
    Ok(())
}

/// 1:140 FUN_140430440 -> FUN_1403bde30: the target record names something
/// that still exists.
fn target_is_exist(ctx: &mut Ctx<'_>, _: &mut Memory<'_>, args: &mut Args) -> Result<(), Fault> {
    let alive = super::target::record_exists(ctx, args[0].value);
    args[0] = Slot::int(alive as i32);
    Ok(())
}

/// 1:209 FUN_14042dcf0 -> FUN_1403d3f10: the object is finished and removed.
fn obj_dead(ctx: &mut Ctx<'_>, m: &mut Memory<'_>, args: &mut Args) -> Result<(), Fault> {
    let id = ctx.sys_of_ai(m, args[0].value)?;
    ctx.object(id)?.sys.gone = true;
    ctx.world.events.push(Event::ObjectDead { object: id });
    Ok(())
}

/// 2:45 FUN_1404320a0: the attack parameter id of the hit being handled.
fn damage_attack_param_id(
    ctx: &mut Ctx<'_>,
    _: &mut Memory<'_>,
    args: &mut Args,
) -> Result<(), Fault> {
    let handle = args[0].value;
    let id = crate::world::object_field(handle).map_or(0, |(id, _)| id);
    let v = ctx.object(id).map_or(0, |o| o.sys.damage_param_id);
    args[0] = Slot::int(v as i32);
    Ok(())
}

#[cfg(test)]
mod tests;
