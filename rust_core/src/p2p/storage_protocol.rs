//! Storage protocol handler - manages fragment storage and retrieval

use super::{P2PError, StorageRequest, StorageResponse, protocol::ErrorCode};
use crate::crypto::{ContentHash, EncryptionKey};
use crate::identity::UserIdentity;

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;

/// Manages local storage of fragments (both own and others')
pub struct StorageManager {
    /// Base path for storage
    storage_path: PathBuf,

    /// Maximum storage offered to network (bytes)
    max_storage_bytes: u64,

    /// Currently used storage (bytes)
    used_storage_bytes: u64,

    /// Index of stored fragments
    fragment_index: HashMap<String, StoredFragment>,

    /// User identity for signing
    identity: Option<UserIdentity>,
}

/// Information about a stored fragment
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StoredFragment {
    /// Fragment ID
    pub fragment_id: String,

    /// Owner's public ID
    pub owner_id: String,

    /// Size in bytes
    pub size_bytes: u64,

    /// Content hash for integrity
    pub content_hash: String,

    /// Creation timestamp
    pub created_at: i64,

    /// Expiration timestamp
    pub expires_at: i64,

    /// Local file path (relative to storage_path)
    pub local_path: String,

    /// Access count
    pub access_count: u64,

    /// Last access timestamp
    pub last_accessed: i64,
}

impl StorageManager {
    /// Create a new storage manager
    pub fn new(storage_path: PathBuf, max_storage_bytes: u64) -> Self {
        Self {
            storage_path,
            max_storage_bytes,
            used_storage_bytes: 0,
            fragment_index: HashMap::new(),
            identity: None,
        }
    }

    /// Set the user identity
    pub fn set_identity(&mut self, identity: UserIdentity) {
        self.identity = Some(identity);
    }

    /// Initialize storage (create directories, load index)
    pub async fn initialize(&mut self) -> Result<(), P2PError> {
        // Create storage directories
        let fragments_dir = self.storage_path.join("fragments");
        let index_path = self.storage_path.join("index.json");

        tokio::fs::create_dir_all(&fragments_dir)
            .await
            .map_err(|e| P2PError::Protocol(format!("Failed to create storage dir: {}", e)))?;

        // Load existing index if present
        if index_path.exists() {
            let index_data = tokio::fs::read_to_string(&index_path)
                .await
                .map_err(|e| P2PError::Protocol(format!("Failed to read index: {}", e)))?;

            self.fragment_index = serde_json::from_str(&index_data)
                .map_err(|e| P2PError::Protocol(format!("Failed to parse index: {}", e)))?;

            // Calculate used storage
            self.used_storage_bytes = self.fragment_index.values().map(|f| f.size_bytes).sum();
        }

        Ok(())
    }

    /// Save the index to disk
    async fn save_index(&self) -> Result<(), P2PError> {
        let index_path = self.storage_path.join("index.json");
        let index_data = serde_json::to_string_pretty(&self.fragment_index)
            .map_err(|e| P2PError::Protocol(format!("Failed to serialize index: {}", e)))?;

        tokio::fs::write(&index_path, index_data)
            .await
            .map_err(|e| P2PError::Protocol(format!("Failed to write index: {}", e)))?;

        Ok(())
    }

    /// Store a fragment
    pub async fn store_fragment(
        &mut self,
        fragment_id: &str,
        owner_id: &str,
        data: &[u8],
        expires_at: i64,
    ) -> Result<StoredFragment, P2PError> {
        let size = data.len() as u64;

        // Check storage capacity
        if self.used_storage_bytes + size > self.max_storage_bytes {
            return Err(P2PError::Protocol("Insufficient storage space".into()));
        }

        // Calculate content hash
        let hash = ContentHash::hash(data);

        // Determine storage path
        let local_path = format!("fragments/{}/{}", &fragment_id[..2], fragment_id);
        let full_path = self.storage_path.join(&local_path);

        // Create parent directory
        if let Some(parent) = full_path.parent() {
            tokio::fs::create_dir_all(parent)
                .await
                .map_err(|e| P2PError::Protocol(format!("Failed to create dir: {}", e)))?;
        }

        // Write fragment to disk
        tokio::fs::write(&full_path, data)
            .await
            .map_err(|e| P2PError::Protocol(format!("Failed to write fragment: {}", e)))?;

        let now = chrono::Utc::now().timestamp();
        let fragment = StoredFragment {
            fragment_id: fragment_id.to_string(),
            owner_id: owner_id.to_string(),
            size_bytes: size,
            content_hash: hash.to_base58(),
            created_at: now,
            expires_at,
            local_path,
            access_count: 0,
            last_accessed: now,
        };

        // Update index
        self.fragment_index
            .insert(fragment_id.to_string(), fragment.clone());
        self.used_storage_bytes += size;

        // Save index
        self.save_index().await?;

        Ok(fragment)
    }

    /// Retrieve a fragment
    pub async fn retrieve_fragment(&mut self, fragment_id: &str) -> Result<Vec<u8>, P2PError> {
        let fragment = self
            .fragment_index
            .get_mut(fragment_id)
            .ok_or_else(|| P2PError::Protocol("Fragment not found".into()))?;

        // Check expiration
        let now = chrono::Utc::now().timestamp();
        if now > fragment.expires_at {
            // Delete expired fragment
            self.delete_fragment(fragment_id).await?;
            return Err(P2PError::Protocol("Fragment expired".into()));
        }

        // Update access stats
        fragment.access_count += 1;
        fragment.last_accessed = now;

        // Read from disk
        let full_path = self.storage_path.join(&fragment.local_path);
        let data = tokio::fs::read(&full_path)
            .await
            .map_err(|e| P2PError::Protocol(format!("Failed to read fragment: {}", e)))?;

        // Verify integrity
        let hash = ContentHash::hash(&data);
        if hash.to_base58() != fragment.content_hash {
            return Err(P2PError::Protocol("Fragment integrity check failed".into()));
        }

        Ok(data)
    }

    /// Delete a fragment
    pub async fn delete_fragment(&mut self, fragment_id: &str) -> Result<(), P2PError> {
        if let Some(fragment) = self.fragment_index.remove(fragment_id) {
            let full_path = self.storage_path.join(&fragment.local_path);

            if full_path.exists() {
                tokio::fs::remove_file(&full_path)
                    .await
                    .map_err(|e| P2PError::Protocol(format!("Failed to delete fragment: {}", e)))?;
            }

            self.used_storage_bytes = self.used_storage_bytes.saturating_sub(fragment.size_bytes);
            self.save_index().await?;
        }

        Ok(())
    }

    /// Extend fragment expiration (after heartbeat)
    pub async fn extend_fragment(
        &mut self,
        fragment_id: &str,
        new_expires_at: i64,
    ) -> Result<(), P2PError> {
        if let Some(fragment) = self.fragment_index.get_mut(fragment_id) {
            fragment.expires_at = new_expires_at;
            self.save_index().await?;
        }
        Ok(())
    }

    /// Extend all fragments for an owner
    pub async fn extend_owner_fragments(
        &mut self,
        owner_id: &str,
        additional_days: u32,
    ) -> Result<u32, P2PError> {
        let now = chrono::Utc::now().timestamp();
        let extension = additional_days as i64 * 24 * 60 * 60;
        let mut count = 0;

        for fragment in self.fragment_index.values_mut() {
            if fragment.owner_id == owner_id {
                fragment.expires_at = now + extension;
                count += 1;
            }
        }

        if count > 0 {
            self.save_index().await?;
        }

        Ok(count)
    }

    /// Clean up expired fragments
    pub async fn cleanup_expired(&mut self) -> Result<u32, P2PError> {
        let now = chrono::Utc::now().timestamp();
        let expired: Vec<String> = self
            .fragment_index
            .iter()
            .filter(|(_, f)| now > f.expires_at)
            .map(|(id, _)| id.clone())
            .collect();

        let count = expired.len() as u32;

        for fragment_id in expired {
            self.delete_fragment(&fragment_id).await?;
        }

        Ok(count)
    }

    /// Generate Proof of Storage for a challenge
    pub async fn prove_storage(
        &self,
        fragment_id: &str,
        challenge: &[u8],
    ) -> Result<Vec<u8>, P2PError> {
        let fragment = self
            .fragment_index
            .get(fragment_id)
            .ok_or_else(|| P2PError::Protocol("Fragment not found".into()))?;

        let full_path = self.storage_path.join(&fragment.local_path);
        let data = tokio::fs::read(&full_path)
            .await
            .map_err(|e| P2PError::Protocol(format!("Failed to read fragment: {}", e)))?;

        // Create proof: BLAKE3(data || challenge)
        let mut proof_data = data;
        proof_data.extend_from_slice(challenge);
        let proof = ContentHash::hash(&proof_data);

        Ok(proof.as_bytes().to_vec())
    }

    /// Verify Proof of Storage
    pub fn verify_storage_proof(
        expected_hash: &ContentHash,
        challenge: &[u8],
        proof: &[u8],
    ) -> bool {
        if proof.len() != 32 {
            return false;
        }

        // Reconstruct what the proof should be
        // This requires knowing the original data, which we don't have here
        // In practice, we'd use a more sophisticated PoSt scheme
        true // Simplified for now
    }

    /// Get storage statistics
    pub fn stats(&self) -> StorageStats {
        let now = chrono::Utc::now().timestamp();
        let expiring_soon = self
            .fragment_index
            .values()
            .filter(|f| f.expires_at - now < 7 * 24 * 60 * 60) // Within 7 days
            .count() as u64;

        StorageStats {
            total_offered: self.max_storage_bytes,
            used_bytes: self.used_storage_bytes,
            available_bytes: self.max_storage_bytes.saturating_sub(self.used_storage_bytes),
            fragment_count: self.fragment_index.len() as u64,
            unique_owners: self
                .fragment_index
                .values()
                .map(|f| &f.owner_id)
                .collect::<std::collections::HashSet<_>>()
                .len() as u64,
            fragments_expiring_soon: expiring_soon,
        }
    }

    /// Check if we have space for a fragment
    pub fn has_space(&self, size_bytes: u64) -> bool {
        self.used_storage_bytes + size_bytes <= self.max_storage_bytes
    }

    /// Get available space
    pub fn available_space(&self) -> u64 {
        self.max_storage_bytes.saturating_sub(self.used_storage_bytes)
    }
}

/// Storage statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StorageStats {
    pub total_offered: u64,
    pub used_bytes: u64,
    pub available_bytes: u64,
    pub fragment_count: u64,
    pub unique_owners: u64,
    pub fragments_expiring_soon: u64,
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[tokio::test]
    async fn test_store_retrieve_fragment() {
        let temp_dir = TempDir::new().unwrap();
        let mut manager = StorageManager::new(temp_dir.path().to_path_buf(), 1_000_000);
        manager.initialize().await.unwrap();

        let data = b"Test fragment data";
        let expires_at = chrono::Utc::now().timestamp() + 86400;

        let fragment = manager
            .store_fragment("frag-001", "owner-abc", data, expires_at)
            .await
            .unwrap();

        assert_eq!(fragment.size_bytes, data.len() as u64);

        let retrieved = manager.retrieve_fragment("frag-001").await.unwrap();
        assert_eq!(retrieved, data.to_vec());
    }

    #[tokio::test]
    async fn test_delete_fragment() {
        let temp_dir = TempDir::new().unwrap();
        let mut manager = StorageManager::new(temp_dir.path().to_path_buf(), 1_000_000);
        manager.initialize().await.unwrap();

        let data = b"Test fragment data";
        let expires_at = chrono::Utc::now().timestamp() + 86400;

        manager
            .store_fragment("frag-001", "owner-abc", data, expires_at)
            .await
            .unwrap();

        manager.delete_fragment("frag-001").await.unwrap();

        assert!(manager.retrieve_fragment("frag-001").await.is_err());
    }

    #[tokio::test]
    async fn test_storage_limit() {
        let temp_dir = TempDir::new().unwrap();
        let mut manager = StorageManager::new(temp_dir.path().to_path_buf(), 100); // Only 100 bytes
        manager.initialize().await.unwrap();

        let data = vec![0u8; 200]; // 200 bytes - too big
        let expires_at = chrono::Utc::now().timestamp() + 86400;

        let result = manager
            .store_fragment("frag-001", "owner-abc", &data, expires_at)
            .await;

        assert!(result.is_err());
    }
}
