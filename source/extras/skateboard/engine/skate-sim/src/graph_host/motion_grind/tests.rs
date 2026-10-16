use super::*;
mod conditions;
mod fixture;
mod routing;
use fixture::{Owner, blend, clip, height_dependent_blend};
use skate_core::animation::{
    grind_control::FadeSettings, playback::TransitionSettings,
    playback_transition::PlaybackTransition, playback_tree::PlaybackTree,
};
use skate_data::state_graph::GraphAttribute;

fn attributes(pairs: &[(&str, &str)]) -> Vec<GraphAttribute> {
    pairs
        .iter()
        .map(|(name, text)| GraphAttribute {
            name: (*name).into(),
            text: (*text).into(),
            float_bits: 0,
            boolean_byte: 0,
        })
        .collect()
}
fn settings() -> Settings {
    Settings {
        height: [0.25, 1.0, 2.0],
        fade: FadeSettings {
            response: 1.0,
            input_scale: 1.0,
            acceleration: 0.25,
            maximum_step: 0.5,
        },
    }
}
fn physical() -> Physical {
    Physical {
        grinding: true,
        grind_name: encode(b"FS_50_50"),
        ground_axis: [1.0, 0.0, 0.0],
        board_axis: [0.0, 0.0, -1.0],
        ground_flag_273: false,
        animation_mirrored: false,
        height: 1.0,
        crouch: 0.75,
        twist: 0.0,
    }
}
