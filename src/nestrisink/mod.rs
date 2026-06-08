/// nestrisink/mod.rs — WebRTC signaller via Nestri P2P relay
/// Windows port: replaces WaylandSrc references with D3D11ScreenCaptureSrc.
/// The signaller logic itself (SDP/ICE exchange over libp2p) is platform-agnostic.

use crate::input::controller::ControllerManager;
use crate::p2p::p2p::NestriConnection;
use gstreamer::glib;
use gstreamer::subclass::prelude::*;
use gstrswebrtc::signaller::Signallable;
use std::sync::Arc;
use tokio::sync::mpsc;

mod imp;

glib::wrapper! {
    pub struct NestriSignaller(ObjectSubclass<imp::Signaller>)
        @implements Signallable;
}

impl NestriSignaller {
    pub async fn new(
        room: String,
        nestri_conn: NestriConnection,
        /// Windows: d3d11screencapturesrc element (passed for data channel callback reference)
        screen_src: Arc<gstreamer::Element>,
        controller_manager: Option<Arc<ControllerManager>>,
        rumble_rx:  Option<mpsc::Receiver<crate::input::controller::RumbleEvent>>,
        attach_rx:  Option<mpsc::Receiver<u8>>,
    ) -> Result<Self, Box<dyn std::error::Error>> {
        let obj: Self = glib::Object::new();
        obj.imp().set_stream_room(room);
        obj.imp().set_nestri_connection(nestri_conn).await?;
        obj.imp().set_screen_src(screen_src);
        if let Some(cm) = controller_manager {
            obj.imp().set_controller_manager(cm);
        }
        if let Some(rx) = rumble_rx {
            obj.imp().set_rumble_rx(rx).await;
        }
        if let Some(rx) = attach_rx {
            obj.imp().set_attach_rx(rx).await;
        }
        Ok(obj)
    }
}

impl Default for NestriSignaller {
    fn default() -> Self {
        panic!("Cannot create NestriSignaller without NestriConnection");
    }
}
