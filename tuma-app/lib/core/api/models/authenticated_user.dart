/// The authenticated account, as returned by `/v1/me` and
/// `/v1/auth/otp/verify`. The account itself is roleless — what it can do
/// is visible in its profiles and memberships.
class AuthenticatedUser {
  AuthenticatedUser({
    required this.id,
    this.phone,
    this.email,
    this.customer,
    this.admin,
    this.merchantMemberships = const [],
  });

  final String id;
  final String? phone;
  final String? email;
  final ProfileInfo? customer;
  final ProfileInfo? admin;
  final List<MerchantMembership> merchantMemberships;

  factory AuthenticatedUser.fromJson(Map<String, dynamic> json) =>
      AuthenticatedUser(
        id: json['id'] as String,
        phone: json['phone'] as String?,
        email: json['email'] as String?,
        customer: (json['customer'] as Map<String, dynamic>?)
            ?.let(ProfileInfo.fromJson),
        admin: (json['admin'] as Map<String, dynamic>?)?.let(ProfileInfo.fromJson),
        merchantMemberships: (json['merchant_memberships'] as List? ?? [])
            .whereType<Map<String, dynamic>>()
            .map(MerchantMembership.fromJson)
            .toList(),
      );

  bool get isCustomer => customer != null;
  bool get isAdmin => admin != null;
  bool get isMerchantOperator => merchantMemberships.isNotEmpty;

  /// The display name across profiles — the customer's name, else the
  /// admin's. Operators manage businesses, not personal profiles.
  String? get displayName {
    final customerName = customer?.name;
    if (customerName != null && customerName.isNotEmpty) return customerName;
    final adminName = admin?.name;
    if (adminName != null && adminName.isNotEmpty) return adminName;
    return null;
  }
}

extension _Let<T> on T {
  R let<R>(R Function(T) transform) => transform(this);
}

/// A customer or admin profile: an id plus the display name.
class ProfileInfo {
  ProfileInfo({required this.id, this.name});

  final String id;
  final String? name;

  factory ProfileInfo.fromJson(Map<String, dynamic> json) => ProfileInfo(
        id: json['id'] as String,
        name: json['name'] as String?,
      );
}

/// A merchant membership: which business this account acts for, with what
/// authority. `storeId` null covers the whole business; set, the member is
/// scoped to that one store.
class MerchantMembership {
  MerchantMembership({
    required this.merchantId,
    required this.merchantName,
    required this.role,
    required this.merchantStatus,
    this.storeId,
  });

  final String merchantId;
  final String merchantName;
  /// `owner` or `manager`.
  final String role;
  /// `active` or `suspended`.
  final String merchantStatus;
  final String? storeId;

  factory MerchantMembership.fromJson(Map<String, dynamic> json) =>
      MerchantMembership(
        merchantId: json['merchant_id'] as String,
        merchantName: json['merchant_name'] as String,
        role: json['role'] as String,
        merchantStatus: json['merchant_status'] as String,
        storeId: json['store_id'] as String?,
      );
}
