//! #DB Store Library Crate
//!
pub mod clientconnect;
pub mod config;
pub mod dbconnect;

use std::marker::PhantomData;

use anyhow::{bail, Context, Ok, Result};
use chrono::{Duration as ChronoDuration, Utc};
use core_types::{DeliveryStatus, DispatchResult, OutBoxEvent, OutboxEventRequest};
use sqlx::{self, PgPool, Postgres, Transaction};
use tracing::{error, info};
use uuid::Uuid;

///
/// DbStore Struct helps to create a new PgPool.
/// This is an asynchronous database pool for PostgreSQL, which is backed by Arc (Atomic reference counter) which does a create a dublicate which .clone() is called, instead it increments the internal counter.
///
#[derive(Clone)]
pub struct DbStore<T> {
    pub pool: T,
}

///
/// Impl DbStore, exposes methods for every new DbStore struct that has been created.
///
impl DbStore<PgPool> {
    /// create and return a new PgPool
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    /// Return the PgPool, created using new method.
    pub fn pool(&self) -> &PgPool {
        &self.pool
    }

    pub async fn begin(&self) -> Result<DbStore<Transaction<'_, Postgres>>> {
        let tx = self.pool.begin().await?;
        Ok(DbStore { pool: tx })
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
    pub async fn insert_outbox_events(&self, req: OutboxEventRequest) -> Result<Uuid> {
        let event_id = Uuid::new_v4();

        sqlx::query!(
            r#"INSERT INTO outbox_events (id, destination_url, payload, encrypted_secret)
             VALUES
             ($1, $2, $3, $4)
             "#,
            event_id,
            req.destination_url,
            req.payload,
            req.encrypted_secret
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
        sqlx::query_as!(OutBoxEvent, r#"SELECT id, destination_url, payload, encrypted_secret, status, attempt_count, max_attempts, next_retry_at, created_at
         FROM outbox_events
         WHERE status = $1 AND next_retry_at <= NOW()
         ORDER BY next_retry_at ASC
         LIMIT $2
         FOR UPDATE SKIP LOCKED
         "#, status, batch_size
        ).fetch_all(&self.pool).await.context("fetching outbox box event with status pending")
    }
}

impl DbStore<Transaction<'_, Postgres>> {
    pub fn pool(&self) -> &Transaction<'_, Postgres> {
        &self.pool
    }

    pub async fn insert_delivery_attempt(
        &mut self,
        attempt_id: Uuid,
        event: &DispatchResult,
        event_id: Uuid,
    ) -> Result<()> {
        let insert_attempt = sqlx::query!(
            r#"
                INSERT INTO delivery_attempts (
                    id, event_id, http_status, response_body, latency_ms, error_message, attempted_at
                )
                VALUES ($1, $2, $3, $4, $5, $6, NOW())
            "#,
            attempt_id,
            event_id,
            event.http_status,
            event.response_body,
            event.latency_ms,
            event.error_message
        )
        .execute(&mut *self.pool)
        .await;

        if let Err(e) = insert_attempt {
            let error_message = format!(
                "Failed to record delivery attempt for event {}: {}",
                event_id, e
            );
            error!(error_message);
            bail!(error_message);
        }
        Ok(())
    }

    pub async fn update_outbox_events_status(
        &mut self,
        event_id: Uuid,
        new_attempt_count: i32,
        status: DeliveryStatus,
    ) -> Result<()> {
        let status: String = status.into();
        let update_outbox = sqlx::query!(
            r#"
                    UPDATE outbox_events
                    SET status = $3,
                        attempt_count = $2
                    WHERE id = $1
                    "#,
            event_id,
            new_attempt_count,
            status
        )
        .execute(&mut *self.pool)
        .await;

        if let Err(e) = update_outbox {
            let error_message = format!(
                "Failed to record delivery attempt for event {}: {}",
                event_id, e
            );
            error!(error_message);
            bail!(error_message);
        };

        Ok(())
    }

    pub async fn update_outbox_events_retry(
        &mut self,
        event_id: Uuid,
        new_attempt_count: i32,
        backoff_seconds: i64,
    ) -> Result<()> {
        let multiplier = 2_i64.saturating_pow((new_attempt_count - 1) as u32);
        let delay_seconds = backoff_seconds.saturating_mul(multiplier);
        let next_retry = Utc::now() + ChronoDuration::seconds(delay_seconds);

        info!(
            "Event {} attempt {} failed. Retrying at {} (delay: {}s)",
            event_id, new_attempt_count, next_retry, delay_seconds
        );

        let update_retry = sqlx::query!(
            r#"
                    UPDATE outbox_events
                    SET attempt_count = $2,
                        next_retry_at = $3
                    WHERE id = $1
                    "#,
            event_id,
            new_attempt_count,
            next_retry
        )
        .execute(&mut *self.pool)
        .await;

        if let Err(e) = update_retry {
            let error_message = format!(
                "Failed to update outbox event retry for event id {} : {}",
                event_id, e
            );
            error!(error_message);
            bail!(error_message);
        };
        Ok(())
    }
}
