//! Immutable stock shot definitions. Field order/units follow TU382CA4660.
use super::angle::CameraShotTuning;
use skate_core::camera::{Shot, ShotDatabase, ShotDefinition};
use skate_data::collections::Collections;
use std::collections::BTreeMap;

const CLASS: &str = "camera_shots";
const CLASS_HASH: u64 = 0xf27dd93e059ef6cb;
const RADIANS: f32 = f32::from_bits(0x3c8efa35);

pub(crate) struct StockShots {
    stock: BTreeMap<String, ShotDefinition>,
    /// Mod replacements (camera/angle.rs), applied whenever a shot is loaded.
    tuned: BTreeMap<String, CameraShotTuning>,
}

impl StockShots {
    pub fn from_collections(data: &Collections) -> Result<Self, String> {
        let mut names = BTreeMap::new();
        for entry in data.entries().iter().filter(|c| c.class_name == CLASS) {
            let text = data.field(CLASS, &entry.key, "CollectionName")?;
            if text.type_name != "EA::Reflection::Text" {
                return Err(format!("Invalid camera collection name {}", entry.key));
            }
            if names
                .insert(
                    super::stock_names::lookup8(entry.key.as_bytes()),
                    text.data.clone(),
                )
                .is_some()
            {
                return Err("Duplicate camera collection hash".into());
            }
        }
        let mut definitions = BTreeMap::new();
        for entry in data.entries().iter().filter(|c| c.class_name == CLASS) {
            let key = entry.key.as_str();
            let float = |field: &str| data.float(CLASS, key, field);
            let enumeration =
                |field| -> Result<u32, String> { Ok(data.words::<1>(CLASS, key, field)?[0]) };
            let boolean =
                |field| -> Result<u8, String> { Ok(u8::from(data.boolean(CLASS, key, field)?)) };
            let floats = |fields: &[&str]| -> Result<Vec<f32>, String> {
                fields.iter().map(|field| float(field)).collect()
            };
            let child = |field| -> Result<Option<String>, String> {
                let words = data.words::<6>(CLASS, key, field)?;
                let class = (u64::from(words[0]) << 32) | u64::from(words[1]);
                let hash = (u64::from(words[2]) << 32) | u64::from(words[3]);
                if hash == 0 {
                    return Ok(None);
                }
                if class != CLASS_HASH {
                    return Err(format!("Non-shot reference {key}/{field}"));
                }
                let name = names
                    .get(&hash)
                    .ok_or_else(|| format!("Missing camera child {key}/{field}: {hash:016X}"))?;
                Ok((!name.is_empty()).then(|| name.to_ascii_lowercase()))
            };
            let shot = Shot {
                distance: float("PositionDistance")?,
                lens_length: float("FramingLensLength")?,
                smoothing: floats(&[
                    "SmoothingDirection",
                    "SmoothingElevation",
                    "SmoothingYaw",
                    "SmoothingPitch",
                ])?
                .try_into()
                .unwrap(),
                reference_weights: floats(&[
                    "ReferenceWeightHead",
                    "ReferenceWeightHips",
                    "ReferenceWeightGrind",
                    "ReferenceWeightCOM",
                    "ReferenceWeightBoard",
                    "ReferenceWeightTrajectory",
                    "ReferenceWeightAnchor",
                    "ReferenceWeightFeet",
                    "ReferenceWeightDampedCOM",
                    "Hash_D70C95EDC16A7DFC",
                ])?
                .try_into()
                .unwrap(),
                board_offset: float("ReferenceBoardOffset")?,
                position_heading: float("PositionHeading")? * RADIANS,
                position_elevation: float("PositionElevation")? * RADIANS,
                framing: [
                    float("FramingRoll")? * RADIANS,
                    float("FramingYaw")? * RADIANS,
                    float("FramingPitch")? * RADIANS,
                ],
                follow_subject_in_air: boolean("OptionFollowSubjectInAir")?,
                mirror_for_stance: boolean("OptionMirrorForStance")?,
                snap_to_reference_point: boolean("OptionSnapToReferencePoint")?,
                use_previous_shot: boolean("OptionUsePreviousShot")?,
                use_drop_predictor: boolean("OptionUseDropPredictor")?,
                use_free_camera_stick: boolean("OptionUseFreeCamStick")?,
                // The sole dynamic attribute has the native missing-value zero.
                // All layout fields above are required in the converted data.
                avoidance_override: if has_field(data, key, "OptionAvoidanceOverride")? {
                    boolean("OptionAvoidanceOverride")?
                } else {
                    0
                },
                blur: float("FXBlurShot")?,
                transition_blur: float("FXBlurTransition")?,
                subject_opacity: float("FXSubjectOpacity")?,
                collision_hint: enumeration("OptionCollisionType")?,
                anchor: enumeration("OptionAnchor")?,
                compass_north: enumeration("OptionCompassNorth")?,
                world_heading: float("WorldHeading")? * RADIANS,
                ..Shot::new()
            };
            let definition = ShotDefinition {
                name: key.to_ascii_lowercase(),
                shot_type: enumeration("ShotType")?,
                shot,
                transition_time: float("TransitionTime")?,
                transition_units: enumeration("TransitionTimeUnits")?,
                children: [
                    child("BlendShot1")?,
                    child("BlendShot2")?,
                    child("BlendShot3")?,
                ],
                blend_points: [
                    float("BlendPoint1")?,
                    float("BlendPoint2")?,
                    float("BlendPoint3")?,
                ],
                blend_value: float("BlendValue")?,
                blend_type: enumeration("BlendType")?,
                blend_smoothing: float("SmoothingBlendValue")?,
            };
            if definitions
                .insert(definition.name.clone(), definition)
                .is_some()
            {
                return Err(format!("Duplicate lowercase camera shot {key}"));
            }
        }
        for definition in definitions.values() {
            for name in definition.children.iter().flatten() {
                if !definitions.contains_key(name) {
                    return Err(format!(
                        "Camera shot {} references missing named shot {name}",
                        definition.name
                    ));
                }
            }
        }
        Ok(Self {
            stock: definitions,
            tuned: BTreeMap::new(),
        })
    }
}

impl StockShots {
    pub fn contains(&self, name: &str) -> bool {
        self.stock.contains_key(&name.to_ascii_lowercase())
    }

    pub fn set_tunings<'a>(
        &mut self,
        tunings: impl Iterator<Item = (&'a str, &'a CameraShotTuning)>,
    ) {
        self.tuned = tunings
            .map(|(name, tuning)| (name.to_ascii_lowercase(), tuning.clone()))
            .collect();
    }
}

/// Retail attribute units: angles are stored in degrees and converted like the stock loader.
fn apply_tuning(definition: &mut ShotDefinition, t: &CameraShotTuning) {
    let shot = &mut definition.shot;
    let set = |field: &mut f32, value: Option<f32>, scale: f32| {
        if let Some(v) = value {
            *field = v * scale;
        }
    };
    set(&mut shot.distance, t.position_distance, 1.0);
    set(&mut shot.lens_length, t.framing_lens_length, 1.0);
    set(&mut shot.position_heading, t.position_heading, RADIANS);
    set(&mut shot.position_elevation, t.position_elevation, RADIANS);
    set(&mut shot.framing[0], t.framing_roll, RADIANS);
    set(&mut shot.framing[1], t.framing_yaw, RADIANS);
    set(&mut shot.framing[2], t.framing_pitch, RADIANS);
    set(&mut shot.board_offset, t.reference_board_offset, 1.0);
    for (field, value) in shot.smoothing.iter_mut().zip([
        t.smoothing_direction,
        t.smoothing_elevation,
        t.smoothing_yaw,
        t.smoothing_pitch,
    ]) {
        set(field, value, 1.0);
    }
    set(&mut definition.transition_time, t.transition_time, 1.0);
}

impl ShotDatabase for StockShots {
    fn load(&self, name: &str) -> Result<ShotDefinition, String> {
        let key = name.to_ascii_lowercase();
        let mut definition = self
            .stock
            .get(&key)
            .cloned()
            .ok_or_else(|| format!("Missing stock camera shot {name}"))?;
        if let Some(tuning) = self.tuned.get(&key) {
            apply_tuning(&mut definition, tuning);
        }
        Ok(definition)
    }
}

fn has_field(data: &Collections, key: &str, name: &str) -> Result<bool, String> {
    let mut current = key;
    for _ in 0..=data.entries().len() {
        let entry = data
            .entries()
            .iter()
            .find(|c| c.class_name == CLASS && c.key == current)
            .ok_or_else(|| format!("Missing camera collection {current}"))?;
        if entry.fields.contains_key(name) {
            return Ok(true);
        }
        if entry.parent.is_empty() {
            return Ok(false);
        }
        current = &entry.parent;
    }
    Err(format!("Cyclic camera collection inheritance {key}"))
}
