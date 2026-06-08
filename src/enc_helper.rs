/// enc_helper.rs — Windows encoder selection
/// Replaces Linux VA-API / VAAPI encoders with:
///   NVIDIA RTX  → nvh264enc / nvh265enc / nvav1enc  (NVENC via CUDA)
///   AMD          → amfh264enc / amfh265enc           (AMF)
///   Intel        → qsvh264enc / qsvh265enc            (QuickSync)
///   Fallback     → mfh264enc / mfh265enc              (Windows Media Foundation)
///   Software     → x264enc / x265enc / svtav1enc

use crate::args::encoding_args::RateControl;
use crate::gpu::{GPUInfo, GPUVendor};
use clap::ValueEnum;
use gstreamer::prelude::*;
use std::error::Error;
use std::str::FromStr;

// ─── Codec types ─────────────────────────────────────────────────────────────

#[derive(Debug, Eq, PartialEq, Clone, ValueEnum)]
pub enum AudioCodec { OPUS }

impl AudioCodec {
    pub fn as_str(&self) -> &'static str {
        match self { Self::OPUS => "Opus" }
    }
}

impl FromStr for AudioCodec {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "opus" => Ok(Self::OPUS),
            _      => Err(format!("Invalid audio codec: {}", s)),
        }
    }
}

#[derive(Debug, Eq, PartialEq, Clone, ValueEnum)]
pub enum VideoCodec { H264, H265, AV1 }

impl VideoCodec {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::H264 => "H.264",
            Self::H265 => "H.265",
            Self::AV1  => "AV1",
        }
    }
}

impl FromStr for VideoCodec {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "h264" | "h.264" | "avc"         => Ok(Self::H264),
            "h265" | "h.265" | "hevc"        => Ok(Self::H265),
            "av1"                            => Ok(Self::AV1),
            _                               => Err(format!("Invalid codec: {}", s)),
        }
    }
}

#[derive(Debug, Eq, PartialEq, Clone)]
pub enum EncoderAPI {
    NVENC,      // NVIDIA NVENC (Windows CUDA path)
    AMF,        // AMD Advanced Media Framework
    QSV,        // Intel QuickSync
    MF,         // Windows Media Foundation (generic HW fallback)
    SOFTWARE,
    UNKNOWN,
}

impl EncoderAPI {
    pub fn to_str(&self) -> &'static str {
        match self {
            Self::NVENC    => "NVIDIA NVENC",
            Self::AMF      => "AMD AMF",
            Self::QSV      => "Intel QuickSync",
            Self::MF       => "Windows Media Foundation",
            Self::SOFTWARE => "Software",
            Self::UNKNOWN  => "Unknown",
        }
    }
}

#[derive(Debug, Eq, PartialEq, Clone, ValueEnum)]
pub enum EncoderType { SOFTWARE, HARDWARE }

impl EncoderType {
    pub fn as_str(&self) -> &'static str {
        match self { Self::SOFTWARE => "Software", Self::HARDWARE => "Hardware" }
    }
}

// ─── Encoder info ────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct VideoEncoderInfo {
    pub name:         String,
    pub codec:        VideoCodec,
    pub encoder_type: EncoderType,
    pub encoder_api:  EncoderAPI,
    pub parameters:   Vec<(String, String)>,
    pub gpu_info:     Option<GPUInfo>,
}

impl VideoEncoderInfo {
    pub fn new(
        name: String,
        codec: VideoCodec,
        encoder_type: EncoderType,
        encoder_api: EncoderAPI,
    ) -> Self {
        Self { name, codec, encoder_type, encoder_api, parameters: Vec::new(), gpu_info: None }
    }

    pub fn get_parameters_string(&self) -> String {
        self.parameters.iter().map(|(k, v)| format!("{}={}", k, v)).collect::<Vec<_>>().join(" ")
    }

    pub fn set_parameter(&mut self, key: &str, value: &str) {
        self.parameters.push((key.into(), value.into()));
    }

    pub fn apply_parameters(&self, element: &gstreamer::Element, verbose: bool) {
        for (key, value) in &self.parameters {
            if element.has_property(key) {
                if verbose { tracing::debug!("Setting {} = {}", key, value); }
                element.set_property_from_str(key, value);
            }
        }
    }
}

// ─── Windows encoder catalogue ───────────────────────────────────────────────
/// Returns the preferred ordered list of (gst_element_name, codec, api) for a given GPU vendor.
/// We probe GStreamer registry at runtime to check availability.

struct EncoderCandidate {
    name:    &'static str,
    codec:   VideoCodec,
    api:     EncoderAPI,
    hw:      bool,
}

fn windows_encoder_candidates(vendor: &GPUVendor) -> Vec<EncoderCandidate> {
    match vendor {
        GPUVendor::NVIDIA => vec![
            // NVENC path — best for RTX series
            EncoderCandidate { name: "nvh264enc",    codec: VideoCodec::H264, api: EncoderAPI::NVENC, hw: true },
            EncoderCandidate { name: "nvh265enc",    codec: VideoCodec::H265, api: EncoderAPI::NVENC, hw: true },
            EncoderCandidate { name: "nvav1enc",     codec: VideoCodec::AV1,  api: EncoderAPI::NVENC, hw: true },
            // Media Foundation fallback
            EncoderCandidate { name: "mfh264enc",    codec: VideoCodec::H264, api: EncoderAPI::MF,    hw: true },
            EncoderCandidate { name: "mfh265enc",    codec: VideoCodec::H265, api: EncoderAPI::MF,    hw: true },
            // Software fallback
            EncoderCandidate { name: "x264enc",      codec: VideoCodec::H264, api: EncoderAPI::SOFTWARE, hw: false },
            EncoderCandidate { name: "openh264enc",  codec: VideoCodec::H264, api: EncoderAPI::SOFTWARE, hw: false },
        ],
        GPUVendor::AMD => vec![
            EncoderCandidate { name: "amfh264enc",   codec: VideoCodec::H264, api: EncoderAPI::AMF,   hw: true },
            EncoderCandidate { name: "amfh265enc",   codec: VideoCodec::H265, api: EncoderAPI::AMF,   hw: true },
            EncoderCandidate { name: "mfh264enc",    codec: VideoCodec::H264, api: EncoderAPI::MF,    hw: true },
            EncoderCandidate { name: "mfh265enc",    codec: VideoCodec::H265, api: EncoderAPI::MF,    hw: true },
            EncoderCandidate { name: "x264enc",      codec: VideoCodec::H264, api: EncoderAPI::SOFTWARE, hw: false },
        ],
        GPUVendor::INTEL => vec![
            EncoderCandidate { name: "qsvh264enc",   codec: VideoCodec::H264, api: EncoderAPI::QSV,   hw: true },
            EncoderCandidate { name: "qsvh265enc",   codec: VideoCodec::H265, api: EncoderAPI::QSV,   hw: true },
            EncoderCandidate { name: "mfh264enc",    codec: VideoCodec::H264, api: EncoderAPI::MF,    hw: true },
            EncoderCandidate { name: "mfh265enc",    codec: VideoCodec::H265, api: EncoderAPI::MF,    hw: true },
            EncoderCandidate { name: "x264enc",      codec: VideoCodec::H264, api: EncoderAPI::SOFTWARE, hw: false },
        ],
        _ => vec![
            // Unknown vendor: try MF first (works on any Windows GPU), then software
            EncoderCandidate { name: "mfh264enc",    codec: VideoCodec::H264, api: EncoderAPI::MF,    hw: true },
            EncoderCandidate { name: "mfh265enc",    codec: VideoCodec::H265, api: EncoderAPI::MF,    hw: true },
            EncoderCandidate { name: "x264enc",      codec: VideoCodec::H264, api: EncoderAPI::SOFTWARE, hw: false },
            EncoderCandidate { name: "openh264enc",  codec: VideoCodec::H264, api: EncoderAPI::SOFTWARE, hw: false },
        ],
    }
}

/// Probe GStreamer registry and return available encoders for the detected GPU.
pub fn get_compatible_encoders(gpus: &[GPUInfo]) -> Vec<VideoEncoderInfo> {
    let mut encoders = Vec::new();
    let registry = gstreamer::Registry::get();

    // Determine the primary GPU vendor (NVIDIA RTX gets priority)
    let primary_vendor = gpus.iter()
        .min_by_key(|g| match g.vendor {
            GPUVendor::NVIDIA  => 0,
            GPUVendor::AMD     => 1,
            GPUVendor::INTEL   => 2,
            GPUVendor::UNKNOWN => 3,
        })
        .map(|g| &g.vendor)
        .unwrap_or(&GPUVendor::UNKNOWN);

    let candidates = windows_encoder_candidates(primary_vendor);
    let primary_gpu = gpus.iter().find(|g| &g.vendor == primary_vendor);

    for candidate in candidates {
        // Check if GStreamer has this encoder plugin available
        if registry.find_feature(candidate.name, gstreamer::ElementFactory::static_type()).is_none() {
            tracing::debug!("Encoder '{}' not found in GStreamer registry, skipping", candidate.name);
            continue;
        }

        // Try to instantiate to confirm it actually works at runtime
        let test_elem = gstreamer::ElementFactory::make(candidate.name).build();
        if test_elem.is_err() {
            tracing::warn!("Encoder '{}' found but failed to instantiate, skipping", candidate.name);
            continue;
        }

        let encoder_type = if candidate.hw { EncoderType::HARDWARE } else { EncoderType::SOFTWARE };

        let mut info = VideoEncoderInfo::new(
            candidate.name.to_string(),
            candidate.codec,
            encoder_type,
            candidate.api,
        );

        if candidate.hw {
            info.gpu_info = primary_gpu.cloned();
        }

        tracing::info!(
            "> [Encoder] {} | Codec: {} | API: {}",
            candidate.name,
            info.codec.as_str(),
            info.encoder_api.to_str()
        );

        encoders.push(info);
    }

    encoders
}

pub fn get_encoder_by_name<'a>(
    encoders: &'a [VideoEncoderInfo],
    name: &str,
) -> Result<VideoEncoderInfo, Box<dyn Error>> {
    encoders.iter()
        .find(|e| e.name.eq_ignore_ascii_case(name))
        .cloned()
        .ok_or_else(|| format!("Encoder '{}' not found or not available", name).into())
}

pub fn get_best_working_encoder(
    encoders: &[VideoEncoderInfo],
    wanted_codec: &Option<VideoCodec>,
    wanted_type: &Option<EncoderType>,
) -> Result<VideoEncoderInfo, Box<dyn Error>> {
    let mut filtered: Vec<&VideoEncoderInfo> = encoders.iter().collect();

    if let Some(codec) = wanted_codec {
        filtered.retain(|e| &e.codec == codec);
    }
    if let Some(enc_type) = wanted_type {
        filtered.retain(|e| &e.encoder_type == enc_type);
    }

    // Prefer hardware > software, NVENC > AMF > QSV > MF > SW
    filtered.sort_by_key(|e| match e.encoder_api {
        EncoderAPI::NVENC    => 0,
        EncoderAPI::AMF      => 1,
        EncoderAPI::QSV      => 2,
        EncoderAPI::MF       => 3,
        EncoderAPI::SOFTWARE => 4,
        EncoderAPI::UNKNOWN  => 5,
    });

    filtered.first()
        .cloned()
        .cloned()
        .ok_or_else(|| "No compatible encoder found for the requested settings".into())
}

// ─── Encoder parameter helpers ───────────────────────────────────────────────

fn modify_encoder_params<F>(encoder: &VideoEncoderInfo, mut param_check: F) -> VideoEncoderInfo
where
    F: FnMut(&str) -> Option<(String, String)>,
{
    let mut enc = encoder.clone();
    let element = match gstreamer::ElementFactory::make(&enc.name).build() {
        Ok(e)  => e,
        Err(_) => return enc,
    };
    element.list_properties().iter().for_each(|prop| {
        let name = prop.name();
        if let Some((k, v)) = param_check(name) {
            enc.set_parameter(&k, &v);
        }
    });
    enc
}

pub fn encoder_cqp_params(encoder: &VideoEncoderInfo, quality: u32) -> VideoEncoderInfo {
    modify_encoder_params(encoder, |prop| {
        let pl = prop.to_lowercase();
        if !pl.contains("qp") { return None; }
        if pl.contains("i") || pl.contains("min") {
            Some((prop.into(), quality.to_string()))
        } else if pl.contains("p") || pl.contains("max") {
            Some((prop.into(), (quality + 2).to_string()))
        } else { None }
    })
}

pub fn encoder_vbr_params(encoder: &VideoEncoderInfo, bitrate: u32, max_bitrate: u32) -> VideoEncoderInfo {
    modify_encoder_params(encoder, |prop| {
        let pl = prop.to_lowercase();
        if !pl.contains("bitrate") { return None; }
        if !pl.contains("max") {
            Some((prop.into(), bitrate.to_string()))
        } else {
            Some((prop.into(), max_bitrate.to_string()))
        }
    })
}

pub fn encoder_cbr_params(encoder: &VideoEncoderInfo, bitrate: u32) -> VideoEncoderInfo {
    modify_encoder_params(encoder, |prop| {
        let pl = prop.to_lowercase();
        if pl.contains("bitrate") && !pl.contains("max") {
            Some((prop.into(), bitrate.to_string()))
        } else { None }
    })
}

pub fn encoder_gop_params(encoder: &VideoEncoderInfo, gop_size: u32) -> VideoEncoderInfo {
    modify_encoder_params(encoder, |prop| {
        let pl = prop.to_lowercase();
        if pl.contains("gop-size") || pl.contains("int-max") || pl.contains("max-dist")
            || pl.contains("intra-period-length") {
            Some((prop.into(), gop_size.to_string()))
        } else { None }
    })
}

pub fn encoder_low_latency_params(
    encoder: &VideoEncoderInfo,
    _rate_control: &RateControl,
    framerate: u32,
    keyframe_dist_secs: u32,
) -> VideoEncoderInfo {
    let mut enc = encoder_gop_params(encoder, framerate * keyframe_dist_secs);
    match enc.encoder_api {
        EncoderAPI::NVENC => {
            // RTX-optimized ultra-low-latency settings
            enc.set_parameter("multi-pass", "disabled");
            enc.set_parameter("preset", "p1");          // Fastest preset
            enc.set_parameter("tune", "ultra-low-latency");
            enc.set_parameter("zerolatency", "true");
            enc.set_parameter("rc-mode", "cbr");        // CBR for consistent latency
            enc.set_parameter("aq-strength", "0");      // Disable AQ for speed
        }
        EncoderAPI::AMF => {
            enc.set_parameter("usage", "ultra-low-latency");
            enc.set_parameter("preset", "speed");
        }
        EncoderAPI::QSV => {
            enc.set_parameter("low-latency", "true");
            enc.set_parameter("target-usage", "7");
        }
        EncoderAPI::MF => {
            enc.set_parameter("low-latency", "true");
        }
        EncoderAPI::SOFTWARE => {
            match enc.name.as_str() {
                "x264enc" => {
                    enc.set_parameter("rc-lookahead", "0");
                    enc.set_parameter("speed-preset", "ultrafast");
                    enc.set_parameter("tune", "zerolatency");
                }
                "openh264enc" => {
                    enc.set_parameter("complexity", "low");
                    enc.set_parameter("usage-type", "screen");
                }
                _ => {}
            }
        }
        _ => {}
    }
    enc
}

pub fn encoder_high_quality_params(
    encoder: &VideoEncoderInfo,
    _rate_control: &RateControl,
    framerate: u32,
    keyframe_dist_secs: u32,
) -> VideoEncoderInfo {
    let mut enc = encoder_gop_params(encoder, framerate * keyframe_dist_secs);
    match enc.encoder_api {
        EncoderAPI::NVENC => {
            enc.set_parameter("multi-pass", "two-pass");
            enc.set_parameter("preset", "p7");
            enc.set_parameter("tune", "high-quality");
            enc.set_parameter("zerolatency", "false");
            enc.set_parameter("spatial-aq", "true");
            enc.set_parameter("rc-lookahead", "3");
        }
        EncoderAPI::AMF => {
            enc.set_parameter("usage", "transcoding");
            enc.set_parameter("preset", "quality");
        }
        EncoderAPI::QSV => {
            enc.set_parameter("low-latency", "false");
            enc.set_parameter("target-usage", "1");
        }
        EncoderAPI::MF => {
            enc.set_parameter("low-latency", "false");
        }
        EncoderAPI::SOFTWARE => {
            match enc.name.as_str() {
                "x264enc" => {
                    enc.set_parameter("rc-lookahead", "3");
                    enc.set_parameter("speed-preset", "medium");
                }
                _ => {}
            }
        }
        _ => {}
    }
    enc
}
