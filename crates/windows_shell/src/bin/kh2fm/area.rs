//! Field areas for the test room: an area's map, ARD scripts, story
//! programs and spawn points loaded by name from the selected install, as
//! the exe's area loader does (FUN_140152680 → FUN_14039c860 →
//! FUN_1403a2170; map names FUN_140161c50, ARD FUN_1403a2060).
use area_sim::{
    Area, PlayerProbe, Request, Spawn,
    progress::{self, Progress},
    script::{self, Location, ScriptRandom},
};
use bevy::prelude::Vec3;
use field_collision::{Body, Query};
use package_reader::{
    ard::{self, tag},
    bar,
    named::Library,
    objentry,
    probe::ProbeSource,
    room_collision,
};
use std::collections::HashMap;
use windows_shell::room::{LoadedRoom, load_room_archive};

type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;

/// Sora's map body: P_EX100.mdlx BAR type 23 entry 0 (kind 4, r 90, h 90,
/// ph 115), collision class 0 (FUN_1403d5840) [collide-01 §2].
pub const SORA_BODY: Body = Body {
    radius: 90.,
    height: 90.,
    offset: 115.,
    mode: 1,
    class: 0,
};

/// The installed files an area needs, opened once per session.
pub struct AreaSource {
    library: Library,
    progress: Progress,
    /// 00objentry type byte per object id.
    object_types: HashMap<u32, u8>,
    rng: ScriptRandom,
    /// The ARD language directory (`*0x1408ec5d0`; "us" in this build).
    language: &'static str,
}

/// One loaded area.
pub struct LoadedArea {
    pub area: u8,
    pub programs: [i16; 3],
    /// Taken by the field when the area becomes current.
    collision: Option<field_collision::World>,
    pub state: Area,
    /// Entities the scripts created, for the NPC port to place.
    #[allow(dead_code)]
    pub spawns: Vec<Spawn>,
    /// Player spawn in scene coordinates (Y up) and scene yaw.
    pub player: (Vec3, f32),
}

impl AreaSource {
    /// Opens the indexes and sets story flags `first..=last` of the world in
    /// ascending order (FUN_14039afc0 each), which fixes every area's
    /// programs for that point of the story.
    pub fn open(source: &ProbeSource, story: &[(u32, u32)]) -> Result<Self> {
        let mut library = Library::open(source)?;
        let progress_bytes = library.read("00progress.bin")?;
        let table = bar::read_bar(&progress_bytes)?;
        let scripts = progress::WORLDS
            .iter()
            .map(|code| {
                let mut name = [0u8; 4];
                name[..2].copy_from_slice(code.as_bytes());
                table
                    .entries
                    .iter()
                    .find(|e| e.name_bytes == name)
                    .and_then(|e| {
                        progress_bytes
                            .get(e.offset as usize..e.offset as usize + e.size as usize)
                            .map(<[u8]>::to_vec)
                    })
            })
            .collect();
        let mut progress = Progress::new(scripts);
        for &(first, last) in story {
            for flag in first..=last {
                if !progress.test(flag) {
                    progress.set(flag);
                }
            }
        }
        let objects = objentry::read(&library.read("00objentry.bin")?)?;
        Ok(Self {
            library,
            progress,
            object_types: objects
                .iter()
                .map(|o| (o.object_id, o.object_type))
                .collect(),
            rng: ScriptRandom::default(),
            language: "us",
        })
    }

    /// FUN_140152be0 (programs from the save table) then the area loader.
    pub fn load(&mut self, world: u8, area: u8, entrance: u8) -> Result<(LoadedRoom, LoadedArea)> {
        let code = progress::WORLDS
            .get(world as usize)
            .ok_or("world index outside the world table")?;
        let name = format!("{code}{area:02}");
        let map = self.library.read(&format!("map/{name}.map"))?;
        let ard_bytes = self
            .library
            .read(&format!("ard/{}/{name}.ard", self.language))?;
        let ard_bar = bar::read_bar(&ard_bytes)?;
        let entry = |kind: u16, name: u32| -> Option<&[u8]> {
            ard_bar
                .entries
                .iter()
                .find(|e| e.entry_type == kind && u32::from_le_bytes(e.name_bytes) == name)
                .and_then(|e| ard_bytes.get(e.offset as usize..e.offset as usize + e.size as usize))
        };
        let programs = self.progress.programs(world, area);
        let location = Location {
            world,
            area,
            entrance,
            programs,
        };
        let scripts = ["map", "btl", "evt"].map(|n| entry(13, tag(n)));
        let mut game = Flags(&self.progress);
        let setup = script::run(&location, scripts, &mut self.rng, &mut game)?;
        let mut state = Area::new(world, area, entrance, setup, |n| {
            entry(12, n).and_then(|d| ard::spawn_set(d).ok())
        });
        let types = &self.object_types;
        let spawns = state.activate(|id| types.get(&id).copied());
        let room = load_room_archive(&map, None)?;
        let collision = map_collision(&map, state.setup.bg_mask)?;
        let player = Area::player_spawn(&spawns)
            .map(|s| {
                let [x, y, z] = s.entity.position;
                // Engine yaw Y faces (sin Y, 0, cos Y); in the scene (x, −y,
                // −z) that is (sin Y, −cos Y) = (sin φ, cos φ), φ = π − Y.
                (Vec3::new(x, -y, -z), std::f32::consts::PI - s.yaw)
            })
            .ok_or_else(|| format!("{name} has no player spawn for entrance {entrance}"))?;
        Ok((
            room,
            LoadedArea {
                area,
                programs,
                collision: Some(collision),
                state,
                spawns,
                player,
            },
        ))
    }
}

impl LoadedArea {
    pub fn take_collision(&mut self) -> field_collision::World {
        self.collision.take().expect("area collision taken once")
    }
}

struct Flags<'a>(&'a Progress);
impl script::Game for Flags<'_> {
    fn flag_set(&self, flag: u32) -> bool {
        self.0.test(flag)
    }
}

/// The map BAR's first type-6 COCT as the body collision world
/// (FUN_1401613c0 +0x68 → FUN_1401722f0 with the area's group mask).
fn map_collision(map: &[u8], mask: u64) -> Result<field_collision::World> {
    let table = bar::read_bar(map)?;
    let e = table
        .entries
        .iter()
        .find(|e| e.entry_type == 6 && e.size > 0)
        .ok_or("map has no collision")?;
    let coct = room_collision::read(&map[e.offset as usize..e.offset as usize + e.size as usize])?;
    Ok(field_collision::World::new(&coct, mask)?)
}

/// The controller's view of the map collision: one body-versus-map move
/// per tick through the exe's solver (FUN_14016fec0 as FUN_1403b9090 calls
/// it). The scene is Y up; the exe is Y down: (x, y, z) ↔ (x, −y, −z).
pub struct MapWorld {
    world: field_collision::World,
    /// Floor normal from the previous tick (obj+0x768 is −normal), (0, −1,
    /// 0) engine-side without a floor.
    floor_normal: std::sync::Mutex<[f32; 3]>,
}

impl MapWorld {
    pub fn new(world: field_collision::World) -> Self {
        Self {
            world,
            floor_normal: std::sync::Mutex::new([0., -1., 0.]),
        }
    }
}

fn engine(v: [f32; 3]) -> [f32; 3] {
    [v[0], -v[1], -v[2]]
}

impl player_sim::World for MapWorld {
    fn floor(&self, x: f32, z: f32, top: f32) -> Option<f32> {
        // The exe's floor ray (FUN_140173b30) straight down from `top`.
        let p0 = [x, -top, -z];
        let p1 = [x, -top + 100_000., -z];
        self.world.ray(p0, p1, SORA_BODY.class).map(|(p, _)| -p[1])
    }

    fn resolve_walls(&self, position: [f32; 2], _radius: f32, _low: f32, _high: f32) -> [f32; 2] {
        // Only the single collide call moves Sora; other callers get the
        // wanted point back.
        position
    }

    fn collide(
        &self,
        start: player_sim::Vec3,
        displacement: player_sim::Vec3,
        airborne: bool,
        _body: &player_sim::Body,
    ) -> player_sim::Collided {
        let mut d = engine(displacement);
        if !airborne {
            // FUN_1403b9090: the move is turned into the floor plane,
            // keeping its length (n = obj+0x768 = −floor normal).
            let n = *self.floor_normal.lock().expect("floor normal");
            d = field_collision::project_on_floor(d, [-n[0], -n[1], -n[2]]);
        }
        let s = field_collision::solve(
            &self.world,
            &SORA_BODY,
            &Query {
                start: engine(start),
                displacement: d,
                airborne,
                extra_snap: 0.,
            },
        );
        *self.floor_normal.lock().expect("floor normal") =
            field_collision::floor_info(&self.world, &s).normal;
        // Airborne: landing needs a floor while not rising (disp.y ≥ 0 in
        // engine space) [FUN_1403b9090].
        let on_floor = if airborne {
            d[1] >= 0. && !s.no_floor
        } else {
            !s.no_floor
        };
        player_sim::Collided {
            position: engine(s.position),
            on_floor,
        }
    }
}

/// The zone task's view of the player (FUN_1403a4f90): scene → engine.
pub fn probe(position: Vec3, yaw: f32, moving: Option<[f32; 2]>, running: bool) -> PlayerProbe {
    // Scene yaw φ faces (sin φ, cos φ); engine yaw Y = π − φ.
    let engine_yaw = area_sim::wrap(std::f32::consts::PI - yaw);
    let (move_direction, move_speed) = match moving {
        Some([x, z]) => ([x, 0., -z], if running { 8 } else { 2 }),
        None => ([0.; 3], 0),
    };
    PlayerProbe {
        position: [position.x, -position.y, -position.z],
        yaw: engine_yaw,
        move_direction,
        move_speed,
        can_move: true,
    }
}

/// What the area asks the field to do this tick.
pub fn requests(area: &mut Area, probe: &PlayerProbe) -> Vec<Request> {
    // 'dsa' story blocks start unset in this state [exit-01 §2.6 U].
    area.tick(probe, false, |_, _| false)
}

/// Scene direction (x, z) from `from` toward the nearest unfired exit
/// trigger (type-1 point, opt-0 zone) whose destination is not `avoid`.
pub fn exit_target(area: &Area, avoid: Option<u8>, from: [f32; 2]) -> Option<[f32; 2]> {
    let mut best: Option<(f32, [f32; 2])> = None;
    for o in &area.objects {
        if o.point.kind != 1 || o.flags & 1 != 0 || Some(o.point.exit_area()) == avoid {
            continue;
        }
        for z in o.zones.iter().filter(|z| z.option == 0) {
            let d = [z.center[0] - from[0], -z.center[2] - from[1]];
            let l = d[0].hypot(d[1]);
            if l > 0. && best.is_none_or(|(b, _)| l < b) {
                best = Some((l, [d[0] / l, d[1] / l]));
            }
        }
    }
    best.map(|(_, d)| d)
}
