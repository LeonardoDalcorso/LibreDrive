//! P2P Networking Module using libp2p
//!
//! Handles peer discovery, NAT traversal, and data transfer in the decentralized network.

mod node;
mod protocol;
mod discovery;
mod storage_protocol;

pub use node::{P2PNode, P2PNodeConfig, P2PEvent};
pub use protocol::{StorageRequest, StorageResponse};
pub use discovery::PeerInfo;

use thiserror::Error;

#[derive(Error, Debug)]
pub enum P2PError {
    #[error("Failed to initialize network: {0}")]
    InitializationFailed(String),

    #[error("Connection failed: {0}")]
    ConnectionFailed(String),

    #[error("Peer not found: {0}")]
    PeerNotFound(String),

    #[error("Transport error: {0}")]
    Transport(String),

    #[error("Protocol error: {0}")]
    Protocol(String),

    #[error("DHT error: {0}")]
    Dht(String),

    #[error("Timeout")]
    Timeout,
}
