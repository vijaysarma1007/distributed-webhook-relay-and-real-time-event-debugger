use super::dispatcher::execute_delivery;
use crate::config::{BASE_BACKOFF_SECONDS, BATCH_SIZE};
use anyhow::{Context, Result};
use core_types::{DeliveryStatus, OutBoxEvent};
use crypto_signer::hashing::SecureSecret;
use db_store::DbStore;
use sqlx::PgPool;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::Semaphore;
use tracing::{error, warn};
use uuid::Uuid;

pub async fn run_dispatch_cycle(
    pool: PgPool,
    secure_secret: &Arc<SecureSecret>,
    semaphore: &Arc<Semaphore>,
) -> Result<()> {
    let db_store = DbStore::<PgPool>::new(pool);
    match fetch_and_lock_pending_events(&db_store, BATCH_SIZE).await {
        Ok(events) if events.is_empty() => {
            tokio::time::sleep(Duration::from_millis(500)).await;
        }
        Ok(events) => {
            let mut join_set = tokio::task::JoinSet::new();

            for event in events {
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

            while let Some(res) = join_set.join_next().await {
                if let Err(err) = res {
                    error!("Dispatch worker task panicked or was cancelled: {err}");
                }
            }
        }
        Err(err) => {
            error!("Database polling error: {:#?}", err);
            tokio::time::sleep(Duration::from_secs(2)).await;
        }
    }

    Ok(())
}

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

async fn process_single_event(
    secure_secret: &Arc<SecureSecret>,
    event: &OutBoxEvent,
    store: DbStore<PgPool>,
) {
    let result = execute_delivery(secure_secret, event).await;
    let attempt_id = Uuid::new_v4();
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

    //Insert into delivery_attempts (via POstgresSQL pg_notify trigger)
    if let Err(e) = store
        .insert_delivery_attempt(attempt_id, &result, event.id)
        .await
        .context("Inserting in delivery_attemp table")
    {
        error!("Failed at inserting delivery attempt: {e}");
        return;
    }

    //Insert into outbox_events status and backoff schedule
    if result.is_success {
        let status = DeliveryStatus::Delivered;
        if let Err(e) = store
            .update_outbox_events_status(event.id, new_attempt_count, status)
            .await
            .context("Updating outbox event status to delivered")
        {
            error!(
                "Error at updated status to delivered for outbox event {} :{}",
                event.id, e
            );
            let _ = &store.pool.rollback().await;
            return;
        } else if new_attempt_count >= event.max_attempts {
            warn!(
                "Event {} reached max atempts ({}/{}). Transitioning to DEAD_LETTER.",
                event.id, new_attempt_count, event.max_attempts
            );
            let status = DeliveryStatus::DeadLetter;
            if let Err(e) = store
                .update_outbox_events_status(event.id, new_attempt_count, status)
                .await
                .context("Updating outbox event status to dead letter")
            {
                error!(
                    "Error at updating status to dead letter for outbox event {} :{}",
                    event.id, e
                );
                let _ = &store.pool.rollback().await;
                return;
            }
        } else {
            if let Err(e) = store
                .update_outbox_events_retry(event.id, new_attempt_count, BASE_BACKOFF_SECONDS)
                .await
                .context("updating retry for outbox event")
            {
                error!(
                    "Error at updating next try for outbox event {} :{}",
                    event.id, e
                );
                let _ = &store.pool.rollback().await;
                return;
            }
        };
    }

    if let Err(e) = &store.pool.commit().await {
        error!(
            "Failed to commit outbox update transaction for the event {}: {}",
            event.id, e
        );
    }
}
