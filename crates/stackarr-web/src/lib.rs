pub mod dav_manager;
pub mod middleware;
pub mod openapi;
pub mod routes;
pub mod state;

#[cfg(feature = "embed-ui")]
pub mod embedded_ui;

pub use state::AppState;

use std::sync::Arc;

use axum::Router;
use axum::http::{HeaderName, HeaderValue, Method};
use axum::middleware::from_fn_with_state;
use tower_http::compression::CompressionLayer;
use tower_http::cors::CorsLayer;
use tower_http::services::{ServeDir, ServeFile};
use tower_http::set_header::SetResponseHeaderLayer;
use tower_http::trace::TraceLayer;
use utoipa::OpenApi;
use utoipa_swagger_ui::SwaggerUi;

/// Build the full API router with all routes mounted.
pub fn build_router(state: Arc<AppState>) -> Router {
    // ── Public routes (no auth required) ─────────────────────────────
    let public_routes = Router::new()
        .merge(routes::health::router())
        .merge(routes::system::public_router())
        .merge(routes::images::router())
        .merge(routes::auth::router())
        .merge(routes::plex::webhook_router())
        .merge(routes::stremio::router());

    // ── Protected routes (require API key) ───────────────────────────
    let protected_routes = Router::new()
        .merge(routes::system::protected_router())
        .merge(routes::series::router())
        .merge(routes::movies::router())
        .merge(routes::queue::router())
        .merge(routes::history::router())
        .merge(routes::releases::router())
        .merge(routes::quality::router())
        .merge(routes::medialibraryfolders::router())
        .merge(routes::tags::router())
        .merge(routes::naming::router())
        .merge(routes::downloadclients::router())
        .merge(routes::indexers::router())
        .merge(routes::calendar::router())
        .merge(routes::wanted::router())
        .merge(routes::rss::router())
        .merge(routes::episodes::router())
        .merge(routes::torrent::router())
        .merge(routes::usenet::router())
        .merge(routes::importlists::router())
        .merge(routes::import_candidates::router())
        .merge(routes::indexarr::router())
        .merge(routes::discover::router())
        .merge(routes::plex::router())
        .merge(routes::blocklist::router())
        .merge(routes::backup::router())
        .merge(routes::logs::router())
        .merge(routes::stream::router())
        .merge(routes::remote::router())
        .merge(routes::search::router())
        .merge(routes::general::router())
        .merge(routes::mediamanagement::router())
        .merge(routes::admin::router())
        .merge(routes::user::router())
        .merge(routes::progress::router())
        .merge(routes::requests::router())
        .merge(routes::watchlist::router())
        .merge(routes::notifications::router())
        .merge(routes::notification_providers::router())
        .merge(routes::scheduler::router())
        .merge(routes::filebrowser::router())
        .merge(routes::activities::router())
        .merge(routes::bootstrap::router())
        .merge(routes::dav::router())
        .layer(from_fn_with_state(
            state.clone(),
            middleware::require_auth_middleware,
        ));

    // ── CORS configuration ───────────────────────────────────────────
    // Allow same-origin, localhost (dev), private IPs, and tauri:// origins.
    // This prevents arbitrary external sites from making credentialed requests.
    let cors = CorsLayer::new()
        .allow_methods([
            Method::GET,
            Method::POST,
            Method::PUT,
            Method::DELETE,
            Method::OPTIONS,
        ])
        .allow_headers([
            axum::http::header::CONTENT_TYPE,
            axum::http::header::AUTHORIZATION,
            HeaderName::from_static("x-api-key"),
            HeaderName::from_static("x-csrf-token"),
        ])
        .allow_origin(tower_http::cors::AllowOrigin::predicate(
            |origin: &HeaderValue, _req: &axum::http::request::Parts| {
                let Ok(origin_str) = origin.to_str() else {
                    return false;
                };
                // Allow localhost (dev), private IPs, tauri apps
                let host = origin_str
                    .strip_prefix("http://")
                    .or_else(|| origin_str.strip_prefix("https://"))
                    .or_else(|| origin_str.strip_prefix("tauri://"))
                    .unwrap_or(origin_str);
                let host_no_port = host.split(':').next().unwrap_or(host);
                host_no_port == "localhost"
                    || host_no_port == "127.0.0.1"
                    || host_no_port == "0.0.0.0"
                    || host_no_port == "tauri.localhost" // Tauri WebView (Android)
                    || host_no_port.starts_with("192.168.")
                    || host_no_port.starts_with("10.")
                    || host_no_port.starts_with("172.")
                    || host_no_port.starts_with("100.") // Tailscale
                    || origin_str.starts_with("tauri://")
            },
        ))
        .allow_credentials(true);

    // ── OpenAPI / Swagger UI ────────────────────────────────────────
    let openapi_doc = openapi::ApiDoc::openapi();
    let swagger_routes = SwaggerUi::new("/swagger-ui").url("/api-docs/openapi.json", openapi_doc);

    // ── WebDAV mount (if DAV module enabled) ──────────────────────────
    let dav_webdav = routes::dav::webdav_router(&state);

    // ── Security headers ─────────────────────────────────────────────
    let mut api_router = Router::new()
        .merge(public_routes)
        .merge(protected_routes)
        .merge(swagger_routes)
        .layer(CompressionLayer::new())
        .layer(TraceLayer::new_for_http())
        .layer(cors)
        .layer(SetResponseHeaderLayer::overriding(
            HeaderName::from_static("x-frame-options"),
            HeaderValue::from_static("DENY"),
        ))
        .layer(SetResponseHeaderLayer::overriding(
            HeaderName::from_static("x-content-type-options"),
            HeaderValue::from_static("nosniff"),
        ))
        .layer(SetResponseHeaderLayer::overriding(
            HeaderName::from_static("x-xss-protection"),
            HeaderValue::from_static("1; mode=block"),
        ))
        .layer(SetResponseHeaderLayer::overriding(
            HeaderName::from_static("referrer-policy"),
            HeaderValue::from_static("strict-origin-when-cross-origin"),
        ))
        .layer(SetResponseHeaderLayer::overriding(
            HeaderName::from_static("permissions-policy"),
            HeaderValue::from_static("camera=(), microphone=(), geolocation=()"),
        ))
        .layer(SetResponseHeaderLayer::overriding(
            HeaderName::from_static("strict-transport-security"),
            HeaderValue::from_static("max-age=31536000; includeSubDomains"),
        ))
        .with_state(state);

    // Mount WebDAV at /dav if the module is enabled
    if let Some(dav_router) = dav_webdav {
        api_router = api_router.nest("/dav", dav_router);
    }

    // Serve UI assets — embedded (if feature enabled and no env override) or from filesystem
    #[cfg(feature = "embed-ui")]
    {
        let use_embedded_ui = std::env::var("STACKARR_UI_DIR").is_err();
        let use_embedded_client = std::env::var("STACKARR_CLIENT_DIR").is_err();

        let mut r = Router::new().merge(api_router);

        if use_embedded_client {
            r = r.nest("/app", embedded_ui::embedded_client_router());
        } else {
            let client_dir = std::env::var("STACKARR_CLIENT_DIR").unwrap();
            let client_fallback = ServeFile::new(format!("{client_dir}/index.html"));
            let client_serve = ServeDir::new(&client_dir).fallback(client_fallback);
            r = r.nest_service("/app", client_serve);
        }

        if use_embedded_ui {
            r = r.fallback_service(embedded_ui::embedded_ui_router());
        } else {
            let ui_dir = std::env::var("STACKARR_UI_DIR").unwrap();
            let spa_fallback = ServeFile::new(format!("{ui_dir}/index.html"));
            let serve_dir = ServeDir::new(&ui_dir).fallback(spa_fallback);
            r = r.fallback_service(serve_dir);
        }

        r
    }

    #[cfg(not(feature = "embed-ui"))]
    {
        let client_dir =
            std::env::var("STACKARR_CLIENT_DIR").unwrap_or_else(|_| "/client".to_string());
        let client_fallback = ServeFile::new(format!("{client_dir}/index.html"));
        let client_serve = ServeDir::new(&client_dir).fallback(client_fallback);

        let ui_dir = std::env::var("STACKARR_UI_DIR").unwrap_or_else(|_| "/ui".to_string());
        let spa_fallback = ServeFile::new(format!("{ui_dir}/index.html"));
        let serve_dir = ServeDir::new(&ui_dir).fallback(spa_fallback);

        Router::new()
            .merge(api_router)
            .nest_service("/app", client_serve)
            .fallback_service(serve_dir)
    }
}

/// Start the Axum server on the given address.
pub async fn run(addr: &str, state: Arc<AppState>) -> anyhow::Result<()> {
    run_with_tls(addr, state, None).await
}

/// TLS listener configuration.  Pass via `run_with_tls` to serve HTTPS on a
/// second port using the cert distributed from the bootstrap node.
pub struct TlsListenerConfig {
    /// Address to bind (e.g. "0.0.0.0:9443").
    pub addr: String,
    /// Watch receiver for cert updates.  Send a new `Some(TlsCertData)` when
    /// the cert has been written to disk; the listener will reload its
    /// rustls config atomically.
    pub cert_rx: tokio::sync::watch::Receiver<Option<TlsCertData>>,
}

/// TLS cert + key PEM bytes for the rustls server config.
#[derive(Clone, Debug)]
pub struct TlsCertData {
    pub cert_pem: Vec<u8>,
    pub key_pem: Vec<u8>,
}

/// Start the Axum server with an optional TLS listener for direct HTTPS.
pub async fn run_with_tls(
    addr: &str,
    state: Arc<AppState>,
    tls: Option<TlsListenerConfig>,
) -> anyhow::Result<()> {
    let router = build_router(state);

    // Start TLS listener (if configured) as a background task
    if let Some(tls_cfg) = tls {
        let tls_router = router.clone();
        tokio::spawn(async move {
            if let Err(e) = run_tls_listener(tls_cfg, tls_router).await {
                tracing::error!(error = %e, "TLS listener exited with error");
            }
        });
    }

    let listener = tokio::net::TcpListener::bind(addr).await?;
    tracing::info!("listening on {addr}");
    axum::serve(listener, router)
        .with_graceful_shutdown(shutdown_signal())
        .await?;
    Ok(())
}

/// Run the TLS listener — accepts connections and serves them using the
/// current rustls config. Reloads config atomically when the watch channel
/// fires.
async fn run_tls_listener(mut tls_cfg: TlsListenerConfig, router: Router) -> anyhow::Result<()> {
    use arc_swap::ArcSwap;
    use tokio_rustls::TlsAcceptor;

    // Wait for initial cert
    loop {
        if tls_cfg.cert_rx.borrow().is_some() {
            break;
        }
        tracing::info!("TLS listener waiting for initial cert from bootstrap");
        if tls_cfg.cert_rx.changed().await.is_err() {
            return Ok(()); // channel closed
        }
    }

    let build_config = |cert: &TlsCertData| -> anyhow::Result<Arc<rustls::ServerConfig>> {
        let certs =
            rustls_pemfile::certs(&mut cert.cert_pem.as_slice()).collect::<Result<Vec<_>, _>>()?;
        let key = rustls_pemfile::private_key(&mut cert.key_pem.as_slice())?
            .ok_or_else(|| anyhow::anyhow!("no private key in PEM"))?;
        let cfg = rustls::ServerConfig::builder()
            .with_no_client_auth()
            .with_single_cert(certs, key)?;
        Ok(Arc::new(cfg))
    };

    let initial_cert = tls_cfg.cert_rx.borrow().clone().unwrap();
    let server_config = Arc::new(ArcSwap::from(build_config(&initial_cert)?));

    // Spawn reload watcher
    let reload_config = Arc::clone(&server_config);
    let mut reload_rx = tls_cfg.cert_rx.clone();
    tokio::spawn(async move {
        while reload_rx.changed().await.is_ok() {
            if let Some(cert) = reload_rx.borrow().clone() {
                match build_config(&cert) {
                    Ok(new_cfg) => {
                        reload_config.store(new_cfg);
                        tracing::info!("TLS config reloaded");
                    }
                    Err(e) => tracing::warn!(error = %e, "failed to build new TLS config"),
                }
            }
        }
    });

    let listener = tokio::net::TcpListener::bind(&tls_cfg.addr).await?;
    tracing::info!("TLS listening on {}", tls_cfg.addr);

    loop {
        let (stream, peer_addr) = match listener.accept().await {
            Ok(v) => v,
            Err(e) => {
                tracing::warn!(error = %e, "TLS accept failed");
                continue;
            }
        };
        let acceptor = TlsAcceptor::from(server_config.load_full());
        let router = router.clone();
        tokio::spawn(async move {
            match acceptor.accept(stream).await {
                Ok(tls_stream) => {
                    let service = hyper::service::service_fn(move |req| {
                        let router = router.clone();
                        async move {
                            use tower::ServiceExt;
                            router.oneshot(req).await
                        }
                    });
                    let io = hyper_util::rt::TokioIo::new(tls_stream);
                    if let Err(e) = hyper::server::conn::http1::Builder::new()
                        .serve_connection(io, service)
                        .await
                    {
                        tracing::debug!(%peer_addr, error = %e, "TLS connection error");
                    }
                }
                Err(e) => tracing::debug!(%peer_addr, error = %e, "TLS handshake failed"),
            }
        });
    }
}

async fn shutdown_signal() {
    tokio::signal::ctrl_c()
        .await
        .expect("failed to install CTRL+C handler");
    tracing::info!("shutdown signal received");
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::Body;
    use http_body_util::BodyExt;
    use tower::ServiceExt;

    use arc_swap::ArcSwap;
    use stackarr_core::config::{AppConfig, EnabledModules};
    use stackarr_core::db::Database;
    use stackarr_core::test_helpers::{TestDb, seed_quality_profile};
    use stackarr_download::DownloadClientManager;
    use stackarr_indexer::IndexerManager;
    use tokio::sync::RwLock;

    async fn test_state() -> (Arc<AppState>, TestDb) {
        let db = TestDb::new().await;
        let database = Database::from_pool(db.pool.clone());
        let config = Arc::new(ArcSwap::from_pointee(AppConfig::default()));
        let state = Arc::new(AppState {
            db: database,
            config,
            modules: EnabledModules::default(),
            start_time: std::time::Instant::now(),
            torrent_session: arc_swap::ArcSwapOption::empty(),
            torrent_api: arc_swap::ArcSwapOption::empty(),
            usenet_queue: arc_swap::ArcSwapOption::empty(),
            indexarr_client: None,
            indexarr_available: false,
            cardigann_engine: Arc::new(stackarr_cardigann::CardigannEngine::new(
                std::path::Path::new(""),
            )),
            indexer_manager: Arc::new(RwLock::new(IndexerManager::new())),
            download_manager: Arc::new(RwLock::new(DownloadClientManager::new())),
            rate_limiter: None,
            tmdb_client: None,
            stream_session_manager: None,
            log_buffer: stackarr_core::log_buffer::LogBuffer::new(),
            cached_api_key: arc_swap::ArcSwap::from_pointee(None),
            cached_auth_method: arc_swap::ArcSwap::from_pointee("none".to_string()),
            scheduler_registry: arc_swap::ArcSwapOption::empty(),
            search_cancel_tokens: dashmap::DashMap::new(),
            dav_manager: arc_swap::ArcSwapOption::empty(),
        });
        (state, db)
    }

    async fn body_to_json(body: Body) -> serde_json::Value {
        let bytes = body.collect().await.unwrap().to_bytes();
        serde_json::from_slice(&bytes).unwrap_or(serde_json::Value::Null)
    }

    #[tokio::test]
    #[ignore = "requires running postgres"]
    async fn test_health_check() {
        let (state, db) = test_state().await;
        let app = build_router(state);

        let resp = app
            .oneshot(
                axum::http::Request::builder()
                    .uri("/health")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(resp.status(), 200);
        db.cleanup().await;
    }

    #[tokio::test]
    #[ignore = "requires running postgres"]
    async fn test_system_health() {
        let (state, db) = test_state().await;
        let app = build_router(state);

        let resp = app
            .oneshot(
                axum::http::Request::builder()
                    .uri("/api/v1/system/health")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(resp.status(), 200);
        let json = body_to_json(resp.into_body()).await;
        assert_eq!(json["status"], "ok");
        db.cleanup().await;
    }

    #[tokio::test]
    #[ignore = "requires running postgres"]
    async fn test_list_series_empty() {
        let (state, db) = test_state().await;
        let app = build_router(state);

        // No API key stored = first boot = no auth required
        let resp = app
            .oneshot(
                axum::http::Request::builder()
                    .uri("/api/v1/series")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(resp.status(), 200);
        let json = body_to_json(resp.into_body()).await;
        assert_eq!(json, serde_json::json!([]));
        db.cleanup().await;
    }

    #[tokio::test]
    #[ignore = "requires running postgres"]
    async fn test_create_series() {
        let (state, db) = test_state().await;
        let profile_id = seed_quality_profile(&db.pool).await;
        let app = build_router(state);

        let body = serde_json::json!({
            "title": "Breaking Bad",
            "path": "/tv/Breaking Bad",
            "qualityProfileId": profile_id,
            "monitored": true,
            "tvdbId": 81189
        });

        let resp = app
            .oneshot(
                axum::http::Request::builder()
                    .method("POST")
                    .uri("/api/v1/series")
                    .header("content-type", "application/json")
                    .body(Body::from(serde_json::to_vec(&body).unwrap()))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert!(resp.status().is_success());
        let json = body_to_json(resp.into_body()).await;
        assert_eq!(json["title"], "Breaking Bad");
        db.cleanup().await;
    }

    #[tokio::test]
    #[ignore = "requires running postgres"]
    async fn test_list_tags_empty() {
        let (state, db) = test_state().await;
        let app = build_router(state);

        let resp = app
            .oneshot(
                axum::http::Request::builder()
                    .uri("/api/v1/tag")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(resp.status(), 200);
        let json = body_to_json(resp.into_body()).await;
        assert_eq!(json, serde_json::json!([]));
        db.cleanup().await;
    }

    #[tokio::test]
    #[ignore = "requires running postgres"]
    async fn test_list_queue_empty() {
        let (state, db) = test_state().await;
        let app = build_router(state);

        let resp = app
            .oneshot(
                axum::http::Request::builder()
                    .uri("/api/v1/queue")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(resp.status(), 200);
        let json = body_to_json(resp.into_body()).await;
        assert_eq!(json, serde_json::json!([]));
        db.cleanup().await;
    }

    #[tokio::test]
    #[ignore = "requires running postgres"]
    async fn test_list_quality_profiles_empty() {
        let (state, db) = test_state().await;
        let app = build_router(state);

        let resp = app
            .oneshot(
                axum::http::Request::builder()
                    .uri("/api/v1/qualityprofile")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(resp.status(), 200);
        db.cleanup().await;
    }

    #[tokio::test]
    #[ignore = "requires running postgres"]
    async fn test_list_media_library_folders_empty() {
        let (state, db) = test_state().await;
        let app = build_router(state);

        let resp = app
            .oneshot(
                axum::http::Request::builder()
                    .uri("/api/v1/medialibraryfolder")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(resp.status(), 200);
        db.cleanup().await;
    }
}
