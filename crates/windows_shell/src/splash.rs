//! The rewrite's own studio splash, "GLURBUS STUDIOS", shown once per launch
//! inside the game's boot splash (title.2ld scene 8), between the Square
//! Enix logo and the copyright block. It is not in the original game. The
//! boot splash holds at frame 520, on its bare backdrop, while this plays.
//! Its timing copies the Square Enix group of that splash (SEQD 1 group 1):
//! frames 0..19 are the bare backdrop, then the logo shows from 20 to 259
//! under an overlay of the backdrop colour that fades out over 20..79 and
//! back in over 200..259.
//!
//! The art is the project owner's own image (`assets/glurbus-splash.png`),
//! not game data.
use package_reader::sequence::Quad;

/// The splash image, embedded in the binary.
pub const IMAGE_PNG: &[u8] = include_bytes!("../assets/glurbus-splash.png");

/// Frames the splash lasts at 60 Hz.
pub const LENGTH: u32 = 260;
/// The boot-splash frame it plays at: the start of SEQD 1 group 2 (the
/// copyright block), after the Square Enix logo has faded out at 519.
pub const BOOT_FRAME: f32 = 520.;

/// The logo's square inside the 1280x720 image, in pixels, inset past the
/// paper's faint edge.
const LOGO_UV: [i32; 4] = [414, 87, 911, 584];
/// Where the logo square sits in layout space: the Disney part's height
/// (224, SEQD 1 sprite group 1), centred on the visible area.
const LOGO_RECT: [f32; 4] = [144., 96., 368., 320.];
/// The full-screen backdrop part of SEQD 1 (sprite group 0).
const BACKDROP: [f32; 4] = [-149., -31., 660., 448.];
/// The backdrop's grey: SEQD 1's sprite colour 0xf5f5f5, which the scene-8
/// captures show as is.
const BACKDROP_GREY: u8 = 0xf5;

/// Pixels this close to white are the paper; they take the backdrop's grey
/// so the logo has no visible box.
const PAPER: u8 = 0xf0;

/// The decoded splash image.
pub struct Image {
    pub rgba: Vec<u8>,
    pub size: [u32; 2],
}

/// Decodes the embedded image and greys its paper to the backdrop.
pub fn load() -> Result<Image, image::ImageError> {
    let decoded = image::load_from_memory_with_format(IMAGE_PNG, image::ImageFormat::Png)?;
    let rgba = decoded.to_rgba8();
    let size = [rgba.width(), rgba.height()];
    let mut rgba = rgba.into_raw();
    match_paper(&mut rgba);
    Ok(Image { rgba, size })
}

fn match_paper(rgba: &mut [u8]) {
    for px in rgba.as_chunks_mut::<4>().0 {
        if px[..3].iter().all(|&c| c >= PAPER) {
            px[..3].fill(BACKDROP_GREY);
        }
    }
}

/// Linear progress through frames `start..=end`, as SEQD animations with
/// the linear flag.
fn progress(t: u32, start: u32, end: u32) -> f32 {
    (t.saturating_sub(start) as f32 / (end - start) as f32).min(1.)
}

/// The splash's frame counter.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Splash {
    pub frame: u32,
}

impl Splash {
    /// Advances one 60 Hz frame; true once the splash is over.
    pub fn update(&mut self) -> bool {
        self.frame += 1;
        self.finished()
    }

    pub fn finished(&self) -> bool {
        self.frame >= LENGTH
    }

    /// Whether the logo is drawn, and the overlay's alpha over it (1
    /// opaque).
    fn state(&self) -> (bool, f32) {
        let t = self.frame;
        match t {
            0..=19 => (false, 0.),
            20..=79 => (true, 1. - progress(t, 20, 79)),
            80..=199 => (true, 0.),
            200..=259 => (true, progress(t, 200, 259)),
            _ => (false, 0.),
        }
    }

    /// This frame's quads: backdrop on the white sheet, then the logo on the
    /// image sheet (if shown), then the overlay on the white sheet. Colours
    /// use the sequence scale (0x80 = 1.0).
    pub fn quads(&self) -> (Vec<Quad>, Option<Quad>, Option<Quad>) {
        let (logo, overlay) = self.state();
        let grey = BACKDROP_GREY as f32 / 255.;
        let backdrop = vec![rect(BACKDROP, [0, 0, 1, 1], [grey, grey, grey, 1.])];
        let logo = logo.then(|| rect(LOGO_RECT, LOGO_UV, [1., 1., 1., 1.]));
        let overlay = (logo.is_some() && overlay > 0.)
            .then(|| rect(BACKDROP, [0, 0, 1, 1], [grey, grey, grey, overlay]));
        (backdrop, logo, overlay)
    }
}

fn rect([x0, y0, x1, y1]: [f32; 4], uv: [i32; 4], colour: [f32; 4]) -> Quad {
    Quad {
        corners: [[x0, y0], [x1, y0], [x0, y1], [x1, y1]],
        uv,
        colors: [colour; 4],
        blend: 0,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn at(frame: u32) -> (bool, f32) {
        Splash { frame }.state()
    }

    #[test]
    fn follows_the_square_enix_group_timing() {
        assert_eq!(at(0), (false, 0.));
        assert_eq!(at(19), (false, 0.));
        assert_eq!(at(20), (true, 1.));
        assert_eq!(at(79), (true, 0.));
        assert_eq!(at(150), (true, 0.));
        assert_eq!(at(200), (true, 0.));
        assert_eq!(at(LENGTH - 1), (true, 1.));
    }

    #[test]
    fn ends_after_its_length() {
        let mut s = Splash::default();
        let frames = (1..).find(|_| s.update()).unwrap();
        assert_eq!(frames, LENGTH);
    }

    #[test]
    fn paper_takes_the_backdrop_grey_and_ink_stays() {
        let mut px = [
            0xfa, 0xfa, 0xfa, 0xff, 0x20, 0x20, 0x20, 0xff, 0xfa, 0x80, 0xfa, 0xff,
        ];
        match_paper(&mut px);
        assert_eq!(&px[..4], &[0xf5, 0xf5, 0xf5, 0xff]);
        assert_eq!(&px[4..8], &[0x20, 0x20, 0x20, 0xff]);
        assert_eq!(&px[8..], &[0xfa, 0x80, 0xfa, 0xff]);
    }

    #[test]
    fn embedded_image_decodes() {
        let i = load().unwrap();
        assert_eq!(i.size, [1280, 720]);
        assert_eq!(i.rgba.len(), 1280 * 720 * 4);
    }
}
