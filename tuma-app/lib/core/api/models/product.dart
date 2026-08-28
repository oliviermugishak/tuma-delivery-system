/// A product as the API returns it (`ProductResponse` in the OpenAPI
/// contract). Price is integer RWF — the server owns money.
class Product {
  Product({
    required this.id,
    required this.storeId,
    required this.name,
    required this.price,
    required this.isAvailable,
    required this.createdAt,
    required this.updatedAt,
    this.description,
    this.imageUrl,
  });

  final String id;
  final String storeId;
  final String name;
  final String? description;
  final int price;
  final String? imageUrl;
  final bool isAvailable;

  /// RFC-3339 strings — kept as strings until a screen needs them parsed.
  final String createdAt;
  final String updatedAt;

  factory Product.fromJson(Map<String, dynamic> json) => Product(
        id: json['id'] as String,
        storeId: json['store_id'] as String,
        name: json['name'] as String,
        description: json['description'] as String?,
        price: json['price'] as int,
        imageUrl: json['image_url'] as String?,
        isAvailable: json['is_available'] as bool,
        createdAt: json['created_at'] as String,
        updatedAt: json['updated_at'] as String,
      );
}
