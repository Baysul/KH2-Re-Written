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
    assert_eq!(&s.velocity[..3], [1.5, 2., 0.]);
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
    assert_eq!(&s.velocity[..3], [7.5, 0., 1.]);
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

struct ThirtyTicks;
impl crate::env::Clips for ThirtyTicks {
    fn length(&self, _: usize) -> u32 {
        30
    }
    fn loop_start(&self, _: usize) -> u32 {
        0
    }
    fn root_delta(&self, _: usize, _: u32) -> crate::sysobj::Vec3 {
        [0.; 3]
    }
    fn events(&self, _: usize) -> Option<&crate::motion::MotionEvents> {
        None
    }
}

#[test]
fn starting_a_motion_clears_the_end_pulse_of_the_previous_one() {
    let mut s = Sysobj::bare();
    s.motion.flags.word |= crate::motion::flag::ENDED;
    assert!(s.motion_ended());
    assert!(s.motion_start(5, 0., &ThirtyTicks));
    assert!(!s.motion_ended(), "a new motion has not ended yet");
}

struct Clock30;
impl crate::env::Env for Clock30 {
    fn floor(&self, _: f32, _: f32, _: f32) -> Option<f32> {
        Some(0.)
    }
    fn clips(&self, _: u32) -> &dyn crate::env::Clips {
        &ThirtyTicks
    }
    fn player_position(&self) -> crate::sysobj::Vec3 {
        [0.; 3]
    }
    fn target_position(&self, _: u32) -> Option<crate::sysobj::Vec3> {
        None
    }
}

#[test]
fn a_motion_queued_behind_the_playing_one_starts_when_its_clock_ends() {
    let mut s = Sysobj::bare();
    assert!(s.motion_start(192, 0., &ThirtyTicks));
    s.motion_queue.push((193, 0.));
    // The clip is 30 ticks long: the queued motion starts on the step that
    // reaches the end, not before, and its own clock begins at 0.
    for _ in 0..29 {
        s.begin_tick(&Clock30);
        assert_eq!(s.motion.slot, 192);
    }
    s.begin_tick(&Clock30);
    assert_eq!(s.motion.slot, 193);
    assert_eq!(s.motion.tick, 0);
    assert!(s.motion_queue.is_empty());
    assert!(!s.motion_ended());
}

/// Run a trap on object 0 (a bare object) whose AI struct is at work word 0
/// and whose first argument vector lives at word 8.
fn run(
    f: crate::world::TrapFn,
    obj: &mut Object,
    world: &mut World,
    vec: [f32; 4],
    extra: Slot,
) -> Slot {
    let mut header = vec![0u8; 44];
    header[..4].copy_from_slice(b"test");
    header[16..20].copy_from_slice(&64u32.to_le_bytes());
    header[20..24].copy_from_slice(&64u32.to_le_bytes());
    header[24..28].copy_from_slice(&64u32.to_le_bytes());
    let mut inst = crate::script::Instance::new(0, &header).unwrap();
    let mut th = crate::script::Thread::new(&inst, 0, 0, &[]);
    let base = inst.work_base();
    let mut memory = Memory::new(&mut inst, &mut th);
    // The AI struct: word 1 holds the engine object's handle.
    memory.write_u32(base + 4, crate::world::object_handle(obj.id));
    for (k, v) in vec.iter().enumerate() {
        memory.write_f32(base + 32 + 4 * k as u32, *v);
    }
    let mut args = [Slot::default(); 16];
    args[0] = Slot::address(base);
    args[1] = Slot::address(base + 32);
    args[2] = extra;
    let mut ctx = Ctx {
        world,
        me: obj,
        env: &NullEnv,
    };
    f(&mut ctx, &mut memory, &mut args).unwrap();
    args[0]
}

#[test]
fn script_vectors_have_y_down_and_the_engine_has_y_up() {
    let mut world = World::default();
    let mut obj = Object::bare(0);
    // A script asks to move down at speed 3: direction (0, 1, 0), speed 3.
    let move_it = crate::traps::lookup(1, 111).unwrap();
    run(
        move_it,
        &mut obj,
        &mut world,
        [0., 1., 0., 0.],
        Slot::float(3.),
    );
    assert_eq!(obj.sys.move_direction, [0., -1., 0.]);
    assert_eq!(obj.sys.move_speed, 3.);
    // The engine fell by 2 this tick; the script reads +2 (down).
    obj.sys.wished = [0., -2., 0.];
    let wished = crate::traps::lookup(1, 64).unwrap();
    let r = run(wished, &mut obj, &mut world, [0.; 4], Slot::default());
    assert_eq!(world.static_read(r.value + 4), Some(2.0f32.to_bits()));
}
