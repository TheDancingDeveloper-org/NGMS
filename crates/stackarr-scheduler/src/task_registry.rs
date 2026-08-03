// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 The StackArr Authors

use std::sync::Arc;

use chrono::{DateTime, Utc};
use dashmap::DashMap;
use serde::Serialize;
use tokio::sync::Notify;

/// Information about a single scheduled task.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TaskInfo {
    pub name: String,
    pub interval_secs: u64,
    pub last_run: Option<DateTime<Utc>>,
    pub last_status: Option<String>,
    pub last_message: Option<String>,
    pub last_duration_ms: Option<u64>,
    pub next_run: Option<DateTime<Utc>>,
    pub running: bool,
}

/// Thread-safe registry tracking all scheduled task states.
pub struct TaskRegistry {
    tasks: DashMap<String, TaskInfo>,
    triggers: DashMap<String, Arc<Notify>>,
}

impl TaskRegistry {
    pub fn new() -> Self {
        Self {
            tasks: DashMap::new(),
            triggers: DashMap::new(),
        }
    }

    /// Register a task with its interval. Called once during scheduler setup.
    pub fn register(&self, name: &str, interval_secs: u64) {
        let now = Utc::now();
        self.tasks.insert(
            name.to_string(),
            TaskInfo {
                name: name.to_string(),
                interval_secs,
                last_run: None,
                last_status: None,
                last_message: None,
                last_duration_ms: None,
                next_run: Some(now + chrono::Duration::seconds(interval_secs as i64)),
                running: false,
            },
        );
        self.triggers
            .insert(name.to_string(), Arc::new(Notify::new()));
    }

    /// Mark a task as currently running.
    pub fn mark_running(&self, name: &str) {
        if let Some(mut info) = self.tasks.get_mut(name) {
            info.running = true;
            info.last_run = Some(Utc::now());
        }
    }

    /// Mark a task as completed with a result.
    pub fn mark_completed(
        &self,
        name: &str,
        success: bool,
        message: Option<String>,
        duration_ms: u64,
    ) {
        if let Some(mut info) = self.tasks.get_mut(name) {
            info.running = false;
            info.last_status = Some(if success { "success" } else { "failed" }.to_string());
            info.last_message = message;
            info.last_duration_ms = Some(duration_ms);
            info.next_run = Some(Utc::now() + chrono::Duration::seconds(info.interval_secs as i64));
        }
    }

    /// Get a snapshot of all registered tasks.
    pub fn list_tasks(&self) -> Vec<TaskInfo> {
        self.tasks.iter().map(|r| r.value().clone()).collect()
    }

    /// Get the Notify handle for a task, used by manual trigger.
    pub fn trigger_handle(&self, name: &str) -> Option<Arc<Notify>> {
        self.triggers.get(name).map(|r| Arc::clone(r.value()))
    }

    /// Manually trigger a task by name. Returns true if the task exists.
    pub fn trigger(&self, name: &str) -> bool {
        if let Some(notify) = self.triggers.get(name) {
            notify.notify_one();
            true
        } else {
            false
        }
    }
}

impl Default for TaskRegistry {
    fn default() -> Self {
        Self::new()
    }
}
