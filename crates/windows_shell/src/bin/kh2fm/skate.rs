//! J: Sora takes the skateboard. Skate 3's riding comes from the user's own
//! Skate 3 Xbox 360 disc, so without an admitted disc the key refuses with
//! a notice and nothing changes.
use bevy::prelude::*;
use skate_source::{Disc, Rejection};

/// How long a notice stays up (frames at 60 Hz).
const NOTICE_FRAMES: u32 = 300;

#[derive(Resource, Default)]
pub struct Skate {
    disc: Option<Disc>,
    /// The notice line (system-font bytes) and its remaining frames.
    pub notice: Option<(Vec<u8>, u32)>,
}

impl Skate {
    fn say(&mut self, text: &str) {
        self.notice = Some((encode(text), NOTICE_FRAMES));
    }
}

/// ASCII into the system font's international encoding: upper case and
/// digits as `camp::encode_number` (FUN_1403adcc0), lower case 0x9a..0xb3
/// and the marks after OpenKH's InternationalSystemDecode table; anything
/// else is a space.
fn encode(text: &str) -> Vec<u8> {
    text.bytes()
        .map(|b| match b {
            b'a'..=b'z' => 0x9a + (b - b'a'),
            b'.' => 0x4f,
            b',' => 0x50,
            b'?' => 0x49,
            b'-' => 0x66,
            b'0'..=b'9' | b'A'..=b'Z' | b'!' | b'\'' => {
                windows_shell::camp::encode_number(&char::from(b).to_string())[0]
            }
            _ => 0x01,
        })
        .collect()
}

/// The on-screen line for a refusal; the full reason goes to the log.
fn refusal(e: &Rejection) -> &'static str {
    match e {
        Rejection::NotConfigured => "Skateboarding needs your Skate 3 ISO. Set it in the launcher.",
        Rejection::Io(..) => "The Skate 3 ISO set in the launcher could not be opened.",
        Rejection::NotXboxImage | Rejection::Malformed(_) => {
            "The Skate 3 ISO set in the launcher is not a readable Xbox 360 disc."
        }
        Rejection::Missing(_) | Rejection::NotXex => "The disc set in the launcher is not Skate 3.",
    }
}

pub fn toggle(keys: Res<ButtonInput<KeyCode>>, mut skate: ResMut<Skate>) {
    if let Some((_, frames)) = &mut skate.notice {
        *frames = frames.saturating_sub(1);
        if *frames == 0 {
            skate.notice = None;
        }
    }
    if !keys.just_pressed(KeyCode::KeyJ) {
        return;
    }
    if skate.disc.is_none() {
        match skate_source::admit(crate::launcher::skate3_iso().as_deref()) {
            Ok(disc) => skate.disc = Some(disc),
            Err(e) => {
                eprintln!("Skate 3 disc refused: {e}");
                return skate.say(refusal(&e));
            }
        }
    }
    skate.say("Skate 3 disc found. Riding is not built yet.");
}
