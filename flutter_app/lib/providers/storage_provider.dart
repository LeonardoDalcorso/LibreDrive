import 'package:flutter_riverpod/flutter_riverpod.dart';
import '../services/database_service.dart';

/// Storage statistics data
class StorageStatsData {
  final int totalBytes;
  final int usedBytes;
  final int availableBytes;
  final int offeredBytes;
  final int fileCount;

  const StorageStatsData({
    required this.totalBytes,
    required this.usedBytes,
    required this.availableBytes,
    required this.offeredBytes,
    required this.fileCount,
  });

  double get usagePercentage => totalBytes > 0 ? usedBytes / totalBytes : 0;

  factory StorageStatsData.empty() => const StorageStatsData(
        totalBytes: 0,
        usedBytes: 0,
        availableBytes: 0,
        offeredBytes: 0,
        fileCount: 0,
      );

  factory StorageStatsData.fromStats(StorageStats stats) => StorageStatsData(
        totalBytes: stats.totalBytes,
        usedBytes: stats.usedBytes,
        availableBytes: stats.availableBytes,
        offeredBytes: stats.offeredBytes,
        fileCount: stats.fileCount,
      );
}

/// Storage stats notifier
class StorageStatsNotifier extends StateNotifier<AsyncValue<StorageStatsData>> {
  StorageStatsNotifier() : super(const AsyncValue.loading()) {
    refresh();
  }

  Future<void> refresh() async {
    state = const AsyncValue.loading();
    try {
      final stats = DatabaseService.getStorageStats();
      state = AsyncValue.data(StorageStatsData.fromStats(stats));
    } catch (e, st) {
      state = AsyncValue.error(e, st);
    }
  }
}

final storageStatsNotifierProvider =
    StateNotifierProvider<StorageStatsNotifier, AsyncValue<StorageStatsData>>(
        (ref) {
  return StorageStatsNotifier();
});

/// Storage stats provider (backwards compatible)
final storageStatsProvider = Provider<AsyncValue<StorageStatsData>>((ref) {
  return ref.watch(storageStatsNotifierProvider);
});

/// Storage quota state
class StorageQuotaState {
  final int quotaMb;
  final int offeredMb;

  const StorageQuotaState({
    this.quotaMb = 10240, // 10 GB
    this.offeredMb = 10240, // 10 GB
  });
}

/// Storage quota notifier
class StorageQuotaNotifier extends StateNotifier<StorageQuotaState> {
  StorageQuotaNotifier() : super(const StorageQuotaState()) {
    _loadFromDb();
  }

  void _loadFromDb() {
    final quotaMb = DatabaseService.getSetting<int>('quotaMb', defaultValue: 10240) ?? 10240;
    final offeredMb = DatabaseService.getSetting<int>('offeredMb', defaultValue: 10240) ?? 10240;
    state = StorageQuotaState(quotaMb: quotaMb, offeredMb: offeredMb);
  }

  Future<void> setQuota(int quotaMb) async {
    await DatabaseService.saveSetting('quotaMb', quotaMb);
    state = StorageQuotaState(
      quotaMb: quotaMb,
      offeredMb: state.offeredMb,
    );
  }

  Future<void> setOffered(int offeredMb) async {
    await DatabaseService.saveSetting('offeredMb', offeredMb);
    state = StorageQuotaState(
      quotaMb: state.quotaMb,
      offeredMb: offeredMb,
    );
  }
}

final storageQuotaProvider =
    StateNotifierProvider<StorageQuotaNotifier, StorageQuotaState>((ref) {
  return StorageQuotaNotifier();
});
