use std::net::IpAddr;
use std::sync::Arc;
use std::time::Instant;

use axum::extract::{ConnectInfo, Path, State};
use axum::http::{HeaderMap, StatusCode};
use axum::response::IntoResponse;
use axum::Json;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::db::UpsertResult;
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

    // Persist server name to SQLite
    match state
        .db
        .upsert_server_name(&body.server_name, &body.server_id.to_string())
        .await
    {
        Ok(UpsertResult::Conflict(existing_id)) => {
            return (
                StatusCode::CONFLICT,
                Json(serde_json::json!({
                    "error": "server name already claimed by another server",
                    "existing_server_id": existing_id
                })),
            )
                .into_response();
        }
        Err(e) => {
            tracing::error!(error = %e, "failed to persist server name to SQLite");
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({"error": "database error"})),
            )
                .into_response();
        }
        Ok(UpsertResult::Created) | Ok(UpsertResult::Updated) => {}
    }

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
    pub code: Option<String>,           // if provided, use this code; if None, generate 4-char (legacy)
    pub claim_type: Option<String>,     // "invite" or "device", defaults to "device"
    pub invite_code: Option<String>,    // pass-through for invite-type claims
    pub ttl_secs: Option<u64>,          // override TTL (invites may live longer)
    // Keep client_token as optional for backward compat with legacy flow
    pub client_token: Option<Uuid>,
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

    // Use server-provided code or generate a 4-char code (legacy)
    let code = match body.code {
        Some(ref c) => c.to_uppercase(),
        None => state.generate_claim_code(),
    };

    let ttl_secs = body.ttl_secs.unwrap_or(state.claim_ttl.as_secs());
    let claim_type = body.claim_type.unwrap_or_else(|| "device".to_string());

    state.claims.insert(
        code.clone(),
        crate::state::PendingClaim {
            server_id: body.server_id,
            client_token: body.client_token,
            expires_at: Instant::now() + std::time::Duration::from_secs(ttl_secs),
            claim_type: claim_type.clone(),
            invite_code: body.invite_code.clone(),
        },
    );

    tracing::info!(server_id = %body.server_id, %code, %claim_type, "claim code created");

    Json(CreateClaimResponse {
        code,
        expires_in_secs: ttl_secs,
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
    version: String,
    claim_type: String,                  // "invite" or "device"
    #[serde(skip_serializing_if = "Option::is_none")]
    invite_code: Option<String>,         // present if claim_type == "invite"
    #[serde(skip_serializing_if = "Option::is_none")]
    client_token: Option<Uuid>,          // present for legacy device claims
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

    tracing::info!(server_id = %claim.server_id, %code, claim_type = %claim.claim_type, "claim redeemed");

    Json(RedeemClaimResponse {
        server_id: server.server_id,
        server_name: server.server_name.clone(),
        public_ip: server.public_ip,
        local_ips: server.local_ips.clone(),
        port: server.port,
        version: server.version.clone(),
        claim_type: claim.claim_type,
        invite_code: claim.invite_code,
        client_token: claim.client_token,
    })
    .into_response()
}

// ── Server name lookup ───────────────────────────────────────────────────────

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct LookupByNameResponse {
    server_id: Uuid,
    server_name: String,
    public_ip: IpAddr,
    local_ips: Vec<IpAddr>,
    port: u16,
}

pub async fn lookup_by_name(
    State(state): State<Arc<BootstrapState>>,
    Path(name): Path<String>,
) -> impl IntoResponse {
    let name_lower = name.to_lowercase();

    // Search the live servers DashMap for a matching server name
    let found = state.servers.iter().find_map(|entry| {
        if entry.server_name.to_lowercase() == name_lower {
            Some(LookupByNameResponse {
                server_id: entry.server_id,
                server_name: entry.server_name.clone(),
                public_ip: entry.public_ip,
                local_ips: entry.local_ips.clone(),
                port: entry.port,
            })
        } else {
            None
        }
    });

    match found {
        Some(resp) => {
            tracing::debug!(name = %name, server_id = %resp.server_id, "server name lookup hit");
            Json(resp).into_response()
        }
        None => {
            (
                StatusCode::NOT_FOUND,
                Json(serde_json::json!({"error": "server not found"})),
            )
                .into_response()
        }
    }
}

// ── Server name registration (BIP39 recovery) ───────────────────────────────

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RegisterNameRequest {
    pub server_id: Uuid,
    pub server_name: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct RegisterNameResponse {
    server_name: String,
    recovery_phrase: String,
}

/// Generate a 12-word BIP39 mnemonic and return the phrase + its SHA-256 hash.
fn generate_recovery_phrase() -> Result<(String, String), String> {
    use bip39::Mnemonic;
    use rand::RngCore;
    use sha2::{Digest, Sha256};

    let mut entropy = [0u8; 16];
    rand::rng().fill_bytes(&mut entropy);
    let mnemonic = Mnemonic::from_entropy(&entropy)
        .map_err(|e| format!("failed to generate mnemonic: {e}"))?;
    let phrase = mnemonic.to_string();
    let hash = Sha256::digest(phrase.as_bytes());
    let hex_hash = format!("{hash:x}");
    Ok((phrase, hex_hash))
}

/// Hash a recovery phrase with SHA-256 and return the hex string.
fn hash_phrase(phrase: &str) -> String {
    use sha2::{Digest, Sha256};
    let hash = Sha256::digest(phrase.as_bytes());
    format!("{hash:x}")
}

pub async fn register_name(
    State(state): State<Arc<BootstrapState>>,
    headers: HeaderMap,
    Json(body): Json<RegisterNameRequest>,
) -> impl IntoResponse {
    if !validate_token(&headers, &state.bootstrap_token) {
        return (StatusCode::UNAUTHORIZED, Json(serde_json::json!({"error": "invalid token"}))).into_response();
    }

    let server_name = body.server_name.trim().to_string();
    if server_name.is_empty() {
        return (StatusCode::BAD_REQUEST, Json(serde_json::json!({"error": "server_name is required"}))).into_response();
    }

    // Check if name already exists
    let existing_owner = match state.db.lookup_by_name(&server_name).await {
        Ok(owner) => owner,
        Err(e) => {
            tracing::error!(error = %e, "failed to query server name");
            return (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({"error": "database error"}))).into_response();
        }
    };

    let server_id_str = body.server_id.to_string();

    match existing_owner {
        Some(ref owner) if owner != &server_id_str => {
            return (
                StatusCode::CONFLICT,
                Json(serde_json::json!({"error": "server name already claimed by another server"})),
            )
                .into_response();
        }
        None => {
            if let Err(e) = state.db.upsert_server_name(&server_name, &server_id_str).await {
                tracing::error!(error = %e, "failed to upsert server name");
                return (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({"error": "database error"}))).into_response();
            }
        }
        Some(_) => {
            // Owned by this server — regenerate the recovery phrase
        }
    }

    let (phrase, hex_hash) = match generate_recovery_phrase() {
        Ok(v) => v,
        Err(e) => {
            tracing::error!(error = %e, "failed to generate recovery phrase");
            return (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({"error": "failed to generate recovery phrase"}))).into_response();
        }
    };

    if let Err(e) = state.db.set_recovery_key_hash(&server_name, &hex_hash).await {
        tracing::error!(error = %e, "failed to store recovery key hash");
        return (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({"error": "database error"}))).into_response();
    }

    tracing::info!(%server_name, server_id = %body.server_id, "server name registered with recovery phrase");

    Json(RegisterNameResponse {
        server_name,
        recovery_phrase: phrase,
    })
    .into_response()
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RecoverNameRequest {
    pub server_name: String,
    pub recovery_phrase: String,
    pub new_server_id: Uuid,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct RecoverNameResponse {
    server_name: String,
    recovery_phrase: String,
}

pub async fn recover_name(
    State(state): State<Arc<BootstrapState>>,
    headers: HeaderMap,
    Json(body): Json<RecoverNameRequest>,
) -> impl IntoResponse {
    if !validate_token(&headers, &state.bootstrap_token) {
        return (StatusCode::UNAUTHORIZED, Json(serde_json::json!({"error": "invalid token"}))).into_response();
    }

    let server_name = body.server_name.trim().to_string();

    let stored_hash = match state.db.get_recovery_key_hash(&server_name).await {
        Ok(Some(hash)) => hash,
        Ok(None) => {
            return (
                StatusCode::NOT_FOUND,
                Json(serde_json::json!({"error": "server name not found or no recovery key set"})),
            )
                .into_response();
        }
        Err(e) => {
            tracing::error!(error = %e, "failed to query recovery key hash");
            return (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({"error": "database error"}))).into_response();
        }
    };

    let provided_hash = hash_phrase(body.recovery_phrase.trim());
    if provided_hash != stored_hash {
        return (
            StatusCode::FORBIDDEN,
            Json(serde_json::json!({"error": "invalid recovery phrase"})),
        )
            .into_response();
    }

    let new_server_id_str = body.new_server_id.to_string();
    if let Err(e) = state.db.transfer_server_name(&server_name, &new_server_id_str).await {
        tracing::error!(error = %e, "failed to transfer server name");
        return (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({"error": "database error"}))).into_response();
    }

    // Rotate recovery phrase after successful recovery
    let (new_phrase, new_hex_hash) = match generate_recovery_phrase() {
        Ok(v) => v,
        Err(e) => {
            tracing::error!(error = %e, "failed to generate new recovery phrase");
            return (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({"error": "failed to generate recovery phrase"}))).into_response();
        }
    };

    if let Err(e) = state.db.set_recovery_key_hash(&server_name, &new_hex_hash).await {
        tracing::error!(error = %e, "failed to store new recovery key hash");
        return (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({"error": "database error"}))).into_response();
    }

    tracing::info!(%server_name, new_server_id = %body.new_server_id, "server name recovered and transferred");

    Json(RecoverNameResponse {
        server_name,
        recovery_phrase: new_phrase,
    })
    .into_response()
}

// ── Name availability check ──────────────────────────────────────────────────

pub async fn check_name(
    State(state): State<Arc<BootstrapState>>,
    Path(name): Path<String>,
) -> impl IntoResponse {
    let name_lower = name.to_lowercase();

    // Check SQLite for the name
    let owner = state.db.lookup_by_name(&name).await.unwrap_or(None);

    match owner {
        Some(server_id) => {
            Json(serde_json::json!({
                "available": false,
                "ownedByServer": server_id,
            }))
                .into_response()
        }
        None => {
            // Also check live DashMap in case the name is registered but not yet in SQLite
            let live_owner = state.servers.iter().find_map(|entry| {
                if entry.server_name.to_lowercase() == name_lower {
                    Some(entry.server_id.to_string())
                } else {
                    None
                }
            });

            match live_owner {
                Some(server_id) => {
                    Json(serde_json::json!({
                        "available": false,
                        "ownedByServer": server_id,
                    }))
                        .into_response()
                }
                None => {
                    Json(serde_json::json!({
                        "available": true,
                    }))
                        .into_response()
                }
            }
        }
    }
}

// ── Port forward check ──────────────────────────────────────────────────────

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CheckPortRequest {
    pub server_id: Uuid,
}

pub async fn check_port(
    State(state): State<Arc<BootstrapState>>,
    headers: HeaderMap,
    Json(body): Json<CheckPortRequest>,
) -> impl IntoResponse {
    if !validate_token(&headers, &state.bootstrap_token) {
        return (StatusCode::UNAUTHORIZED, Json(serde_json::json!({"error": "invalid token"}))).into_response();
    }

    let server = match state.servers.get(&body.server_id) {
        Some(s) => s,
        None => {
            return (
                StatusCode::NOT_FOUND,
                Json(serde_json::json!({"error": "server not registered"})),
            )
                .into_response();
        }
    };

    let public_ip = server.public_ip;
    let port = server.port;
    drop(server); // Release DashMap ref before making HTTP call

    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(5))
        .build()
        .unwrap_or_else(|_| reqwest::Client::new());

    let url = format!("http://{public_ip}:{port}/api/v1/system/status");
    let start = Instant::now();

    match client.get(&url).send().await {
        Ok(resp) => {
            let latency_ms = start.elapsed().as_millis() as u64;
            let status_code = resp.status().as_u16();
            Json(serde_json::json!({
                "reachable": true,
                "publicIp": public_ip.to_string(),
                "port": port,
                "latencyMs": latency_ms,
                "statusCode": status_code,
            }))
                .into_response()
        }
        Err(e) => {
            Json(serde_json::json!({
                "reachable": false,
                "publicIp": public_ip.to_string(),
                "port": port,
                "error": e.to_string(),
            }))
                .into_response()
        }
    }
}

// ── Health ───────────────────────────────────────────────────────────────────

pub async fn health(State(state): State<Arc<BootstrapState>>) -> Json<serde_json::Value> {
    Json(serde_json::json!({
        "status": "ok",
        "servers": state.servers.len(),
        "pending_claims": state.claims.len(),
    }))
}
