//! The rewrite's own changes to the title, at the project owner's request
//! (not in the original game). Both edit data loaded from the install in
//! memory; nothing of the game's is stored in the repository.
//!
//! - The boot splash's copyright block (title.2ld SEQD 1 group 2) holds for
//!   one second less.
//! - The logo banner reads "Final Re:WRITE" instead of "Final Mix". The new
//!   text (`assets/final-rewrite-text.png`, drawn from a system font by
//!   `tools/Make-BannerText.ps1`) replaces the old text in title image 7.
//!   The banner's ornaments move outward to fit it, and its end lines are
//!   squeezed to keep their tapers.
use crate::layout2d::Texture;
use package_reader::layout::Layout;

/// Frames cut from the copyright block's hold (60 Hz).
pub const COPYRIGHT_CUT: i32 = 60;

/// The boot-splash sequence and its copyright group.
const SPLASH_SEQUENCE: usize = 1;
const COPYRIGHT_GROUP: usize = 2;
/// Frames of the copyright group's six animations in the installed build:
/// the backdrop lead-in (two), the block, its fade-in overlay, its fade-out
/// overlay and the bare backdrop after it.
const COPYRIGHT_FRAMES: [[i32; 2]; 6] = [
    [520, 539],
    [520, 539],
    [540, 779],
    [540, 599],
    [720, 779],
    [780, 850],
];

/// Shortens the copyright block's hold: the block ends, fades out and the
/// backdrop tail all come `COPYRIGHT_CUT` frames earlier, so scene 8 is that
/// much shorter. False (and no change) if the group is not the one expected.
pub fn shorten_copyright(layout: &mut Layout) -> bool {
    let Some(seq) = layout.sequences.get_mut(SPLASH_SEQUENCE) else {
        return false;
    };
    let Some(range) = seq
        .groups
        .get(COPYRIGHT_GROUP)
        .map(|g| g.animations.clone())
    else {
        return false;
    };
    let Some(anims) = seq.animations.get_mut(range) else {
        return false;
    };
    if anims.len() != COPYRIGHT_FRAMES.len()
        || anims
            .iter()
            .zip(COPYRIGHT_FRAMES)
            .any(|(a, f)| a.frames != f)
    {
        return false;
    }
    anims[2].frames[1] -= COPYRIGHT_CUT;
    for a in &mut anims[4..] {
        a.frames = a.frames.map(|f| f - COPYRIGHT_CUT);
    }
    true
}

/// The new banner text, embedded in the binary.
pub const BANNER_TEXT_PNG: &[u8] = include_bytes!("../assets/final-rewrite-text.png");

/// Title image 7 (the logo parts), as its 1024x1024 HD replacement.
pub const LOGO_IMAGE: usize = 7;
const LOGO_SIZE: [u32; 2] = [1024, 1024];
/// The banner's rows and the columns cleared and rebuilt (the Disney and
/// Square Enix logos to the right are left alone).
const BAND_ROWS: std::ops::Range<usize> = 0..56;
const BAND_COLS: std::ops::Range<usize> = 64..560;
/// Columns of the banner parts in the replacement image: the left line
/// (taper at 70), left ornament, old text, right ornament, right line.
const LEFT_LINE: [usize; 2] = [70, 160];
const LEFT_ORNAMENT: [usize; 2] = [160, 207];
const RIGHT_ORNAMENT: [usize; 2] = [419, 467];
const RIGHT_LINE: [usize; 2] = [467, 555];
/// Where the text image's top row goes, and the banner's centre column.
const TEXT_TOP: usize = 12;
const CENTRE: usize = 313;

/// Rewrites the banner in place. False (and no change) for any other image
/// size.
pub fn final_rewrite_banner(t: &mut Texture) -> Result<bool, image::ImageError> {
    let text =
        image::load_from_memory_with_format(BANNER_TEXT_PNG, image::ImageFormat::Png)?.to_rgba8();
    Ok(compose_banner(
        t,
        text.as_raw(),
        [text.width(), text.height()],
    ))
}

fn compose_banner(t: &mut Texture, text: &[u8], text_size: [u32; 2]) -> bool {
    let [tw, th] = text_size.map(|v| v as usize);
    let gap = RIGHT_ORNAMENT[0] - LEFT_ORNAMENT[1];
    let shift = tw.saturating_sub(gap).div_ceil(2);
    let fits = shift < LEFT_LINE[1] - LEFT_LINE[0] - 8
        && shift < RIGHT_LINE[1] - RIGHT_LINE[0] - 8
        && TEXT_TOP + th <= BAND_ROWS.end;
    if t.size != LOGO_SIZE || t.rgba.len() != 1024 * 1024 * 4 || !fits {
        return false;
    }
    let width = LOGO_SIZE[0] as usize;
    let src = t.rgba.clone();
    let at = |x: usize, y: usize| (y * width + x) * 4;
    let copy_column = |from: usize, to: usize, rgba: &mut [u8]| {
        for y in BAND_ROWS {
            rgba[at(to, y)..at(to, y) + 4].copy_from_slice(&src[at(from, y)..at(from, y) + 4]);
        }
    };
    for y in BAND_ROWS {
        t.rgba[at(BAND_COLS.start, y)..at(BAND_COLS.end, y)].fill(0);
    }
    // Lines: squeezed by `shift` columns (nearest column), keeping their
    // outer ends.
    let [l0, l1] = LEFT_LINE;
    for x in l0..l1 - shift {
        copy_column(
            l0 + (x - l0) * (l1 - l0) / (l1 - shift - l0),
            x,
            &mut t.rgba,
        );
    }
    let [r0, r1] = RIGHT_LINE;
    for x in r0 + shift..r1 {
        copy_column(
            r0 + (x - r0 - shift) * (r1 - r0) / (r1 - r0 - shift),
            x,
            &mut t.rgba,
        );
    }
    for x in LEFT_ORNAMENT[0]..LEFT_ORNAMENT[1] {
        copy_column(x, x - shift, &mut t.rgba);
    }
    for x in RIGHT_ORNAMENT[0]..RIGHT_ORNAMENT[1] {
        copy_column(x, x + shift, &mut t.rgba);
    }
    // The text over the cleared middle.
    let left = CENTRE - tw / 2;
    for y in 0..th {
        for x in 0..tw {
            let s = (y * tw + x) * 4;
            if text[s + 3] != 0 {
                let d = at(left + x, TEXT_TOP + y);
                t.rgba[d..d + 4].copy_from_slice(&text[s..s + 4]);
            }
        }
    }
    true
}

#[cfg(test)]
mod tests {
    use super::*;
    use package_reader::layout::{Prop, Scene};
    use package_reader::sequence::{Animation, AnimationGroup, LINEAR, Sequence};

    fn animation(frames: [i32; 2]) -> Animation {
        Animation {
            flags: LINEAR,
            sprite_group: 0,
            frames,
            translate: [0; 4],
            pivot: [0; 4],
            rotation: [0.; 6],
            scale: [1.; 2],
            scale_x: [1.; 2],
            scale_y: [1.; 2],
            curve: [0.; 4],
            bounce: [0.; 4],
            bounce_count: [0; 2],
            blend: 0,
            color: [0x8080_8080; 2],
        }
    }

    fn splash_layout(copyright: [[i32; 2]; 6]) -> Layout {
        let group = |animations| AnimationGroup {
            animations,
            play_count: 1,
            loop_frames: [0, 0],
            light_position_x: 0,
            text_position_y: 0,
            text_scale: 0,
            ui_padding: 0,
            text_position_x: 0,
        };
        let mut animations = vec![animation([0, 259]), animation([260, 519])];
        animations.extend(copyright.map(animation));
        let sequence = |groups| Sequence {
            sprites: vec![],
            parts: vec![],
            sprite_groups: std::iter::once(0..0).collect(),
            animations: animations.clone(),
            groups,
        };
        Layout {
            props: (0..3)
                .map(|g| Prop {
                    texture: 0,
                    sequence: SPLASH_SEQUENCE,
                    group: g,
                    show_at: 0,
                    position: [0, 0],
                })
                .collect(),
            scenes: vec![Scene {
                props: 0..3,
                max_frame: 0,
            }],
            sequences: vec![
                sequence(vec![]),
                sequence(vec![group(0..1), group(1..2), group(2..8)]),
            ],
        }
    }

    fn scene_length(l: &Layout) -> u32 {
        let mut p = package_reader::layout::ScenePlayer::new(l, 0);
        let mut frames = 0;
        while p.state != 3 && frames < 2000 {
            p.update(l, 1.);
            frames += 1;
        }
        frames
    }

    #[test]
    fn copyright_holds_a_second_less() {
        let mut l = splash_layout(COPYRIGHT_FRAMES);
        let before = scene_length(&l);
        assert!(shorten_copyright(&mut l));
        let frames: Vec<_> = l.sequences[1].animations[2..]
            .iter()
            .map(|a| a.frames)
            .collect();
        assert_eq!(
            frames,
            [
                [520, 539],
                [520, 539],
                [540, 719],
                [540, 599],
                [660, 719],
                [720, 790]
            ]
        );
        assert_eq!(before - scene_length(&l), COPYRIGHT_CUT as u32);
    }

    #[test]
    fn other_copyright_timing_is_left_alone() {
        let mut changed = COPYRIGHT_FRAMES;
        changed[2][1] = 700;
        let mut l = splash_layout(changed);
        assert!(!shorten_copyright(&mut l));
        assert_eq!(l.sequences[1].animations[4].frames, [540, 700]);
    }

    fn mark(x: usize, y: usize) -> [u8; 4] {
        [(x / 4) as u8, y as u8, 7, 255]
    }

    /// A logo image whose pixels hold their own x / 4 and y, opaque.
    fn logo() -> Texture {
        let mut rgba = vec![0; 1024 * 1024 * 4];
        for y in 0..1024 {
            for x in 0..1024 {
                let i = (y * 1024 + x) * 4;
                rgba[i..i + 4].copy_from_slice(&mark(x, y));
            }
        }
        Texture {
            rgba,
            size: [1024, 1024],
            texel_scale: 2.,
        }
    }

    fn pixel(t: &Texture, x: usize, y: usize) -> [u8; 4] {
        let i = (y * 1024 + x) * 4;
        t.rgba[i..i + 4].try_into().unwrap()
    }

    #[test]
    fn banner_moves_ornaments_and_draws_the_text() {
        let mut t = logo();
        // 252 wide: 40 more than the old gap, so each ornament moves 20.
        let text = [[9u8, 9, 9, 255]; 252 * 4].concat();
        assert!(compose_banner(&mut t, &text, [252, 4]));
        // Ornaments moved out by 20.
        assert_eq!(pixel(&t, 160 - 20, 30), mark(160, 30));
        assert_eq!(pixel(&t, 466 + 20, 30), mark(466, 30));
        // Line ends kept, the old text gone, the new text drawn.
        assert_eq!(pixel(&t, 70, 30), mark(70, 30));
        assert_eq!(pixel(&t, 554, 30), mark(554, 30));
        assert_eq!(pixel(&t, 64, 30), [0; 4]);
        assert_eq!(pixel(&t, 313, 20), [0; 4]);
        assert_eq!(pixel(&t, 313, TEXT_TOP + 1), [9, 9, 9, 255]);
        // Outside the band nothing changes.
        assert_eq!(pixel(&t, 313, 60), mark(313, 60));
        assert_eq!(pixel(&t, 700, 30), mark(700, 30));
    }

    #[test]
    fn banner_skips_other_sizes() {
        let mut t = Texture {
            rgba: vec![1; 512 * 512 * 4],
            size: [512, 512],
            texel_scale: 1.,
        };
        assert!(!compose_banner(&mut t, &[0; 4], [1, 1]));
        assert!(t.rgba.iter().all(|&b| b == 1));
    }

    #[test]
    fn embedded_text_fits_the_banner() {
        let mut t = logo();
        assert!(final_rewrite_banner(&mut t).unwrap());
    }
}
