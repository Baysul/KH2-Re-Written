use super::*;
use crate::input::platform::{DeviceError, DevicePacket};
use skate_core::input::xbox::XboxState;

impl ControllerInput {
    /// Feed test packets through the production conversion, cache, history and Pad.
    /// Only the operating-system read is supplied by the test.
    pub(crate) fn sample_raw_for_test(&mut self, state: XboxState) {
        self.collect([
            Ok(DevicePacket {
                number: self.publications as u32 + 1,
                state,
                subtype: 1,
                kind: None,
            }),
            Err(DeviceError::Disconnected),
            Err(DeviceError::Disconnected),
            Err(DeviceError::Disconnected),
        ]);
        assert!(self.publish_actions());
    }
}

fn packet(number: u32, buttons: u16, left: [i16; 2]) -> Result<DevicePacket, DeviceError> {
    Ok(DevicePacket {
        number,
        state: XboxState {
            buttons,
            triggers: [0; 2],
            left,
            right: [0; 2],
        },
        subtype: 1,
        kind: None,
    })
}
