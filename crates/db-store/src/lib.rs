//! #DB Store Library Crate
//!
pub mod clientconnect;
pub mod config;
pub mod dbconnect;
pub mod helper;

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

/// impl [`DbStore`] struct of type [`Transaction<'_, Postgres>`], exposing the methods avaliable while dispatching the events from pending events in multi threaded format
impl DbStore<Transaction<'_, Postgres>> {
    /// returns the pool of type [`Transaction<'_, Postgres>`]
    pub fn pool(&self) -> &Transaction<'_, Postgres> {
        &self.pool
    }

    ///
    /// # Insert Delivery attempt
    ///
    /// This asynchronous function inserts a new record into delivery_attempts table for every succesful or failed request to the client's server
    ///
    /// # Arguments
    /// * `attempt_id` - [`Uuid`] a new unique id for the new delivery attempt
    /// * `event` - [`&DispatchResult`] struct contains response from every client request
    /// * `event_id` -[Uuid] unique id of the pending event from 'outbox_events' table
    ///
    /// # Returns
    /// Returns an [`anyhow::Result`] with unit type
    ///
    pub async fn insert_delivery_attempt(
        &mut self,
        attempt_id: Uuid,
        event: &DispatchResult,
        event_id: Uuid,
    ) -> Result<()> {
        // extracts the status from the response or defaults with 500 status code
        let status = event.http_status.unwrap_or(500);
        // inserts the entry in the 'delivery_attempts' table
        let insert_attempt = sqlx::query!(
            r#"
                INSERT INTO delivery_attempts (
                    id, event_id, http_status, response_body, latency_ms, error_message, attempted_at
                )
                VALUES ($1, $2, $3, $4, $5, $6, NOW())
            "#,
            attempt_id,
            event_id,
            status,
            event.response_body,
            event.latency_ms,
            event.error_message
        )
        .execute(&mut *self.pool)
        .await;
        // returns an error if fails to register a delivery attempt.
        if let Err(e) = insert_attempt {
            // sets the error message
            let error_message = format!(
                "Failed to record delivery attempt for event {}: {}",
                event_id, e
            );
            // logs the error message in the terminal
            error!(error_message);
            // returns early with anyhow::Result::Err
            bail!(error_message);
        }
        // for succesful registery of dalivery attempt returns a anyhow::Result::Ok with no data.
        Ok(())
    }

    ///
    /// # update outbox event status
    ///
    /// This asynchronous function is used to update the status, attempt count of a particular outbox event based on the succesful or failed delivery to/from the client's server.
    ///
    /// # Arguments
    /// * `event_id` - [`Uuid`] unique id of the document from outbox event that needs to be updated.
    /// * `new_attempt_count` -[`i32`] updates the attempt count with an increment for every failed response from the client server, while checking the maximum allowed attempts in check
    /// * `status` - [`DeliveryStatus`] enum, accepts the delivery enum that the event needs to be updated with.
    ///
    /// # Returns
    /// Returns a [`anyhow::Result`] with unit or no return types.
    ///
    pub async fn update_outbox_events_status(
        &mut self,
        event_id: Uuid,
        new_attempt_count: i32,
        status: DeliveryStatus,
    ) -> Result<()> {
        // converts the status enum to corresponding String.
        let status: String = status.into();
        // updates the outbox_events table with specfic event id with latest status and incremented attempt count
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
        // catch for any error while updating the outbox events table
        if let Err(e) = update_outbox {
            // format the error message with event id and error message
            let error_message = format!(
                "Failed to record delivery attempt for event {}: {}",
                event_id, e
            );
            // log the error message in the terminal
            error!(error_message);
            // return early with error message
            bail!(error_message);
        };
        // return anyhow::Result with unit data
        Ok(())
    }

    ///
    /// # update outbox events retry
    ///
    /// This asynchronous function updates the outbox events with specific id with next retry time provided it is within the maximum allowed retry limit number.
    ///
    /// # Arguments
    /// * `event_id` - [`Uuid`] unique id for the event from outbox events table which needs to be updated
    /// * `new_attempt_count` -[`i32`] incremented count for retry, if the earlier delivery attempt has failed.
    /// * `backoff_seconds` - [`i64`] time in milliseconds which will be used to calculate the next retry.
    ///
    /// # Return
    /// returns a anyhow::Result of unit type
    ///
    pub async fn update_outbox_events_retry(
        &mut self,
        event_id: Uuid,
        new_attempt_count: i32,
        backoff_seconds: i64,
    ) -> Result<()> {
        // This will exponentailly multiple 2 with a power, saturation power will keep a check on the maximum value of i64.
        let multiplier = 2_i64.saturating_pow((new_attempt_count - 1) as u32);
        // backoff seconds represent the base multipleir with exponential components.
        let delay_seconds = backoff_seconds.saturating_mul(multiplier);
        // adds the current time with retry time and adds up next_retry
        let next_retry = Utc::now() + ChronoDuration::seconds(delay_seconds);
        // logs the success info in the terminal
        info!(
            "Event {} attempt {} failed. Retrying at {} (delay: {}s)",
            event_id, new_attempt_count, next_retry, delay_seconds
        );
        // update the document with specific event id in outbox_events table with new attempt count and next_retry
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
        // executes the error code block if outbox_events table, if it fails to updates the row with specific event id
        if let Err(e) = update_retry {
            // format the error message
            let error_message = format!(
                "Failed to update outbox event retry for event id {} : {}",
                event_id, e
            );
            // logs the error message on terminal
            error!(error_message);
            // return early with an error message
            bail!(error_message);
        };
        // returns anyhow:Result with unit tuple
        Ok(())
    }
}
