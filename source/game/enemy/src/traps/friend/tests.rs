//! Synthetic objects only.
use super::*;
use crate::friend::{Friend, Personality};
use crate::traps::test_support::{ai_object, call, frame, vector as put_vector};
use crate::world::{Object, World};

fn genie(id: ObjId) -> Object {
    let mut o = Object::bare(id);
    o.friend = Some(Box::new(Friend::new(Personality::Genie, 2., 8.)));
    o
}

#[test]
fn the_leave_follow_and_inertia_flags_round_trip() {
    let mut world = World::default();
    let mut me = genie(0);
    let mut f = frame();
    let ai = ai_object(&mut f, 0, 0);
    let a = [Slot::address(ai)];
    assert_eq!(
        call(&mut world, &mut me, &mut f, lookup(10, 42).unwrap(), &a).as_int(),
        0
    );
    call(&mut world, &mut me, &mut f, lookup(10, 41).unwrap(), &a);
    assert_eq!(
        call(&mut world, &mut me, &mut f, lookup(10, 42).unwrap(), &a).as_int(),
        1
    );
    for (index, bit, on) in [
        (28u16, flag::INERTIA_OFF, true),
        (27, flag::INERTIA_OFF, false),
        (46, flag::NO_ENEMY_FOLLOW, true),
        (47, flag::NO_ENEMY_FOLLOW, false),
        (48, flag::NO_PLAYER_FOLLOW, true),
        (49, flag::NO_PLAYER_FOLLOW, false),
    ] {
        call(&mut world, &mut me, &mut f, lookup(10, index).unwrap(), &a);
        assert_eq!(
            me.friend.as_ref().unwrap().flags & bit != 0,
            on,
            "10:{index}"
        );
    }
}

#[test]
fn control_reads_the_free_windows_and_genie_forms_name_their_limits() {
    let mut world = World::default();
    let mut me = genie(0);
    let mut f = frame();
    let ai = ai_object(&mut f, 0, 0);
    let a = [Slot::address(ai)];
    assert_eq!(
        call(&mut world, &mut me, &mut f, lookup(10, 36).unwrap(), &a).as_int(),
        0
    );
    me.sys.motion.flags.word |= 0x8;
    assert_eq!(
        call(&mut world, &mut me, &mut f, lookup(10, 36).unwrap(), &a).as_int(),
        1
    );
    assert_eq!(
        call(&mut world, &mut me, &mut f, lookup(10, 37).unwrap(), &a).as_int(),
        0
    );
    call(
        &mut world,
        &mut me,
        &mut f,
        lookup(10, 9).unwrap(),
        &[Slot::address(ai), Slot::int(0x219)],
    );
    assert_eq!(
        call(&mut world, &mut me, &mut f, lookup(10, 10).unwrap(), &a).as_int(),
        0x222
    );
}

#[test]
fn chicken_littles_aim_point_goes_in_and_out_in_the_scripts_frame() {
    let mut world = World::default();
    let mut me = genie(0);
    let mut f = frame();
    let ai = ai_object(&mut f, 0, 0);
    let v = put_vector(&mut f, 32, [1., -2., 3., 1.]);
    call(
        &mut world,
        &mut me,
        &mut f,
        lookup(10, 59).unwrap(),
        &[Slot::address(ai), Slot::address(v)],
    );
    assert_eq!(me.friend.as_ref().unwrap().aim, [1., 2., 3.]);
    let r = call(
        &mut world,
        &mut me,
        &mut f,
        lookup(10, 5).unwrap(),
        &[Slot::address(ai)],
    );
    let lane = |k: u32| f32::from_bits(world.static_read(r.value + 4 * k).unwrap());
    assert_eq!([lane(0), lane(1), lane(2)], [1., -2., 3.]);
}
