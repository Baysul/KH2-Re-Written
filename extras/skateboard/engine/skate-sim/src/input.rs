//! Controller input adapter; no animation or physics state mutation here.
//! skate-sim has no device backend (skate-game's SDL3/XInput platform layer
//! is removed): the host supplies one XInput-convention pad packet per slot.
use bevy::prelude::*;

#[cfg(any())]
#[path = "input/tests/action_docs.rs"]
mod action_docs;
mod controllers;
pub(crate) mod gesture_catalog;
pub(crate) mod gesture_input;
pub(crate) mod gesture_mapping;
mod gesture_mapping_data;
pub mod platform;
pub(crate) use controllers::ControllerInput;
use skate_core::input::tick::TickInput;

#[derive(Resource, Clone, Copy, Debug)]
pub(crate) struct PublishedTickInput(pub TickInput);

impl Default for PublishedTickInput {
    fn default() -> Self {
        Self(TickInput::new(
            0,
            skate_core::input::gameplay_map::GameplayActions::from_values([0.0; 18]),
            false,
        ))
    }
}

/// skate-game's `publish_actions` with no pause menu, debug camera or mods:
/// only the camera runtime's manual cam can suppress gameplay.
pub(crate) fn publish_actions(
    input: &mut ControllerInput,
    published: &mut PublishedTickInput,
    camera: &crate::camera::CameraRuntime,
) {
    let blocked = camera.manual_cam.manual_cam_active();
    if blocked {
        input.discard_gameplay();
    }
    input.publish_actions();
    let tick = input.tick_input();
    let values = *tick.actions().values();
    let tick = TickInput::new(
        tick.tick(),
        skate_core::input::gameplay_map::GameplayActions::from_values(values),
        tick.controller_available(),
    );
    published.0 = if blocked {
        TickInput::new(
            tick.tick(),
            skate_core::input::gameplay_map::GameplayActions::from_values([0.0; 18]),
            tick.controller_available(),
        )
    } else {
        tick
    };
}
