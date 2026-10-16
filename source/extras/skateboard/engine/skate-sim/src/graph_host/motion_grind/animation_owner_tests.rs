use super::*;
use crate::graph_host::motion_grind::animation::endpoints;
use skate_core::animation::{
    output::attributes::MotionGraphAttribute,
    phase_blend::PhaseBlend,
    playback_clip::{ClipAttribute, PlaybackClip},
    playback_parameters::{AttributeSink, SettableAttribute},
    playback_tree::PlaybackTree,
    skeleton_input::name::encode,
};
use skate_data::animation_metadata::AnimationMetadata;

fn owner() -> MotionAnimation {
    let metadata = AnimationMetadata::parse(
        r#"{
        "version":1,"source_bank":"synthetic-test",
        "source_sha256":"0000000000000000000000000000000000000000000000000000000000000000",
        "source_bytes":48,"clips":[],"unsupported_trees":[]
    }"#,
    )
    .unwrap();
    let mut owner = MotionAnimation::from_metadata(metadata);
    let clip = |value: f32| PlaybackTree::Clip {
        name: "independent-grind-test".into(),
        clip: PlaybackClip::new(
            61.0,
            60.0,
            1.0,
            0,
            vec![ClipAttribute {
                name: encode(b"twist"),
                kind: 0,
                begin: -1.0,
                end: -1.0,
                payload: vec![value.to_bits()],
            }],
        ),
    };
    owner.current = Some(PlaybackTree::PhaseBlend(
        PhaseBlend::new(encode(b"twist"), vec![clip(-2.0), clip(4.0)]).unwrap(),
    ));
    owner
}
