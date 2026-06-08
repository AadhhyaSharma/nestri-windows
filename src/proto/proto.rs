// Proto message definitions — copied verbatim from original Nestri source.
// These are platform-agnostic (pure Rust/prost, no Linux deps).

#[derive(Clone, PartialEq, Eq, Hash, ::prost::Message)]
pub struct ProtoTimestampEntry {
    #[prost(string, tag = "1")]
    pub stage: ::prost::alloc::string::String,
    #[prost(message, optional, tag = "2")]
    pub time: ::core::option::Option<::prost_types::Timestamp>,
}

#[derive(Clone, PartialEq, ::prost::Message)]
pub struct ProtoLatencyTracker {
    #[prost(string, tag = "1")]
    pub sequence_id: ::prost::alloc::string::String,
    #[prost(message, repeated, tag = "2")]
    pub timestamps: ::prost::alloc::vec::Vec<ProtoTimestampEntry>,
}

// Mouse messages
#[derive(Clone, Copy, PartialEq, Eq, Hash, ::prost::Message)]
pub struct ProtoMouseMove {
    #[prost(int32, tag = "1")] pub x: i32,
    #[prost(int32, tag = "2")] pub y: i32,
}
#[derive(Clone, Copy, PartialEq, Eq, Hash, ::prost::Message)]
pub struct ProtoMouseMoveAbs {
    #[prost(int32, tag = "1")] pub x: i32,
    #[prost(int32, tag = "2")] pub y: i32,
}
#[derive(Clone, Copy, PartialEq, Eq, Hash, ::prost::Message)]
pub struct ProtoMouseWheel {
    #[prost(int32, tag = "1")] pub x: i32,
    #[prost(int32, tag = "2")] pub y: i32,
}
#[derive(Clone, Copy, PartialEq, Eq, Hash, ::prost::Message)]
pub struct ProtoMouseKeyDown { #[prost(int32, tag = "1")] pub key: i32 }
#[derive(Clone, Copy, PartialEq, Eq, Hash, ::prost::Message)]
pub struct ProtoMouseKeyUp   { #[prost(int32, tag = "1")] pub key: i32 }

// Keyboard messages
#[derive(Clone, Copy, PartialEq, Eq, Hash, ::prost::Message)]
pub struct ProtoKeyDown { #[prost(int32, tag = "1")] pub key: i32 }
#[derive(Clone, Copy, PartialEq, Eq, Hash, ::prost::Message)]
pub struct ProtoKeyUp   { #[prost(int32, tag = "1")] pub key: i32 }

// Controller messages
#[derive(Clone, PartialEq, Eq, Hash, ::prost::Message)]
pub struct ProtoControllerAttach {
    #[prost(string, tag = "1")] pub id: ::prost::alloc::string::String,
    #[prost(int32,  tag = "2")] pub session_slot: i32,
    #[prost(string, tag = "3")] pub session_id: ::prost::alloc::string::String,
}
#[derive(Clone, PartialEq, Eq, Hash, ::prost::Message)]
pub struct ProtoControllerDetach {
    #[prost(int32,  tag = "1")] pub session_slot: i32,
    #[prost(string, tag = "2")] pub session_id: ::prost::alloc::string::String,
}
#[derive(Clone, PartialEq, Eq, Hash, ::prost::Message)]
pub struct ProtoControllerRumble {
    #[prost(int32,  tag = "1")] pub session_slot: i32,
    #[prost(string, tag = "2")] pub session_id: ::prost::alloc::string::String,
    #[prost(int32,  tag = "3")] pub low_frequency: i32,
    #[prost(int32,  tag = "4")] pub high_frequency: i32,
    #[prost(int32,  tag = "5")] pub duration: i32,
}
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct ProtoControllerStateBatch {
    #[prost(int32,  tag = "1")]  pub session_slot: i32,
    #[prost(string, tag = "2")]  pub session_id: ::prost::alloc::string::String,
    #[prost(enumeration = "proto_controller_state_batch::UpdateType", tag = "3")] pub update_type: i32,
    #[prost(uint32, tag = "4")]  pub sequence: u32,
    #[prost(map = "int32, bool", tag = "5")] pub button_changed_mask: ::std::collections::HashMap<i32, bool>,
    #[prost(int32,  optional, tag = "6")]  pub left_stick_x:   ::core::option::Option<i32>,
    #[prost(int32,  optional, tag = "7")]  pub left_stick_y:   ::core::option::Option<i32>,
    #[prost(int32,  optional, tag = "8")]  pub right_stick_x:  ::core::option::Option<i32>,
    #[prost(int32,  optional, tag = "9")]  pub right_stick_y:  ::core::option::Option<i32>,
    #[prost(int32,  optional, tag = "10")] pub left_trigger:   ::core::option::Option<i32>,
    #[prost(int32,  optional, tag = "11")] pub right_trigger:  ::core::option::Option<i32>,
    #[prost(int32,  optional, tag = "12")] pub dpad_x:         ::core::option::Option<i32>,
    #[prost(int32,  optional, tag = "13")] pub dpad_y:         ::core::option::Option<i32>,
    #[prost(uint32, optional, tag = "14")] pub changed_fields: ::core::option::Option<u32>,
}
pub mod proto_controller_state_batch {
    #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, ::prost::Enumeration)]
    #[repr(i32)]
    pub enum UpdateType { FullState = 0, Delta = 1 }
    impl UpdateType {
        pub fn as_str_name(&self) -> &'static str {
            match self { Self::FullState => "FULL_STATE", Self::Delta => "DELTA" }
        }
    }
}

// WebRTC / signaling
#[derive(Clone, PartialEq, Eq, Hash, ::prost::Message)]
pub struct RtcIceCandidateInit {
    #[prost(string, tag = "1")] pub candidate: ::prost::alloc::string::String,
    #[prost(uint32, optional, tag = "2")] pub sdp_m_line_index: ::core::option::Option<u32>,
    #[prost(string, optional, tag = "3")] pub sdp_mid: ::core::option::Option<::prost::alloc::string::String>,
    #[prost(string, optional, tag = "4")] pub username_fragment: ::core::option::Option<::prost::alloc::string::String>,
}
#[derive(Clone, PartialEq, Eq, Hash, ::prost::Message)]
pub struct RtcSessionDescriptionInit {
    #[prost(string, tag = "1")] pub sdp: ::prost::alloc::string::String,
    #[prost(string, tag = "2")] pub r#type: ::prost::alloc::string::String,
}
#[derive(Clone, PartialEq, Eq, Hash, ::prost::Message)]
pub struct ProtoIce {
    #[prost(message, optional, tag = "1")] pub candidate: ::core::option::Option<RtcIceCandidateInit>,
}
#[derive(Clone, PartialEq, Eq, Hash, ::prost::Message)]
pub struct ProtoSdp {
    #[prost(message, optional, tag = "1")] pub sdp: ::core::option::Option<RtcSessionDescriptionInit>,
}
#[derive(Clone, PartialEq, Eq, Hash, ::prost::Message)]
pub struct ProtoRaw { #[prost(string, tag = "1")] pub data: ::prost::alloc::string::String }

#[derive(Clone, PartialEq, Eq, Hash, ::prost::Message)]
pub struct ProtoClientRequestRoomStream {
    #[prost(string, tag = "1")] pub room_name: ::prost::alloc::string::String,
    #[prost(string, tag = "2")] pub session_id: ::prost::alloc::string::String,
}
#[derive(Clone, PartialEq, Eq, Hash, ::prost::Message)]
pub struct ProtoClientDisconnected {
    #[prost(string, tag = "1")] pub session_id: ::prost::alloc::string::String,
    #[prost(int32, repeated, tag = "2")] pub controller_slots: ::prost::alloc::vec::Vec<i32>,
}
#[derive(Clone, PartialEq, Eq, Hash, ::prost::Message)]
pub struct ProtoServerPushStream {
    #[prost(string, tag = "1")] pub room_name: ::prost::alloc::string::String,
}

#[derive(Clone, PartialEq, ::prost::Message)]
pub struct ProtoMessageBase {
    #[prost(string, tag = "1")]  pub payload_type: ::prost::alloc::string::String,
    #[prost(message, optional, tag = "2")] pub latency: ::core::option::Option<ProtoLatencyTracker>,
}

#[derive(Clone, PartialEq, ::prost::Message)]
pub struct ProtoMessage {
    #[prost(message, optional, tag = "1")] pub message_base: ::core::option::Option<ProtoMessageBase>,
    #[prost(oneof = "proto_message::Payload", tags = "2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 20, 21, 22, 23, 24, 25")]
    pub payload: ::core::option::Option<proto_message::Payload>,
}

pub mod proto_message {
    #[derive(Clone, PartialEq, ::prost::Oneof)]
    pub enum Payload {
        #[prost(message, tag = "2")]  MouseMove(super::ProtoMouseMove),
        #[prost(message, tag = "3")]  MouseMoveAbs(super::ProtoMouseMoveAbs),
        #[prost(message, tag = "4")]  MouseWheel(super::ProtoMouseWheel),
        #[prost(message, tag = "5")]  MouseKeyDown(super::ProtoMouseKeyDown),
        #[prost(message, tag = "6")]  MouseKeyUp(super::ProtoMouseKeyUp),
        #[prost(message, tag = "7")]  KeyDown(super::ProtoKeyDown),
        #[prost(message, tag = "8")]  KeyUp(super::ProtoKeyUp),
        #[prost(message, tag = "9")]  ControllerAttach(super::ProtoControllerAttach),
        #[prost(message, tag = "10")] ControllerDetach(super::ProtoControllerDetach),
        #[prost(message, tag = "11")] ControllerRumble(super::ProtoControllerRumble),
        #[prost(message, tag = "12")] ControllerStateBatch(super::ProtoControllerStateBatch),
        #[prost(message, tag = "20")] Ice(super::ProtoIce),
        #[prost(message, tag = "21")] Sdp(super::ProtoSdp),
        #[prost(message, tag = "22")] Raw(super::ProtoRaw),
        #[prost(message, tag = "23")] ClientRequestRoomStream(super::ProtoClientRequestRoomStream),
        #[prost(message, tag = "24")] ClientDisconnected(super::ProtoClientDisconnected),
        #[prost(message, tag = "25")] ServerPushStream(super::ProtoServerPushStream),
    }
}
