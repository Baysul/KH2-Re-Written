//! Synthetic objects only.
use super::*;
use crate::env::{Clips, NullEnv};
use crate::world::Object;

struct Room {
    player: Vec3,
    battle: bool,
    hp: Option<(i32, i32)>,
}

impl Env for Room {
    fn floor(&self, _: f32, _: f32, _: f32) -> Option<f32> {
        Some(0.)
    }
    fn clips(&self, model: u32) -> &dyn Clips {
        NullEnv.clips(model)
    }
    fn player_position(&self) -> Vec3 {
        self.player
    }
    fn target_position(&self, _: u32) -> Option<Vec3> {
        None
    }
    fn battle(&self) -> bool {
        self.battle
    }
    fn player_hp(&self) -> Option<(i32, i32)> {
        self.hp
    }
}

fn summon(p: Personality, at: Vec3) -> Object {
    let mut o = Object::bare(0);
    o.sys.position = at;
    // A free window (+0x18c bits 2..4): the AI is in control.
    o.sys.motion.flags.word |= 0x4;
    o.friend = Some(Box::new(Friend::new(p, 2., 8.)));
    o
}

fn enemy(world: &mut World, at: Vec3) -> ObjId {
    let id = world.objects.len() as ObjId;
    let mut o = Object::bare(id);
    o.sys.position = at;
    o.sys.class_flags |= class_flag::ENEMY;
    o.sys.hp = 10;
    world.objects.push(Some(Box::new(o)));
    id
}

#[test]
fn a_summon_walks_then_runs_after_sora_and_stops_near_him() {
    let mut world = World::default();
    let mut me = summon(Personality::PeterPan, [0., 0., 0.]);
    let mut room = Room {
        player: [0., 0., 600.],
        battle: false,
        hp: None,
    };
    // The first frame only clears the fresh flag.
    tick(&mut world, 0, &mut me, &room);
    assert_eq!(me.sys.move_speed, 0.);
    // 600 away: band 0 → 1 (over 200), then 1 → 2 (over 240); the
    // smoothing snaps the speed to walk, then run.
    for _ in 0..8 {
        tick(&mut world, 0, &mut me, &room);
    }
    assert_eq!(me.friend.as_ref().unwrap().band, 2);
    assert_eq!(me.sys.move_speed, 8.);
    assert!(
        me.sys.move_direction[2] > 0.99,
        "{:?}",
        me.sys.move_direction
    );
    // Next to Sora the band falls back to standing.
    room.player = [0., 0., 50.];
    for _ in 0..12 {
        tick(&mut world, 0, &mut me, &room);
    }
    assert_eq!(me.friend.as_ref().unwrap().band, 0);
    assert_eq!(me.sys.move_speed, 0.);
}

#[test]
fn without_a_free_window_the_ai_leaves_the_object_alone() {
    let mut world = World::default();
    let mut me = summon(Personality::Genie, [0., 0., 0.]);
    me.sys.motion.flags.word = 0;
    me.sys.move_speed = 3.;
    let room = Room {
        player: [0., 0., 900.],
        battle: false,
        hp: None,
    };
    for _ in 0..3 {
        tick(&mut world, 0, &mut me, &room);
    }
    assert_eq!(me.sys.move_speed, 3.);
}

#[test]
fn in_battle_chicken_little_targets_the_nearest_enemy_and_starts_battle() {
    let mut world = World::default();
    world.objects.push(None);
    let near = enemy(&mut world, [0., 0., 300.]);
    let _far = enemy(&mut world, [0., 0., 550.]);
    let mut me = summon(Personality::ChickenLittle, [0., 0., 0.]);
    let room = Room {
        player: [0., 0., -100.],
        battle: true,
        hp: None,
    };
    tick(&mut world, 0, &mut me, &room);
    let r = tick(&mut world, 0, &mut me, &room);
    assert_eq!(me.sys.target.id, crate::traps::target::OBJECT_TARGET | near);
    assert_eq!(r.label, Some(&b"battle"[..]));
}

#[test]
fn genie_heals_sora_when_his_hp_is_under_a_quarter() {
    let mut world = World::default();
    let mut me = summon(Personality::Genie, [0., 0., 0.]);
    let room = Room {
        player: [0., 0., 900.],
        battle: true,
        hp: Some((10, 100)),
    };
    tick(&mut world, 0, &mut me, &room);
    let healed =
        (0..20).any(|_| tick(&mut world, 0, &mut me, &room).command == Some((1, PLAYER_OBJECT)));
    assert!(healed);
}

/// Effects under handle 100 still play.
struct Effects;

impl Env for Effects {
    fn floor(&self, _: f32, _: f32, _: f32) -> Option<f32> {
        Some(0.)
    }
    fn clips(&self, model: u32) -> &dyn Clips {
        NullEnv.clips(model)
    }
    fn player_position(&self) -> Vec3 {
        [0.; 3]
    }
    fn target_position(&self, _: u32) -> Option<Vec3> {
        None
    }
    fn effect_active(&self, handle: u32) -> bool {
        handle < 100
    }
}

#[test]
fn the_watch_list_keeps_32_playing_effects_and_forgets_ended_ones() {
    let mut f = Friend::new(Personality::ChickenLittle, 2., 8.);
    f.add_watch(0, &Effects);
    assert!(f.watch.is_empty());
    f.add_watch(150, &Effects);
    assert!(f.watches(150));
    // The next add sweeps the ended effect out.
    f.add_watch(1, &Effects);
    assert_eq!(f.watch, vec![1]);
    // Adding a listed effect moves it to the end.
    f.add_watch(2, &Effects);
    f.add_watch(1, &Effects);
    assert_eq!(f.watch, vec![2, 1]);
    for e in 3..40 {
        f.add_watch(e, &Effects);
    }
    assert_eq!(f.watch.len(), 32);
    assert_eq!(f.watch[0], 8);
    assert!(!f.watches(2) && f.watches(39));
}

#[test]
fn soras_attack_notice_lasts_eight_frames_of_the_same_attack_and_untakes() {
    let mut world = World::default();
    world.objects.push(None);
    let foe = enemy(&mut world, [0., 0., 100.]);
    let mut me = summon(Personality::PeterPan, [0., 0., 0.]);
    me.friend.as_deref_mut().unwrap().flags |= flag::NOTIFY_TAKEN;
    note_player_attack(&mut world, Some(&mut me), 7, foe);
    assert_eq!(me.friend.as_deref().unwrap().flags & flag::NOTIFY_TAKEN, 0);
    assert!(world.player_notify.fresh());
    assert_eq!(world.player_notify.target, foe);
    for _ in 0..8 {
        age_player_notify(&mut world, Some(7));
    }
    assert!(world.player_notify.fresh());
    age_player_notify(&mut world, Some(7));
    assert!(!world.player_notify.fresh());
    // Another attack of Sora's, or none, drops it.
    note_player_attack(&mut world, None, 8, 0);
    assert_eq!(world.player_notify.target, 0);
    age_player_notify(&mut world, None);
    assert_eq!(world.player_notify, PlayerNotify::default());
}
