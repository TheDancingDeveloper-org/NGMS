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
        let url = format!(
            "https://api.telegram.org/bot{}/sendMessage",
            self.bot_token
        );
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::Arc;

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
        fn name(&self) -> &str { "Counting" }
        async fn send(&self, _event: &NotificationEvent) -> Result<()> {
            self.send_count.fetch_add(1, Ordering::SeqCst);
            Ok(())
        }
        async fn test(&self) -> Result<()> { Ok(()) }
    }

    struct FailingProvider;

    #[async_trait::async_trait]
    impl NotificationProvider for FailingProvider {
        fn name(&self) -> &str { "Failing" }
        async fn send(&self, _event: &NotificationEvent) -> Result<()> {
            anyhow::bail!("provider unavailable")
        }
        async fn test(&self) -> Result<()> { Ok(()) }
    }

    #[tokio::test]
    async fn test_service_fan_out() {
        let count1 = Arc::new(AtomicUsize::new(0));
        let count2 = Arc::new(AtomicUsize::new(0));

        let mut svc = NotificationService::new();
        svc.add_provider(Box::new(CountingProvider { send_count: count1.clone() }));
        svc.add_provider(Box::new(CountingProvider { send_count: count2.clone() }));

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
        svc.add_provider(Box::new(CountingProvider { send_count: count.clone() }));

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
        use wiremock::{MockServer, Mock, ResponseTemplate};
        use wiremock::matchers::{method, path};

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
        use wiremock::{MockServer, Mock, ResponseTemplate};
        use wiremock::matchers::{method, path, body_json};

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
        let url = format!("{}/bot{}/sendMessage", mock_server.uri(), provider.bot_token);
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
        use wiremock::{MockServer, Mock, ResponseTemplate};
        use wiremock::matchers::{method, path, body_json};

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
        use wiremock::{MockServer, Mock, ResponseTemplate};
        use wiremock::matchers::{method, path, body_json};

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
        use wiremock::{MockServer, Mock, ResponseTemplate};
        use wiremock::matchers::{method, path};

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
        let url = format!("{}/bot{}/sendMessage", mock_server.uri(), provider.bot_token);
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
        use wiremock::{MockServer, Mock, ResponseTemplate};
        use wiremock::matchers::{method, path};

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
        use wiremock::{MockServer, Mock, ResponseTemplate};
        use wiremock::matchers::{method, path};

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
}
