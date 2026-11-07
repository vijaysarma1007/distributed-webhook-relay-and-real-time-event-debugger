//! # config file for dispatch worker service crate
//!
///Maximum allowed text size allowed from response received from client's server in kb.
pub(crate) const MAXIMUM_ALLOWED_TEXT_SIZE_FROM_RESPONSE: i16 = 4;

pub(crate) const BATCH_SIZE: i64 = 25;

pub(crate) const MAX_CONCURRENT_DISPACHES: usize = 20;

pub(crate) const BASE_BACKOFF_SECONDS: i64 = 5;
