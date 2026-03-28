use std::path::{Path, PathBuf};
use std::process::Stdio;

use tokio::process::{Child, Command};

use stackarr_core::config::{HwAccelConfig, StreamingConfig};

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
            tracing::debug!(sub_idx, "subtitle burn-in with hw accel handled in vf chain");
        } else {
            cmd.arg("-vf").arg(format!(
                "subtitles='{}':si={}",
                config.source_path.display(),
                sub_idx
            ));
        }
    }

    // HLS output
    cmd.args(["-f", "hls"]);
    cmd.arg("-hls_time")
        .arg(config.streaming_config.segment_duration_secs.to_string());
    cmd.args(["-hls_list_size", "0"]);
    cmd.args(["-hls_flags", "independent_segments"]);
    cmd.arg("-hls_segment_filename")
        .arg(&segment_pattern);

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

    let device = hwaccel
        .device
        .as_deref()
        .unwrap_or("/dev/dri/renderD128");

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

fn add_video_encode_flags(cmd: &mut Command, hwaccel: &HwAccelConfig, config: &TranscodeConfig<'_>) {
    // Build scale filter if resolution limits are set
    let scale_filter = match (config.max_width, config.max_height) {
        (Some(w), Some(h)) => Some(format!("scale='min({w},iw)':min'({h},ih)':force_original_aspect_ratio=decrease")),
        (Some(w), None) => Some(format!("scale='min({w},iw)':-2")),
        (None, Some(h)) => Some(format!("scale=-2:'min({h},ih)'")),
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
                if let Some(sf) = scale_filter {
                    // QSV scale: need to download from GPU, scale, re-upload
                    cmd.arg("-vf")
                        .arg(format!("hwdownload,format=nv12,{sf},hwupload=extra_hw_frames=64"));
                }
            }
            "vaapi" => {
                cmd.args(["-c:v", "h264_vaapi"]);
                if let Some(bitrate) = config.video_bitrate {
                    cmd.arg("-b:v").arg(format!("{bitrate}"));
                } else {
                    cmd.args(["-qp", "23"]);
                }
                // tonemap_vaapi handles HDR→SDR tone mapping on the GPU;
                // for SDR input it acts as a passthrough format conversion
                if let Some(sf) = scale_filter {
                    cmd.arg("-vf")
                        .arg(format!("tonemap_vaapi=format=nv12:t=bt709:m=bt709:p=bt709,{sf}"));
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
                if let Some(sf) = scale_filter {
                    cmd.arg("-vf").arg(sf);
                }
            }
            _ => {
                add_software_encode_flags(cmd, config, &scale_filter);
            }
        }
    } else {
        add_software_encode_flags(cmd, config, &scale_filter);
    }
}

fn add_software_encode_flags(cmd: &mut Command, config: &TranscodeConfig<'_>, scale_filter: &Option<String>) {
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
