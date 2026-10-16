//! Exercise the production vert-input adapter from raw controller samples.
use super::transition_action;
use skate_core::input::{
    gameplay_map::GameplayActions,
    pad::Pad,
    xbox::{self, XboxState},
};

fn sample(left_y: i16, triggers: [u8; 2], right_y: i16) -> f32 {
    let mut pad = Pad::new();
    pad.update(&xbox::convert(
        &XboxState {
            buttons: 0,
            triggers,
            left: [0, left_y],
            right: [0, right_y],
        },
        0,
    ));
    transition_action(&mut GameplayActions::from_pad(&pad))
}
