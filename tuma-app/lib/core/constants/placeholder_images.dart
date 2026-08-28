/// Stand-in imagery until real uploading lands.
///
/// Store and product `image_url`s are null until the upload slice exists;
/// these verified Unsplash CDN URLs give the catalogue a real face in the
/// meantime. [placeholderImageFor] picks deterministically from the pool,
/// so a store or product keeps its picture across rebuilds and restarts.
/// When uploading arrives, real URLs simply win in `RemoteImage` — no UI
/// changes needed.
library;

const List<String> _placeholderImages = [
  'https://images.unsplash.com/photo-1504674900247-0877df9cc836?auto=format&fit=crop&w=900&q=70',
  'https://images.unsplash.com/photo-1555396273-367ea4eb4db5?auto=format&fit=crop&w=900&q=70',
  'https://images.unsplash.com/photo-1414235077428-338989a2e8c0?auto=format&fit=crop&w=900&q=70',
  'https://images.unsplash.com/photo-1517248135467-4c7edcad34c4?auto=format&fit=crop&w=900&q=70',
  'https://images.unsplash.com/photo-1546069901-ba9599a7e63c?auto=format&fit=crop&w=900&q=70',
  'https://images.unsplash.com/photo-1567620905732-2d1ec7ab7445?auto=format&fit=crop&w=900&q=70',
  'https://images.unsplash.com/photo-1565299624946-b28f40a0ae38?auto=format&fit=crop&w=900&q=70',
  'https://images.unsplash.com/photo-1568901346375-23c9450c58cd?auto=format&fit=crop&w=900&q=70',
  'https://images.unsplash.com/photo-1495474472287-4d71bcdd2085?auto=format&fit=crop&w=900&q=70',
  'https://images.unsplash.com/photo-1552566626-52f8b828add9?auto=format&fit=crop&w=900&q=70',
];

/// The placeholder for a given store or product: a stable hash of [seed]
/// (name or id) picks the picture, so it never flickers between rebuilds.
String placeholderImageFor(String seed) =>
    _placeholderImages[_stableHash(seed) % _placeholderImages.length];

/// djb2 — tiny, and stable across runs and platforms, unlike `hashCode`.
int _stableHash(String seed) {
  var hash = 5381;
  for (final unit in seed.codeUnits) {
    hash = ((hash << 5) + hash + unit) & 0x7fffffff;
  }
  return hash;
}
