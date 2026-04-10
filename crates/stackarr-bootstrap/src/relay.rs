//! HTTPS relay — proxies API requests from mobile apps to StackArr servers.
//!
//! Mobile apps connect to `https://streamrelay.indexarr.net/relay/{server_id}/...`
//! and the relay forwards the request to `http://{public_ip}:{port}/...` over
//! plain HTTP.  TLS terminates at Caddy on Vultr.
//!
//! The relay is transparent with respect to authentication — it passes the
//! `Authorization` header through to the upstream server which validates it.

use std::sync::Arc;
use std::time::Duration;

use axum::body::Body;
use axum::extract::{Path, Request, State};
use axum::http::{HeaderMap, HeaderName, HeaderValue, StatusCode};
use axum::response::{IntoResponse, Response};
use futures_util::TryStreamExt;
use uuid::Uuid;

use crate::state::BootstrapState;

/// Headers to forward from the client request to the upstream server.
const FORWARD_REQUEST_HEADERS: &[&str] = &[
    "authorization",
    "content-type",
    "accept",
    "range",
    "cookie",
    "if-none-match",
    "if-modified-since",
    "user-agent",
];

/// Headers to forward from the upstream response back to the client.
const FORWARD_RESPONSE_HEADERS: &[&str] = &[
    "content-type",
    "content-length",
    "content-disposition",
    "cache-control",
    "etag",
    "accept-ranges",
    "content-range",
    "location",
    "set-cookie",
    "x-request-id",
];

pub async fn relay_handler(
    State(state): State<Arc<BootstrapState>>,
    Path((server_id, path)): Path<(Uuid, String)>,
    req: Request,
) -> Response {
    // Look up the server in the registry
    let Some(server) = state.servers.get(&server_id) else {
        return (
            StatusCode::BAD_GATEWAY,
            serde_json::json!({"error": "server not registered or offline"}).to_string(),
        )
            .into_response();
    };

    let upstream_base = format!("http://{}:{}", server.public_ip, server.port);
    drop(server); // release DashMap ref before async work

    // Build the upstream URL
    let query = req
        .uri()
        .query()
        .map(|q| format!("?{q}"))
        .unwrap_or_default();
    let upstream_url = format!("{upstream_base}/{path}{query}");

    // Build the outbound request
    let method = req.method().clone();
    let client_headers = req.headers().clone();

    // Determine timeout — longer for streaming paths
    let timeout = if is_streaming_path(&path) {
        Duration::from_secs(3600) // 1 hour for video streams
    } else {
        state.relay_timeout
    };

    // Extract request body
    let body_bytes = match axum::body::to_bytes(req.into_body(), 10 * 1024 * 1024).await {
        Ok(b) => b,
        Err(_) => {
            return (
                StatusCode::PAYLOAD_TOO_LARGE,
                serde_json::json!({"error": "request body too large"}).to_string(),
            )
                .into_response();
        }
    };

    // Build reqwest request
    let mut upstream_req = state
        .relay_client
        .request(
            reqwest::Method::from_bytes(method.as_str().as_bytes()).unwrap_or(reqwest::Method::GET),
            &upstream_url,
        )
        .timeout(timeout);

    // Forward selected headers
    let mut req_headers = reqwest::header::HeaderMap::new();
    for &name in FORWARD_REQUEST_HEADERS {
        if let Some(val) = client_headers.get(name) {
            if let Ok(hname) = reqwest::header::HeaderName::from_bytes(name.as_bytes()) {
                if let Ok(hval) = reqwest::header::HeaderValue::from_bytes(val.as_bytes()) {
                    req_headers.insert(hname, hval);
                }
            }
        }
    }
    // Add relay indicator
    req_headers.insert("x-relay", "true".parse().unwrap());
    upstream_req = upstream_req.headers(req_headers);

    // Add body for methods that have one
    if !body_bytes.is_empty() {
        upstream_req = upstream_req.body(body_bytes);
    }

    // Send to upstream
    let upstream_resp = match upstream_req.send().await {
        Ok(r) => r,
        Err(e) => {
            tracing::warn!(
                server_id = %server_id,
                url = %upstream_url,
                error = %e,
                "relay upstream request failed"
            );
            let msg = if e.is_connect() {
                "server unreachable"
            } else if e.is_timeout() {
                "server request timed out"
            } else {
                "relay request failed"
            };
            return (
                StatusCode::BAD_GATEWAY,
                serde_json::json!({"error": msg}).to_string(),
            )
                .into_response();
        }
    };

    // Build response — stream the body
    let status =
        StatusCode::from_u16(upstream_resp.status().as_u16()).unwrap_or(StatusCode::BAD_GATEWAY);

    let mut response_headers = HeaderMap::new();
    for &name in FORWARD_RESPONSE_HEADERS {
        if let Ok(hname) = HeaderName::from_bytes(name.as_bytes()) {
            if let Some(val) = upstream_resp.headers().get(name) {
                if let Ok(hval) = HeaderValue::from_bytes(val.as_bytes()) {
                    response_headers.insert(hname, hval);
                }
            }
        }
    }

    // Stream the response body
    let body_stream = upstream_resp
        .bytes_stream()
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e));
    let body = Body::from_stream(body_stream);

    let mut response = Response::new(body);
    *response.status_mut() = status;
    *response.headers_mut() = response_headers;

    response
}

/// Check if a path looks like a streaming request (longer timeout).
fn is_streaming_path(path: &str) -> bool {
    path.contains("/stream/")
        || path.contains("/direct/")
        || path.contains("/hls/")
        || path.contains("/transcode/")
}
