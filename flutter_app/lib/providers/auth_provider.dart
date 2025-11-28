import 'dart:convert';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_secure_storage/flutter_secure_storage.dart';

import '../services/seed_phrase_service.dart';

/// Auth state
class AuthState {
  final bool isLoggedIn;
  final String? publicId;
  final String? username;
  final bool isLoading;

  const AuthState({
    this.isLoggedIn = false,
    this.publicId,
    this.username,
    this.isLoading = false,
  });

  AuthState copyWith({
    bool? isLoggedIn,
    String? publicId,
    String? username,
    bool? isLoading,
  }) {
    return AuthState(
      isLoggedIn: isLoggedIn ?? this.isLoggedIn,
      publicId: publicId ?? this.publicId,
      username: username ?? this.username,
      isLoading: isLoading ?? this.isLoading,
    );
  }
}

/// Auth notifier with real BIP-39 seed phrase support
class AuthNotifier extends StateNotifier<AuthState> {
  final FlutterSecureStorage _storage = const FlutterSecureStorage();

  AuthNotifier() : super(const AuthState()) {
    _loadSavedAuth();
  }

  Future<void> _loadSavedAuth() async {
    state = state.copyWith(isLoading: true);
    try {
      final publicId = await _storage.read(key: 'public_id');
      final username = await _storage.read(key: 'username');
      final hasMasterKey = await SeedPhraseService.hasExistingIdentity();

      if (publicId != null && username != null && hasMasterKey) {
        state = AuthState(
          isLoggedIn: true,
          publicId: publicId,
          username: username,
          isLoading: false,
        );
      } else {
        state = const AuthState(isLoading: false);
      }
    } catch (e) {
      state = const AuthState(isLoading: false);
    }
  }

  /// Generate new seed phrase (12 words)
  List<String> generateNewSeedPhrase() {
    return SeedPhraseService.generateSeedPhrase();
  }

  /// Create account with generated seed phrase
  Future<void> createAccount({
    required String username,
    required String password,
    required List<String> seedWords,
  }) async {
    state = state.copyWith(isLoading: true);

    try {
      final seedPhrase = seedWords.join(' ');

      // Validate seed phrase
      if (!SeedPhraseService.validateSeedPhrase(seedPhrase)) {
        throw Exception('Invalid seed phrase');
      }

      // Derive master key from seed phrase
      final masterKey = await SeedPhraseService.deriveMasterKey(seedPhrase);

      // Generate public ID
      final publicId = SeedPhraseService.generatePublicId(masterKey);

      // Save master key
      await SeedPhraseService.saveMasterKey(masterKey);

      // Save encrypted seed phrase (optional, for backup viewing)
      await SeedPhraseService.saveSeedPhrase(seedPhrase, password);

      // Save user info
      await _storage.write(key: 'public_id', value: publicId);
      await _storage.write(key: 'username', value: username);
      await _storage.write(key: 'password_hash', value: _hashPassword(password));

      state = AuthState(
        isLoggedIn: true,
        publicId: publicId,
        username: username,
        isLoading: false,
      );
    } catch (e) {
      state = state.copyWith(isLoading: false);
      rethrow;
    }
  }

  /// Recover account using seed phrase
  Future<void> recoverAccount({
    required String seedPhrase,
    String? password,
  }) async {
    state = state.copyWith(isLoading: true);

    try {
      // Validate seed phrase
      if (!SeedPhraseService.validateSeedPhrase(seedPhrase)) {
        throw Exception('Frase de recuperação inválida. Verifique as palavras.');
      }

      // Derive the SAME master key from seed phrase
      final masterKey = await SeedPhraseService.deriveMasterKey(seedPhrase);

      // Generate the SAME public ID (deterministic!)
      final publicId = SeedPhraseService.generatePublicId(masterKey);

      // Save master key
      await SeedPhraseService.saveMasterKey(masterKey);

      // Save user info
      await _storage.write(key: 'public_id', value: publicId);
      await _storage.write(key: 'username', value: 'Recovered User');

      // If password provided, save encrypted seed
      if (password != null && password.isNotEmpty) {
        await SeedPhraseService.saveSeedPhrase(seedPhrase, password);
        await _storage.write(key: 'password_hash', value: _hashPassword(password));
      }

      state = AuthState(
        isLoggedIn: true,
        publicId: publicId,
        username: 'Recovered User',
        isLoading: false,
      );
    } catch (e) {
      state = state.copyWith(isLoading: false);
      rethrow;
    }
  }

  /// Update username
  Future<void> updateUsername(String username) async {
    await _storage.write(key: 'username', value: username);
    state = state.copyWith(username: username);
  }

  /// Logout and clear all data
  Future<void> logout() async {
    await SeedPhraseService.clearAll();
    await _storage.deleteAll();
    state = const AuthState();
  }

  /// Verify password
  Future<bool> verifyPassword(String password) async {
    final storedHash = await _storage.read(key: 'password_hash');
    if (storedHash == null) return false;
    return storedHash == _hashPassword(password);
  }

  /// Get seed phrase (requires password verification)
  Future<String?> getSeedPhrase(String password) async {
    if (!await verifyPassword(password)) return null;
    return SeedPhraseService.loadSeedPhrase(password);
  }

  /// Get current public ID
  String? get currentPublicId => state.publicId;

  String _hashPassword(String password) {
    final bytes = utf8.encode(password + 'libredrive-salt');
    var hash = bytes;
    for (var i = 0; i < 1000; i++) {
      hash = utf8.encode(hash.toString());
    }
    return base64Encode(hash);
  }
}

/// Providers
final authProvider = StateNotifierProvider<AuthNotifier, AuthState>((ref) {
  return AuthNotifier();
});

final isLoggedInProvider = Provider<bool>((ref) {
  return ref.watch(authProvider).isLoggedIn;
});

final currentUserProvider = Provider<String?>((ref) {
  return ref.watch(authProvider).username;
});

final publicIdProvider = Provider<String?>((ref) {
  return ref.watch(authProvider).publicId;
});

final isAuthLoadingProvider = Provider<bool>((ref) {
  return ref.watch(authProvider).isLoading;
});
