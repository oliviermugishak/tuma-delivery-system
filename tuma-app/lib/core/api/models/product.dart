/// One sellable line of a store's menu — the `MenuItemResponse` of the
/// OpenAPI contract: a store_product joined with its catalog identity.
/// The customer buys this (per-store price), never an abstract product.
/// Price is integer RWF — the server owns money.
class MenuItem {
  MenuItem({
    required this.id,
    required this.name,
    required this.price,
    required this.isAvailable,
    this.description,
    this.imageUrl,
    this.images = const [],
  });

  /// The store_product id — what checkout sends.
  final String id;
  final String name;
  final String? description;
  final int price;
  final String? imageUrl;

  /// The product's full gallery, cover first — composed URLs straight
  /// from the server. Empty when the merchant uploaded nothing. Up to
  /// eight per product (server-enforced).
  final List<String> images;

  /// The image to render wherever one picture suffices: the gallery
  /// cover, else the legacy URL, else null (icon fallback).
  String? get displayImage => images.isNotEmpty ? images.first : imageUrl;

  final bool isAvailable;

  factory MenuItem.fromJson(Map<String, dynamic> json) => MenuItem(
        id: json['id'] as String,
        name: json['name'] as String,
        description: json['description'] as String?,
        price: json['price'] as int,
        imageUrl: json['image_url'] as String?,
        images: (json['images'] as List?)
                ?.whereType<String>()
                .toList() ??
            const [],
        isAvailable: json['is_available'] as bool,
      );
}
