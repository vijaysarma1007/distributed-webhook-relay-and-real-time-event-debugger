//! # Tests case for crpto signer library crate
//!

use crypto_signer::generate_signature;

/// ## constants used in testing
const SECRET: &str = "secure_secret";
const TIMESTAMP1: i64 = 1710000000;
const TIMESTAMP2: i64 = 1710000005;
const PAYLOAD: &str = "{name:\"vijay\", age: \"33\"}";

/// ## Asseritng that a HMAC signature is generated
///
/// This test case will assert that HMAC signature is successfully generated and is exactly of size 64 bits
///
#[test]
fn test_generate_hmac() {
    let result = generate_signature(PAYLOAD, SECRET, TIMESTAMP1);
    assert!(result.is_ok(), "HMAC geenration should succeed.");
    let signature = result.unwrap();

    assert_eq!(signature.len(), 64, "SHA-256 signature must be 64 chars.")
}

/// ## Asserting that a HMAC signature is generated for two equal payloads, created at same timestamp.
///
/// This test case will assert that HMAC signature for provided payloads and timestamp generates same signatures.
///
#[test]
fn test_equal_signatures() {
    let result1 = generate_signature(PAYLOAD, SECRET, TIMESTAMP1);
    let result2 = generate_signature(PAYLOAD, SECRET, TIMESTAMP1);

    assert!(
        result1.is_ok() && result2.is_ok(),
        "Both HMAC signatures must succeed"
    );

    assert_eq!(
        result1, result2,
        "HMAC signature for result1 and result2 must match"
    );
}

/// ## Asserting that a HMAC signature is generated for unequal payloads, created at different timestamps
///
/// This test case will assert that HMAC signature for provided same payloads and different timestamps generates two different HMAC signatures.
///
#[test]
fn test_unequal_signature() {
    let result1 = generate_signature(PAYLOAD, SECRET, TIMESTAMP1);
    let result2 = generate_signature(PAYLOAD, SECRET, TIMESTAMP2);

    assert!(
        result1.is_ok() && result2.is_ok(),
        "Both HMAC signatures must succeed"
    );
    assert_ne!(
        result1, result2,
        "HMAC signature for result1 and result2 must not match"
    );
}
