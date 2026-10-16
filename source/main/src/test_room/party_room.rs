//! Donald and Goofy in the test room: their models, staff and shield, and
//! motions found by name in the installed archives, the behaviour of
//! `player::party_members`, the hits their weapon windows land on the enemy,
//! and the mesh presentation.
//!
//! Built: standing, walking, running, the two ground attacks and Donald's
//! four spells (`player::party_spell`: Fire, Blizzard, Thunder and Cure
//! with their effect elements, volumes, MP and heal). Air attacks, items,
//! KO and revive, Goofy's spells and Limits follow the specs under
//! `party-01`.
use super::*;
use data::path_lookup::Library;
use player::party_members::{self, Follower, Target};
use player::party_spell;
use std::sync::Arc;

/// The motion ids loaded for a member: idle, walk, run and the two ground
/// attacks.
const MOTIONS: [u16; 5] = [0, 1, 2, 151, 152];

/// One motion, sampled per tick (60 Hz).
pub struct MotionData {
    /// Model vertices with the root removed.
    pub frames: Vec<Vec<Vec3>>,
    /// The weapon's vertices in model space (empty when it has no motion).
    pub weapon: Vec<Vec<Vec3>>,
    /// The weapon bone 2 matrix in model space (attack spheres).
    pub blade: Vec<Mat4>,
    /// Root translation per tick (model space).
    pub root: Vec<Vec3>,
    /// Weapon range 10: (start, end, ATKP id, collision group).
    pub hits: Vec<(u32, u32, u16, u8)>,
    pub loop_start: u32,
    /// Frame trigger 9 (the spell release, `ctl+0x155` bit 0).
    pub release: Option<u32>,
    /// The staff's top as a bone matrix (weapon slot 0 top,
    /// FUN_1403b5450): bone 2's rotation at the weapon vertex farthest from
    /// its origin [I: the Keyblade rule of `spell_fx`].
    pub staff: Vec<Mat4>,
}

impl MotionData {
    pub fn end(&self) -> u32 {
        self.frames.len().saturating_sub(1) as u32
    }

    pub fn frame_of(&self, tick: u32) -> usize {
        let end = self.end();
        let t = if tick <= end || self.loop_start >= end {
            tick.min(end)
        } else {
            self.loop_start + (tick - self.loop_start) % (end - self.loop_start)
        };
        t as usize
    }
}

/// A member's installed model, weapon and motions.
pub struct PartyKind {
    pub name: &'static str,
    pub model: model_inspection::InspectionModel,
    pub weapon: Option<model_inspection::InspectionModel>,
    pub motions: HashMap<u16, MotionData>,
    /// The weapon's attack spheres by group: offset and radius (bone 2).
    pub spheres: HashMap<u8, Vec<(Vec3, f32)>>,
    /// Strength and Magic from the character's LVUP row at level 1.
    pub strength: i32,
    pub magic: i32,
    /// Starting HP and MP (00battle `plrp`, the new-game row).
    pub hp: i32,
    pub mp: i32,
    pub records: &'static [party_members::Record],
    /// The member's own effects (`obj/<name>.a.us`) and the heights of its
    /// type-1 and type-9 collision centres (bone codes 0x4001, 0x4009).
    pub pax: Option<Arc<data::object_effect::Pax>>,
    pub body_lift: f32,
    pub heal_lift: f32,
}

/// A member in the room.
pub struct MemberRt {
    pub kind: Arc<PartyKind>,
    pub follower: Follower,
    /// The game tick the current attack started, for hit instance keys.
    pub started: u64,
    /// Absorbed by a Drive form (FUN_140400b20): out of the field until
    /// the revert brings it back (drive_room).
    pub stored: bool,
    /// Current and maximum HP, and MP with its charge (FUN_1403d3cf0).
    pub hp: i32,
    pub max_hp: i32,
    pub mp: player::magic::Mp,
    /// The spell act running (the act manager, obj+0x390) and the number
    /// of casts so far (keys of its effects and volumes).
    pub cast: Option<party_spell::Cast>,
    pub casts: u64,
    /// A spell attack began this tick and has no act yet.
    pub armed: bool,
    /// A cast the tour orders: record and heal slot (taken by the next
    /// tick).
    pub forced: Option<(party_members::Record, u8)>,
}

fn matrix(m: &[u32; 16], root: Vec3) -> Mat4 {
    let mut m = Mat4::from_cols_array(&m.map(f32::from_bits));
    m.w_axis -= root.extend(0.);
    m
}

/// Samples one motion (and the weapon's) at every tick.
fn build_motion(
    model: &model_inspection::InspectionModel,
    weapon: Option<&model_inspection::InspectionModel>,
    hand_bone: usize,
    selected: &motion::SelectedMotion,
    own: Option<&motion::SelectedMotion>,
) -> Result<MotionData, Box<dyn std::error::Error>> {
    let (frame_start, frame_end, fps, frame_return) = selected.motion.frame_window()?;
    if !(1. ..=60.).contains(&fps) || !(0. ..frame_end).contains(&frame_start) {
        return Err("party motion frame window outside profile".into());
    }
    let scale = 60. / fps;
    let start = (frame_start * scale).round() as u32;
    let end = (frame_end * scale).round() as u32 - start;
    let loop_start = ((frame_return * scale).round() as u32).saturating_sub(start);
    let mut data = MotionData {
        frames: Vec::new(),
        weapon: Vec::new(),
        blade: Vec::new(),
        root: Vec::new(),
        hits: Vec::new(),
        loop_start,
        release: selected
            .triggers
            .iter()
            .flat_map(|t| &t.records.frames)
            .filter(|f| f.trigger == 9)
            .map(|f| f.frame.max(0) as u32)
            .min(),
        staff: Vec::new(),
    };
    if let Some(own) = own {
        data.hits = own
            .triggers
            .iter()
            .flat_map(|t| &t.records.ranges)
            .filter(|r| r.trigger == 10 && r.parameters.len() >= 2)
            .map(|r| {
                (
                    r.start_frame.max(0) as u32,
                    if r.end_frame < 0 {
                        end
                    } else {
                        r.end_frame as u32
                    },
                    r.parameters[0] as u16,
                    r.parameters[1] as u8,
                )
            })
            .collect();
    }
    for offset in 0..=end {
        let key = frame_start + offset as f32 / scale;
        let (pose, evaluated) = model.skinning.sample_evaluated(&selected.motion, key)?;
        let r = Vec3::from_array(evaluated.root_translation);
        data.root.push(r);
        data.frames.push(
            pose.position_bits
                .iter()
                .map(|p| Vec3::from_array(p.map(f32::from_bits)) - r)
                .collect(),
        );
        let hand = pose
            .matrix_bits
            .get(hand_bone)
            .map_or(Mat4::IDENTITY, |m| matrix(m, r));
        if let (Some(w), Some(own)) = (weapon, own) {
            let (wp, _) = w.skinning.sample_evaluated(&own.motion, key)?;
            data.weapon.push(
                wp.position_bits
                    .iter()
                    .map(|p| hand.transform_point3(Vec3::from_array(p.map(f32::from_bits))))
                    .collect(),
            );
            let blade = wp.matrix_bits.get(2).map_or(hand, |m| {
                hand * Mat4::from_cols_array(&m.map(f32::from_bits))
            });
            data.blade.push(blade);
            let weapon_now = data.weapon.last().expect("pushed above");
            let top = weapon_now
                .iter()
                .copied()
                .max_by(|a, b| {
                    a.distance_squared(blade.w_axis.truncate())
                        .total_cmp(&b.distance_squared(blade.w_axis.truncate()))
                })
                .unwrap_or(blade.w_axis.truncate());
            data.staff.push(Mat4::from_cols(
                blade.x_axis,
                blade.y_axis,
                blade.z_axis,
                top.extend(1.),
            ));
        }
    }
    Ok(data)
}

/// Loads `obj/<name>.mdlx`, `<name>.mset` and the weapon `W_<rest>` by name.
#[allow(clippy::too_many_arguments)]
pub fn load(
    source: &ProbeSource,
    library: &mut Library,
    system: &[u8],
    lvup: &[u8],
    plrp: &[u8],
    name: &'static str,
    character: usize,
    records: &'static [party_members::Record],
) -> Result<PartyKind, Box<dyn std::error::Error>> {
    let found = library
        .find(&format!("obj/{name}.mdlx"))
        .ok_or_else(|| format!("obj/{name}.mdlx is not installed"))?;
    let mut models =
        model_inspection::read_model_at(source, found.index, found.ordinal, found.name_hash)?;
    if models.is_empty() {
        return Err("party model has no mesh".into());
    }
    let model = models.remove(0);
    let selection = |n: &str| -> Result<motion::MotionSetSelection, Box<dyn std::error::Error>> {
        let set = library
            .find(&format!("obj/{n}.mset"))
            .ok_or_else(|| format!("obj/{n}.mset is not installed"))?;
        Ok(motion::MotionSetSelection {
            index: set.index.to_string(),
            ordinal: set.ordinal,
            name_hash: set.name_hash,
            battle: true,
            has_weapon: true,
        })
    };
    // The base motions plus the cast motions of the member's spells.
    let mut wanted: Vec<u16> = MOTIONS.to_vec();
    for r in records.iter().filter(|r| r.spell.is_some()) {
        if !wanted.contains(&r.motion) {
            wanted.push(r.motion);
        }
    }
    let actor_motions = motion::read_motion_ids_optional(source, &selection(name)?, &wanted)?;
    // The staff or shield: W_EX020 for P_EX020.
    let weapon_name = format!("W_{}", &name[2..]);
    let weapon_selection = selection(&weapon_name).ok();
    let mut weapon = None;
    let mut spheres: HashMap<u8, Vec<(Vec3, f32)>> = HashMap::new();
    let mut weapon_motions = Vec::new();
    if let Some(w) = library.find(&format!("obj/{weapon_name}.mdlx")) {
        let archive = library.read_found(&w)?;
        let mut models = model_inspection::read_model_at(source, w.index, w.ordinal, w.name_hash)?;
        if !models.is_empty() {
            weapon = Some(models.remove(0));
        }
        if let Ok(entry) = bar_entry_typed(&archive, 23)
            && let Ok(shapes) = object_collision::read(entry)
        {
            for e in shapes
                .iter()
                .filter(|e| e.kind == object_collision::ATTACK && e.bone == 2)
            {
                spheres.entry(e.group).or_default().push((
                    Vec3::new(
                        e.position[0] as f32,
                        e.position[1] as f32,
                        e.position[2] as f32,
                    ),
                    e.radius as f32,
                ));
            }
        }
        if let Some(sel) = &weapon_selection {
            weapon_motions = motion::read_motion_ids_optional(source, sel, &wanted)?;
        }
    }
    // The hand joint: the character's SKLT primary bone.
    let sk = sklt::read(bar_entry(system, b"sklt")?)?;
    let hand_bone = sk
        .iter()
        .find(|e| e.character_id as usize == character)
        .and_then(|e| usize::try_from(e.primary_bone).ok())
        .ok_or("SKLT entry for the party member absent")?;
    let mut motions = HashMap::new();
    for (i, selected) in actor_motions.iter().enumerate() {
        let Some(selected) = selected else {
            if MOTIONS.contains(&wanted[i]) {
                return Err(format!("{name}: motion {} is not in its set", wanted[i]).into());
            }
            continue;
        };
        let data = build_motion(
            &model,
            weapon.as_ref(),
            hand_bone,
            selected,
            weapon_motions.get(i).and_then(|m| m.as_ref()),
        )?;
        motions.insert(wanted[i], data);
    }
    // The member's own effects and collision heights.
    let pax = library
        .read(&format!("obj/{name}.a.us"))
        .ok()
        .and_then(|bytes| {
            let table = data::bar::read_bar(&bytes).ok()?;
            let e = table
                .entries
                .iter()
                .find(|e| e.entry_type == 18 && e.size > 0)?;
            data::object_effect::read_pax(
                bytes.get(e.offset as usize..e.offset as usize + e.size as usize)?,
            )
            .ok()
        })
        .map(Arc::new);
    let (mut body_lift, mut heal_lift) = (85f32, 105f32);
    if let Some(found) = library.find(&format!("obj/{name}.mdlx"))
        && let Ok(archive) = library.read_found(&found)
        && let Ok(entry) = bar_entry_typed(&archive, 23)
        && let Ok(shapes) = object_collision::read(entry)
    {
        if let Some(c) = shapes.iter().find(|c| c.kind == 1 && c.group == 0) {
            body_lift = c.position_height as f32;
        }
        if let Some(c) = shapes.iter().find(|c| c.kind == 9 && c.group == 0) {
            heal_lift = c.position_height as f32;
        }
    }
    let row = data::battle_tables::player_level(lvup, character, 1)?;
    let (hp, mp) = visual::hud::starting_stats_of(plrp, character as u8)?;
    Ok(PartyKind {
        name,
        model,
        weapon,
        motions,
        spheres,
        strength: row.strength as i32,
        magic: row.magic as i32,
        hp,
        mp,
        records,
        pax,
        body_lift,
        heal_lift,
    })
}

/// Marks a member's root and its mesh parts.
#[derive(Component)]
pub struct PartyRoot(pub usize);
#[derive(Component)]
pub struct PartyPart {
    pub member: usize,
    pub ordinal: usize,
    pub weapon: bool,
}

/// Instance keys of party attack windows live above Sora's and the spells'.
const KEY_BASE: u64 = 1 << 61;
/// Effect handles of the casts (`player::party_spell` keys).
const FX_BASE: u64 = 1 << 60;
/// Donald's MP Charge length (`prty[7]`, 1200).
const CHARGE: f32 = 1200.;

/// The enemy's HIT spheres at its current motion tick: centre, radius and
/// whether the shape is a high one.
fn enemy_bodies(rt: &EnemyRuntime) -> Vec<(Vec3, f32, bool)> {
    let enemy_at = Vec3::from_array(rt.enemy.position);
    let enemy_turn = Quat::from_rotation_y(rt.enemy.yaw);
    let ec = &rt.clips.0[&rt.enemy.motion];
    let bones = &ec.bones[(rt.enemy.motion_tick as usize).min(ec.bones.len() - 1)];
    rt.body
        .iter()
        .filter_map(|&(b, offset, r, high)| {
            let mat = bones.get(&b)?;
            Some((
                enemy_at + enemy_turn * mat.transform_point3(offset),
                r,
                high,
            ))
        })
        .collect()
}

/// `target_pos(btlobj_target)`: the centre of the enemy's first TARGET
/// shape, else above its origin.
fn enemy_centre(rt: &EnemyRuntime) -> Vec3 {
    let origin = Vec3::from_array(rt.enemy.position);
    let turn = Quat::from_rotation_y(rt.enemy.yaw);
    let shape = rt.targets.first().and_then(|&(bone, offset, _, _)| {
        let ec = rt.clips.0.get(&rt.enemy.motion)?;
        let bones = ec
            .bones
            .get((rt.enemy.motion_tick as usize).min(ec.bones.len().saturating_sub(1)))?;
        Some(origin + turn * bones.get(&bone)?.transform_point3(offset))
    });
    shape.unwrap_or(origin + Vec3::Y * 70.)
}

/// A member's attacker terms: Strength and Magic of its level row, a
/// player-type owner.
fn attacker(kind: &PartyKind) -> combat::AttackerStats {
    combat::AttackerStats {
        strength: kind.strength,
        magic: kind.magic,
        max: 99999,
        min: 1,
        mods: combat::AttackerMods {
            player_type: true,
            ..combat::AttackerMods::default()
        },
    }
}

/// Starts a cast the tour orders: the spell's act against the enemy, on the
/// next tick. Returns false when the member cannot start it.
pub fn order_cast(game: &mut Game, member: usize, spell: party_members::Spell, heal: u8) -> bool {
    let Some(m) = game.party.get_mut(member) else {
        return false;
    };
    let Some(record) = m
        .kind
        .records
        .iter()
        .find(|r| r.spell == Some(spell))
        .copied()
    else {
        return false;
    };
    if m.stored {
        return false;
    }
    // It starts as soon as the member is free; no new decision is made
    // meanwhile.
    m.forced = Some((record, heal));
    true
}

/// One volume of a cast against the enemy's body: the hit record, the
/// ATKP step row and the damage with the caster's stats.
#[allow(clippy::too_many_arguments)]
fn land_volume(
    rt: &mut EnemyRuntime,
    name: &str,
    stats: &combat::AttackerStats,
    v: &player::magic::Volume,
    key: u64,
    facing: Vec3,
    tick: u64,
    sound_queue: &mut Vec<(u32, &'static str)>,
    log: &mut Option<File>,
) {
    if !rt.enabled || !rt.enemy.targetable() || !ensure_attack_rows(rt, v.atkp, v.sub) {
        return;
    }
    let bodies = enemy_bodies(rt);
    rt.instance_seen.insert(key, tick);
    let (aid, grp) = *rt.instance_ids.entry(key).or_insert_with(|| {
        let combo = rt
            .steps
            .get(&(v.atkp, v.sub))
            .map_or(0, |r| r[0].combo_group as i32);
        (
            rt.ids.allocate(),
            if combo != 0 {
                combo
            } else {
                combat::allocate_group(&mut rt.next_group)
            },
        )
    });
    if rt.enemy.record.has(aid) {
        return;
    }
    let mut hit = None;
    for c in combat::swept_volume(v.centre, v.previous, v.radius, v.height) {
        if hit.is_none()
            && let Some(&(_, _, high)) = bodies.iter().find(|(b, r, _)| {
                combat::volume_hits_sphere(c, v.radius, v.height, b.to_array(), *r)
            })
        {
            hit = Some((Vec3::from_array(c), high));
        }
    }
    let Some((at, high)) = hit else { return };
    let rows = rt.steps[&(v.atkp, v.sub)].clone();
    let cure = matches!(rows[0].kind, 5 | 6);
    let (counted, row) = rt.enemy.record.hit(aid, grp, cure, |count| {
        let r = data::battle_tables::step_index(&rows, count);
        (r, rows[r].interval)
    });
    if !counted {
        return;
    }
    let attack = params_of(&rows[row]);
    let enemy_at = Vec3::from_array(rt.enemy.position);
    let dir = match v.moving {
        Some(m) => Vec3::from_array(m),
        None => enemy_at - at,
    } * Vec3::new(1., 0., 1.);
    let dir = dir.normalize_or(facing);
    if let Some(damage) = rt
        .enemy
        .hit(dir.to_array(), high, &attack, stats, rt.difficulty)
    {
        if let Some(&id) = rt.hit_sounds.get(&v.atkp)
            && id >= 0
        {
            sound_queue.push((id as u32, "party_hit"));
        }
        log_event(
            log,
            serde_json::json!({"event":"party_hit","tick":tick,"member":name,
            "atkp":v.atkp,"step":row,"damage":damage,"enemy_hp":rt.enemy.hp}),
        );
    }
}

fn log_event(log: &mut Option<File>, record: serde_json::Value) {
    if let Some(file) = log.as_mut() {
        let _ = serde_json::to_writer(&mut *file, &record);
        let _ = std::io::Write::write_all(file, b"\n");
    }
}

/// One tick of every member: behaviour, the spell acts, then the hits their
/// weapons and spells land.
pub fn tick(game: &mut Game, tick: u64) {
    let Game {
        player,
        params,
        world,
        party,
        enemy,
        log,
        sound_queue,
        fx_calls,
        fx_live,
        magic_tour,
        magic_shift,
        party_spells,
        ..
    } = game;
    let body = player::Body {
        radius: params.radius,
        height: params.height,
        step: params.step_height,
    };
    let target = enemy
        .as_ref()
        .filter(|rt| rt.enabled && rt.enemy.targetable())
        .map(|rt| Target {
            position: rt.enemy.position,
            radius: 40.,
        });
    let centre = enemy
        .as_ref()
        .filter(|rt| rt.enabled && rt.enemy.targetable())
        .map(|rt| enemy_centre(rt).to_array());
    let hp: [Option<(i32, i32)>; 3] = {
        let sora = enemy.as_ref().map(|rt| (rt.sora_hp, rt.sora_target.max_hp));
        let mut hp = [sora, None, None];
        for (i, m) in party.iter().enumerate().take(2) {
            hp[i + 1] = Some((m.hp, m.max_hp));
        }
        hp
    };
    // The tour orders its casts; the AI casts on its own elsewhere.
    let spells =
        *party_spells && (!*magic_tour || super::magic_tour::free_party(tick, *magic_shift));
    // Heals the acts land this tick: party slot, ATKP id, healer index.
    let mut heals: Vec<(usize, u16, usize)> = Vec::new();
    for (index, m) in party.iter_mut().enumerate() {
        if m.stored {
            continue;
        }
        m.mp.tick(1., 1.);
        let kind = m.kind.clone();
        let length = |id: u16| kind.motions.get(&id).map(|d| d.end() + 1);
        let root = |id: u16, t: u32| {
            kind.motions
                .get(&id)
                .and_then(|d| d.root.get(t as usize))
                .map_or([0.; 3], |r| r.to_array())
        };
        let scene = party_members::Scene {
            leader: player.position,
            leader_yaw: player.yaw,
            target,
            world: &*world,
            body,
            length: &length,
            root: &root,
            busy: m.cast.is_some() || m.forced.is_some(),
            mp: Some((m.mp.cur, m.mp.max, m.mp.charging())),
            spells,
            hp,
        };
        let was = m.follower.attack.is_some();
        m.follower.tick(kind.records, &scene);
        // The tour's order starts the act as the AI's pick would.
        if m.follower.attack.is_none()
            && m.cast.is_none()
            && let Some((record, heal)) = m.forced.take()
        {
            // With no enemy to face the act goes straight ahead.
            let ahead = Target {
                position: [
                    m.follower.position[0] + m.follower.yaw.sin() * 300.,
                    m.follower.position[1],
                    m.follower.position[2] + m.follower.yaw.cos() * 300.,
                ],
                radius: 40.,
            };
            m.follower.begin(&record, &target.unwrap_or(ahead), heal);
            m.started = tick;
            m.armed = true;
        }
        let now = m.follower.attack;
        if !was
            && !m.armed
            && let Some(a) = now
        {
            m.started = tick;
            m.armed = a.spell.is_some();
        }
        // The act manager starts the spell's act (FUN_1403caa90).
        if m.armed
            && let Some(a) = now
            && let Some(spell) = a.spell
        {
            m.armed = false;
            m.casts += 1;
            let release = kind
                .motions
                .get(&a.motion)
                .and_then(|d| d.release)
                .unwrap_or(26);
            let key = FX_BASE | (index as u64) << 40 | m.casts << 8;
            m.cast = Some(party_spell::Cast::new(
                spell,
                index + 1,
                a.heal as usize,
                key,
                release,
            ));
            log_event(
                log,
                serde_json::json!({"event":"party_cast","tick":tick,"member":kind.name,
                "spell":format!("{spell:?}"),"motion":a.motion,"release":release,
                "motion_end":kind.motions.get(&a.motion).map(|d| d.end()),
                "heal":a.heal,"mp":m.mp.cur,"charging":m.mp.charging()}),
            );
        }
        if let Some(cast) = m.cast.as_mut() {
            let pos = m.follower.position;
            let facing = [m.follower.yaw.sin(), m.follower.yaw.cos()];
            let ground = |from: [f32; 3], max: f32| -> Option<[f32; 3]> {
                world
                    .floor(from[0], from[2], from[1])
                    .filter(|h| from[1] - h <= max)
                    .map(|h| [from[0], h, from[2]])
            };
            let bg = |p: [f32; 3], _r: f32| -> bool {
                world.floor(p[0], p[2], p[1]).is_none() || {
                    let w = world.resolve_walls([p[0], p[2]], 1., p[1] - 1., p[1] + 1.);
                    (w[0] - p[0]).abs() > 1e-3 || (w[1] - p[2]).abs() > 1e-3
                }
            };
            let effect = |k: u64| fx_live.get(&k).copied();
            let ctx = party_spell::Context {
                caster: pos,
                facing,
                centre: [pos[0], pos[1] + kind.body_lift, pos[2]],
                target: centre,
                effect: &effect,
                ground: &ground,
                bg_hit: &bg,
            };
            let out = cast.tick(&ctx);
            let spell = cast.spell;
            if cast.finished() {
                m.cast = None;
            }
            for fx in out.fx {
                let pax = match &fx {
                    player::magic::Fx::Start { .. } => kind.pax.clone(),
                    _ => None,
                };
                fx_calls.push(spell_fx::Call { pax, fx });
            }
            if let Some(cost) = out.spend {
                m.mp.change(-(cost as i32), CHARGE);
                log_event(
                    log,
                    serde_json::json!({"event":"party_release","tick":tick,"member":kind.name,
                    "spell":format!("{spell:?}"),"mp":m.mp.cur,"charging":m.mp.charging()}),
                );
            }
            for strike in out.strikes {
                heals.push((strike.party, strike.atkp, index));
            }
            if let Some(rt) = enemy.as_mut() {
                let stats = attacker(&kind);
                for v in &out.volumes {
                    let key = KEY_BASE | 1 << 47 | (v.key & 0x7fff_ffff_ffff);
                    land_volume(
                        rt,
                        kind.name,
                        &stats,
                        v,
                        key,
                        Vec3::new(facing[0], 0., facing[1]),
                        tick,
                        sound_queue,
                        log,
                    );
                }
            }
        }
        // The weapon's attack windows against the enemy's body.
        let (Some(a), Some(rt)) = (m.follower.attack, enemy.as_mut()) else {
            continue;
        };
        let Some(data) = kind.motions.get(&a.motion) else {
            continue;
        };
        if !rt.enabled || !rt.enemy.targetable() || data.blade.is_empty() {
            continue;
        }
        let f = (a.tick as usize).min(data.blade.len() - 1);
        let turn = Quat::from_rotation_y(m.follower.yaw);
        let at = Vec3::from_array(m.follower.position);
        let enemy_at = Vec3::from_array(rt.enemy.position);
        let bodies = enemy_bodies(rt);
        for (w, &(start, end, atkp, group)) in data.hits.iter().enumerate() {
            if a.tick < start || a.tick > end {
                continue;
            }
            let Some(list) = kind.spheres.get(&group) else {
                continue;
            };
            let key = KEY_BASE | (index as u64) << 48 | m.started << 4 | w as u64;
            rt.instance_seen.insert(key, tick);
            let (aid, grp) = *rt.instance_ids.entry(key).or_insert_with(|| {
                (
                    rt.ids.allocate(),
                    combat::allocate_group(&mut rt.next_group),
                )
            });
            if rt.enemy.record.has(aid) || !ensure_attack_rows(rt, atkp, 0) {
                continue;
            }
            let mut hit = None;
            for &(offset, radius) in list {
                let c = at + turn * data.blade[f].transform_point3(offset);
                if let Some(&(_, _, high)) = bodies.iter().find(|(b, r, _)| {
                    combat::spheres_overlap(c.to_array(), radius, b.to_array(), *r)
                }) {
                    hit = Some(high);
                    break;
                }
            }
            let Some(high) = hit else {
                continue;
            };
            let rows = rt.steps[&(atkp, 0)].clone();
            let cure = matches!(rows[0].kind, 5 | 6);
            let (counted, row) = rt.enemy.record.hit(aid, grp, cure, |count| {
                let r = data::battle_tables::step_index(&rows, count);
                (r, rows[r].interval)
            });
            if !counted {
                continue;
            }
            let attack = params_of(&rows[row]);
            let dir = ((enemy_at - at) * Vec3::new(1., 0., 1.)).normalize_or(Vec3::new(
                m.follower.yaw.sin(),
                0.,
                m.follower.yaw.cos(),
            ));
            let stats = attacker(&kind);
            if let Some(damage) = rt
                .enemy
                .hit(dir.to_array(), high, &attack, &stats, rt.difficulty)
            {
                if let Some(&id) = rt.hit_sounds.get(&atkp)
                    && id >= 0
                {
                    sound_queue.push((id as u32, "party_hit"));
                }
                log_event(
                    log,
                    serde_json::json!({"event":"party_hit","tick":tick,
                    "member":kind.name,"atkp":atkp,"damage":damage,"enemy_hp":rt.enemy.hp}),
                );
            }
        }
    }
    // attack_strike on the chosen member: Cure's heal is an ATKP Type 6 row,
    // (Power x Magic + 99) / 100 with the healer's Magic, clamped to the
    // maximum HP.
    for (slot, atkp, healer) in heals {
        let Some((stats, healer_name)) =
            party.get(healer).map(|m| (attacker(&m.kind), m.kind.name))
        else {
            continue;
        };
        let Some(rt) = enemy.as_mut() else { continue };
        if !ensure_attack_rows(rt, atkp, 0) {
            continue;
        }
        let attack = params_of(&rt.steps[&(atkp, 0)][0]);
        let full = combat::TargetState::full(rt.sora_target.max_hp);
        let amount = combat::damage_with(&stats, &attack, &rt.sora_target, &full);
        let record = if slot == 0 {
            rt.sora_hp = (rt.sora_hp + amount).min(rt.sora_target.max_hp);
            serde_json::json!({"event":"party_heal","tick":tick,"healer":healer_name,
                "atkp":atkp,"amount":amount,"target":"Sora","hp":rt.sora_hp})
        } else if let Some(member) = party.get_mut(slot - 1) {
            member.hp = (member.hp + amount).min(member.max_hp);
            serde_json::json!({"event":"party_heal","tick":tick,"healer":healer_name,
                "atkp":atkp,"amount":amount,"target":member.kind.name,"hp":member.hp})
        } else {
            continue;
        };
        log_event(log, record);
    }
}

/// Places and poses the members.
pub fn present(
    game: Res<Game>,
    mut roots: Query<(&PartyRoot, &mut Transform)>,
    parts: Query<(&PartyPart, &Mesh3d)>,
    mut meshes: ResMut<Assets<Mesh>>,
) {
    for (root, mut t) in &mut roots {
        if let Some(m) = game.party.get(root.0) {
            t.translation = Vec3::from_array(m.follower.position);
            t.rotation = Quat::from_rotation_y(m.follower.yaw);
        }
    }
    for (part, handle) in &parts {
        let Some(m) = game.party.get(part.member) else {
            continue;
        };
        let Some(data) = m.kind.motions.get(&m.follower.motion()) else {
            continue;
        };
        let f = data.frame_of(m.follower.tick);
        let (frame, model) = if part.weapon {
            // A motion without its own weapon pose keeps the idle one.
            let (source, at) = if data.weapon.is_empty() {
                (m.kind.motions.get(&0), 0)
            } else {
                (Some(data), f)
            };
            let (Some(source), Some(weapon)) = (source, &m.kind.weapon) else {
                continue;
            };
            let Some(frame) = source.weapon.get(at) else {
                continue;
            };
            (frame, weapon)
        } else {
            (&data.frames[f], &m.kind.model)
        };
        let positions: Vec<[f32; 3]> = model.parts[part.ordinal]
            .indices
            .iter()
            .map(|&i| frame[i as usize].to_array())
            .collect();
        if let Some(mut mesh) = meshes.get_mut(&handle.0) {
            mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, positions);
        }
    }
}
