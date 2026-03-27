use std::net::IpAddr;
use std::sync::Arc;
use std::time::Instant;

use axum::extract::{ConnectInfo, Path, State};
use axum::http::{HeaderMap, StatusCode};
use axum::response::IntoResponse;
use axum::Json;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::state::{BootstrapState, ServerRegistration};

// ── Auth helper ─────────────────────────────────────────────────────────────

fn validate_token(headers: &HeaderMap, expected: &str) -> bool {
    headers
        .get("authorization")
        .and_then(|v| v.to_str().ok())
        .and_then(|s| s.strip_prefix("Bearer "))
        .is_some_and(|token| token.trim() == expected)
}

// ── Server registration ─────────────────────────────────────────────────────

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RegisterRequest {
    pub server_id: Uuid,
    pub server_name: String,
    pub local_ips: Vec<IpAddr>,
    pub port: u16,
    pub version: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct RegisterResponse {
    public_ip: IpAddr,
    ttl_secs: u64,
}

/// Extract the real client IP, checking CF and proxy headers before falling back to ConnectInfo.
fn real_ip(headers: &HeaderMap, fallback: IpAddr) -> IpAddr {
    // Cloudflare sets this to the true client IP
    if let Some(val) = headers.get("cf-connecting-ip") {
        if let Ok(s) = val.to_str() {
            if let Ok(ip) = s.trim().parse::<IpAddr>() {
                return ip;
            }
        }
    }
    // Standard reverse proxy header
    if let Some(val) = headers.get("x-forwarded-for") {
        if let Ok(s) = val.to_str() {
            if let Some(first) = s.split(',').next() {
                if let Ok(ip) = first.trim().parse::<IpAddr>() {
                    return ip;
                }
            }
        }
    }
    fallback
}

pub async fn register_server(
    State(state): State<Arc<BootstrapState>>,
    ConnectInfo(addr): ConnectInfo<std::net::SocketAddr>,
    headers: HeaderMap,
    Json(body): Json<RegisterRequest>,
) -> impl IntoResponse {
    if !validate_token(&headers, &state.bootstrap_token) {
        return (StatusCode::UNAUTHORIZED, Json(serde_json::json!({"error": "invalid token"}))).into_response();
    }

    let public_ip = real_ip(&headers, addr.ip());
    let ttl_secs = state.registration_ttl.as_secs();

    state.servers.insert(
        body.server_id,
        ServerRegistration {
            server_id: body.server_id,
            server_name: body.server_name,
            public_ip,
            local_ips: body.local_ips,
            port: body.port,
            version: body.version,
            last_heartbeat: Instant::now(),
        },
    );

    tracing::debug!(server_id = %body.server_id, %public_ip, "server registered/heartbeat");

    Json(RegisterResponse { public_ip, ttl_secs }).into_response()
}

pub async fn deregister_server(
    State(state): State<Arc<BootstrapState>>,
    headers: HeaderMap,
    Path(server_id): Path<Uuid>,
) -> impl IntoResponse {
    if !validate_token(&headers, &state.bootstrap_token) {
        return StatusCode::UNAUTHORIZED.into_response();
    }

    state.servers.remove(&server_id);
    tracing::info!(%server_id, "server deregistered");
    StatusCode::NO_CONTENT.into_response()
}

// ── Claim codes ─────────────────────────────────────────────────────────────

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateClaimRequest {
    pub server_id: Uuid,
    pub client_token: Uuid,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct CreateClaimResponse {
    code: String,
    expires_in_secs: u64,
}

pub async fn create_claim(
    State(state): State<Arc<BootstrapState>>,
    headers: HeaderMap,
    Json(body): Json<CreateClaimRequest>,
) -> impl IntoResponse {
    if !validate_token(&headers, &state.bootstrap_token) {
        return (StatusCode::UNAUTHORIZED, Json(serde_json::json!({"error": "invalid token"}))).into_response();
    }

    // Verify server is registered
    if !state.servers.contains_key(&body.server_id) {
        return (
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({"error": "server not registered"})),
        )
            .into_response();
    }

    let code = state.generate_claim_code();
    let expires_in_secs = state.claim_ttl.as_secs();

    state.claims.insert(
        code.clone(),
        crate::state::PendingClaim {
            server_id: body.server_id,
            client_token: body.client_token,
            expires_at: Instant::now() + state.claim_ttl,
        },
    );

    tracing::info!(server_id = %body.server_id, %code, "claim code created");

    Json(CreateClaimResponse {
        code,
        expires_in_secs,
    })
    .into_response()
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct RedeemClaimResponse {
    server_id: Uuid,
    server_name: String,
    public_ip: IpAddr,
    local_ips: Vec<IpAddr>,
    port: u16,
    client_token: Uuid,
    version: String,
}

pub async fn redeem_claim(
    State(state): State<Arc<BootstrapState>>,
    Path(code): Path<String>,
) -> impl IntoResponse {
    let code = code.to_uppercase();

    // Remove and consume the claim (one-time use)
    let Some((_, claim)) = state.claims.remove(&code) else {
        return (
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({"error": "invalid or expired claim code"})),
        )
            .into_response();
    };

    // Check expiry
    if Instant::now() >= claim.expires_at {
        return (
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({"error": "claim code has expired"})),
        )
            .into_response();
    }

    // Look up the server
    let Some(server) = state.servers.get(&claim.server_id) else {
        return (
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({"error": "server is no longer registered"})),
        )
            .into_response();
    };

    tracing::info!(server_id = %claim.server_id, %code, "claim redeemed");

    Json(RedeemClaimResponse {
        server_id: server.server_id,
        server_name: server.server_name.clone(),
        public_ip: server.public_ip,
        local_ips: server.local_ips.clone(),
        port: server.port,
        client_token: claim.client_token,
        version: server.version.clone(),
    })
    .into_response()
}

// ── Health ───────────────────────────────────────────────────────────────────

pub async fn health(State(state): State<Arc<BootstrapState>>) -> Json<serde_json::Value> {
    Json(serde_json::json!({
        "status": "ok",
        "servers": state.servers.len(),
        "pending_claims": state.claims.len(),
    }))
}
