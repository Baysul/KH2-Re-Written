//! Sora's magic in the test room: the owned spells, the cast command
//! (`player_sim::magic`), the running spells' volumes and what they do to
//! the Shadow and to Sora (heals, recoil, Leaf Bracer).
//!
//! Spell attacks go through the same hit pipeline as the Keyblade (attack
//! ids, hit records, ATKP level rows by sub-table) in `combat_tick`; this
//! module only owns the casts and the volumes they emit.
use super::*;
use player_sim::magic::{self, Feedback, Spell};

/// The room's world id for `magc` record selection (a record applies in
/// world 0 or its own; Pride Lands' 0x0a variants are not used here).
const WORLD: u8 = 2;
/// Attack-instance keys of spell volumes live above the weapon windows'
/// `(action_started << 7) | hand << 5 | window` ids.
pub const KEY_BASE: u64 = 1 << 62;
/// Sound id of the "cannot" cue (system bank 5, command-01 §6.3).
const SE_ERROR: u32 = 5;

/// 03system and 00battle data of the six spells.
pub struct MagicSource {
    pub records: Vec<magic::Record>,
    pub rows: Vec<magic::Row>,
    pub commands: HashMap<u16, package_reader::command_table::Command>,
    /// The spell commands' names (system-font strings) by command id.
    pub names: HashMap<u16, Vec<u8>>,
    /// Sora's TARGET shape lift above his origin (Blizzard starts there).
    pub target_lift: f32,
}

/// `magc` (00battle), `pref/magi` and the `cmd` table.
pub fn load(
    battle: &[u8],
    pref: &[u8],
    system: &[u8],
    target_lift: f32,
) -> Result<MagicSource, Box<dyn std::error::Error>> {
    let magc = bar_entry(battle, b"magc")?;
    let count = magc
        .get(4..8)
        .map(|w| u32::from_le_bytes(w.try_into().unwrap()) as usize)
        .ok_or("magc truncated")?;
    let records = (0..count)
        .map(|k| {
            magc.get(8 + k * magic::Record::SIZE..)
                .and_then(magic::Record::read)
                .ok_or("magc record truncated")
        })
        .collect::<Result<Vec<_>, _>>()?;
    // `pref/magi`: i32 count, then one offset per row; an empty offset or
    // an index past the count reads row 0 (FUN_1403e9560).
    let magi = bar_entry(pref, b"magi")?;
    let n = magi
        .get(0..4)
        .map(|w| u32::from_le_bytes(w.try_into().unwrap()) as usize)
        .ok_or("magi truncated")?;
    let mut rows = Vec::new();
    for i in 0..n.min(64) {
        let off = magi
            .get(4 + i * 4..8 + i * 4)
            .map(|w| u32::from_le_bytes(w.try_into().unwrap()) as usize)
            .ok_or("magi truncated")?;
        let row = if off == 0 {
            None
        } else {
            magi.get(off..).and_then(magic::Row::read)
        };
        rows.push(row);
    }
    let first = rows.first().copied().flatten().unwrap_or_default();
    let rows = rows.into_iter().map(|r| r.unwrap_or(first)).collect();
    let commands = package_reader::command_table::read(bar_entry(system, b"cmd\0")?)?
        .into_iter()
        .map(|c| (c.id, c))
        .collect();
    Ok(MagicSource {
        records,
        rows,
        commands,
        names: HashMap::new(),
        target_lift,
    })
}

/// The cast motion ids the records use (world 0 only), without
/// repeats.
pub fn motion_ids(records: &[magic::Record]) -> Vec<u16> {
    let mut out: Vec<u16> = Vec::new();
    for r in records.iter().filter(|r| r.world == 0) {
        for m in [r.ground.0, r.finisher.0, r.air.0] {
            if m >= 0 && !out.contains(&(m as u16)) {
                out.push(m as u16);
            }
        }
    }
    out
}

/// Live magic state for the session.
pub struct MagicRt {
    pub src: MagicSource,
    /// Owned element items by magic type (the level).
    pub levels: [u8; 6],
    /// Camp Customize's "Don't MP Charge" bits by magic type.
    pub dont_charge: u8,
    pub spells: Vec<Spell>,
    pub serial: u64,
    /// What the last tick's collisions told the spells.
    pub feedback: Feedback,
    /// Clash counts of guard volumes by key, kept across ticks.
    pub reflected: HashMap<u64, u32>,
    /// This tick's volumes, for `combat_tick`.
    pub volumes: Vec<magic::Volume>,
    /// The volume keys that hit last tick (Wisdom's shots read them).
    pub last_hits: Vec<u64>,
    /// The open Magic submenu's rows, by magic type.
    pub menu_types: Vec<u8>,
}

impl MagicRt {
    pub fn new(src: MagicSource, levels: [u8; 6]) -> Self {
        MagicRt {
            src,
            levels,
            dont_charge: 0,
            spells: Vec::new(),
            serial: 0,
            feedback: Feedback::default(),
            reflected: HashMap::new(),
            volumes: Vec::new(),
            last_hits: Vec::new(),
            menu_types: Vec::new(),
        }
    }

    /// FUN_1403c59d0: the record in force for a magic type.
    pub fn current(&self, magic_type: u8) -> Option<&magic::Record> {
        magic::Record::current(
            &self.src.records,
            magic_type,
            self.levels[magic_type as usize],
            WORLD,
        )
    }

    /// FUN_1403f2910: the Magic submenu's entries in magic-type order, each
    /// with its menu colour.
    pub fn menu_list(&self, mp: &magic::Mp) -> Vec<(u8, usize)> {
        (0..6u8)
            .filter_map(|t| self.colour(t, mp).map(|c| (t, c)))
            .collect()
    }

    /// The menu colour of the spell of `magic_type` (FUN_140401d60).
    pub fn colour(&self, magic_type: u8, mp: &magic::Mp) -> Option<usize> {
        let rec = self.current(magic_type)?;
        let cmd = self.src.commands.get(&rec.command)?;
        Some(magic::colour(
            cmd.cost,
            mp,
            self.dont_charge >> magic_type & 1 != 0,
            false,
        ))
    }
}

/// The cast command for a magic type (menu or shortcut). Returns whether a
/// cast started.
pub fn request(game: &mut Game, magic_type: u8, tick: u64) -> bool {
    let Game {
        magic,
        player,
        clips,
        sound_queue,
        ..
    } = game;
    let Some(m) = magic.as_mut() else {
        return false;
    };
    let Some(rec) = m.current(magic_type).cloned() else {
        return false;
    };
    let Some(cmd) = m.src.commands.get(&rec.command).copied() else {
        return false;
    };
    let colour = magic::colour(
        cmd.cost,
        &player.mp,
        m.dont_charge >> magic_type & 1 != 0,
        false,
    );
    // The command's form mask (cmd +0x2c) is checked against the form 0.
    if colour >= 4 {
        sound_queue.push((SE_ERROR, "magic_denied"));
        return false;
    }
    // Cure on a shortcut targets Sora himself.
    let target_is_self = magic_type == magic::CURE;
    let Some((action, start)) = player.cast(&rec, cmd.cost, 0, target_is_self, clips) else {
        return false;
    };
    // Leaf Bracer (ability 114): Cure on oneself gives 60 ticks of
    // invulnerability and a super guard from the cast's start.
    let leaf = magic_type == magic::CURE && player.abilities.has(114);
    if leaf {
        m.serial += 1;
        let ctx_party = [player.position];
        let ground = |_: [f32; 3], _: f32| None;
        let bg = |_: [f32; 3], _: f32| false;
        let ctx = magic::Context {
            dt: 1.,
            caster: player.position,
            facing: [player.yaw.sin(), player.yaw.cos()],
            caster_centre: [
                player.position[0],
                player.position[1] + m.src.target_lift,
                player.position[2],
            ],
            airborne: false,
            casting: true,
            target: None,
            party: &ctx_party,
            ground: &ground,
            bg_hit: &bg,
        };
        let serial = m.serial;
        m.spells.push(Spell::leaf_bracer(serial, &ctx));
    }
    let mp = player.mp;
    let record = serde_json::json!({"event":"cast","tick":tick,"magic":magic_type,
        "level":rec.level,"command":rec.command,"action":format!("{action:?}"),
        "motion":start.motion,"row":start.row,"finisher":start.finisher,
        "airborne":start.airborne,"mp":mp.cur,"charging":mp.charging()});
    game.action_started = tick;
    game.event(record);
    true
}

/// One tick of the casts: release, then every spell's volumes, heals,
/// recoil and invulnerability.
pub fn tick(game: &mut Game, tick: u64) {
    let mut events: Vec<serde_json::Value> = Vec::new();
    {
        let Game {
            magic,
            player,
            enemy,
            world,
            ..
        } = game;
        let Some(m) = magic.as_mut() else { return };
        let pos = player.position;
        let facing = [player.yaw.sin(), player.yaw.cos()];
        let action = player.action;
        let centre = [pos[0], pos[1] + m.src.target_lift, pos[2]];
        let party = [pos];
        // The locked target's TARGET centre: the enemy while it can be hit.
        let target = enemy
            .as_ref()
            .filter(|rt| rt.enemy.targetable())
            .and_then(|rt| enemy_targets(rt).first().map(|t| t.centre));
        let ground = |from: [f32; 3], max: f32| -> Option<[f32; 3]> {
            world
                .floor(from[0], from[2], from[1])
                .filter(|h| from[1] - h <= max)
                .map(|h| [from[0], h, from[2]])
        };
        // FUN_1403d0bb0 stand-in: the centre left the floor (arena edge) or
        // entered a wall.
        let bg = |p: [f32; 3], _r: f32| -> bool {
            world.floor(p[0], p[2], p[1]).is_none() || {
                let w = world.resolve_walls([p[0], p[2]], 1., p[1] - 1., p[1] + 1.);
                (w[0] - p[0]).abs() > 1e-3 || (w[1] - p[2]).abs() > 1e-3
            }
        };
        let ctx = |airborne: bool, casting: bool| magic::Context {
            dt: 1.,
            caster: pos,
            facing,
            caster_centre: centre,
            airborne,
            casting,
            target,
            party: &party,
            ground: &ground,
            bg_hit: &bg,
        };
        // Feedback from last tick's collisions reaches the spells now.
        m.feedback.reflected = m.reflected.iter().map(|(&k, &c)| (k, c)).collect();
        if let Some(r) = player.take_released() {
            m.serial += 1;
            let row = m
                .src
                .rows
                .get(r.row)
                .or(m.src.rows.first())
                .copied()
                .unwrap_or_default();
            let c = ctx(r.airborne, true);
            m.spells.push(Spell::new(
                m.serial,
                r.magic_type,
                r.level,
                r.row,
                row,
                r.form,
                &c,
            ));
            events.push(serde_json::json!({"event":"spell_release","tick":tick,
                "magic":r.magic_type,"level":r.level,"row":r.row,"finisher":r.finisher,
                "mp":player.mp.cur,"charging":player.mp.charging(),
                "target":target.map(|t| t.to_vec())}));
        }
        let mut volumes = Vec::new();
        let mut strikes = Vec::new();
        let mut recoil = None;
        let mut invulnerable: Option<f32> = None;
        let fb = std::mem::take(&mut m.feedback);
        m.last_hits = fb.hit.clone();
        let airborne = matches!(action, Action::AirCast(_) | Action::AirAttack(_));
        let casting = matches!(action, Action::Cast(_) | Action::AirCast(_));
        for s in m.spells.iter_mut() {
            let out = s.tick(&ctx(airborne, casting), &fb);
            volumes.extend(out.volumes);
            strikes.extend(out.strikes);
            if out.recoil.is_some() {
                recoil = out.recoil;
            }
            if let Some(t) = out.invulnerable {
                invulnerable = Some(invulnerable.map_or(t, |v| v.max(t)));
            }
        }
        m.spells.retain(|s| !s.finished());
        m.volumes = volumes;
        if let Some(strength) = recoil {
            // obj_hitback(S, roty(obj_dir, pi), f7): back along the facing.
            player.recoil([-facing[0], -facing[1]], strength);
        }
        if let Some(rt) = enemy.as_mut() {
            if let Some(t) = invulnerable {
                rt.sora_invulnerable = rt.sora_invulnerable.max(t);
            }
            // attack_strike on the party: Cure's heal is an ATKP Type 6 row
            // (Power x Magic + 99) / 100, added to HP and clamped.
            for st in strikes {
                if st.party != 0 || !ensure_attack_rows(rt, st.atkp, st.sub) {
                    continue;
                }
                let attack = params_of(&rt.steps[&(st.atkp, st.sub)][0]);
                let full = combat::TargetState::full(rt.sora_target.max_hp);
                let heal = combat::damage_with(&rt.sora, &attack, &rt.sora_target, &full);
                rt.sora_hp = (rt.sora_hp + heal).min(rt.sora_target.max_hp);
                events.push(
                    serde_json::json!({"event":"heal","tick":tick,"atkp":st.atkp,
                    "sub":st.sub,"amount":heal,"sora_hp":rt.sora_hp}),
                );
            }
        }
    }
    for e in events {
        game.event(e);
    }
}
