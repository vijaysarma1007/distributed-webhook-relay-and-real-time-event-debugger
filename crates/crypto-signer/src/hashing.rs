//!
//! # Hashing File
//!
//! Contains the code to combine secret phrase from .env file and encrypt or decrypt the client's secret and store or forward to and from outbox events
//!
use aes_gcm::{aead::Aead, Aes256Gcm, Key, Nonce};
use anyhow::{bail, Context, Ok, Result};
use hkdf::{self, Hkdf};
use hmac::KeyInit;
use rand::fill;
use sha2::Sha256;

///
/// # Secure Secret struct
///
/// Enabels to convert client's secret using master phrase.
///
pub struct SecureSecret {
    master_passphrase: Vec<u8>,
}

///
/// #  impl seccure secret struct
///
/// This exposes methods for every secure seceret struct created.
///
impl SecureSecret {
    /// new associate method takes master phrase as `&str` and returns a Self containing a vec master phrase converetd to bytes.
    ///
    /// # Arguments
    /// * `master_passphrase` - master phrase from .env file as `&str`.
    ///
    /// # Returns
    /// returns a [`SecureSecret`] struct with converted master phrase into vec of bytes.
    ///
    pub fn new(master_passphrase: &str) -> Self {
        Self {
            master_passphrase: master_passphrase.as_bytes().to_vec(),
        }
    }

    /// Private function which derives a 32-byte AES-GCM key using a unique salt. This is used internally.
    ///
    /// # Arguments
    /// * `&self` - auto input which exposes [`SecureSecret`] struct inside function
    /// * `salt` - auto generated salt in bytes [`&[u8]`]
    ///
    /// # Returns
    /// Returns a [`anyhow::Result`] containing vec of bytes of exact size of 32 bits.
    ///
    fn derive_key(&self, salt: &[u8]) -> Result<[u8; 32]> {
        // Initates hkdf new struct with sha256 encryption, takes [`Option<salt>`] and master passphrase.
        let hk = Hkdf::<Sha256>::new(Some(salt), &self.master_passphrase);
        // Initiate an empty vec of size 32 bits.
        let mut okm = [0_u8; 32];
        // expands or over writes the Hmac<Sha256> ecoding into exactly 32 bits of length in the initiated empty vec.
        hk.expand(b"webhook-relay-encryption", &mut okm)
            .context("encryting salt with master phrase")?;
        // returns the option with encrypted key
        Ok(okm)
    }

    /// function to encrypt the client's secret
    ///
    /// # Arguments
    /// * `plaintext` - intakes client's secret as plain str in [`&str`]
    ///
    /// # Returns
    /// returns a [`anyhow::Result`] containing the encrypted `Vec<u8>`
    ///
    pub fn encrypt(&self, plaintext: &str) -> Result<Vec<u8>> {
        // Initiate an emprty vec,exactly of size 16 bits.
        let mut salt = [0_u8; 16];
        // Fills the empty vec with random bytes of 16 bits length.
        fill(&mut salt);
        // utilizes the private derive function which takes salt and sends out a sha256 in vec<u8> of 32 bit size
        let key_bytes = self.derive_key(&salt).context("deriving keys from salt")?;
        // adds a encryption methods to the keys
        let key = Key::<Aes256Gcm>::from(key_bytes);
        // create a new cipher from key
        let cipher = Aes256Gcm::new(&key);

        // creates a vec<u8> of 12 bits size.
        let mut nounce_bytes = [0_u8; 12];
        // fills the emoty nounce vec with random bits
        fill(&mut nounce_bytes);
        //create a new Nonce array from nonce bytes
        let nounce = Nonce::from(nounce_bytes);
        // encrypt the client's secret using cipher and nounce
        let ciphertext = cipher
            .encrypt(&nounce, plaintext.as_bytes())
            .context("encrypting plain text")?;
        //creates an empty vec of initial capacity of (salt size (16) + nounce size (12) = 28)  + length of cipher text
        let mut output = Vec::with_capacity(28 + ciphertext.len());
        // extend output from salt ([u8]), appends salt bytes to empty vec
        output.extend_from_slice(&salt);
        // extend output from nounce bytes ([u8]), appends nounce bytes to empty vec
        output.extend_from_slice(&nounce_bytes);
        // extend ciphertext from ciphertext (Vec<u8>), appends cipher text btyes to empty vec.
        output.extend(ciphertext);
        // returns an option with encrypted Vec<u8> for client's secret.
        Ok(output)
    }

    /// function to decrypt the client's secret, intakes encrypted Vec<u8> from secret stored in database.
    ///
    /// # Arguments
    /// * `payload` - accepts &[u8], which is a encrypted client's secret.
    ///
    /// # Returns
    /// returns a [`anyhow::Result`] containing a `String` exposing the client's secret
    pub fn decrypt(&self, payload: &[u8]) -> Result<String> {
        // check if the encrypting size is less than 28, if yes then returns an error and exits the process.
        if payload.len() < 28 {
            bail!("Payload too short");
        }
        // extracts the salt from first 16 bytes.
        let salt = &payload[..16];
        // extracts the nonce_bytes from next 12 bytes.
        let nonce_bytes = &payload[16..28];
        // extracts the cipher text from the remaining bytes.
        let ciphertext = &payload[28..];
        // derives a keys from the salt.
        let key_bytes = self.derive_key(salt).context("derving keys from salt")?;
        // creates the keys from derived keys
        let key = Key::<Aes256Gcm>::from(key_bytes);
        // create a new cipher struct with new key
        let cipher = Aes256Gcm::new(&key);
        // extract nonce from nonce bytes
        let nounce = Nonce::try_from(nonce_bytes).context("nonce from bytes")?;
        // decrypt the &[u8], using nounce and cipher text, into vec<u8>
        let plaintext = cipher
            .decrypt(&nounce, ciphertext)
            .context("Decrpting the payload")?;
        // converts and returns a option string from vec<u8>
        String::from_utf8(plaintext).context("converting to plaintext from utf8")
    }
}
