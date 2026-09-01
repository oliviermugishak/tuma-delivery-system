/// A saved delivery address — the redesign's saved-address-first
/// checkout and the profile's "Delivery locations" list. Checkout
/// snapshots the chosen address onto the order; the row is a source,
/// never a live reference.
class Address {
  Address({
    required this.id,
    required this.label,
    required this.addressText,
    required this.isDefault,
    required this.kind,
    this.note,
    this.lat,
    this.lng,
  });

  final String id;
  final String label;
  final String addressText;
  final double? lat;
  final double? lng;
  final bool isDefault;
  /// Home / work / other — the save screen's label chips.
  final String kind;
  /// The rider note that travels with this address ("blue gate, ring the
  /// bell") — checkout pre-fills the order's rider note from it.
  final String? note;

  factory Address.fromJson(Map<String, dynamic> json) => Address(
        id: json['id'] as String,
        label: json['label'] as String,
        addressText: json['address_text'] as String,
        lat: (json['lat'] as num?)?.toDouble(),
        lng: (json['lng'] as num?)?.toDouble(),
        isDefault: json['is_default'] as bool? ?? false,
        kind: json['kind'] as String? ?? 'other',
        note: json['note'] as String?,
      );

  Map<String, dynamic> toCreateJson() => {
        'label': label,
        'address_text': addressText,
        'lat': lat,
        'lng': lng,
        'is_default': isDefault,
        'kind': kind,
        if (note != null) 'note': note,
      };
}
