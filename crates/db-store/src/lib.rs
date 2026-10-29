//! #DB Store Library Crate
//!
use anyhow::{Context, Ok, Result};
use core_types::{DeliveryStatus, IngestWebhookRequest, OutBoxEvent};
use sqlx::{self, PgPool};
use uuid::Uuid;

///
/// DbStore Struct helps to create a new PgPool.
/// This is an asynchronous database pool for PostgreSQL, which is backed by Arc (Atomic reference counter) which does a create a dublicate which .clone() is called, instead it increments the internal counter.
///
#[derive(Clone)]
pub struct DbStore {
    pool: PgPool,
}

///
/// Impl DbStore, exposes methods for every new DbStore struct that has been created.
///
impl DbStore {
    /// create and return a new PgPool
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    /// Return the PgPool, created using new method.
    pub fn pool(&self) -> &PgPool {
        &self.pool
    }

    ///use this method to create a new outbox event.
    ///
    /// # Arguments
    ///
    /// * `req` - accepts `IngestWebhookRequest` struct to create a new outbox event document.
    ///
    /// # Returns
    ///
    /// Returns a [`anyhow::Result`] with either [`Ok(Uuid)`] or an [`Error`] with appropriate context.
    ///
    pub async fn insert_outbox_events(&self, req: IngestWebhookRequest) -> Result<Uuid> {
        let event_id = Uuid::new_v4();

        sqlx::query!(
            r#"INSERT INTO outbox_events (id, destination_url, payload, secret_key)
             VALUES
             ($1, $2, $3, $4)
             "#,
            event_id,
            req.destination_url,
            req.payload,
            req.secret_key
        )
        .execute(&self.pool)
        .await
        .context("Inserting into outbox events")?;

        Ok(event_id)
    }

    ///use this method to get all the `PENDING` outbox events, arranged in ascending order as per `next_retry_at` field.
    ///
    /// # Arguments
    ///
    /// * `batch_size` - accepts an integer of type i64, this is used to set the number of queries that needs to be fetched at each request. No default value is set.
    ///
    /// # Returns
    ///
    /// Returns a `Vec<OutBoxEvent>` containing the matched records, or a [`sqlx::Error`] with a anyhow context message, if it fails.
    pub async fn fetch_pending_events(&self, batch_size: i64) -> Result<Vec<OutBoxEvent>> {
        let status: String = DeliveryStatus::Pending.into();
        sqlx::query_as!(OutBoxEvent, r#"SELECT id, destination_url, payload, secret_key, status, attempt_count, max_attempts, next_retry_at, created_at
         FROM outbox_events
         WHERE status = $1 AND next_retry_at <= NOW()
         ORDER BY next_retry_at ASC
         LIMIT $2
         FOR UPDATE SKIP LOCKED
         "#, status, batch_size
        ).fetch_all(&self.pool).await.context("fetching outbox box event with status pending")
    }
}
