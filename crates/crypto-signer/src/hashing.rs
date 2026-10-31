use aes_gcm::{aead::Aead, Aes256Gcm, Key, Nonce};
use anyhow::{bail, Context, Ok, Result};
use hkdf::{self, Hkdf};
use hmac::KeyInit;
use rand::fill;
use sha2::Sha256;

pub struct SecureSecret {
    master_passphrase: Vec<u8>,
}

impl SecureSecret {
    pub fn new(master_passphrase: &str) -> Self {
        Self {
            master_passphrase: master_passphrase.as_bytes().to_vec(),
        }
    }

    /// Derives a 32-byte AES-GCM key using a unique salt
    fn derive_key(&self, salt: &[u8]) -> Result<[u8; 32]> {
        let hk = Hkdf::<Sha256>::new(Some(salt), &self.master_passphrase);
        let mut okm = [0_u8; 32];
        hk.expand(b"webhook-relay-encryption", &mut okm)
            .context("encryting salt with master phrase")?;
        Ok(okm)
    }

    pub fn encrypt(&self, plaintext: &str) -> Result<Vec<u8>> {
        let mut salt = [0_u8; 16];
        fill(&mut salt);

        let key_bytes = self.derive_key(&salt).context("deriving keys from salt")?;
        let key = Key::<Aes256Gcm>::from(key_bytes);
        let cipher = Aes256Gcm::new(&key);

        let mut nounce_bytes = [0_u8; 12];
        fill(&mut nounce_bytes);
        let nounce = Nonce::from(nounce_bytes);

        let ciphertext = cipher
            .encrypt(&nounce, plaintext.as_bytes())
            .context("encrypting plain text")?;

        let mut output = Vec::with_capacity(28 + ciphertext.len());
        output.extend_from_slice(&salt);
        output.extend_from_slice(&nounce_bytes);
        output.extend(ciphertext);

        Ok(output)
    }

    pub fn decrypt(&self, payload: &[u8]) -> Result<String> {
        if payload.len() < 28 {
            bail!("Payload too short");
        }
        let salt = &payload[..16];
        let nonce_bytes = &payload[16..28];
        let ciphertext = &payload[28..];

        let key_bytes = self.derive_key(salt).context("derving keys from salt")?;
        let key = Key::<Aes256Gcm>::from(key_bytes);
        let cipher = Aes256Gcm::new(&key);
        let nounce = Nonce::try_from(nonce_bytes).context("nonce from bytes")?;
        let plaintext = cipher
            .decrypt(&nounce, ciphertext)
            .context("Decrpting the payload")?;

        String::from_utf8(plaintext).context("converting to plaintext from utf8")
    }
}
