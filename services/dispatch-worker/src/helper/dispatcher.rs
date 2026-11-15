//!
//! # Dispatcher
//!
//! This function is responsible for dispatching single event to the client server, while decrypting the secret phrase and adding signature to the request.
//!
//!
use crate::config::MAXIMUM_ALLOWED_TEXT_SIZE_FROM_RESPONSE;
use chrono::Utc;
use core_types::{DispatchResult, OutBoxEvent};
use crypto_signer::{generate_signature, hashing::SecureSecret};
use db_store::clientconnect::ClientConnect;
use std::time::Instant;
///
/// # Execute Delivery
///
/// This function is used to send a request to the client server with appropriate signature and body and error handling.
///
/// # Arguments
/// * `crypto` - [`&secureSecret`] A reference to Secure sceret struct for encoding and decoding client's secret.
/// * `event` - [`&OutBoxEvent`] A reference to OutboxEvent struct which contains data and payload required to send to client's server.
///
/// # Returns
/// Returns a struct of [`DispatchResult`] which contains https status <Option>, latency (in miliseconds) <default: 0>, response body <Option>, error message <Option>, is success <Boolean>, on success or fail from client's server response.
///
pub async fn execute_delivery(crypto: &SecureSecret, event: &OutBoxEvent) -> DispatchResult {
    // extracting encrypted secret from event object.
    let encrypted_secret = &event.encrypted_secret;
    // Decrypting the secret use secure secret struct which creates a key using secret master phrase, output a string.
    let secret = match crypto.decrypt(&encrypted_secret) {
        // returns a string on succesfuly decryption
        Ok(secure) => secure,
        // returns an dispatch result struct with error type, if failed
        Err(err) => {
            return DispatchResult {
                http_status: Some(500),
                latency_ms: 0,
                response_body: None,
                error_message: Some(format!("Decryption Failure: {}", err)),
                is_success: false,
            }
        }
    };

    // gets the current timestamp in milliseconds
    let timestamp = Utc::now().timestamp();
    // extract the payload from event, which will be attached to the request body that will be sent to client.
    let payload = &event.payload.to_string();
    // generates a signature using payload, secret and timestamp whicch will be attached to request header "X-Signature".
    let signature = match generate_signature(&payload, &secret, timestamp) {
        // returns a signature in hexacode format
        Ok(signature) => signature,
        // returns a dispatch result struct with error message, if fails
        Err(err) => {
            return DispatchResult {
                http_status: Some(500),
                latency_ms: 0,
                response_body: None,
                error_message: Some(format!("Signing error: {}", err)),
                is_success: false,
            };
        }
    };

    // starts the timer from 0 ,just before request is sent to client's server
    let start = Instant::now();
    // initialise a new reqwest connection and send the payload to client server and awaits the response
    let response = ClientConnect::default().connect(event, &signature).await;
    // calculates the time taken to execute the request to client server, time elapsed returns a u128 type, which then returns a minimum of either max of i32 or generated elapsed time and returns a i32 which will be safe to store in database
    let latency_ms = start.elapsed().as_millis().min(i32::MAX as u128) as i32;

    // matches with the response received from client server
    match response {
        // for succesful reponse, with a success status of 200 to 299
        Ok(res) => {
            // extracts the status from response body as i32
            let status = res.status().as_u16() as i32;
            // check if the reponse status is between 200 to 299.
            let is_success = (200..=299).contains(&status);
            // calculate the maximum allowed body size allowed in bytes
            let allowed_text_bytes_size = (MAXIMUM_ALLOWED_TEXT_SIZE_FROM_RESPONSE * 1024) as usize;
            // checks if the response body has text body
            let body = match res.text().await {
                // if body has text body it will execute the following code block
                Ok(text) => {
                    // if the text size exceeds more than allowed bytes size then trim or truncate it
                    if text.len() > allowed_text_bytes_size {
                        Some(format!(
                            "{}... [truncated]",
                            &text[..allowed_text_bytes_size]
                        ))
                    } else if text.is_empty() {
                        // sends None if text body is empty
                        None
                    } else {
                        // returns the text as it is , if the text byte size is less than maximum allowed text byte size.
                        Some(text)
                    }
                }
                // returns a None on error if no text body found
                Err(_) => None,
            };
            // returns a dispatch result struct with extracted body with appropriate response status code.
            DispatchResult {
                http_status: Some(status),
                latency_ms,
                response_body: body,
                error_message: None,
                is_success,
            }
        }
        // if connection fails , then following error object will be executed.
        Err(error) => {
            // extract the status code from error object as i32.
            let status = error.status().map(|s| s.as_u16() as i32);
            // returns a dispatch result struct with appropriate status code ans error message
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
