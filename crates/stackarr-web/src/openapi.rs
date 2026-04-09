use utoipa::OpenApi;
use utoipa::openapi::security::{ApiKey, ApiKeyValue, Http, HttpAuthScheme, SecurityScheme};

/// OpenAPI documentation for the StackArr API.
///
/// Endpoints are annotated incrementally — not all routes appear here yet.
/// Visit `/swagger-ui/` for the interactive documentation.
#[derive(OpenApi)]
#[openapi(
    info(
        title = "StackArr API",
        version = "1.0.0",
        description = "Unified media management server API. Manages TV series, movies, downloads, indexers, notifications, and more.",
        license(name = "Proprietary"),
    ),
    servers(
        (url = "/", description = "Current instance"),
    ),
    tags(
        (name = "Health", description = "Health checks and metrics"),
        (name = "System", description = "System status, commands, and configuration"),
        (name = "Auth", description = "Authentication and session management"),
        (name = "Series", description = "TV series management"),
        (name = "Movies", description = "Movie management"),
        (name = "Episodes", description = "Episode management"),
        (name = "Queue", description = "Download queue"),
        (name = "History", description = "Event history"),
        (name = "Quality", description = "Quality profiles and custom formats"),
        (name = "Indexers", description = "Indexer configuration"),
        (name = "Download Clients", description = "Download client configuration"),
        (name = "Notifications", description = "Notification provider configuration"),
        (name = "Scheduler", description = "Background task scheduler"),
        (name = "Logs", description = "Application logs"),
        (name = "Activities", description = "Running and recent activities"),
        (name = "Backup", description = "Backup and restore"),
        (name = "Tags", description = "Tag management"),
        (name = "Media Library", description = "Media library folders"),
        (name = "Naming", description = "File naming configuration"),
        (name = "Calendar", description = "Upcoming episodes and movies"),
        (name = "Wanted", description = "Missing and cutoff unmet media"),
        (name = "RSS", description = "RSS feed management"),
        (name = "Releases", description = "Release search and grab"),
        (name = "Torrent", description = "Embedded torrent engine"),
        (name = "Usenet", description = "Embedded usenet engine"),
        (name = "Plex", description = "Plex integration"),
        (name = "Discover", description = "Media discovery and recommendations"),
        (name = "Streaming", description = "Video streaming"),
        (name = "Import Lists", description = "Import list management"),
        (name = "Blocklist", description = "Blocklisted releases"),
        (name = "Search", description = "Global search"),
        (name = "Config", description = "Application configuration"),
        (name = "Admin", description = "User and invite management"),
        (name = "User", description = "User profile and preferences"),
        (name = "File Browser", description = "Server filesystem browsing"),
    ),
    paths(
        // Wave 1 — Health & System
        crate::routes::health::health_check,
        crate::routes::health::system_health,
        crate::routes::health::system_diagnostics,
        crate::routes::health::prometheus_metrics,
        crate::routes::logs::get_logs,
        crate::routes::logs::list_log_files,
        crate::routes::activities::list_activities,
        crate::routes::activities::clear_activities,
        crate::routes::activities::running_count,
        // Notification providers
        crate::routes::notification_providers::list_providers,
        crate::routes::notification_providers::get_provider,
        crate::routes::notification_providers::create_provider,
        crate::routes::notification_providers::update_provider,
        crate::routes::notification_providers::delete_provider,
        crate::routes::notification_providers::test_saved_provider,
        crate::routes::notification_providers::test_provider_config,
        // Series
        crate::routes::series::list_series,
        crate::routes::series::get_series,
        crate::routes::series::create_series,
        crate::routes::series::update_series,
        crate::routes::series::delete_series,
        crate::routes::series::lookup_series,
        // Movies
        crate::routes::movies::list_movies,
        crate::routes::movies::get_movie,
        crate::routes::movies::create_movie,
        crate::routes::movies::update_movie,
        crate::routes::movies::delete_movie,
        crate::routes::movies::lookup_movie,
        // Queue
        crate::routes::queue::list_queue,
        crate::routes::queue::delete_queue_item,
        // Quality Profiles
        crate::routes::quality::list_profiles,
        crate::routes::quality::get_profile,
        crate::routes::quality::create_profile,
        crate::routes::quality::update_profile,
        crate::routes::quality::delete_profile,
        crate::routes::quality::list_custom_formats,
        crate::routes::quality::get_custom_format,
        crate::routes::quality::create_custom_format,
        crate::routes::quality::update_custom_format,
        crate::routes::quality::delete_custom_format,
        crate::routes::quality::test_custom_format,
        // Tags
        crate::routes::tags::list_tags,
        crate::routes::tags::create_tag,
        crate::routes::tags::update_tag,
        crate::routes::tags::delete_tag,
        // Download Clients
        crate::routes::downloadclients::list_download_clients,
        crate::routes::downloadclients::create_download_client,
        crate::routes::downloadclients::update_download_client,
        crate::routes::downloadclients::delete_download_client,
        crate::routes::downloadclients::test_download_client,
        // Scheduler
        crate::routes::scheduler::list_tasks,
        crate::routes::scheduler::trigger_task,
    ),
    components(schemas(
        crate::routes::health::HealthResponse,
    )),
    modifiers(&SecurityAddon),
)]
pub struct ApiDoc;

/// Adds security schemes to the OpenAPI spec.
struct SecurityAddon;

impl utoipa::Modify for SecurityAddon {
    fn modify(&self, openapi: &mut utoipa::openapi::OpenApi) {
        if let Some(components) = openapi.components.as_mut() {
            components.add_security_scheme(
                "ApiKeyAuth",
                SecurityScheme::ApiKey(ApiKey::Header(ApiKeyValue::new("X-Api-Key"))),
            );
            components.add_security_scheme(
                "BearerAuth",
                SecurityScheme::Http(Http::new(HttpAuthScheme::Bearer)),
            );
            components.add_security_scheme(
                "BasicAuth",
                SecurityScheme::Http(Http::new(HttpAuthScheme::Basic)),
            );
        }
    }
}
