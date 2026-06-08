/// input/controller.rs — Windows input injection stub
/// Full SendInput + ViGEmBus implementation reserved for data channel phase.

pub struct ControllerManager;

impl ControllerManager {
    pub fn new() -> Self {
        ControllerManager
    }
}

pub struct RumbleEvent {
    pub large_motor: u8,
    pub small_motor: u8,
}
