import 'package:tuma_app/core/api/api_client.dart';
import 'package:tuma_app/core/api/models/authenticated_user.dart';
import 'package:tuma_app/core/api/models/otp_verify_result.dart';

/// Typed wrapper around the auth endpoints. Hand-written per the
/// foundation's "no codegen for mobile" rule (see AGENTS.md).
class AuthApi {
  AuthApi(this._client);

  final ApiClient _client;

  /// Asks the server to issue an OTP. Always returns normally — the server
  /// never reveals whether the phone is known.
  Future<void> requestOtp(String phone) async {
    await _client.post('/auth/otp/request', body: {'phone': phone});
  }

  /// Verifies the code and on success returns the token + the user it was
  /// minted for. Register and login are one flow; the server upserts the
  /// user. Throws [ApiBadRequest] on an invalid/expired code.
  Future<OtpVerifyResult> verifyOtp({
    required String phone,
    required String code,
    String? name,
  }) async {
    final body = <String, dynamic>{'phone': phone, 'code': code};
    if (name != null && name.isNotEmpty) {
      body['name'] = name;
    }
    final response = await _client.post('/auth/otp/verify', body: body);
    return OtpVerifyResult(
      token: response['token'] as String,
      user: AuthenticatedUser.fromJson(response['user'] as Map<String, dynamic>),
    );
  }

  /// Returns the current user or throws [ApiUnauthorized] if no valid
  /// token is set on the client.
  Future<AuthenticatedUser> me() async {
    final response = await _client.get('/me');
    return AuthenticatedUser.fromJson(response as Map<String, dynamic>);
  }

  /// Updates the caller's own name (`PATCH /me`). Used by the name-capture
  /// step: the OTP code is consumed by verify, so a first-time user's name
  /// lands here rather than in a second verify call. Pass [token] — the
  /// session hasn't accepted it yet at that point in the flow.
  Future<AuthenticatedUser> updateName(String name, {String? token}) async {
    final response =
        await _client.patch('/me', body: {'name': name}, token: token);
    return AuthenticatedUser.fromJson(response as Map<String, dynamic>);
  }

  /// Invalidates the session server-side. Bearer clients have no refresh
  /// row to revoke; the client is expected to drop the token locally.
  Future<void> logout() async {
    await _client.post('/auth/logout');
  }
}
