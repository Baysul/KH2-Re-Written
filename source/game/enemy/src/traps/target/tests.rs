//! The battle target table, on synthetic objects.
use super::*;
use crate::env::NullEnv;
use crate::traps::test_support::{ai_object, call, frame, world_with};
use crate::world::{Object, World};

fn register(world: &mut World, id: u32, priority: i32) {
    let mut f = frame();
    let mut me = Object::bare(0);
    let ai = ai_object(&mut f, 0, id);
    call(
        world,
        &mut me,
        &mut f,
        target_set_before_player,
        &[Slot::address(ai), Slot::int(priority)],
    );
}

fn pick(world: &mut World) -> Option<ObjId> {
    let mut me = Object::bare(0);
    let mut ctx = Ctx {
        world,
        me: &mut me,
        env: &NullEnv,
    };
    pick_battle_target(&mut ctx)
}

#[test]
fn priority_zero_wins_then_the_lowest_priority() {
    let mut world = world_with(&[0., 10., 20., 30.]);
    register(&mut world, 1, 5);
    register(&mut world, 2, 3);
    assert_eq!(pick(&mut world), Some(2));
    register(&mut world, 3, 0);
    assert_eq!(pick(&mut world), Some(3));
}

#[test]
fn a_registered_object_keeps_its_slot_and_takes_the_new_priority() {
    let mut world = world_with(&[0., 10.]);
    register(&mut world, 1, 5);
    register(&mut world, 1, 2);
    let used = world
        .battle_targets
        .iter()
        .filter(|t| t.object == Some(1))
        .count();
    assert_eq!(used, 1);
    assert!(world.battle_targets.iter().any(|t| t.priority == 2));
}

#[test]
fn gone_objects_leave_the_table_and_ties_draw_from_the_engine_generator() {
    let mut world = world_with(&[0., 10., 20., 30.]);
    register(&mut world, 1, 4);
    register(&mut world, 2, 4);
    // Each object takes the last free slot, so object 2 (slot 6) is met
    // before object 1 (slot 7). The first draw of the engine generator
    // (seed 1) is 0x10dcd, a small fraction of the range: the first met.
    assert_eq!(pick(&mut world), Some(2));
    world.objects[2].as_mut().unwrap().sys.gone = true;
    assert_eq!(pick(&mut world), Some(1));
    register(&mut world, 3, 9);
    assert!(world.battle_targets.iter().all(|t| t.object != Some(2)));
}

#[test]
fn kind_zero_search_uses_the_table_and_falls_back_to_sora() {
    let mut world = world_with(&[0., 10.]);
    let mut me = Object::bare(0);
    let mut ctx = Ctx {
        world: &mut world,
        me: &mut me,
        env: &NullEnv,
    };
    assert_eq!(search(&mut ctx, 0, 0).unwrap().id, PLAYER_TARGET);
    register(ctx.world, 1, 7);
    assert_eq!(search(&mut ctx, 0, 0).unwrap().id, OBJECT_TARGET | 1);
}
