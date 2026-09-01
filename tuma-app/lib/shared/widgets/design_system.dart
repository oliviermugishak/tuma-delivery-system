import 'package:flutter/material.dart';

import 'package:tuma_app/core/theme/app_colors.dart';
import 'package:tuma_app/core/theme/app_theme.dart';

/// The redesign's shared primitives — the signature moves (DESIGN
/// PRINCIPLES.md Part 2). Screens compose from these; per-screen
/// inventions need justification (P15).

/// The status dot row (P14's vocabulary): colored dot + status words.
/// Colors come from [statusColor], one meaning per color everywhere.
class StatusRow extends StatelessWidget {
  const StatusRow({
    super.key,
    required this.color,
    required this.label,
    this.pulsing = false,
  });

  final Color color;
  final String label;
  final bool pulsing;

  @override
  Widget build(BuildContext context) {
    return Row(
      mainAxisSize: MainAxisSize.min,
      children: [
        PulsingDot(color: color, pulsing: pulsing),
        const SizedBox(width: 6),
        Text(
          label,
          style: AppTheme.sub(context.textTheme).copyWith(
            color: color,
            fontWeight: FontWeight.w600,
            fontSize: 12.5,
          ),
        ),
      ],
    );
  }
}

/// The pulsing dot — teal-and-live (signature move 2): a solid dot with
/// an expanding ring on a 2s loop when [pulsing]. Never more than one
/// pulsing element per screen.
class PulsingDot extends StatefulWidget {
  const PulsingDot({
    super.key,
    required this.color,
    this.pulsing = false,
    this.size = 8,
  });

  final Color color;
  final bool pulsing;
  final double size;

  @override
  State<PulsingDot> createState() => _PulsingDotState();
}

class _PulsingDotState extends State<PulsingDot>
    with SingleTickerProviderStateMixin {
  AnimationController? _controller;

  @override
  void initState() {
    super.initState();
    if (widget.pulsing) _start();
  }

  @override
  void didUpdateWidget(PulsingDot oldWidget) {
    super.didUpdateWidget(oldWidget);
    if (widget.pulsing && _controller == null) _start();
    if (!widget.pulsing && _controller != null) _stop();
  }

  void _start() {
    _controller = AnimationController(
      vsync: this,
      duration: const Duration(seconds: 2),
    )..repeat();
  }

  void _stop() {
    _controller?.dispose();
    _controller = null;
  }

  @override
  void dispose() {
    _controller?.dispose();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    final dot = Container(
      width: widget.size,
      height: widget.size,
      decoration: BoxDecoration(
        color: widget.color,
        shape: BoxShape.circle,
      ),
    );
    final controller = _controller;
    if (!widget.pulsing || controller == null) return dot;
    return AnimatedBuilder(
      animation: controller,
      builder: (context, _) {
        final t = controller.value;
        // The ring expands to ~2.2× the dot while fading out — the spec's
        // `box-shadow 0 0 0 9px` loop.
        final ringSize = widget.size + 18 * t;
        return SizedBox(
          width: ringSize,
          height: ringSize,
          child: Stack(
            alignment: Alignment.center,
            children: [
              Container(
                width: ringSize,
                height: ringSize,
                decoration: BoxDecoration(
                  shape: BoxShape.circle,
                  border: Border.all(
                    color: widget.color.withValues(alpha: 0.45 * (1 - t)),
                    width: 2,
                  ),
                ),
              ),
              dot,
            ],
          ),
        );
      },
    );
  }
}

/// The uppercase micro-label (signature move 4): "DELIVER TO",
/// "ORDER SUMMARY", "ACCOUNT" — quiet section structure without
/// dividers.
class MicroLabel extends StatelessWidget {
  const MicroLabel(this.text, {super.key});

  final String text;

  @override
  Widget build(BuildContext context) {
    return Text(
      text.toUpperCase(),
      style: AppTheme.cap(context.textTheme),
    );
  }
}

/// The stat label (signature move 5): the accent, 800-weight, wider
/// tracking variant — "STOP 1 OF 2 · PICK UP". Marks rider stages.
class StatLabel extends StatelessWidget {
  const StatLabel(this.text, {super.key});

  final String text;

  @override
  Widget build(BuildContext context) {
    return Text(
      text.toUpperCase(),
      style: AppTheme.cap(context.textTheme).copyWith(
        color: AppColors.primary,
        fontWeight: FontWeight.w800,
        letterSpacing: 1.2,
        fontSize: 11,
      ),
    );
  }
}

/// The accent strip (signature move 1): a full-width yellow bar for the
/// single most important fact — "Collect 8,000 RWF cash". One per screen,
/// only when money or the critical fact is present (P8's carrier).
class AccentStrip extends StatelessWidget {
  const AccentStrip({
    super.key,
    required this.icon,
    required this.text,
  });

  final IconData icon;
  final String text;

  @override
  Widget build(BuildContext context) {
    return Container(
      padding: const EdgeInsets.symmetric(horizontal: 14, vertical: 12),
      decoration: BoxDecoration(
        color: AppColors.primary,
        borderRadius: BorderRadius.circular(12),
      ),
      child: Row(
        children: [
          Icon(icon, size: 22, color: AppColors.onPrimary),
          const SizedBox(width: 10),
          Expanded(
            child: Text(
              text,
              style: AppTheme.hd(context.textTheme).copyWith(
                color: AppColors.onPrimary,
                fontWeight: FontWeight.w700,
              ),
            ),
          ),
        ],
      ),
    );
  }
}

/// The lifecycle stepper (P11): Placed → Preparing → Picked up →
/// On the way → Delivered. Done dots are accent, the current dot is the
/// pulsing teal, future dots are quiet wells. Labels under dots.
class OrderStepper extends StatelessWidget {
  const OrderStepper({
    super.key,
    required this.labels,
    required this.currentIndex,
    this.completed = false,
  });

  static const _defaults = ['Placed', 'Preparing', 'Picked up', 'On the way', 'Delivered'];

  /// The five lifecycle labels in order.
  const OrderStepper.lifecycle({super.key, required int index, this.completed = false})
      : labels = _defaults,
        currentIndex = index;

  final List<String> labels;
  final int currentIndex;
  final bool completed;

  @override
  Widget build(BuildContext context) {
    return Row(
      children: [
        for (var i = 0; i < labels.length; i++) ...[
          if (i > 0)
            Expanded(
              child: Container(
                height: 2,
                margin: const EdgeInsets.only(bottom: 18),
                color: i <= currentIndex
                    ? (completed ? AppColors.success : AppColors.primary)
                    : AppColors.line,
              ),
            ),
          _Step(
            label: labels[i],
            state: completed
                ? _StepState.doneOk
                : i < currentIndex
                    ? _StepState.done
                    : i == currentIndex
                        ? _StepState.current
                        : _StepState.future,
          ),
        ],
      ],
    );
  }
}

enum _StepState { future, done, current, doneOk }

class _Step extends StatelessWidget {
  const _Step({required this.label, required this.state});

  final String label;
  final _StepState state;

  @override
  Widget build(BuildContext context) {
    final isOk = state == _StepState.doneOk;
    final color = switch (state) {
      _StepState.done => AppColors.primary,
      _StepState.current => AppColors.success,
      _StepState.doneOk => AppColors.success,
      _StepState.future => AppColors.line,
    };
    return Column(
      children: [
        state == _StepState.current
            ? const PulsingDot(color: AppColors.success, pulsing: true, size: 14)
            : Container(
                width: 14,
                height: 14,
                decoration: BoxDecoration(
                  shape: BoxShape.circle,
                  color: state == _StepState.future
                      ? AppColors.surfaceHigh
                      : color,
                  border: Border.all(color: color, width: 2),
                ),
              ),
        const SizedBox(height: 7),
        SizedBox(
          width: 54,
          child: Text(
            label,
            textAlign: TextAlign.center,
            style: AppTheme.sub(context.textTheme).copyWith(
              fontSize: 9.5,
              color: state == _StepState.current || isOk
                  ? AppColors.success
                  : AppColors.onSurfaceMuted,
              fontWeight: state == _StepState.current || isOk
                  ? FontWeight.w700
                  : FontWeight.w500,
            ),
          ),
        ),
      ],
    );
  }
}

/// The dashed add row (signature move 7): the universal "create
/// something new" affordance — "+ Add new address".
class DashedAddRow extends StatelessWidget {
  const DashedAddRow({
    super.key,
    required this.label,
    required this.onTap,
  });

  final String label;
  final VoidCallback onTap;

  @override
  Widget build(BuildContext context) {
    return CustomPaint(
      foregroundPainter: _DashedBorderPainter(
        radius: 14,
        color: AppColors.line,
      ),
      child: Material(
        color: Colors.transparent,
        borderRadius: BorderRadius.circular(14),
        clipBehavior: Clip.antiAlias,
        child: InkWell(
          onTap: onTap,
          child: SizedBox(
            height: 48,
            child: Row(
              mainAxisAlignment: MainAxisAlignment.center,
              children: [
                const Icon(Icons.add_rounded, size: 18, color: AppColors.primary),
                const SizedBox(width: 8),
                Text(
                  label,
                  style: AppTheme.bd(context.textTheme).copyWith(
                    color: AppColors.primary,
                    fontWeight: FontWeight.w600,
                  ),
                ),
              ],
            ),
          ),
        ),
      ),
    );
  }
}

class _DashedBorderPainter extends CustomPainter {
  _DashedBorderPainter({required this.radius, required this.color});

  final double radius;
  final Color color;

  @override
  void paint(Canvas canvas, Size size) {
    final rrect = RRect.fromRectAndRadius(
      Offset.zero & size,
      Radius.circular(radius),
    );
    final path = Path()..addRRect(rrect);
    final dashed = Path();
    const dashLen = 6.0;
    for (final metric in path.computeMetrics()) {
      var distance = 0.0;
      while (distance < metric.length) {
        final next = (distance + dashLen).clamp(0.0, metric.length);
        dashed.addPath(metric.extractPath(distance, next), Offset.zero);
        distance = next + dashLen;
      }
    }
    canvas.drawPath(
      dashed,
      Paint()
        ..style = PaintingStyle.stroke
        ..strokeWidth = 1.5
        ..color = color,
    );
  }

  @override
  bool shouldRepaint(_DashedBorderPainter oldDelegate) =>
      oldDelegate.color != color || oldDelegate.radius != radius;
}

/// The elongated dot carousel indicators (signature move 6): 6dp dots,
/// the active grows to a 16dp accent pill.
class DotIndicators extends StatelessWidget {
  const DotIndicators({
    super.key,
    required this.count,
    required this.activeIndex,
  });

  final int count;
  final int activeIndex;

  @override
  Widget build(BuildContext context) {
    return Row(
      mainAxisAlignment: MainAxisAlignment.center,
      children: [
        for (var i = 0; i < count; i++)
          AnimatedContainer(
            duration: const Duration(milliseconds: 200),
            width: i == activeIndex ? 16 : 6,
            height: 6,
            margin: const EdgeInsets.symmetric(horizontal: 2.5),
            decoration: BoxDecoration(
              color: i == activeIndex ? AppColors.primary : AppColors.surfaceHigh,
              borderRadius: BorderRadius.circular(999),
            ),
          ),
      ],
    );
  }
}

/// The category tint (P15's carrier): a merchant category resolves to a
/// 135° two-stop gradient + an icon. The merchant's identity at a glance
/// wherever photography is absent — and beneath photography as the
/// loading/fallback surface, so empty media looks intentional.
class CategoryTint {
  const CategoryTint({
    required this.begin,
    required this.end,
    required this.icon,
    required this.iconColor,
  });

  final Color begin;
  final Color end;
  final IconData icon;
  final Color iconColor;

  /// The tint for a category/store name. Matching is by keywords; the
  /// generic white-tint is the honest fallback.
  static CategoryTint forCategory(String category, {String? hint}) {
    final text = '$category ${hint ?? ''}'.toLowerCase();
    if (text.contains('coffee') || text.contains('café') || text.contains('cafe')) {
      return coffee;
    }
    if (text.contains('supermarket') ||
        text.contains('market') ||
        text.contains('grocery') ||
        text.contains('bakery')) {
      return supermarket;
    }
    if (text.contains('milk') ||
        text.contains('dairy') ||
        text.contains('drink') ||
        text.contains('juice') ||
        text.contains('soda') ||
        text.contains('water')) {
      return dairy;
    }
    if (text.contains('kfc') ||
        text.contains('chicken') ||
        text.contains('grill') ||
        text.contains('fast food') ||
        text.contains('burger') ||
        text.contains('pizza') ||
        text.contains('restaurant') ||
        text.contains('food')) {
      return fastFood;
    }
    return generic;
  }

  static final CategoryTint fastFood = CategoryTint(
    begin: AppColors.tintKfc.withValues(alpha: 0.32),
    end: AppColors.tintKfc.withValues(alpha: 0.06),
    icon: Icons.restaurant_rounded,
    iconColor: AppColors.tintKfcIcon,
  );
  static final CategoryTint supermarket = CategoryTint(
    begin: AppColors.tintSupermarket.withValues(alpha: 0.30),
    end: AppColors.tintSupermarket.withValues(alpha: 0.05),
    icon: Icons.local_grocery_store_rounded,
    iconColor: AppColors.tintSupermarketIcon,
  );
  static final CategoryTint coffee = CategoryTint(
    begin: AppColors.tintCoffee.withValues(alpha: 0.38),
    end: AppColors.tintCoffee.withValues(alpha: 0.08),
    icon: Icons.coffee_rounded,
    iconColor: AppColors.tintCoffeeIcon,
  );
  static final CategoryTint dairy = CategoryTint(
    begin: AppColors.tintDairy.withValues(alpha: 0.28),
    end: AppColors.tintDairy.withValues(alpha: 0.05),
    icon: Icons.local_drink_rounded,
    iconColor: AppColors.tintDairyIcon,
  );
  static final CategoryTint generic = CategoryTint(
    begin: Colors.white.withValues(alpha: 0.07),
    end: Colors.white.withValues(alpha: 0.02),
    icon: Icons.shopping_basket_rounded,
    iconColor: AppColors.tintGenericIcon,
  );
}

/// The tinted tile (signature move 3): the 135° category gradient with a
/// white category icon — standing in for photography everywhere, and the
/// fallback under a failing/absent RemoteImage.
class TintedTile extends StatelessWidget {
  const TintedTile({
    super.key,
    required this.tint,
    this.size,
    this.borderRadius = 12,
    this.iconSize = 30,
    this.child,
  });

  final CategoryTint tint;

  /// Null = expand to the parent's constraints.
  final double? size;
  final double borderRadius;
  final double iconSize;

  /// Overrides the default category icon (e.g. a specific dish icon).
  final Widget? child;

  @override
  Widget build(BuildContext context) {
    final decoration = BoxDecoration(
      gradient: LinearGradient(
        begin: Alignment.topLeft,
        end: Alignment.bottomRight,
        colors: [tint.begin, tint.end],
      ),
      borderRadius: BorderRadius.circular(borderRadius),
    );
    final content = child ??
        Icon(tint.icon, size: iconSize, color: tint.iconColor);
    if (size != null) {
      return Container(
        width: size,
        height: size,
        decoration: decoration,
        alignment: Alignment.center,
        child: content,
      );
    }
    return Container(
      width: double.infinity,
      height: double.infinity,
      decoration: decoration,
      alignment: Alignment.center,
      child: content,
    );
  }
}

/// The avatar: accent circle + initials (the profile/rider identity).
class AccentAvatar extends StatelessWidget {
  const AccentAvatar({
    super.key,
    required this.text,
    this.size = 44,
    this.tinted = false,
  });

  final String text;
  final double size;

  /// The rider/customer card variant: tinted accent at 15% with accent
  /// text instead of the solid accent fill.
  final bool tinted;

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
    final initials = _initials ?? '?';
    return Container(
      width: size,
      height: size,
      decoration: BoxDecoration(
        shape: BoxShape.circle,
        color: tinted
            ? AppColors.primary.withValues(alpha: 0.15)
            : AppColors.primary,
      ),
      alignment: Alignment.center,
      child: Text(
        initials,
        style: TextStyle(
          fontSize: size * 0.33,
          fontWeight: FontWeight.w800,
          color: tinted ? AppColors.primary : AppColors.onPrimary,
        ),
      ),
    );
  }
}

extension on BuildContext {
  TextTheme get textTheme => Theme.of(this).textTheme;
}
