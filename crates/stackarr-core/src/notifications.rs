use crate::db::Database;

/// High-level helpers for creating notifications.
pub struct NotificationService;

impl NotificationService {
    /// Notify all users about a new episode being available.
    pub async fn notify_new_episode(
        db: &Database,
        series_title: &str,
        season: i32,
        episode: i32,
        episode_title: Option<&str>,
    ) -> crate::Result<u64> {
        let title = format!("{series_title} - S{season:02}E{episode:02}");
        let body = episode_title.map(|t| format!("\"{t}\" is now available"));
        let data = serde_json::json!({
            "type": "new_episode",
            "seriesTitle": series_title,
            "season": season,
            "episode": episode,
            "episodeTitle": episode_title,
        });
        db.create_notification_for_all_users("new_episode", &title, body.as_deref(), Some(data))
            .await
    }

    /// Notify all users about a new movie being available.
    pub async fn notify_new_movie(
        db: &Database,
        movie_title: &str,
        year: Option<i32>,
    ) -> crate::Result<u64> {
        let title = match year {
            Some(y) => format!("{movie_title} ({y})"),
            None => movie_title.to_string(),
        };
        let body = Some("is now available".to_string());
        let data = serde_json::json!({
            "type": "new_movie",
            "movieTitle": movie_title,
            "year": year,
        });
        db.create_notification_for_all_users("new_movie", &title, body.as_deref(), Some(data))
            .await
    }

    /// Notify a specific user about a media request status change.
    pub async fn notify_request_update(
        db: &Database,
        user_id: i64,
        request_title: &str,
        new_status: &str,
    ) -> crate::Result<crate::models::user::UserNotification> {
        let title = format!("Request Update: {request_title}");
        let body = match new_status {
            "approved" => Some("Your request has been approved".to_string()),
            "declined" => Some("Your request has been declined".to_string()),
            "available" => Some("Your requested media is now available".to_string()),
            _ => Some(format!("Status changed to: {new_status}")),
        };
        let data = serde_json::json!({
            "type": "request_update",
            "title": request_title,
            "status": new_status,
        });
        db.create_notification(
            user_id,
            "request_update",
            &title,
            body.as_deref(),
            Some(data),
        )
        .await
    }
}
