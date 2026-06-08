/// nestri-server — Windows streaming server (pure Windows port)
///
/// Linux → Windows substitution map:
///   /sys/class/drm GPU enum   → DXGI IDXGIFactory1::EnumAdapters1
///   waylanddisplaysrc         → d3d11screencapturesrc  (GStreamer Win plugin)
///   pulsesrc / pipewiresrc    → wasapisrc               (WASAPI loopback)
///   nvh264enc (VA-API path)   → nvh264enc (CUDA/NVENC Windows path)
///   amfh264enc fallback       → mfh264enc (Media Foundation)
///   vimputti / uinput         → Win32 SendInput + ViGEmBus
///   P2P relay                 → unchanged (libp2p is cross-platform)
///
/// Web client: https://xtreme-gaming.pages.dev
/// Stream URL: https://xtreme-gaming.pages.dev/play?room=<your-room-name>

use anyhow::{Context, Result};
use args::NestriArgs;
use clap::Parser;
use enc_helper::{EncoderType, VideoCodec};
use gpu::{GpuInfo, GpuVendor};

mod args;
mod enc_helper;
mod gpu;
mod input;
mod latency;
mod nestrisink;
mod p2p;
mod proto;

/// The public Cloudflare Pages URL for the Nestri web client
pub const WEB_CLIENT_URL: &str = "https://xtreme-gaming.pages.dev";

/// Default public relay used when no custom relay is specified
pub const DEFAULT_RELAY: &str =
    "/dnsaddr/relay.dathorse.com/p2p/12D3KooWPK4v5wKYNYx9oXWjqLM8Xix6nm13o91j1Feqq98fLBsw";

#[tokio::main]
async fn main() -> Result<()> {
    // Initialise structured logging
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::from_default_env()
                .add_directive("nestri=info".parse().unwrap()),
        )
        .init();

    let args = NestriArgs::parse();

    // ── GPU detection ────────────────────────────────────────────────────────
    tracing::info!("Gathering GPU information via DXGI..");
    let gpus = gpu::enumerate_gpus()?;
    if gpus.is_empty() {
        return Err("No GPUs found via DXGI".into());
    }

    let gpu = gpus
        .iter()
        .find(|g| g.index == args.gpu_index as usize)
        .unwrap_or(&gpus[0]);

    tracing::info!(
        "Using GPU [{}]: {} (VRAM: {} MB)",
        gpu.index,
        gpu.name,
        gpu.dedicated_vram_mb
    );

    // ── Encoder selection ─────────────────────────────────────────────────────
    let encoder = enc_helper::select_encoder(
        &args.encoding.video_codec,
        &args.encoding.encoder_type,
        gpu,
    );
    tracing::info!("Selected encoder: {}", encoder.gst_element_name);

    // ── Print stream URL ──────────────────────────────────────────────────────
    let room = std::env::var("NESTRI_ROOM")
        .unwrap_or_else(|_| args.room_name.clone().unwrap_or_else(|| "nestri-windows".to_string()));
    let stream_url = format!("{}/play?room={}", WEB_CLIENT_URL, room);

    tracing::info!("════════════════════════════════════════════════");
    tracing::info!("  Nestri Streaming Server — READY");
    tracing::info!("  Room:       {}", room);
    tracing::info!("  Stream URL: {}", stream_url);
    tracing::info!("  Open this link on any device to connect!");
    tracing::info!("════════════════════════════════════════════════");

    // Print to stdout as well for the launcher to capture and display
    println!("NESTRI_STREAM_URL={}", stream_url);
    println!("NESTRI_ROOM={}", room);

    // ── GStreamer pipeline ─────────────────────────────────────────────────────
    gstreamer::init().context("Failed to initialise GStreamer")?;

    let pipeline = build_pipeline(&args, gpu, &encoder, &room)?;

    pipeline
        .set_state(gstreamer::State::Playing)
        .context("Failed to start GStreamer pipeline")?;

    tracing::info!("Pipeline started — streaming to room '{}'", room);

    // ── Main loop (bus messages) ──────────────────────────────────────────────
    let bus = pipeline.bus().unwrap();
    for msg in bus.iter_timed(gstreamer::ClockTime::NONE) {
        use gstreamer::MessageView;
        match msg.view() {
            MessageView::Eos(..) => {
                tracing::info!("End of stream.");
                break;
            }
            MessageView::Error(err) => {
                tracing::error!(
                    "GStreamer error: {} ({:?})",
                    err.error(),
                    err.debug()
                );
                break;
            }
            MessageView::Warning(w) => {
                tracing::warn!("GStreamer warning: {}", w.error());
            }
            _ => {}
        }
    }

    pipeline
        .set_state(gstreamer::State::Null)
        .context("Failed to stop pipeline")?;

    Ok(())
}

/// Build the GStreamer pipeline:
///   d3d11screencapturesrc → [D3D11 video path] → video encoder → webrtcsink
///   wasapisrc → audioconvert → audiorate → opusenc → webrtcsink
fn build_pipeline(
    args:    &NestriArgs,
    gpu:     &GpuInfo,
    encoder: &enc_helper::EncoderInfo,
    room:    &str,
) -> Result<gstreamer::Pipeline> {
    let pipeline = gstreamer::Pipeline::new();

    // ── Audio source (WASAPI loopback — captures system audio output) ─────────
    let audio_src = gstreamer::ElementFactory::make("wasapisrc")
        .name("audio_src")
        .property("loopback", true)
        .build()
        .map_err(|_| "wasapisrc not found — ensure GStreamer is installed with wasapi plugin")?;

    let audio_convert = gstreamer::ElementFactory::make("audioconvert")
        .name("audio_convert")
        .build()?;

    let audio_rate = gstreamer::ElementFactory::make("audiorate")
        .name("audio_rate")
        .build()?;

    let opus_enc = gstreamer::ElementFactory::make("opusenc")
        .name("opus_enc")
        .property("bitrate", args.encoding.audio_bitrate * 1000_i32)
        .property("audio-type", 2051_i32) // restricted-lowdelay
        .build()
        .map_err(|_| "opusenc not found — ensure GStreamer is installed with opus plugin")?;

    // ── Video source (D3D11 screen capture) ──────────────────────────────────
    // d3d11screencapturesrc — GStreamer Windows D3D11 screen capture
    let video_src = gstreamer::ElementFactory::make("d3d11screencapturesrc")
        .name("video_src")
        .property("adapter", gpu.index as u32)
        .property("monitor-index", args.monitor_index as i32)
        .property("show-cursor", true)
        .build()
        .map_err(|_| "d3d11screencapturesrc not found — install GStreamer with d3d11 plugin")?;

    // Framerate cap
    let caps_filter = gstreamer::ElementFactory::make("capsfilter")
        .name("caps_filter")
        .build()?;
    let caps = gstreamer::Caps::builder("video/x-raw")
        .field("framerate", gstreamer::Fraction::new(args.encoding.framerate, 1))
        .build();
    caps_filter.set_property("caps", &caps);

    // ── Video encoder (NVENC / MF / x264) ────────────────────────────────────
    let video_enc_builder = gstreamer::ElementFactory::make(&encoder.gst_element_name)
        .name("video_enc");

    // Apply encoder-specific properties
    let video_enc = match encoder.gst_element_name.as_str() {
        "nvh264enc" => {
            // NVENC: CBR, zero-latency, max quality
            video_enc_builder
                .property("bitrate", args.encoding.bitrate_kbps as u32)
                .property("rc-mode", 2_i32)      // cbr
                .property("zerolatency", true)
                .property("preset", 5_i32)        // low-latency-hp
                .property("adapter", gpu.index as u32)
                .build()?
        }
        "mfh264enc" => {
            video_enc_builder
                .property("bitrate", args.encoding.bitrate_kbps as u32)
                .property("rc-mode", 2_i32) // cbr
                .build()?
        }
        "x264enc" => {
            video_enc_builder
                .property("bitrate", args.encoding.bitrate_kbps as u32)
                .property("tune", 4_u32) // zerolatency
                .property("speed-preset", 6_u32) // faster
                .build()?
        }
        _ => video_enc_builder.build()?,
    };

    // RTP packetisation
    let rtp_pay = gstreamer::ElementFactory::make("rtph264pay")
        .name("rtp_pay")
        .property("config-interval", -1_i32) // send SPS/PPS with every keyframe
        .build()?;

    // ── WebRTC sink (nestrisink handles signalling with relay) ────────────────
    let relay_url = std::env::var("NESTRI_RELAY_URL")
        .unwrap_or_else(|_| DEFAULT_RELAY.to_string());

    let webrtc_sink = nestrisink::NestriSink::new(room, &relay_url)?;

    // ── Wire up pipeline ──────────────────────────────────────────────────────
    pipeline.add_many([
        &audio_src, &audio_convert, &audio_rate, &opus_enc,
        &video_src, &caps_filter, &video_enc, &rtp_pay,
        webrtc_sink.element(),
    ])?;

    gstreamer::Element::link_many([&audio_src, &audio_convert, &audio_rate, &opus_enc])?;
    gstreamer::Element::link_many([&video_src, &caps_filter, &video_enc, &rtp_pay])?;

    // Link audio + video into webrtcsink
    webrtc_sink.link_audio(&opus_enc)?;
    webrtc_sink.link_video(&rtp_pay)?;

    Ok(pipeline)
}
