use crate::config::MAXIMUM_ALLOWED_TEXT_SIZE_FROM_RESPONSE;
use chrono::Utc;
use core_types::OutBoxEvent;
use crypto_signer::{generate_signature, hashing::SecureSecret};
use db_store::clientconnect::ClientConnect;
use std::time::Instant;

pub struct DispatchResult {
    pub http_status: Option<i32>,
    pub latency_ms: i32,
    pub response_body: Option<String>,
    pub error_message: Option<String>,
    pub is_success: bool,
}

pub async fn execute_delivery(crypto: &SecureSecret, event: OutBoxEvent) -> DispatchResult {
    let encrypted_secret = &event.encrypted_secret;
    let secret = match crypto.decrypt(&encrypted_secret) {
        Ok(secure) => secure,
        Err(err) => {
            return DispatchResult {
                http_status: None,
                latency_ms: 0,
                response_body: None,
                error_message: Some(format!("Decryption Failure: {}", err)),
                is_success: false,
            }
        }
    };

    let timestamp = Utc::now().timestamp();
    let payload = &event.payload.to_string();
    let signature = match generate_signature(&payload, &secret, timestamp) {
        Ok(signature) => signature,
        Err(err) => {
            return DispatchResult {
                http_status: None,
                latency_ms: 0,
                response_body: None,
                error_message: Some(format!("Signing error: {}", err)),
                is_success: false,
            };
        }
    };

    let start = Instant::now();

    let response = ClientConnect::default().connect(event, &signature).await;

    let latency_ms = start.elapsed().as_millis().min(i32::MAX as u128) as i32;

    match response {
        Ok(res) => {
            let status = res.status().as_u16() as i32;
            let is_success = (200..=299).contains(&status);
            let allowed_text_bytes_size = (MAXIMUM_ALLOWED_TEXT_SIZE_FROM_RESPONSE * 1024) as usize;
            let body = match res.text().await {
                Ok(text) => {
                    if text.len() > allowed_text_bytes_size {
                        Some(format!(
                            "{}... [truncated]",
                            &text[..allowed_text_bytes_size]
                        ))
                    } else if text.is_empty() {
                        None
                    } else {
                        Some(text)
                    }
                }

                Err(_) => None,
            };

            DispatchResult {
                http_status: Some(status),
                latency_ms,
                response_body: body,
                error_message: None,
                is_success,
            }
        }
        Err(error) => {
            let status = error.status().map(|s| s.as_u16() as i32);
            DispatchResult {
                http_status: status,
                latency_ms,
                response_body: None,
                error_message: Some(error.to_string()),
                is_success: false,
            }
        }
    }
}
