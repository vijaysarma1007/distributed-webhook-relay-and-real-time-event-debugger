//!
//! # Dispatch worker service bin - main
//!
//! This is the main function for dispatch worker service bin
//!

mod config;
mod helper;

use crate::config::{MAXIMUM_DATABASE_CONNECTIONS, MAX_CONCURRENT_DISPACHES, TOKIO_SLEEP_TIME};
use anyhow::{Context, Ok, Result as AnyhowResult};
use crypto_signer::hashing::SecureSecret;
use db_store::{dbconnect::connect, helper::next_wakeup_delay};
use dotenvy;
use helper::dipatch_cycle::run_dispatch_cycle;
use sqlx::postgres::PgListener;
use std::{sync::Arc, time::Duration};
use tokio::{self, signal, sync::Semaphore, time::sleep};
use tracing::{self, error, info};
use tracing_subscriber;
///
/// # Main function with tokio main
///
/// This function initiates databse connection with established postgres server, actively listens to the pg notifications if any
///
/// # Arguments
/// None
///
/// # Returns
///  Returrns a [`anyhow::Result`] with no content
///
#[tokio::main]
async fn main() -> AnyhowResult<()> {
    // used to trace or log any commits to postgres or server, in the terminal
    tracing_subscriber::fmt()
        .with_max_level(tracing_subscriber::filter::LevelFilter::DEBUG)
        .init();
    // enable the environment file
    dotenvy::dotenv().context("Intializing env file").ok();
    // read the database url for postgres from env file.
    let database_url = std::env::var("DATABASE_URL").context("Database url must be set")?;
    // read the secret master phrase form env file, which will be used to encode or decode client's secret
    let master_passphrase =
        std::env::var("SECRET_PHASE").context("secret phrase value must be set")?;
    // connect to postgres sql server using sqlx crate
    let pool = connect(&database_url, MAXIMUM_DATABASE_CONNECTIONS)
        .await
        .context("connecting to postgres from dispatch worker")?;
    // logs a successsful message in terminal
    info!("Dispatch worker connected to postgres SQL");
    // creates a new Arc for secure secret struct using master passphrase.
    let securesecret = Arc::new(SecureSecret::new(&master_passphrase));
    // create a new Arc for semaphore struct with maximum number of concurrent dispatch connections
    let semaphore = Arc::new(Semaphore::new(MAX_CONCURRENT_DISPACHES));
    // activates a new pg listener with the databse url
    let mut pg_listener = PgListener::connect(&database_url)
        .await
        .context("Connecting to pg listener in dispatch worker")?;
    // emits a successful message in terminal after connecting to pg listener.
    info!("Connected to Pg listener from dispatch worker");
    // actively listens to 'outbox_channel' custom function from postrges database will emit a signal whenever a new outbox_event is created.
    pg_listener
        .listen("outbox_channel")
        .await
        .context("Listening to webhook_stream from postgres database")?;
    // emits a succesful message in terminal, after it connects to the active postgres listener channel
    info!("outbox worker listening for real time database notifications");
    // continues to loop and listner to either of the two arms, this will the current worker dispatch running in the server
    loop {
        // Running dispatch cycle otuside tokio select, processes the pending events which met the deadline immediately.
        // No duplicated logic - sleep will make the tokio runtime wait which is equal to next retry.
        // akways retry the next sleep
        // once the tokio runtime is set to sleep, tokio select will race between the sleep time of tokio and incoming pg notify listenser, as soon as there some trigger from postgres sql via pg notify, tokio sleep will be cancelled and it will immediately head to first line of code within the loop which is ru_dispatch_cycle.
        if let Err(e) = run_dispatch_cycle(pool.clone(), &securesecret, &semaphore).await {
            error!("Dispatch cycle failed: {e}");
        }
        // calculates the sleep time for tokio runtime via this function which will fetch the minimum sleep time until next dispatch cycle
        let wait_duration = next_wakeup_delay(&pool.clone())
            .await
            .unwrap_or(Duration::from_secs(TOKIO_SLEEP_TIME));
        // if the wait duration is zero, it will loop back to first time and imeediately run the dipatch cycle.
        if wait_duration.is_zero() {
            continue;
        }

        // tokio select lets you to define several arms, which will return first succesful event, on first come first serve basis, similar to promise::race in node js
        tokio::select! {
            // if the user click ctrl + C it will terminate the loop and shuts the terminal, with info message in the terminal
            _ = signal::ctrl_c() => {
                info!("Shutdown signal recieved. Exiting loop cleanly");
                break;
            }
            // this will make tokio runtime sleep for exactly the duration of next retry
            _ = sleep(wait_duration) => {
                info!("Next retry scheduled in {:?}. waiting for timer or pg_notify..", wait_duration);
                }
            // below code block will be executed if a new outbox_events is created in the postgres databse and it emits an signal to the databse in "outbox_channel" channel.
            notification = pg_listener.recv() => {
                //match the incoming notification from pg active listener with std::result::Result
                match notification {
                    // for the succesful notification run the dispatch cycle with a sucess info message in the terminal
                    std::result::Result::Ok(notif) => {
                        info!("New outboox event triggered! ID: {}", notif.payload());
                    }
                    // if fails to recieve or translate the notication it will emit a erro message in the termial and puts the tokio runtime to sleep for sometime, keeping a tokio runtime to sleep will help the loop to stop explosive recalls in the loop.
                    Err(e) => {
                        error!("Database listener errror: {}. Attempting reconnect...", e);
                        tokio::time::sleep(Duration::from_secs(TOKIO_SLEEP_TIME)).await;
                    }
                }
            }
        }
    }
    // Return an unit with anyhow::Result
    Ok(())
}
