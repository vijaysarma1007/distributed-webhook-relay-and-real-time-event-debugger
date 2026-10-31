//!
//! # Crypto Signer
//!

use anyhow::{Context, Result};
use hex::encode;
use hmac::{Hmac, KeyInit, Mac};
use sha2::Sha256;

///Creates a custom type of `HmacSha256`, combines sha256 encryption with hmac.
type HmacSha256 = Hmac<Sha256>;

///use this function to generate a unquie signature between server and client. This ensures that the request is coming from authentic sources and which makes it lesslikely to replicate the request from unverified sources.
/// **Note:** mac is updated with timestamp to avoid any duplicate request, if in case hacker is able to replicate the request.
///
/// # Arguments
/// * `payload` - data that needs to be forwaded to client server in &str.
/// * `secret` - secret password that enables successful authentic handshake between server and the client, in &str.
/// * `timestamp` -  time in milliseconds in i64.
///
/// # Returns
///
/// Returns a `anyhow::Result` containing the ecoded hex or returns an error, if failed.
///
pub fn generate_signature(payload: &str, secret: &str, timestamp: i64) -> Result<String, String> {
    let mut mac = HmacSha256::new_from_slice(secret.as_bytes())
        .context("creating mac with sha256")
        .map_err(|e| format!("Invalid HMAC key : {e}"))?;

    mac.update(format!("{}.{}", timestamp, payload).as_bytes());
    let result = mac.finalize();
    let hex_encode = encode(result.into_bytes());
    Ok(hex_encode)
}
