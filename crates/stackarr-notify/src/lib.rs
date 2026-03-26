use anyhow::Result;
use serde::{Deserialize, Serialize};

// ── Events ──────────────────────────────────────────────────────────────────

/// Events that can trigger notifications.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", tag = "type")]
pub enum NotificationEvent {
    Grab {
        title: String,
        quality: String,
        indexer: String,
    },
    Import {
        title: String,
        quality: String,
    },
    Upgrade {
        title: String,
        old_quality: String,
        new_quality: String,
    },
    HealthIssue {
        source: String,
        message: String,
    },
    DownloadFailure {
        title: String,
        message: String,
    },
}

impl NotificationEvent {
    /// Short human-readable summary of the event.
    pub fn summary(&self) -> String {
        match self {
            Self::Grab { title, quality, .. } => format!("Grabbed: {title} [{quality}]"),
            Self::Import { title, quality } => format!("Imported: {title} [{quality}]"),
            Self::Upgrade {
                title,
                new_quality,
                ..
            } => format!("Upgraded: {title} [{new_quality}]"),
            Self::HealthIssue { source, message } => format!("Health: {source} - {message}"),
            Self::DownloadFailure { title, message } => {
                format!("Failed: {title} - {message}")
            }
        }
    }
}

// ── Provider trait ──────────────────────────────────────────────────────────

#[async_trait::async_trait]
pub trait NotificationProvider: Send + Sync {
    /// Human-readable name for this provider type.
    fn name(&self) -> &str;

    /// Send a notification event.
    async fn send(&self, event: &NotificationEvent) -> Result<()>;

    /// Test the connection / configuration.
    async fn test(&self) -> Result<()>;
}

// ── Webhook provider ────────────────────────────────────────────────────────

pub struct WebhookProvider {
    client: reqwest::Client,
    url: String,
}

impl WebhookProvider {
    pub fn new(url: String) -> Self {
        Self {
            client: reqwest::Client::new(),
            url,
        }
    }
}

#[async_trait::async_trait]
impl NotificationProvider for WebhookProvider {
    fn name(&self) -> &str {
        "Webhook"
    }

    async fn send(&self, event: &NotificationEvent) -> Result<()> {
        tracing::debug!(url = %self.url, "sending webhook notification");
        self.client
            .post(&self.url)
            .json(event)
            .send()
            .await?
            .error_for_status()?;
        Ok(())
    }

    async fn test(&self) -> Result<()> {
        let test_event = NotificationEvent::HealthIssue {
            source: "test".to_string(),
            message: "This is a test notification from StackArr".to_string(),
        };
        self.send(&test_event).await
    }
}

// ── Discord provider ────────────────────────────────────────────────────────

pub struct DiscordProvider {
    client: reqwest::Client,
    webhook_url: String,
}

impl DiscordProvider {
    pub fn new(webhook_url: String) -> Self {
        Self {
            client: reqwest::Client::new(),
            webhook_url,
        }
    }
}

#[async_trait::async_trait]
impl NotificationProvider for DiscordProvider {
    fn name(&self) -> &str {
        "Discord"
    }

    async fn send(&self, event: &NotificationEvent) -> Result<()> {
        tracing::debug!("sending discord notification");
        let body = serde_json::json!({
            "content": event.summary(),
        });
        self.client
            .post(&self.webhook_url)
            .json(&body)
            .send()
            .await?
            .error_for_status()?;
        Ok(())
    }

    async fn test(&self) -> Result<()> {
        let test_event = NotificationEvent::HealthIssue {
            source: "test".to_string(),
            message: "This is a test notification from StackArr".to_string(),
        };
        self.send(&test_event).await
    }
}

// ── Notification service ────────────────────────────────────────────────────

/// Dispatches events to all configured providers.
pub struct NotificationService {
    providers: Vec<Box<dyn NotificationProvider>>,
}

impl NotificationService {
    pub fn new() -> Self {
        Self {
            providers: Vec::new(),
        }
    }

    pub fn add_provider(&mut self, provider: Box<dyn NotificationProvider>) {
        self.providers.push(provider);
    }

    /// Send an event to all providers, logging errors but not failing.
    pub async fn notify(&self, event: &NotificationEvent) {
        for provider in &self.providers {
            if let Err(e) = provider.send(event).await {
                tracing::error!(
                    provider = provider.name(),
                    error = %e,
                    "notification send failed"
                );
            }
        }
    }
}

impl Default for NotificationService {
    fn default() -> Self {
        Self::new()
    }
}
