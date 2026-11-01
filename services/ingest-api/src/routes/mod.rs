use axum::Router;

use crate::AppState;

pub(super) mod v1;

pub fn v1events(state: AppState) -> Router {
    Router::new().nest("/v1", v1::events::handle_events(state))
}
