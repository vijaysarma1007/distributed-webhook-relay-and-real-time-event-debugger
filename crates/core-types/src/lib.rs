//! # Core types crate

use serde::{Deserialize, Serialize};
use uuid::Uuid;
use chrono::{ DateTime , Utc};
/// Represents the lifecycle status of an outgoing delivery request.
///
/// # Delivery Status Enum
///
/// Variants are serialized and deserialized in `SCREAMING_SNAKE_CASE`
/// (e.g., `"PENDING"`, `"DELIVERED"`, `"FAILED"`, `"DEAD_LETTER"`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum DeliveryStatus {
    /// The update request is queued or currently being processed.
    Pending,

    /// The update request completed and was acknowledged successfully (e.g., HTTP 2xx).
    Delivered,

    /// The recipient server returned a client or server error (e.g., HTTP 400, 401, 500).
    Failed,

    /// The request failed permanently and will not be retried (e.g., invalid destination address or HTTP 404).
    DeadLetter,
}

/// Reprsents the Data Structure for Incoming Webhook request or from origin.
/// 
/// # Ingest Webhook Request Struct
/// 
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IngestWebhookRequest {
    /// Reciever or client's endpoint in String
    pub destination_url: String,

    /// Payload value that needs to sent to client's environment
    pub payload: serde_json::Value,

    /// secret key is shared between client and server
    pub secret_key: String
}

/// Represents the Data structure for outgoing request to the client's server.
/// 
/// # Out Box Event Struct
/// 
/// varaints in this struct are added as a `"db"` feature with `"sqlx"` as its dependency.
/// `derive(sqlx::FromRow)` automatically generates the code needed to convert a raw database row into rust struct.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "db", derive(sqlx::FromRow))]
pub struct OutBoxEvent {
    /// Generates uniqe id using Uuid crate.
    pub id: Uuid,

    /// url end point of client's server.
    pub destination_url: String,

    /// Payload or data that needs to be sent to client's server.
    pub payload: serde_json::Value,

    /// Secret key required to validate the request.
    pub secret_key: String,

    /// Delivery status with respect to request sent to client's server (e.g., `"PENDING"`, `"DELIVERED"`, `"FAILED"`, `"DEAD_LETTER"`).
    pub status: DeliveryStatus,

    /// Set the current attempt count.
    pub attempt_count: i32,

    /// Set the maximum number of attempts, before setting the status to `DeliveryStatus::DEAD_LETTER`.
    pub max_attempt: i32,

    /// Set the time in milliseconds for next try.
    pub next_retry_at: DateTime<Utc>,

    /// Set the created time in miliseconds.
    pub created_at : DateTime<Utc>
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LiveEventNotification {
    pub attempt_id: Uuid,
    pub event_id: Uuid,
    pub http_status: Option<i32>,
    pub latency_ms: i32,
    pub desitnation_url: String,
    pub attempted_at: DateTime<Utc>
}