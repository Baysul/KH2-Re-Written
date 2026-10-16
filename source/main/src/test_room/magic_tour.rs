//! `--magic-tour <new-capture-dir>` (with `--magic-form <n>` to cast in a
//! Drive form): an unattended run that casts each spell from the ground and
//! the air, then probes how a cast joins the combo: a cast pressed at each
//! distance after an attack, a cast after two attacks (the finisher), and an
//! attack pressed after a cast that hit. Sora is invincible, the enemy
//! stands in front of him and takes every hit. The run's log carries a
//! `cast` / `cast_refused` event for each press. After Sora's casts, Donald
//! casts each of his four spells on order, then the party's AI runs free.
use player::magic;
use player::party_members::Spell;

/// Ticks of the tour: the six spells on the ground, then in the air.
const GROUND: [(u64, u8); 6] = [
    (100, magic::FIRE),
    (260, magic::BLIZZARD),
    (420, magic::THUNDER),
    (580, magic::CURE),
    (740, magic::MAGNET),
    (900, magic::REFLECT),
];
const AIR: [(u64, u8); 3] = [
    (1085, magic::FIRE),
    (1225, magic::BLIZZARD),
    (1365, magic::THUNDER),
];
const AIR_JUMPS: [u64; 3] = [1060, 1200, 1340];
/// Attack, then Fire this many ticks later.
const PROBES: [u64; 10] = [6, 10, 14, 18, 22, 26, 30, 36, 44, 56];
const PROBE_START: u64 = 1550;
const PROBE_SPACING: u64 = 160;
/// Attack, attack 30 ticks later, then Fire after these many more (a
/// combo finisher on the third hit).
const FINISH: [u64; 4] = [28, 34, 40, 48];
const FINISH_START: u64 = 3200;
const FINISH_SPACING: u64 = 200;
const SECOND_ATTACK: u64 = 30;
/// Fire, then an attack this many ticks later (the cast hit the enemy).
const AFTER: [u64; 3] = [62, 70, 78];
const AFTER_START: u64 = 4000;
const AFTER_SPACING: u64 = 200;
/// Cure on Donald, then on Goofy (the party list's picks).
const PARTY_CURES: [(u64, usize); 2] = [(4650, 1), (4800, 2)];
/// Donald's four spells on order, then the party's AI casting on its own.
const PARTY_CASTS: [(u64, usize, Spell, u8); 6] = [
    (5050, 0, Spell::Fire, 0),
    (5250, 0, Spell::Thunder, 0),
    (5450, 0, Spell::Blizzard, 0),
    (5650, 0, Spell::Cure, 2),
    (5800, 1, Spell::Turbo, 0),
    (6200, 1, Spell::Tornado, 0),
];
/// The AI's own casts run from here to the end.
pub const FREE_PARTY: u64 = 6800;
/// Magic Lock-On: Fire pressed with the enemy beyond the automatic
/// target's range, with the ability (first) and without it (second).
const LOCK: [(u64, bool); 2] = [(8400, true), (8600, false)];
const LOCK_DISTANCE: f32 = 1400.;
pub const END: u64 = 8800;

/// The party slot Cure targets when it is cast this tick (0 = Sora).
pub fn cure_target(tick: u64, shift: u64) -> usize {
    let Some(tick) = tick.checked_sub(shift) else {
        return 0;
    };
    PARTY_CURES.iter().find(|c| c.0 == tick).map_or(0, |c| c.1)
}

/// A party member's ordered cast this tick: member index (0 Donald, 1
/// Goofy), the act, and the party slot Cure heals.
pub fn party_cast(tick: u64, shift: u64) -> Option<(usize, Spell, u8)> {
    let tick = tick.checked_sub(shift)?;
    PARTY_CASTS
        .iter()
        .find(|c| c.0 == tick)
        .map(|c| (c.1, c.2, c.3))
}

/// Whether Magic Lock-On is on from this tick on, when a probe changes it.
pub fn lock_on(tick: u64, shift: u64) -> Option<bool> {
    let tick = tick.checked_sub(shift)?;
    LOCK.iter().find(|l| l.0 == tick + 1).map(|l| l.1)
}

/// How far ahead of Sora the enemy is put back this tick.
pub fn place_distance(tick: u64, shift: u64) -> f32 {
    let far = tick
        .checked_sub(shift)
        .is_some_and(|t| LOCK.iter().any(|l| l.0 == t + 2));
    if far { LOCK_DISTANCE } else { 110. }
}

/// The party's AI may cast spells.
pub fn free_party(tick: u64, shift: u64) -> bool {
    tick.checked_sub(shift).is_some_and(|t| t >= FREE_PARTY)
}

fn probe_base(i: usize) -> u64 {
    PROBE_START + i as u64 * PROBE_SPACING
}

fn finish_base(i: usize) -> u64 {
    FINISH_START + i as u64 * FINISH_SPACING
}

fn after_base(i: usize) -> u64 {
    AFTER_START + i as u64 * AFTER_SPACING
}

/// The spell cast this tick, if any.
pub fn cast(tick: u64, shift: u64) -> Option<u8> {
    let tick = tick.checked_sub(shift)?;
    for (t, m) in GROUND.iter().chain(&AIR) {
        if *t == tick {
            return Some(*m);
        }
    }
    let at = |base: u64, k: u64| base + k == tick;
    if PARTY_CURES.iter().any(|c| c.0 == tick) {
        return Some(magic::CURE);
    }
    if LOCK.iter().any(|l| l.0 == tick) {
        return Some(magic::FIRE);
    }
    if PROBES
        .iter()
        .enumerate()
        .any(|(i, &k)| at(probe_base(i), k))
        || FINISH
            .iter()
            .enumerate()
            .any(|(i, &k)| at(finish_base(i), SECOND_ATTACK + k))
        || (0..AFTER.len()).any(|i| after_base(i) == tick)
    {
        return Some(magic::FIRE);
    }
    None
}

/// The enemy is put back 110 ahead of Sora two ticks before each action.
pub fn place(tick: u64, shift: u64) -> bool {
    let Some(tick) = tick.checked_sub(shift) else {
        return tick == 60;
    };
    let starts = GROUND
        .iter()
        .chain(&AIR)
        .map(|s| s.0)
        .chain(AIR_JUMPS)
        .chain((0..PROBES.len()).map(probe_base))
        .chain((0..FINISH.len()).map(finish_base))
        .chain((0..AFTER.len()).map(after_base))
        .chain(PARTY_CURES.iter().map(|c| c.0))
        .chain(PARTY_CASTS.iter().map(|c| c.0))
        .chain(LOCK.iter().map(|l| l.0));
    tick == 60 || starts.into_iter().any(|b| b == tick + 2)
}

/// The attack pressed this tick.
pub fn attack(tick: u64, shift: u64) -> bool {
    let Some(tick) = tick.checked_sub(shift) else {
        return false;
    };
    (0..PROBES.len()).any(|i| probe_base(i) == tick)
        || (0..FINISH.len()).any(|i| {
            let b = finish_base(i);
            tick == b || tick == b + SECOND_ATTACK
        })
        || AFTER
            .iter()
            .enumerate()
            .any(|(i, &k)| after_base(i) + k == tick)
}

pub fn jump(tick: u64, shift: u64) -> bool {
    tick.checked_sub(shift)
        .is_some_and(|t| AIR_JUMPS.contains(&t))
}

/// Capture ticks: shortly after each cast.
pub fn captures(shift: u64) -> Vec<u64> {
    let mut at = Vec::new();
    for (t, _) in GROUND.iter().chain(&AIR) {
        at.extend([t + 14, t + 26, t + 40, t + 60]);
    }
    for (i, k) in FINISH.iter().enumerate() {
        at.push(finish_base(i) + SECOND_ATTACK + k + 12);
    }
    for (i, k) in AFTER.iter().enumerate() {
        at.push(after_base(i) + k + 16);
    }
    at.extend(PARTY_CURES.iter().flat_map(|c| [c.0 + 20, c.0 + 40]));
    at.extend(
        PARTY_CASTS
            .iter()
            .flat_map(|c| [c.0 + 20, c.0 + 50, c.0 + 80, c.0 + 100, c.0 + 130]),
    );
    at.extend((0..8).map(|i| FREE_PARTY + 100 + i * 120));
    at.extend(LOCK.iter().flat_map(|l| [l.0 + 14, l.0 + 40]));
    at.sort_unstable();
    at.dedup();
    at.iter().map(|t| t + shift).collect()
}
