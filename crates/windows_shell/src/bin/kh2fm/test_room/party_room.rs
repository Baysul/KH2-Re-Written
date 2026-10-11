//! Donald and Goofy in the test room: their models and motions found by
//! name in the installed archives, the follow behaviour of
//! `player_sim::party`, and the mesh presentation.
//!
//! First pass: they stand, walk and run behind Sora. Attacks, spells,
//! items, KO and Limits follow the specs under `party-01`.
use super::*;
use crate::npcs::{self, Anim};
use package_reader::named::Library;
use player_sim::party::Follower;
use std::sync::Arc;

/// The motion ids loaded for a member: idle, walk, run (P_EX020/P_EX030
/// slots 4m+k through the set's chooser).
const MOTIONS: [u16; 3] = [0, 1, 2];

/// A member's installed model and animations.
pub struct PartyKind {
    pub name: &'static str,
    pub model: model_inspection::InspectionModel,
    /// Per-tick vertex positions by motion id.
    pub anims: HashMap<u16, Anim>,
}

/// A member in the room.
pub struct MemberRt {
    pub kind: Arc<PartyKind>,
    pub follower: Follower,
}

/// Loads `obj/<name>.mdlx` and `obj/<name>.mset` by name.
pub fn load(
    source: &ProbeSource,
    library: &mut Library,
    name: &'static str,
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
    let set = library
        .find(&format!("obj/{name}.mset"))
        .ok_or_else(|| format!("obj/{name}.mset is not installed"))?;
    let selection = motion::MotionSetSelection {
        index: set.index.to_string(),
        ordinal: set.ordinal,
        name_hash: set.name_hash,
        battle: true,
        has_weapon: true,
    };
    let mut anims = HashMap::new();
    for selected in motion::read_motion_ids(source, &selection, &MOTIONS)? {
        // read_motion_ids returns the motions in request order.
        let id = MOTIONS[anims.len()];
        let anim = npcs::build_anim(&model, &Arc::new(selected), None)?;
        anims.insert(id, anim);
    }
    Ok(PartyKind { name, model, anims })
}

/// Marks a member's root and its mesh parts.
#[derive(Component)]
pub struct PartyRoot(pub usize);
#[derive(Component)]
pub struct PartyPart {
    pub member: usize,
    pub ordinal: usize,
}

/// One tick of every member behind Sora.
pub fn tick(game: &mut Game) {
    let Game {
        player,
        params,
        world,
        party,
        ..
    } = game;
    let body = player_sim::Body {
        radius: params.radius,
        height: params.height,
        step: params.step_height,
    };
    for m in party.iter_mut() {
        m.follower.tick(player.position, player.yaw, &*world, &body);
    }
}

/// The frame of an animation for a tick counter: plays to its end, then
/// loops from its loop start.
fn frame_of(anim: &Anim, tick: u32) -> usize {
    let end = anim.frames.len().saturating_sub(1) as u32;
    let t = if tick <= end || anim.loop_start >= end {
        tick.min(end)
    } else {
        anim.loop_start + (tick - anim.loop_start) % (end - anim.loop_start)
    };
    t as usize
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
        let Some(anim) = m.kind.anims.get(&m.follower.motion()) else {
            continue;
        };
        let frame = &anim.frames[frame_of(anim, m.follower.tick)];
        let positions: Vec<[f32; 3]> = m.kind.model.parts[part.ordinal]
            .indices
            .iter()
            .map(|&i| frame[i as usize].to_array())
            .collect();
        if let Some(mut mesh) = meshes.get_mut(&handle.0) {
            mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, positions);
        }
    }
}
