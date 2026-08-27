import 'package:flutter/material.dart';
import 'package:google_fonts/google_fonts.dart';

import 'package:tuma_app/core/theme/app_colors.dart';

/// Tuma theme — dark-on-navy identity.
///
/// Poppins carries headlines/display, Inter carries body/labels.
/// Component-level styling grows slice by slice; this file holds global
/// defaults only.
abstract final class AppTheme {
  static ThemeData dark() {
    final base = ThemeData(
      useMaterial3: true,
      brightness: Brightness.dark,
      colorScheme: const ColorScheme.dark(
        primary: AppColors.primary,
        onPrimary: AppColors.onPrimary,
        secondary: AppColors.primaryDeep,
        onSecondary: AppColors.onPrimary,
        surface: AppColors.surface,
        onSurface: AppColors.onSurface,
        error: AppColors.error,
      ),
      scaffoldBackgroundColor: AppColors.surface,
    );

    final poppins = GoogleFonts.poppinsTextTheme(base.textTheme);
    final inter = GoogleFonts.interTextTheme(base.textTheme);
    final textTheme = poppins.copyWith(
      bodyLarge: inter.bodyLarge,
      bodyMedium: inter.bodyMedium,
      bodySmall: inter.bodySmall,
      labelLarge: inter.labelLarge,
      labelMedium: inter.labelMedium,
      labelSmall: inter.labelSmall,
    );

    return base.copyWith(
      textTheme: textTheme,
      appBarTheme: AppBarTheme(
        backgroundColor: AppColors.surface,
        foregroundColor: AppColors.onSurface,
        elevation: 0,
        scrolledUnderElevation: 0,
        centerTitle: false,
        titleTextStyle: textTheme.titleLarge?.copyWith(
          color: AppColors.onSurface,
          fontWeight: FontWeight.w800,
        ),
      ),
      cardTheme: CardThemeData(
        elevation: 0,
        color: AppColors.surfaceAlt,
        shape: RoundedRectangleBorder(
          borderRadius: BorderRadius.circular(16),
          side: const BorderSide(color: AppColors.surfaceBorder),
        ),
      ),
      filledButtonTheme: FilledButtonThemeData(
        style: FilledButton.styleFrom(
          minimumSize: const Size.fromHeight(52),
          backgroundColor: AppColors.primary,
          foregroundColor: AppColors.onPrimary,
          textStyle: textTheme.labelLarge?.copyWith(fontWeight: FontWeight.w700),
          shape: RoundedRectangleBorder(borderRadius: BorderRadius.circular(12)),
        ),
      ),
      inputDecorationTheme: InputDecorationTheme(
        filled: true,
        fillColor: AppColors.surfaceAlt,
        hintStyle: textTheme.bodyMedium?.copyWith(color: AppColors.onSurfaceMuted),
        border: OutlineInputBorder(
          borderRadius: BorderRadius.circular(12),
          borderSide: const BorderSide(color: AppColors.surfaceBorder),
        ),
        enabledBorder: OutlineInputBorder(
          borderRadius: BorderRadius.circular(12),
          borderSide: const BorderSide(color: AppColors.surfaceBorder),
        ),
        focusedBorder: OutlineInputBorder(
          borderRadius: BorderRadius.circular(12),
          borderSide: const BorderSide(color: AppColors.primary, width: 1.5),
        ),
      ),
    );
  }
}
