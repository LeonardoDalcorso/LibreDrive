import 'dart:convert';
import 'dart:typed_data';
import 'package:hive_flutter/hive_flutter.dart';

import 'crypto_service.dart';
import 'database_service.dart';

/// Serviço de Storage Seguro Descentralizado
///
/// Armazena arquivos com:
/// - Criptografia AES-256-GCM
/// - Fragmentação com redundância (shards)
/// - Verificação de integridade via Merkle Tree
/// - Preparado para distribuição P2P
class SecureStorageService {
  static const String _shardsBoxName = 'encrypted_shards';
  static const String _filesMetaBoxName = 'files_metadata';

  static Box<String>? _shardsBox;
  static Box<String>? _filesMetaBox;

  /// Inicializa o storage seguro
  static Future<void> initialize() async {
    _shardsBox = await Hive.openBox<String>(_shardsBoxName);
    _filesMetaBox = await Hive.openBox<String>(_filesMetaBoxName);
  }

  /// Upload de arquivo com criptografia completa
  static Future<SecureFileMetadata> uploadFile({
    required Uint8List fileData,
    required String fileName,
    String? folderId,
  }) async {
    // 1. Gerar ID único para o arquivo
    final fileId = CryptoService.generateFileId(fileData, fileName);

    // 2. Criptografar arquivo (com chunks e Merkle Tree)
    final encryptedFile = await CryptoService.encryptFile(fileData, fileId);

    // 3. Criar shards com redundância
    final shards = CryptoService.createShards(encryptedFile);

    // 4. Salvar shards localmente (em produção seriam distribuídos P2P)
    for (final shard in shards) {
      await _saveShard(shard);
    }

    // 5. Criar e salvar metadata
    final metadata = SecureFileMetadata(
      fileId: fileId,
      fileName: fileName,
      originalSize: fileData.length,
      encryptedSize: encryptedFile.chunks.fold<int>(
        0,
        (sum, c) => sum + c.data.ciphertext.length,
      ),
      merkleRoot: encryptedFile.merkleRoot,
      originalHash: encryptedFile.originalHash,
      shardIds: shards.map((s) => s.shardId).toList(),
      totalShards: shards.length,
      dataShards: 10,
      parityShards: 4,
      folderId: folderId,
      createdAt: DateTime.now(),
      status: FileEncryptionStatus.encrypted,
    );

    await _saveFileMetadata(metadata);

    // 6. Salvar no banco de dados principal também
    await DatabaseService.saveFile(FileRecord(
      id: fileId,
      name: fileName,
      path: '', // Não usamos path, está criptografado
      size: fileData.length,
      mimeType: _getMimeType(fileName),
      createdAt: DateTime.now(),
      modifiedAt: DateTime.now(),
      folderId: folderId,
      status: 'encrypted',
    ));

    return metadata;
  }

  /// Download e descriptografia de arquivo
  static Future<Uint8List> downloadFile(String fileId) async {
    // 1. Buscar metadata
    final metadata = await _getFileMetadata(fileId);
    if (metadata == null) {
      throw SecureStorageException('File not found: $fileId');
    }

    // 2. Buscar shards
    final shards = <Shard?>[];
    for (final shardId in metadata.shardIds) {
      final shard = await _getShard(shardId);
      shards.add(shard);
    }

    // 3. Verificar se temos shards suficientes
    final availableShards = shards.where((s) => s != null).length;
    if (availableShards < metadata.dataShards) {
      throw SecureStorageException(
        'Not enough shards available: $availableShards/${metadata.dataShards}',
      );
    }

    // 4. Reconstruir arquivo criptografado
    final encryptedFile = CryptoService.reconstructFromShards(
      shards,
      metadata.dataShards,
    );

    // 5. Descriptografar
    final decrypted = await CryptoService.decryptFile(encryptedFile);

    // 6. Verificar hash final
    final hash = CryptoService.sha256Hash(decrypted);
    if (hash != metadata.originalHash) {
      throw SecureStorageException('File integrity verification failed!');
    }

    return decrypted;
  }

  /// Deletar arquivo e seus shards
  static Future<void> deleteFile(String fileId) async {
    final metadata = await _getFileMetadata(fileId);
    if (metadata == null) return;

    // Deletar todos os shards
    for (final shardId in metadata.shardIds) {
      await _shardsBox?.delete(shardId);
    }

    // Deletar metadata
    await _filesMetaBox?.delete(fileId);

    // Deletar do banco principal
    await DatabaseService.deleteFile(fileId);
  }

  /// Lista todos os arquivos criptografados
  static Future<List<SecureFileMetadata>> listFiles() async {
    final files = <SecureFileMetadata>[];

    for (final key in _filesMetaBox?.keys ?? []) {
      final json = _filesMetaBox?.get(key);
      if (json != null) {
        files.add(SecureFileMetadata.fromJson(jsonDecode(json)));
      }
    }

    return files;
  }

  /// Verifica integridade de um arquivo
  static Future<IntegrityReport> verifyFileIntegrity(String fileId) async {
    final metadata = await _getFileMetadata(fileId);
    if (metadata == null) {
      return IntegrityReport(
        fileId: fileId,
        isValid: false,
        errors: ['File metadata not found'],
        shardsChecked: 0,
        shardsValid: 0,
      );
    }

    final errors = <String>[];
    var shardsValid = 0;

    for (final shardId in metadata.shardIds) {
      final shard = await _getShard(shardId);
      if (shard == null) {
        errors.add('Shard missing: $shardId');
        continue;
      }

      // Verificar hash do shard
      final calculatedHash = CryptoService.sha256Hash(shard.data);
      if (calculatedHash != shard.hash) {
        errors.add('Shard corrupted: $shardId');
        continue;
      }

      shardsValid++;
    }

    final isValid = shardsValid >= metadata.dataShards && errors.isEmpty;

    return IntegrityReport(
      fileId: fileId,
      isValid: isValid,
      errors: errors,
      shardsChecked: metadata.shardIds.length,
      shardsValid: shardsValid,
    );
  }

  /// Estatísticas de storage
  static Future<SecureStorageStats> getStats() async {
    final files = await listFiles();

    var totalOriginalSize = 0;
    var totalEncryptedSize = 0;
    var totalShards = 0;

    for (final file in files) {
      totalOriginalSize += file.originalSize;
      totalEncryptedSize += file.encryptedSize;
      totalShards += file.totalShards;
    }

    return SecureStorageStats(
      totalFiles: files.length,
      totalOriginalSize: totalOriginalSize,
      totalEncryptedSize: totalEncryptedSize,
      totalShards: totalShards,
      encryptionOverhead: totalOriginalSize > 0
          ? (totalEncryptedSize - totalOriginalSize) / totalOriginalSize
          : 0,
    );
  }

  // ==================== PRIVATE METHODS ====================

  static Future<void> _saveShard(Shard shard) async {
    await _shardsBox?.put(shard.shardId, jsonEncode(shard.toJson()));
  }

  static Future<Shard?> _getShard(String shardId) async {
    final json = _shardsBox?.get(shardId);
    if (json == null) return null;
    return Shard.fromJson(jsonDecode(json));
  }

  static Future<void> _saveFileMetadata(SecureFileMetadata metadata) async {
    await _filesMetaBox?.put(metadata.fileId, jsonEncode(metadata.toJson()));
  }

  static Future<SecureFileMetadata?> _getFileMetadata(String fileId) async {
    final json = _filesMetaBox?.get(fileId);
    if (json == null) return null;
    return SecureFileMetadata.fromJson(jsonDecode(json));
  }

  static String _getMimeType(String fileName) {
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
}

// ==================== DATA CLASSES ====================

class SecureFileMetadata {
  final String fileId;
  final String fileName;
  final int originalSize;
  final int encryptedSize;
  final String merkleRoot;
  final String originalHash;
  final List<String> shardIds;
  final int totalShards;
  final int dataShards;
  final int parityShards;
  final String? folderId;
  final DateTime createdAt;
  final FileEncryptionStatus status;

  SecureFileMetadata({
    required this.fileId,
    required this.fileName,
    required this.originalSize,
    required this.encryptedSize,
    required this.merkleRoot,
    required this.originalHash,
    required this.shardIds,
    required this.totalShards,
    required this.dataShards,
    required this.parityShards,
    this.folderId,
    required this.createdAt,
    required this.status,
  });

  Map<String, dynamic> toJson() => {
    'fileId': fileId,
    'fileName': fileName,
    'originalSize': originalSize,
    'encryptedSize': encryptedSize,
    'merkleRoot': merkleRoot,
    'originalHash': originalHash,
    'shardIds': shardIds,
    'totalShards': totalShards,
    'dataShards': dataShards,
    'parityShards': parityShards,
    'folderId': folderId,
    'createdAt': createdAt.toIso8601String(),
    'status': status.name,
  };

  factory SecureFileMetadata.fromJson(Map<String, dynamic> json) => SecureFileMetadata(
    fileId: json['fileId'],
    fileName: json['fileName'],
    originalSize: json['originalSize'],
    encryptedSize: json['encryptedSize'],
    merkleRoot: json['merkleRoot'],
    originalHash: json['originalHash'],
    shardIds: List<String>.from(json['shardIds']),
    totalShards: json['totalShards'],
    dataShards: json['dataShards'],
    parityShards: json['parityShards'],
    folderId: json['folderId'],
    createdAt: DateTime.parse(json['createdAt']),
    status: FileEncryptionStatus.values.byName(json['status']),
  );
}

enum FileEncryptionStatus {
  pending,
  encrypting,
  encrypted,
  distributing,
  distributed,
  error,
}

class IntegrityReport {
  final String fileId;
  final bool isValid;
  final List<String> errors;
  final int shardsChecked;
  final int shardsValid;

  IntegrityReport({
    required this.fileId,
    required this.isValid,
    required this.errors,
    required this.shardsChecked,
    required this.shardsValid,
  });
}

class SecureStorageStats {
  final int totalFiles;
  final int totalOriginalSize;
  final int totalEncryptedSize;
  final int totalShards;
  final double encryptionOverhead;

  SecureStorageStats({
    required this.totalFiles,
    required this.totalOriginalSize,
    required this.totalEncryptedSize,
    required this.totalShards,
    required this.encryptionOverhead,
  });
}

class SecureStorageException implements Exception {
  final String message;
  SecureStorageException(this.message);

  @override
  String toString() => 'SecureStorageException: $message';
}
