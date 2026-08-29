/// Stand-in store facts until the server owns them: what the store
/// offers, how far it is, and how long delivery takes.
///
/// Deterministic per store name — the same store always shows the same
/// numbers (re-randomizing on every rebuild would look broken), exactly
/// like the Unsplash placeholder imagery. When real category/distance/
/// ETA fields land on the API, they simply win and this file retires.
typedef StoreFacts = ({String category, double km, int minutes});

/// Hot food names the placeholder category is picked from.
const List<String> _hotFoods = [
  'Brochettes',
  'Grill & BBQ',
  'Burgers',
  'Fried Chicken',
  'Pizza',
  'Rice & Sauce',
  'Chapati & Beans',
  'Samosas',
  'Kebabs',
  'Hot Dogs',
];

StoreFacts placeholderStoreFacts(String seed) {
  final category = _hotFoods[_stableHash('$seed:category') % _hotFoods.length];
  // Both floats in a range of 10, per the founder's placeholder ask.
  final km = 1 + (_stableHash('$seed:km') % 90) / 10; // 1.0–9.9 km
  final minutes = 1 + (_stableHash('$seed:minutes') % 90) ~/ 10; // 1–9 min
  return (category: category, km: km, minutes: minutes);
}

/// djb2 — the same stable hash the placeholder imagery uses.
int _stableHash(String input) {
  var hash = 5381;
  for (final unit in input.codeUnits) {
    hash = ((hash << 5) + hash + unit) & 0x7fffffff;
  }
  return hash;
}
