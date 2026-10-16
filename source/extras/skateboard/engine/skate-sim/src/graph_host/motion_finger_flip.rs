//! FingerFlipOut: TU3 Begin82BB3198, Update82BB7C40, empty End82B61BB8.
use skate_core::point_graph::PointGraph;
use skate_data::collections::Collections;

pub(super) struct Settings {
    minimum: f32,
    maximum: f32,
    curve: PointGraph<8>,
}

impl Settings {
    pub(super) fn load(data: &Collections) -> Result<Self, String> {
        // GlobalAttributeCollection164 is selected at8289FD10..30. The
        // anim_motion schema places this PointNegGraphData8 field at190.
        // Preserve numeric identities because the retail vault omits names.
        let words = data.words::<20>(
            "anim_motion",
            "Hash_41DB0C4F82003A15",
            "Hash_E0C1407B688858AD",
        )?;
        let minimum = f32::from_bits(words[0]);
        let maximum = f32::from_bits(words[2]);
        if !minimum.is_finite() || !maximum.is_finite() || minimum > maximum {
            return Err("FingerFlipOut has invalid stock timer bounds".into());
        }
        Ok(Self {
            minimum,
            maximum,
            curve: PointGraph {
                x: std::array::from_fn(|i| f32::from_bits(words[4 + i])),
                y: std::array::from_fn(|i| f32::from_bits(words[12 + i])),
            },
        })
    }
}

#[derive(Default)]
pub(super) struct State {
    elapsed: f32,
}

impl State {
    pub(super) fn begin(&mut self) {
        self.elapsed = 0.0;
    }

    pub(super) fn update(&mut self, grab_present: bool, dt: f32, settings: &Settings) -> f32 {
        //82454FD8 tests membership, including a zero-valued intent. The time
        //service supplies dt; this is not the magnitude of the grab input.
        self.elapsed = if grab_present {
            self.elapsed - dt
        } else {
            self.elapsed + dt
        };
        self.elapsed = self.elapsed.clamp(settings.minimum, settings.maximum);
        settings.curve.evaluate(self.elapsed)
    }
}
