//! # Configuration file for db store library crate.

/// `MAX_TIMEOUT` set to acuire timeout
pub(crate) const MAX_TIMEOUT: u64 = 5;
/// `IDLE_TIMEOUT` set to idle timeout
pub(crate) const IDLE_TIMEOUT: u64 = 600;
/// `MINIMUN_CONNECTIONS` set to minimum connections at 5.
pub(crate) const MINIMUN_CONNECTIONS: u32 = 5;
/// `CLIENT_CONNECTION_TIMEOUT`- maximum time to wait until scueesful connection is made with client.
pub(crate) const CLIENT_CONNECTION_TIMEOUT: u64 = 4;
/// `CLIENT_REQUEST_TIMEOUT` - maximum time to wait for the request get processed.
pub(crate) const CLIENT_REQUEST_TIMEOUT: u64 = 10;
/// `CLIENT_IDLE_CONNECTION_TIMEOUT` - set the maximum time for connection when idle
pub(crate) const CLIENT_IDLE_CONNECTION_TIMEOUT: u64 = 120;
/// `IDLE_SAFETY_POLL` - used to calculate the minimum safety idle poll time for outbox delivery
pub(crate) const IDLE_SAFETY_POLL: u64 = 30;
/// `MINIMUM_WAITING_TIME` - used to set the minimum garuntee waiting time if future event next_retry is less than 1.
pub(crate) const MINIMUM_WAITING_TIME: u64 = 1;
