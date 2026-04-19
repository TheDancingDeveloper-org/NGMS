//! Embedded UI assets served from the binary via rust-embed.
//!
//! When the `embed-ui` feature is enabled, the React UI build output is compiled
//! into the binary. This module provides axum handlers that serve these assets
//! with proper content types and SPA fallback (index.html for unmatched routes).

use axum::Router;
use axum::http::{StatusCode, Uri, header};
use axum::response::{IntoResponse, Response};
use rust_embed::Embed;

#[derive(Embed)]
#[folder = "../../ui/dist"]
struct UiAssets;

#[derive(Embed)]
#[folder = "../../client/dist"]
struct ClientAssets;

/// Build a stateless axum router that serves embedded UI assets with SPA fallback.
pub fn embedded_ui_router() -> Router {
    Router::new().fallback(serve_ui_asset)
}

/// Build a stateless axum router that serves embedded client assets with SPA fallback.
pub fn embedded_client_router() -> Router {
    Router::new().fallback(serve_client_asset)
}

async fn serve_ui_asset(uri: Uri) -> Response {
    serve_embedded::<UiAssets>(uri.path())
}

async fn serve_client_asset(uri: Uri) -> Response {
    serve_embedded::<ClientAssets>(uri.path())
}

fn serve_embedded<E: Embed>(path: &str) -> Response {
    // Strip leading slash
    let path = path.trim_start_matches('/');

    // Try the exact path first, then fall back to index.html (SPA routing)
    let (file, effective_path) = if path.is_empty() {
        (E::get("index.html"), "index.html")
    } else {
        match E::get(path) {
            Some(f) => (Some(f), path),
            None => (E::get("index.html"), "index.html"),
        }
    };

    match file {
        Some(content) => {
            let mime = mime_guess::from_path(effective_path).first_or_octet_stream();

            (
                StatusCode::OK,
                [
                    (header::CONTENT_TYPE, mime.as_ref().to_string()),
                    (
                        header::CACHE_CONTROL,
                        if effective_path.contains('.') && !effective_path.ends_with(".html") {
                            // Hashed assets get long cache
                            "public, max-age=31536000, immutable".to_string()
                        } else {
                            // HTML files get short cache
                            "public, max-age=60".to_string()
                        },
                    ),
                ],
                content.data.to_vec(),
            )
                .into_response()
        }
        None => StatusCode::NOT_FOUND.into_response(),
    }
}
