//! Embedding entry point for a host Bevy app that does not use skate-game's
//! plugins. This is glue only: it loads the same prepared files skate-game
//! reads and runs one fixed step in skate-game's FixedUpdate order
//! (input publish -> `controls::sample` -> `physics::advance`).
use crate::{
    camera::{CameraAngleSettings, CameraRuntime},
    difficulty::Difficulty,
    graph_runtime::StockGraphs,
    input::{
        ControllerInput, PublishedTickInput,
        platform::{DeviceError, DevicePacket, SUBTYPE_GAMEPAD},
    },
    physics::{GamePhysics, PlayerControls, SkaterRuntime, ground::Terrain},
};
use bevy::prelude::*;
pub use skate_core::input::xbox::XboxState;
use skate_core::physics::{board::BodyId, drive_frames::RetailAffineTransform};
pub use skate_core::player::state::PhysicalStateId;
use std::path::Path;

/// Which collision world to build when no `.skate` map is loaded.
#[derive(Clone, Debug, Default, PartialEq)]
pub enum SimTerrain {
    /// skate-game's no-map world: the authored test course with its rails.
    #[default]
    Course,
    /// The authored 100 m flat comparison floor (skate-game test terrain), no rails.
    Flat,
    /// The host's own collision triangles (metres, Y up), a wheel-ground
    /// spawn point and heading (radians about Y), no rails.
    Host {
        triangles: Vec<[[f32; 3]; 3]>,
        spawn: [f32; 3],
        heading: f32,
    },
}

/// The single skater's complete simulation state.
pub struct Sim {
    pub(crate) physics: GamePhysics,
    pub(crate) skater: SkaterRuntime,
    pub(crate) controls: PlayerControls,
    pub(crate) graphs: StockGraphs,
    pub(crate) camera: CameraRuntime,
    pub(crate) camera_angle: CameraAngleSettings,
    pub(crate) controller: ControllerInput,
    pub(crate) published: PublishedTickInput,
    packet_number: u32,
    last_state: Option<XboxState>,
}

/// One bone of the published skater pose.
#[derive(Clone, Copy, Debug)]
pub struct BonePose {
    /// Model space: relative to [`Sim::skater_root`], native Y-up axes.
    pub model: Mat4,
    /// Relative to the parent bone's `model` (equal to `model` for roots).
    pub local: Mat4,
}

impl Sim {
    /// skate-game's startup with no map, the saved difficulty
    /// (`settings/difficulty.json` beside `asset_root`, default Easy) and the
    /// test course. `asset_root` is the prepared `private/stock/...` root
    /// skate-game's `--asset-root` / setup points at.
    pub fn load(asset_root: &Path) -> Result<Sim, String> {
        Self::load_with(
            asset_root,
            SimTerrain::Course,
            Difficulty::load(asset_root)?,
        )
    }

    /// As [`Sim::load`], choosing the no-map terrain and difficulty.
    pub fn load_with(
        asset_root: &Path,
        terrain: SimTerrain,
        difficulty: Difficulty,
    ) -> Result<Sim, String> {
        skate_data::input_config::StockGameplayConfig::load(asset_root)
            .map_err(|e| e.to_string())?;
        // The skater model is the host's, so only the graph paths are read
        // (GameAssets::load would also require private/skater.glb).
        let manifest_path = asset_root.join("private/game.json");
        let manifest_text = std::fs::read_to_string(&manifest_path)
            .map_err(|e| format!("{}: {e}", manifest_path.display()))?;
        let manifest = skate_data::GameAssets::parse(&manifest_text).map_err(|e| e.to_string())?;
        let graphs = StockGraphs::load(asset_root, &manifest)?;
        let physics = match terrain {
            SimTerrain::Course => GamePhysics::load_with_difficulty(asset_root, None, difficulty)?,
            SimTerrain::Flat => {
                let mut physics = GamePhysics::load_with_terrain(asset_root, Terrain::Flat)?;
                physics.set_difficulty(difficulty);
                physics
            }
            SimTerrain::Host {
                triangles,
                spawn,
                heading,
            } => {
                let v = |p: [f32; 3]| skate_core::math::Vector3::new(p[0], p[1], p[2]);
                let triangles: Vec<_> = triangles.iter().map(|t| t.map(v)).collect();
                GamePhysics::load_host(asset_root, &triangles, v(spawn), heading, difficulty)?
            }
        };
        let skater = SkaterRuntime::load(asset_root, &graphs, &physics, difficulty.profile_key())?;
        let controls = PlayerControls::load(asset_root)?;
        let camera = CameraRuntime::load(asset_root)?;
        let camera_angle = CameraAngleSettings::load(asset_root);
        Ok(Sim {
            physics,
            skater,
            controls,
            graphs,
            camera,
            camera_angle,
            controller: ControllerInput::default(),
            published: PublishedTickInput::default(),
            packet_number: 0,
            last_state: None,
        })
    }

    /// One fixed step. `pad` is controller slot 0 in XInput convention (see
    /// `input::platform` for the button bits and SDL conversions); `None`
    /// means no controller is connected. With `input_available == false` the
    /// slot is reported disconnected regardless of `pad`.
    ///
    /// Order matches skate-game: PreUpdate (camera-angle sync, device
    /// collection), then FixedUpdate Input -> Controls -> Physics.
    pub fn tick(&mut self, pad: Option<XboxState>, input_available: bool) -> Result<(), String> {
        if self.physics.failed {
            return Err(format!(
                "Simulation stopped after a failed tick {}",
                self.physics.ticks
            ));
        }
        crate::camera::angle::sync_values(&self.camera_angle, &mut self.camera);
        let pad = pad.filter(|_| input_available);
        // XInput dwPacketNumber semantics: advances only when the state changes.
        if let Some(state) = pad
            && self.last_state != Some(state)
        {
            self.packet_number = self.packet_number.wrapping_add(1);
        }
        self.last_state = pad;
        let number = self.packet_number;
        self.controller
            .collect(std::array::from_fn(|slot| match (slot, pad) {
                (0, Some(state)) => Ok(DevicePacket {
                    number,
                    state,
                    subtype: SUBTYPE_GAMEPAD,
                }),
                _ => Err(DeviceError::Disconnected),
            }));
        crate::input::publish_actions(&mut self.controller, &mut self.published, &self.camera);
        crate::physics::sample_controls(
            &self.published,
            &self.physics,
            &self.skater,
            &self.camera,
            &mut self.controls,
        )?;
        crate::physics::advance(
            &mut self.physics,
            &mut self.skater,
            &mut self.controls,
            &self.graphs,
            &self.published,
            &mut self.camera,
        )
    }

    /// Current fixed-step period. The native clock can change it (camera
    /// slow-motion requests); skate-game copies it to `Time<Fixed>` each tick.
    pub fn period(&self) -> std::time::Duration {
        self.physics.period()
    }

    pub fn ticks(&self) -> u64 {
        self.physics.ticks
    }

    pub fn failed(&self) -> bool {
        self.physics.failed
    }

    pub fn state(&self) -> PhysicalStateId {
        self.skater.player_state.current()
    }

    /// Board deck rigid-body frame in world space (native Y-up metres).
    pub fn deck_transform(&self) -> Transform {
        affine(self.physics.board.part_transforms()[BodyId::Deck.index()])
    }

    /// Skater animation root (`animation_to_world`), what skate-game puts on
    /// its `PlayerRoot` entity.
    pub fn skater_root(&self) -> Transform {
        Transform::from_matrix(crate::animation::native_matrix(
            self.skater.animated_skeleton.roots.animation_to_world,
        ))
    }

    /// Stock rig bone names, indexed like [`Sim::pose`].
    pub fn bone_names(&self) -> &[String] {
        &self.skater.animation.evaluator.frames.bone_names
    }

    /// Stock rig parents (`-1` for a root), indexed like [`Sim::pose`].
    pub fn bone_parents(&self) -> &[i32] {
        &self.skater.animation.evaluator.frames.parents
    }

    /// Increments whenever the physical pose is republished.
    pub fn pose_generation(&self) -> u64 {
        self.skater.pose_generation
    }

    /// The completed physical pose (`render_pose`). Apply
    /// [`crate::animation::render_basis`] per bone for skate-game's GLB skin.
    pub fn pose(&self) -> Vec<BonePose> {
        let model: Vec<Mat4> = self
            .skater
            .render_pose
            .iter()
            .copied()
            .map(crate::animation::native_matrix)
            .collect();
        let parents = self.bone_parents();
        model
            .iter()
            .enumerate()
            .map(|(i, &m)| {
                let parent = parents
                    .get(i)
                    .and_then(|&p| usize::try_from(p).ok())
                    .and_then(|p| model.get(p));
                BonePose {
                    model: m,
                    local: parent.map_or(m, |p| p.inverse() * m),
                }
            })
            .collect()
    }

    /// Gameplay camera transform (Bevy convention, looks along -Z) and
    /// vertical field of view in radians, once the camera has a frame.
    pub fn camera(&self) -> Option<(Transform, f32)> {
        self.camera.presentation_frame().map(|frame| {
            (
                crate::presentation::camera_transform(frame),
                frame.field_of_view_degrees.to_radians(),
            )
        })
    }

    /// The camera's aspect ratio input (skate-game sets it from the window).
    pub fn set_camera_aspect_ratio(&mut self, ratio: f32) {
        if ratio.is_finite() && ratio > 0. {
            self.camera.set_aspect_ratio(ratio);
        }
    }
}

fn affine(t: RetailAffineTransform) -> Transform {
    let basis = Mat3::from_cols_array_2d(&t.basis.columns);
    Transform {
        translation: Vec3::new(t.translation.x, t.translation.y, t.translation.z),
        rotation: Quat::from_mat3(&basis).normalize(),
        ..default()
    }
}
