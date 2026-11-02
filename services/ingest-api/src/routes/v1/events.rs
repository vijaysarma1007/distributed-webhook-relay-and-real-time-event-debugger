//!
//! # Events handler
//!
//! Contains the route and the handler to create a new ingest event and save in the outbox events table in postgres.
//!
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

///
/// # Insert Outbox Event Response Success Struct
///
/// returns this struct on successful creation of the event, which contains the event id and the status , which is default to `PENDING`.
///
#[derive(Debug, Serialize)]
struct InsertOutboxEventResponseSuccess {
    status: String,
    event_id: Uuid,
}

///
/// # Insert Outbox Event Response Error Struct
///
/// returns this struct if failed to create an event, which contains error in String format.
///
#[derive(Debug, Serialize)]
struct InsertOutboxEventResponseError {
    error: String,
}

///
/// binds the appropriate handler with the path.
///
/// # Arguments
/// * `state` - accepts the AppState Struct
///
/// # Returns
/// Returns a [`Router`] Struct
///
pub fn handle_events(state: AppState) -> Router {
    Router::new()
        .route("/events", post(handle_ingest))
        .with_state(state)
}

///
/// # Handle Ingest handler
///
/// # Arguments
/// * `state` - accepts the AppState Struct.
/// * `payload` - accepts the IngestWebHookRequest struct which extracts into Json using serde::serialize.
///
/// # Returns
/// Returns a Response which implements IntoResponse Trait.
///
async fn handle_ingest(
    State(state): State<AppState>,
    Json(payload): Json<IngestWebhookRequest>,
) -> Response {
    // Extracts secret phrase from state (AppState struct) as String ands converts into &str.
    let secret_phrase = state.secret_phrase.as_str();
    // Create a new SecureSecret struct which accepts secret phrase.
    let secure_secret = SecureSecret::new(secret_phrase);

    // Combines with secret phrase with client's secret to encrypt the client's secret, returns a Vec<u8> on success and error on fails.
    let encrypted_secure = match secure_secret.encrypt(secret_phrase) {
        Ok(enc) => enc,
        Err(err) => {
            let err_res = InsertOutboxEventResponseError {
                error: format!("Failed encrypting secret phrase: {err}"),
            };
            return (StatusCode::INTERNAL_SERVER_ERROR, Json(err_res)).into_response();
        }
    };

    // Converts IngestWebhookRequest struct to OutboxEventRequest struct which is  required to send to backend as backend Db accepts encrypted_secret from String to Vec<u8>.
    let event = OutboxEventRequest {
        destination_url: payload.destination_url,
        payload: payload.payload,
        encrypted_secret: encrypted_secure,
    };

    // Sends the out box event struct as request (req) amd awaits for the response, returns a OK with event id on success and Error if fails.
    match state.store.insert_outbox_events(event).await {
        Ok(event_id) => {
            // on success, creates an insert outbox event response success struct which coontains status set to `PENDING` as DeliveryStatus enum as default , returns a tuple with Ok Statuscode from axum and response in JSON which is serialised using serde::Serialize.
            let res = InsertOutboxEventResponseSuccess {
                status: DeliveryStatus::Pending.into(),
                event_id,
            };
            (StatusCode::OK, Json(res)).into_response()
        }
        Err(err) => {
            // on failure, creates an insert outbox event response error struct which contains error message as String, and returns a tuple with Internal Server Error statuscode from axum and response body in json which is serailized from serde::Serialize
            let err_res = InsertOutboxEventResponseError {
                error: err.to_string(),
            };
            (StatusCode::INTERNAL_SERVER_ERROR, Json(err_res)).into_response()
        }
    }
}
