//! # Dispatch cycle
//!
//! This function is used to dipatch the pending events from outbox_events table to the client server, and updates the outbox_events or delivery_ateempts table as per response from client.
//!
//!

use super::dispatcher::execute_delivery;
use crate::config::{BASE_BACKOFF_SECONDS, BATCH_SIZE};
use anyhow::{Context, Result};
use chrono::Utc;
use core_types::{DeliveryStatus, OutBoxEvent};
use crypto_signer::hashing::SecureSecret;
use db_store::DbStore;
use sqlx::PgPool;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::Semaphore;
use tracing::{error, info, warn};
use uuid::Uuid;
///
/// # Run dipsatch cycle
///
/// This function is used to run the entire dispatch cycle from dispatch worker service.
///
/// # Arguments
/// * `pool` - [`PgPool`] A succesful postgres connection object struct.
/// * `secure_secret` - [`&Arc<SecureSecret>`] - Atomic reference counter to Secure Secret  object struct.
/// * `semaphore` - [`&Arc<Semaphore>`] - Atmoic reference counter to semaphore or gate checker object struct.
///
/// # Returns
/// [`anyhow::Result`] - returns an anyhow result with no value.
///
pub async fn run_dispatch_cycle(
    pool: PgPool,
    secure_secret: &Arc<SecureSecret>,
    semaphore: &Arc<Semaphore>,
) -> Result<()> {
    // create a new db store from the existing pstgres pool.
    let db_store = DbStore::<PgPool>::new(pool);
    // fetch the events form outbox_events table with status: 'PENDING'
    match fetch_and_lock_pending_events(&db_store, BATCH_SIZE).await {
        // if the fetched events is an empty vec or of zero size then sleep the server for a while.
        Ok(events) if events.is_empty() => {
            tokio::time::sleep(Duration::from_millis(500)).await;
        }
        // executes the below block of code if fetched events is not empty.
        Ok(events) => {
            // for every event fetched 'JoinSet' allows to spawn a sperate async task for each eveent to get processed in parallel. it doesn't create a new thread instead it uses tokio runtime which will automatically executes parallel event when the other event is still awaiting for a response. This method uses minimum CPU usage.
            let mut join_set = tokio::task::JoinSet::new();
            // loop over each event
            for event in events {
                // check if event has a permit , if not return an error, it loops back if in case number of events exceeds the semaphore limits.
                let permit = match semaphore.clone().acquire_owned().await {
                    Ok(p) => p,
                    Err(_) => break,
                };

                // Clone these inside the loop so they can be reused across iterations
                let secure_secret = secure_secret.clone();
                let store = db_store.clone();

                join_set.spawn(async move {
                    let _permit = permit;
                    process_single_event(&secure_secret, &event, store).await
                });
            }
            // join_set.join_next() is a tokio method that asynchronously waits for any single task within a JoinSet to finish executing, and then removes the task from the set.
            while let Some(res) = join_set.join_next().await {
                // catches any error for join set join.
                if let Err(err) = res {
                    error!("Dispatch worker task panicked or was cancelled: {err}");
                }
            }
        }
        // The error arm for fetch pending events from outbox_events table, catch the error if it fails to fetch the events.
        Err(err) => {
            error!("Database polling error: {:#?}", err);
            tokio::time::sleep(Duration::from_secs(2)).await;
        }
    }

    Ok(())
}

///
/// # Fetch and lock pending events
///
/// function to get all the events and lock the rows which are cosumed by backend for updation, this also elimiates any dublicate ownership of the row from postgres database.
///
/// # Arguments
/// * `db_store` - [`&DbStore<PgPool>`] accepts a struct type of DbStore of type PgPool
/// * batch_size - [`i64`] accepts the batch size or number of events that needs to fetched in each call.
///
/// # Returns
/// Returns a [`anyhow::Result`] with vec of [`OutBoxEvents`] struct on success.
///
async fn fetch_and_lock_pending_events(
    db_store: &DbStore<PgPool>,
    batch_size: i64,
) -> Result<Vec<OutBoxEvent>> {
    let events = db_store
        .fetch_pending_events(batch_size)
        .await
        .context("Fetching and locking all the pending events")?;
    Ok(events)
}

///
/// # process single event
///
/// This function will proces single event fetched by fetch pending eveents function
///
/// # Arguments
/// * `secure_secret` - [`&Arc<SecureSecret>`] accepts a reference to secrue secret struct enclosed in Arc.
/// * `event` - [`&OutBoxEvent`] - accepts a outbox event which contains the payload for each event.
/// & `store` - [`DbStore<PgPool>`] - accepts a DbStore struct of sub type Pg Pool
///
/// # Returns
/// None
async fn process_single_event(
    secure_secret: &Arc<SecureSecret>,
    event: &OutBoxEvent,
    store: DbStore<PgPool>,
) {
    // executes each delivery
    let result = execute_delivery(secure_secret, event).await;
    // create a new delivery attemp unique id
    let attempt_id = Uuid::new_v4();
    // increment the new_attempt_count from events [`OutBoxEvent`] by 1.
    let new_attempt_count = event.attempt_count + 1;

    // This piece of code is important for executing concurrent writes in 2 different tables in database.
    // This code checks out a database connection from the PgPool ans starts an atomic PostgresSQL transaction.
    // Since this code block will conduct 2 seperate write across different tables at a time, we want the transaction to fail if either of the write in any table fails.
    // Example: we will be writing  into delivery_attemps table and outbox_events table, without transaction there might a chance of mismatch amongst the updates.
    let mut store = match store.begin().await {
        Ok(t) => t,
        Err(e) => {
            error!("Failed to open transaction for event {}: {}", event.id, e);
            return;
        }
    };

    info!("result from dipatch worker : {:?}", result);
    //Insert into delivery_attempts as error, if store fails to register a delivery attempt (via POstgresSQL pg_notify trigger)
    if let Err(e) = store
        .insert_delivery_attempt(attempt_id, &result, event.id)
        .await
        .context("Inserting in delivery_attemp table")
    {
        error!("Failed at inserting delivery attempt: {e}");
        return;
    }
    // get the current time
    let current_time = Utc::now();
    //Insert into outbox_events status and backoff schedule if the result is successful or delivery is executed
    if result.is_success {
        // if client server returns a response with status code 200 to 299, then set the status to delievred.
        let status = DeliveryStatus::Delivered;
        // try updating outbox event status in outbox_events table with delivered status
        if let Err(e) = store
            .update_outbox_events_status(event.id, new_attempt_count, status)
            .await
            .context("Updating outbox event status to delivered")
        {
            // catch if an error is generated while updating the outbox events.
            error!(
                "Error at updated status to delivered for outbox event {} :{}",
                event.id, e
            );
            // stops the transaction from store and returns early.
            let _ = &store.pool.rollback().await;
            return;
        }
    } else if (new_attempt_count >= event.max_attempts) || (current_time > event.next_retry_at) {
        // this code block will be executed if current number of attempts crosses maximum allowed attempts or current time is more than next retry
        warn!(
            "Event {} reached max atempts ({}/{}). Transitioning to DEAD_LETTER.",
            event.id, new_attempt_count, event.max_attempts
        );
        // changes the status to DEADLETTER (Terminal state)
        let status = DeliveryStatus::DeadLetter;
        // updates the outbox events with  new status.
        if let Err(e) = store
            .update_outbox_events_status(event.id, new_attempt_count, status)
            .await
            .context("Updating outbox event status to dead letter")
        {
            // catch if an error is generated while updating the outbox events.
            error!(
                "Error at updating status to dead letter for outbox event {} :{}",
                event.id, e
            );
            // stops the transaction from store and returns early.
            let _ = &store.pool.rollback().await;
            return;
        }
    } else {
        // if new attempt count is less than maximum allowed attempts then update the outbox event if a new next_try_at value calcuted using base backoff seconds.
        if let Err(e) = store
            .update_outbox_events_retry(event.id, new_attempt_count, BASE_BACKOFF_SECONDS)
            .await
            .context("updating retry for outbox event")
        {
            // catch if an error is generated while updating the outbox events.
            error!(
                "Error at updating next try for outbox event {} :{}",
                event.id, e
            );
            // stops the transaction from store and returns early.
            let _ = &store.pool.rollback().await;
            return;
        }
    };
    // try to commit the entire trasaction, closes succesfully if both the updates in outbox_events and delievry_attempts are successful, if either fails transaction will emit an error.
    if let Err(e) = &store.pool.commit().await {
        error!(
            "Failed to commit outbox update transaction for the event {}: {}",
            event.id, e
        );
    }
}
