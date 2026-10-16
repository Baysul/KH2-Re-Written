use super::*;

fn owner(tree: PlaybackTree) -> MotionAnimation {
    // Synthetic independent fixture; no claim that this is stock animation data.
    let metadata = AnimationMetadata::parse(
        r#"{
        "version":1,"source_bank":"synthetic-test",
        "source_sha256":"0000000000000000000000000000000000000000000000000000000000000000",
        "source_bytes":48,"clips":[],"unsupported_trees":[]
    }"#,
    )
    .unwrap();
    let mut animation = MotionAnimation::from_metadata(metadata);
    animation.current = Some(tree);
    animation
}

fn clip(begin: f32, blend_value: f32) -> PlaybackTree {
    PlaybackTree::Clip {
        name: "synthetic-manual-takeoff".into(),
        clip: PlaybackClip::new(
            61.0,
            60.0,
            1.0,
            0,
            vec![
                ClipAttribute {
                    name: encode(b"manualInto"),
                    kind: 0,
                    begin,
                    end: begin + 0.1,
                    payload: vec![17.0f32.to_bits()],
                },
                ClipAttribute {
                    name: encode(b"height"),
                    kind: 0,
                    begin: -1.0,
                    end: -1.0,
                    payload: vec![blend_value.to_bits()],
                },
            ],
        ),
    }
}
