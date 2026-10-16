//! Donald's spell acts, written out from the BDX script `p_ex020` (the acts
//! `fire` L2003, `blizzard` L2571 with the shot thread L2767, `thunder`
//! L5052 and `cure` L5453): when the cast motion's frame trigger 9 releases,
//! which effect elements start, where the attack volumes sit and for how
//! long, and the heal.
//!
//! Goofy's `tornado` and `turbo` are movement acts: the member's movement
//! lives in `party_members` (`special_step`); here the act spends its MP at
//! the start (his start callback passes 1 to L2869) and Tornado's own
//! attack object (ATKP 1188, sphere 200, 100 above the member) lives while
//! the motion's range 16 is active. The weapon windows (ATKP 1160, 1162)
//! come from the weapon motion.
//!
//! Positions are the simulation's Y-up space; the script's Y-down offsets
//! are negated. A script wait of `n` frames is `n` ticks (the scripts count
//! `frametime`, 1 per tick, as the spells of `magic` do). Effect handles
//! are keys on `Fx`; the room answers `effect_pos` / `effect_is_alive` with
//! last tick's state.
//!
//! Left to the room: the voices (`?1,262`), the MP deduction (`Output::spend`)
//! and the damage of the volumes and the heal.

use crate::Vec3;
use crate::magic::{Anchor, Fx, FxLive, Role, Strike, Volume};
use crate::party_members::Spell;

/// The ATKP ids of the acts.
pub const FIRE_ATKP: u16 = 1163;
pub const BLIZZARD_ATKP: u16 = 1164;
pub const THUNDER_ATKP: u16 = 1165;
pub const CURE_ATKP: u16 = 1525;
pub const TORNADO_ATKP: u16 = 1188;

/// Fire: the ring's volume (`attack_set_radius(a, 220, 70)`) follows the
/// second effect for at most this long.
const FIRE_VOLUME: f32 = 60.;
/// Fire: ticks between the cast effect and the ring.
const FIRE_RING_WAIT: u32 = 4;
/// Thunder: ticks between the cast effect and the strike (`L2284(obj, 10)`)
/// and the life of its two volumes.
const THUNDER_WAIT: u32 = 10;
const THUNDER_LIVE: u32 = 20;
/// Blizzard's shot (`L3446`): speed per tick (`S+228` = 60), life before
/// the fade (`S+224` = 30), fade length, hit sphere (`attack_set_radius(a,
/// 80, 0)`). The shot never turns: its turn rate (`S+112+8`) stays 0.
const SHOT_SPEED: f32 = 60.;
const SHOT_LIFE: u32 = 30;
const SHOT_FADE: u32 = 8;
const SHOT_RADIUS: f32 = 80.;

/// What an act reads each tick.
pub struct Context<'a> {
    /// The caster's origin and facing in XZ.
    pub caster: Vec3,
    pub facing: [f32; 2],
    /// Where an effect with no answer is placed (the caster's body centre).
    pub centre: Vec3,
    /// The enemy target's centre (`target_pos(btlobj_target)`), if any.
    pub target: Option<Vec3>,
    /// Last tick's state of an effect handle.
    pub effect: &'a dyn Fn(u64) -> Option<FxLive>,
    /// `bg_ground_pos`: the floor under a point within a distance.
    pub ground: &'a dyn Fn(Vec3, f32) -> Option<Vec3>,
    /// `attack_is_hit_bg` for a sphere.
    pub bg_hit: &'a dyn Fn(Vec3, f32) -> bool,
    /// The member's attack motion has ended.
    pub over: bool,
}

/// One tick of an act.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Output {
    pub fx: Vec<Fx>,
    pub volumes: Vec<Volume>,
    pub strikes: Vec<Strike>,
    /// MP cost deducted by trap (1,276) this tick.
    pub spend: Option<u8>,
}

#[derive(Clone, Debug, PartialEq)]
struct Shot {
    pos: Vec3,
    dir: Vec3,
    /// Iterations run (the life counter `S+224`).
    run: u32,
    /// Ticks left of the attack after the fade began (`local 4`).
    fading: Option<u32>,
    previous: Option<Vec3>,
    /// The attack object still exists.
    attack: bool,
}

/// A cast in progress.
#[derive(Clone, Debug, PartialEq)]
pub struct Cast {
    pub spell: Spell,
    /// The caster's slot in the party (1 Donald, 2 Goofy).
    pub slot: usize,
    /// Cure: the party slot healed.
    pub heal: usize,
    /// Key base: effect handles `key + n`, attack objects `key + 0x10 + n`.
    key: u64,
    /// Frame trigger 9's tick in the cast motion.
    release: u32,
    age: u32,
    done: bool,
    /// Ring (Fire) or strike (Thunder) timers.
    started: Option<u32>,
    /// Fire: ticks the volume has followed the ring.
    followed: f32,
    previous: Option<Vec3>,
    shot: Option<Shot>,
    /// Thunder's strike point.
    strike: Option<Vec3>,
    /// Effect handles seen dead (bit per handle).
    gone: u8,
    /// The motion's range 16 (Tornado's attack object lives in it).
    range: Option<(u32, u32)>,
}

fn add(a: Vec3, b: Vec3) -> Vec3 {
    [a[0] + b[0], a[1] + b[1], a[2] + b[2]]
}

fn scale(a: Vec3, k: f32) -> Vec3 {
    a.map(|c| c * k)
}

impl Cast {
    /// A cast of `spell` by party member `slot`, released at tick `release`
    /// of its motion. `key` must be unique per cast and leave 0x20 keys
    /// free.
    pub fn new(spell: Spell, slot: usize, heal: usize, key: u64, release: u32) -> Self {
        Cast {
            spell,
            slot,
            heal,
            key,
            release,
            age: 0,
            done: false,
            started: None,
            followed: 0.,
            previous: None,
            shot: None,
            strike: None,
            gone: 0,
            range: None,
        }
    }

    /// The motion's range 16, in ticks of the motion.
    pub fn with_range(mut self, range: Option<(u32, u32)>) -> Self {
        self.range = range;
        self
    }

    pub fn finished(&self) -> bool {
        self.done
    }

    fn fx(&self, n: u64) -> u64 {
        self.key + n
    }

    fn live(&self, ctx: &Context, n: u64) -> Option<FxLive> {
        (ctx.effect)(self.fx(n))
    }

    /// `effect_is_alive`: a tracked effect answers; an untracked one (no
    /// PAX or element) leaves the script to its own timers, and a dead one
    /// stays dead.
    fn running(&mut self, ctx: &Context, n: u64) -> bool {
        if self.gone & (1 << n) != 0 {
            return false;
        }
        match self.live(ctx, n).filter(|l| l.tracked) {
            Some(l) if !l.alive => {
                self.gone |= 1 << n;
                false
            }
            _ => true,
        }
    }

    fn tracked(&self, ctx: &Context, n: u64) -> bool {
        self.live(ctx, n).is_some_and(|l| l.tracked)
    }

    /// One tick; call it on the tick the act starts and every tick after.
    pub fn tick(&mut self, ctx: &Context) -> Output {
        let mut out = Output::default();
        let me = Anchor::Party(self.slot);
        let r = self.release;
        let age = self.age;
        match self.spell {
            Spell::Fire => {
                if age == r {
                    out.spend = Some(10);
                    out.fx.push(Fx::Start {
                        key: self.fx(0),
                        id: 0,
                        bound: Some(me),
                        at: None,
                    });
                }
                if age == r + FIRE_RING_WAIT {
                    out.fx.push(Fx::Start {
                        key: self.fx(1),
                        id: 1,
                        bound: Some(me),
                        at: None,
                    });
                    self.started = Some(age);
                }
                if self.started.is_some() {
                    let alive = self.running(ctx, 1);
                    if self.followed < FIRE_VOLUME && alive {
                        // attack_set_pos(effect_pos) each frame.
                        let at = match self.live(ctx, 1) {
                            Some(l) if l.tracked => l.pos,
                            _ => add(ctx.centre, scale([ctx.facing[0], 0., ctx.facing[1]], 120.)),
                        };
                        out.volumes.push(Volume {
                            key: self.key + 0x10,
                            atkp: FIRE_ATKP,
                            sub: 0,
                            centre: at,
                            radius: 220.,
                            height: 70.,
                            previous: self.previous,
                            moving: None,
                            role: Role::Damage,
                        });
                        self.previous = Some(at);
                        self.followed += 1.;
                    }
                    if self.followed >= FIRE_VOLUME || !alive {
                        // The loop ends, the attack is freed, and the act
                        // waits for the effect to die.
                        self.followed = FIRE_VOLUME;
                        if !alive || !self.tracked(ctx, 1) {
                            self.done = true;
                        }
                    }
                }
            }
            Spell::Thunder => {
                if age == r {
                    out.fx.push(Fx::Start {
                        key: self.fx(0),
                        id: 7,
                        bound: Some(me),
                        at: None,
                    });
                }
                if age == r + THUNDER_WAIT {
                    out.spend = Some(10);
                    // target_pos, else 300 ahead of the caster.
                    let p = ctx.target.unwrap_or_else(|| {
                        add(ctx.caster, scale([ctx.facing[0], 0., ctx.facing[1]], 300.))
                    });
                    // bg_ground_pos from 80 above the point; the strike is
                    // a ground one when the floor lies at most 160 below.
                    let grounded = (ctx.ground)([p[0], p[1] + 80., p[2]], 10000.)
                        .is_some_and(|g| p[1] - g[1] <= 160.);
                    out.fx.push(Fx::Start {
                        key: self.fx(1),
                        id: if grounded { 8 } else { 9 },
                        bound: None,
                        at: Some(p),
                    });
                    self.strike = Some(p);
                    self.started = Some(age);
                }
                if let (Some(p), Some(since)) = (self.strike, self.started)
                    && age < since + THUNDER_LIVE
                {
                    // Sphere 160 at the point, and a column (25 x 320)
                    // centred 320 above it.
                    out.volumes.push(Volume {
                        key: self.key + 0x10,
                        atkp: THUNDER_ATKP,
                        sub: 0,
                        centre: p,
                        radius: 160.,
                        height: 160.,
                        previous: None,
                        moving: None,
                        role: Role::Damage,
                    });
                    out.volumes.push(Volume {
                        key: self.key + 0x11,
                        atkp: THUNDER_ATKP,
                        sub: 0,
                        centre: [p[0], p[1] + 320., p[2]],
                        radius: 25.,
                        height: 320.,
                        previous: None,
                        moving: None,
                        role: Role::Damage,
                    });
                }
                if self.started.is_some_and(|s| age + 1 >= s + THUNDER_LIVE) {
                    self.done = true;
                }
            }
            Spell::Blizzard => {
                if age == r {
                    out.spend = Some(10);
                    out.fx.push(Fx::Start {
                        key: self.fx(0),
                        id: 3,
                        bound: Some(me),
                        at: None,
                    });
                }
                // The shot thread starts on the following tick and takes
                // the cast effect's position.
                if age == r + 1 && self.shot.is_none() {
                    let pos = match self.live(ctx, 0) {
                        Some(l) if l.tracked => l.pos,
                        _ => ctx.centre,
                    };
                    let dir = [ctx.facing[0], 0., ctx.facing[1]];
                    out.fx.push(Fx::Start {
                        key: self.fx(1),
                        id: 4,
                        bound: None,
                        at: Some(pos),
                    });
                    out.fx.push(Fx::Dir(self.fx(1), dir));
                    self.shot = Some(Shot {
                        pos,
                        dir,
                        run: 0,
                        fading: None,
                        previous: None,
                        attack: true,
                    });
                    self.started = Some(age);
                }
                let alive_fx = self.started.is_none() || self.running(ctx, 1);
                let tracked = self.tracked(ctx, 1);
                if let Some(s) = self.shot.as_mut() {
                    let first = s.run == 0 && s.previous.is_none();
                    // The loop runs while the effect lives; a background
                    // hit ends it before it moves again.
                    if !first && (!alive_fx || (ctx.bg_hit)(s.pos, SHOT_RADIUS)) {
                        out.fx.push(Fx::Kill(self.key + 1));
                        s.attack = false;
                        self.done = true;
                    } else {
                        if !first {
                            s.run += 1;
                            s.pos = add(s.pos, scale(s.dir, SHOT_SPEED));
                        }
                        out.fx.push(Fx::Dir(self.key + 1, s.dir));
                        out.fx.push(Fx::Pos(self.key + 1, s.pos));
                        if s.run == SHOT_LIFE && s.fading.is_none() {
                            out.fx.push(Fx::Fade(self.key + 1, SHOT_FADE as f32));
                            s.fading = Some(SHOT_FADE);
                        } else if let Some(left) = s.fading.as_mut() {
                            *left = left.saturating_sub(1);
                            if *left == 0 {
                                s.attack = false;
                            }
                        }
                        if s.attack {
                            out.volumes.push(Volume {
                                key: self.key + 0x10,
                                atkp: BLIZZARD_ATKP,
                                sub: 0,
                                centre: s.pos,
                                radius: SHOT_RADIUS,
                                height: SHOT_RADIUS,
                                previous: s.previous,
                                moving: s.previous.map(|_| s.dir),
                                role: Role::Damage,
                            });
                        }
                        s.previous = Some(s.pos);
                        // Without a tracked effect the fade is a timer.
                        if s.fading == Some(0) && !tracked {
                            self.done = true;
                        }
                    }
                }
            }
            Spell::Tornado | Spell::Turbo => {
                if age == 0 {
                    out.spend = Some(10);
                }
                if self.spell == Spell::Tornado
                    && self.range.is_some_and(|(lo, hi)| (lo..=hi).contains(&age))
                {
                    out.volumes.push(Volume {
                        key: self.key + 0x10,
                        atkp: TORNADO_ATKP,
                        sub: 0,
                        centre: [ctx.caster[0], ctx.caster[1] + 100., ctx.caster[2]],
                        radius: 200.,
                        height: 200.,
                        previous: self.previous,
                        moving: None,
                        role: Role::Damage,
                    });
                    self.previous = Some([ctx.caster[0], ctx.caster[1] + 100., ctx.caster[2]]);
                }
                if ctx.over {
                    self.done = true;
                }
            }
            Spell::Cure => {
                if age == r {
                    out.fx.push(Fx::Start {
                        key: self.fx(0),
                        id: 11,
                        bound: Some(me),
                        at: None,
                    });
                    out.spend = Some(255);
                    out.fx.push(Fx::Start {
                        key: self.fx(1),
                        id: 12,
                        bound: Some(Anchor::Party(self.heal)),
                        at: None,
                    });
                    out.strikes.push(Strike {
                        atkp: CURE_ATKP,
                        sub: 0,
                        party: self.heal,
                    });
                    self.done = true;
                }
            }
        }
        self.age += 1;
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run(
        spell: Spell,
        release: u32,
        ticks: u32,
        effect: &dyn Fn(u64) -> Option<FxLive>,
        target: Option<Vec3>,
    ) -> Vec<Output> {
        let ground = |p: Vec3, _: f32| Some([p[0], 0., p[2]]);
        let bg = |_: Vec3, _: f32| false;
        let ctx = Context {
            caster: [0.; 3],
            facing: [0., 1.],
            centre: [0., 85., 0.],
            target,
            effect,
            ground: &ground,
            bg_hit: &bg,
            over: false,
        };
        let mut c = Cast::new(spell, 1, 0, 0x100, release);
        (0..ticks).map(|_| c.tick(&ctx)).collect()
    }

    fn none(_: u64) -> Option<FxLive> {
        None
    }

    #[test]
    fn fire_spends_ten_at_the_release_and_starts_the_ring_four_ticks_later() {
        let out = run(Spell::Fire, 26, 40, &none, None);
        let spent: Vec<usize> = (0..40).filter(|&i| out[i].spend.is_some()).collect();
        assert_eq!(spent, vec![26]);
        let rings: Vec<usize> = (0..40)
            .filter(|&i| {
                out[i]
                    .fx
                    .iter()
                    .any(|f| matches!(f, Fx::Start { id: 1, .. }))
            })
            .collect();
        assert_eq!(rings, vec![30]);
        // The ring's volume starts with the ring and is the 220 x 70 one.
        let v = &out[30].volumes[0];
        assert_eq!((v.atkp, v.radius, v.height), (1163, 220., 70.));
    }

    #[test]
    fn fires_volume_follows_the_ring_for_sixty_ticks() {
        let alive = |k: u64| {
            (k == 0x101).then_some(FxLive {
                alive: true,
                pos: [10., 20., 30.],
                tracked: true,
            })
        };
        let out = run(Spell::Fire, 26, 130, &alive, None);
        let n = out.iter().filter(|o| !o.volumes.is_empty()).count();
        assert_eq!(n, 60);
        assert_eq!(out[31].volumes[0].centre, [10., 20., 30.]);
    }

    #[test]
    fn thunder_strikes_the_target_ten_ticks_after_the_release_for_twenty() {
        let out = run(Spell::Thunder, 26, 70, &none, Some([0., 40., 400.]));
        let strike: Vec<usize> = (0..70).filter(|&i| out[i].spend.is_some()).collect();
        assert_eq!(strike, vec![36]);
        let live: Vec<usize> = (0..70).filter(|&i| !out[i].volumes.is_empty()).collect();
        assert_eq!(live.first(), Some(&36));
        assert_eq!(live.len(), 20);
        // Floor within 160 below: the ground strike (element 8).
        assert!(
            out[36]
                .fx
                .iter()
                .any(|f| matches!(f, Fx::Start { id: 8, .. }))
        );
        assert_eq!(out[36].volumes.len(), 2);
        assert_eq!(out[36].volumes[1].centre, [0., 360., 400.]);
    }

    #[test]
    fn thunder_in_the_air_uses_the_air_element() {
        let out = run(Spell::Thunder, 26, 40, &none, Some([0., 500., 400.]));
        assert!(
            out[36]
                .fx
                .iter()
                .any(|f| matches!(f, Fx::Start { id: 9, .. }))
        );
    }

    #[test]
    fn thunder_without_a_target_falls_300_ahead() {
        let out = run(Spell::Thunder, 26, 40, &none, None);
        assert_eq!(out[36].volumes[0].centre, [0., 0., 300.]);
    }

    #[test]
    fn blizzards_shot_flies_straight_at_sixty_a_tick_and_ends_after_thirty_eight() {
        let alive = |k: u64| match k {
            0x100 => Some(FxLive {
                alive: true,
                pos: [0., 90., 20.],
                tracked: true,
            }),
            0x101 => Some(FxLive {
                alive: true,
                pos: [0., 90., 20.],
                tracked: true,
            }),
            _ => None,
        };
        let out = run(Spell::Blizzard, 20, 80, &alive, None);
        let first = (0..80).find(|&i| !out[i].volumes.is_empty()).unwrap();
        assert_eq!(first, 21);
        let v0 = out[first].volumes[0];
        let v1 = out[first + 1].volumes[0];
        assert_eq!(v0.centre, [0., 90., 20.]);
        assert_eq!(v1.centre, [0., 90., 80.]);
        assert_eq!(v0.radius, 80.);
        // Fade begins at the thirtieth move, the attack goes eight ticks later.
        let last = (0..80).rev().find(|&i| !out[i].volumes.is_empty()).unwrap();
        assert_eq!(last - first, 30 + 8 - 1);
        assert!(out[first + 30].fx.iter().any(|f| matches!(f, Fx::Fade(..))));
    }

    #[test]
    fn tornado_spends_ten_at_the_start_and_its_attack_lives_in_the_range() {
        let ground = |_: Vec3, _: f32| None;
        let bg = |_: Vec3, _: f32| false;
        let mut c = Cast::new(Spell::Tornado, 2, 0, 0x100, 0).with_range(Some((50, 380)));
        let mut spent = Vec::new();
        let mut live = Vec::new();
        for t in 0..420u32 {
            let ctx = Context {
                caster: [0., 40., 0.],
                facing: [0., 1.],
                centre: [0.; 3],
                target: None,
                effect: &none,
                ground: &ground,
                bg_hit: &bg,
                over: t >= 400,
            };
            let o = c.tick(&ctx);
            if o.spend.is_some() {
                spent.push(t);
            }
            if let Some(v) = o.volumes.first() {
                live.push((t, v.atkp, v.centre[1], v.radius));
            }
        }
        assert_eq!(spent, vec![0]);
        assert_eq!(live.first(), Some(&(50, 1188, 140., 200.)));
        assert_eq!(live.last().map(|l| l.0), Some(380));
        assert!(c.finished());
    }

    #[test]
    fn cure_spends_everything_and_heals_the_chosen_member_at_the_release() {
        let mut c = Cast::new(Spell::Cure, 1, 2, 0x100, 26);
        let ground = |_: Vec3, _: f32| None;
        let bg = |_: Vec3, _: f32| false;
        let ctx = Context {
            caster: [0.; 3],
            facing: [0., 1.],
            centre: [0.; 3],
            target: None,
            effect: &none,
            ground: &ground,
            bg_hit: &bg,
            over: false,
        };
        let mut healed = Vec::new();
        for t in 0..30 {
            let o = c.tick(&ctx);
            if !o.strikes.is_empty() {
                healed.push((t, o.strikes[0], o.spend));
            }
        }
        assert_eq!(
            healed,
            vec![(
                26,
                Strike {
                    atkp: 1525,
                    sub: 0,
                    party: 2
                },
                Some(255)
            )]
        );
        assert!(c.finished());
    }
}
