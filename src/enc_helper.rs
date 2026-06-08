/// enc_helper.rs — Windows encoder selection
/// NVIDIA RTX  → nvh264enc (NVENC via CUDA) — primary target
/// AMD          → amfh264enc / mfh264enc
/// Intel        → qsvh264enc / mfh264enc
/// Fallback     → mfh264enc (Windows Media Foundation)
/// Software     → x264enc / openh264enc

use crate::gpu::{GPUInfo, GPUVendor};
use std::error::Error;

#[derive(Debug, Eq, PartialEq, Clone)]
pub enum VideoCodec { H264, H265, AV1 }

impl std::str::FromStr for VideoCodec {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "h264" | "h.264" | "avc"  => Ok(Self::H264),
            "h265" | "h.265" | "hevc" => Ok(Self::H265),
            "av1"                      => Ok(Self::AV1),
            _                          => Err(format!("Unknown codec: {}", s)),
        }
    }
}

#[derive(Debug, Eq, PartialEq, Clone)]
pub enum EncoderType { SOFTWARE, HARDWARE }

#[derive(Debug, Eq, PartialEq, Clone)]
pub enum EncoderAPI { NVENC, AMF, QSV, MF, SOFTWARE, UNKNOWN }

impl EncoderAPI {
    pub fn to_str(&self) -> &'static str {
        match self {
            Self::NVENC    => "NVIDIA NVENC",
            Self::AMF      => "AMD AMF",
            Self::QSV      => "Intel QuickSync",
            Self::MF       => "Windows Media Foundation",
            Self::SOFTWARE => "Software (x264)",
            Self::UNKNOWN  => "Unknown",
        }
    }
}

#[derive(Debug, Clone)]
pub struct VideoEncoderInfo {
    pub name:         String,
    pub codec:        VideoCodec,
    pub encoder_type: EncoderType,
    pub encoder_api:  EncoderAPI,
}

struct Candidate {
    name:  &'static str,
    codec: VideoCodec,
    api:   EncoderAPI,
    hw:    bool,
}

fn candidates_for(vendor: &GPUVendor) -> Vec<Candidate> {
    match vendor {
        GPUVendor::NVIDIA => vec![
            Candidate { name: "nvh264enc",   codec: VideoCodec::H264, api: EncoderAPI::NVENC,    hw: true  },
            Candidate { name: "nvh265enc",   codec: VideoCodec::H265, api: EncoderAPI::NVENC,    hw: true  },
            Candidate { name: "mfh264enc",   codec: VideoCodec::H264, api: EncoderAPI::MF,       hw: true  },
            Candidate { name: "x264enc",     codec: VideoCodec::H264, api: EncoderAPI::SOFTWARE, hw: false },
        ],
        GPUVendor::AMD => vec![
            Candidate { name: "amfh264enc",  codec: VideoCodec::H264, api: EncoderAPI::AMF,      hw: true  },
            Candidate { name: "mfh264enc",   codec: VideoCodec::H264, api: EncoderAPI::MF,       hw: true  },
            Candidate { name: "x264enc",     codec: VideoCodec::H264, api: EncoderAPI::SOFTWARE, hw: false },
        ],
        GPUVendor::INTEL => vec![
            Candidate { name: "qsvh264enc",  codec: VideoCodec::H264, api: EncoderAPI::QSV,      hw: true  },
            Candidate { name: "mfh264enc",   codec: VideoCodec::H264, api: EncoderAPI::MF,       hw: true  },
            Candidate { name: "x264enc",     codec: VideoCodec::H264, api: EncoderAPI::SOFTWARE, hw: false },
        ],
        _ => vec![
            Candidate { name: "mfh264enc",   codec: VideoCodec::H264, api: EncoderAPI::MF,       hw: true  },
            Candidate { name: "x264enc",     codec: VideoCodec::H264, api: EncoderAPI::SOFTWARE, hw: false },
            Candidate { name: "openh264enc", codec: VideoCodec::H264, api: EncoderAPI::SOFTWARE, hw: false },
        ],
    }
}

/// Probe GStreamer registry and return available encoders for the detected GPU.
pub fn get_compatible_encoders(gpus: &[GPUInfo]) -> Vec<VideoEncoderInfo> {
    let primary_vendor = gpus.iter()
        .min_by_key(|g| match g.vendor {
            GPUVendor::NVIDIA  => 0,
            GPUVendor::AMD     => 1,
            GPUVendor::INTEL   => 2,
            GPUVendor::UNKNOWN => 3,
        })
        .map(|g| &g.vendor)
        .unwrap_or(&GPUVendor::UNKNOWN);

    let mut encoders = Vec::new();
    for c in candidates_for(primary_vendor) {
        // Check GStreamer registry
        if gstreamer::ElementFactory::find(c.name).is_none() {
            tracing::debug!("Encoder '{}' not in GStreamer registry, skipping", c.name);
            continue;
        }
        tracing::info!("[Encoder] {} | API: {}", c.name, c.api.to_str());
        encoders.push(VideoEncoderInfo {
            name: c.name.to_string(),
            codec: c.codec,
            encoder_type: if c.hw { EncoderType::HARDWARE } else { EncoderType::SOFTWARE },
            encoder_api: c.api,
        });
    }
    encoders
}

pub fn get_best_working_encoder(
    encoders: &[VideoEncoderInfo],
    wanted_codec: &Option<VideoCodec>,
    wanted_type:  &Option<EncoderType>,
) -> Result<VideoEncoderInfo, Box<dyn Error>> {
    let mut filtered: Vec<&VideoEncoderInfo> = encoders.iter().collect();

    if let Some(codec) = wanted_codec {
        filtered.retain(|e| &e.codec == codec);
    }
    if let Some(etype) = wanted_type {
        let hw_only = filtered.iter().any(|e| &e.encoder_type == etype);
        if hw_only {
            filtered.retain(|e| &e.encoder_type == etype);
        }
    }

    filtered.into_iter().next()
        .cloned()
        .ok_or_else(|| format!("No suitable encoder found").into())
}
