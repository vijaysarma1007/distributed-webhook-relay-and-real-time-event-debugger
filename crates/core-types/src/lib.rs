//! # Core types crate
//!
//! High-level architecture overview.
//!
use anyhow::{Error, Result};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;
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

/// Into trait converts arms of DeliveryStatus enum into corresponsing string, used in db-store library crate.
/// 
#[allow(dead_code, unreachable_code, unused)]
impl Into<String> for DeliveryStatus {
    fn into(self) -> String {
        match self {
            Self::Pending => String::from("PENDING"),
            Self::DeadLetter => String::from("DEADLETTER"),
            Self::Failed => String::from("FAILED"),
            Self::Delivered => String::from("DELIVERED"),
            _ => Err(format!("Unknown database delivery status").to_owned()).unwrap(),
        }
    }
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
    pub secret_key: String,
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
    pub status: String,

    /// Set the current attempt count.
    pub attempt_count: i32,

    /// Set the maximum number of attempts, before setting the status to `DeliveryStatus::DEAD_LETTER`.
    pub max_attempts: i32,

    /// Set the time in milliseconds for next try.
    pub next_retry_at: DateTime<Utc>,

    /// Set the created time in miliseconds.
    pub created_at: DateTime<Utc>,
}

/// Represents the data structure for delivery attempt, every outbox event even if it is failed or passed shall be recorded here. This is immutable struct which has one to many relationship with outbox event.
///
/// # Delivery Attempt Struct
///
/// varaints in this struct are added as a `"db"` feature with `"sqlx"` as its dependency.
/// `derive(sqlx::FromRow)` automatically generates the code needed to convert a raw database row into rust struct
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "db", derive(sqlx::FromRow))]
pub struct DeliveryAttempt {
    ///Primary key for this specific execution
    pub id: Uuid,

    /// Foriegn key referencing for outbox event
    pub event_id: Uuid,

    /// status code with each attempt e.g: Some(200),Some(404) etc
    pub http_status: Option<i32>,

    /// The raw text or JSON returned by the external server, using for debugging
    pub response_body: Option<String>,

    /// Round trip duration in milliseconds
    pub latency_ms: i32,

    ///Populated when the client encounters an error e.g: connection error, bad request etc.
    pub error_message: Option<String>,

    /// Timestamp marking exactly when the network request was fired.
    pub attempted_at: DateTime<Utc>,
}

///This struct represents the date transfer object (DTO) designed for low overhead, real time streaming from PostgreSQL to debugger frontend ui. This struct represents the minimal metadata needed to display a live event row on the debugger dashbaord.
/// useful for bandwidth optimization.
///
/// # Live Event Notification Struct
///
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LiveEventNotification {
    /// primary key for each notification
    pub attempt_id: Uuid,

    /// Outbox unique id
    pub event_id: Uuid,

    /// status code e.g: 200 ,400, 404, 401, 500 etc.
    pub http_status: Option<i32>,

    /// Round trip duration in milliseconds
    pub latency_ms: i32,

    /// Destination url or end point of the client
    pub desitnation_url: String,

    /// Timestamp marking exactly when the network was fired.
    pub attempted_at: DateTime<Utc>,
}
