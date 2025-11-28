//! File Manager - High-level file operations
//!
//! Handles the complete lifecycle of files: encryption, erasure coding,
//! distribution to peers, and retrieval.

use super::{ErasureConfig, ErasureDecoder, ErasureEncoder, StorageError};
use crate::crypto::{ContentHash, EncryptionKey, FileEncryptor};
use crate::identity::UserIdentity;

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use tokio::sync::mpsc;

/// File metadata stored locally and in DHT
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileMetadata {
    /// Unique file ID (content hash of original file)
    pub file_id: String,

    /// Original filename
    pub filename: String,

    /// Original file size (bytes)
    pub size: u64,

    /// MIME type
    pub mime_type: String,

    /// Content hash of encrypted data
    pub encrypted_hash: String,

    /// Erasure coding config used
    pub erasure_config: ErasureConfig,

    /// Shard IDs and their storage locations
    pub shards: Vec<ShardLocation>,

    /// Creation timestamp
    pub created_at: i64,

    /// Last modified timestamp
    pub modified_at: i64,

    /// Owner's public ID
    pub owner_id: String,

    /// Is file shared with others?
    pub is_shared: bool,

    /// Share recipients (if shared)
    pub shared_with: Vec<String>,

    /// File encryption key (encrypted with owner's master key)
    pub encrypted_file_key: Vec<u8>,

    /// Parent folder ID (for organization)
    pub folder_id: Option<String>,

    /// Custom tags
    pub tags: Vec<String>,
}

/// Location of a shard in the network
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ShardLocation {
    /// Shard index
    pub index: usize,

    /// Shard ID (for DHT lookup)
    pub shard_id: String,

    /// Peer IDs storing this shard
    pub peers: Vec<String>,

    /// Shard size
    pub size: u64,

    /// Content hash for verification
    pub hash: String,
}

/// Upload progress tracking
#[derive(Debug, Clone)]
pub struct UploadProgress {
    pub file_id: String,
    pub filename: String,
    pub total_bytes: u64,
    pub uploaded_bytes: u64,
    pub shards_total: usize,
    pub shards_uploaded: usize,
    pub stage: UploadStage,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UploadStage {
    Reading,
    Encrypting,
    Encoding,
    Distributing,
    Verifying,
    Complete,
    Failed,
}

/// Download progress tracking
#[derive(Debug, Clone)]
pub struct DownloadProgress {
    pub file_id: String,
    pub filename: String,
    pub total_bytes: u64,
    pub downloaded_bytes: u64,
    pub shards_total: usize,
    pub shards_downloaded: usize,
    pub stage: DownloadStage,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DownloadStage {
    Locating,
    Downloading,
    Reconstructing,
    Decrypting,
    Complete,
    Failed,
}

/// Manages file uploads, downloads, and metadata
pub struct FileManager {
    /// User identity for encryption
    identity: UserIdentity,

    /// Local file index
    file_index: HashMap<String, FileMetadata>,

    /// Erasure coding config
    erasure_config: ErasureConfig,

    /// Local cache path
    cache_path: PathBuf,

    /// Progress sender for uploads
    upload_progress_tx: Option<mpsc::UnboundedSender<UploadProgress>>,

    /// Progress sender for downloads
    download_progress_tx: Option<mpsc::UnboundedSender<DownloadProgress>>,
}

impl FileManager {
    /// Create a new file manager
    pub fn new(identity: UserIdentity, cache_path: PathBuf) -> Self {
        Self {
            identity,
            file_index: HashMap::new(),
            erasure_config: ErasureConfig::default(),
            cache_path,
            upload_progress_tx: None,
            download_progress_tx: None,
        }
    }

    /// Set erasure config
    pub fn with_erasure_config(mut self, config: ErasureConfig) -> Self {
        self.erasure_config = config;
        self
    }

    /// Set upload progress channel
    pub fn with_upload_progress(
        mut self,
        tx: mpsc::UnboundedSender<UploadProgress>,
    ) -> Self {
        self.upload_progress_tx = Some(tx);
        self
    }

    /// Set download progress channel
    pub fn with_download_progress(
        mut self,
        tx: mpsc::UnboundedSender<DownloadProgress>,
    ) -> Self {
        self.download_progress_tx = Some(tx);
        self
    }

    /// Prepare a file for upload (encrypt and encode)
    pub async fn prepare_upload(
        &self,
        file_path: &str,
        filename: &str,
    ) -> Result<PreparedFile, StorageError> {
        // Read file
        let data = tokio::fs::read(file_path)
            .await
            .map_err(|e| StorageError::Io(e))?;

        let original_size = data.len();
        let original_hash = ContentHash::hash(&data);

        // Generate per-file encryption key
        let file_key = EncryptionKey::generate();

        // Encrypt file
        let encryptor = FileEncryptor::new(file_key.clone());
        let encrypted = encryptor
            .encrypt_file(&data)
            .map_err(|e| StorageError::Encryption(e.to_string()))?;

        let encrypted_data = encrypted.to_bytes()
            .map_err(|e| StorageError::Serialization(e.to_string()))?;

        let encrypted_hash = ContentHash::hash(&encrypted_data);

        // Erasure encode
        let encoder = ErasureEncoder::new(self.erasure_config)?;
        let shards = encoder.encode(&encrypted_data)?;

        // Encrypt the file key with user's master key
        let encrypted_file_key = self
            .identity
            .encrypt(file_key.as_bytes())
            .map_err(|e| StorageError::Encryption(e.to_string()))?;

        // Determine MIME type
        let mime_type = mime_guess::from_path(filename)
            .first_or_octet_stream()
            .to_string();

        let now = chrono::Utc::now().timestamp();

        let shard_locations: Vec<ShardLocation> = shards
            .iter()
            .map(|s| ShardLocation {
                index: s.index,
                shard_id: s.id(&original_hash.to_base58()),
                peers: vec![], // Will be filled during distribution
                size: s.data.len() as u64,
                hash: ContentHash::hash(&s.data).to_base58(),
            })
            .collect();

        let metadata = FileMetadata {
            file_id: original_hash.to_base58(),
            filename: filename.to_string(),
            size: original_size as u64,
            mime_type,
            encrypted_hash: encrypted_hash.to_base58(),
            erasure_config: self.erasure_config,
            shards: shard_locations,
            created_at: now,
            modified_at: now,
            owner_id: self.identity.public_id(),
            is_shared: false,
            shared_with: vec![],
            encrypted_file_key,
            folder_id: None,
            tags: vec![],
        };

        Ok(PreparedFile { metadata, shards })
    }

    /// Reconstruct a file from shards
    pub async fn reconstruct_file(
        &self,
        metadata: &FileMetadata,
        shard_data: Vec<Option<Vec<u8>>>,
    ) -> Result<Vec<u8>, StorageError> {
        // Verify we have enough shards
        let available = shard_data.iter().filter(|s| s.is_some()).count();
        if available < metadata.erasure_config.data_shards {
            return Err(StorageError::InsufficientFragments {
                have: available,
                need: metadata.erasure_config.data_shards,
            });
        }

        // Convert to Shard format for decoder
        let shards: Vec<Option<super::erasure::Shard>> = shard_data
            .into_iter()
            .enumerate()
            .map(|(i, opt)| {
                opt.map(|data| super::erasure::Shard {
                    index: i,
                    data,
                    is_parity: i >= metadata.erasure_config.data_shards,
                    original_size: 0, // Not needed for decoding
                })
            })
            .collect();

        // Decode erasure coding
        let decoder = ErasureDecoder::new(metadata.erasure_config)?;

        // We need to know the encrypted size for proper reconstruction
        let encrypted_size: usize = metadata.shards.iter().map(|s| s.size as usize).sum();
        let estimated_size = encrypted_size / metadata.erasure_config.total_shards()
            * metadata.erasure_config.data_shards;

        let encrypted_data = decoder.decode(shards, estimated_size)?;

        // Decrypt file key
        let file_key_bytes = self
            .identity
            .decrypt(&metadata.encrypted_file_key)
            .map_err(|e| StorageError::Encryption(e.to_string()))?;

        let file_key_arr: [u8; 32] = file_key_bytes
            .try_into()
            .map_err(|_| StorageError::Encryption("Invalid file key length".into()))?;

        let file_key = EncryptionKey::new(file_key_arr);

        // Parse encrypted file structure
        let encrypted_file = crate::crypto::EncryptedFile::from_bytes(&encrypted_data)
            .map_err(|e| StorageError::Encryption(e.to_string()))?;

        // Decrypt file
        let encryptor = FileEncryptor::new(file_key);
        let plaintext = encryptor
            .decrypt_file(&encrypted_file)
            .map_err(|e| StorageError::Encryption(e.to_string()))?;

        // Verify content hash
        let hash = ContentHash::hash(&plaintext);
        if hash.to_base58() != metadata.file_id {
            return Err(StorageError::IntegrityCheckFailed);
        }

        Ok(plaintext)
    }

    /// Add file metadata to local index
    pub fn add_to_index(&mut self, metadata: FileMetadata) {
        self.file_index
            .insert(metadata.file_id.clone(), metadata);
    }

    /// Get file metadata by ID
    pub fn get_metadata(&self, file_id: &str) -> Option<&FileMetadata> {
        self.file_index.get(file_id)
    }

    /// List all files
    pub fn list_files(&self) -> Vec<&FileMetadata> {
        self.file_index.values().collect()
    }

    /// List files in a folder
    pub fn list_folder(&self, folder_id: Option<&str>) -> Vec<&FileMetadata> {
        self.file_index
            .values()
            .filter(|f| f.folder_id.as_deref() == folder_id)
            .collect()
    }

    /// Search files by name or tags
    pub fn search(&self, query: &str) -> Vec<&FileMetadata> {
        let query_lower = query.to_lowercase();
        self.file_index
            .values()
            .filter(|f| {
                f.filename.to_lowercase().contains(&query_lower)
                    || f.tags.iter().any(|t| t.to_lowercase().contains(&query_lower))
            })
            .collect()
    }

    /// Delete file from index
    pub fn remove_from_index(&mut self, file_id: &str) -> Option<FileMetadata> {
        self.file_index.remove(file_id)
    }

    /// Get total storage used
    pub fn total_storage_used(&self) -> u64 {
        self.file_index.values().map(|f| f.size).sum()
    }

    /// Get file count
    pub fn file_count(&self) -> usize {
        self.file_index.len()
    }

    /// Export index to JSON
    pub fn export_index(&self) -> Result<String, StorageError> {
        serde_json::to_string_pretty(&self.file_index)
            .map_err(|e| StorageError::Serialization(e.to_string()))
    }

    /// Import index from JSON
    pub fn import_index(&mut self, json: &str) -> Result<usize, StorageError> {
        let index: HashMap<String, FileMetadata> = serde_json::from_str(json)
            .map_err(|e| StorageError::Serialization(e.to_string()))?;

        let count = index.len();
        self.file_index.extend(index);
        Ok(count)
    }
}

/// Prepared file ready for distribution
pub struct PreparedFile {
    /// File metadata
    pub metadata: FileMetadata,

    /// Encoded shards
    pub shards: Vec<super::erasure::Shard>,
}

impl PreparedFile {
    /// Get shard by index
    pub fn get_shard(&self, index: usize) -> Option<&super::erasure::Shard> {
        self.shards.get(index)
    }

    /// Get all shard data for distribution
    pub fn shard_data(&self) -> Vec<(&str, &[u8])> {
        self.metadata
            .shards
            .iter()
            .zip(self.shards.iter())
            .map(|(loc, shard)| (loc.shard_id.as_str(), shard.data.as_slice()))
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn create_test_identity() -> UserIdentity {
        UserIdentity::generate(Some("test")).unwrap().0
    }

    #[tokio::test]
    async fn test_prepare_upload() {
        let temp_dir = TempDir::new().unwrap();
        let identity = create_test_identity();

        // Create test file
        let test_file = temp_dir.path().join("test.txt");
        tokio::fs::write(&test_file, b"Hello, CloudP2P! This is a test file.")
            .await
            .unwrap();

        let manager = FileManager::new(identity, temp_dir.path().to_path_buf());

        let prepared = manager
            .prepare_upload(test_file.to_str().unwrap(), "test.txt")
            .await
            .unwrap();

        assert_eq!(prepared.metadata.filename, "test.txt");
        assert_eq!(prepared.shards.len(), 14); // 10 data + 4 parity
        assert!(prepared.metadata.size > 0);
    }

    #[tokio::test]
    async fn test_full_cycle() {
        let temp_dir = TempDir::new().unwrap();
        let identity = create_test_identity();

        // Create test file
        let original_data = b"Hello, CloudP2P! This is a test file for full cycle testing.";
        let test_file = temp_dir.path().join("test.txt");
        tokio::fs::write(&test_file, original_data).await.unwrap();

        let manager = FileManager::new(identity, temp_dir.path().to_path_buf());

        // Prepare upload
        let prepared = manager
            .prepare_upload(test_file.to_str().unwrap(), "test.txt")
            .await
            .unwrap();

        // Simulate getting shards back (all available)
        let shard_data: Vec<Option<Vec<u8>>> = prepared
            .shards
            .iter()
            .map(|s| Some(s.data.clone()))
            .collect();

        // Reconstruct
        let reconstructed = manager
            .reconstruct_file(&prepared.metadata, shard_data)
            .await
            .unwrap();

        assert_eq!(reconstructed, original_data.to_vec());
    }

    #[tokio::test]
    async fn test_reconstruct_with_missing_shards() {
        let temp_dir = TempDir::new().unwrap();
        let identity = create_test_identity();

        // Create test file
        let original_data: Vec<u8> = (0..10000).map(|i| (i % 256) as u8).collect();
        let test_file = temp_dir.path().join("test.bin");
        tokio::fs::write(&test_file, &original_data).await.unwrap();

        let manager = FileManager::new(identity, temp_dir.path().to_path_buf());

        // Prepare upload
        let prepared = manager
            .prepare_upload(test_file.to_str().unwrap(), "test.bin")
            .await
            .unwrap();

        // Simulate losing 4 shards (maximum allowed with default config)
        let mut shard_data: Vec<Option<Vec<u8>>> = prepared
            .shards
            .iter()
            .map(|s| Some(s.data.clone()))
            .collect();

        shard_data[0] = None;
        shard_data[3] = None;
        shard_data[7] = None;
        shard_data[12] = None;

        // Reconstruct
        let reconstructed = manager
            .reconstruct_file(&prepared.metadata, shard_data)
            .await
            .unwrap();

        assert_eq!(reconstructed, original_data);
    }
}
