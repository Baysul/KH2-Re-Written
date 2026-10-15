//! Device packet shapes shared with skate-game's platform layer. The SDL3 and
//! XInput backends are removed; a host builds `XboxState` itself (XInput
//! convention: `wButtons` bits, trigger bytes, signed stick axes, Y up).
use skate_core::input::xbox::XboxState;

/// XInput `wButtons` bits, the layout `xbox::convert` consumes.
pub const BUTTON_NAMES: [(&str, u16); 14] = [
    ("dpad_up", 0x0001),
    ("dpad_down", 0x0002),
    ("dpad_left", 0x0004),
    ("dpad_right", 0x0008),
    ("start", 0x0010),
    ("back", 0x0020),
    ("left_stick", 0x0040),
    ("right_stick", 0x0080),
    ("left_shoulder", 0x0100),
    ("right_shoulder", 0x0200),
    ("a", 0x1000),
    ("b", 0x2000),
    ("x", 0x4000),
    ("y", 0x8000),
];

pub fn button_mask(name: &str) -> Option<u16> {
    BUTTON_NAMES
        .iter()
        .find(|(n, _)| n.eq_ignore_ascii_case(name))
        .map(|&(_, bit)| bit)
}

/// SDL trigger range 0..=32767 back to the XInput byte. SDL expands XInput's
/// byte as b*257 over the full axis, then rescales to 0..=32767, so rounding to
/// nearest returns the original byte exactly.
pub fn xinput_trigger(value: i16) -> u8 {
    ((u32::from(value.max(0) as u16) * 255 + 16383) / 32767) as u8
}

/// SDL reports stick Y as positive-down by storing `~y`; invert it the same way.
pub fn xinput_y(value: i16) -> i16 {
    !value
}

/// XINPUT_DEVSUBTYPE_GAMEPAD, what skate-game's SDL backend reports.
pub const SUBTYPE_GAMEPAD: u8 = 1;

pub(crate) struct DevicePacket {
    pub number: u32,
    pub state: XboxState,
    pub subtype: u8,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum DeviceError {
    Disconnected,
}
