use crate::config::{IDLE_TIMEOUT, MAX_TIMEOUT, MINIMUN_CONNECTIONS};
use anyhow::{Context, Result};
use sqlx::{postgres::PgPoolOptions, Pool, Postgres};
use std::time::Duration;

/// connect to the postgres databse
/// # Methods used
/// * `acquire_timeout` - wait time for checking out a connection from the pool.
/// * `idle_timeout` - if a connection is not used or sits idle for the specified time it will free the TCP connection, helps in memory managing the postgres server.
/// * `min_connections` - minimum connections lets to keep active number of connections all the time, this will help to avoid cold starts.
///
/// # Arguments
/// * `database_url`- Accepts a [`&str`] as database url, where postgres sql is hosted.
/// * `max_connections` - Accepts a [`u32`] which will accept number of paralel queries that it needs to be executed.
///
/// # Returns
///
/// Returns a [`DbStore`] struct containing the postgres pool for successful connection.
///
pub async fn connect(database_url: &str, max_connections: u32) -> Result<Pool<Postgres>> {
    let pool = PgPoolOptions::new()
        .max_connections(max_connections)
        .min_connections(MINIMUN_CONNECTIONS)
        .acquire_timeout(Duration::from_secs(MAX_TIMEOUT))
        .idle_timeout(Duration::from_secs(IDLE_TIMEOUT))
        .connect(database_url)
        .await
        .context("Connecting to the database")?;

    Ok(pool)
}
