//! Cryptography Module - End-to-end encryption for CloudP2P
//!
//! Provides AES-256-GCM encryption, Ed25519 signatures, and secure key derivation.

pub mod encryption;
mod hashing;

pub use encryption::{EncryptionKey, FileEncryptor, EncryptedFile};
pub use hashing::ContentHash;

use ed25519_dalek::{SigningKey, VerifyingKey};
use thiserror::Error;

#[derive(Error, Debug)]
pub enum CryptoError {
    #[error("Encryption failed: {0}")]
    EncryptionFailed(String),

    #[error("Decryption failed: {0}")]
    DecryptionFailed(String),

    #[error("Invalid key: {0}")]
    InvalidKey(String),

    #[error("Invalid data: {0}")]
    InvalidData(String),

    #[error("Signature verification failed")]
    SignatureVerificationFailed,
}

/// Signing key pair (Ed25519)
#[derive(Clone)]
pub struct SigningKeyPair {
    pub signing_key: SigningKey,
    pub verifying_key: VerifyingKey,
}

impl SigningKeyPair {
    /// Sign a message
    pub fn sign(&self, message: &[u8]) -> Vec<u8> {
        use ed25519_dalek::Signer;
        self.signing_key.sign(message).to_bytes().to_vec()
    }

    /// Verify a signature
    pub fn verify(&self, message: &[u8], signature: &[u8]) -> Result<(), CryptoError> {
        use ed25519_dalek::{Signature, Verifier};

        if signature.len() != 64 {
            return Err(CryptoError::InvalidData("Invalid signature length".into()));
        }

        let sig_bytes: [u8; 64] = signature.try_into().unwrap();
        let sig = Signature::from_bytes(&sig_bytes);

        self.verifying_key
            .verify(message, &sig)
            .map_err(|_| CryptoError::SignatureVerificationFailed)
    }
}

/// Secure random bytes generator
pub fn random_bytes(len: usize) -> Vec<u8> {
    use rand::RngCore;
    let mut bytes = vec![0u8; len];
    rand::thread_rng().fill_bytes(&mut bytes);
    bytes
}

/// Secure random 32-byte array
pub fn random_32_bytes() -> [u8; 32] {
    use rand::RngCore;
    let mut bytes = [0u8; 32];
    rand::thread_rng().fill_bytes(&mut bytes);
    bytes
}

/// Derive a key from password using Argon2id
pub fn derive_key_from_password(password: &[u8], salt: &[u8]) -> Result<[u8; 32], CryptoError> {
    use argon2::{Argon2, PasswordHasher};
    use argon2::password_hash::SaltString;

    // Argon2id with recommended parameters
    let argon2 = Argon2::default();

    // Create salt string (needs to be valid base64)
    let salt_b64 = base64::Engine::encode(&base64::engine::general_purpose::STANDARD_NO_PAD, salt);
    let salt_str = SaltString::from_b64(&salt_b64)
        .map_err(|e| CryptoError::InvalidKey(e.to_string()))?;

    let hash = argon2
        .hash_password(password, &salt_str)
        .map_err(|e| CryptoError::InvalidKey(e.to_string()))?;

    let hash_bytes = hash.hash.ok_or(CryptoError::InvalidKey("No hash output".into()))?;
    let bytes = hash_bytes.as_bytes();

    let mut key = [0u8; 32];
    key.copy_from_slice(&bytes[..32]);
    Ok(key)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_random_bytes() {
        let bytes1 = random_bytes(32);
        let bytes2 = random_bytes(32);

        assert_eq!(bytes1.len(), 32);
        assert_ne!(bytes1, bytes2); // Should be different (with overwhelming probability)
    }

    #[test]
    fn test_signing_keypair() {
        let seed = random_32_bytes();
        let signing_key = SigningKey::from_bytes(&seed);
        let verifying_key = signing_key.verifying_key();

        let keypair = SigningKeyPair {
            signing_key,
            verifying_key,
        };

        let message = b"Test message";
        let signature = keypair.sign(message);

        assert!(keypair.verify(message, &signature).is_ok());
        assert!(keypair.verify(b"Wrong message", &signature).is_err());
    }
}
