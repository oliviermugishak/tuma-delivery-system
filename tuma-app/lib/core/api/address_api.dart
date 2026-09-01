import 'package:tuma_app/core/api/api_client.dart';
import 'package:tuma_app/core/api/models/address.dart';

/// The saved-address book (GET/POST/PATCH/DELETE /v1/addresses).
/// Hand-written per the no-codegen rule.
class AddressApi {
  AddressApi(this._client);

  final ApiClient _client;

  /// The caller's addresses, default first, then newest.
  Future<List<Address>> list() async {
    final response = await _client.getList('/addresses');
    return response
        .whereType<Map<String, dynamic>>()
        .map(Address.fromJson)
        .toList();
  }

  /// Save a new address; the first one becomes the default.
  Future<Address> create(Address address) async {
    final response =
        await _client.post('/addresses', body: address.toCreateJson());
    return Address.fromJson(response as Map<String, dynamic>);
  }

  /// The address book API grows (update/delete) when a screen needs them;
  /// checkout + profile-location only read and create today.
}
