import 'dart:convert';
import 'dart:math';
import 'dart:typed_data';
import 'package:cryptography/cryptography.dart';
import 'package:crypto/crypto.dart' as crypto;
import 'package:flutter_secure_storage/flutter_secure_storage.dart';

import 'seed_phrase_service.dart';

/// Serviço de Criptografia Descentralizado estilo Blockchain
///
/// Características:
/// - Criptografia AES-256-GCM (mesmo padrão do Bitcoin/Ethereum)
/// - Hash SHA-256 para integridade (blockchain-style)
/// - Derivação de chaves com HKDF
/// - Fragmentação com redundância (erasure coding simplificado)
/// - Merkle Tree para verificação de integridade
/// - Master key derivada da seed phrase BIP-39
class CryptoService {
  static final _secureStorage = const FlutterSecureStorage();
  static final _aesGcm = AesGcm.with256bits();
  static const _chunkSize = 64 * 1024; // 64KB chunks

  // ==================== MASTER KEY ====================

  /// Recupera a chave mestra derivada da seed phrase
  /// A mesma seed phrase SEMPRE gera a mesma master key
  static Future<SecretKey> getMasterKey() async {
    // Try to load master key from secure storage (derived from seed phrase)
    final masterKeyBytes = await SeedPhraseService.loadMasterKey();

    if (masterKeyBytes != null) {
      return SecretKey(masterKeyBytes);
    }

    // Fallback: check old storage format for backwards compatibility
    final stored = await _secureStorage.read(key: 'master_key');
    if (stored != null) {
      final bytes = base64Decode(stored);
      return SecretKey(bytes);
    }

    // If no master key exists, generate a temporary one
    // This should not happen in normal flow (user should create account first)
    final key = await _aesGcm.newSecretKey();
    final bytes = await key.extractBytes();
    await _secureStorage.write(key: 'master_key', value: base64Encode(bytes));

    return key;
  }

  /// Deriva uma chave específica para cada arquivo (blockchain-style)
  static Future<SecretKey> deriveFileKey(String fileId) async {
    final masterKey = await getMasterKey();
    final masterBytes = await masterKey.extractBytes();

    // HKDF para derivar chave única por arquivo
    final hkdf = Hkdf(hmac: Hmac.sha256(), outputLength: 32);
    final derivedKey = await hkdf.deriveKey(
      secretKey: SecretKey(masterBytes),
      nonce: utf8.encode(fileId),
      info: utf8.encode('libredrive-file-key-v1'),
    );

    return derivedKey;
  }

  // ==================== ENCRYPTION ====================

  /// Criptografa dados com AES-256-GCM
  static Future<EncryptedData> encrypt(Uint8List plaintext, SecretKey key) async {
    final nonce = _aesGcm.newNonce();

    final secretBox = await _aesGcm.encrypt(
      plaintext,
      secretKey: key,
      nonce: nonce,
    );

    return EncryptedData(
      ciphertext: Uint8List.fromList(secretBox.cipherText),
      nonce: Uint8List.fromList(secretBox.nonce),
      mac: Uint8List.fromList(secretBox.mac.bytes),
    );
  }

  /// Descriptografa dados
  static Future<Uint8List> decrypt(EncryptedData encrypted, SecretKey key) async {
    final secretBox = SecretBox(
      encrypted.ciphertext,
      nonce: encrypted.nonce,
      mac: Mac(encrypted.mac),
    );

    final plaintext = await _aesGcm.decrypt(secretBox, secretKey: key);
    return Uint8List.fromList(plaintext);
  }

  // ==================== FILE ENCRYPTION ====================

  /// Criptografa um arquivo completo com chunking
  static Future<EncryptedFile> encryptFile(Uint8List fileData, String fileId) async {
    final fileKey = await deriveFileKey(fileId);
    final chunks = <EncryptedChunk>[];
    final chunkHashes = <String>[];

    // Dividir em chunks e criptografar cada um
    for (var i = 0; i < fileData.length; i += _chunkSize) {
      final end = min(i + _chunkSize, fileData.length);
      final chunk = fileData.sublist(i, end);

      // Hash do chunk original (para Merkle Tree)
      final chunkHash = sha256Hash(chunk);
      chunkHashes.add(chunkHash);

      // Criptografar chunk
      final encrypted = await encrypt(Uint8List.fromList(chunk), fileKey);

      chunks.add(EncryptedChunk(
        index: chunks.length,
        data: encrypted,
        originalHash: chunkHash,
      ));
    }

    // Construir Merkle Root (blockchain-style)
    final merkleRoot = _buildMerkleRoot(chunkHashes);

    // Hash do arquivo original
    final originalHash = sha256Hash(fileData);

    return EncryptedFile(
      fileId: fileId,
      chunks: chunks,
      merkleRoot: merkleRoot,
      originalHash: originalHash,
      originalSize: fileData.length,
      chunkSize: _chunkSize,
      createdAt: DateTime.now(),
    );
  }

  /// Descriptografa um arquivo
  static Future<Uint8List> decryptFile(EncryptedFile file) async {
    final fileKey = await deriveFileKey(file.fileId);
    final decryptedChunks = <Uint8List>[];

    // Descriptografar cada chunk
    for (final chunk in file.chunks) {
      final decrypted = await decrypt(chunk.data, fileKey);

      // Verificar integridade do chunk
      final hash = sha256Hash(decrypted);
      if (hash != chunk.originalHash) {
        throw CryptoException('Chunk ${chunk.index} integrity check failed!');
      }

      decryptedChunks.add(decrypted);
    }

    // Juntar chunks
    final totalLength = decryptedChunks.fold<int>(0, (sum, c) => sum + c.length);
    final result = Uint8List(totalLength);
    var offset = 0;
    for (final chunk in decryptedChunks) {
      result.setRange(offset, offset + chunk.length, chunk);
      offset += chunk.length;
    }

    // Verificar hash final
    final finalHash = sha256Hash(result);
    if (finalHash != file.originalHash) {
      throw CryptoException('File integrity check failed!');
    }

    return result;
  }

  // ==================== HASHING (Blockchain-style) ====================

  /// SHA-256 hash (mesmo usado no Bitcoin)
  static String sha256Hash(List<int> data) {
    final digest = crypto.sha256.convert(data);
    return digest.toString();
  }

  /// Double SHA-256 (usado no Bitcoin para extra segurança)
  static String doubleSha256(List<int> data) {
    final first = crypto.sha256.convert(data);
    final second = crypto.sha256.convert(first.bytes);
    return second.toString();
  }

  /// Constrói Merkle Root dos chunks (blockchain-style)
  static String _buildMerkleRoot(List<String> hashes) {
    if (hashes.isEmpty) return sha256Hash([]);
    if (hashes.length == 1) return hashes.first;

    var currentLevel = List<String>.from(hashes);

    while (currentLevel.length > 1) {
      final nextLevel = <String>[];

      for (var i = 0; i < currentLevel.length; i += 2) {
        if (i + 1 < currentLevel.length) {
          // Combinar dois hashes
          final combined = currentLevel[i] + currentLevel[i + 1];
          nextLevel.add(sha256Hash(utf8.encode(combined)));
        } else {
          // Número ímpar, duplicar o último
          final combined = currentLevel[i] + currentLevel[i];
          nextLevel.add(sha256Hash(utf8.encode(combined)));
        }
      }

      currentLevel = nextLevel;
    }

    return currentLevel.first;
  }

  // ==================== ERASURE CODING (Redundância) ====================

  /// Cria shards com redundância (simplificado)
  /// Em produção usaria Reed-Solomon real
  static List<Shard> createShards(EncryptedFile file, {int dataShards = 10, int parityShards = 4}) {
    final shards = <Shard>[];
    final totalShards = dataShards + parityShards;

    // Serializar arquivo criptografado
    final fileBytes = file.serialize();
    final shardSize = (fileBytes.length / dataShards).ceil();

    // Criar data shards
    for (var i = 0; i < dataShards; i++) {
      final start = i * shardSize;
      final end = min(start + shardSize, fileBytes.length);
      final data = fileBytes.sublist(start, end);

      shards.add(Shard(
        index: i,
        data: Uint8List.fromList(data),
        hash: sha256Hash(data),
        isParityShard: false,
        fileId: file.fileId,
      ));
    }

    // Criar parity shards (XOR simples para demo)
    for (var i = 0; i < parityShards; i++) {
      final parityData = _createParityShard(shards, i, shardSize);
      shards.add(Shard(
        index: dataShards + i,
        data: parityData,
        hash: sha256Hash(parityData),
        isParityShard: true,
        fileId: file.fileId,
      ));
    }

    return shards;
  }

  static Uint8List _createParityShard(List<Shard> dataShards, int parityIndex, int size) {
    final result = Uint8List(size);

    for (var i = 0; i < dataShards.length; i++) {
      final shard = dataShards[i];
      for (var j = 0; j < min(size, shard.data.length); j++) {
        result[j] ^= shard.data[j];
      }
    }

    // Adicionar variação baseada no índice de paridade
    final random = Random(parityIndex);
    for (var i = 0; i < result.length; i++) {
      result[i] ^= random.nextInt(256);
    }

    return result;
  }

  /// Reconstrói arquivo a partir dos shards
  static EncryptedFile reconstructFromShards(List<Shard?> shards, int dataShards) {
    // Verificar se temos shards suficientes
    final availableShards = shards.where((s) => s != null).length;
    if (availableShards < dataShards) {
      throw CryptoException('Not enough shards: have $availableShards, need $dataShards');
    }

    // Coletar dados dos data shards disponíveis
    final dataBytes = <int>[];
    for (var i = 0; i < dataShards; i++) {
      if (shards[i] != null) {
        dataBytes.addAll(shards[i]!.data);
      }
    }

    return EncryptedFile.deserialize(Uint8List.fromList(dataBytes));
  }

  // ==================== IDENTITY ====================

  /// Gera um ID único para o usuário (estilo wallet address)
  static Future<String> generateUserId() async {
    final masterKey = await getMasterKey();
    final bytes = await masterKey.extractBytes();
    final hash = doubleSha256(bytes);
    return 'ld_${hash.substring(0, 40)}'; // Similar a endereço Ethereum
  }

  /// Gera ID único para arquivo
  static String generateFileId(Uint8List data, String fileName) {
    final combined = [...data, ...utf8.encode(fileName), ...utf8.encode(DateTime.now().toIso8601String())];
    return sha256Hash(combined);
  }
}

// ==================== DATA CLASSES ====================

class EncryptedData {
  final Uint8List ciphertext;
  final Uint8List nonce;
  final Uint8List mac;

  EncryptedData({
    required this.ciphertext,
    required this.nonce,
    required this.mac,
  });

  Uint8List serialize() {
    final buffer = BytesBuilder();
    // Format: [nonce_len(4)] [nonce] [mac_len(4)] [mac] [ciphertext]
    buffer.add(_intToBytes(nonce.length));
    buffer.add(nonce);
    buffer.add(_intToBytes(mac.length));
    buffer.add(mac);
    buffer.add(ciphertext);
    return buffer.toBytes();
  }

  static EncryptedData deserialize(Uint8List bytes) {
    var offset = 0;

    final nonceLen = _bytesToInt(bytes.sublist(offset, offset + 4));
    offset += 4;
    final nonce = bytes.sublist(offset, offset + nonceLen);
    offset += nonceLen;

    final macLen = _bytesToInt(bytes.sublist(offset, offset + 4));
    offset += 4;
    final mac = bytes.sublist(offset, offset + macLen);
    offset += macLen;

    final ciphertext = bytes.sublist(offset);

    return EncryptedData(
      ciphertext: Uint8List.fromList(ciphertext),
      nonce: Uint8List.fromList(nonce),
      mac: Uint8List.fromList(mac),
    );
  }
}

class EncryptedChunk {
  final int index;
  final EncryptedData data;
  final String originalHash;

  EncryptedChunk({
    required this.index,
    required this.data,
    required this.originalHash,
  });

  Map<String, dynamic> toJson() => {
    'index': index,
    'data': base64Encode(data.serialize()),
    'originalHash': originalHash,
  };

  factory EncryptedChunk.fromJson(Map<String, dynamic> json) => EncryptedChunk(
    index: json['index'],
    data: EncryptedData.deserialize(base64Decode(json['data'])),
    originalHash: json['originalHash'],
  );
}

class EncryptedFile {
  final String fileId;
  final List<EncryptedChunk> chunks;
  final String merkleRoot;
  final String originalHash;
  final int originalSize;
  final int chunkSize;
  final DateTime createdAt;

  EncryptedFile({
    required this.fileId,
    required this.chunks,
    required this.merkleRoot,
    required this.originalHash,
    required this.originalSize,
    required this.chunkSize,
    required this.createdAt,
  });

  Uint8List serialize() {
    final json = jsonEncode(toJson());
    return Uint8List.fromList(utf8.encode(json));
  }

  static EncryptedFile deserialize(Uint8List bytes) {
    final json = jsonDecode(utf8.decode(bytes));
    return EncryptedFile.fromJson(json);
  }

  Map<String, dynamic> toJson() => {
    'fileId': fileId,
    'chunks': chunks.map((c) => c.toJson()).toList(),
    'merkleRoot': merkleRoot,
    'originalHash': originalHash,
    'originalSize': originalSize,
    'chunkSize': chunkSize,
    'createdAt': createdAt.toIso8601String(),
  };

  factory EncryptedFile.fromJson(Map<String, dynamic> json) => EncryptedFile(
    fileId: json['fileId'],
    chunks: (json['chunks'] as List).map((c) => EncryptedChunk.fromJson(c)).toList(),
    merkleRoot: json['merkleRoot'],
    originalHash: json['originalHash'],
    originalSize: json['originalSize'],
    chunkSize: json['chunkSize'],
    createdAt: DateTime.parse(json['createdAt']),
  );
}

class Shard {
  final int index;
  final Uint8List data;
  final String hash;
  final bool isParityShard;
  final String fileId;

  Shard({
    required this.index,
    required this.data,
    required this.hash,
    required this.isParityShard,
    required this.fileId,
  });

  /// ID único do shard para DHT
  String get shardId => '${fileId}_shard_$index';

  Map<String, dynamic> toJson() => {
    'index': index,
    'data': base64Encode(data),
    'hash': hash,
    'isParityShard': isParityShard,
    'fileId': fileId,
  };

  factory Shard.fromJson(Map<String, dynamic> json) => Shard(
    index: json['index'],
    data: base64Decode(json['data']),
    hash: json['hash'],
    isParityShard: json['isParityShard'],
    fileId: json['fileId'],
  );
}

class CryptoException implements Exception {
  final String message;
  CryptoException(this.message);

  @override
  String toString() => 'CryptoException: $message';
}

// Helpers
Uint8List _intToBytes(int value) {
  return Uint8List(4)..buffer.asByteData().setInt32(0, value, Endian.big);
}

int _bytesToInt(List<int> bytes) {
  return Uint8List.fromList(bytes).buffer.asByteData().getInt32(0, Endian.big);
}
