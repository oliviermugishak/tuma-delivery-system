/// The authenticated user, as returned by `/v1/me` and `/v1/auth/otp/verify`.
class AuthenticatedUser {
  AuthenticatedUser({
    required this.id,
    required this.role,
    this.name,
    this.phone,
    this.email,
  });

  final String id;
  final String role;
  final String? name;
  final String? phone;
  final String? email;

  factory AuthenticatedUser.fromJson(Map<String, dynamic> json) =>
      AuthenticatedUser(
        id: json['id'] as String,
        role: json['role'] as String,
        name: json['name'] as String?,
        phone: json['phone'] as String?,
        email: json['email'] as String?,
      );

  bool get isCustomer => role == 'customer';
  bool get isMerchant => role == 'merchant';
  bool get isAdmin => role == 'admin';
}
