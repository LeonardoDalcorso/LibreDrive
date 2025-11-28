//! Storage Protocol - Request/Response messages for storage operations

use serde::{Deserialize, Serialize};
use crate::crypto::ContentHash;

/// Storage request types
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum StorageRequest {
    /// Store a data fragment
    Store {
        /// Unique fragment ID (hash of content)
        fragment_id: String,

        /// Owner's public ID
        owner_id: String,

        /// Encrypted fragment data
        data: Vec<u8>,

        /// Expiration timestamp (Unix)
        expires_at: i64,

        /// Storage contract signature
        signature: Vec<u8>,
    },

    /// Retrieve a stored fragment
    Retrieve {
        /// Fragment ID to retrieve
        fragment_id: String,

        /// Requester's public ID
        requester_id: String,

        /// Proof of ownership
        signature: Vec<u8>,
    },

    /// Delete a fragment (by owner)
    Delete {
        /// Fragment ID to delete
        fragment_id: String,

        /// Owner's public ID
        owner_id: String,

        /// Deletion authorization signature
        signature: Vec<u8>,
    },

    /// Heartbeat to renew storage contract
    Heartbeat {
        /// Owner's public ID
        owner_id: String,

        /// Timestamp
        timestamp: i64,

        /// Signature
        signature: Vec<u8>,
    },

    /// Query storage availability
    QueryAvailability {
        /// Required storage in bytes
        required_bytes: u64,

        /// Requester ID
        requester_id: String,
    },

    /// Proof of Storage challenge
    StorageChallenge {
        /// Fragment ID to prove
        fragment_id: String,

        /// Random challenge bytes
        challenge: Vec<u8>,

        /// Challenger's signature
        signature: Vec<u8>,
    },

    /// Request peer's storage info
    GetStorageInfo,
}

/// Storage response types
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum StorageResponse {
    /// Fragment stored successfully
    Stored {
        fragment_id: String,
        /// Storage node's signature as receipt
        receipt: Vec<u8>,
    },

    /// Fragment data returned
    Data {
        fragment_id: String,
        data: Vec<u8>,
        /// Proof of integrity
        hash: String,
    },

    /// Fragment deleted
    Deleted {
        fragment_id: String,
        confirmation: Vec<u8>,
    },

    /// Heartbeat acknowledged
    HeartbeatAck {
        /// New expiration timestamp
        new_expiration: i64,
    },

    /// Storage availability response
    Availability {
        /// Available bytes
        available_bytes: u64,

        /// Offered bytes (total capacity this peer provides)
        offered_bytes: u64,

        /// Peer's reliability score
        reliability: f32,
    },

    /// Proof of Storage response
    StorageProof {
        fragment_id: String,
        /// Hash of (fragment_data || challenge)
        proof: Vec<u8>,
    },

    /// Storage info response
    StorageInfo {
        /// Total offered storage
        offered_bytes: u64,

        /// Used storage
        used_bytes: u64,

        /// Number of fragments stored
        fragment_count: u64,

        /// Node uptime percentage
        uptime: f32,
    },

    /// Error response
    Error {
        code: ErrorCode,
        message: String,
    },
}

/// Error codes for storage operations
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum ErrorCode {
    /// Fragment not found
    NotFound,

    /// Not enough storage space
    InsufficientSpace,

    /// Invalid signature
    InvalidSignature,

    /// Fragment expired
    Expired,

    /// Permission denied
    PermissionDenied,

    /// Rate limited
    RateLimited,

    /// Invalid request
    InvalidRequest,

    /// Internal error
    InternalError,
}

/// Storage contract - agreement between data owner and storage peer
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StorageContract {
    /// Fragment ID
    pub fragment_id: String,

    /// Owner's public ID
    pub owner_id: String,

    /// Storage peer's public ID
    pub storage_peer_id: String,

    /// Fragment size in bytes
    pub size_bytes: u64,

    /// Creation timestamp
    pub created_at: i64,

    /// Expiration timestamp (90 days from last heartbeat)
    pub expires_at: i64,

    /// Owner's signature
    pub owner_signature: Vec<u8>,

    /// Storage peer's signature
    pub storage_signature: Vec<u8>,
}

impl StorageContract {
    /// Create a new storage contract
    pub fn new(
        fragment_id: String,
        owner_id: String,
        storage_peer_id: String,
        size_bytes: u64,
        expiration_days: u32,
    ) -> Self {
        let now = chrono::Utc::now().timestamp();
        let expires_at = now + (expiration_days as i64 * 24 * 60 * 60);

        Self {
            fragment_id,
            owner_id,
            storage_peer_id,
            size_bytes,
            created_at: now,
            expires_at,
            owner_signature: vec![],
            storage_signature: vec![],
        }
    }

    /// Get the data to sign
    pub fn signing_data(&self) -> Vec<u8> {
        format!(
            "{}:{}:{}:{}:{}:{}",
            self.fragment_id,
            self.owner_id,
            self.storage_peer_id,
            self.size_bytes,
            self.created_at,
            self.expires_at
        )
        .into_bytes()
    }

    /// Check if contract is expired
    pub fn is_expired(&self) -> bool {
        let now = chrono::Utc::now().timestamp();
        now > self.expires_at
    }

    /// Extend expiration (after heartbeat)
    pub fn extend(&mut self, days: u32) {
        let now = chrono::Utc::now().timestamp();
        self.expires_at = now + (days as i64 * 24 * 60 * 60);
    }

    /// Days until expiration
    pub fn days_until_expiration(&self) -> i64 {
        let now = chrono::Utc::now().timestamp();
        (self.expires_at - now) / (24 * 60 * 60)
    }
}

/// Fragment metadata stored alongside fragment data
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FragmentMetadata {
    /// Fragment ID (content hash)
    pub fragment_id: String,

    /// Owner's public ID
    pub owner_id: String,

    /// Original file ID this fragment belongs to
    pub file_id: String,

    /// Fragment index in the file
    pub fragment_index: u32,

    /// Total fragments for this file
    pub total_fragments: u32,

    /// Fragment size (encrypted)
    pub size_bytes: u64,

    /// Content hash for integrity verification
    pub content_hash: String,

    /// Erasure coding parameters
    pub erasure_data_shards: u32,
    pub erasure_parity_shards: u32,

    /// Creation timestamp
    pub created_at: i64,

    /// Last access timestamp
    pub last_accessed: i64,

    /// Storage contract ID
    pub contract_id: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_storage_contract() {
        let contract = StorageContract::new(
            "frag-001".to_string(),
            "owner-abc".to_string(),
            "storage-xyz".to_string(),
            1024 * 1024, // 1 MB
            90,          // 90 days
        );

        assert!(!contract.is_expired());
        assert!(contract.days_until_expiration() >= 89);
    }

    #[test]
    fn test_contract_expiration() {
        let mut contract = StorageContract::new(
            "frag-001".to_string(),
            "owner-abc".to_string(),
            "storage-xyz".to_string(),
            1024,
            0, // Expires immediately
        );

        contract.expires_at = chrono::Utc::now().timestamp() - 1;
        assert!(contract.is_expired());

        contract.extend(90);
        assert!(!contract.is_expired());
    }
}
