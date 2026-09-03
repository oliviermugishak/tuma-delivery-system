import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:google_fonts/google_fonts.dart';

import 'package:tuma_app/core/theme/app_colors.dart';

/// Tuma v3 theme — Light / Green.
///
/// Poppins carries EVERYTHING, weights 400/500/600 only (ceiling 600).
/// Depth = white surfaces on the warm canvas; shadows only on floating
/// elements (cart bar, search, FABs); hairlines only for chips/OTP/tab
/// tracks. Type scale: d1 23/600 · d2 22/600 · sec 17/600 · hd 15/600 ·
/// bd 14/500 · sub 12.5 · cap 11/500 (sentence case, no tracking).
/// This file holds global defaults only; screens compose from here.
abstract final class AppTheme {
  // Type scale -------------------------------------------------------------
  static TextStyle d1(TextTheme t) => t.titleLarge!.copyWith(
        fontSize: 23,
        fontWeight: FontWeight.w600,
        letterSpacing: -0.2,
        color: AppColors.onSurface,
      );
  static TextStyle d2(TextTheme t) => t.titleLarge!.copyWith(
        fontSize: 22,
        fontWeight: FontWeight.w600,
        letterSpacing: -0.2,
        color: AppColors.onSurface,
      );
  static TextStyle sec(TextTheme t) => t.titleMedium!.copyWith(
        fontSize: 17,
        fontWeight: FontWeight.w600,
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
        fontWeight: FontWeight.w500,
        color: AppColors.onSurfaceMuted,
      );

  static ThemeData light() {
    final base = ThemeData(
      useMaterial3: true,
      brightness: Brightness.light,
      colorScheme: const ColorScheme.light(
        primary: AppColors.primary,
        onPrimary: AppColors.onPrimary,
        secondary: AppColors.fill,
        onSecondary: AppColors.onSurface,
        surface: AppColors.canvas,
        onSurface: AppColors.onSurface,
        error: AppColors.error,
      ),
      scaffoldBackgroundColor: AppColors.canvas,
      // Green ripple at 8% everywhere.
      splashColor: const Color(0x14156B4C),
      highlightColor: const Color(0x0A156B4C),
    );

    final textTheme = GoogleFonts.poppinsTextTheme(base.textTheme);

    return base.copyWith(
      textTheme: textTheme,
      appBarTheme: AppBarTheme(
        backgroundColor: AppColors.canvas,
        foregroundColor: AppColors.onSurface,
        elevation: 0,
        scrolledUnderElevation: 0,
        centerTitle: false,
        systemOverlayStyle: SystemUiOverlayStyle.dark,
        titleTextStyle: textTheme.titleMedium?.copyWith(
          fontSize: 16,
          color: AppColors.onSurface,
          fontWeight: FontWeight.w600,
        ),
      ),
      // White cards on canvas: no border, no shadow.
      cardTheme: CardThemeData(
        elevation: 0,
        color: AppColors.surface,
        shape: const RoundedRectangleBorder(
          borderRadius: BorderRadius.all(Radius.circular(16)),
        ),
      ),
      // The button: 50px, r12, flat green, 600 15px. Disabled = fill well
      // with muted text — never an alpha fade.
      filledButtonTheme: FilledButtonThemeData(
        style: FilledButton.styleFrom(
          minimumSize: const Size.fromHeight(50),
          backgroundColor: AppColors.primary,
          foregroundColor: AppColors.onPrimary,
          disabledBackgroundColor: AppColors.fill,
          disabledForegroundColor: AppColors.onSurfaceMuted,
          textStyle: textTheme.titleSmall?.copyWith(
            fontSize: 15,
            fontWeight: FontWeight.w600,
            color: AppColors.onPrimary,
          ),
          shape: RoundedRectangleBorder(borderRadius: BorderRadius.circular(12)),
        ),
      ),
      outlinedButtonTheme: OutlinedButtonThemeData(
        style: OutlinedButton.styleFrom(
          minimumSize: const Size.fromHeight(50),
          foregroundColor: AppColors.onSurface,
          side: const BorderSide(color: AppColors.hairline),
          textStyle: textTheme.titleSmall?.copyWith(
            fontSize: 15,
            fontWeight: FontWeight.w600,
            color: AppColors.onSurface,
          ),
          shape: RoundedRectangleBorder(borderRadius: BorderRadius.circular(12)),
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
      // Inputs: fill wells, r12, no border until green focus.
      inputDecorationTheme: InputDecorationTheme(
        filled: true,
        fillColor: AppColors.fill,
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
        backgroundColor: AppColors.surface,
        shape: const RoundedRectangleBorder(
          borderRadius: BorderRadius.all(Radius.circular(20)),
        ),
        titleTextStyle: textTheme.titleMedium?.copyWith(
          fontSize: 17,
          fontWeight: FontWeight.w600,
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
        backgroundColor: AppColors.surface,
        elevation: 3,
        actionTextColor: AppColors.primary,
        contentTextStyle: textTheme.bodyMedium?.copyWith(
          color: AppColors.onSurface,
        ),
        shape: RoundedRectangleBorder(borderRadius: BorderRadius.circular(12)),
      ),
      dividerTheme: const DividerThemeData(
        color: AppColors.line,
        thickness: 1,
        space: 1,
      ),
      progressIndicatorTheme: const ProgressIndicatorThemeData(
        color: AppColors.primary,
        linearTrackColor: AppColors.fill,
      ),
    );
  }
}
