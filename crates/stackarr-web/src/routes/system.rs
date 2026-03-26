use std::sync::Arc;

use axum::extract::State;
use axum::routing::{get, post};
use axum::{Json, Router};
use serde::{Deserialize, Serialize};

use crate::AppState;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct StatusResponse {
    version: &'static str,
    instance_name: String,
    is_setup: bool,
}

async fn get_status(State(state): State<Arc<AppState>>) -> Json<StatusResponse> {
    let cfg = state.config.load();
    Json(StatusResponse {
        version: env!("CARGO_PKG_VERSION"),
        instance_name: cfg.general.instance_name.clone(),
        is_setup: true,
    })
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
#[allow(dead_code)]
struct InitRequest {
    instance_name: Option<String>,
    database_url: Option<String>,
}

#[derive(Serialize)]
struct InitResponse {
    success: bool,
    message: String,
}

async fn init_setup(
    State(_state): State<Arc<AppState>>,
    Json(_body): Json<InitRequest>,
) -> Json<InitResponse> {
    // TODO: first-boot wizard: persist enabled_modules, write config file,
    // run migrations if needed.
    Json(InitResponse {
        success: true,
        message: "Setup complete".to_string(),
    })
}

pub fn router() -> Router<Arc<AppState>> {
    Router::new()
        .route("/api/v1/system/status", get(get_status))
        .route("/api/v1/setup/init", post(init_setup))
}
