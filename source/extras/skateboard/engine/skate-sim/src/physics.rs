//! Host game physics ownership and schedule. Physical calculations stay in core.
/// Shared stock physics_mode key for loaded settings and per-frame mode packets.
#[cfg(any())]
pub(crate) const PHYSICS_MODE: &str = "easy";

mod air_phase;
mod air_reckoning;
mod air_trajectory;
mod animated_skeleton;
mod boneless;
pub(crate) mod camera_output;
mod climbing;
mod clock;
pub(crate) mod colliders;
mod controls;
mod foot_ik;
mod foot_ik_queries;
mod foot_physical_output;
mod footplant;
pub(crate) mod ground;
mod handplant;
pub(crate) mod network;
mod plant_skeleton;
mod render_pose;
mod riding_outputs;
mod skateboard_controller;
mod skater;
mod skeleton_air;
pub(crate) mod skeleton_body;
pub(crate) mod skeleton_colliders;
mod skeleton_controller;
mod skeleton_feedback;
mod skeleton_input_runtime;
mod skeleton_output;
pub(crate) mod solid_contacts;
mod solve;
pub(crate) use skater::SkaterRuntime;
// skate-game re-exports input_phase::facing_from_visual for its multiplayer module (removed).
mod animation_feedback;
mod animation_feedback_settings;
mod animation_input;
// skate-sim: the latched animation events `host_audio` takes after each step.
pub(crate) use animation_input::AudioEvents;
mod animation_phase;
mod biped_ground;
#[cfg(debug_assertions)]
mod dev_trace;
mod frame;
mod grind;
mod grind_air_settings;
mod grind_camera;
mod grind_chromosome;
mod grind_host;
mod grind_materials;
mod grind_trick;
mod ground_animation;
mod ground_exit;
mod ground_phase;
mod ground_runtime;
mod input_phase;
mod landing_quality;
mod offboard;
mod player_input;
mod player_state;
mod respawn;
mod revert_state;
mod settings;
mod skeleton_grind_air;
mod slide_state;
mod teleport_state;
mod wipeout;
mod wipeout_states;
//TEMPORARY opt-in observations for the bottom-up source audit.
mod biped_air;
mod known_air;
mod landing_on_deck;
mod offboard_audit_trace;
use bevy::prelude::*;
pub(crate) use controls::PlayerControls;
pub(crate) use controls::sample as sample_controls;
use riding_outputs::RidingOutputs;
use settings::PhysicsSettings;
#[cfg(any())]
use skate_core::physics::board::BodyId;
use skate_core::{
    math::Vector3,
    physics::{
        board_runtime::{BoardMotion, BoardRuntime},
        board_world::{BoardWorld, ContactRetentionSettings},
        collision::WorldContactSettings,
        drive_frames::RetailAffineTransform,
    },
};

#[derive(Resource)]
pub(crate) struct GamePhysics {
    pub(crate) network_proxies: network::Proxies,
    pub(crate) network_active: bool,
    pub(crate) network_contacts: usize,
    clock: clock::SimulationClock,
    pub board: BoardRuntime,
    pub riding: RidingOutputs,
    world: BoardWorld,
    grind_world: std::sync::Arc<crate::grind_world::StaticProvider>,
    grind_materials: grind_materials::GrindMaterials,
    offboard_grab_scene: offboard::grab_scene::Registry,
    pub(crate) settings: PhysicsSettings,
    animation_profile: animation_phase::AnimationProfile,
    query: WorldContactSettings,
    retention: ContactRetentionSettings,
    pub ticks: u64,
    pub contact_count: usize,
    pub failed: bool,
    exchange: SimulationExchange,
    /// ProcessedPhysIn reset82BF9EF0 sets0x2000; initial stancebit20 is clear.
    /// Animation packet publication owns subsequent stance-bit updates.
    pub processed_flags_2468: u32,
    /// Toolkit ctor82C0680C clears8384bit7; wipeout entry/exit owns changes.
    pub board_wiping_out: bool,
}

/// Cross-phase records for the current fixed tick. Subsystems retain their
/// private native-shaped storage; only these buffers cross the coordinator.
pub(crate) struct SimulationExchange {
    commands: skate_core::physics::phase::PhysicsCommandBuffer,
    events: skate_core::physics::phase::PhysicsEventBuffer,
    physical_output: Option<skate_core::physics::phase::PhysicalOutputSnapshot>,
}

impl SimulationExchange {
    fn new(tick: u64) -> Self {
        Self {
            commands: skate_core::physics::phase::PhysicsCommandBuffer::new(tick),
            events: skate_core::physics::phase::PhysicsEventBuffer::new(tick),
            physical_output: None,
        }
    }

    fn emit_event(
        &mut self,
        tick: u64,
        event: skate_core::physics::phase::PhysicsEvent,
    ) -> Result<(), String> {
        self.events.emit(tick, event)
    }

    fn publish_output(&mut self, output: skate_core::physics::phase::PhysicalOutputSnapshot) {
        self.physical_output = Some(output);
    }

    fn output(&self) -> Option<&skate_core::physics::phase::PhysicalOutputSnapshot> {
        self.physical_output.as_ref()
    }

    fn events(&self) -> &[skate_core::physics::phase::PhysicsEvent] {
        self.events.events()
    }

    pub(super) fn request_state(
        &mut self,
        state: skate_core::player::state::PhysicalStateId,
    ) -> Result<(), String> {
        let tick = self.commands.tick();
        self.commands.push(
            tick,
            skate_core::physics::phase::PhysicsCommand::RequestState(state),
        )
    }
}

#[cfg(any())]
mod exchange_tests {
    use super::*;

    #[test]
    fn exchange_keeps_events_on_the_authoritative_tick() {
        let mut exchange = SimulationExchange::new(8);
        assert!(
            exchange
                .emit_event(
                    7,
                    skate_core::physics::phase::PhysicsEvent::StateChanged {
                        from: skate_core::player::state::PhysicalStateId::PhysicsGround,
                        to: skate_core::player::state::PhysicalStateId::PhysicsAir,
                    },
                )
                .is_err()
        );
        assert!(
            exchange
                .emit_event(
                    8,
                    skate_core::physics::phase::PhysicsEvent::StateChanged {
                        from: skate_core::player::state::PhysicalStateId::PhysicsGround,
                        to: skate_core::player::state::PhysicalStateId::PhysicsAir,
                    },
                )
                .is_ok()
        );
        assert!(
            exchange
                .request_state(skate_core::player::state::PhysicalStateId::PhysicsAir)
                .is_ok()
        );
        assert_eq!(exchange.commands.tick(), 8);
        assert_eq!(exchange.commands.commands().len(), 1);
        assert_eq!(exchange.events().len(), 1);
    }
}

impl GamePhysics {
    pub(crate) fn set_gesture_preferences(&mut self, gestures: Option<[u32; 4]>) {
        self.animation_profile.gesture_selections = Some(
            gestures
                .filter(|g| g.iter().all(|v| *v < 37))
                .unwrap_or([0, 1, 2, 3]),
        );
    }
    pub(crate) fn set_equipment_preferences(&mut self, truck: f32, wheel: f32) {
        if truck.is_finite() && wheel.is_finite() {
            self.animation_profile.truck_tightness = truck.clamp(0.0, 1.0);
            self.animation_profile.wheel_hardness = wheel.clamp(0.0, 1.0);
        }
    }
    pub(crate) fn set_difficulty(&mut self, difficulty: crate::difficulty::Difficulty) {
        // Actor publication carries this selector into the next physical packet.
        // Keep the board, active trick, equipment preferences and controller history.
        self.animation_profile.physics_mode = difficulty as u32;
    }
    pub(crate) fn period(&self) -> std::time::Duration {
        self.clock.period()
    }

    pub(crate) fn difficulty_index(&self) -> u32 {
        self.animation_profile.physics_mode
    }

    pub(crate) fn world_triangles(&self) -> &[skate_core::physics::board_world::WorldTriangle] {
        self.world.triangles()
    }

    pub(crate) fn set_external_queries(
        &mut self,
        queries: Option<std::sync::Arc<dyn skate_core::physics::board_world::ExternalQueries>>,
    ) {
        self.world.set_external_queries(queries);
    }
    pub(crate) fn world(&self) -> &BoardWorld {
        &self.world
    }

    /// Flat-world convenience used by private-asset integration tests.
    pub fn load(asset_root: &std::path::Path) -> Result<Self, String> {
        Self::load_with_terrain(asset_root, ground::Terrain::Flat)
    }

    pub(crate) fn load_with_terrain(
        asset_root: &std::path::Path,
        terrain: ground::Terrain,
    ) -> Result<Self, String> {
        Self::load_with_world(asset_root, terrain, None)
    }

    pub(crate) fn load_with_world(
        asset_root: &std::path::Path,
        terrain: ground::Terrain,
        map: Option<&skate_data::skate_map::SkateMap>,
    ) -> Result<Self, String> {
        Self::load_world_difficulty(
            asset_root,
            terrain,
            map,
            crate::difficulty::Difficulty::Easy,
        )
    }

    pub fn load_with_difficulty(
        asset_root: &std::path::Path,
        map: Option<&skate_data::skate_map::SkateMap>,
        difficulty: crate::difficulty::Difficulty,
    ) -> Result<Self, String> {
        Self::load_world_difficulty(asset_root, ground::Terrain::Course, map, difficulty)
    }

    fn load_world_difficulty(
        asset_root: &std::path::Path,
        terrain: ground::Terrain,
        map: Option<&skate_data::skate_map::SkateMap>,
        difficulty: crate::difficulty::Difficulty,
    ) -> Result<Self, String> {
        Self::load_full(asset_root, terrain, map, None, difficulty)
    }

    /// skate-sim: a host world with its wheel-ground spawn (metres, Y up)
    /// and heading, no rails.
    pub(crate) fn load_host(
        asset_root: &std::path::Path,
        triangles: &[[Vector3; 3]],
        spawn: Vector3,
        heading: f32,
        difficulty: crate::difficulty::Difficulty,
    ) -> Result<Self, String> {
        Self::load_full(
            asset_root,
            ground::Terrain::Flat,
            None,
            Some((triangles, spawn, heading)),
            difficulty,
        )
    }

    fn load_full(
        asset_root: &std::path::Path,
        terrain: ground::Terrain,
        map: Option<&skate_data::skate_map::SkateMap>,
        host: Option<(&[[Vector3; 3]], Vector3, f32)>,
        difficulty: crate::difficulty::Difficulty,
    ) -> Result<Self, String> {
        let data = crate::custom_difficulty::load_collections(asset_root)?;
        let settings = PhysicsSettings::load(&data)?;
        let animation_profile =
            animation_phase::AnimationProfile::load(&data, difficulty.profile_key())?;
        eprintln!(
            "SKATE_PHYSICS_MODE {} index={}",
            difficulty.key(),
            animation_profile.physics_mode
        );
        let mut spawn = RetailAffineTransform {
            translation: Vector3::new(
                0.0,
                ground::HEIGHT + settings.wheel_radius - settings.authored[0].translation.y,
                0.0,
            ),
            ..RetailAffineTransform::IDENTITY
        };
        if let Some(map) = map {
            // Package spawn is the wheel-ground anchor, in native Y-up metres.
            spawn.translation = Vector3::new(
                map.spawn[0],
                map.spawn[1] + settings.wheel_radius - settings.authored[0].translation.y,
                map.spawn[2],
            );
            spawn.basis = skate_core::math::Basis3 {
                columns: Mat3::from_rotation_y(map.heading).to_cols_array_2d(),
            };
        }
        if let Some((_, at, heading)) = host {
            // As a map spawn: the wheel-ground anchor.
            spawn.translation = Vector3::new(
                at.x,
                at.y + settings.wheel_radius - settings.authored[0].translation.y,
                at.z,
            );
            spawn.basis = skate_core::math::Basis3 {
                columns: Mat3::from_rotation_y(heading).to_cols_array_2d(),
            };
        }
        let board = BoardRuntime::new(
            settings.masses,
            settings.authored,
            spawn,
            settings.step.simulation,
            BoardMotion::Active,
        );
        let world = match (map, host) {
            (Some(map), _) => crate::skate_world::collision_world(map, settings.floor_material)?,
            (None, Some((triangles, ..))) => {
                ground::host_world(settings.floor_material, triangles)?
            }
            (None, None) => terrain.world(settings.floor_material),
        };
        let grind_world =
            std::sync::Arc::new(if map.is_none() && terrain == ground::Terrain::Course {
                crate::grind_world::StaticProvider::authored(&crate::grind_world::test_rails())?
            } else {
                crate::grind_world::StaticProvider::new(map)?
            });
        if let Some(map) = map {
            eprintln!(
                "SKATE_GRIND_READY splines={} primitives={}",
                map.rails.len(),
                grind_world.primitives().len()
            );
        }
        let grind_materials = grind_materials::GrindMaterials::new(&settings);
        //Both authored terrains contain static collision surfaces only, with
        //no interactable objects or assembly-bound grab splines. Do not infer
        //those identities from triangle/mesh IDs. Queries use the real registry.
        let offboard_grab_scene =
            offboard::grab_scene::Registry::new(&world, Vec::new(), Vec::new())?;
        let processed_flags_2468 = 0x2000;
        let riding = RidingOutputs::load(&data, &board, processed_flags_2468)?;
        let (query, retention) = ground::query_settings();
        Ok(Self {
            network_proxies: network::Proxies::default(),
            network_active: false,
            network_contacts: 0,
            clock: clock::SimulationClock::default(),
            board,
            riding,
            world,
            grind_world,
            grind_materials,
            offboard_grab_scene,
            settings,
            animation_profile,
            query,
            retention,
            ticks: 0,
            contact_count: 0,
            failed: false,
            exchange: SimulationExchange::new(0),
            processed_flags_2468,
            board_wiping_out: false,
        })
    }

    #[cfg(any())]
    fn advance_board(&mut self) -> Result<(), String> {
        self.board.clear_forces();
        self.riding.start_wheel_queries(&self.board, &self.world)?;
        self.riding.finish_wheel_queries()?;
        let volumes = colliders::world_volumes(&self.board, &self.settings);
        let contacts = self
            .world
            .query_primitives(&volumes, self.query, self.retention);
        self.contact_count = contacts.len();
        // The graph/ground-state consumer must supply recovered steering and
        // forces before this is playable. This tick verifies physical assembly.
        self.board.advance(contacts, [0.0; 2], self.settings.step);
        self.riding.finish_post_physics(
            &mut self.board,
            self.board_wiping_out,
            self.processed_flags_2468,
            self.settings.step.simulation.time_step,
        )?;
        self.ticks += 1;
        if self.board.bodies().iter().any(|body| {
            let p = body.rates.position;
            !p.x.is_finite() || !p.y.is_finite() || !p.z.is_finite()
        }) {
            self.failed = true;
            return Err(format!(
                "Board solver produced a non-finite pose on tick {}",
                self.ticks
            ));
        }
        Ok(())
    }
}

/// Collision+16 low 16 bits: the board's surface vote (82C08818, 12 when a board
/// contact is water), as respawn reads it. Read-only, for game_audio (the audio
/// record's `+813`, the board in water).
pub(crate) fn board_surface(physics: &GamePhysics) -> u32 {
    ground_runtime::active_surface(&physics.riding, &physics.board) & 0xffff
}

/// skate-game's FixedUpdate `advance` system without its Bevy resources,
/// AppExit, performance timing and mod suspension gate (skate-sim has no mods).
pub(crate) fn advance(
    physics: &mut GamePhysics,
    skater: &mut SkaterRuntime,
    controls: &mut PlayerControls,
    graphs: &crate::graph_runtime::StockGraphs,
    input: &crate::input::PublishedTickInput,
    camera: &mut crate::camera::CameraRuntime,
) -> Result<(), String> {
    if physics.failed {
        return Ok(());
    }
    let mut actions = input.0.actions();
    let input_available = input.0.controller_available();
    #[cfg(debug_assertions)]
    let _physics_trace = dev_trace::begin(physics, skater, *input.0.actions().values());
    if let Err(message) = frame::advance(
        physics,
        skater,
        controls,
        graphs,
        &mut actions,
        input_available,
        camera,
    ) {
        #[cfg(debug_assertions)]
        dev_trace::dump(&format!("tick={} physics_error={message}", physics.ticks));
        physics.failed = true;
        let message = format!(
            "{message}; state={:?}; tick={}; mapped_input={:?}; force_mode={}; board_axis_y={}; flags={:08x}/{:08x}/{:08x}/{:08x}/{:08x}",
            skater.player_state.current(),
            physics.ticks,
            input.0.actions(),
            skater.skeleton_input.force_mode,
            skater.skeleton_input.drive_frames[0][2][1],
            skater.player_input.processed.flags_2468,
            skater.player_input.processed.flags_2472,
            skater.player_input.processed.flags_2476,
            skater.player_input.processed.flags_2480,
            skater.player_input.processed.flags_2484,
        );
        error!("{message}");
        return Err(message);
    }
    Ok(())
}

impl GamePhysics {
    fn finish_skater(&mut self, skater: &mut SkaterRuntime) -> Result<(), String> {
        //Skateboard::UpdatePostPhysics82C02158 prepares the wall probe for
        //the next StartBoard, using this input phase's cached deck toolkit.
        self.riding.probes.prepare_wall(
            &skater.player_input.processed,
            skater
                .player_input
                .toolkit
                .as_ref()
                .ok_or("Postphysics wall probe requires the current board toolkit")?,
        );
        self.riding.finish_post_physics(
            &mut self.board,
            self.board_wiping_out,
            self.processed_flags_2468,
            self.settings.step.simulation.time_step,
        )?;
        let partial = skateboard_controller::partial_request(
            &skater.skateboard_controller,
            &self.riding.ground,
        );
        skeleton_feedback::publish(self, skater, partial);
        if skater.player_state.current()
            == skate_core::player::state::PhysicalStateId::WipeoutGround
        {
            wipeout_states::post_physics(skater);
        }
        if skater.player_state.current() == skate_core::player::state::PhysicalStateId::PhysicsAir {
            air_phase::update_apex(self, &mut skater.air_state);
        }
        if skater.player_state.current() == skate_core::player::state::PhysicalStateId::KnownAir {
            known_air::post_physics(self, skater)?;
        }
        if skater.player_state.current().is_grind()
            || skater.player_state.current()
                == skate_core::player::state::PhysicalStateId::Nonspecific
        {
            grind::post(self, skater)?;
        }
        wipeout::check_after_physics(self, skater)?;
        if skater.player_state.current()
            == skate_core::player::state::PhysicalStateId::PhysicsAirSecondary
        {
            grind_trick::post_velocity(self, skater);
        }
        offboard::post_physics::advance(self, skater)?;
        let compression = skater.skeleton_output.average_compressions(&self.board);
        render_pose::publish(self, skater, compression)?;
        self.ticks += 1;
        let invalid_board = self.board.bodies().iter().enumerate().find(|(_, body)| {
            let p = body.rates.position;
            let v = body.rates.linear_velocity;
            let w = body.rates.angular_velocity;
            [p.x, p.y, p.z, v.x, v.y, v.z, w.x, w.y, w.z]
                .into_iter()
                .any(|value| !value.is_finite())
        });
        let invalid_skeleton = skater
            .skeleton
            .bodies()
            .iter()
            .enumerate()
            .find(|(_, body)| {
                let p = body.rates.position;
                let v = body.rates.linear_velocity;
                let w = body.rates.angular_velocity;
                [p.x, p.y, p.z, v.x, v.y, v.z, w.x, w.y, w.z]
                    .into_iter()
                    .any(|value| !value.is_finite())
            });
        if invalid_board.is_some() || invalid_skeleton.is_some() {
            self.failed = true;
            return Err(format!(
                "Shared skater solver produced non-finite rates on tick{}; board={invalid_board:?}; skeleton={invalid_skeleton:?}",
                self.ticks
            ));
        }
        Ok(())
    }
}
