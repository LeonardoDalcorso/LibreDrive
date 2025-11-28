//! CloudP2P Core - Decentralized P2P Storage System
//!
//! This crate provides the core functionality for a secure, decentralized
//! file storage system using P2P networking, end-to-end encryption, and
//! erasure coding for data redundancy.

pub mod crypto;
pub mod identity;
pub mod p2p;
pub mod storage;

use thiserror::Error;

/// Main error type for CloudP2P operations
#[derive(Error, Debug)]
pub enum CloudP2PError {
    #[error("Cryptographic error: {0}")]
    Crypto(#[from] crypto::CryptoError),

    #[error("Identity error: {0}")]
    Identity(#[from] identity::IdentityError),

    #[error("P2P network error: {0}")]
    P2P(#[from] p2p::P2PError),

    #[error("Storage error: {0}")]
    Storage(#[from] storage::StorageError),

    #[error("Serialization error: {0}")]
    Serialization(String),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
}

pub type Result<T> = std::result::Result<T, CloudP2PError>;

/// Core configuration for CloudP2P node
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct CloudP2PConfig {
    /// Storage quota offered to network (in bytes)
    pub storage_offered_bytes: u64,

    /// Maximum storage allowed to use (in bytes)
    pub storage_quota_bytes: u64,

    /// Days until data expires without heartbeat
    pub expiration_days: u32,

    /// Bootstrap nodes for initial P2P connection
    pub bootstrap_nodes: Vec<String>,

    /// Local storage path
    pub data_path: String,

    /// Enable relay for NAT traversal
    pub enable_relay: bool,

    /// Enable mDNS for local peer discovery
    pub enable_mdns: bool,
}

impl Default for CloudP2PConfig {
    fn default() -> Self {
        Self {
            storage_offered_bytes: 10 * 1024 * 1024 * 1024, // 10 GB
            storage_quota_bytes: 10 * 1024 * 1024 * 1024,   // 10 GB
            expiration_days: 90,
            bootstrap_nodes: vec![],
            data_path: "./cloudp2p_data".to_string(),
            enable_relay: true,
            enable_mdns: true,
        }
    }
}

/// Represents the current user/node in the network
pub struct CloudP2PNode {
    pub identity: identity::UserIdentity,
    pub config: CloudP2PConfig,
    // P2P node and storage manager will be added
}

impl CloudP2PNode {
    /// Create a new node from a seed phrase (recovery)
    pub fn from_seed_phrase(
        seed_phrase: &str,
        password: Option<&str>,
        config: CloudP2PConfig,
    ) -> Result<Self> {
        let identity = identity::UserIdentity::from_seed_phrase(seed_phrase, password)?;
        Ok(Self { identity, config })
    }

    /// Create a new node with a fresh identity
    pub fn new(password: Option<&str>, config: CloudP2PConfig) -> Result<(Self, String)> {
        let (identity, seed_phrase) = identity::UserIdentity::generate(password)?;
        Ok((Self { identity, config }, seed_phrase))
    }

    /// Get the node's public ID (for sharing)
    pub fn public_id(&self) -> String {
        self.identity.public_id()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_create_node() {
        let config = CloudP2PConfig::default();
        let (node, seed_phrase) = CloudP2PNode::new(Some("test_password"), config).unwrap();

        assert!(!seed_phrase.is_empty());
        assert!(!node.public_id().is_empty());

        // Test recovery
        let config2 = CloudP2PConfig::default();
        let recovered = CloudP2PNode::from_seed_phrase(&seed_phrase, Some("test_password"), config2).unwrap();

        assert_eq!(node.public_id(), recovered.public_id());
    }
}
