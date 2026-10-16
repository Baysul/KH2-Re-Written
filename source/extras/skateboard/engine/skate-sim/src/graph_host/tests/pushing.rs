use super::*;
use crate::graph_host::pushing_settings::{PushTreeAttribute, PushTreeMetadata, PushTreeSource};
use skate_core::{
    point_graph::PointGraph,
    riding::{
        push_animation::{PushAnimationCurves, PushBlendParameters},
        push_behaviors::{PushAttributes, PushClipMetrics},
    },
};
use skate_data::{collections::Collections, state_graph::GraphAttribute};
fn settings() -> PushingSettings {
    let curve = PointGraph {
        x: std::array::from_fn(|i| i as f32),
        y: [1.0; 8],
    };
    let regular = PushAttributes::from_clips([
        PushClipMetrics {
            length: 1.0,
            begin_velocity: 0.0,
            end_velocity: 4.0,
        },
        PushClipMetrics {
            length: 2.0,
            begin_velocity: 4.0,
            end_velocity: 8.0,
        },
        PushClipMetrics {
            length: 1.0,
            begin_velocity: 0.0,
            end_velocity: 1.0,
        },
        PushClipMetrics {
            length: 2.0,
            begin_velocity: 4.0,
            end_velocity: 5.0,
        },
    ]);
    let mongo = PushAttributes::from_clips(regular.clips.map(|mut c| {
        c.begin_velocity += 2.0;
        c.end_velocity += 5.0;
        c
    }));
    PushingSettings {
        curves: PushAnimationCurves {
            button_time_max: curve,
            button_time_to_dv: curve,
            blend_speed_over_frames: curve,
            blend_acc_over_frames: curve,
        },
        teleport_window: 0.5,
        maximum_holding_acceleration: 4.0,
        out_speed_weight: 0.4,
        maximum_out_factor: 0.659,
        regular,
        mongo,
    }
}
fn state() -> PushState {
    let unset = PushBlendParameters {
        hstr_vel_b: -1.0,
        lstr_vel_b: -1.0,
        vel_e: -1.0,
    };
    PushState {
        out_factor: 0.375,
        current_push_dv: 2.5,
        current: unset,
        target: unset,
        continue_push: false,
    }
}
fn context<'a>(
    settings: &'a PushingSettings,
    state: &'a mut PushState,
    intents: &'a IntentMap,
) -> PushContext<'a> {
    PushContext {
        settings,
        shared: state,
        motion_intents: intents,
        forward_speed: 2.0,
        delta_seconds: 1.0,
        time_since_teleport: 1.0,
        is_switch: false,
        foot_frame: None,
    }
}
struct Trees {
    requested: Vec<String>,
    missing_end: bool,
}
impl PushTreeSource for Trees {
    fn metadata(&mut self, name: &str, mask: u32) -> Result<PushTreeMetadata, String> {
        assert_eq!(mask, 15);
        self.requested.push(name.into());
        let mut attributes = vec![
            PushTreeAttribute {
                name: "HStr_Vel_B".into(),
                first_payload: 1.0,
            },
            PushTreeAttribute {
                name: "Unrelated".into(),
                first_payload: 50.0,
            },
            PushTreeAttribute {
                name: "LStr_Vel_B".into(),
                first_payload: 2.0,
            },
        ];
        if !self.missing_end {
            attributes.push(PushTreeAttribute {
                name: "Vel_E".into(),
                first_payload: 6.0,
            });
        }
        Ok(PushTreeMetadata {
            length: 0.5,
            attributes,
        })
    }
}
fn collections() -> Collections {
    let mut fields = Vec::new();
    for name in [
        "pushing_usemaxpushfromteleporttime",
        "max_holding_acc",
        "dynamic_out_factor_speed_vs_acc",
        "last_push_out_time",
    ] {
        fields.push(format!(
            r#""{name}":{{"type":"EA::Reflection::Float","data":"3F000000"}}"#
        ));
    }
    let data = (0..20)
        .map(|i| format!("{:08X}", (i as f32).to_bits()))
        .collect::<String>();
    for name in [
        "button_time_max",
        "button_time_to_dv",
        "blend_speed_over_frames",
        "blend_acc_over_frames",
    ] {
        fields.push(format!(r#""{name}":{{"type":"Graph","data":"{data}"}}"#));
    }
    let json = format!(
        r#"{{"version":1,"collections":[{{"class":"anim_motion","key":"pushing","parent":"","fields":{{{}}},"source":"test","sha256":"test"}}]}}"#,
        fields.join(",")
    );
    let unique = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let root = std::env::temp_dir().join(format!(
        "skate-pushing-test-{}-{unique}",
        std::process::id()
    ));
    let stock = root.join("private/stock");
    std::fs::create_dir_all(&stock).unwrap();
    let file = stock.join("skater-collections.json");
    std::fs::write(&file, json).unwrap();
    let result = Collections::load(&root);
    std::fs::remove_file(file).unwrap();
    std::fs::remove_dir(stock).unwrap();
    std::fs::remove_dir(root.join("private")).unwrap();
    std::fs::remove_dir(root).unwrap();
    result.unwrap()
}
