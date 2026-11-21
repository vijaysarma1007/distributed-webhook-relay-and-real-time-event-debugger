//!
//! # Hepler functions for db crate lilbrary
//!
//! This has all the functions that are additionally used to execute some special calls from the database
//!

use crate::config::{IDLE_SAFETY_POLL, MINIMUM_WAITING_TIME};
use anyhow::Result;
use chrono::Utc;
use sqlx::PgPool;
use std::time::Duration;

/// This function to get the minimum next retry time from outbox events with pending status, which will used to make the tokio loop wait to emit retry event.
///
/// # Next Wakeup Delay
///
/// # Arguments
/// None
///
/// # Returns
///
/// Returns a [`anyhow::Result`] of minimum duration in seconds.
///
pub async fn next_wakeup_delay(pool: &PgPool) -> Result<Duration> {
    // Sets the idle safety poll from config file into Duration struct
    let idle_safety_poll = Duration::from_secs(IDLE_SAFETY_POLL);
    // Gets the minimum next retry from outbox_events with status = PENDING
    let earliest = sqlx::query_scalar!(
        r#"
              SELECT MIN(next_retry_at)
              FROM outbox_events
              WHERE status = 'PENDING'
              "#
    )
    .fetch_one(pool)
    .await?;

    // compare the fetched next retry from the database, with current time and return the calculated minimum idle time
    match earliest {
        Some(ts) => {
            // fetches the current UTC time
            let now = Utc::now();
            // if next retry is less than current time, then set the duration to zero.
            if ts <= now {
                Ok(Duration::ZERO)
            } else {
                // if next retry is more than current time, then execute the following code block
                // calculate the wait time, covert to standard time (which check if the time is a positive value, if not return an error), if error then return the minimum waiting time and then compare the minimum of idle safety poll and returns the minimum waiting time used for tokio runtime.
                let wait = (ts - now)
                    .to_std()
                    .unwrap_or(Duration::from_secs(MINIMUM_WAITING_TIME))
                    .min(idle_safety_poll);
                // returns ok with minimum wait time
                Ok(wait)
            }
        }
        // if no minimum next retry is found, then return ok variant with minimum idle safety poll to tokio runtime
        None => Ok(idle_safety_poll),
    }
}
