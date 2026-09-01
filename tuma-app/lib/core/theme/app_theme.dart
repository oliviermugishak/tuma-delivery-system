import 'package:flutter/material.dart';
import 'package:google_fonts/google_fonts.dart';

import 'package:tuma_app/core/theme/app_colors.dart';

/// Tuma v2 theme — the redesigns' exact component values.
///
/// Inter carries EVERYTHING (the redesign spec dropped the Poppins mix).
/// Depth is surface layering + 8%-white hairlines, never shadows (P6).
/// Type scale from the spec: d1 23/800 · d2 22/800 · sec 17/700 ·
/// hd 15/600 · bd 14/500 · sub 12.5 · cap 11/600 +0.9 tracking upper.
/// This file holds global defaults only; screens compose from here.
abstract final class AppTheme {
  // Type scale (spec-exact) ------------------------------------------------
  static TextStyle d1(TextTheme t) => t.titleLarge!.copyWith(
        fontSize: 23,
        fontWeight: FontWeight.w800,
        letterSpacing: -0.2,
        color: AppColors.onSurface,
      );
  static TextStyle d2(TextTheme t) => t.titleLarge!.copyWith(
        fontSize: 22,
        fontWeight: FontWeight.w800,
        letterSpacing: -0.2,
        color: AppColors.onSurface,
      );
  static TextStyle sec(TextTheme t) => t.titleMedium!.copyWith(
        fontSize: 17,
        fontWeight: FontWeight.w700,
        color: AppColors.onSurface,
      );
  static TextStyle hd(TextTheme t) => t.titleSmall!.copyWith(
        fontSize: 15,
        fontWeight: FontWeight.w600,
        color: AppColors.onSurface,
      );
  static TextStyle bd(TextTheme t) => t.bodyMedium!.copyWith(
        fontSize: 14,
        fontWeight: FontWeight.w500,
        color: AppColors.onSurface,
      );
  static TextStyle sub(TextTheme t) => t.bodySmall!.copyWith(
        fontSize: 12.5,
        color: AppColors.onSurfaceMuted,
      );
  static TextStyle cap(TextTheme t) => t.labelSmall!.copyWith(
        fontSize: 11,
        fontWeight: FontWeight.w600,
        letterSpacing: 0.9,
        color: AppColors.onSurfaceMuted,
      );

  static ThemeData dark() {
    final base = ThemeData(
      useMaterial3: true,
      brightness: Brightness.dark,
      colorScheme: const ColorScheme.dark(
        primary: AppColors.primary,
        onPrimary: AppColors.onPrimary,
        secondary: AppColors.surfaceHigh,
        onSecondary: AppColors.onSurface,
        surface: AppColors.surface,
        onSurface: AppColors.onSurface,
        error: AppColors.error,
      ),
      scaffoldBackgroundColor: AppColors.surface,
    );

    final textTheme = GoogleFonts.interTextTheme(base.textTheme);

    return base.copyWith(
      textTheme: textTheme,
      appBarTheme: AppBarTheme(
        backgroundColor: AppColors.surface,
        foregroundColor: AppColors.onSurface,
        elevation: 0,
        scrolledUnderElevation: 0,
        centerTitle: false,
        titleTextStyle: textTheme.titleMedium?.copyWith(
          fontSize: 16,
          color: AppColors.onSurface,
          fontWeight: FontWeight.w600,
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
      // The spec's .btn: 50px, r14, 600 15px — primary (accent) is the
      // one per screen (P5); outline + outline-danger + outline-okay are
      // the secondary ranks.
      filledButtonTheme: FilledButtonThemeData(
        style: FilledButton.styleFrom(
          minimumSize: const Size.fromHeight(50),
          backgroundColor: AppColors.primary,
          foregroundColor: AppColors.onPrimary,
          textStyle: textTheme.titleSmall?.copyWith(
            fontSize: 15,
            fontWeight: FontWeight.w600,
            color: AppColors.onPrimary,
          ),
          shape: RoundedRectangleBorder(borderRadius: BorderRadius.circular(14)),
        ),
      ),
      outlinedButtonTheme: OutlinedButtonThemeData(
        style: OutlinedButton.styleFrom(
          minimumSize: const Size.fromHeight(50),
          foregroundColor: AppColors.onSurface,
          side: const BorderSide(color: AppColors.line),
          textStyle: textTheme.titleSmall?.copyWith(
            fontSize: 15,
            fontWeight: FontWeight.w600,
            color: AppColors.onSurface,
          ),
          shape: RoundedRectangleBorder(borderRadius: BorderRadius.circular(14)),
        ),
      ),
      textButtonTheme: TextButtonThemeData(
        style: TextButton.styleFrom(
          foregroundColor: AppColors.primary,
          textStyle: textTheme.titleSmall?.copyWith(
            fontSize: 14,
            fontWeight: FontWeight.w600,
            color: AppColors.primary,
          ),
        ),
      ),
      // Inputs: surface-high wells, r12, no border until focus.
      inputDecorationTheme: InputDecorationTheme(
        filled: true,
        fillColor: AppColors.surfaceHigh,
        hintStyle: textTheme.bodyMedium?.copyWith(color: AppColors.onSurfaceMuted),
        contentPadding: const EdgeInsets.symmetric(horizontal: 14, vertical: 13),
        border: OutlineInputBorder(
          borderRadius: BorderRadius.circular(12),
          borderSide: BorderSide.none,
        ),
        enabledBorder: OutlineInputBorder(
          borderRadius: BorderRadius.circular(12),
          borderSide: BorderSide.none,
        ),
        focusedBorder: OutlineInputBorder(
          borderRadius: BorderRadius.circular(12),
          borderSide: const BorderSide(color: AppColors.primary, width: 1.5),
        ),
      ),
      dialogTheme: DialogThemeData(
        backgroundColor: const Color(0xFF0D1530),
        shape: RoundedRectangleBorder(
          borderRadius: BorderRadius.circular(20),
          side: const BorderSide(color: AppColors.surfaceBorder),
        ),
        titleTextStyle: textTheme.titleMedium?.copyWith(
          fontSize: 17,
          fontWeight: FontWeight.w700,
          color: AppColors.onSurface,
        ),
        contentTextStyle: textTheme.bodyMedium?.copyWith(
          fontWeight: FontWeight.w400,
          color: AppColors.onSurfaceMuted,
          height: 1.55,
        ),
      ),
      snackBarTheme: SnackBarThemeData(
        behavior: SnackBarBehavior.floating,
        backgroundColor: AppColors.surfaceHigh,
        contentTextStyle: textTheme.bodyMedium?.copyWith(
          color: AppColors.onSurface,
        ),
        shape: RoundedRectangleBorder(borderRadius: BorderRadius.circular(12)),
      ),
      dividerTheme: const DividerThemeData(
        color: AppColors.surfaceBorder,
        thickness: 1,
        space: 1,
      ),
      progressIndicatorTheme: const ProgressIndicatorThemeData(
        color: AppColors.primary,
        linearTrackColor: AppColors.surfaceHigh,
      ),
    );
  }
}
