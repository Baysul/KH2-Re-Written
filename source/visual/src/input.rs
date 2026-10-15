//! Bevy device state into the exe's PADDATA (`pad`): the default keyboard
//! and mouse bindings (FUN_1404ea260 / FUN_140102ca0) and the first gamepad
//! in XInput order (FUN_140134d10).
use crate::controller::{self, PadData, button};
use bevy::prelude::*;

/// PADDATA from keyboard, mouse (wheel = D-pad up/down for one frame) and
/// the first gamepad; also whether the keyboard/mouse was used and the
/// active pad's glyph family (1 Xbox, 2 PlayStation, 3 other).
pub fn read_devices(
    keys: &ButtonInput<KeyCode>,
    mouse: &ButtonInput<MouseButton>,
    wheel: f32,
    pads: &Query<(&Gamepad, Option<&Name>)>,
) -> (PadData, bool, Option<u8>) {
    let mut p = PadData::default();
    let key_map: [(&[KeyCode], u16); 16] = [
        (&[KeyCode::Enter, KeyCode::NumpadEnter], button::CROSS),
        (&[KeyCode::Space, KeyCode::Backspace], button::CIRCLE),
        (&[KeyCode::ShiftRight], button::SQUARE),
        (&[KeyCode::KeyF, KeyCode::ControlRight], button::TRIANGLE),
        (&[KeyCode::KeyR], button::R1),
        (&[KeyCode::KeyE], button::R2),
        (&[KeyCode::ShiftLeft], button::L1),
        (&[KeyCode::KeyC], button::L2),
        (&[KeyCode::KeyX], button::L3),
        (&[KeyCode::AltRight], button::R3),
        (&[KeyCode::ArrowUp], button::UP),
        (&[KeyCode::ArrowDown], button::DOWN),
        (&[KeyCode::ArrowLeft], button::LEFT),
        (&[KeyCode::ArrowRight], button::RIGHT),
        (&[KeyCode::KeyM], button::START),
        (&[KeyCode::KeyN], button::SELECT),
    ];
    for (codes, bit) in key_map {
        if codes.iter().any(|c| keys.pressed(*c)) {
            p.buttons |= bit;
        }
    }
    for (b, bit) in [
        (MouseButton::Left, button::CROSS),
        (MouseButton::Right, button::SQUARE),
        (MouseButton::Middle, button::R3),
    ] {
        if mouse.pressed(b) {
            p.buttons |= bit;
        }
    }
    if wheel > 0. {
        p.buttons |= button::UP;
    } else if wheel < 0. {
        p.buttons |= button::DOWN;
    }
    // WASD deflect the left stick fully; Left Alt held: 96 (walk).
    let reach = if keys.pressed(KeyCode::AltLeft) {
        96
    } else {
        127
    };
    let axis = |neg: KeyCode, pos: KeyCode| -> u8 {
        let v = keys.pressed(pos) as i32 - keys.pressed(neg) as i32;
        (0x80 + v * reach).clamp(0, 255) as u8
    };
    p.left = [
        axis(KeyCode::KeyA, KeyCode::KeyD),
        axis(KeyCode::KeyW, KeyCode::KeyS),
    ];
    let keyboard = p.buttons != 0 || p.left != [0x80; 2];
    let mut glyph = None;
    if let Some((g, name)) = pads.iter().next() {
        let map = [
            (GamepadButton::DPadUp, button::UP),
            (GamepadButton::DPadDown, button::DOWN),
            (GamepadButton::DPadLeft, button::LEFT),
            (GamepadButton::DPadRight, button::RIGHT),
            (GamepadButton::Start, button::START),
            (GamepadButton::Select, button::SELECT),
            (GamepadButton::LeftThumb, button::L3),
            (GamepadButton::RightThumb, button::R3),
            (GamepadButton::LeftTrigger, button::L1),
            (GamepadButton::RightTrigger, button::R1),
            (GamepadButton::South, button::CROSS),
            (GamepadButton::East, button::CIRCLE),
            (GamepadButton::West, button::SQUARE),
            (GamepadButton::North, button::TRIANGLE),
        ];
        let mut b = 0;
        for (gb, bit) in map {
            if g.pressed(gb) {
                b |= bit;
            }
        }
        // Triggers count as L2/R2 past 127/255.
        for (gb, bit) in [
            (GamepadButton::LeftTrigger2, button::L2),
            (GamepadButton::RightTrigger2, button::R2),
        ] {
            if g.get(gb).unwrap_or(0.) > 127. / 255. {
                b |= bit;
            }
        }
        // Gamepad Y is up; PADDATA Y grows down.
        let (l, r) = (g.left_stick(), g.right_stick());
        let (ls, rs) = (
            controller::stick_bytes(l.x, -l.y),
            controller::stick_bytes(r.x, -r.y),
        );
        let active = b != 0 || ls != [0x80; 2] || rs != [0x80; 2];
        p.buttons |= b;
        if ls != [0x80; 2] {
            p.left = ls;
        }
        p.right = rs;
        if active {
            let n = name
                .map(|n| n.as_str().to_ascii_lowercase())
                .unwrap_or_default();
            glyph = Some(
                if [
                    "dualsense",
                    "dualshock",
                    "ps4",
                    "ps5",
                    "wireless controller",
                ]
                .iter()
                .any(|k| n.contains(k))
                {
                    2
                } else if n.contains("xbox") || n.contains("xinput") {
                    1
                } else {
                    3
                },
            );
        }
    }
    (p, keyboard, glyph)
}
