use std::sync::Arc;

use arc_swap::ArcSwap;
use stackarr_core::config::{AppConfig, EnabledModules};
use stackarr_core::db::Database;

/// Shared application state available to all request handlers.
#[derive(Clone)]
pub struct AppState {
    pub db: Database,
    pub config: Arc<ArcSwap<AppConfig>>,
    pub modules: EnabledModules,
}
