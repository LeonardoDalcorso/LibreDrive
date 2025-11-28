import 'package:flutter_riverpod/flutter_riverpod.dart';

/// Network status data
class NetworkStatusData {
  final bool isOnline;
  final int connectedPeers;
  final List<String> listeningAddresses;
  final int daysUntilExpiration;
  final DateTime? lastHeartbeat;

  const NetworkStatusData({
    required this.isOnline,
    required this.connectedPeers,
    required this.listeningAddresses,
    required this.daysUntilExpiration,
    this.lastHeartbeat,
  });

  factory NetworkStatusData.offline() => const NetworkStatusData(
        isOnline: false,
        connectedPeers: 0,
        listeningAddresses: [],
        daysUntilExpiration: 0,
      );
}

/// Network status provider
final networkStatusProvider = FutureProvider<NetworkStatusData>((ref) async {
  // TODO: Get real status from Rust P2P node
  await Future.delayed(const Duration(milliseconds: 300));

  // Mock data
  return NetworkStatusData(
    isOnline: true,
    connectedPeers: 12,
    listeningAddresses: [
      '/ip4/192.168.1.100/tcp/4001',
      '/ip4/0.0.0.0/udp/4001/quic-v1',
    ],
    daysUntilExpiration: 87,
    lastHeartbeat: DateTime.now().subtract(const Duration(hours: 2)),
  );
});

/// Network event types
enum NetworkEventType {
  peerConnected,
  peerDisconnected,
  storageRequest,
  storageComplete,
  heartbeatSent,
  error,
}

/// Network event
class NetworkEvent {
  final NetworkEventType type;
  final String? peerId;
  final String? message;
  final DateTime timestamp;

  NetworkEvent({
    required this.type,
    this.peerId,
    this.message,
    DateTime? timestamp,
  }) : timestamp = timestamp ?? DateTime.now();
}

/// Network events stream provider
final networkEventsProvider = StreamProvider<NetworkEvent>((ref) async* {
  // TODO: Stream events from Rust P2P node
  while (true) {
    await Future.delayed(const Duration(seconds: 30));
    yield NetworkEvent(
      type: NetworkEventType.heartbeatSent,
      message: 'Heartbeat enviado',
    );
  }
});

/// P2P Node controller
class P2PNodeController extends StateNotifier<P2PNodeState> {
  P2PNodeController() : super(P2PNodeState.initial);

  Future<void> start() async {
    state = P2PNodeState.connecting;
    try {
      // TODO: Start P2P node via Rust
      await Future.delayed(const Duration(seconds: 2));
      state = P2PNodeState.connected;
    } catch (e) {
      state = P2PNodeState.error;
    }
  }

  Future<void> stop() async {
    // TODO: Stop P2P node via Rust
    state = P2PNodeState.disconnected;
  }

  Future<void> sendHeartbeat() async {
    // TODO: Send heartbeat via Rust
  }
}

enum P2PNodeState {
  initial,
  connecting,
  connected,
  disconnected,
  error,
}

final p2pNodeProvider =
    StateNotifierProvider<P2PNodeController, P2PNodeState>((ref) {
  return P2PNodeController();
});
