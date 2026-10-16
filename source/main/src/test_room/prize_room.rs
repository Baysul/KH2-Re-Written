//! What a kill gives in the test room: EXP and level-ups (FUN_140410af0,
//! FUN_1403ecf30), the prizes scattered on the ground (FUN_1403dc170) and
//! their pickup effects (FUN_1403dca50).
use super::*;
use player::enemy_prizes::{self, Orb};

/// One line of the unattended log.
fn log_event(log: &mut Option<std::fs::File>, record: serde_json::Value) {
    if let Some(file) = log.as_mut() {
        let _ = serde_json::to_writer(&mut *file, &record);
        let _ = std::io::Write::write_all(file, b"\n");
    }
}

pub fn tick(game: &mut Game, tick: u64) {
    let Game {
        player,
        enemy,
        world,
        log,
        autoplay,
        drive,
        sound_queue,
        ..
    } = game;
    // The Drive form Sora is in (0 without one).
    let form = drive.as_ref().filter(|d| d.active).map_or(0, |d| d.form);
    let Some(rt) = enemy.as_mut() else { return };
    if autoplay.is_some() && tick == 1 {
        // The unattended run starts two EXP short of level 2.
        rt.exp_total = 38;
    }
    let dead = rt.enemy.current_act() == Some(shadow::ActKind::Dead);
    if dead && !rt.was_dead {
        kill(player, rt, world, log, tick);
        // FUN_1403e8780: Wisdom counts Heartless, Final Nobodies.
        if let Some(d) = drive.as_mut() {
            d.pending_exp += player::forms::kill_exp(form, d.enemy_character);
        }
    }
    rt.was_dead = dead;
    // The prizes fall and wait to be collected.
    let at = player.position;
    let draw = player.abilities.draw;
    let mut collected = Vec::new();
    for (i, orb) in rt.orbs.iter_mut().enumerate() {
        let floor = world
            .floor(orb.position[0], orb.position[2], orb.position[1] + 30.)
            .unwrap_or(0.);
        if let Some(id) = orb.tick(floor) {
            sound_queue.push((id, "prize_bounce"));
        }
        // A dead Sora collects nothing (FUN_1403dc500: 0x9b8 bit 2).
        if !player.dead() && orb.collected_by(at, draw) {
            collected.push(i);
        }
    }
    for i in collected.into_iter().rev() {
        let orb = rt.orbs.remove(i);
        let e = enemy_prizes::effect(orb.kind);
        // FUN_140413650: the descriptor's pickup sound.
        sound_queue.push((enemy_prizes::sounds(orb.kind).1, "prize_pickup"));
        // Antiform's pickup skips HP orbs (formcmd-01 §5).
        if e.hp > 0 && form != player::forms::ANTI {
            rt.sora_hp = (rt.sora_hp + e.hp).min(rt.sora_target.max_hp);
        }
        if e.mp > 0 {
            player.mp.add(e.mp);
        }
        if e.drive > 0 {
            let (charging, boost) = (player.mp_charging, player.abilities.drive_boost);
            player.drive.gain(e.drive as f32, charging, boost, 1.);
            // FUN_1403e8760: Master Form EXP is the orb's drive value.
            if form == player::forms::MASTER
                && let Some(d) = drive.as_mut()
            {
                d.pending_exp += e.drive as i32;
            }
        }
        rt.munny += e.munny as u32;
        log_event(
            log,
            serde_json::json!({"event":"prize","tick":tick,"kind":orb.kind,
                "hp":rt.sora_hp,"mp":player.mp.cur,"drive":player.drive.level,
                "drive_partial":player.drive.partial,"munny":rt.munny}),
        );
    }
}

/// The kill (FUN_140410af0): EXP, level-ups, then the prize scatter.
fn kill(
    player: &mut Player,
    rt: &mut EnemyRuntime,
    world: &RoomWorld,
    log: &mut Option<std::fs::File>,
    tick: u64,
) {
    let (enemy_exp, level_exp) = rt.exp_rule;
    let critical = rt.difficulty == combat::Difficulty::Critical;
    let low = rt.sora_hp * 2 <= rt.sora_target.max_hp;
    let boost = low.then_some(player.abilities.experience_boost as u32);
    let exp = enemy_prizes::kill_exp(enemy_exp, level_exp, critical, boost);
    // FUN_1403ed110: No Experience (ability bit 116) takes the kill's EXP.
    if !player.abilities.has(abilities::bit::NO_EXPERIENCE) {
        grant_exp(rt, exp, log, tick);
    }
    // FUN_1403dc170: counts scaled by Jackpot, munny converted by Drive
    // Converter, three item rolls with Lucky Lucky.
    let ab = &player.abilities;
    let jackpot = 1. + ab.jackpot;
    let converter = ab.drive_converter;
    let at = rt.enemy.position;
    let floor = world
        .floor(at[0], at[2], at[1] + 30.)
        .unwrap_or(at[1].max(0.));
    for (kind, n) in enemy_prizes::drops(&rt.prize.counts, jackpot, converter) {
        for _ in 0..n {
            let r = [
                rt.rng.range(0., 1.),
                rt.rng.range(0., 1.),
                rt.rng.range(0., 1.),
            ];
            rt.orbs
                .push(Orb::spawn(kind, [at[0], floor.max(at[1]) + 20., at[2]], r));
        }
    }
    let luck = 1. + ab.lucky_lucky;
    for (item, chance) in rt.prize.items {
        if item != 0 && rt.rng.range(0., 100.) < chance as f32 * luck {
            log_event(
                log,
                serde_json::json!({"event":"item_drop","tick":tick,"item":item}),
            );
        }
    }
}

/// EXP into Sora's total, then each level gained (FUN_140410af0's level
/// loop). The debug menu's level-up uses it too.
pub fn grant_exp(rt: &mut EnemyRuntime, exp: u32, log: &mut Option<std::fs::File>, tick: u64) {
    if exp == 0 {
        return;
    }
    rt.exp_total = (rt.exp_total + exp).min(9_999_999);
    let before = rt.level;
    let levels = &rt.sora_levels;
    let after = enemy_prizes::level_for(rt.exp_total, before, |l| {
        levels.get(l as usize - 1).map(|r| r.exp)
    });
    for l in before + 1..=after {
        // Each level's row sets Strength, Magic, Defence and AP; the
        // room's equipment bonuses stay on top.
        let (old, new) = (levels[l as usize - 2], levels[l as usize - 1]);
        rt.sora.strength += new.strength as i32 - old.strength as i32;
        rt.sora.magic += new.magic as i32 - old.magic as i32;
        rt.sora_target.defense += new.defense as i32 - old.defense as i32;
        // [I] the first growth choice's ability.
        let learned = new.abilities[0];
        if learned != 0 {
            rt.learned.push(learned);
        }
        log_event(
            log,
            serde_json::json!({"event":"level_up","tick":tick,"level":l,
                "strength":rt.sora.strength,"magic":rt.sora.magic,
                "defense":rt.sora_target.defense,"learned":learned}),
        );
    }
    if after != before {
        rt.level = after;
        rt.numbers_dirty = true;
    }
    log_event(
        log,
        serde_json::json!({"event":"exp","tick":tick,"gain":exp,"total":rt.exp_total,
            "level":rt.level}),
    );
}
