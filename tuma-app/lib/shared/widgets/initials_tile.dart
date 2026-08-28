import 'package:flutter/material.dart';

import 'package:tuma_app/core/theme/app_colors.dart';

/// Brand-gradient initials tile — the user avatar (users have no images;
/// stores and products use `RemoteImage` instead).
class InitialsTile extends StatelessWidget {
  const InitialsTile({
    super.key,
    required this.text,
    this.size = 56,
    this.borderRadius = 16,
  });

  final String text;
  final double size;
  final double borderRadius;

  /// First letters of the first two words; `null` when nothing starts with
  /// a letter (e.g. a phone number) — the tile falls back to a person icon.
  String? get _initials {
    final letters = <String>[];
    for (final word in text.trim().split(RegExp(r'\s+'))) {
      if (word.isEmpty) continue;
      final first = String.fromCharCode(word.runes.first);
      if (RegExp(r'[A-Za-z]').hasMatch(first)) {
        letters.add(first.toUpperCase());
      }
      if (letters.length == 2) break;
    }
    return letters.isEmpty ? null : letters.join();
  }

  @override
  Widget build(BuildContext context) {
    final initials = _initials;
    return Container(
      width: size,
      height: size,
      decoration: BoxDecoration(
        gradient: const LinearGradient(
          begin: Alignment.topLeft,
          end: Alignment.bottomRight,
          colors: [AppColors.primary, AppColors.primaryDeep],
        ),
        borderRadius: BorderRadius.circular(borderRadius),
      ),
      alignment: Alignment.center,
      child: initials == null
          ? Icon(Icons.person, color: AppColors.onPrimary, size: size * 0.45)
          : Text(
              initials,
              style: Theme.of(context).textTheme.titleLarge?.copyWith(
                    color: AppColors.onPrimary,
                    fontWeight: FontWeight.w700,
                    fontSize: size * 0.32,
                  ),
            ),
    );
  }
}
