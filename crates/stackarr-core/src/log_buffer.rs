// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 The StackArr Authors

//! In-memory ring buffer for application log entries.
//!
//! A custom `tracing::Layer` captures log events into a bounded buffer.
//! The HTTP API can then serve these entries with optional filtering.

use std::collections::VecDeque;
use std::sync::Arc;

use chrono::{DateTime, Utc};
use parking_lot::RwLock;
use serde::{Deserialize, Serialize};
use tracing::field::{Field, Visit};
use tracing_subscriber::Layer;
use tracing_subscriber::layer::Context;

/// Maximum number of log entries kept in the ring buffer.
const MAX_LOG_ENTRIES: usize = 20_000;

/// A single captured log entry.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LogEntry {
    pub timestamp: DateTime<Utc>,
    pub level: String,
    pub target: String,
    pub message: String,
    /// Monotonic sequence number for pagination / polling.
    pub seq: u64,
}

/// Thread-safe ring buffer of log entries.
#[derive(Clone)]
pub struct LogBuffer {
    inner: Arc<RwLock<LogBufferInner>>,
}

struct LogBufferInner {
    entries: VecDeque<LogEntry>,
    next_seq: u64,
}

impl Default for LogBuffer {
    fn default() -> Self {
        Self::new()
    }
}

impl LogBuffer {
    pub fn new() -> Self {
        Self {
            inner: Arc::new(RwLock::new(LogBufferInner {
                entries: VecDeque::with_capacity(MAX_LOG_ENTRIES),
                next_seq: 0,
            })),
        }
    }

    /// Push a log entry into the buffer.
    fn push(&self, mut entry: LogEntry) {
        let mut inner = self.inner.write();
        entry.seq = inner.next_seq;
        inner.next_seq += 1;

        if inner.entries.len() >= MAX_LOG_ENTRIES {
            inner.entries.pop_front();
        }
        inner.entries.push_back(entry);
    }

    /// Get entries with optional filters, after a given sequence number.
    pub fn get_entries(
        &self,
        after_seq: Option<u64>,
        level: Option<&str>,
        target: Option<&str>,
        limit: usize,
    ) -> Vec<LogEntry> {
        let inner = self.inner.read();
        inner
            .entries
            .iter()
            .filter(|e| {
                if let Some(after) = after_seq
                    && e.seq <= after
                {
                    return false;
                }
                if let Some(lvl) = level
                    && !e.level.eq_ignore_ascii_case(lvl)
                {
                    return false;
                }
                if let Some(t) = target
                    && !e.target.starts_with(t)
                {
                    return false;
                }
                true
            })
            .rev()
            .take(limit)
            .cloned()
            .collect::<Vec<_>>()
            .into_iter()
            .rev()
            .collect()
    }

    /// Get the latest sequence number.
    pub fn latest_seq(&self) -> u64 {
        let inner = self.inner.read();
        if inner.next_seq > 0 {
            inner.next_seq - 1
        } else {
            0
        }
    }
}

/// A visitor that extracts fields from tracing events into strings.
struct FieldVisitor {
    message: String,
}

impl Visit for FieldVisitor {
    fn record_debug(&mut self, field: &Field, value: &dyn std::fmt::Debug) {
        if field.name() == "message" {
            self.message = format!("{value:?}");
        } else {
            if !self.message.is_empty() {
                self.message.push(' ');
            }
            self.message
                .push_str(&format!("{}={:?}", field.name(), value));
        }
    }

    fn record_str(&mut self, field: &Field, value: &str) {
        if field.name() == "message" {
            self.message = value.to_string();
        } else {
            if !self.message.is_empty() {
                self.message.push(' ');
            }
            self.message
                .push_str(&format!("{}={}", field.name(), value));
        }
    }
}

/// Tracing layer that captures events into a `LogBuffer`.
pub struct LogBufferLayer {
    buffer: LogBuffer,
}

impl LogBufferLayer {
    pub fn new(buffer: LogBuffer) -> Self {
        Self { buffer }
    }
}

impl<S> Layer<S> for LogBufferLayer
where
    S: tracing::Subscriber + for<'a> tracing_subscriber::registry::LookupSpan<'a>,
{
    fn on_event(&self, event: &tracing::Event<'_>, _ctx: Context<'_, S>) {
        let metadata = event.metadata();

        let mut visitor = FieldVisitor {
            message: String::new(),
        };
        event.record(&mut visitor);

        let entry = LogEntry {
            timestamp: Utc::now(),
            level: metadata.level().to_string(),
            target: metadata.target().to_string(),
            message: visitor.message,
            seq: 0, // set by push()
        };

        self.buffer.push(entry);
    }
}
