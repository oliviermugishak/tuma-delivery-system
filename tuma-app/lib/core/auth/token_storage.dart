import 'package:flutter_secure_storage/flutter_secure_storage.dart';

/// Persists the 30-day mobile JWT. flutter_secure_storage uses the platform
/// keystore (Keychain on iOS, EncryptedSharedPreferences on Android, libsecret
/// on Linux). Storing the token here means the next launch can route straight
/// to the home screen.
class TokenStorage {
  TokenStorage({FlutterSecureStorage? storage})
      : _storage = storage ??
            const FlutterSecureStorage(
              aOptions: AndroidOptions(encryptedSharedPreferences: true),
            );

  static const String _tokenKey = 'tuma.auth.token';
  static const String _userKey = 'tuma.auth.user.id';

  final FlutterSecureStorage _storage;

  Future<String?> readToken() => _storage.read(key: _tokenKey);

  Future<void> writeToken(String token) =>
      _storage.write(key: _tokenKey, value: token);

  Future<void> clear() async {
    await _storage.delete(key: _tokenKey);
    await _storage.delete(key: _userKey);
  }
}
