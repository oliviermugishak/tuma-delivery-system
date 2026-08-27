import 'package:tuma_app/core/api/models/authenticated_user.dart';

/// Result of a successful `/v1/auth/otp/verify`: token + the user it was
/// minted for.
class OtpVerifyResult {
  OtpVerifyResult({required this.token, required this.user});

  final String token;
  final AuthenticatedUser user;
}
