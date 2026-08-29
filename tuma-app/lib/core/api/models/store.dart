import 'package:tuma_app/core/api/models/product.dart';

/// A store as the API returns it (`StoreResponse` in the OpenAPI contract).
/// Delivery fee is integer RWF — the server owns money.
class Store {
  Store({
    required this.id,
    required this.merchantId,
    required this.name,
    required this.deliveryFee,
    required this.isOpen,
    required this.createdAt,
    required this.updatedAt,
    this.description,
    this.imageUrl,
    this.addressText,
    this.category,
    this.lat,
    this.lng,
  });

  final String id;
  final String merchantId;
  final String name;
  final String? description;
  final String? imageUrl;
  final String? addressText;
  /// What the store sells, in a word or two — server-owned taxonomy.
  final String? category;
  final double? lat;
  final double? lng;
  final int deliveryFee;
  final bool isOpen;

  /// RFC-3339 strings — kept as strings until a screen needs them parsed.
  final String createdAt;
  final String updatedAt;

  factory Store.fromJson(Map<String, dynamic> json) => Store(
        id: json['id'] as String,
        merchantId: json['merchant_id'] as String,
        name: json['name'] as String,
        description: json['description'] as String?,
        imageUrl: json['image_url'] as String?,
        addressText: json['address_text'] as String?,
        category: json['category'] as String?,
        lat: (json['lat'] as num?)?.toDouble(),
        lng: (json['lng'] as num?)?.toDouble(),
        deliveryFee: json['delivery_fee'] as int,
        isOpen: json['is_open'] as bool,
        createdAt: json['created_at'] as String,
        updatedAt: json['updated_at'] as String,
      );
}

/// `GET /v1/stores/{id}` — the store plus its available products.
class StoreDetail {
  StoreDetail({required this.store, required this.products});

  final Store store;
  final List<MenuItem> products;

  factory StoreDetail.fromJson(Map<String, dynamic> json) => StoreDetail(
        store: Store.fromJson(json['store'] as Map<String, dynamic>),
        products: (json['products'] as List)
            .whereType<Map<String, dynamic>>()
            .map(MenuItem.fromJson)
            .toList(),
      );
}
