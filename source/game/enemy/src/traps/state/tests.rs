//! Synthetic objects only.
use super::*;
use crate::env::NullEnv;
use crate::sysobj::Sysobj;
use crate::world::{Object, World};

#[test]
fn collision_masks_pair_a_disable_word_with_a_reset_of_both() {
    let mut s = Sysobj::bare();
    s.disable_collision(3);
    s.disable_collision(34);
    // Index 34 lands one word further, as the executable indexes it.
    assert_eq!(s.collision, [0, 1 << 3, 1 << 2]);
    s.reset_collision(3);
    assert_eq!(s.collision[1], 0);
    assert_eq!(s.collision[2], 1 << 2, "other words stay");
}

#[test]
fn obj_fly_normalises_the_direction_and_decays_without_a_request() {
    let mut s = Sysobj::bare();
    s.start_steer([3., 4., 0.], 5., 0.5);
    assert!(!s.is_jump(), "mode 5 has the fly bit, not the jump bit");
    assert!(s.is_fly());
    // The first run happens on the next frame; it keeps half the velocity.
    s.end_tick(&NullEnv);
    let step = s.steer.unwrap().velocity;
    assert_eq!(step, [1.5, 2., 0.]);
    // The displacement of the two-frame span is spread over its frames.
    assert_eq!(s.moved, [1.5, 2., 0.]);
    assert!(s.steer_moving);
}

#[test]
fn obj_fly_follows_a_move_request_with_its_inertia() {
    let mut s = Sysobj::bare();
    s.start_steer([1., 0., 0.], 10., 0.75);
    s.move_direction = [0., 0., 1.];
    s.move_speed = 4.;
    s.end_tick(&NullEnv);
    // target (0, 0, 4) * 0.25 + (10, 0, 0) * 0.75
    assert_eq!(s.steer.unwrap().velocity, [7.5, 0., 1.]);
}

#[test]
fn going_to_the_ground_mode_ends_the_fly() {
    let mut s = Sysobj::bare();
    s.start_steer([0., 1., 0.], 1., 0.9);
    s.to_ground_mode();
    assert!(!s.is_fly());
}

#[test]
fn spec_word_12_is_the_turn_rate() {
    let mut s = Sysobj::bare();
    assert!(s.poke_word(0x134, 0.1f32.to_bits()));
    assert_eq!(s.turn_rate, 0.1);
    assert_eq!(s.peek_word(0x134), Some(0.1f32.to_bits()));
}

#[test]
fn teamwork_blocks_are_shared_by_key() {
    let mut world = World::default();
    let mut obj = Object::bare(0);
    let a = world.alloc(8);
    world.teamwork.insert(7, a);
    assert_eq!(world.static_read(a), Some(0));
    let mut ctx = Ctx {
        world: &mut world,
        me: &mut obj,
        env: &NullEnv,
    };
    let mut inst = crate::script::Instance::new(0, &{
        let mut h = vec![0u8; 44];
        h[..4].copy_from_slice(b"test");
        h[16..20].copy_from_slice(&64u32.to_le_bytes());
        h[20..24].copy_from_slice(&64u32.to_le_bytes());
        h[24..28].copy_from_slice(&64u32.to_le_bytes());
        h
    })
    .unwrap();
    let mut th = crate::script::Thread::new(&inst, 0, 0, &[]);
    let mut memory = Memory::new(&mut inst, &mut th);
    let mut args = [Slot::default(); 16];
    args[0] = Slot::int(7);
    args[1] = Slot::int(8);
    teamwork_alloc(&mut ctx, &mut memory, &mut args).unwrap();
    assert_eq!(args[0].value, a);
    assert_eq!(ctx.world.static_read(a).unwrap() & 0xff, 1);
    args[0] = Slot::int(9);
    teamwork_alloc(&mut ctx, &mut memory, &mut args).unwrap();
    assert_ne!(args[0].value, a);
    assert_eq!(ctx.world.static_read(args[0].value), Some(0));
}

#[test]
fn routine_option_flags_are_bits_across_words() {
    let mut s = Sysobj::bare();
    assert!(!s.rtn_flag(40));
    s.set_rtn_flag(40);
    assert!(s.rtn_flag(40));
    assert!(!s.rtn_flag(8));
    assert_eq!(s.rtn_flags, [0, 1 << 8]);
}

#[test]
fn scale_w_is_word_0x3c() {
    let mut s = Sysobj::bare();
    assert_eq!(s.peek_word(0x3c), Some(1.0f32.to_bits()));
    assert!(s.poke_word(0x3c, 2.4f32.to_bits()));
    assert_eq!(s.scale_w, 2.4);
}
