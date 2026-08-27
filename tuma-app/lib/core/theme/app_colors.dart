import 'dart:ui';

/// Tuma design tokens — the evolve-emerald identity.
///
/// Source: tuma-docs/Tuma_Flutter_Mobile_App_Engineering_Spec.md (ch 4).
/// This file is the ONLY place raw hex values may appear in the app.
abstract final class AppColors {
  // Brand -----------------------------------------------------------------
  /// Emerald — trust, navigation, filled CTAs, selected chips.
  static const Color primary = Color(0xFF0B6E4F);

  /// Deep emerald — cart bar and dark emphasis where white must pop.
  static const Color primaryDark = Color(0xFF08412F);

  /// Orange — energy: promos, popular badges, sheet CTAs. Use sparingly.
  static const Color accent = Color(0xFFFF7A1A);

  // Surfaces --------------------------------------------------------------
  /// Warm greige scaffold background.
  static const Color surface = Color(0xFFF7F7F5);

  /// Cards, sheets, inputs.
  static const Color card = Color(0xFFFFFFFF);

  /// Hairline borders for cards, dividers, inputs.
  static const Color cardBorder = Color(0xFFECECE8);

  // Text ------------------------------------------------------------------
  static const Color text = Color(0xFF1B1B1F);
  static const Color textMuted = Color(0xFF6B6B70);
  static const Color onPrimary = Color(0xFFFFFFFF);

  // Feedback --------------------------------------------------------------
  static const Color error = Color(0xFFD64545);
}
