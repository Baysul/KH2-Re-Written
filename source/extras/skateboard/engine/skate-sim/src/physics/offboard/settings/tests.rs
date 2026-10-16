use super::{Settings, curves, metrics};
use skate_data::animation_metadata::{ClipAttribute, ClipMetadata};
fn clip() -> ClipMetadata {
    ClipMetadata {
        name: "NB_WALK_FWD_CYC".into(),
        source_offset: 48,
        fps_bits: 60.0f32.to_bits(),
        frames_bits: 57.0f32.to_bits(),
        base_speed_bits: 3.0f32.to_bits(),
        flags_word: 0,
        attributes: vec![ClipAttribute {
            name: "ANIMTRANSZ".into(),
            type_id: 0,
            begin_bits: (-1.0f32).to_bits(),
            end_bits: (-1.0f32).to_bits(),
            payload_words: vec![1.5f32.to_bits()],
            source_offset: 128,
        }],
    }
}
#[test]
#[ignore = "requires extracted private stock assets"]
fn actual_stock_settings_and_metrics_load_together() {
    let root =
        std::path::PathBuf::from(std::env::var_os("SKATE3_ASSET_ROOT").expect("SKATE3_ASSET_ROOT"));
    let collections = skate_data::collections::Collections::load(&root).unwrap();
    let metadata = skate_data::animation_banks::AnimationBanks::load(&root)
        .unwrap()
        .metadata()
        .unwrap();
    let settings = Settings::load(&collections, &metadata).unwrap();
    assert!(settings.metrics.iter().all(Option::is_some));
    let speeds = settings.metrics.map(|m| {
        let m = m.unwrap();
        (-1.0 / m.end_time) * m.translation_z
    });
    assert!(speeds[0] > 0.0 && speeds[0] < speeds[1] && speeds[1] < speeds[2]);
    assert_eq!(settings.controller.movement_intent.sprint_time_cap, 4.0);
}
