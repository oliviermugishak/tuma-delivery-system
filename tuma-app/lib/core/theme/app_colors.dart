import 'dart:ui';

/// Tuma v3 design tokens — Light / Green (the restyle brief's `:root`).
/// Tokens are the only place raw hex values may appear in the app (see
/// AGENTS.md). The color vocabulary is deliberately closed: green =
/// action/active/money/live, orange = attention signal only (never
/// buttons/links/body), red = destruction/failure only, and calm states
/// (preparing/confirmed) are neutral. Depth = white surfaces on the warm
/// canvas; hairlines survive only on chips, OTP boxes, tab tracks.
abstract final class AppColors {
  // Surfaces --------------------------------------------------------------
  /// The app background — everything sits on this. Warm off-white.
  static const Color canvas = Color(0xFFF4F4F1);

  /// Card surface — white cards, bars, sheets on the canvas.
  /// No borders, no shadows (the sanctioned shadow whitelist excepts
  /// floating elements only).
  static const Color surface = Color(0xFFFFFFFF);

  /// Fill — wells, inputs, tracks, disabled buttons, icon containers.
  static const Color fill = Color(0xFFEFEFEC);

  /// The bottom nav's field.
  static const Color navSurface = Color(0xFFFFFFFF);

  /// 8% ink hairline: chips, OTP boxes, tab tracks, dividers ONLY —
  /// never a card border.
  static const Color hairline = Color(0x141C1D1A);

  /// The visible divider: inner card dividers, stepper connector lines.
  static const Color line = Color(0xFFE5E5E0);

  // Text ------------------------------------------------------------------
  /// Primary text (ink).
  static const Color onSurface = Color(0xFF1C1D1A);

  /// Secondary text — labels, metadata, captions.
  static const Color onSurfaceMuted = Color(0xFF7E8079);

  // Accent ----------------------------------------------------------------
  /// Green — buttons, links, PRICES, active nav/tabs, live states,
  /// progress, selected. The one color that means go.
  static const Color primary = Color(0xFF156B4C);

  /// Text/icons placed on green fills.
  static const Color onPrimary = Color(0xFFFFFFFF);

  /// Soft green — active nav pill, info banners, live card tint, count
  /// chips, accent wells. Replaces every withValues alpha well.
  static const Color greenSoft = Color(0xFFE9F3EE);

  // Signal ----------------------------------------------------------------
  /// Orange — SIGNAL ONLY: star, bolt, cart badge, POPULAR, delayed,
  /// rider marker. Never buttons, links, or body text.
  static const Color orange = Color(0xFFF08C22);

  /// Soft orange — the POPULAR pill background family.
  static const Color orangeSoft = Color(0xFFFCEEE0);

  /// Destruction/failure only — cancel, sign-out text, errors.
  static const Color error = Color(0xFFD64545);

  /// Calm statuses (preparing/confirmed): the dot color; their text is
  /// [onSurfaceMuted].
  static const Color dotNeutral = Color(0xFFC9CBC4);

  // Media fallback / skeletons --------------------------------------------
  /// The image-fallback tile — replaces the retired category tints.
  static const Color fallbackTile = Color(0xFFE7E2D8);

  /// The icon on the fallback tile.
  static const Color fallbackIcon = Color(0xFFA89F8F);

  static const Color skeletonBase = Color(0xFFECECE8);
  static const Color skeletonHi = Color(0xFFF7F7F5);

  // Over-photo treatments -------------------------------------------------
  /// Ghost buttons floating over photos — rgba(20,21,18,.55).
  static const Color ghostBg = Color(0x8C141512);

  /// The store banner's top scrim — rgba(20,21,18,.45).
  static const Color scrim = Color(0x73141512);

  /// The one decorative gradient left in the app — the banner's warm
  /// ramp, #F5A54E → #EC7F3E at 100°. No site in the current app paints
  /// it yet (the banner tag component doesn't exist); the ONLY painted
  /// LinearGradient is the banner scrim.
  static const List<Color> bannerGrad = [Color(0xFFF5A54E), Color(0xFFEC7F3E)];

  // Stepper / indicator neutrals -------------------------------------------
  /// Upcoming stepper dots: [fill] center with this border.
  static const Color stepFuture = Color(0xFFD9D9D3);

  /// Inactive carousel dots.
  static const Color dotInactive = Color(0xFFD8D8D3);
}
