/// proto/mod.rs — Protobuf message definitions (unchanged from original)
/// These are auto-generated from protobuf schemas and are platform-agnostic.

pub mod proto;

use proto::proto_message::Payload;
use proto::{ProtoMessage, ProtoMessageBase, ProtoLatencyTracker};
use prost::Message;

/// Helper to create a wrapped ProtoMessage with a payload_type label.
pub fn create_message(payload: Payload, payload_type: &str, latency: Option<ProtoLatencyTracker>) -> ProtoMessage {
    ProtoMessage {
        message_base: Some(ProtoMessageBase {
            payload_type: payload_type.to_string(),
            latency,
        }),
        payload: Some(payload),
    }
}
