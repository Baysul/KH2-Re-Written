use super::*;

#[test]
#[ignore = "requires private stock banks; selection/timeline/landing pose bridge, not physical acceptance"]
fn stock_match_air_time_selects_jump_advances_pose_and_preserves_landing_identity() {
    use skate_core::animation::{
        playback::{PlaybackRequest, PlaybackService, TransitionSettings},
        playback_tree::{Evaluation, PoseCommand},
    };
    use skate_data::animation_metadata::TreeMetadata;
    let root = std::path::PathBuf::from(
        std::env::var_os("SKATE3_ASSET_ROOT").expect("Set SKATE3_ASSET_ROOT"),
    );
    let banks = skate_data::animation_banks::AnimationBanks::load(&root).unwrap();
    let evaluator = crate::animation_pose::PoseEvaluator::from_banks(&banks).unwrap();
    for held in [false, true] {
        for running in [false, true] {
            let family = if held { "BR" } else { "NB" };
            let selection = format!(
                "SS_{family}_JUMP_{}",
                if running { "RUN_INTO" } else { "STAND_0_INTO" }
            );
            let metadata = banks.metadata().unwrap();
            let TreeMetadata::SelectionSpace(space) = metadata.tree(&selection).unwrap() else {
                panic!("Expected stock jump selection space");
            };
            let indices = [
                "CadenceStartPercent",
                "AnimTime",
                "AnimTransX",
                "AnimTransY",
                "AnimTransZ",
            ]
            .map(|name| {
                space
                    .parameters
                    .iter()
                    .position(|p| p.name.eq_ignore_ascii_case(name))
                    .unwrap()
            });
            let candidate = space
                .candidates
                .iter()
                .find(|c| {
                    c.child.starts_with(&format!("J{family}_"))
                        && f32::from_bits(c.value_bits[indices[1]]) > 0.0
                        && indices[2..]
                            .iter()
                            .map(|&i| f32::from_bits(c.value_bits[i]).powi(2))
                            .sum::<f32>()
                            < 4.0
                })
                .expect("Stock candidate inside native displacement cap");
            let selected = candidate.child.clone();
            let values = indices.map(|i| f32::from_bits(candidate.value_bits[i]));
            let translation = [values[2], values[3], values[4], 0.0];
            let suffix = selected.split_once("_TO_").unwrap().1;
            let landing = format!(
                "{family}_LAND_{suffix}_INTO_{}",
                if running { "RUN_FWD" } else { "STAND" }
            );
            metadata
                .tree(&landing)
                .expect("Stock land.xml counterpart exists");
            let mut animation = MotionAnimation::from_metadata(metadata);
            //Explicit initialized owner, as in the stock body-tweak fixture.
            animation.skater_animation_flags = Some(0);
            let transition = |seconds| TransitionSettings {
                kind: 1,
                seconds,
                under: 0,
                matching: 0,
                use_channels_from_weights: false,
            };
            animation
                .play(PlaybackRequest {
                    animation: selection,
                    speed: 1.0,
                    start_time: 0.0,
                    transition: transition(0.1),
                })
                .unwrap();
            let mut state = State::default();
            state.begin(values[0], values[1], translation);
            state.update(&mut animation, values[1], values[1], translation);
            animation.apply_parameters().unwrap();
            animation.advance(1.0 / 60.0, 0.0);
            animation.refresh_tree_attributes().unwrap();
            assert!(
                animation
                    .tree_attributes()
                    .iter()
                    .any(|a| a.name == encode(selected.as_bytes())),
                "Selected clip lost landing identity: {selected}"
            );
            let first = animation
                .evaluate_pose(Evaluation {
                    cull_threshold: 0.01,
                    update_history: true,
                })
                .unwrap();
            let first_pose = evaluator.evaluate(&first).unwrap();
            let length = animation.current_length().unwrap();
            state.update(&mut animation, values[1] * 0.5, values[1], translation);
            animation.apply_parameters().unwrap();
            animation.advance(1.0 / 60.0, 0.0);
            let midway = animation
                .evaluate_pose(Evaluation {
                    cull_threshold: 0.01,
                    update_history: true,
                })
                .unwrap();
            let expected_time = (length * 0.5 + 1.0 / 60.0).min(length);
            assert!(
                midway
                    .iter()
                    .any(|c| matches!(c, PoseCommand::Clip { name, time, .. }
                if name == &selected && (*time - expected_time).abs() < 0.0001))
            );
            let midway_pose = evaluator.evaluate(&midway).unwrap();
            assert!(
                first_pose
                    .iter()
                    .zip(&midway_pose)
                    .any(|(a, b)| a.rotation != b.rotation || a.translation != b.translation)
            );
            //Named stock landing tree and .05 blend; direct selection is NOT
            //a substitute for the parent's full graph/physics regression.
            animation
                .play(PlaybackRequest {
                    animation: landing.clone(),
                    speed: 1.0,
                    start_time: 0.0,
                    transition: transition(0.05),
                })
                .unwrap();
            for _ in 0..4 {
                animation.apply_parameters().unwrap();
                animation.advance(1.0 / 60.0, 0.0);
                animation.refresh_tree_attributes().unwrap();
                let commands = animation
                    .evaluate_pose(Evaluation {
                        cull_threshold: 0.01,
                        update_history: true,
                    })
                    .unwrap();
                assert!(
                    commands
                        .iter()
                        .any(|c| matches!(c, PoseCommand::Clip { name, .. }
                    if name == &landing))
                );
                assert!(evaluator.evaluate(&commands).unwrap().iter().all(|s| {
                    s.rotation
                        .iter()
                        .chain(s.translation.iter())
                        .all(|v| v.is_finite())
                }));
            }
        }
    }
}
