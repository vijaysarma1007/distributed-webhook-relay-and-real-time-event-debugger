//! # Testing encryption and decryption of client secret
use crypto_signer::hashing::SecureSecret;

/// Constants used for testing
const MASTER_PASSPHRASE: &str = "some_secret_phrase";
const CLIENT_SECRET: &str = "client_secret";

/// create a new secure secret using a master passphrase and encrpyt client's seceret in database and decrypt back the client secret.
#[test]
fn test_encrypt_and_decrypt_of_client_secret_equal() {
    let secure_secret = SecureSecret::new(MASTER_PASSPHRASE);
    let encoded_secret = secure_secret.encrypt(CLIENT_SECRET).unwrap();
    let decrypt_secret = secure_secret.decrypt(&encoded_secret).unwrap();
    assert_eq!(
        CLIENT_SECRET, decrypt_secret,
        "decrypted string must be equal to client secret"
    );
}
