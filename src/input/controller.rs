/// input/controller.rs — Windows input injection
///
/// Replaces the Linux vimputti/uinput-based system with:
///   - Keyboard & Mouse → Win32 SendInput() API
///   - Gamepad input    → ViGEmBus (vigem-client crate) XInput emulation
///
/// The ControllerManager owns virtual Xbox 360 controllers
/// and accepts input events from the WebRTC data channel.

use anyhow::Result;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::{mpsc, Mutex};
use tracing;

#[cfg(target_os = "windows")]
use windows::Win32::UI::Input::KeyboardAndMouse::{
    SendInput, INPUT, INPUT_0, INPUT_KEYBOARD, INPUT_MOUSE,
    KEYBD_EVENT_FLAGS, KEYEVENTF_KEYUP, KEYEVENTF_SCANCODE,
    KEYBDINPUT, MOUSEINPUT, MOUSE_EVENT_FLAGS,
    MOUSEEVENTF_MOVE, MOUSEEVENTF_LEFTDOWN, MOUSEEVENTF_LEFTUP,
    MOUSEEVENTF_RIGHTDOWN, MOUSEEVENTF_RIGHTUP, MOUSEEVENTF_MIDDLEDOWN,
    MOUSEEVENTF_MIDDLEUP, MOUSEEVENTF_WHEEL, MOUSEEVENTF_ABSOLUTE,
    VIRTUAL_KEY,
};

#[cfg(target_os = "windows")]
use vigem_client::{Client, TargetId, Xbox360Wired, XButtons, XGamepad};

// ─── Input event types (deserialized from WebRTC data channel) ───────────────

#[derive(Debug, Clone)]
pub enum InputEvent {
    // Keyboard
    KeyDown { key_code: u16, scan_code: u16 },
    KeyUp   { key_code: u16, scan_code: u16 },

    // Mouse
    MouseMove     { dx: i32, dy: i32, absolute: bool, x: i32, y: i32 },
    MouseButtonDown { button: u8 },
    MouseButtonUp   { button: u8 },
    MouseWheel    { delta: i32 },

    // Gamepad (XInput compatible)
    GamepadState {
        slot:          u8,
        buttons:       u16,
        left_trigger:  u8,
        right_trigger: u8,
        left_x:        i16,
        left_y:        i16,
        right_x:       i16,
        right_y:       i16,
    },
    GamepadRumble {
        slot:         u8,
        left_motor:   u16,
        right_motor:  u16,
    },
}

// ─── Rumble feedback type (sent back to browser client) ──────────────────────
/// (slot, strong_motor, weak_motor, duration_ms, session_id)
pub type RumbleEvent = (u32, u16, u16, u16, String);

// ─── Controller slot ─────────────────────────────────────────────────────────

#[cfg(target_os = "windows")]
struct VirtualController {
    target: Xbox360Wired<Arc<Client>>,
}

#[cfg(target_os = "windows")]
impl VirtualController {
    fn new(client: Arc<Client>, slot: u8) -> Result<Self> {
        let id = TargetId::XBOX360_WIRED;
        let mut target = Xbox360Wired::new(client, id);
        target.plugin()?;
        target.wait_ready()?;
        tracing::info!("Virtual Xbox360 controller plugged in at slot {}", slot);
        Ok(Self { target })
    }

    fn update(&mut self, gamepad: XGamepad) -> Result<()> {
        self.target.update(&gamepad)?;
        Ok(())
    }
}

// ─── Controller Manager ──────────────────────────────────────────────────────

pub struct ControllerManager {
    #[cfg(target_os = "windows")]
    client: Arc<Client>,

    #[cfg(target_os = "windows")]
    controllers: Mutex<HashMap<u8, VirtualController>>,

    /// Channel to send rumble events back to the WebRTC data channel sender
    rumble_tx: mpsc::Sender<RumbleEvent>,
    /// Channel to notify when a controller attaches
    attach_tx: mpsc::Sender<u8>,
}

impl ControllerManager {
    #[cfg(target_os = "windows")]
    pub fn new() -> Result<(Arc<Self>, mpsc::Receiver<RumbleEvent>, mpsc::Receiver<u8>)> {
        let client = Arc::new(Client::connect()?);
        let (rumble_tx, rumble_rx) = mpsc::channel::<RumbleEvent>(64);
        let (attach_tx, attach_rx) = mpsc::channel::<u8>(16);

        let manager = Arc::new(Self {
            client,
            controllers: Mutex::new(HashMap::new()),
            rumble_tx,
            attach_tx,
        });

        Ok((manager, rumble_rx, attach_rx))
    }

    #[cfg(not(target_os = "windows"))]
    pub fn new() -> Result<(Arc<Self>, mpsc::Receiver<RumbleEvent>, mpsc::Receiver<u8>)> {
        let (rumble_tx, rumble_rx) = mpsc::channel::<RumbleEvent>(64);
        let (attach_tx, attach_rx) = mpsc::channel::<u8>(16);
        Ok((Arc::new(Self { rumble_tx, attach_tx }), rumble_rx, attach_rx))
    }

    /// Handle a full gamepad state update from the client
    #[cfg(target_os = "windows")]
    pub async fn update_gamepad(&self, slot: u8, state: XGamepad) -> Result<()> {
        let mut controllers = self.controllers.lock().await;
        if !controllers.contains_key(&slot) {
            // Auto-create controller when first used
            let ctrl = VirtualController::new(self.client.clone(), slot)?;
            controllers.insert(slot, ctrl);
            let _ = self.attach_tx.send(slot).await;
        }
        if let Some(ctrl) = controllers.get_mut(&slot) {
            ctrl.update(state)?;
        }
        Ok(())
    }

    #[cfg(not(target_os = "windows"))]
    pub async fn update_gamepad(&self, _slot: u8, _state: ()) -> Result<()> { Ok(()) }
}

// ─── Win32 keyboard/mouse injection ──────────────────────────────────────────

#[cfg(target_os = "windows")]
pub fn send_key_down(key_code: u16, scan_code: u16) {
    unsafe {
        let input = INPUT {
            r#type: INPUT_KEYBOARD,
            Anonymous: INPUT_0 {
                ki: KEYBDINPUT {
                    wVk:         VIRTUAL_KEY(key_code),
                    wScan:       scan_code,
                    dwFlags:     KEYEVENTF_SCANCODE,
                    time:        0,
                    dwExtraInfo: 0,
                },
            },
        };
        SendInput(&[input], std::mem::size_of::<INPUT>() as i32);
    }
}

#[cfg(target_os = "windows")]
pub fn send_key_up(key_code: u16, scan_code: u16) {
    unsafe {
        let input = INPUT {
            r#type: INPUT_KEYBOARD,
            Anonymous: INPUT_0 {
                ki: KEYBDINPUT {
                    wVk:         VIRTUAL_KEY(key_code),
                    wScan:       scan_code,
                    dwFlags:     KEYEVENTF_KEYUP | KEYEVENTF_SCANCODE,
                    time:        0,
                    dwExtraInfo: 0,
                },
            },
        };
        SendInput(&[input], std::mem::size_of::<INPUT>() as i32);
    }
}

#[cfg(target_os = "windows")]
pub fn send_mouse_move(dx: i32, dy: i32, absolute: bool, x: i32, y: i32) {
    unsafe {
        let flags = if absolute {
            MOUSEEVENTF_MOVE | MOUSEEVENTF_ABSOLUTE
        } else {
            MOUSEEVENTF_MOVE
        };
        let input = INPUT {
            r#type: INPUT_MOUSE,
            Anonymous: INPUT_0 {
                mi: MOUSEINPUT {
                    dx:          if absolute { x } else { dx },
                    dy:          if absolute { y } else { dy },
                    mouseData:   0,
                    dwFlags:     flags,
                    time:        0,
                    dwExtraInfo: 0,
                },
            },
        };
        SendInput(&[input], std::mem::size_of::<INPUT>() as i32);
    }
}

#[cfg(target_os = "windows")]
pub fn send_mouse_button(button: u8, down: bool) {
    let flags: MOUSE_EVENT_FLAGS = match (button, down) {
        (0, true)  => MOUSEEVENTF_LEFTDOWN,
        (0, false) => MOUSEEVENTF_LEFTUP,
        (1, true)  => MOUSEEVENTF_RIGHTDOWN,
        (1, false) => MOUSEEVENTF_RIGHTUP,
        (2, true)  => MOUSEEVENTF_MIDDLEDOWN,
        (2, false) => MOUSEEVENTF_MIDDLEUP,
        _          => return,
    };
    unsafe {
        let input = INPUT {
            r#type: INPUT_MOUSE,
            Anonymous: INPUT_0 {
                mi: MOUSEINPUT {
                    dx: 0, dy: 0,
                    mouseData:   0,
                    dwFlags:     flags,
                    time:        0,
                    dwExtraInfo: 0,
                },
            },
        };
        SendInput(&[input], std::mem::size_of::<INPUT>() as i32);
    }
}

#[cfg(target_os = "windows")]
pub fn send_mouse_wheel(delta: i32) {
    unsafe {
        let input = INPUT {
            r#type: INPUT_MOUSE,
            Anonymous: INPUT_0 {
                mi: MOUSEINPUT {
                    dx: 0, dy: 0,
                    mouseData:   delta as u32,
                    dwFlags:     MOUSEEVENTF_WHEEL,
                    time:        0,
                    dwExtraInfo: 0,
                },
            },
        };
        SendInput(&[input], std::mem::size_of::<INPUT>() as i32);
    }
}

/// Dispatch an InputEvent to the appropriate Win32 API.
/// Called from the data channel handler.
pub async fn dispatch_input(event: InputEvent, controller_manager: Option<Arc<ControllerManager>>) {
    match event {
        #[cfg(target_os = "windows")]
        InputEvent::KeyDown { key_code, scan_code } => {
            send_key_down(key_code, scan_code);
        }
        #[cfg(target_os = "windows")]
        InputEvent::KeyUp { key_code, scan_code } => {
            send_key_up(key_code, scan_code);
        }
        #[cfg(target_os = "windows")]
        InputEvent::MouseMove { dx, dy, absolute, x, y } => {
            send_mouse_move(dx, dy, absolute, x, y);
        }
        #[cfg(target_os = "windows")]
        InputEvent::MouseButtonDown { button } => {
            send_mouse_button(button, true);
        }
        #[cfg(target_os = "windows")]
        InputEvent::MouseButtonUp { button } => {
            send_mouse_button(button, false);
        }
        #[cfg(target_os = "windows")]
        InputEvent::MouseWheel { delta } => {
            send_mouse_wheel(delta);
        }
        #[cfg(target_os = "windows")]
        InputEvent::GamepadState {
            slot, buttons, left_trigger, right_trigger,
            left_x, left_y, right_x, right_y,
        } => {
            if let Some(mgr) = controller_manager {
                let gamepad = XGamepad {
                    buttons:       XButtons(buttons),
                    left_trigger,
                    right_trigger,
                    thumb_lx:     left_x,
                    thumb_ly:     left_y,
                    thumb_rx:     right_x,
                    thumb_ry:     right_y,
                };
                if let Err(e) = mgr.update_gamepad(slot, gamepad).await {
                    tracing::warn!("Gamepad update error: {}", e);
                }
            }
        }
        _ => {
            tracing::debug!("Input event received (non-Windows build, ignoring)");
        }
    }
}
