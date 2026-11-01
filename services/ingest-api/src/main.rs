pub mod routes;

use anyhow::{bail, Context, Result};
use axum::{self, Router};
use core_types;
use crypto_signer::{generate_signature, hashing::SecureSecret};
use db_store::{dbconnect::connect, DbStore};
use dotenvy;
use std::net::SocketAddr;
use tokio;
use tracing_subscriber;

const MAX_CONNECTIONS: u32 = 10;

#[derive(Clone)]
pub struct AppState {
    store: DbStore,
    secret_phrase: String,
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_max_level(tracing_subscriber::filter::LevelFilter::DEBUG)
        .init();

    dotenvy::dotenv()
        .context("Initializing environment variables")
        .ok();

    let database_url = std::env::var("DATABASE_URL").context("Loading db url")?;
    let secret_phrase = std::env::var("SECRET_PHASE").context("loading secret phase")?;
    let port = std::env::var("INGEST_SERVICE_PORT")
        .context("loading ingest service port")
        .unwrap_or_else(|_| "3000".to_string())
        .parse::<u16>()
        .context("parsing string into u16")
        .unwrap_or_else(|_| 3000);

    let pool = connect(&database_url, MAX_CONNECTIONS)
        .await
        .context("connecting database from ingest api service")?;

    let state = AppState {
        store: DbStore::new(pool),
        secret_phrase,
    };

    let app = Router::new().nest("/api", routes::v1events(state));

    let address = SocketAddr::from(([0, 0, 0, 0], port));

    let listener = tokio::net::TcpListener::bind(address)
        .await
        .context(format!("connecting tcp server to the port {}", port))?;
    axum::serve(listener, app)
        .await
        .context("serving tcp listener to app router")?;
    println!("listening in port{}", port);
    Ok(())
}
