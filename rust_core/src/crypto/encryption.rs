//! File encryption using AES-256-GCM
//!
//! Provides secure file encryption with authenticated encryption.

use super::CryptoError;
use aes_gcm::{
    aead::{Aead, KeyInit, OsRng},
    Aes256Gcm, Key, Nonce,
};
use rand::RngCore;

const NONCE_SIZE: usize = 12;
const KEY_SIZE: usize = 32;

/// AES-256-GCM encryption key
#[derive(Clone)]
pub struct EncryptionKey {
    key: [u8; KEY_SIZE],
}

impl EncryptionKey {
    /// Create a new encryption key from bytes
    pub fn new(key: [u8; KEY_SIZE]) -> Self {
        Self { key }
    }

    /// Generate a random encryption key
    pub fn generate() -> Self {
        let mut key = [0u8; KEY_SIZE];
        OsRng.fill_bytes(&mut key);
        Self { key }
    }

    /// Encrypt data with AES-256-GCM
    /// Returns: nonce (12 bytes) || ciphertext || tag (16 bytes)
    pub fn encrypt(&self, plaintext: &[u8]) -> Result<Vec<u8>, CryptoError> {
        let cipher = Aes256Gcm::new(Key::<Aes256Gcm>::from_slice(&self.key));

        // Generate random nonce
        let mut nonce_bytes = [0u8; NONCE_SIZE];
        OsRng.fill_bytes(&mut nonce_bytes);
        let nonce = Nonce::from_slice(&nonce_bytes);

        // Encrypt
        let ciphertext = cipher
            .encrypt(nonce, plaintext)
            .map_err(|e| CryptoError::EncryptionFailed(e.to_string()))?;

        // Prepend nonce to ciphertext
        let mut result = Vec::with_capacity(NONCE_SIZE + ciphertext.len());
        result.extend_from_slice(&nonce_bytes);
        result.extend_from_slice(&ciphertext);

        Ok(result)
    }

    /// Decrypt data encrypted with AES-256-GCM
    /// Input format: nonce (12 bytes) || ciphertext || tag (16 bytes)
    pub fn decrypt(&self, ciphertext: &[u8]) -> Result<Vec<u8>, CryptoError> {
        if ciphertext.len() < NONCE_SIZE + 16 {
            return Err(CryptoError::DecryptionFailed("Ciphertext too short".into()));
        }

        let cipher = Aes256Gcm::new(Key::<Aes256Gcm>::from_slice(&self.key));

        // Extract nonce and actual ciphertext
        let nonce = Nonce::from_slice(&ciphertext[..NONCE_SIZE]);
        let encrypted_data = &ciphertext[NONCE_SIZE..];

        // Decrypt
        cipher
            .decrypt(nonce, encrypted_data)
            .map_err(|e| CryptoError::DecryptionFailed(e.to_string()))
    }

    /// Get the raw key bytes (be careful with this!)
    pub fn as_bytes(&self) -> &[u8; KEY_SIZE] {
        &self.key
    }
}

/// File encryptor with streaming support for large files
pub struct FileEncryptor {
    key: EncryptionKey,
    chunk_size: usize,
}

impl FileEncryptor {
    /// Create a new file encryptor
    pub fn new(key: EncryptionKey) -> Self {
        Self {
            key,
            chunk_size: 64 * 1024, // 64 KB chunks
        }
    }

    /// Set custom chunk size
    pub fn with_chunk_size(mut self, size: usize) -> Self {
        self.chunk_size = size;
        self
    }

    /// Encrypt a file in chunks
    /// Each chunk is independently encrypted for random access
    pub fn encrypt_file(&self, data: &[u8]) -> Result<EncryptedFile, CryptoError> {
        let mut chunks = Vec::new();
        let mut chunk_offsets = Vec::new();
        let mut current_offset = 0;

        for chunk in data.chunks(self.chunk_size) {
            let encrypted_chunk = self.key.encrypt(chunk)?;
            chunk_offsets.push(current_offset);
            current_offset += encrypted_chunk.len();
            chunks.push(encrypted_chunk);
        }

        // Flatten chunks into single buffer
        let total_size: usize = chunks.iter().map(|c| c.len()).sum();
        let mut encrypted_data = Vec::with_capacity(total_size);
        for chunk in &chunks {
            encrypted_data.extend_from_slice(chunk);
        }

        Ok(EncryptedFile {
            data: encrypted_data,
            chunk_offsets,
            original_size: data.len(),
            chunk_size: self.chunk_size,
        })
    }

    /// Decrypt an entire file
    pub fn decrypt_file(&self, encrypted: &EncryptedFile) -> Result<Vec<u8>, CryptoError> {
        let mut plaintext = Vec::with_capacity(encrypted.original_size);

        for i in 0..encrypted.chunk_offsets.len() {
            let start = encrypted.chunk_offsets[i];
            let end = if i + 1 < encrypted.chunk_offsets.len() {
                encrypted.chunk_offsets[i + 1]
            } else {
                encrypted.data.len()
            };

            let chunk = self.key.decrypt(&encrypted.data[start..end])?;
            plaintext.extend_from_slice(&chunk);
        }

        Ok(plaintext)
    }

    /// Decrypt a specific chunk (for random access)
    pub fn decrypt_chunk(&self, encrypted: &EncryptedFile, chunk_index: usize) -> Result<Vec<u8>, CryptoError> {
        if chunk_index >= encrypted.chunk_offsets.len() {
            return Err(CryptoError::InvalidData("Chunk index out of bounds".into()));
        }

        let start = encrypted.chunk_offsets[chunk_index];
        let end = if chunk_index + 1 < encrypted.chunk_offsets.len() {
            encrypted.chunk_offsets[chunk_index + 1]
        } else {
            encrypted.data.len()
        };

        self.key.decrypt(&encrypted.data[start..end])
    }
}

/// Encrypted file with chunk metadata
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct EncryptedFile {
    /// Encrypted data
    pub data: Vec<u8>,

    /// Offsets of each encrypted chunk
    pub chunk_offsets: Vec<usize>,

    /// Original file size
    pub original_size: usize,

    /// Chunk size used for encryption
    pub chunk_size: usize,
}

impl EncryptedFile {
    /// Get the number of chunks
    pub fn chunk_count(&self) -> usize {
        self.chunk_offsets.len()
    }

    /// Get encrypted size
    pub fn encrypted_size(&self) -> usize {
        self.data.len()
    }

    /// Serialize to bytes
    pub fn to_bytes(&self) -> Result<Vec<u8>, CryptoError> {
        bincode::serialize(self)
            .map_err(|e| CryptoError::InvalidData(e.to_string()))
    }

    /// Deserialize from bytes
    pub fn from_bytes(bytes: &[u8]) -> Result<Self, CryptoError> {
        bincode::deserialize(bytes)
            .map_err(|e| CryptoError::InvalidData(e.to_string()))
    }
}

/// Per-file encryption key (derived from master key + file ID)
pub fn derive_file_key(master_key: &EncryptionKey, file_id: &[u8]) -> EncryptionKey {
    use hkdf::Hkdf;
    use sha2::Sha256;

    let hk = Hkdf::<Sha256>::new(Some(file_id), master_key.as_bytes());
    let mut file_key = [0u8; KEY_SIZE];
    hk.expand(b"cloudp2p-file-key", &mut file_key).unwrap();

    EncryptionKey::new(file_key)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_encrypt_decrypt() {
        let key = EncryptionKey::generate();
        let plaintext = b"Hello, CloudP2P! This is a secret message.";

        let ciphertext = key.encrypt(plaintext).unwrap();
        let decrypted = key.decrypt(&ciphertext).unwrap();

        assert_eq!(plaintext.to_vec(), decrypted);
    }

    #[test]
    fn test_different_keys_fail() {
        let key1 = EncryptionKey::generate();
        let key2 = EncryptionKey::generate();

        let plaintext = b"Secret data";
        let ciphertext = key1.encrypt(plaintext).unwrap();

        // Decryption with wrong key should fail
        assert!(key2.decrypt(&ciphertext).is_err());
    }

    #[test]
    fn test_file_encryptor() {
        let key = EncryptionKey::generate();
        let encryptor = FileEncryptor::new(key.clone()).with_chunk_size(1024);

        // Create test data larger than chunk size
        let data: Vec<u8> = (0..5000).map(|i| (i % 256) as u8).collect();

        let encrypted = encryptor.encrypt_file(&data).unwrap();
        let decrypted = encryptor.decrypt_file(&encrypted).unwrap();

        assert_eq!(data, decrypted);
        assert!(encrypted.chunk_count() > 1);
    }

    #[test]
    fn test_chunk_random_access() {
        let key = EncryptionKey::generate();
        let encryptor = FileEncryptor::new(key.clone()).with_chunk_size(100);

        let data: Vec<u8> = (0..500).map(|i| (i % 256) as u8).collect();
        let encrypted = encryptor.encrypt_file(&data).unwrap();

        // Decrypt individual chunks
        let chunk0 = encryptor.decrypt_chunk(&encrypted, 0).unwrap();
        let chunk2 = encryptor.decrypt_chunk(&encrypted, 2).unwrap();

        assert_eq!(&data[0..100], &chunk0[..]);
        assert_eq!(&data[200..300], &chunk2[..]);
    }

    #[test]
    fn test_derive_file_key() {
        let master_key = EncryptionKey::generate();
        let file_id1 = b"file-001";
        let file_id2 = b"file-002";

        let key1 = derive_file_key(&master_key, file_id1);
        let key2 = derive_file_key(&master_key, file_id2);
        let key1_again = derive_file_key(&master_key, file_id1);

        // Different file IDs = different keys
        assert_ne!(key1.as_bytes(), key2.as_bytes());

        // Same file ID = same key
        assert_eq!(key1.as_bytes(), key1_again.as_bytes());
    }
}
