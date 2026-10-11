//! Donald and Goofy in the test room: their models, staff and shield, and
//! motions found by name in the installed archives, the behaviour of
//! `player_sim::party`, the hits their weapon windows land on the enemy,
//! and the mesh presentation.
//!
//! Built: standing, walking, running and the two ground attacks. Air
//! attacks, spells, items, KO and revive, and Limits follow the specs under
//! `party-01`.
use super::*;
use package_reader::named::Library;
use player_sim::party::{self, Follower, Target};
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
}

impl MotionData {
    pub fn end(&self) -> u32 {
        self.frames.len().saturating_sub(1) as u32
    }

    fn frame_of(&self, tick: u32) -> usize {
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
    pub records: &'static [party::Record],
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
            data.blade.push(wp.matrix_bits.get(2).map_or(hand, |m| {
                hand * Mat4::from_cols_array(&m.map(f32::from_bits))
            }));
        }
    }
    Ok(data)
}

/// Loads `obj/<name>.mdlx`, `<name>.mset` and the weapon `W_<rest>` by name.
pub fn load(
    source: &ProbeSource,
    library: &mut Library,
    system: &[u8],
    lvup: &[u8],
    name: &'static str,
    character: usize,
    records: &'static [party::Record],
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
    let actor_motions = motion::read_motion_ids(source, &selection(name)?, &MOTIONS)?;
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
            weapon_motions = motion::read_motion_ids(source, sel, &MOTIONS)?;
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
        let data = build_motion(
            &model,
            weapon.as_ref(),
            hand_bone,
            selected,
            weapon_motions.get(i),
        )?;
        motions.insert(MOTIONS[i], data);
    }
    let row = package_reader::battle_tables::player_level(lvup, character, 1)?;
    Ok(PartyKind {
        name,
        model,
        weapon,
        motions,
        spheres,
        strength: row.strength as i32,
        magic: row.magic as i32,
        records,
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

/// One tick of every member: behaviour, then the hits their weapons land.
pub fn tick(game: &mut Game, tick: u64) {
    let Game {
        player,
        params,
        world,
        party,
        enemy,
        log,
        sound_queue,
        ..
    } = game;
    let body = player_sim::Body {
        radius: params.radius,
        height: params.height,
        step: params.step_height,
    };
    let target = enemy
        .as_ref()
        .filter(|rt| rt.enemy.targetable())
        .map(|rt| Target {
            position: rt.enemy.position,
            radius: 40.,
        });
    for (index, m) in party.iter_mut().enumerate() {
        if m.stored {
            continue;
        }
        let kind = m.kind.clone();
        let length = |id: u16| kind.motions.get(&id).map(|d| d.end() + 1);
        let root = |id: u16, t: u32| {
            kind.motions
                .get(&id)
                .and_then(|d| d.root.get(t as usize))
                .map_or([0.; 3], |r| r.to_array())
        };
        let scene = party::Scene {
            leader: player.position,
            leader_yaw: player.yaw,
            target,
            world: &*world,
            body,
            length: &length,
            root: &root,
        };
        let was = m.follower.attack.is_some();
        m.follower.tick(kind.records, &scene);
        if !was && m.follower.attack.is_some() {
            m.started = tick;
        }
        // The weapon's attack windows against the enemy's body.
        let (Some(a), Some(rt)) = (m.follower.attack, enemy.as_mut()) else {
            continue;
        };
        let Some(data) = kind.motions.get(&a.motion) else {
            continue;
        };
        if !rt.enemy.targetable() || data.blade.is_empty() {
            continue;
        }
        let f = (a.tick as usize).min(data.blade.len() - 1);
        let turn = Quat::from_rotation_y(m.follower.yaw);
        let at = Vec3::from_array(m.follower.position);
        let enemy_at = Vec3::from_array(rt.enemy.position);
        let enemy_turn = Quat::from_rotation_y(rt.enemy.yaw);
        let ec = &rt.clips.0[&rt.enemy.motion];
        let bones = &ec.bones[(rt.enemy.motion_tick as usize).min(ec.bones.len() - 1)];
        let bodies: Vec<(Vec3, f32, bool)> = rt
            .body
            .iter()
            .filter_map(|&(b, offset, r, high)| {
                let mat = bones.get(&b)?;
                Some((
                    enemy_at + enemy_turn * mat.transform_point3(offset),
                    r,
                    high,
                ))
            })
            .collect();
        for (w, &(start, end, atkp, group)) in data.hits.iter().enumerate() {
            if a.tick < start || a.tick > end {
                continue;
            }
            let Some(list) = kind.spheres.get(&group) else {
                continue;
            };
            let key = KEY_BASE | (index as u64) << 48 | m.started << 4 | w as u64;
            let (aid, grp) = *rt.instance_ids.entry(key).or_insert_with(|| {
                rt.next_group += 1;
                (rt.ids.allocate(), rt.next_group)
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
                let r = package_reader::battle_tables::step_index(&rows, count);
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
            let stats = combat::AttackerStats {
                strength: kind.strength,
                magic: kind.magic,
                max: 99999,
                min: 1,
                mods: combat::AttackerMods {
                    player_type: true,
                    ..combat::AttackerMods::default()
                },
            };
            if let Some(damage) = rt
                .enemy
                .hit(dir.to_array(), high, &attack, &stats, rt.difficulty)
            {
                if let Some(&id) = rt.hit_sounds.get(&atkp)
                    && id >= 0
                {
                    sound_queue.push((id as u32, "party_hit"));
                }
                let record = serde_json::json!({"event":"party_hit","tick":tick,
                    "member":kind.name,"atkp":atkp,"damage":damage,"enemy_hp":rt.enemy.hp});
                if let Some(file) = log.as_mut() {
                    let _ = serde_json::to_writer(&mut *file, &record);
                    let _ = std::io::Write::write_all(file, b"\n");
                }
            }
        }
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
