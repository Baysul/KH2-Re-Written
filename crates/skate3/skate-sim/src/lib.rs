//! The Skate 3 skater simulation (skate-game's physics, graphs, input and
//! camera) without rendering, audio, menus, mods or networking, for a Bevy
//! 0.19 host. [`host::Sim`] is the embedding entry point.
#![allow(dead_code)]
pub mod animation;
mod animation_pose;
mod apt_display;
mod apt_imports;
mod apt_movie;
pub mod apt_scene;
mod apt_text;
mod apt_vm;
mod camera;
mod custom_difficulty;
pub mod difficulty;
mod graph_host;
mod graph_runtime;
mod grind_world;
pub mod host;
pub mod host_hud;
pub mod hud_runtime;
pub mod input;
mod physics;
pub mod presentation;
mod scoring_runtime;
mod skate_world;
mod skater_animation;

pub use difficulty::Difficulty;
pub use host::{BonePose, PhysicalStateId, Sim, SimTerrain, XboxState};
