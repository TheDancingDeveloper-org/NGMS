// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 The StackArr Authors

pub mod direct;
pub mod error;
pub mod ffmpeg;
pub mod ffprobe;
pub mod hls;
pub mod provision;
pub mod session;
pub mod subtitle;
pub mod types;

pub use error::{StreamError, StreamResult};
pub use provision::{FfmpegPaths, ensure_ffmpeg};
pub use session::{DetectedAccel, SessionManager, probe_hwaccel};
pub use types::*;
