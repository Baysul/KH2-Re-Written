use super::*;

#[test]
#[ignore = "requires private stock OffBoard bank; graph leaf/pose regression, not gameplay acceptance"]
fn stock_air_tweak_channels_parameters_pose_requests_and_teardown() {
    use skate_core::animation::playback::{PlaybackRequest, PlaybackService};
    use skate_core::animation::playback_tree::{Evaluation, PoseCommand};
    use skate_data::animation_banks::AnimationBanks;
    let root = std::path::PathBuf::from(
        std::env::var_os("SKATE3_ASSET_ROOT").expect("Set SKATE3_ASSET_ROOT"),
    );
    let banks = AnimationBanks::load(&root).unwrap();
    let evaluator = crate::animation_pose::PoseEvaluator::from_banks(&banks).unwrap();
    let settings = Settings {
        nb_cycle: "B_OBAIR_BODYTWEAK_NB_CYC".into(),
        nb_into: "B_OBAIR_BODYTWEAK_NB_INTO".into(),
        br_cycle: "B_OBAIR_BODYTWEAK_BR_CYC".into(),
        br_into: "B_OBAIR_BODYTWEAK_BR_INTO".into(),
    };
    for held in [false, true] {
        let mut animation = MotionAnimation::from_metadata(banks.metadata().unwrap());
        animation.skater_animation_flags = Some(0);
        animation
            .play(PlaybackRequest {
                animation: if held {
                    "BR_STAND_0_CYC"
                } else {
                    "NB_STAND_0_CYC"
                }
                .into(),
                speed: 1.0,
                start_time: 0.0,
                transition: TransitionSettings {
                    kind: 1,
                    seconds: 0.0,
                    under: 0,
                    matching: 0,
                    use_channels_from_weights: false,
                },
            })
            .unwrap();
        let mut controls = Controls::default();
        let mut state = State::default();
        let mut physical = Physical {
            time_to_land: 2.0,
            scalar_92: 0.0,
            board_held: held,
        };
        let mut poses = Vec::new();
        state.begin();
        animation.motion_intents.insert("OB_AirBodyTweakX", 0.0);
        animation.motion_intents.insert("OB_AirBodyTweakY", 0.0);
        state
            .update(&settings, &mut animation, &mut controls, physical)
            .unwrap();
        assert!(!animation.channels.has(CHANNEL));
        for direction in [[1.0, 0.0], [-1.0, 0.0], [0.0, 1.0], [0.0, -1.0]] {
            animation
                .motion_intents
                .insert("OB_AirBodyTweakX", direction[0]);
            animation
                .motion_intents
                .insert("OB_AirBodyTweakY", direction[1]);
            for _ in 0..90 {
                state
                    .update(&settings, &mut animation, &mut controls, physical)
                    .unwrap();
                assert!(animation.pending_parameter(encode(b"bodytweakx")).is_some());
                assert!(animation.pending_parameter(encode(b"bodytweaky")).is_some());
                assert_ne!(animation.skater_animation_flags.unwrap() & 0x8000, 0);
                assert!(controls.seed_from_air_tweak);
                animation.apply_parameters().unwrap();
                animation.advance(1.0 / 60.0, 0.0);
                animation.refresh_tree_attributes().unwrap();
                let commands = animation
                    .evaluate_pose(Evaluation {
                        cull_threshold: 0.01,
                        update_history: true,
                    })
                    .unwrap();
                let pose = evaluator.evaluate(&commands).unwrap();
                assert!(pose.iter().all(|s| {
                    s.rotation
                        .iter()
                        .chain(s.translation.iter())
                        .all(|v| v.is_finite())
                }));
                if commands
                    .iter()
                    .any(|c| matches!(c, PoseCommand::ChannelBlend { .. }))
                {
                    poses.push(pose);
                }
            }
        }
        assert!(state.into_started && state.cycle_started);
        assert!(poses.windows(2).any(|p| {
            p[0].iter()
                .zip(&p[1])
                .any(|(a, b)| a.rotation != b.rotation || a.translation != b.translation)
        }));
        physical.time_to_land = 0.05;
        animation.skater_animation_flags = Some(0);
        state
            .update(&settings, &mut animation, &mut controls, physical)
            .unwrap();
        assert_ne!(animation.skater_animation_flags.unwrap() & 0x10000, 0);
        state.end(&mut animation);
        for _ in 0..10 {
            animation.advance(1.0 / 60.0, 0.0);
        }
        assert!(!animation.channels.has(CHANNEL));
        assert!(
            controls.seed_from_air_tweak,
            "native End preserves Wipeout handoff latch"
        );
    }
}
