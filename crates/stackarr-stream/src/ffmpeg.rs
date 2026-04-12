use std::path::{Path, PathBuf};
use std::process::Stdio;

use tokio::process::{Child, Command};

use stackarr_core::config::{HwAccelConfig, QualityTierConfig, StreamingConfig};

use crate::error::{StreamError, StreamResult};

/// Configuration for a single transcode job.
pub struct TranscodeConfig<'a> {
    pub source_path: &'a Path,
    pub output_dir: &'a Path,
    pub video_stream_index: usize,
    pub audio_stream_index: usize,
    pub subtitle_stream_index: Option<usize>,
    pub max_width: Option<u32>,
    pub max_height: Option<u32>,
    pub video_bitrate: Option<u64>,
    pub streaming_config: &'a StreamingConfig,
}

/// A running transcode process.
pub struct TranscodeJob {
    pub child: Child,
    pub output_dir: PathBuf,
    pub playlist_path: PathBuf,
}

impl TranscodeJob {
    /// Kill the transcode process.
    pub async fn kill(&mut self) {
        if let Err(e) = self.child.kill().await {
            tracing::warn!(error = %e, "failed to kill ffmpeg process");
        }
    }

    /// Check if the process has exited.
    pub fn try_wait(&mut self) -> Option<std::process::ExitStatus> {
        self.child.try_wait().ok().flatten()
    }

    /// Read stderr from the process (for error diagnostics after exit).
    pub async fn take_stderr(&mut self) -> String {
        use tokio::io::AsyncReadExt;
        if let Some(mut stderr) = self.child.stderr.take() {
            let mut buf = Vec::new();
            let _ = stderr.read_to_end(&mut buf).await;
            let s = String::from_utf8_lossy(&buf);
            // Return last 500 chars (most relevant error info)
            if s.len() > 500 {
                s[s.len() - 500..].to_string()
            } else {
                s.to_string()
            }
        } else {
            String::new()
        }
    }
}

/// Start a transcoding job that outputs HLS segments.
pub async fn start_transcode(config: &TranscodeConfig<'_>) -> StreamResult<TranscodeJob> {
    let playlist_path = config.output_dir.join("master.m3u8");
    let segment_pattern = config.output_dir.join("%04d.ts");

    let mut cmd = Command::new(&config.streaming_config.ffmpeg_path);

    // Hardware acceleration input flags
    let hwaccel = &config.streaming_config.hwaccel;
    add_hwaccel_input_flags(&mut cmd, hwaccel);

    // Input file
    cmd.arg("-i").arg(config.source_path);

    // Stream selection
    cmd.arg("-map")
        .arg(format!("0:v:{}", config.video_stream_index));
    cmd.arg("-map")
        .arg(format!("0:a:{}", config.audio_stream_index));

    // Video encoding
    add_video_encode_flags(&mut cmd, hwaccel, config);

    // Audio encoding — always transcode to AAC for browser compatibility
    cmd.args(["-c:a", "aac", "-b:a", "192k", "-ac", "2"]);

    // Subtitle burn-in (if requested and using software encoding or hw download)
    if let Some(sub_idx) = config.subtitle_stream_index {
        if hwaccel.enabled {
            // For QSV/VAAPI: download → burn subs → re-upload
            // This is handled in the video filter chain in add_video_encode_flags
            tracing::debug!(
                sub_idx,
                "subtitle burn-in with hw accel handled in vf chain"
            );
        } else {
            // Escape single quotes and backslashes for ffmpeg filter syntax
            let escaped_path = config
                .source_path
                .display()
                .to_string()
                .replace('\\', r"\\")
                .replace('\'', r"'\''");
            cmd.arg("-vf")
                .arg(format!("subtitles='{escaped_path}':si={sub_idx}"));
        }
    }

    // Force keyframes at regular intervals so HLS segments are predictable.
    // Without this, libx264 defaults to keyframes every ~250 frames (~10s at 24fps),
    // making segments much longer than the target and delaying first-segment readiness.
    let seg_secs = config.streaming_config.segment_duration_secs;
    cmd.arg("-force_key_frames")
        .arg(format!("expr:gte(t,n_forced*{seg_secs})"));

    // HLS output
    cmd.args(["-f", "hls"]);
    cmd.arg("-hls_time").arg(seg_secs.to_string());
    cmd.args(["-hls_list_size", "0"]);
    // EVENT type tells players this is a growing VOD (play from start),
    // not a live stream (sync to live edge). Without this, HLS.js enters
    // live mode and may stall on a slow transcode that trickles segments.
    cmd.args(["-hls_playlist_type", "event"]);
    cmd.args(["-hls_flags", "independent_segments"]);
    cmd.arg("-hls_segment_filename").arg(&segment_pattern);

    // Start from the beginning, allow segment generation
    cmd.args(["-start_number", "0"]);

    cmd.arg(&playlist_path);

    // Suppress interactive prompts, overwrite output
    cmd.arg("-y");
    cmd.args(["-nostdin"]);

    cmd.stdout(Stdio::null());
    cmd.stderr(Stdio::piped());

    tracing::info!(
        source = %config.source_path.display(),
        output = %config.output_dir.display(),
        hwaccel = hwaccel.enabled,
        accel_type = %hwaccel.accel_type,
        "starting transcode"
    );

    let child = cmd
        .spawn()
        .map_err(|e| StreamError::Transcode(format!("failed to spawn ffmpeg: {e}")))?;

    Ok(TranscodeJob {
        child,
        output_dir: config.output_dir.to_path_buf(),
        playlist_path,
    })
}

fn add_hwaccel_input_flags(cmd: &mut Command, hwaccel: &HwAccelConfig) {
    if !hwaccel.enabled {
        return;
    }

    let device = hwaccel.device.as_deref().unwrap_or("/dev/dri/renderD128");

    match hwaccel.accel_type.as_str() {
        "qsv" => {
            cmd.arg("-init_hw_device")
                .arg(format!("qsv=hw,child_device={device}"));
            cmd.args(["-filter_hw_device", "hw"]);
            cmd.args(["-hwaccel", "qsv"]);
            cmd.args(["-hwaccel_output_format", "qsv"]);
        }
        "vaapi" => {
            cmd.arg("-vaapi_device").arg(device);
            cmd.args(["-hwaccel", "vaapi"]);
            cmd.args(["-hwaccel_output_format", "vaapi"]);
        }
        "nvenc" => {
            cmd.args(["-hwaccel", "cuda"]);
            cmd.args(["-hwaccel_output_format", "cuda"]);
        }
        _ => {}
    }
}

fn add_video_encode_flags(
    cmd: &mut Command,
    hwaccel: &HwAccelConfig,
    config: &TranscodeConfig<'_>,
) {
    // Build scale filters for both software and hardware pipelines
    let scale_filter_sw = match (config.max_width, config.max_height) {
        (Some(w), Some(h)) => Some(format!(
            "scale='min({w},iw)':'min({h},ih)':force_original_aspect_ratio=decrease"
        )),
        (Some(w), None) => Some(format!("scale='min({w},iw)':-2")),
        (None, Some(h)) => Some(format!("scale=-2:'min({h},ih)'")),
        (None, None) => None,
    };
    let scale_filter_vaapi = match (config.max_width, config.max_height) {
        (Some(w), Some(h)) => Some(format!(
            "scale_vaapi=w='min({w},iw)':h='min({h},ih)':force_original_aspect_ratio=decrease"
        )),
        (Some(w), None) => Some(format!("scale_vaapi=w='min({w},iw)':h=-2")),
        (None, Some(h)) => Some(format!("scale_vaapi=w=-2:h='min({h},ih)'")),
        (None, None) => None,
    };

    if hwaccel.enabled {
        match hwaccel.accel_type.as_str() {
            "qsv" => {
                cmd.args(["-c:v", "h264_qsv"]);
                cmd.args(["-preset", "medium"]);
                if let Some(bitrate) = config.video_bitrate {
                    cmd.arg("-b:v").arg(format!("{bitrate}"));
                    cmd.args(["-maxrate", &format!("{}", bitrate * 12 / 10)]);
                    cmd.args(["-bufsize", &format!("{}", bitrate * 2)]);
                } else {
                    cmd.args(["-global_quality", "23"]);
                }
                if let Some(sf) = &scale_filter_sw {
                    // QSV scale: need to download from GPU, scale, re-upload
                    cmd.arg("-vf").arg(format!(
                        "hwdownload,format=nv12,{sf},hwupload=extra_hw_frames=64"
                    ));
                }
            }
            "vaapi" => {
                cmd.args(["-c:v", "h264_vaapi"]);
                if let Some(bitrate) = config.video_bitrate {
                    cmd.arg("-b:v").arg(format!("{bitrate}"));
                    cmd.args(["-maxrate", &format!("{}", bitrate * 12 / 10)]);
                    cmd.args(["-bufsize", &format!("{}", bitrate * 2)]);
                } else {
                    cmd.args(["-qp", "23"]);
                }
                // tonemap_vaapi handles HDR→SDR tone mapping on the GPU;
                // for SDR input it acts as a passthrough format conversion
                // scale_vaapi runs entirely on GPU (no hwdownload needed)
                if let Some(sf) = &scale_filter_vaapi {
                    cmd.arg("-vf").arg(format!(
                        "tonemap_vaapi=format=nv12:t=bt709:m=bt709:p=bt709,{sf}"
                    ));
                } else {
                    cmd.arg("-vf")
                        .arg("tonemap_vaapi=format=nv12:t=bt709:m=bt709:p=bt709");
                }
            }
            "nvenc" => {
                cmd.args(["-c:v", "h264_nvenc"]);
                cmd.args(["-preset", "p4"]);
                if let Some(bitrate) = config.video_bitrate {
                    cmd.arg("-b:v").arg(format!("{bitrate}"));
                } else {
                    cmd.args(["-cq", "23"]);
                }
                if let Some(sf) = &scale_filter_sw {
                    cmd.arg("-vf").arg(sf);
                }
            }
            _ => {
                add_software_encode_flags(cmd, config, &scale_filter_sw);
            }
        }
    } else {
        add_software_encode_flags(cmd, config, &scale_filter_sw);
    }
}

// ── Multi-rendition transcode ────────────────────────────────────────────

/// A running multi-rendition transcode (multiple ffmpeg processes).
pub struct MultiRenditionJob {
    pub jobs: Vec<TranscodeJob>,
    pub tiers: Vec<QualityTierConfig>,
    pub output_dir: PathBuf,
    pub master_playlist_path: PathBuf,
}

impl MultiRenditionJob {
    /// Kill all ffmpeg processes.
    pub async fn kill_all(&mut self) {
        for job in &mut self.jobs {
            job.kill().await;
        }
    }
}

/// Start multiple ffmpeg processes for adaptive bitrate streaming.
/// Each tier gets its own ffmpeg process writing to `{output_dir}/v{n}/`.
/// All processes use aligned keyframes for seamless switching.
pub async fn start_multi_rendition_transcode(
    source_path: &Path,
    output_dir: &Path,
    video_stream_index: usize,
    audio_stream_index: usize,
    subtitle_stream_index: Option<usize>,
    tiers: &[QualityTierConfig],
    streaming_config: &StreamingConfig,
) -> StreamResult<MultiRenditionJob> {
    let mut jobs = Vec::new();

    for (i, tier) in tiers.iter().enumerate() {
        let tier_dir = output_dir.join(format!("v{i}"));
        tokio::fs::create_dir_all(&tier_dir).await?;

        let playlist_path = tier_dir.join("stream.m3u8");
        let segment_pattern = tier_dir.join("%04d.ts");

        let mut cmd = Command::new(&streaming_config.ffmpeg_path);

        // Hardware acceleration
        let hwaccel = &streaming_config.hwaccel;
        add_hwaccel_input_flags(&mut cmd, hwaccel);

        // Input
        cmd.arg("-i").arg(source_path);

        // Stream selection
        cmd.arg("-map").arg(format!("0:v:{video_stream_index}"));
        cmd.arg("-map").arg(format!("0:a:{audio_stream_index}"));

        // Build a TranscodeConfig for this tier to reuse encode flag logic
        let tier_config = TranscodeConfig {
            source_path,
            output_dir: &tier_dir,
            video_stream_index,
            audio_stream_index,
            subtitle_stream_index,
            max_width: Some(tier.max_width),
            max_height: Some(tier.max_height),
            video_bitrate: Some(tier.video_bitrate),
            streaming_config,
        };

        // Video encoding (reuse existing logic)
        add_video_encode_flags(&mut cmd, hwaccel, &tier_config);

        // Audio encoding
        let audio_br = format!("{}k", tier.audio_bitrate / 1000);
        cmd.args(["-c:a", "aac", "-b:a", &audio_br, "-ac", "2"]);

        // Subtitle burn-in
        if let Some(sub_idx) = subtitle_stream_index
            && !hwaccel.enabled
        {
            cmd.arg("-vf").arg(format!(
                "subtitles='{}':si={sub_idx}",
                source_path.display()
            ));
        }

        // Force aligned keyframes across all renditions (critical for ABR switching)
        cmd.args(["-g", "48", "-keyint_min", "48"]);
        cmd.arg("-force_key_frames").arg("expr:gte(t,n_forced*2)");

        // HLS output
        cmd.args(["-f", "hls"]);
        cmd.arg("-hls_time")
            .arg(streaming_config.segment_duration_secs.to_string());
        cmd.args(["-hls_list_size", "0"]);
        cmd.args(["-hls_playlist_type", "event"]);
        cmd.args(["-hls_flags", "independent_segments"]);
        cmd.arg("-hls_segment_filename").arg(&segment_pattern);
        cmd.args(["-start_number", "0"]);
        cmd.arg(&playlist_path);
        cmd.arg("-y");
        cmd.args(["-nostdin"]);

        cmd.stdout(Stdio::null());
        cmd.stderr(Stdio::piped());

        tracing::info!(
            tier = %tier.name,
            resolution = %format!("{}x{}", tier.max_width, tier.max_height),
            bitrate = tier.video_bitrate,
            rendition = i,
            "starting rendition transcode"
        );

        let child = cmd.spawn().map_err(|e| {
            StreamError::Transcode(format!("failed to spawn ffmpeg for {}: {e}", tier.name))
        })?;

        jobs.push(TranscodeJob {
            child,
            output_dir: tier_dir,
            playlist_path,
        });
    }

    // Generate master playlist
    let master_path = output_dir.join("master.m3u8");
    let master_content = generate_master_playlist_content(tiers);
    tokio::fs::write(&master_path, &master_content).await?;

    tracing::info!(renditions = jobs.len(), "multi-rendition transcode started");

    Ok(MultiRenditionJob {
        jobs,
        tiers: tiers.to_vec(),
        output_dir: output_dir.to_path_buf(),
        master_playlist_path: master_path,
    })
}

/// Generate HLS multi-variant master playlist content.
fn generate_master_playlist_content(tiers: &[QualityTierConfig]) -> String {
    let mut lines = vec!["#EXTM3U".to_string(), "#EXT-X-VERSION:6".to_string()];

    for (i, tier) in tiers.iter().enumerate() {
        let total_bitrate = tier.video_bitrate + tier.audio_bitrate;
        lines.push(format!(
            "#EXT-X-STREAM-INF:BANDWIDTH={total_bitrate},RESOLUTION={}x{},NAME=\"{}\"",
            tier.max_width, tier.max_height, tier.name
        ));
        lines.push(format!("v{i}/stream.m3u8"));
    }

    lines.join("\n") + "\n"
}

fn add_software_encode_flags(
    cmd: &mut Command,
    config: &TranscodeConfig<'_>,
    scale_filter: &Option<String>,
) {
    cmd.args(["-c:v", "libx264"]);
    cmd.args(["-preset", "veryfast"]);
    if let Some(bitrate) = config.video_bitrate {
        cmd.arg("-b:v").arg(format!("{bitrate}"));
        cmd.args(["-maxrate", &format!("{}", bitrate * 12 / 10)]);
        cmd.args(["-bufsize", &format!("{}", bitrate * 2)]);
    } else {
        cmd.args(["-crf", "23"]);
    }
    // Force level 4.1 and yuv420p for maximum browser compatibility
    cmd.args(["-level", "4.1"]);
    cmd.args(["-pix_fmt", "yuv420p"]);

    if let Some(sf) = scale_filter {
        cmd.arg("-vf").arg(sf);
    }
}
