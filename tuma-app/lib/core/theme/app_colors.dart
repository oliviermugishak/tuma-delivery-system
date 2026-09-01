import 'dart:ui';

/// Tuma v2 design tokens — the redesigns' exact values (redesigns/*.html
/// `:root` block). Tokens are the only place raw hex values may appear in
/// the app (see AGENTS.md). The color vocabulary is deliberately closed
/// (P14): accent = action/active/money, teal = success/live, amber =
/// delay, red = destruction/failure only. Purple and mixed-orange are
/// deleted.
abstract final class AppColors {
  // Surfaces --------------------------------------------------------------
  /// The app background — everything sits on this.
  static const Color surface = Color(0xFF0A1022);

  /// Card surface — one step above the background.
  static const Color surfaceAlt = Color(0xFF111A33);

  /// High surface — chips, icon buttons, filled wells inside cards.
  static const Color surfaceHigh = Color(0xFF1A2547);

  /// The bottom nav's slightly distinct field.
  static const Color navSurface = Color(0xFF0B1226);

  /// Hairline: 8% white — the only divider/border language (P6).
  static const Color surfaceBorder = Color(0x14FFFFFF);

  /// A visible hairline for inner dividers on cards.
  static const Color line = Color(0xFF2A3559);

  // Text ------------------------------------------------------------------
  /// Primary text.
  static const Color onSurface = Color(0xFFF4F6FB);

  /// Secondary text — labels, metadata, captions.
  static const Color onSurfaceMuted = Color(0xFF9AA6C0);

  // Accent ----------------------------------------------------------------
  /// Accent — action, active state, money (P14). CTAs, prices, live
  /// progress, the fee pill, the cash strip.
  static const Color primary = Color(0xFFFBBF24);

  /// Text/icons placed on accent fills.
  static const Color onPrimary = Color(0xFF0A1022);

  /// The old orange is retired as a brand fill; kept ONLY as the deep
  /// flame/food tint color inside category tiles.
  static const Color primaryDeep = Color(0xFFFC7701);

  // Feedback --------------------------------------------------------------
  /// Success / live (P14) — delivered states, online, the pulsing dot.
  static const Color success = Color(0xFF2DD4BF);

  /// Delay (P14) — running late, the flame. Amber, never red: slowness
  /// isn't failure.
  static const Color warning = Color(0xFFF59E0B);

  /// Destruction/failure only (P14) — cancel, sign-out text, errors.
  static const Color error = Color(0xFFF87171);

  // Category tints (P15 — the tinted tile system) --------------------------
  /// Each merchant category carries a 135° two-stop tint + icon color.
  /// Background gradients are the tint color at .32 → .06 alpha.
  static const Color tintKfc = Color(0xFFE4002B);
  static const Color tintKfcIcon = Color(0xFFFF7A7A);
  static const Color tintSupermarket = Color(0xFFF59E0B);
  static const Color tintSupermarketIcon = Color(0xFFFBBF24);
  static const Color tintCoffee = Color(0xFFA15A2C);
  static const Color tintCoffeeIcon = Color(0xFFD9A066);
  static const Color tintDairy = Color(0xFF60A5FA);
  static const Color tintDairyIcon = Color(0xFF93C5FD);
  static const Color tintGeneric = Color(0xFFFFFFFF);
  static const Color tintGenericIcon = Color(0xFF9AA6C0);
}
