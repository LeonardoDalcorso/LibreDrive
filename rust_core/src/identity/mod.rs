//! Identity Module - BIP39 Seed Phrase based identity management
//!
//! This module handles user identity creation and recovery using
//! BIP39 mnemonic seed phrases (10 words), similar to Bitcoin wallets.

mod seed;
mod keys;

pub use seed::SeedPhrase;
pub use keys::KeyPair;

use crate::crypto::{self, EncryptionKey, SigningKeyPair};
use ed25519_dalek::{SigningKey, VerifyingKey};
use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Error, Debug)]
pub enum IdentityError {
    #[error("Invalid seed phrase: {0}")]
    InvalidSeedPhrase(String),

    #[error("Key derivation failed: {0}")]
    KeyDerivation(String),

    #[error("Cryptographic error: {0}")]
    Crypto(#[from] crypto::CryptoError),
}

/// User identity derived from seed phrase
/// Contains all cryptographic keys needed for the system
#[derive(Clone)]
pub struct UserIdentity {
    /// Master seed (derived from mnemonic + password)
    master_seed: [u8; 64],

    /// Signing key pair for authentication and signatures
    signing_keys: SigningKeyPair,

    /// Encryption key for file encryption
    encryption_key: EncryptionKey,

    /// Node ID for P2P network (derived from public key)
    node_id: [u8; 32],
}

impl UserIdentity {
    /// Generate a new identity with a fresh seed phrase
    /// Returns the identity and the seed phrase (MUST be saved by user)
    pub fn generate(password: Option<&str>) -> Result<(Self, String), IdentityError> {
        let seed_phrase = SeedPhrase::generate(10)?;
        let mnemonic_str = seed_phrase.to_string();

        let identity = Self::from_seed_phrase(&mnemonic_str, password)?;
        Ok((identity, mnemonic_str))
    }

    /// Recover identity from existing seed phrase
    pub fn from_seed_phrase(
        mnemonic: &str,
        password: Option<&str>,
    ) -> Result<Self, IdentityError> {
        let seed_phrase = SeedPhrase::from_phrase(mnemonic)?;
        let master_seed = seed_phrase.to_seed(password.unwrap_or(""));

        // Derive signing keys (for authentication)
        let signing_keys = Self::derive_signing_keys(&master_seed)?;

        // Derive encryption key (for file encryption)
        let encryption_key = Self::derive_encryption_key(&master_seed)?;

        // Derive node ID from public key
        let node_id = Self::derive_node_id(&signing_keys);

        Ok(Self {
            master_seed,
            signing_keys,
            encryption_key,
            node_id,
        })
    }

    /// Derive Ed25519 signing keys from master seed
    fn derive_signing_keys(master_seed: &[u8; 64]) -> Result<SigningKeyPair, IdentityError> {
        use hkdf::Hkdf;
        use sha2::Sha256;

        let hk = Hkdf::<Sha256>::new(Some(b"cloudp2p-signing"), master_seed);
        let mut signing_seed = [0u8; 32];
        hk.expand(b"ed25519-signing-key", &mut signing_seed)
            .map_err(|e| IdentityError::KeyDerivation(e.to_string()))?;

        let signing_key = SigningKey::from_bytes(&signing_seed);
        let verifying_key = signing_key.verifying_key();

        Ok(SigningKeyPair {
            signing_key,
            verifying_key,
        })
    }

    /// Derive AES-256 encryption key from master seed
    fn derive_encryption_key(master_seed: &[u8; 64]) -> Result<EncryptionKey, IdentityError> {
        use hkdf::Hkdf;
        use sha2::Sha256;

        let hk = Hkdf::<Sha256>::new(Some(b"cloudp2p-encryption"), master_seed);
        let mut enc_key = [0u8; 32];
        hk.expand(b"aes-256-gcm-key", &mut enc_key)
            .map_err(|e| IdentityError::KeyDerivation(e.to_string()))?;

        Ok(EncryptionKey::new(enc_key))
    }

    /// Derive node ID from public signing key
    fn derive_node_id(signing_keys: &SigningKeyPair) -> [u8; 32] {
        use sha2::{Sha256, Digest};
        let mut hasher = Sha256::new();
        hasher.update(signing_keys.verifying_key.as_bytes());
        hasher.finalize().into()
    }

    /// Get public ID (can be shared with others)
    pub fn public_id(&self) -> String {
        bs58::encode(&self.node_id).into_string()
    }

    /// Get the signing key pair
    pub fn signing_keys(&self) -> &SigningKeyPair {
        &self.signing_keys
    }

    /// Get the encryption key
    pub fn encryption_key(&self) -> &EncryptionKey {
        &self.encryption_key
    }

    /// Get the node ID bytes
    pub fn node_id(&self) -> &[u8; 32] {
        &self.node_id
    }

    /// Sign a message
    pub fn sign(&self, message: &[u8]) -> Vec<u8> {
        use ed25519_dalek::Signer;
        self.signing_keys.signing_key.sign(message).to_bytes().to_vec()
    }

    /// Verify a signature
    pub fn verify(&self, message: &[u8], signature: &[u8]) -> bool {
        use ed25519_dalek::{Signature, Verifier};
        if signature.len() != 64 {
            return false;
        }
        let sig_bytes: [u8; 64] = signature.try_into().unwrap();
        let sig = Signature::from_bytes(&sig_bytes);
        self.signing_keys.verifying_key.verify(message, &sig).is_ok()
    }

    /// Encrypt data with the user's encryption key
    pub fn encrypt(&self, plaintext: &[u8]) -> Result<Vec<u8>, IdentityError> {
        self.encryption_key.encrypt(plaintext).map_err(Into::into)
    }

    /// Decrypt data with the user's encryption key
    pub fn decrypt(&self, ciphertext: &[u8]) -> Result<Vec<u8>, IdentityError> {
        self.encryption_key.decrypt(ciphertext).map_err(Into::into)
    }

    /// Generate a heartbeat message (to prove liveness)
    pub fn generate_heartbeat(&self) -> HeartbeatMessage {
        let timestamp = chrono::Utc::now().timestamp();
        let message = format!("heartbeat:{}:{}", self.public_id(), timestamp);
        let signature = self.sign(message.as_bytes());

        HeartbeatMessage {
            node_id: self.public_id(),
            timestamp,
            signature,
        }
    }
}

/// Heartbeat message for proving liveness (avoids 90-day expiration)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HeartbeatMessage {
    pub node_id: String,
    pub timestamp: i64,
    pub signature: Vec<u8>,
}

impl HeartbeatMessage {
    /// Verify the heartbeat signature
    pub fn verify(&self, verifying_key: &VerifyingKey) -> bool {
        use ed25519_dalek::{Signature, Verifier};

        let message = format!("heartbeat:{}:{}", self.node_id, self.timestamp);

        if self.signature.len() != 64 {
            return false;
        }
        let sig_bytes: [u8; 64] = self.signature.clone().try_into().unwrap();
        let sig = Signature::from_bytes(&sig_bytes);

        verifying_key.verify(message.as_bytes(), &sig).is_ok()
    }

    /// Check if heartbeat is recent (within allowed window)
    pub fn is_recent(&self, max_age_seconds: i64) -> bool {
        let now = chrono::Utc::now().timestamp();
        (now - self.timestamp).abs() < max_age_seconds
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_identity_generation() {
        let (identity, seed_phrase) = UserIdentity::generate(Some("password123")).unwrap();

        // Seed phrase should have 12 words (BIP39 standard)
        assert_eq!(seed_phrase.split_whitespace().count(), 12);

        // Public ID should not be empty
        assert!(!identity.public_id().is_empty());
    }

    #[test]
    fn test_identity_recovery() {
        let (original, seed_phrase) = UserIdentity::generate(Some("password123")).unwrap();

        let recovered = UserIdentity::from_seed_phrase(&seed_phrase, Some("password123")).unwrap();

        // Same seed phrase + password = same identity
        assert_eq!(original.public_id(), recovered.public_id());
    }

    #[test]
    fn test_sign_verify() {
        let (identity, _) = UserIdentity::generate(None).unwrap();

        let message = b"Hello, CloudP2P!";
        let signature = identity.sign(message);

        assert!(identity.verify(message, &signature));
        assert!(!identity.verify(b"Wrong message", &signature));
    }

    #[test]
    fn test_encrypt_decrypt() {
        let (identity, _) = UserIdentity::generate(None).unwrap();

        let plaintext = b"Secret file content";
        let ciphertext = identity.encrypt(plaintext).unwrap();
        let decrypted = identity.decrypt(&ciphertext).unwrap();

        assert_eq!(plaintext.to_vec(), decrypted);
    }

    #[test]
    fn test_heartbeat() {
        let (identity, _) = UserIdentity::generate(None).unwrap();

        let heartbeat = identity.generate_heartbeat();

        assert!(heartbeat.verify(&identity.signing_keys().verifying_key));
        assert!(heartbeat.is_recent(60)); // Within 60 seconds
    }
}
