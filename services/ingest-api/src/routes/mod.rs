//!
//! # V1 Module
//!
//! nests all the `/api` with `/v1` path
//!
use axum::Router;

use crate::AppState;

pub(super) mod v1;

/// Handler to mount all the paths to `/v1`
pub fn v1events(state: AppState) -> Router {
    Router::new().nest("/v1", v1::events::handle_events(state))
}
