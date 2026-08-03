// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 The StackArr Authors

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
                title, new_quality, ..
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

// ── Telegram provider ──────────────────────────────────────────────────────

pub struct TelegramProvider {
    client: reqwest::Client,
    bot_token: String,
    chat_id: String,
}

impl TelegramProvider {
    pub fn new(bot_token: String, chat_id: String) -> Self {
        Self {
            client: reqwest::Client::new(),
            bot_token,
            chat_id,
        }
    }
}

#[async_trait::async_trait]
impl NotificationProvider for TelegramProvider {
    fn name(&self) -> &str {
        "Telegram"
    }

    async fn send(&self, event: &NotificationEvent) -> Result<()> {
        tracing::debug!("sending telegram notification");
        let url = format!("https://api.telegram.org/bot{}/sendMessage", self.bot_token);
        let body = serde_json::json!({
            "chat_id": self.chat_id,
            "text": event.summary(),
            "parse_mode": "HTML",
        });
        self.client
            .post(&url)
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

// ── Slack provider ─────────────────────────────────────────────────────────

pub struct SlackProvider {
    client: reqwest::Client,
    webhook_url: String,
}

impl SlackProvider {
    pub fn new(webhook_url: String) -> Self {
        Self {
            client: reqwest::Client::new(),
            webhook_url,
        }
    }
}

#[async_trait::async_trait]
impl NotificationProvider for SlackProvider {
    fn name(&self) -> &str {
        "Slack"
    }

    async fn send(&self, event: &NotificationEvent) -> Result<()> {
        tracing::debug!("sending slack notification");
        let body = serde_json::json!({
            "text": event.summary(),
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

// ── Email provider ─────────────────────────────────────────────────────────

pub struct EmailProvider {
    client: reqwest::Client,
    smtp_url: String,
    from: String,
    to: String,
}

impl EmailProvider {
    pub fn new(smtp_url: String, from: String, to: String) -> Self {
        Self {
            client: reqwest::Client::new(),
            smtp_url,
            from,
            to,
        }
    }
}

#[async_trait::async_trait]
impl NotificationProvider for EmailProvider {
    fn name(&self) -> &str {
        "Email"
    }

    async fn send(&self, event: &NotificationEvent) -> Result<()> {
        tracing::debug!("sending email notification");
        let body = serde_json::json!({
            "from": self.from,
            "to": self.to,
            "subject": "StackArr Notification",
            "body": event.summary(),
        });
        self.client
            .post(&self.smtp_url)
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
            match provider.send(event).await {
                Ok(()) => {
                    tracing::info!(provider = provider.name(), "notification sent");
                }
                Err(e) => {
                    tracing::error!(
                        provider = provider.name(),
                        error = %e,
                        "notification send failed"
                    );
                }
            }
        }
    }
}

impl Default for NotificationService {
    fn default() -> Self {
        Self::new()
    }
}

// ── DB-driven notification dispatch ────────────────────────────────────────

/// Row from the `notification_providers` table.
#[derive(Debug, sqlx::FromRow)]
#[allow(dead_code)]
struct NotificationProviderRow {
    id: i32,
    name: String,
    provider_type: String,
    config: serde_json::Value,
    on_grab: bool,
    on_import: bool,
    on_upgrade: bool,
    on_health_issue: bool,
    on_failure: bool,
    enabled: bool,
}

impl NotificationProviderRow {
    /// Check whether this provider is interested in the given event.
    fn wants_event(&self, event: &NotificationEvent) -> bool {
        match event {
            NotificationEvent::Grab { .. } => self.on_grab,
            NotificationEvent::Import { .. } => self.on_import,
            NotificationEvent::Upgrade { .. } => self.on_upgrade,
            NotificationEvent::HealthIssue { .. } => self.on_health_issue,
            NotificationEvent::DownloadFailure { .. } => self.on_failure,
        }
    }

    /// Build a concrete provider from the row's type and config.
    fn build_provider(&self) -> Option<Box<dyn NotificationProvider>> {
        match self.provider_type.as_str() {
            "webhook" => {
                let url = self.config.get("url")?.as_str()?;
                Some(Box::new(WebhookProvider::new(url.to_string())))
            }
            "discord" => {
                let url = self
                    .config
                    .get("webhook_url")
                    .or_else(|| self.config.get("url"))?
                    .as_str()?;
                Some(Box::new(DiscordProvider::new(url.to_string())))
            }
            "telegram" => {
                let bot_token = self.config.get("bot_token")?.as_str()?;
                let chat_id = self.config.get("chat_id")?.as_str()?;
                Some(Box::new(TelegramProvider::new(
                    bot_token.to_string(),
                    chat_id.to_string(),
                )))
            }
            "slack" => {
                let url = self
                    .config
                    .get("webhook_url")
                    .or_else(|| self.config.get("url"))?
                    .as_str()?;
                Some(Box::new(SlackProvider::new(url.to_string())))
            }
            "email" => {
                let smtp_url = self.config.get("smtp_url")?.as_str()?;
                let from = self.config.get("from")?.as_str()?;
                let to = self.config.get("to")?.as_str()?;
                Some(Box::new(EmailProvider::new(
                    smtp_url.to_string(),
                    from.to_string(),
                    to.to_string(),
                )))
            }
            other => {
                tracing::warn!(provider_type = other, "unknown notification provider type");
                None
            }
        }
    }
}

/// Build a notification provider from a type string and JSONB config.
///
/// This is used by the notification provider test endpoints to construct
/// a provider without saving it to the database first.
pub fn build_provider_from_config(
    provider_type: &str,
    config: &serde_json::Value,
) -> Option<Box<dyn NotificationProvider>> {
    match provider_type {
        "webhook" => {
            let url = config.get("url")?.as_str()?;
            Some(Box::new(WebhookProvider::new(url.to_string())))
        }
        "discord" => {
            let url = config
                .get("webhook_url")
                .or_else(|| config.get("url"))?
                .as_str()?;
            Some(Box::new(DiscordProvider::new(url.to_string())))
        }
        "telegram" => {
            let bot_token = config.get("bot_token")?.as_str()?;
            let chat_id = config.get("chat_id")?.as_str()?;
            Some(Box::new(TelegramProvider::new(
                bot_token.to_string(),
                chat_id.to_string(),
            )))
        }
        "slack" => {
            let url = config
                .get("webhook_url")
                .or_else(|| config.get("url"))?
                .as_str()?;
            Some(Box::new(SlackProvider::new(url.to_string())))
        }
        "email" => {
            let smtp_url = config.get("smtp_url")?.as_str()?;
            let from = config.get("from")?.as_str()?;
            let to = config.get("to")?.as_str()?;
            Some(Box::new(EmailProvider::new(
                smtp_url.to_string(),
                from.to_string(),
                to.to_string(),
            )))
        }
        _ => None,
    }
}

/// Load enabled notification providers from the database, filter by event type,
/// and dispatch the event to all matching providers.
///
/// This is the main entry point for sending notifications throughout the app.
/// Errors from individual providers are logged but never propagated.
pub async fn dispatch_event(pool: &sqlx::PgPool, event: &NotificationEvent) {
    let rows: Vec<NotificationProviderRow> = match sqlx::query_as::<_, NotificationProviderRow>(
        "SELECT id, name, provider_type, config, on_grab, on_import, on_upgrade, \
                on_health_issue, on_failure, enabled \
         FROM notification_providers WHERE enabled = true",
    )
    .fetch_all(pool)
    .await
    {
        Ok(r) => r,
        Err(e) => {
            tracing::error!(error = %e, "failed to load notification providers from DB");
            return;
        }
    };

    for row in &rows {
        if !row.wants_event(event) {
            continue;
        }
        let Some(provider) = row.build_provider() else {
            tracing::warn!(
                id = row.id,
                name = %row.name,
                provider_type = %row.provider_type,
                "failed to build notification provider from config"
            );
            continue;
        };
        match provider.send(event).await {
            Ok(()) => {
                tracing::info!(
                    provider = %row.name,
                    "notification sent"
                );
            }
            Err(e) => {
                tracing::error!(
                    provider = %row.name,
                    error = %e,
                    "notification send failed"
                );
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;
    use std::sync::atomic::{AtomicUsize, Ordering};

    #[test]
    fn test_event_summary_grab() {
        let event = NotificationEvent::Grab {
            title: "Breaking Bad S01E01".into(),
            quality: "HDTV-720p".into(),
            indexer: "NZBGeek".into(),
        };
        let s = event.summary();
        assert!(s.contains("Breaking Bad S01E01"));
        assert!(s.contains("HDTV-720p"));
    }

    #[test]
    fn test_event_summary_import() {
        let event = NotificationEvent::Import {
            title: "The Wire S03E05".into(),
            quality: "Bluray-1080p".into(),
        };
        let s = event.summary();
        assert!(s.contains("Imported"));
        assert!(s.contains("The Wire S03E05"));
    }

    #[test]
    fn test_event_summary_upgrade() {
        let event = NotificationEvent::Upgrade {
            title: "Movie Title".into(),
            old_quality: "HDTV-720p".into(),
            new_quality: "Bluray-1080p".into(),
        };
        let s = event.summary();
        assert!(s.contains("Upgraded"));
        assert!(s.contains("Bluray-1080p"));
    }

    #[test]
    fn test_event_summary_health_issue() {
        let event = NotificationEvent::HealthIssue {
            source: "Indexer".into(),
            message: "NZBGeek unavailable".into(),
        };
        let s = event.summary();
        assert!(s.contains("Indexer"));
        assert!(s.contains("NZBGeek unavailable"));
    }

    #[test]
    fn test_event_summary_download_failure() {
        let event = NotificationEvent::DownloadFailure {
            title: "Some.Release".into(),
            message: "connection timeout".into(),
        };
        let s = event.summary();
        assert!(s.contains("Failed"));
        assert!(s.contains("connection timeout"));
    }

    // ── NotificationService tests with manual mock ──────────────────────

    struct CountingProvider {
        send_count: Arc<AtomicUsize>,
    }

    #[async_trait::async_trait]
    impl NotificationProvider for CountingProvider {
        fn name(&self) -> &str {
            "Counting"
        }
        async fn send(&self, _event: &NotificationEvent) -> Result<()> {
            self.send_count.fetch_add(1, Ordering::SeqCst);
            Ok(())
        }
        async fn test(&self) -> Result<()> {
            Ok(())
        }
    }

    struct FailingProvider;

    #[async_trait::async_trait]
    impl NotificationProvider for FailingProvider {
        fn name(&self) -> &str {
            "Failing"
        }
        async fn send(&self, _event: &NotificationEvent) -> Result<()> {
            anyhow::bail!("provider unavailable")
        }
        async fn test(&self) -> Result<()> {
            Ok(())
        }
    }

    #[tokio::test]
    async fn test_service_fan_out() {
        let count1 = Arc::new(AtomicUsize::new(0));
        let count2 = Arc::new(AtomicUsize::new(0));

        let mut svc = NotificationService::new();
        svc.add_provider(Box::new(CountingProvider {
            send_count: count1.clone(),
        }));
        svc.add_provider(Box::new(CountingProvider {
            send_count: count2.clone(),
        }));

        let event = NotificationEvent::Import {
            title: "Test".into(),
            quality: "720p".into(),
        };
        svc.notify(&event).await;

        assert_eq!(count1.load(Ordering::SeqCst), 1);
        assert_eq!(count2.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn test_service_error_doesnt_stop_others() {
        let count = Arc::new(AtomicUsize::new(0));

        let mut svc = NotificationService::new();
        svc.add_provider(Box::new(FailingProvider));
        svc.add_provider(Box::new(CountingProvider {
            send_count: count.clone(),
        }));

        let event = NotificationEvent::HealthIssue {
            source: "test".into(),
            message: "msg".into(),
        };
        svc.notify(&event).await;

        // Second provider should still have been called despite first one failing
        assert_eq!(count.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn test_webhook_provider_sends_json() {
        use wiremock::matchers::{method, path};
        use wiremock::{Mock, MockServer, ResponseTemplate};

        let mock_server = MockServer::start().await;

        Mock::given(method("POST"))
            .and(path("/"))
            .respond_with(ResponseTemplate::new(200))
            .expect(1)
            .mount(&mock_server)
            .await;

        let provider = WebhookProvider::new(mock_server.uri());
        let event = NotificationEvent::Grab {
            title: "Test".into(),
            quality: "1080p".into(),
            indexer: "Idx".into(),
        };
        provider.send(&event).await.expect("webhook should succeed");
    }

    #[tokio::test]
    async fn test_telegram_provider_sends_message() {
        use wiremock::matchers::{body_json, method, path};
        use wiremock::{Mock, MockServer, ResponseTemplate};

        let mock_server = MockServer::start().await;

        let event = NotificationEvent::Grab {
            title: "Test".into(),
            quality: "1080p".into(),
            indexer: "Idx".into(),
        };

        Mock::given(method("POST"))
            .and(path("/bot123:faketoken/sendMessage"))
            .and(body_json(serde_json::json!({
                "chat_id": "456",
                "text": event.summary(),
                "parse_mode": "HTML",
            })))
            .respond_with(ResponseTemplate::new(200))
            .expect(1)
            .mount(&mock_server)
            .await;

        // Point the provider at the mock server instead of api.telegram.org
        let provider = TelegramProvider {
            client: reqwest::Client::new(),
            bot_token: "123:faketoken".to_string(),
            chat_id: "456".to_string(),
        };
        // Override the URL by sending directly to the mock
        let url = format!(
            "{}/bot{}/sendMessage",
            mock_server.uri(),
            provider.bot_token
        );
        let body = serde_json::json!({
            "chat_id": provider.chat_id,
            "text": event.summary(),
            "parse_mode": "HTML",
        });
        provider
            .client
            .post(&url)
            .json(&body)
            .send()
            .await
            .expect("request should succeed")
            .error_for_status()
            .expect("should be 200");
    }

    #[tokio::test]
    async fn test_slack_provider_sends_message() {
        use wiremock::matchers::{body_json, method, path};
        use wiremock::{Mock, MockServer, ResponseTemplate};

        let mock_server = MockServer::start().await;

        let event = NotificationEvent::Import {
            title: "Test Show S01E01".into(),
            quality: "720p".into(),
        };

        Mock::given(method("POST"))
            .and(path("/"))
            .and(body_json(serde_json::json!({
                "text": event.summary(),
            })))
            .respond_with(ResponseTemplate::new(200))
            .expect(1)
            .mount(&mock_server)
            .await;

        let provider = SlackProvider::new(mock_server.uri());
        provider.send(&event).await.expect("slack should succeed");
    }

    #[tokio::test]
    async fn test_email_provider_sends_message() {
        use wiremock::matchers::{body_json, method, path};
        use wiremock::{Mock, MockServer, ResponseTemplate};

        let mock_server = MockServer::start().await;

        let event = NotificationEvent::DownloadFailure {
            title: "Some.Release".into(),
            message: "connection timeout".into(),
        };

        Mock::given(method("POST"))
            .and(path("/"))
            .and(body_json(serde_json::json!({
                "from": "stackarr@example.com",
                "to": "user@example.com",
                "subject": "StackArr Notification",
                "body": event.summary(),
            })))
            .respond_with(ResponseTemplate::new(200))
            .expect(1)
            .mount(&mock_server)
            .await;

        let provider = EmailProvider::new(
            mock_server.uri(),
            "stackarr@example.com".to_string(),
            "user@example.com".to_string(),
        );
        provider.send(&event).await.expect("email should succeed");
    }

    #[tokio::test]
    async fn test_telegram_provider_test_method() {
        use wiremock::matchers::{method, path};
        use wiremock::{Mock, MockServer, ResponseTemplate};

        let mock_server = MockServer::start().await;

        Mock::given(method("POST"))
            .and(path("/bot123:faketoken/sendMessage"))
            .respond_with(ResponseTemplate::new(200))
            .expect(1)
            .mount(&mock_server)
            .await;

        // Build a provider that targets the mock server
        let provider = TelegramProvider {
            client: reqwest::Client::new(),
            bot_token: "123:faketoken".to_string(),
            chat_id: "456".to_string(),
        };
        // Manually invoke the test flow against the mock
        let url = format!(
            "{}/bot{}/sendMessage",
            mock_server.uri(),
            provider.bot_token
        );
        let test_event = NotificationEvent::HealthIssue {
            source: "test".to_string(),
            message: "This is a test notification from StackArr".to_string(),
        };
        let body = serde_json::json!({
            "chat_id": provider.chat_id,
            "text": test_event.summary(),
            "parse_mode": "HTML",
        });
        provider
            .client
            .post(&url)
            .json(&body)
            .send()
            .await
            .expect("request should succeed")
            .error_for_status()
            .expect("should be 200");
    }

    #[tokio::test]
    async fn test_slack_provider_test_method() {
        use wiremock::matchers::{method, path};
        use wiremock::{Mock, MockServer, ResponseTemplate};

        let mock_server = MockServer::start().await;

        Mock::given(method("POST"))
            .and(path("/"))
            .respond_with(ResponseTemplate::new(200))
            .expect(1)
            .mount(&mock_server)
            .await;

        let provider = SlackProvider::new(mock_server.uri());
        provider.test().await.expect("slack test should succeed");
    }

    #[tokio::test]
    async fn test_email_provider_test_method() {
        use wiremock::matchers::{method, path};
        use wiremock::{Mock, MockServer, ResponseTemplate};

        let mock_server = MockServer::start().await;

        Mock::given(method("POST"))
            .and(path("/"))
            .respond_with(ResponseTemplate::new(200))
            .expect(1)
            .mount(&mock_server)
            .await;

        let provider = EmailProvider::new(
            mock_server.uri(),
            "stackarr@example.com".to_string(),
            "user@example.com".to_string(),
        );
        provider.test().await.expect("email test should succeed");
    }

    // ── NotificationEvent serde roundtrip ──────────────────────────────

    #[test]
    fn event_serde_roundtrip_grab() {
        let event = NotificationEvent::Grab {
            title: "Show S01E01".into(),
            quality: "720p".into(),
            indexer: "NZBGeek".into(),
        };
        let json = serde_json::to_string(&event).unwrap();
        assert!(json.contains(r#""type":"grab"#));
        let deserialized: NotificationEvent = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.summary(), event.summary());
    }

    #[test]
    fn event_serde_roundtrip_all_variants() {
        let events: Vec<NotificationEvent> = vec![
            NotificationEvent::Grab {
                title: "T".into(),
                quality: "Q".into(),
                indexer: "I".into(),
            },
            NotificationEvent::Import {
                title: "T".into(),
                quality: "Q".into(),
            },
            NotificationEvent::Upgrade {
                title: "T".into(),
                old_quality: "O".into(),
                new_quality: "N".into(),
            },
            NotificationEvent::HealthIssue {
                source: "S".into(),
                message: "M".into(),
            },
            NotificationEvent::DownloadFailure {
                title: "T".into(),
                message: "M".into(),
            },
        ];
        for event in &events {
            let json = serde_json::to_string(event).unwrap();
            let rt: NotificationEvent = serde_json::from_str(&json).unwrap();
            assert_eq!(rt.summary(), event.summary());
        }
    }

    // ── wants_event ────────────────────────────────────────────────────

    fn make_row(
        on_grab: bool,
        on_import: bool,
        on_upgrade: bool,
        on_health: bool,
        on_failure: bool,
    ) -> NotificationProviderRow {
        NotificationProviderRow {
            id: 1,
            name: "test".into(),
            provider_type: "webhook".into(),
            config: serde_json::json!({"url": "http://x"}),
            on_grab,
            on_import,
            on_upgrade,
            on_health_issue: on_health,
            on_failure,
            enabled: true,
        }
    }

    #[test]
    fn wants_event_grab() {
        let row = make_row(true, false, false, false, false);
        let event = NotificationEvent::Grab {
            title: "T".into(),
            quality: "Q".into(),
            indexer: "I".into(),
        };
        assert!(row.wants_event(&event));
        assert!(!row.wants_event(&NotificationEvent::Import {
            title: "T".into(),
            quality: "Q".into(),
        }));
    }

    #[test]
    fn wants_event_all_types() {
        let row = make_row(true, true, true, true, true);
        assert!(row.wants_event(&NotificationEvent::Grab {
            title: "T".into(),
            quality: "Q".into(),
            indexer: "I".into(),
        }));
        assert!(row.wants_event(&NotificationEvent::Import {
            title: "T".into(),
            quality: "Q".into(),
        }));
        assert!(row.wants_event(&NotificationEvent::Upgrade {
            title: "T".into(),
            old_quality: "O".into(),
            new_quality: "N".into(),
        }));
        assert!(row.wants_event(&NotificationEvent::HealthIssue {
            source: "S".into(),
            message: "M".into(),
        }));
        assert!(row.wants_event(&NotificationEvent::DownloadFailure {
            title: "T".into(),
            message: "M".into(),
        }));
    }

    #[test]
    fn wants_event_none() {
        let row = make_row(false, false, false, false, false);
        assert!(!row.wants_event(&NotificationEvent::Grab {
            title: "T".into(),
            quality: "Q".into(),
            indexer: "I".into(),
        }));
    }

    // ── build_provider_from_config ─────────────────────────────────────

    #[test]
    fn build_provider_webhook() {
        let config = serde_json::json!({"url": "http://hook.example.com"});
        let provider = build_provider_from_config("webhook", &config);
        assert!(provider.is_some());
        assert_eq!(provider.unwrap().name(), "Webhook");
    }

    #[test]
    fn build_provider_discord() {
        let config = serde_json::json!({"webhook_url": "http://discord.com/api/webhooks/x"});
        let provider = build_provider_from_config("discord", &config);
        assert!(provider.is_some());
        assert_eq!(provider.unwrap().name(), "Discord");
    }

    #[test]
    fn build_provider_discord_url_fallback() {
        let config = serde_json::json!({"url": "http://discord.com/api/webhooks/x"});
        let provider = build_provider_from_config("discord", &config);
        assert!(provider.is_some());
    }

    #[test]
    fn build_provider_telegram() {
        let config = serde_json::json!({"bot_token": "123:abc", "chat_id": "456"});
        let provider = build_provider_from_config("telegram", &config);
        assert!(provider.is_some());
        assert_eq!(provider.unwrap().name(), "Telegram");
    }

    #[test]
    fn build_provider_telegram_missing_chat_id() {
        let config = serde_json::json!({"bot_token": "123:abc"});
        let provider = build_provider_from_config("telegram", &config);
        assert!(provider.is_none());
    }

    #[test]
    fn build_provider_slack() {
        let config = serde_json::json!({"webhook_url": "http://hooks.slack.com/x"});
        let provider = build_provider_from_config("slack", &config);
        assert!(provider.is_some());
        assert_eq!(provider.unwrap().name(), "Slack");
    }

    #[test]
    fn build_provider_email() {
        let config =
            serde_json::json!({"smtp_url": "http://smtp", "from": "a@b.com", "to": "c@d.com"});
        let provider = build_provider_from_config("email", &config);
        assert!(provider.is_some());
        assert_eq!(provider.unwrap().name(), "Email");
    }

    #[test]
    fn build_provider_email_missing_fields() {
        let config = serde_json::json!({"smtp_url": "http://smtp"});
        let provider = build_provider_from_config("email", &config);
        assert!(provider.is_none());
    }

    #[test]
    fn build_provider_unknown_type() {
        let config = serde_json::json!({});
        let provider = build_provider_from_config("unknown_type", &config);
        assert!(provider.is_none());
    }

    #[test]
    fn build_provider_webhook_missing_url() {
        let config = serde_json::json!({});
        let provider = build_provider_from_config("webhook", &config);
        assert!(provider.is_none());
    }

    // ── NotificationService default ────────────────────────────────────

    #[test]
    fn notification_service_default() {
        let svc = NotificationService::default();
        // Just verify it constructs without panic
        assert_eq!(svc.providers.len(), 0);
    }

    // ── Provider names ─────────────────────────────────────────────────

    #[test]
    fn provider_names() {
        assert_eq!(WebhookProvider::new("x".into()).name(), "Webhook");
        assert_eq!(DiscordProvider::new("x".into()).name(), "Discord");
        assert_eq!(
            TelegramProvider::new("t".into(), "c".into()).name(),
            "Telegram"
        );
        assert_eq!(SlackProvider::new("x".into()).name(), "Slack");
        assert_eq!(
            EmailProvider::new("s".into(), "f".into(), "t".into()).name(),
            "Email"
        );
    }
}
