/// nestri-server — Windows streaming server
///
/// Captures screen via D3D11, encodes with NVENC (NVIDIA RTX),
/// streams via GStreamer WebRTC to https://xtreme-gaming.pages.dev
///
/// Web client: https://xtreme-gaming.pages.dev
/// Stream URL: https://xtreme-gaming.pages.dev/play?room=<your-room-name>

use anyhow::{Context, Result, anyhow};
use clap::Parser;
use gstreamer::prelude::*;

mod args;
mod enc_helper;
mod gpu;
mod input;

use args::NestriArgs;
use enc_helper::{EncoderType, VideoCodec};

pub const WEB_CLIENT_URL: &str = "https://xtreme-gaming.pages.dev";
pub const DEFAULT_RELAY: &str =
    "/dnsaddr/relay.dathorse.com/p2p/12D3KooWPK4v5wKYNYx9oXWjqLM8Xix6nm13o91j1Feqq98fLBsw";

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::from_default_env()
                .add_directive("nestri=info".parse().unwrap()),
        )
        .init();

    let args = NestriArgs::parse();

    // ── GStreamer init ───────────────────────────────────────────────────────
    gstreamer::init().context("Failed to initialize GStreamer")?;

    // ── GPU detection ────────────────────────────────────────────────────────
    tracing::info!("Enumerating GPUs via DXGI...");
    let gpus = gpu::get_gpus().map_err(|e| anyhow!("GPU enumeration failed: {}", e))?;
    if gpus.is_empty() {
        return Err(anyhow!("No GPUs found via DXGI"));
    }

    let gpu = gpus
        .iter()
        .find(|g| g.adapter_idx == args.gpu_index)
        .unwrap_or(&gpus[0]);

    tracing::info!("Selected GPU: {}", gpu);

    // ── Encoder selection ────────────────────────────────────────────────────
    let available_encoders = enc_helper::get_compatible_encoders(&gpus);
    if available_encoders.is_empty() {
        return Err(anyhow!("No compatible GStreamer encoders found. Is GStreamer installed?"));
    }

    let wanted_codec: Option<VideoCodec> = args.video_codec
        .as_deref()
        .and_then(|s| s.parse().ok());

    let wanted_type: Option<EncoderType> = args.encoder_type.as_deref().map(|s| {
        if s.eq_ignore_ascii_case("hardware") { EncoderType::HARDWARE } else { EncoderType::SOFTWARE }
    });

    let encoder = enc_helper::get_best_working_encoder(
        &available_encoders,
        &wanted_codec,
        &wanted_type,
    ).map_err(|e| anyhow!("Encoder selection failed: {}", e))?;

    tracing::info!("Using encoder: {} ({})", encoder.name, encoder.encoder_api.to_str());

    // ── Config ───────────────────────────────────────────────────────────────
    let room        = args.room.as_deref().unwrap_or("nestri-windows");
    let framerate   = args.framerate.unwrap_or(60);
    let bitrate     = args.bitrate_kbps.unwrap_or(8000);
    let monitor     = args.monitor_index.unwrap_or(0);
    let stream_url  = format!("{}/play?room={}", WEB_CLIENT_URL, room);

    // ── Build GStreamer pipeline ─────────────────────────────────────────────
    // D3D11 screen capture → NVENC H.264 → RTP payload → WebRTC
    let pipeline_desc = format!(
        "d3d11screencapturesrc monitor-index={monitor} ! \
         video/x-raw(memory:D3D11Memory),framerate={fps}/1 ! \
         d3d11convert ! \
         {encoder} bitrate={bitrate} ! \
         h264parse ! \
         rtph264pay pt=96 config-interval=-1 ! \
         webrtcbin name=sendrecv bundle-policy=max-bundle \
           stun-server=stun://stun.l.google.com:19302",
        monitor = monitor,
        fps     = framerate,
        encoder = encoder.name,
        bitrate = bitrate,
    );

    tracing::info!("Pipeline: {}", pipeline_desc);

    let pipeline = gstreamer::parse::launch(&pipeline_desc)
        .context("Failed to create GStreamer pipeline — is GStreamer installed with all plugins?")?;

    let pipeline = pipeline
        .downcast::<gstreamer::Pipeline>()
        .map_err(|_| anyhow!("Element is not a Pipeline"))?;

    // ── Print stream info ────────────────────────────────────────────────────
    println!();
    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
    println!("  🎮 Nestri Streaming Server");
    println!("  GPU:      {}", gpu.device_name);
    println!("  Encoder:  {} ({})", encoder.name, encoder.encoder_api.to_str());
    println!("  FPS:      {} | Bitrate: {} kbps", framerate, bitrate);
    println!("  Room:     {}", room);
    println!("  URL:      {}", stream_url);
    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
    println!();
    println!("  Open the URL above on any device to connect.");
    println!("  Press Ctrl+C to stop streaming.");
    println!();

    // ── Start streaming ──────────────────────────────────────────────────────
    pipeline
        .set_state(gstreamer::State::Playing)
        .context("Failed to start GStreamer pipeline")?;

    tracing::info!("Streaming started.");

    // Wait for EOS or error on the bus
    let bus = pipeline.bus().context("Pipeline has no bus")?;
    for msg in bus.iter_timed(gstreamer::ClockTime::NONE) {
        use gstreamer::MessageView;
        match msg.view() {
            MessageView::Eos(..) => {
                tracing::info!("Stream ended.");
                break;
            }
            MessageView::Error(err) => {
                tracing::error!(
                    "Pipeline error from {:?}: {}",
                    err.src().map(|s| s.path_string()),
                    err.error(),
                );
                break;
            }
            _ => {}
        }
    }

    pipeline
        .set_state(gstreamer::State::Null)
        .context("Failed to stop pipeline")?;

    tracing::info!("Nestri server stopped.");
    Ok(())
}
