//! Peer discovery and management

use libp2p::{Multiaddr, PeerId};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::time::{Duration, Instant};

/// Information about a discovered peer
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PeerInfo {
    /// Peer ID
    pub peer_id: String,

    /// Known addresses
    #[serde(skip)]
    pub addresses: Vec<Multiaddr>,

    /// Storage offered (bytes)
    pub storage_offered: u64,

    /// Storage available (bytes)
    pub storage_available: u64,

    /// Reliability score (0.0 - 1.0)
    pub reliability: f32,

    /// Latency in milliseconds
    pub latency_ms: u32,

    /// Last seen timestamp
    pub last_seen: i64,

    /// Is this peer behind NAT?
    pub behind_nat: bool,

    /// Peer's agent version
    pub agent_version: String,
}

impl PeerInfo {
    /// Create new peer info
    pub fn new(peer_id: PeerId) -> Self {
        Self {
            peer_id: peer_id.to_string(),
            addresses: vec![],
            storage_offered: 0,
            storage_available: 0,
            reliability: 0.5, // Start at neutral
            latency_ms: 0,
            last_seen: chrono::Utc::now().timestamp(),
            behind_nat: false,
            agent_version: String::new(),
        }
    }

    /// Update last seen
    pub fn touch(&mut self) {
        self.last_seen = chrono::Utc::now().timestamp();
    }

    /// Check if peer is stale (not seen recently)
    pub fn is_stale(&self, max_age_seconds: i64) -> bool {
        let now = chrono::Utc::now().timestamp();
        now - self.last_seen > max_age_seconds
    }

    /// Calculate overall score for peer selection
    pub fn score(&self) -> f32 {
        let reliability_weight = 0.4;
        let latency_weight = 0.3;
        let availability_weight = 0.3;

        let latency_score = 1.0 - (self.latency_ms.min(1000) as f32 / 1000.0);
        let availability_ratio = if self.storage_offered > 0 {
            self.storage_available as f32 / self.storage_offered as f32
        } else {
            0.0
        };

        self.reliability * reliability_weight
            + latency_score * latency_weight
            + availability_ratio * availability_weight
    }
}

/// Peer manager for tracking and selecting storage peers
pub struct PeerManager {
    /// Known peers
    peers: HashMap<String, PeerInfo>,

    /// Blacklisted peers (misbehaving)
    blacklist: HashMap<String, i64>, // peer_id -> blacklist expiry

    /// Minimum required reliability score
    min_reliability: f32,

    /// Maximum peer age before considered stale
    max_age_seconds: i64,
}

impl PeerManager {
    /// Create a new peer manager
    pub fn new() -> Self {
        Self {
            peers: HashMap::new(),
            blacklist: HashMap::new(),
            min_reliability: 0.3,
            max_age_seconds: 3600, // 1 hour
        }
    }

    /// Add or update a peer
    pub fn add_peer(&mut self, mut info: PeerInfo) {
        info.touch();
        self.peers.insert(info.peer_id.clone(), info);
    }

    /// Get peer info
    pub fn get_peer(&self, peer_id: &str) -> Option<&PeerInfo> {
        self.peers.get(peer_id)
    }

    /// Update peer reliability (positive or negative)
    pub fn update_reliability(&mut self, peer_id: &str, delta: f32) {
        if let Some(peer) = self.peers.get_mut(peer_id) {
            peer.reliability = (peer.reliability + delta).clamp(0.0, 1.0);

            // Auto-blacklist if reliability drops too low
            if peer.reliability < 0.1 {
                self.blacklist_peer(peer_id, 3600); // 1 hour
            }
        }
    }

    /// Blacklist a peer for a duration
    pub fn blacklist_peer(&mut self, peer_id: &str, duration_seconds: i64) {
        let expiry = chrono::Utc::now().timestamp() + duration_seconds;
        self.blacklist.insert(peer_id.to_string(), expiry);
    }

    /// Check if peer is blacklisted
    pub fn is_blacklisted(&self, peer_id: &str) -> bool {
        if let Some(expiry) = self.blacklist.get(peer_id) {
            let now = chrono::Utc::now().timestamp();
            if now < *expiry {
                return true;
            }
        }
        false
    }

    /// Remove stale peers
    pub fn prune_stale(&mut self) {
        let now = chrono::Utc::now().timestamp();

        // Remove stale peers
        self.peers
            .retain(|_, peer| now - peer.last_seen <= self.max_age_seconds);

        // Remove expired blacklist entries
        self.blacklist.retain(|_, expiry| now < *expiry);
    }

    /// Select best peers for storing data
    /// Returns peers sorted by score, filtered by requirements
    pub fn select_storage_peers(
        &self,
        required_bytes: u64,
        count: usize,
    ) -> Vec<&PeerInfo> {
        let mut candidates: Vec<&PeerInfo> = self
            .peers
            .values()
            .filter(|p| {
                !self.is_blacklisted(&p.peer_id)
                    && p.reliability >= self.min_reliability
                    && p.storage_available >= required_bytes
                    && !p.is_stale(self.max_age_seconds)
            })
            .collect();

        // Sort by score (descending)
        candidates.sort_by(|a, b| b.score().partial_cmp(&a.score()).unwrap());

        candidates.into_iter().take(count).collect()
    }

    /// Get all healthy peers
    pub fn healthy_peers(&self) -> Vec<&PeerInfo> {
        self.peers
            .values()
            .filter(|p| {
                !self.is_blacklisted(&p.peer_id)
                    && p.reliability >= self.min_reliability
                    && !p.is_stale(self.max_age_seconds)
            })
            .collect()
    }

    /// Get total available storage across all peers
    pub fn total_available_storage(&self) -> u64 {
        self.healthy_peers()
            .iter()
            .map(|p| p.storage_available)
            .sum()
    }

    /// Get peer count
    pub fn peer_count(&self) -> usize {
        self.peers.len()
    }

    /// Get healthy peer count
    pub fn healthy_peer_count(&self) -> usize {
        self.healthy_peers().len()
    }
}

impl Default for PeerManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_peer(id: &str, available: u64, reliability: f32) -> PeerInfo {
        PeerInfo {
            peer_id: id.to_string(),
            addresses: vec![],
            storage_offered: available * 2,
            storage_available: available,
            reliability,
            latency_ms: 100,
            last_seen: chrono::Utc::now().timestamp(),
            behind_nat: false,
            agent_version: "test".to_string(),
        }
    }

    #[test]
    fn test_peer_score() {
        let peer = create_test_peer("test", 1_000_000, 0.8);
        let score = peer.score();
        assert!(score > 0.0 && score <= 1.0);
    }

    #[test]
    fn test_peer_selection() {
        let mut manager = PeerManager::new();

        manager.add_peer(create_test_peer("peer1", 1_000_000, 0.9));
        manager.add_peer(create_test_peer("peer2", 2_000_000, 0.7));
        manager.add_peer(create_test_peer("peer3", 500_000, 0.5));
        manager.add_peer(create_test_peer("peer4", 1_000_000, 0.2)); // Low reliability

        let selected = manager.select_storage_peers(500_000, 3);

        assert_eq!(selected.len(), 3);
        // peer1 should be first (highest reliability with enough space)
        assert_eq!(selected[0].peer_id, "peer1");
    }

    #[test]
    fn test_blacklist() {
        let mut manager = PeerManager::new();
        manager.add_peer(create_test_peer("bad_peer", 1_000_000, 0.9));

        manager.blacklist_peer("bad_peer", 3600);

        assert!(manager.is_blacklisted("bad_peer"));
        assert_eq!(manager.select_storage_peers(500_000, 10).len(), 0);
    }
}
