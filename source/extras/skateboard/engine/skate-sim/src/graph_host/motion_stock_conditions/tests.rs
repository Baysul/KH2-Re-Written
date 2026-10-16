use super::*;
use crate::graph_host::motion_animation::MotionAnimation;

#[test]
#[ignore = "requires private stock banks; named stop-clip predicate regression, not gameplay acceptance"]
fn stock_stop_clips_drive_obstacle_distance_predicate() {
    let root = std::path::PathBuf::from(
        std::env::var_os("SKATE3_ASSET_ROOT").expect("Set SKATE3_ASSET_ROOT"),
    );
    let banks = skate_data::animation_banks::AnimationBanks::load(&root).unwrap();
    let metadata = banks.metadata().unwrap();
    let animation = MotionAnimation::from_metadata(banks.metadata().unwrap());
    //Every authored EnoughDistToObstacle dependency in OffBoard/intostand.xml.
    for prefix in ["WALK", "RUN"] {
        for phase in [0, 25, 50, 75] {
            let name = format!("BR_{prefix}_FWD_{phase}_INTO_STAND_0");
            let clip = metadata.clip(&name).unwrap();
            let attribute = clip
                .attributes
                .iter()
                .find(|a| a.name.eq_ignore_ascii_case("AnimTransZ"))
                .expect("Authored stopping displacement");
            assert_eq!(attribute.type_id, 0, "{name}");
            let displacement = f32::from_bits(attribute.payload_words[0]);
            assert_eq!(
                animation
                    .stock_clip_translation_z("OffBoard", &name)
                    .unwrap(),
                displacement
            );
            let boundary = displacement + f32::from_bits(0x3e99_999a);
            assert!(enough_distance(boundary, displacement), "{name}");
            assert!(!enough_distance(boundary - 0.001, displacement), "{name}");
        }
    }
}
