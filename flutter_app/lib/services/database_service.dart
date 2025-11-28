import 'package:hive_flutter/hive_flutter.dart';

/// Database service using Hive for local persistence
class DatabaseService {
  static const String _filesBoxName = 'files';
  static const String _foldersBoxName = 'folders';
  static const String _settingsBoxName = 'settings';

  static Box<Map>? _filesBox;
  static Box<Map>? _foldersBox;
  static Box? _settingsBox;

  static Future<void> initialize() async {
    _filesBox = await Hive.openBox<Map>(_filesBoxName);
    _foldersBox = await Hive.openBox<Map>(_foldersBoxName);
    _settingsBox = await Hive.openBox(_settingsBoxName);
  }

  // ==================== FILES ====================

  static Future<void> saveFile(FileRecord file) async {
    await _filesBox?.put(file.id, file.toMap());
  }

  static Future<void> deleteFile(String fileId) async {
    await _filesBox?.delete(fileId);
  }

  static List<FileRecord> getAllFiles() {
    final files = <FileRecord>[];
    _filesBox?.values.forEach((map) {
      try {
        files.add(FileRecord.fromMap(Map<String, dynamic>.from(map)));
      } catch (_) {}
    });
    return files;
  }

  static List<FileRecord> getFilesByFolder(String? folderId) {
    return getAllFiles().where((f) => f.folderId == folderId).toList();
  }

  static List<FileRecord> searchFiles(String query) {
    final lowerQuery = query.toLowerCase();
    return getAllFiles().where((f) {
      return f.name.toLowerCase().contains(lowerQuery);
    }).toList();
  }

  static List<FileRecord> getRecentFiles({int limit = 10}) {
    final files = getAllFiles();
    files.sort((a, b) => b.modifiedAt.compareTo(a.modifiedAt));
    return files.take(limit).toList();
  }

  // ==================== FOLDERS ====================

  static Future<void> saveFolder(FolderRecord folder) async {
    await _foldersBox?.put(folder.id, folder.toMap());
  }

  static Future<void> deleteFolder(String folderId) async {
    await _foldersBox?.delete(folderId);
    // Delete all files in folder
    final files = getFilesByFolder(folderId);
    for (final file in files) {
      await deleteFile(file.id);
    }
  }

  static List<FolderRecord> getAllFolders() {
    final folders = <FolderRecord>[];
    _foldersBox?.values.forEach((map) {
      try {
        folders.add(FolderRecord.fromMap(Map<String, dynamic>.from(map)));
      } catch (_) {}
    });
    return folders;
  }

  static int getFileCountInFolder(String folderId) {
    return getFilesByFolder(folderId).length;
  }

  // ==================== SETTINGS ====================

  static Future<void> saveSetting(String key, dynamic value) async {
    await _settingsBox?.put(key, value);
  }

  static T? getSetting<T>(String key, {T? defaultValue}) {
    return _settingsBox?.get(key, defaultValue: defaultValue) as T?;
  }

  // ==================== STORAGE STATS ====================

  static StorageStats getStorageStats() {
    final files = getAllFiles();
    final totalUsed = files.fold<int>(0, (sum, f) => sum + f.size);
    final quotaMb = getSetting<int>('quotaMb', defaultValue: 10240) ?? 10240;
    final offeredMb = getSetting<int>('offeredMb', defaultValue: 10240) ?? 10240;

    return StorageStats(
      totalBytes: quotaMb * 1024 * 1024,
      usedBytes: totalUsed,
      availableBytes: (quotaMb * 1024 * 1024) - totalUsed,
      offeredBytes: offeredMb * 1024 * 1024,
      fileCount: files.length,
    );
  }
}

/// File record for database
class FileRecord {
  final String id;
  final String name;
  final String path;
  final int size;
  final String mimeType;
  final DateTime createdAt;
  final DateTime modifiedAt;
  final String? folderId;
  final String status;

  const FileRecord({
    required this.id,
    required this.name,
    required this.path,
    required this.size,
    required this.mimeType,
    required this.createdAt,
    required this.modifiedAt,
    this.folderId,
    this.status = 'synced',
  });

  Map<String, dynamic> toMap() => {
        'id': id,
        'name': name,
        'path': path,
        'size': size,
        'mimeType': mimeType,
        'createdAt': createdAt.toIso8601String(),
        'modifiedAt': modifiedAt.toIso8601String(),
        'folderId': folderId,
        'status': status,
      };

  factory FileRecord.fromMap(Map<String, dynamic> map) => FileRecord(
        id: map['id'] as String,
        name: map['name'] as String,
        path: map['path'] as String? ?? '',
        size: map['size'] as int,
        mimeType: map['mimeType'] as String,
        createdAt: DateTime.parse(map['createdAt'] as String),
        modifiedAt: DateTime.parse(map['modifiedAt'] as String),
        folderId: map['folderId'] as String?,
        status: map['status'] as String? ?? 'synced',
      );
}

/// Folder record for database
class FolderRecord {
  final String id;
  final String name;
  final String? parentId;
  final DateTime createdAt;

  const FolderRecord({
    required this.id,
    required this.name,
    this.parentId,
    required this.createdAt,
  });

  Map<String, dynamic> toMap() => {
        'id': id,
        'name': name,
        'parentId': parentId,
        'createdAt': createdAt.toIso8601String(),
      };

  factory FolderRecord.fromMap(Map<String, dynamic> map) => FolderRecord(
        id: map['id'] as String,
        name: map['name'] as String,
        parentId: map['parentId'] as String?,
        createdAt: DateTime.parse(map['createdAt'] as String),
      );
}

/// Storage statistics
class StorageStats {
  final int totalBytes;
  final int usedBytes;
  final int availableBytes;
  final int offeredBytes;
  final int fileCount;

  const StorageStats({
    required this.totalBytes,
    required this.usedBytes,
    required this.availableBytes,
    required this.offeredBytes,
    required this.fileCount,
  });

  double get usagePercentage => totalBytes > 0 ? usedBytes / totalBytes : 0;
}
