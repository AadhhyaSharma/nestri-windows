pub mod encoding_args;

use crate::enc_helper::{EncoderType, VideoCodec};
use clap::Parser;

#[derive(Parser, Debug, Clone)]
#[command(author, version, about = "Nestri Windows Streaming Server")]
pub struct Args {
    #[command(flatten)]
    pub app: AppArgs,

    #[command(flatten)]
    pub device: DeviceArgs,

    #[command(flatten)]
    pub encoding: EncodingArgs,
}

impl Args {
    pub fn new() -> Self {
        Self::parse()
    }

    pub fn debug_print(&self) {
        tracing::debug!("Args: {:?}", self);
    }
}

#[derive(Parser, Debug, Clone)]
pub struct AppArgs {
    /// Relay server URL (libp2p multiaddr)
    #[arg(long, env = "NESTRI_RELAY_URL", default_value = "")]
    pub relay_url: String,

    /// Stream room name
    #[arg(long, env = "NESTRI_ROOM", default_value = "nestri-windows")]
    pub room: String,

    /// Target framerate
    #[arg(long, env = "NESTRI_FRAMERATE", default_value_t = 60)]
    pub framerate: u32,

    /// Enable verbose logging
    #[arg(long, short = 'v', default_value_t = false)]
    pub verbose: bool,

    /// Monitor index to capture (0 = primary)
    #[arg(long, env = "NESTRI_MONITOR", default_value_t = 0)]
    pub monitor_index: u32,

    /// Enable hardware zero-copy path (experimental)
    #[arg(long, default_value_t = false)]
    pub zero_copy: bool,
}

#[derive(Parser, Debug, Clone)]
pub struct DeviceArgs {
    /// GPU vendor to use (nvidia/amd/intel)
    #[arg(long, env = "NESTRI_GPU_VENDOR")]
    pub gpu_vendor: Option<String>,

    /// GPU device name substring filter
    #[arg(long, env = "NESTRI_GPU_NAME")]
    pub gpu_name: Option<String>,

    /// GPU adapter index (0-based, from DXGI)
    #[arg(long, env = "NESTRI_GPU_INDEX")]
    pub gpu_index: Option<u32>,
}

#[derive(Parser, Debug, Clone)]
pub struct EncodingArgs {
    #[command(flatten)]
    pub video: VideoEncodingArgs,

    #[command(flatten)]
    pub audio: AudioEncodingArgs,
}

#[derive(Parser, Debug, Clone)]
pub struct VideoEncodingArgs {
    /// Force a specific encoder (e.g. nvh264enc, mfh264enc, x264enc)
    #[arg(long, env = "NESTRI_VIDEO_ENCODER")]
    pub encoder: Option<String>,

    /// Preferred video codec
    #[arg(long, env = "NESTRI_VIDEO_CODEC", default_value = "h264")]
    pub codec: Option<VideoCodec>,

    /// Prefer hardware or software encoding
    #[arg(long, env = "NESTRI_ENCODER_TYPE", default_value = "hardware")]
    pub encoder_type: Option<EncoderType>,

    /// Latency vs quality tradeoff
    #[arg(long, env = "NESTRI_LATENCY", default_value = "lowest-latency")]
    pub latency_control: encoding_args::LatencyControl,

    /// Rate control mode
    #[arg(long, env = "NESTRI_RATE_CONTROL", default_value = "cbr:8000")]
    pub rate_control_str: String,

    /// Keyframe interval in seconds
    #[arg(long, env = "NESTRI_KEYFRAME_SECS", default_value_t = 2)]
    pub keyframe_dist_secs: u32,
}

impl VideoEncodingArgs {
    pub fn rate_control(&self) -> encoding_args::RateControl {
        encoding_args::RateControl::parse(&self.rate_control_str)
    }
}

#[derive(Parser, Debug, Clone)]
pub struct AudioEncodingArgs {
    /// Force audio encoder name
    #[arg(long, env = "NESTRI_AUDIO_ENCODER")]
    pub encoder: Option<String>,

    /// Audio bitrate kbps
    #[arg(long, env = "NESTRI_AUDIO_BITRATE", default_value_t = 128)]
    pub bitrate_kbps: u32,
}
