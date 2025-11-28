//! P2P Node implementation using libp2p

use super::{P2PError, StorageRequest, StorageResponse};
use crate::identity::UserIdentity;

use libp2p::{
    autonat,
    dcutr,
    gossipsub::{self, IdentTopic, MessageAuthenticity},
    identify,
    kad::{self, store::MemoryStore, Mode, Record, RecordKey},
    mdns,
    noise,
    relay,
    request_response::{self, ProtocolSupport},
    swarm::{NetworkBehaviour, SwarmEvent},
    tcp, yamux, Multiaddr, PeerId, StreamProtocol, Swarm, SwarmBuilder,
};

use futures::StreamExt;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::time::Duration;
use tokio::sync::mpsc;

const PROTOCOL_VERSION: &str = "/cloudp2p/1.0.0";
const STORAGE_PROTOCOL: &str = "/cloudp2p/storage/1.0.0";

/// Configuration for P2P node
#[derive(Debug, Clone)]
pub struct P2PNodeConfig {
    /// Bootstrap nodes for initial network join
    pub bootstrap_nodes: Vec<Multiaddr>,

    /// Enable mDNS for local network discovery
    pub enable_mdns: bool,

    /// Enable relay for NAT traversal
    pub enable_relay: bool,

    /// Listen addresses
    pub listen_addresses: Vec<Multiaddr>,

    /// External address (if known)
    pub external_address: Option<Multiaddr>,
}

impl Default for P2PNodeConfig {
    fn default() -> Self {
        Self {
            bootstrap_nodes: vec![],
            enable_mdns: true,
            enable_relay: true,
            listen_addresses: vec![
                "/ip4/0.0.0.0/tcp/0".parse().unwrap(),
                "/ip4/0.0.0.0/udp/0/quic-v1".parse().unwrap(),
            ],
            external_address: None,
        }
    }
}

/// Events emitted by the P2P node
#[derive(Debug, Clone)]
pub enum P2PEvent {
    /// Connected to a new peer
    PeerConnected(PeerId),

    /// Disconnected from a peer
    PeerDisconnected(PeerId),

    /// Received a storage request
    StorageRequest {
        peer: PeerId,
        request: StorageRequest,
    },

    /// Received a storage response
    StorageResponse {
        peer: PeerId,
        response: StorageResponse,
    },

    /// Received a gossip message
    GossipMessage {
        topic: String,
        data: Vec<u8>,
        source: Option<PeerId>,
    },

    /// Found data in DHT
    DhtValue {
        key: Vec<u8>,
        value: Vec<u8>,
    },

    /// Network status update
    NetworkStatus {
        connected_peers: usize,
        listening_addresses: Vec<Multiaddr>,
    },

    /// Node started listening
    Listening(Multiaddr),

    /// Error occurred
    Error(String),
}

/// Combined network behaviour
#[derive(NetworkBehaviour)]
pub struct CloudP2PBehaviour {
    /// Kademlia DHT for peer/content discovery
    pub kademlia: kad::Behaviour<MemoryStore>,

    /// mDNS for local network discovery
    pub mdns: mdns::tokio::Behaviour,

    /// Identify protocol for peer information
    pub identify: identify::Behaviour,

    /// Gossipsub for pub/sub messaging
    pub gossipsub: gossipsub::Behaviour,

    /// Relay client for NAT traversal
    pub relay_client: relay::client::Behaviour,

    /// DCUtR for direct connection upgrade through relay
    pub dcutr: dcutr::Behaviour,

    /// AutoNAT for NAT detection
    pub autonat: autonat::Behaviour,

    /// Request-response for storage operations
    pub storage: request_response::cbor::Behaviour<StorageRequest, StorageResponse>,
}

/// Main P2P node
pub struct P2PNode {
    /// Local peer ID
    pub local_peer_id: PeerId,

    /// libp2p swarm
    swarm: Swarm<CloudP2PBehaviour>,

    /// Event sender
    event_tx: mpsc::UnboundedSender<P2PEvent>,

    /// Event receiver
    event_rx: mpsc::UnboundedReceiver<P2PEvent>,

    /// Connected peers
    connected_peers: HashSet<PeerId>,

    /// Peer storage info (how much each peer offers/uses)
    peer_storage_info: HashMap<PeerId, PeerStorageInfo>,
}

/// Storage info for a peer
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PeerStorageInfo {
    /// Storage offered (bytes)
    pub offered: u64,

    /// Storage used (bytes)
    pub used: u64,

    /// Last heartbeat timestamp
    pub last_heartbeat: i64,

    /// Reputation score
    pub reputation: f32,
}

impl P2PNode {
    /// Create a new P2P node
    pub async fn new(
        identity: &UserIdentity,
        config: P2PNodeConfig,
    ) -> Result<Self, P2PError> {
        // Create libp2p keypair from identity
        let keypair = Self::derive_libp2p_keypair(identity)?;
        let local_peer_id = PeerId::from(keypair.public());

        tracing::info!("Creating P2P node with PeerId: {}", local_peer_id);

        // Build the swarm
        let swarm = Self::build_swarm(keypair, &config).await?;

        // Create event channel
        let (event_tx, event_rx) = mpsc::unbounded_channel();

        Ok(Self {
            local_peer_id,
            swarm,
            event_tx,
            event_rx,
            connected_peers: HashSet::new(),
            peer_storage_info: HashMap::new(),
        })
    }

    /// Derive libp2p keypair from user identity
    fn derive_libp2p_keypair(
        identity: &UserIdentity,
    ) -> Result<libp2p::identity::Keypair, P2PError> {
        use hkdf::Hkdf;
        use sha2::Sha256;

        // Derive a separate key for libp2p from the signing key
        let hk = Hkdf::<Sha256>::new(
            Some(b"cloudp2p-libp2p"),
            identity.signing_keys().signing_key.as_bytes(),
        );

        let mut libp2p_seed = [0u8; 32];
        hk.expand(b"libp2p-ed25519", &mut libp2p_seed)
            .map_err(|e| P2PError::InitializationFailed(e.to_string()))?;

        let secret_key = libp2p::identity::ed25519::SecretKey::try_from_bytes(libp2p_seed)
            .map_err(|e| P2PError::InitializationFailed(e.to_string()))?;

        let keypair = libp2p::identity::ed25519::Keypair::from(secret_key);
        Ok(libp2p::identity::Keypair::from(keypair))
    }

    /// Build the libp2p swarm with all protocols
    async fn build_swarm(
        keypair: libp2p::identity::Keypair,
        config: &P2PNodeConfig,
    ) -> Result<Swarm<CloudP2PBehaviour>, P2PError> {
        let peer_id = PeerId::from(keypair.public());

        let swarm = SwarmBuilder::with_existing_identity(keypair)
            .with_tokio()
            .with_tcp(
                tcp::Config::default(),
                noise::Config::new,
                yamux::Config::default,
            )
            .map_err(|e| P2PError::InitializationFailed(e.to_string()))?
            .with_quic()
            .with_relay_client(noise::Config::new, yamux::Config::default)
            .map_err(|e| P2PError::InitializationFailed(e.to_string()))?
            .with_behaviour(|keypair, relay_client| {
                // Kademlia DHT
                let kademlia = {
                    let store = MemoryStore::new(peer_id);
                    let mut config = kad::Config::new(StreamProtocol::new(PROTOCOL_VERSION));
                    config.set_query_timeout(Duration::from_secs(60));
                    let mut behaviour = kad::Behaviour::with_config(peer_id, store, config);
                    behaviour.set_mode(Some(Mode::Server));
                    behaviour
                };

                // mDNS
                let mdns = mdns::tokio::Behaviour::new(
                    mdns::Config::default(),
                    peer_id,
                ).expect("Failed to create mDNS behaviour");

                // Identify
                let identify = identify::Behaviour::new(
                    identify::Config::new(PROTOCOL_VERSION.to_string(), keypair.public())
                        .with_agent_version(format!("cloudp2p/{}", env!("CARGO_PKG_VERSION"))),
                );

                // Gossipsub
                let gossipsub = {
                    let config = gossipsub::ConfigBuilder::default()
                        .heartbeat_interval(Duration::from_secs(10))
                        .validation_mode(gossipsub::ValidationMode::Strict)
                        .build()
                        .expect("Valid gossipsub config");

                    gossipsub::Behaviour::new(
                        MessageAuthenticity::Signed(keypair.clone()),
                        config,
                    ).expect("Valid gossipsub behaviour")
                };

                // DCUtR
                let dcutr = dcutr::Behaviour::new(peer_id);

                // AutoNAT
                let autonat = autonat::Behaviour::new(peer_id, autonat::Config::default());

                // Storage request-response protocol
                let storage = request_response::cbor::Behaviour::new(
                    [(StreamProtocol::new(STORAGE_PROTOCOL), ProtocolSupport::Full)],
                    request_response::Config::default(),
                );

                CloudP2PBehaviour {
                    kademlia,
                    mdns,
                    identify,
                    gossipsub,
                    relay_client,
                    dcutr,
                    autonat,
                    storage,
                }
            })
            .map_err(|e| P2PError::InitializationFailed(e.to_string()))?
            .with_swarm_config(|cfg| {
                cfg.with_idle_connection_timeout(Duration::from_secs(60))
            })
            .build();

        Ok(swarm)
    }

    /// Start listening on configured addresses
    pub async fn start(&mut self, config: &P2PNodeConfig) -> Result<(), P2PError> {
        // Listen on configured addresses
        for addr in &config.listen_addresses {
            self.swarm
                .listen_on(addr.clone())
                .map_err(|e| P2PError::Transport(e.to_string()))?;
        }

        // Add bootstrap nodes to Kademlia
        for addr in &config.bootstrap_nodes {
            if let Some(peer_id) = Self::extract_peer_id(addr) {
                self.swarm.behaviour_mut().kademlia.add_address(&peer_id, addr.clone());
            }
        }

        // Bootstrap Kademlia
        if !config.bootstrap_nodes.is_empty() {
            self.swarm
                .behaviour_mut()
                .kademlia
                .bootstrap()
                .map_err(|e| P2PError::Dht(e.to_string()))?;
        }

        // Subscribe to important topics
        self.subscribe_to_topic("cloudp2p/heartbeats")?;
        self.subscribe_to_topic("cloudp2p/storage/offers")?;
        self.subscribe_to_topic("cloudp2p/storage/requests")?;

        Ok(())
    }

    /// Extract peer ID from multiaddr
    fn extract_peer_id(addr: &Multiaddr) -> Option<PeerId> {
        addr.iter().find_map(|p| {
            if let libp2p::multiaddr::Protocol::P2p(peer_id) = p {
                Some(peer_id)
            } else {
                None
            }
        })
    }

    /// Subscribe to a gossipsub topic
    pub fn subscribe_to_topic(&mut self, topic: &str) -> Result<(), P2PError> {
        let topic = IdentTopic::new(topic);
        self.swarm
            .behaviour_mut()
            .gossipsub
            .subscribe(&topic)
            .map_err(|e| P2PError::Protocol(e.to_string()))?;
        Ok(())
    }

    /// Publish a message to a topic
    pub fn publish(&mut self, topic: &str, data: Vec<u8>) -> Result<(), P2PError> {
        let topic = IdentTopic::new(topic);
        self.swarm
            .behaviour_mut()
            .gossipsub
            .publish(topic, data)
            .map_err(|e| P2PError::Protocol(e.to_string()))?;
        Ok(())
    }

    /// Store data in DHT
    pub fn put_dht(&mut self, key: Vec<u8>, value: Vec<u8>) -> Result<(), P2PError> {
        let record = Record {
            key: RecordKey::new(&key),
            value,
            publisher: Some(self.local_peer_id),
            expires: None,
        };

        self.swarm
            .behaviour_mut()
            .kademlia
            .put_record(record, kad::Quorum::One)
            .map_err(|e| P2PError::Dht(e.to_string()))?;

        Ok(())
    }

    /// Get data from DHT
    pub fn get_dht(&mut self, key: Vec<u8>) -> kad::QueryId {
        self.swarm
            .behaviour_mut()
            .kademlia
            .get_record(RecordKey::new(&key))
    }

    /// Send a storage request to a peer
    pub fn send_storage_request(
        &mut self,
        peer: PeerId,
        request: StorageRequest,
    ) -> request_response::OutboundRequestId {
        self.swarm
            .behaviour_mut()
            .storage
            .send_request(&peer, request)
    }

    /// Get event receiver
    pub fn event_receiver(&mut self) -> &mut mpsc::UnboundedReceiver<P2PEvent> {
        &mut self.event_rx
    }

    /// Get connected peers count
    pub fn connected_peers_count(&self) -> usize {
        self.connected_peers.len()
    }

    /// Get list of connected peers
    pub fn connected_peers(&self) -> Vec<PeerId> {
        self.connected_peers.iter().cloned().collect()
    }

    /// Run the event loop (should be spawned as a task)
    pub async fn run(&mut self) {
        loop {
            match self.swarm.select_next_some().await {
                SwarmEvent::NewListenAddr { address, .. } => {
                    tracing::info!("Listening on {}", address);
                    let _ = self.event_tx.send(P2PEvent::Listening(address));
                }

                SwarmEvent::ConnectionEstablished { peer_id, .. } => {
                    tracing::info!("Connected to {}", peer_id);
                    self.connected_peers.insert(peer_id);
                    let _ = self.event_tx.send(P2PEvent::PeerConnected(peer_id));
                }

                SwarmEvent::ConnectionClosed { peer_id, .. } => {
                    tracing::info!("Disconnected from {}", peer_id);
                    self.connected_peers.remove(&peer_id);
                    let _ = self.event_tx.send(P2PEvent::PeerDisconnected(peer_id));
                }

                SwarmEvent::Behaviour(event) => {
                    self.handle_behaviour_event(event).await;
                }

                _ => {}
            }
        }
    }

    /// Handle behaviour events
    async fn handle_behaviour_event(&mut self, event: CloudP2PBehaviourEvent) {
        match event {
            CloudP2PBehaviourEvent::Mdns(mdns::Event::Discovered(peers)) => {
                for (peer_id, addr) in peers {
                    tracing::debug!("mDNS discovered: {} at {}", peer_id, addr);
                    self.swarm.behaviour_mut().kademlia.add_address(&peer_id, addr);
                }
            }

            CloudP2PBehaviourEvent::Kademlia(kad::Event::OutboundQueryProgressed {
                result: kad::QueryResult::GetRecord(Ok(kad::GetRecordOk::FoundRecord(record))),
                ..
            }) => {
                let _ = self.event_tx.send(P2PEvent::DhtValue {
                    key: record.record.key.to_vec(),
                    value: record.record.value,
                });
            }

            CloudP2PBehaviourEvent::Gossipsub(gossipsub::Event::Message {
                message,
                propagation_source,
                ..
            }) => {
                let _ = self.event_tx.send(P2PEvent::GossipMessage {
                    topic: message.topic.to_string(),
                    data: message.data,
                    source: Some(propagation_source),
                });
            }

            CloudP2PBehaviourEvent::Storage(request_response::Event::Message {
                peer,
                message,
            }) => {
                match message {
                    request_response::Message::Request { request, channel, .. } => {
                        let _ = self.event_tx.send(P2PEvent::StorageRequest {
                            peer,
                            request: request.clone(),
                        });

                        // TODO: Handle request and send response via channel
                    }
                    request_response::Message::Response { response, .. } => {
                        let _ = self.event_tx.send(P2PEvent::StorageResponse {
                            peer,
                            response,
                        });
                    }
                }
            }

            CloudP2PBehaviourEvent::Identify(identify::Event::Received { peer_id, info, .. }) => {
                tracing::debug!(
                    "Identified peer {}: {} ({})",
                    peer_id,
                    info.agent_version,
                    info.protocol_version
                );

                // Add observed addresses to Kademlia
                for addr in info.listen_addrs {
                    self.swarm.behaviour_mut().kademlia.add_address(&peer_id, addr);
                }
            }

            _ => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_create_node() {
        let (identity, _) = UserIdentity::generate(None).unwrap();
        let config = P2PNodeConfig::default();

        let node = P2PNode::new(&identity, config).await;
        assert!(node.is_ok());

        let node = node.unwrap();
        assert_eq!(node.connected_peers_count(), 0);
    }
}
