//! # config file for dispatch worker service crate
//!
/// Maximum allowed text size allowed from response received from client's server in kb.
pub(crate) const MAXIMUM_ALLOWED_TEXT_SIZE_FROM_RESPONSE: i16 = 4;
/// Maximum number of events called in each request from outbox events
pub(crate) const BATCH_SIZE: i64 = 25;
/// Emits the maximum number of dispatches at a time to client server
pub(crate) const MAX_CONCURRENT_DISPACHES: usize = 20;
/// Time increments for next request dispatch
pub(crate) const BASE_BACKOFF_SECONDS: i64 = 5;
/// Maximum Number of concurrent database connections.
pub(crate) const MAXIMUM_DATABASE_CONNECTIONS: u32 = 10;
/// sleep time for tokio runtime if no notication is recived.
pub(crate) const TOKIO_SLEEP_TIME: u64 = 2;
