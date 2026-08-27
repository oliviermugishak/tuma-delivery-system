import 'dart:ui';

/// Tuma brand tokens.
///
/// The brand mark is a gold rider-glove over a deep-navy field. Tokens are
/// the only place raw hex values may appear in the app (see AGENTS.md).
abstract final class AppColors {
  // Brand -----------------------------------------------------------------
  /// Gold — primary action, the brand mark itself, filled CTAs.
  static const Color primary = Color(0xFFFDBB01);

  /// Orange — emphasis and energy where the gold is too soft.
  static const Color primaryDeep = Color(0xFFFC7701);

  // Surfaces --------------------------------------------------------------
  /// Dark navy scaffold — the field the gold mark sits on.
  static const Color surface = Color(0xFF061323);

  /// Lighter navy — sheets, cards, app bars over the dark scaffold.
  static const Color surfaceAlt = Color(0xFF0B1A3D);

  /// Borders and dividers on dark surfaces.
  static const Color surfaceBorder = Color(0x14D3DEE9);

  // Text ------------------------------------------------------------------
  /// Primary text on dark surfaces.
  static const Color onSurface = Color(0xFFD3DEE9);

  /// Muted text on dark surfaces.
  static const Color onSurfaceMuted = Color(0x99D3DEE9);

  /// Text placed on gold/orange fills.
  static const Color onPrimary = Color(0xFF061323);

  // Feedback --------------------------------------------------------------
  static const Color error = Color(0xFFFF6B6B);
  static const Color success = Color(0xFF4ECDC4);
}
