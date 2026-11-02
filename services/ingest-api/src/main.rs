//!
//! # Ingest Api
//!
//! Use this service as the entry point to register client's endpoint, secret and payload.
//! This will create an event inside outbox events, on success
//! The client's secret is combined with secret phrase stored in .env file to encrypt and decrypt the same, this helps to avoid in revealing any secret if in case databased is compromised.
//!

pub mod config;
pub mod routes;

use anyhow::{Context, Result};
use axum::{self, Router};
use config::MAX_CONNECTIONS;
use db_store::{dbconnect::connect, DbStore};
use dotenvy;
use std::net::SocketAddr;
use tokio;
use tracing_subscriber;

///
/// # App State Struct
///
/// This struct stores the new db connection and secret phrase to encode or decode the client's secret.
///
/// # Arguments
/// * `store` - accepts a connection of type [`DbStore`].
/// * `secret_phrase` - A secret phrase accepts `String`, from .env file
///
#[derive(Clone)]
pub struct AppState {
    store: DbStore,
    secret_phrase: String,
}

/// [`tokio::main`] from tokio crate automatically enables concurrency
///
/// # Returns
///  Returns a [`anyhow::Result`] with a unit/ null , and [`anyhow::Error`] if it fails.
/// main is an asynchronous function which waits for the remote network to repond, await method executes the function which it is attached to.
#[tokio::main]
async fn main() -> Result<()> {
    // Used from tracing_subscriber crate to collect, filter and formats the events to the log
    tracing_subscriber::fmt()
        .with_max_level(tracing_subscriber::filter::LevelFilter::DEBUG)
        .init();

    // Enables the current file to read the values from .env file
    dotenvy::dotenv()
        .context("Initializing environment variables")
        .ok();

    // Get the database url from the .env file, to make a succesfuly connection to the postgres db server.
    let database_url = std::env::var("DATABASE_URL").context("Loading db url")?;
    // Get the secret phrase from the .env file to encode or decode the client's secret.
    let secret_phrase = std::env::var("SECRET_PHASE").context("loading secret phase")?;
    // Get the port number for the ingest api service binary crate, reads the port as a String and then parses into 16 bit integer format.
    let port = std::env::var("INGEST_SERVICE_PORT")
        .context("loading ingest service port")
        .unwrap_or_else(|_| "3000".to_string())
        .parse::<u16>()
        .context("parsing string into u16")
        .unwrap_or_else(|_| 3000);

    // Connect is derived from db_store::dbconnect which takes database url from .env file and max connections to make a succesful connections with postgres db via sqlx crate, returns a Pool struct.
    let pool = connect(&database_url, MAX_CONNECTIONS)
        .await
        .context("connecting database from ingest api service")?;

    // Creating an AppState struct which internally creates a db store from Pool struct and secret phrase from .env file.
    let state = AppState {
        store: DbStore::new(pool),
        secret_phrase,
    };

    // Creates a network route from axum crate. "/api" represents the base strcut for the endpoint, while it allows nesting other endpoint on top.
    let app = Router::new().nest("/api", routes::v1events(state));

    // Creates an address using port fetched from .env file.
    let address = SocketAddr::from(([0, 0, 0, 0], port));

    // Creates a continous TCP listender using tokio crate and binds with the port number created.
    let listener = tokio::net::TcpListener::bind(address)
        .await
        .context(format!("connecting tcp server to the port {}", port))?;

    // Combines the active TCP listener from tokio crate to routes created using axum crate.
    axum::serve(listener, app)
        .await
        .context("serving tcp listener to app router")?;

    println!("listening in port{}", port);
    // Returns OK without any value.
    Ok(())
}
