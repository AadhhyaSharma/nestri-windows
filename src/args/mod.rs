pub mod encoding_args;

use clap::Parser;

/// Nestri Windows Streaming Server — command-line arguments
#[derive(Parser, Debug, Clone)]
#[command(author, version, about = "Nestri Windows Streaming Server")]
pub struct NestriArgs {
    /// Relay server URL (libp2p multiaddr)
    #[arg(long, env = "NESTRI_RELAY_URL")]
    pub relay_url: Option<String>,

    /// Stream room name
    #[arg(long, env = "NESTRI_ROOM", default_value = "nestri-windows")]
    pub room: Option<String>,

    /// Target framerate
    #[arg(long, env = "NESTRI_FRAMERATE")]
    pub framerate: Option<u32>,

    /// Monitor index to capture (0 = primary)
    #[arg(long, env = "NESTRI_MONITOR")]
    pub monitor_index: Option<u32>,

    /// GPU adapter index (0-based, from DXGI)
    #[arg(long, env = "NESTRI_GPU_INDEX", default_value_t = 0)]
    pub gpu_index: u32,

    /// GPU vendor preference (nvidia/amd/intel)
    #[arg(long, env = "NESTRI_GPU_VENDOR")]
    pub gpu_vendor: Option<String>,

    /// Preferred video codec (h264/h265/av1)
    #[arg(long, env = "NESTRI_VIDEO_CODEC")]
    pub video_codec: Option<String>,

    /// Encoder type preference (hardware/software)
    #[arg(long, env = "NESTRI_ENCODER_TYPE")]
    pub encoder_type: Option<String>,

    /// Force a specific encoder element (e.g. nvh264enc)
    #[arg(long, env = "NESTRI_VIDEO_ENCODER")]
    pub encoder: Option<String>,

    /// Video bitrate in kbps
    #[arg(long, env = "NESTRI_BITRATE_KBPS")]
    pub bitrate_kbps: Option<u32>,

    /// Audio bitrate in kbps
    #[arg(long, env = "NESTRI_AUDIO_BITRATE")]
    pub audio_bitrate_kbps: Option<u32>,

    /// Rate control mode (cbr:8000 / vbr:6000:12000 / cqp:28)
    #[arg(long, env = "NESTRI_RATE_CONTROL", default_value = "cbr:8000")]
    pub rate_control: Option<String>,

    /// Enable verbose logging
    #[arg(long, short = 'v', default_value_t = false)]
    pub verbose: bool,
}
