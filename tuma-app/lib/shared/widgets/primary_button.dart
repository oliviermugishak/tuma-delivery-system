import 'package:flutter/material.dart';

import 'package:tuma_app/core/theme/app_colors.dart';

/// The one dominant action per screen: full-width gold→orange CTA with an
/// inline loading state.
class PrimaryButton extends StatelessWidget {
  const PrimaryButton({
    super.key,
    required this.label,
    this.onPressed,
    this.loading = false,
  });

  final String label;
  final VoidCallback? onPressed;
  final bool loading;

  @override
  Widget build(BuildContext context) {
    final enabled = onPressed != null && !loading;
    return Opacity(
      opacity: enabled ? 1 : 0.55,
      child: SizedBox(
        width: double.infinity,
        height: 52,
        child: DecoratedBox(
          decoration: BoxDecoration(
            gradient: const LinearGradient(
              colors: [AppColors.primary, AppColors.primaryDeep],
            ),
            borderRadius: BorderRadius.circular(14),
          ),
          child: Material(
            color: Colors.transparent,
            child: InkWell(
              borderRadius: BorderRadius.circular(14),
              onTap: enabled ? onPressed : null,
              child: Center(
                child: loading
                    ? const SizedBox(
                        width: 22,
                        height: 22,
                        child: CircularProgressIndicator(
                          strokeWidth: 2.5,
                          valueColor: AlwaysStoppedAnimation(AppColors.onPrimary),
                        ),
                      )
                    : Text(
                        label,
                        style: Theme.of(context).textTheme.titleMedium?.copyWith(
                              color: AppColors.onPrimary,
                              fontWeight: FontWeight.w700,
                            ),
                      ),
              ),
            ),
          ),
        ),
      ),
    );
  }
}
