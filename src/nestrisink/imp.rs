/// nestrisink/imp.rs — GObject subclass implementing the Nestri WebRTC signaller
/// Windows port: WaylandSrc → D3D11ScreenCaptureSrc, input dispatch via Win32 SendInput.

use crate::input::controller::ControllerManager;
use crate::p2p::p2p::NestriConnection;
use crate::p2p::p2p_protocol_stream::NestriStreamProtocol;
use crate::proto::proto::proto_message::Payload;
use crate::proto::proto::{
    ProtoControllerAttach, ProtoControllerRumble, ProtoIce, ProtoMessage, ProtoSdp,
    ProtoServerPushStream, RtcIceCandidateInit, RtcSessionDescriptionInit,
};
use crate::input::controller::RumbleEvent;
use anyhow::Result;
use glib::subclass::prelude::*;
use gstreamer::glib;
use gstreamer::prelude::*;
use gstreamer_webrtc::{WebRTCSDPType, WebRTCSessionDescription, gst_sdp};
use gstrswebrtc::signaller::{Signallable, SignallableImpl};
use parking_lot::RwLock as PLRwLock;
use prost::Message as ProstMessage;
use std::sync::{Arc, LazyLock};
use tokio::sync::{Mutex, mpsc};

pub struct Signaller {
    stream_room:        PLRwLock<Option<String>>,
    stream_protocol:    PLRwLock<Option<Arc<NestriStreamProtocol>>>,
    /// Windows: reference to d3d11screencapturesrc (replaces wayland_src)
    screen_src:         PLRwLock<Option<Arc<gstreamer::Element>>>,
    data_channel:       PLRwLock<Option<Arc<gstreamer_webrtc::WebRTCDataChannel>>>,
    controller_manager: PLRwLock<Option<Arc<ControllerManager>>>,
    rumble_rx:          Mutex<Option<mpsc::Receiver<RumbleEvent>>>,
    attach_rx:          Mutex<Option<mpsc::Receiver<u8>>>,
}

impl Default for Signaller {
    fn default() -> Self {
        Self {
            stream_room:        PLRwLock::new(None),
            stream_protocol:    PLRwLock::new(None),
            screen_src:         PLRwLock::new(None),
            data_channel:       PLRwLock::new(None),
            controller_manager: PLRwLock::new(None),
            rumble_rx:          Mutex::new(None),
            attach_rx:          Mutex::new(None),
        }
    }
}

impl Signaller {
    pub async fn set_nestri_connection(&self, conn: NestriConnection) -> Result<()> {
        let proto = NestriStreamProtocol::new(conn).await?;
        *self.stream_protocol.write() = Some(Arc::new(proto));
        Ok(())
    }

    pub fn set_stream_room(&self, room: String) {
        *self.stream_room.write() = Some(room);
    }

    pub fn set_screen_src(&self, src: Arc<gstreamer::Element>) {
        *self.screen_src.write() = Some(src);
    }

    pub fn set_controller_manager(&self, mgr: Arc<ControllerManager>) {
        *self.controller_manager.write() = Some(mgr);
    }

    pub async fn set_rumble_rx(&self, rx: mpsc::Receiver<RumbleEvent>) {
        *self.rumble_rx.lock().await = Some(rx);
    }

    pub async fn set_attach_rx(&self, rx: mpsc::Receiver<u8>) {
        *self.attach_rx.lock().await = Some(rx);
    }

    pub async fn take_rumble_rx(&self) -> Option<mpsc::Receiver<RumbleEvent>> {
        self.rumble_rx.lock().await.take()
    }

    pub async fn take_attach_rx(&self) -> Option<mpsc::Receiver<u8>> {
        self.attach_rx.lock().await.take()
    }

    fn get_stream_protocol(&self) -> Option<Arc<NestriStreamProtocol>> {
        self.stream_protocol.read().clone()
    }

    fn get_screen_src(&self) -> Option<Arc<gstreamer::Element>> {
        self.screen_src.read().clone()
    }

    fn get_controller_manager(&self) -> Option<Arc<ControllerManager>> {
        self.controller_manager.read().clone()
    }

    fn set_data_channel(&self, dc: gstreamer_webrtc::WebRTCDataChannel) {
        *self.data_channel.write() = Some(Arc::new(dc));
    }

    fn register_callbacks(&self) {
        let Some(proto) = self.get_stream_protocol() else {
            gstreamer::error!(gstreamer::CAT_DEFAULT, "Stream protocol not set");
            return;
        };

        // answer → emit session-description
        {
            let self_obj = self.obj().clone();
            proto.register_callback("answer", move |msg| {
                if let Some(Payload::Sdp(sdp)) = msg.payload {
                    if let Some(s) = sdp.sdp {
                        let sdp_msg = gst_sdp::SDPMessage::parse_buffer(s.sdp.as_bytes())
                            .map_err(|e| anyhow::anyhow!("Invalid SDP: {e:?}"))?;
                        let answer = WebRTCSessionDescription::new(WebRTCSDPType::Answer, sdp_msg);
                        self_obj.emit_by_name::<()>("session-description", &[&"unique-session-id", &answer]);
                    }
                } else {
                    anyhow::bail!("Failed to decode answer");
                }
                Ok(())
            });
        }

        // ice-candidate → emit handle-ice
        {
            let self_obj = self.obj().clone();
            proto.register_callback("ice-candidate", move |msg| {
                if let Some(Payload::Ice(ice)) = msg.payload {
                    if let Some(candidate) = ice.candidate {
                        let idx = candidate.sdp_m_line_index.unwrap_or(0);
                        self_obj.emit_by_name::<()>("handle-ice", &[
                            &"unique-session-id", &idx, &candidate.sdp_mid, &candidate.candidate,
                        ]);
                    }
                } else {
                    anyhow::bail!("Failed to decode ICE candidate");
                }
                Ok(())
            });
        }

        // push-stream-ok → request session
        {
            let self_obj = self.obj().clone();
            proto.register_callback("push-stream-ok", move |msg| {
                if let Some(Payload::ServerPushStream(_)) = msg.payload {
                    self_obj.emit_by_name::<()>("session-requested", &[
                        &"unique-session-id", &"consumer-identifier", &None::<WebRTCSessionDescription>,
                    ]);
                } else {
                    anyhow::bail!("Failed to decode push-stream-ok");
                }
                Ok(())
            });
        }

        // webrtcbin-ready → create data channel for input
        {
            let self_obj = self.obj().clone();
            self_obj.connect_closure(
                "webrtcbin-ready",
                false,
                glib::closure!(move |signaller: &super::NestriSignaller, _: &str, webrtcbin: &gstreamer::Element| {
                    let dc = webrtcbin.emit_by_name::<Option<gstreamer_webrtc::WebRTCDataChannel>>(
                        "create-data-channel",
                        &[
                            &"nestri-data-channel",
                            &gstreamer::Structure::builder("config")
                                .field("ordered",         &true)
                                .field("max-retransmits", &2u32)
                                .field("priority",        "high")
                                .field("protocol",        "raw")
                                .build(),
                        ],
                    );
                    if let Some(dc) = dc {
                        signaller.imp().set_data_channel(dc.clone());
                        let signaller  = signaller.clone();
                        let dc         = Arc::new(dc);
                        tokio::spawn(async move {
                            let rumble_rx = signaller.imp().take_rumble_rx().await;
                            let attach_rx = signaller.imp().take_attach_rx().await;
                            let cm        = signaller.imp().get_controller_manager();
                            let screen    = signaller.imp().get_screen_src();
                            setup_data_channel(cm, rumble_rx, attach_rx, dc, screen);
                        });
                    } else {
                        gstreamer::error!(gstreamer::CAT_DEFAULT, "Failed to create data channel");
                    }
                }),
            );
        }
    }
}

impl SignallableImpl for Signaller {
    fn start(&self) {
        gstreamer::info!(gstreamer::CAT_DEFAULT, "Nestri Windows signaller started");
        self.register_callbacks();

        let Some(room) = self.stream_room.read().clone() else {
            gstreamer::error!(gstreamer::CAT_DEFAULT, "Stream room not set");
            return;
        };
        let Some(proto) = self.get_stream_protocol() else {
            gstreamer::error!(gstreamer::CAT_DEFAULT, "Stream protocol not set");
            return;
        };

        let msg = crate::proto::create_message(
            Payload::ServerPushStream(ProtoServerPushStream { room_name: room }),
            "push-stream-room",
            None,
        );
        if let Err(e) = proto.send_message(&msg) {
            tracing::error!("Failed to send push-stream-room: {:?}", e);
        }
    }

    fn stop(&self) {
        gstreamer::info!(gstreamer::CAT_DEFAULT, "Nestri Windows signaller stopped");
    }

    fn send_sdp(&self, _session_id: &str, sdp: &WebRTCSessionDescription) {
        let Some(proto) = self.get_stream_protocol() else { return; };
        let msg = crate::proto::create_message(
            Payload::Sdp(ProtoSdp {
                sdp: Some(RtcSessionDescriptionInit {
                    sdp:    sdp.sdp().as_text().unwrap(),
                    r#type: "offer".to_string(),
                }),
            }),
            "offer",
            None,
        );
        if let Err(e) = proto.send_message(&msg) {
            tracing::error!("Failed to send SDP offer: {:?}", e);
        }
    }

    fn add_ice(&self, _session_id: &str, candidate: &str, sdp_m_line_index: u32, sdp_mid: Option<String>) {
        let Some(proto) = self.get_stream_protocol() else { return; };
        let msg = crate::proto::create_message(
            Payload::Ice(ProtoIce {
                candidate: Some(RtcIceCandidateInit {
                    candidate: candidate.to_string(),
                    sdp_mid,
                    sdp_m_line_index: Some(sdp_m_line_index),
                    ..Default::default()
                }),
            }),
            "ice-candidate",
            None,
        );
        if let Err(e) = proto.send_message(&msg) {
            tracing::error!("Failed to send ICE candidate: {:?}", e);
        }
    }

    fn end_session(&self, session_id: &str) {
        gstreamer::info!(gstreamer::CAT_DEFAULT, "Session ended: {}", session_id);
    }
}

#[glib::object_subclass]
impl ObjectSubclass for Signaller {
    const NAME: &'static str = "NestriWindowsSignaller";
    type Type       = super::NestriSignaller;
    type ParentType = glib::Object;
    type Interfaces = (Signallable,);
}

impl ObjectImpl for Signaller {
    fn properties() -> &'static [glib::ParamSpec] {
        static PROPS: LazyLock<Vec<glib::ParamSpec>> = LazyLock::new(|| {
            vec![
                glib::ParamSpecBoolean::builder("manual-sdp-munging")
                    .nick("Manual SDP munging")
                    .blurb("Whether the signaller manages SDP munging itself")
                    .default_value(false)
                    .read_only()
                    .build(),
            ]
        });
        PROPS.as_ref()
    }
    fn property(&self, _id: usize, pspec: &glib::ParamSpec) -> glib::Value {
        match pspec.name() {
            "manual-sdp-munging" => false.to_value(),
            _ => unimplemented!(),
        }
    }
}

// ─── Data channel input handler ───────────────────────────────────────────────
/// Receives binary protobuf messages from WebRTC data channel.
/// Dispatches keyboard/mouse to Win32 SendInput and gamepad to ViGEmBus.

fn setup_data_channel(
    controller_manager: Option<Arc<ControllerManager>>,
    rumble_rx:  Option<mpsc::Receiver<RumbleEvent>>,
    _attach_rx: Option<mpsc::Receiver<u8>>,
    data_channel: Arc<gstreamer_webrtc::WebRTCDataChannel>,
    _screen_src: Option<Arc<gstreamer::Element>>,
) {
    use crate::input::controller::{dispatch_input, InputEvent};
    use crate::proto::proto::proto_message::Payload;

    let (tx, mut rx) = mpsc::unbounded_channel::<Vec<u8>>();
    let cm = controller_manager.clone();

    // Spawn input processor
    tokio::spawn(async move {
        while let Some(data) = rx.recv().await {
            if let Ok(msg) = ProtoMessage::decode(data.as_slice()) {
                if let Some(base) = &msg.message_base {
                    if base.payload_type == "input" {
                        if let Some(payload) = msg.payload {
                            let event = proto_payload_to_input_event(payload);
                            if let Some(event) = event {
                                dispatch_input(event, cm.clone()).await;
                            }
                        }
                    }
                }
            }
        }
    });

    // Connect data channel on-message signal
    let tx_clone = tx.clone();
    data_channel.connect_closure(
        "on-message-data",
        false,
        glib::closure!(move |_dc: &gstreamer_webrtc::WebRTCDataChannel, data: Option<&glib::Bytes>| {
            if let Some(bytes) = data {
                let _ = tx_clone.send(bytes.to_vec());
            }
        }),
    );

    // Forward rumble events back over data channel
    if let Some(mut rr) = rumble_rx {
        let dc = data_channel.clone();
        let cm2 = controller_manager.clone();
        tokio::spawn(async move {
            while let Some((slot, strong, weak, duration_ms, session_id)) = rr.recv().await {
                let rumble_msg = crate::proto::create_message(
                    Payload::ControllerRumble(ProtoControllerRumble {
                        session_slot:   slot as i32,
                        session_id,
                        low_frequency:  strong as i32,
                        high_frequency: weak as i32,
                        duration:       duration_ms as i32,
                    }),
                    "controller-rumble",
                    None,
                );
                let mut buf = Vec::new();
                if let Ok(()) = ProstMessage::encode(&rumble_msg, &mut buf) {
                    dc.send_data(Some(&glib::Bytes::from_owned(buf)));
                }
            }
        });
    }
}

/// Convert a proto Payload variant to a Windows InputEvent.
fn proto_payload_to_input_event(payload: Payload) -> Option<crate::input::controller::InputEvent> {
    use crate::input::controller::InputEvent;
    match payload {
        Payload::KeyDown(k)         => Some(InputEvent::KeyDown { key_code: k.key as u16, scan_code: k.key as u16 }),
        Payload::KeyUp(k)           => Some(InputEvent::KeyUp   { key_code: k.key as u16, scan_code: k.key as u16 }),
        Payload::MouseMove(m)       => Some(InputEvent::MouseMove { dx: m.x, dy: m.y, absolute: false, x: 0, y: 0 }),
        Payload::MouseMoveAbs(m)    => Some(InputEvent::MouseMove { dx: 0, dy: 0, absolute: true, x: m.x, y: m.y }),
        Payload::MouseWheel(w)      => Some(InputEvent::MouseWheel { delta: w.y }),
        Payload::MouseKeyDown(k)    => Some(InputEvent::MouseButtonDown { button: k.key as u8 }),
        Payload::MouseKeyUp(k)      => Some(InputEvent::MouseButtonUp   { button: k.key as u8 }),
        Payload::ControllerStateBatch(b) => {
            // Map Linux event codes in button_changed_mask → XInput bitmask
            // Full XInput gamepad state reconstruction
            Some(InputEvent::GamepadState {
                slot:          b.session_slot as u8,
                buttons:       linux_buttons_to_xinput(&b.button_changed_mask),
                left_trigger:  b.left_trigger.unwrap_or(0).clamp(-32768, 32767) as u8,
                right_trigger: b.right_trigger.unwrap_or(0).clamp(-32768, 32767) as u8,
                left_x:        b.left_stick_x.unwrap_or(0) as i16,
                left_y:        b.left_stick_y.unwrap_or(0) as i16,
                right_x:       b.right_stick_x.unwrap_or(0) as i16,
                right_y:       b.right_stick_y.unwrap_or(0) as i16,
            })
        }
        _ => None,
    }
}

/// Map Linux gamepad button event codes to XInput button bitmask.
/// Linux BTN_SOUTH(304) = A, BTN_EAST(305) = B, etc.
fn linux_buttons_to_xinput(buttons: &std::collections::HashMap<i32, bool>) -> u16 {
    let mut mask: u16 = 0;
    for (&code, &pressed) in buttons {
        if !pressed { continue; }
        let bit: u16 = match code {
            304 => 0x1000, // BTN_SOUTH = A
            305 => 0x2000, // BTN_EAST  = B
            307 => 0x4000, // BTN_NORTH = Y
            308 => 0x8000, // BTN_WEST  = X
            310 => 0x0100, // BTN_TL    = LB
            311 => 0x0200, // BTN_TR    = RB
            317 => 0x0040, // BTN_THUMBL = LS
            318 => 0x0080, // BTN_THUMBR = RS
            314 => 0x0010, // BTN_SELECT = Back
            315 => 0x0020, // BTN_START  = Start
            316 => 0x0400, // BTN_MODE   = Guide
            _   => 0,
        };
        mask |= bit;
    }
    mask
}
