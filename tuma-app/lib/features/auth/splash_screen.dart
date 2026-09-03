import 'package:flutter/material.dart';

import 'package:tuma_app/core/theme/app_brand.dart';
import 'package:tuma_app/core/theme/app_colors.dart';

/// Splash shown while the session hydrates from secure storage. It never
/// navigates itself — the router redirect takes over the moment the session
/// is known (see app_router.dart).
class SplashScreen extends StatefulWidget {
  const SplashScreen({super.key});

  @override
  State<SplashScreen> createState() => _SplashScreenState();
}

class _SplashScreenState extends State<SplashScreen> with TickerProviderStateMixin {
  late final AnimationController _reveal = AnimationController(
    vsync: this,
    duration: const Duration(milliseconds: 1100),
  )..forward();

  late final AnimationController _bar = AnimationController(
    vsync: this,
    duration: const Duration(milliseconds: 1200),
  )..repeat(reverse: true);

  late final Animation<double> _markOpacity = CurvedAnimation(
    parent: _reveal,
    curve: const Interval(0.0, 0.5, curve: Curves.easeOut),
  );

  late final Animation<double> _markScale = Tween<double>(begin: 0.85, end: 1).animate(
    CurvedAnimation(parent: _reveal, curve: Curves.easeOutCubic),
  );

  late final Animation<double> _textOpacity = CurvedAnimation(
    parent: _reveal,
    curve: const Interval(0.35, 1.0, curve: Curves.easeOut),
  );

  @override
  void dispose() {
    _reveal.dispose();
    _bar.dispose();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    final textTheme = Theme.of(context).textTheme;
    return Scaffold(
      backgroundColor: AppColors.canvas,
      body: Stack(
        fit: StackFit.expand,
        children: [
          Center(
            child: Column(
              mainAxisSize: MainAxisSize.min,
              children: [
                ScaleTransition(
                  scale: _markScale,
                  child: FadeTransition(
                    opacity: _markOpacity,
                    child: const BrandMark(size: 128),
                  ),
                ),
                const SizedBox(height: 24),
                FadeTransition(
                  opacity: _textOpacity,
                  child: Column(
                    mainAxisSize: MainAxisSize.min,
                    children: [
                      Text(
                        'Tuma',
                        style: textTheme.displaySmall?.copyWith(
                          color: AppColors.onSurface,
                          fontWeight: FontWeight.w600,
                        ),
                      ),
                      const SizedBox(height: 8),
                      Text(
                        'Everything you crave, delivered.',
                        style: textTheme.bodyMedium?.copyWith(
                          color: AppColors.onSurfaceMuted,
                        ),
                      ),
                    ],
                  ),
                ),
              ],
            ),
          ),
          Positioned(
            left: 56,
            right: 56,
            bottom: 56,
            child: FadeTransition(
              opacity: _textOpacity,
              child: AnimatedBuilder(
                animation: _bar,
                builder: (context, _) {
                  return ClipRRect(
                    borderRadius: BorderRadius.circular(2),
                    child: SizedBox(
                      height: 3,
                      child: Stack(
                        children: [
                          Container(color: AppColors.canvas),
                          Align(
                            alignment: Alignment(-1 + 2 * _bar.value, 0),
                            child: FractionallySizedBox(
                              widthFactor: 0.35,
                              child: Container(
                                decoration: const BoxDecoration(
                                  color: AppColors.primary,
                                  borderRadius: BorderRadius.all(Radius.circular(2)),
                                ),
                              ),
                            ),
                          ),
                        ],
                      ),
                    ),
                  );
                },
              ),
            ),
          ),
        ],
      ),
    );
  }
}
