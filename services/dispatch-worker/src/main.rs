mod config;
mod helper;

use crate::config::{MAXIMUM_DATABASE_CONNECTIONS, MAX_CONCURRENT_DISPACHES};
use anyhow::{Context, Ok, Result as AnyhowResult};
use crypto_signer::hashing::SecureSecret;
use db_store::dbconnect::connect;
use dotenvy;
use helper::dipatch_cycle::run_dispatch_cycle;
use sqlx::postgres::PgListener;
use std::{sync::Arc, time::Duration};
use tokio::{self, signal, sync::Semaphore};
use tracing::{self, error, info};
use tracing_subscriber;

#[tokio::main]
async fn main() -> AnyhowResult<()> {
    tracing_subscriber::fmt()
        .with_max_level(tracing_subscriber::filter::LevelFilter::DEBUG)
        .init();
    dotenvy::dotenv().context("Intializing env file").ok();

    let database_url = std::env::var("DATABASE_URL").context("Database url must be set")?;
    let master_passphrase =
        std::env::var("SECRET_PHASE").context("secret phrase value must be set")?;

    let pool = connect(&database_url, MAXIMUM_DATABASE_CONNECTIONS)
        .await
        .context("connecting to postgres from dispatch worker")?;

    info!("Dispatch worker connected to postgres SQL");
    let securesecret = Arc::new(SecureSecret::new(&master_passphrase));
    let semaphore = Arc::new(Semaphore::new(MAX_CONCURRENT_DISPACHES));

    let mut pg_listener = PgListener::connect(&database_url)
        .await
        .context("Connecting to pg listener in dispatch worker")?;

    info!("Connected to Pg listener from dispatch worker");

    pg_listener
        .listen("outbox_channel")
        .await
        .context("Listening to webhook_stream from postgres database")?;

    info!("outbox worker listening for real time database notifications");

    loop {
        tokio::select! {
            _ = signal::ctrl_c() => {
                info!("Shutdown signal recieved. Exiting loop cleanly");
                break;
            }

            notification = pg_listener.recv() => {
                match notification {
                    std::result::Result::Ok(notif) => {
                        info!("New outboox event triggered! ID: {}", notif.payload());
                        if let Err(e) = run_dispatch_cycle(pool.clone(), &securesecret, &semaphore).await{
                                    error!("Dispatch cycle failed: {e}");
                        }
                    }
                    Err(e) => {
                        error!("Database listener errror: {}. Attempting reconnect...", e);
                        tokio::time::sleep(Duration::from_secs(2)).await;
                    }
                }
            }
        }
    }

    Ok(())
}
