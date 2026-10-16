use super::*;
use sdl3::gamepad::GamepadType as T;

fn sdl(gamepad_type: T, vendor_product: Option<(u16, u16)>, paddles: u8) -> ControllerKind {
    from_sdl(
        SdlReport {
            name: "Test pad".into(),
            gamepad_type,
            vendor_id: vendor_product.map(|v| v.0),
            product_id: vendor_product.map(|v| v.1),
            path: "XInput#0".into(),
            paddles,
            touchpad: false,
            misc_button: false,
        },
        &[],
    )
}









/// Reads slot 0 through XInput (and XInputGetCapabilitiesEx) on this machine.
/// Needs a connected pad; prints what the fallback reports.
#[cfg(windows)]
#[test]
#[ignore]
fn xinput_identity_of_connected_pad() {
    let kinds: Vec<_> = (0..4).map(crate::input::platform::xinput_identity).collect();
    for (slot, kind) in kinds.iter().enumerate() {
        println!("XInput slot {slot}: {:?}", kind.as_ref().map(ControllerKind::summary));
    }
    assert!(kinds.iter().any(Option::is_some), "no XInput pad connected");
}
