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
  });

  /// The store_product id — what checkout sends.
  final String id;
  final String name;
  final String? description;
  final int price;
  final String? imageUrl;
  final bool isAvailable;

  factory MenuItem.fromJson(Map<String, dynamic> json) => MenuItem(
        id: json['id'] as String,
        name: json['name'] as String,
        description: json['description'] as String?,
        price: json['price'] as int,
        imageUrl: json['image_url'] as String?,
        isAvailable: json['is_available'] as bool,
      );
}
