import 'package:tuma_app/core/api/models/order.dart';
import 'package:tuma_app/core/api/models/store.dart';

/// One product hit from `GET /v1/search` (`ProductHitResponse`): the
/// store_product a customer buys — per-store price — dressed by its
/// catalog identity and the open store that fulfills it. Distance facts
/// are server-computed when the request carried a location; null
/// otherwise, never client math.
class ProductHit {
  ProductHit({
    required this.storeProductId,
    required this.storeId,
    required this.storeName,
    required this.name,
    required this.price,
    this.description,
    this.imageUrl,
    this.distanceM,
    this.etaMin,
  });

  final String storeProductId;
  final String storeId;
  final String storeName;
  final String name;
  final String? description;
  final int price; // integer RWF
  final String? imageUrl;
  final int? distanceM;
  final int? etaMin;

  factory ProductHit.fromJson(Map<String, dynamic> json) => ProductHit(
        storeProductId: json['store_product_id'] as String,
        storeId: json['store_id'] as String,
        storeName: json['store_name'] as String,
        name: json['name'] as String,
        description: json['description'] as String?,
        price: asInt(json['price']),
        imageUrl: json['image_url'] as String?,
        distanceM: (json['distance_m'] as num?)?.toInt(),
        etaMin: (json['eta_min'] as num?)?.toInt(),
      );
}

/// `GET /v1/search` — the discovery feed. Empty query: the most-purchased
/// products plus every open store; with a query: matched products and
/// stores. The server owns both sections and the open-store rule.
class SearchResult {
  SearchResult({required this.products, required this.stores});

  final List<ProductHit> products;
  final List<Store> stores;

  factory SearchResult.fromJson(Map<String, dynamic> json) => SearchResult(
        products: (json['products'] as List)
            .whereType<Map<String, dynamic>>()
            .map(ProductHit.fromJson)
            .toList(),
        stores: (json['stores'] as List)
            .whereType<Map<String, dynamic>>()
            .map(Store.fromJson)
            .toList(),
      );
}
