//! The Keyblade's swing trail: the weapon motion's range trigger 12 marks
//! the swing, the weapon PAX's trail element (id 10, flag 0x10000, a
//! BonePos pair on bone 2) draws it, and every tick the two points of the
//! pair on the blade bone are handed to the running instance
//! (FUN_1402c5f70 → FUN_1403b54f0 build them the same way).
//!
//! [I] The spawn rule (the range's first tick starts element 10 once) and
//! the bone length (FUN_1401a7af0's skeleton field, taken from the blade
//! mesh's extent along its bone) are inferred; the element, its points and
//! the strip are the exe's.
use super::*;
use data::object_effect::{BonePos, Pax};

/// The weapon PAX element the trail uses.
const TRAIL_ELEMENT: u16 = 10;

/// The two points of `bone` in engine world space for this tick, or `None`
/// without a weapon pose.
pub fn points(game: &Game, bone: &BonePos) -> Option<[[f32; 3]; 2]> {
    let clip = *game.poses.get(&(game.player.action, game.player.armed))?;
    let cp = game.clip_poses.get(clip)?;
    let frame = game.player.motion_frame(&game.clips) as usize;
    let blade = cp.blade.get(frame.min(cp.blade.len().checked_sub(1)?))?;
    let hand = cp.weapon.get(frame.min(cp.weapon.len().checked_sub(1)?))?;
    let length = game.trail_length?;
    let at = Vec3::from_array(game.player.position);
    let turn = Quat::from_rotation_y(game.player.yaw);
    let to_scene = |ratio: f32| {
        let p = at + turn * (*hand * *blade).transform_point3(Vec3::X * length * ratio);
        to_world(p.to_array())
    };
    Some([to_scene(bone.ratio_a), to_scene(bone.ratio_b)])
}

/// The blade bone's length: the mesh extent along the bone's x axis divided
/// by the trail element's far ratio (3.7 for the Keyblades).
fn calibrate(game: &Game, far_ratio: f32) -> Option<f32> {
    let clip = *game.poses.get(&(game.player.action, game.player.armed))?;
    let cp = game.clip_poses.get(clip)?;
    let blade = cp.blade.first()?;
    let verts = cp.weapon_frames.first()?;
    let inverse = blade.inverse();
    let (mut lo, mut hi) = (f32::MAX, f32::MIN);
    for v in verts {
        let x = inverse.transform_point3(*v).x;
        lo = lo.min(x);
        hi = hi.max(x);
    }
    if lo > hi || far_ratio == 0. {
        return None;
    }
    // The ratios are negative: the tip lies on the bone's -x side.
    Some(lo.abs().max(hi.abs()) / far_ratio.abs())
}

/// Starts the trail element when a trail window opens.
pub fn tick(game: &mut Game, pax: Option<&Arc<Pax>>) {
    let Some(pax) = pax else { return };
    if !game.player.armed {
        game.trail_scan = None;
        return;
    }
    let Some(element) = pax.by_id(TRAIL_ELEMENT) else {
        return;
    };
    if game.trail_length.is_none() {
        game.trail_length = calibrate(game, element.bone_pos.ratio_b);
        let tick = game.tick;
        let length = game.trail_length;
        game.event(serde_json::json!({"event":"weapon_trail_length","tick":tick,"length":length}));
    }
    let Some(&clip) = game.poses.get(&(game.player.action, game.player.armed)) else {
        return;
    };
    let frame = game.player.motion_frame(&game.clips);
    let started = game.action_started;
    let from = match game.trail_scan {
        Some((s, f)) if s == started && f <= frame => Some(f),
        _ => None,
    };
    game.trail_scan = Some((started, frame));
    let due = game.clip_poses[clip].trails.iter().any(|&(start, end)| {
        frame <= end
            && match from {
                Some(prev) => start > prev && start <= frame,
                None => start <= frame,
            }
    });
    if due {
        game.fx_queue.push(drive_fx::Placed {
            req: drive_fx::FxRequest {
                pax: pax.clone(),
                id: TRAIL_ELEMENT,
                owner: 0xfc,
            },
            anchor: drive_fx::Anchor::Weapon,
        });
    }
}
