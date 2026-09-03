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

  /// Patch an existing address with the full editable surface (the
  /// location screen's edit mode). The note rides along: under the
  /// server's S32 semantics an absent/null field KEEPS the saved note,
  /// so a cleared note field must send the empty string — the wire's
  /// explicit-clear value — or the rider would keep a note the customer
  /// deleted.
  Future<Address> update(Address address) async {
    final response = await _client.patch(
      '/addresses/${address.id}',
      body: {
        'label': address.label,
        'address_text': address.addressText,
        'lat': address.lat,
        'lng': address.lng,
        'is_default': address.isDefault,
        'kind': address.kind,
        'note': address.note ?? '',
      },
    );
    return Address.fromJson(response as Map<String, dynamic>);
  }

  /// Removes one saved address. Another customer's id is a 404 —
  /// indistinguishable from a missing one.
  Future<void> delete(String id) async {
    await _client.delete('/addresses/$id');
  }

  /// The address book API grows (delete) when a screen needs it;
  /// checkout + profile-location read, create, and edit today.
}
