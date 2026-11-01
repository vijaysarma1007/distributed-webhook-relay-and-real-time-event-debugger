use crate::AppState;
use axum::{
    extract::State,
    http::StatusCode,
    response::{IntoResponse, Response},
    routing::post,
    Json, Router,
};
use core_types::{DeliveryStatus, IngestWebhookRequest, OutboxEventRequest};
use crypto_signer::hashing::SecureSecret;
use serde::Serialize;
use uuid::Uuid;

#[derive(Debug, Serialize)]
struct InsertOutboxEventResponseSuccess {
    status: String,
    event_id: Uuid,
}

#[derive(Debug, Serialize)]
struct InsertOutboxEventResponseError {
    error: String,
}

pub fn handle_events(state: AppState) -> Router {
    Router::new()
        .route("/events", post(handle_ingest))
        .with_state(state)
}

async fn handle_ingest(
    State(state): State<AppState>,
    Json(payload): Json<IngestWebhookRequest>,
) -> Response {
    let secret_phrase = state.secret_phrase.as_str();
    let secure_secret = SecureSecret::new(secret_phrase);

    let encrypted_secure = match secure_secret.encrypt(secret_phrase) {
        Ok(enc) => enc,
        Err(err) => {
            let err_res = InsertOutboxEventResponseError {
                error: format!("Failed encrypting secret phrase: {err}"),
            };
            return (StatusCode::INTERNAL_SERVER_ERROR, Json(err_res)).into_response();
        }
    };

    let event = OutboxEventRequest {
        destination_url: payload.destination_url,
        payload: payload.payload,
        encrypted_secret: encrypted_secure,
    };

    match state.store.insert_outbox_events(event).await {
        Ok(event_id) => {
            let res = InsertOutboxEventResponseSuccess {
                status: DeliveryStatus::Pending.into(),
                event_id,
            };
            (StatusCode::OK, Json(res)).into_response()
        }
        Err(err) => {
            let err_res = InsertOutboxEventResponseError {
                error: err.to_string(),
            };
            (StatusCode::INTERNAL_SERVER_ERROR, Json(err_res)).into_response()
        }
    }
}
