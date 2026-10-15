//! skate-sim: skate-game's scoring HUD (`scoring_hud.rs` setup/advance/render)
//! without Bevy rendering. The host draws `draws()` as textured triangles in
//! the movie's 1280x720 space: colour = clamp(texel * multiply + add).
use crate::{apt_scene, host::Sim, hud_runtime};
use std::collections::BTreeMap;
use std::path::Path;

pub use apt_scene::{Draw, Fill, Vertex};

pub struct ScoreHud {
    runtime: hud_runtime::Runtime,
    source: serde_json::Value,
    shapes: apt_scene::Shapes,
    failed: bool,
}

impl ScoreHud {
    /// Loads `runtime/trickdisplay.json` under `root` (the prepared
    /// `private/hud`) for the sim's current scoring state.
    pub fn load(root: &Path, sim: &Sim) -> Result<Self, String> {
        let path = root.join("runtime/trickdisplay.json");
        let source: serde_json::Value = serde_json::from_slice(
            &std::fs::read(&path).map_err(|e| format!("{}: {e}", path.display()))?,
        )
        .map_err(|e| e.to_string())?;
        let runtime = hud_runtime::Runtime::load(&source, sim.skater.scoring.hud_input())?;
        let shapes: apt_scene::Shapes =
            serde_json::from_value(source["shapes"].clone()).map_err(|e| e.to_string())?;
        Ok(Self {
            runtime,
            source,
            shapes,
            failed: false,
        })
    }

    /// Every RGBA texture the HUD draws with (path under the HUD root, and
    /// its width and height; the file holds width * height * 4 bytes).
    pub fn texture_files(&self) -> BTreeMap<String, [u32; 2]> {
        let mut files = BTreeMap::new();
        for texture in self
            .shapes
            .values()
            .flatten()
            .filter_map(|s| s.texture.as_ref())
        {
            files.insert(texture.rgba.clone(), [texture.width, texture.height]);
        }
        for font in self.runtime.bindings.movie.text_assets.fonts.values() {
            files.insert(font.texture.clone(), font.size);
            if let Some(fg) = &font.foreground {
                files.insert(fg.texture.clone(), fg.size);
            }
        }
        files
    }

    /// Starts over (a new ride), as skate-game's reset on a map change.
    pub fn reset(&mut self, sim: &Sim) -> Result<(), String> {
        self.runtime = hud_runtime::Runtime::load(&self.source, sim.skater.scoring.hud_input())?;
        self.failed = false;
        Ok(())
    }

    /// One fixed step after the sim's tick.
    pub fn update(&mut self, sim: &Sim) -> Result<(), String> {
        if self.failed {
            return Ok(());
        }
        let s = &sim.skater.scoring;
        let result =
            self.runtime
                .update(s.hud_input(), s.new_trick, s.modified_trick, s.close_tricks);
        self.failed = result.is_err();
        result
    }

    /// This frame's textured triangles (empty once the HUD failed).
    pub fn draws(&mut self) -> Result<Vec<Draw>, String> {
        if self.failed {
            return Ok(Vec::new());
        }
        let result = apt_scene::draw(&self.runtime.bindings.movie, &self.runtime.vm, &self.shapes);
        self.failed = result.is_err();
        result
    }
}
