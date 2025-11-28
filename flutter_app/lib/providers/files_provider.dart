import 'dart:typed_data';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:file_picker/file_picker.dart';

import '../services/database_service.dart';
import '../services/secure_storage_service.dart';

/// File data model
class FileData {
  final String fileId;
  final String fileName;
  final String filePath;
  final int size;
  final String mimeType;
  final DateTime createdAt;
  final DateTime modifiedAt;
  final String? folderId;
  final List<String> tags;
  final bool isShared;
  final FileStatus status;
  final bool isEncrypted;
  final String? merkleRoot;

  const FileData({
    required this.fileId,
    required this.fileName,
    this.filePath = '',
    required this.size,
    required this.mimeType,
    required this.createdAt,
    required this.modifiedAt,
    this.folderId,
    this.tags = const [],
    this.isShared = false,
    this.status = FileStatus.synced,
    this.isEncrypted = false,
    this.merkleRoot,
  });

  factory FileData.fromRecord(FileRecord record) => FileData(
        fileId: record.id,
        fileName: record.name,
        filePath: record.path,
        size: record.size,
        mimeType: record.mimeType,
        createdAt: record.createdAt,
        modifiedAt: record.modifiedAt,
        folderId: record.folderId,
        status: FileStatus.values.firstWhere(
          (s) => s.name == record.status,
          orElse: () => FileStatus.synced,
        ),
        isEncrypted: record.status == 'encrypted',
      );

  factory FileData.fromSecureMetadata(SecureFileMetadata meta) => FileData(
        fileId: meta.fileId,
        fileName: meta.fileName,
        size: meta.originalSize,
        mimeType: _getMimeType(meta.fileName),
        createdAt: meta.createdAt,
        modifiedAt: meta.createdAt,
        folderId: meta.folderId,
        status: FileStatus.encrypted,
        isEncrypted: true,
        merkleRoot: meta.merkleRoot,
      );
}

enum FileStatus {
  synced,
  syncing,
  pendingUpload,
  encrypting,
  encrypted,
  error,
}

/// Folder data model
class FolderData {
  final String folderId;
  final String name;
  final String? parentId;
  final int fileCount;
  final DateTime createdAt;

  const FolderData({
    required this.folderId,
    required this.name,
    this.parentId,
    this.fileCount = 0,
    required this.createdAt,
  });

  factory FolderData.fromRecord(FolderRecord record, int fileCount) => FolderData(
        folderId: record.id,
        name: record.name,
        parentId: record.parentId,
        fileCount: fileCount,
        createdAt: record.createdAt,
      );
}

/// Files notifier for state management
class FilesNotifier extends StateNotifier<AsyncValue<List<FileData>>> {
  FilesNotifier() : super(const AsyncValue.loading()) {
    loadFiles();
  }

  Future<void> loadFiles() async {
    state = const AsyncValue.loading();
    try {
      final records = DatabaseService.getAllFiles();
      final files = records.map((r) => FileData.fromRecord(r)).toList();
      files.sort((a, b) => b.modifiedAt.compareTo(a.modifiedAt));
      state = AsyncValue.data(files);
    } catch (e, st) {
      state = AsyncValue.error(e, st);
    }
  }

  Future<void> addFile(FileData file) async {
    final record = FileRecord(
      id: file.fileId,
      name: file.fileName,
      path: file.filePath,
      size: file.size,
      mimeType: file.mimeType,
      createdAt: file.createdAt,
      modifiedAt: file.modifiedAt,
      folderId: file.folderId,
      status: file.status.name,
    );
    await DatabaseService.saveFile(record);
    await loadFiles();
  }

  Future<void> deleteFile(String fileId) async {
    // Deletar do storage seguro também
    try {
      await SecureStorageService.deleteFile(fileId);
    } catch (_) {}
    await DatabaseService.deleteFile(fileId);
    await loadFiles();
  }

  /// Upload com criptografia completa
  Future<void> uploadFromPicker({String? folderId}) async {
    try {
      final result = await FilePicker.platform.pickFiles(
        allowMultiple: true,
        withData: true, // Importante para web
      );

      if (result != null && result.files.isNotEmpty) {
        for (final file in result.files) {
          if (file.bytes == null) continue;

          // Upload com criptografia
          final metadata = await SecureStorageService.uploadFile(
            fileData: Uint8List.fromList(file.bytes!),
            fileName: file.name,
            folderId: folderId,
          );

          // Criar FileData a partir do metadata criptografado
          final fileData = FileData.fromSecureMetadata(metadata);

          // Já foi salvo no DatabaseService pelo SecureStorageService
          // Apenas recarregar a lista
        }

        await loadFiles();
      }
    } catch (e) {
      print('Upload error: $e');
      state = AsyncValue.error(e, StackTrace.current);
    }
  }

  /// Download e descriptografia de arquivo
  Future<Uint8List?> downloadFile(String fileId) async {
    try {
      return await SecureStorageService.downloadFile(fileId);
    } catch (e) {
      print('Download error: $e');
      return null;
    }
  }

  /// Verificar integridade de um arquivo
  Future<bool> verifyFileIntegrity(String fileId) async {
    try {
      final report = await SecureStorageService.verifyFileIntegrity(fileId);
      return report.isValid;
    } catch (e) {
      return false;
    }
  }
}

final filesNotifierProvider =
    StateNotifierProvider<FilesNotifier, AsyncValue<List<FileData>>>((ref) {
  return FilesNotifier();
});

/// Recent files provider
final recentFilesProvider = Provider<AsyncValue<List<FileData>>>((ref) {
  final filesState = ref.watch(filesNotifierProvider);
  return filesState.whenData((files) => files.take(10).toList());
});

/// All files provider (with folder filter)
final allFilesProvider =
    Provider.family<AsyncValue<List<FileData>>, String?>((ref, folderId) {
  final filesState = ref.watch(filesNotifierProvider);
  return filesState.whenData((files) {
    if (folderId == null) {
      return files.where((f) => f.folderId == null).toList();
    }
    return files.where((f) => f.folderId == folderId).toList();
  });
});

/// Folders notifier
class FoldersNotifier extends StateNotifier<AsyncValue<List<FolderData>>> {
  FoldersNotifier() : super(const AsyncValue.loading()) {
    loadFolders();
  }

  Future<void> loadFolders({bool showLoading = true}) async {
    if (showLoading) {
      state = const AsyncValue.loading();
    }
    try {
      final records = DatabaseService.getAllFolders();
      final folders = records.map((r) {
        final fileCount = DatabaseService.getFileCountInFolder(r.id);
        return FolderData.fromRecord(r, fileCount);
      }).toList();
      folders.sort((a, b) => b.createdAt.compareTo(a.createdAt));
      state = AsyncValue.data(folders);
    } catch (e, st) {
      state = AsyncValue.error(e, st);
    }
  }

  Future<void> createFolder(String name, {String? parentId}) async {
    try {
      final now = DateTime.now();
      final record = FolderRecord(
        id: 'folder_${now.millisecondsSinceEpoch}',
        name: name,
        parentId: parentId,
        createdAt: now,
      );
      await DatabaseService.saveFolder(record);
      await loadFolders(showLoading: false);
    } catch (e) {
      print('Error creating folder: $e');
      rethrow;
    }
  }

  Future<void> deleteFolder(String folderId) async {
    await DatabaseService.deleteFolder(folderId);
    await loadFolders();
  }
}

final foldersNotifierProvider =
    StateNotifierProvider<FoldersNotifier, AsyncValue<List<FolderData>>>((ref) {
  return FoldersNotifier();
});

/// Folders provider (backwards compatible)
final foldersProvider = Provider<AsyncValue<List<FolderData>>>((ref) {
  return ref.watch(foldersNotifierProvider);
});

/// File search provider
final fileSearchProvider =
    Provider.family<AsyncValue<List<FileData>>, String>((ref, query) {
  if (query.isEmpty) return const AsyncValue.data([]);

  final filesState = ref.watch(filesNotifierProvider);
  final lowerQuery = query.toLowerCase();

  return filesState.whenData((files) {
    return files.where((f) {
      return f.fileName.toLowerCase().contains(lowerQuery) ||
          f.tags.any((t) => t.toLowerCase().contains(lowerQuery));
    }).toList();
  });
});

/// Upload state
class UploadState {
  final List<UploadTask> tasks;

  const UploadState({this.tasks = const []});
}

class UploadTask {
  final String taskId;
  final String fileName;
  final int totalBytes;
  final int uploadedBytes;
  final UploadStage stage;

  const UploadTask({
    required this.taskId,
    required this.fileName,
    required this.totalBytes,
    required this.uploadedBytes,
    required this.stage,
  });

  double get progress => totalBytes > 0 ? uploadedBytes / totalBytes : 0;
}

enum UploadStage {
  preparing,
  encrypting,
  encoding,
  distributing,
  complete,
  failed,
}

/// Upload controller
class UploadController extends StateNotifier<UploadState> {
  UploadController() : super(const UploadState());

  Future<void> uploadFile(String filePath, String fileName) async {
    final taskId = DateTime.now().millisecondsSinceEpoch.toString();

    state = UploadState(
      tasks: [
        ...state.tasks,
        UploadTask(
          taskId: taskId,
          fileName: fileName,
          totalBytes: 100,
          uploadedBytes: 0,
          stage: UploadStage.preparing,
        ),
      ],
    );

    // Simulate upload progress
    for (var i = 0; i <= 100; i += 10) {
      await Future.delayed(const Duration(milliseconds: 200));
      _updateTask(taskId, i, _stageFromProgress(i));
    }
  }

  UploadStage _stageFromProgress(int progress) {
    if (progress < 20) return UploadStage.preparing;
    if (progress < 40) return UploadStage.encrypting;
    if (progress < 60) return UploadStage.encoding;
    if (progress < 100) return UploadStage.distributing;
    return UploadStage.complete;
  }

  void _updateTask(String taskId, int uploadedBytes, UploadStage stage) {
    state = UploadState(
      tasks: state.tasks.map((t) {
        if (t.taskId == taskId) {
          return UploadTask(
            taskId: t.taskId,
            fileName: t.fileName,
            totalBytes: t.totalBytes,
            uploadedBytes: uploadedBytes,
            stage: stage,
          );
        }
        return t;
      }).toList(),
    );
  }

  void removeTask(String taskId) {
    state = UploadState(
      tasks: state.tasks.where((t) => t.taskId != taskId).toList(),
    );
  }
}

final uploadProvider =
    StateNotifierProvider<UploadController, UploadState>((ref) {
  return UploadController();
});

// Helper
String _getMimeType(String fileName) {
  final ext = fileName.split('.').last.toLowerCase();
  const mimeTypes = {
    'jpg': 'image/jpeg',
    'jpeg': 'image/jpeg',
    'png': 'image/png',
    'gif': 'image/gif',
    'pdf': 'application/pdf',
    'doc': 'application/msword',
    'docx': 'application/vnd.openxmlformats-officedocument.wordprocessingml.document',
    'xls': 'application/vnd.ms-excel',
    'xlsx': 'application/vnd.openxmlformats-officedocument.spreadsheetml.sheet',
    'mp4': 'video/mp4',
    'mp3': 'audio/mpeg',
    'zip': 'application/zip',
    'txt': 'text/plain',
  };
  return mimeTypes[ext] ?? 'application/octet-stream';
}
