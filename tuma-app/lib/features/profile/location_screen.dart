import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:go_router/go_router.dart';

import 'package:tuma_app/core/theme/app_colors.dart';
import 'package:tuma_app/features/location/customer_location.dart';
import 'package:tuma_app/features/location/delivery_pin_map.dart';

/// The customer's delivery location, set on a real map. The same pin
/// picker checkout uses, as its own screen — so anyone can set a
/// location without GPS. Saving persists it: Home's distances sharpen,
/// checkout seeds its map with it.
class DeliveryLocationScreen extends ConsumerStatefulWidget {
  const DeliveryLocationScreen({super.key});

  @override
  ConsumerState<DeliveryLocationScreen> createState() =>
      _DeliveryLocationScreenState();
}

class _DeliveryLocationScreenState
    extends ConsumerState<DeliveryLocationScreen> {
  CustomerLocation? _pin;

  @override
  void initState() {
    super.initState();
    _initPin();
  }

  Future<void> _initPin() async {
    final persisted = await ref.read(customerLocationProvider.future);
    if (!mounted || _pin != null) return;
    setState(() => _pin = persisted);
  }

  Future<void> _save() async {
    final pin = _pin;
    if (pin == null) return;
    await ref.read(customerLocationProvider.notifier).setPin(pin);
    if (!mounted) return;
    ScaffoldMessenger.of(context).showSnackBar(
      SnackBar(
        content: const Text(
          'Delivery location saved.',
          style: TextStyle(color: AppColors.onSurface),
        ),
        behavior: SnackBarBehavior.floating,
        duration: const Duration(seconds: 1),
        backgroundColor: AppColors.surfaceAlt,
        shape: RoundedRectangleBorder(
          borderRadius: BorderRadius.circular(12),
        ),
      ),
    );
    context.pop();
  }

  @override
  Widget build(BuildContext context) {
    final textTheme = Theme.of(context).textTheme;
    final pin = _pin;

    return Scaffold(
      backgroundColor: AppColors.surface,
      body: SafeArea(
        child: Column(
          crossAxisAlignment: CrossAxisAlignment.start,
          children: [
            Padding(
              padding: const EdgeInsets.fromLTRB(12, 8, 20, 8),
              child: Row(
                children: [
                  IconButton(
                    icon: const Icon(Icons.arrow_back_ios_new_rounded,
                        size: 18, color: AppColors.onSurface),
                    onPressed: () => context.pop(),
                  ),
                  const SizedBox(width: 4),
                  Text(
                    'Delivery location',
                    style: textTheme.titleLarge?.copyWith(
                      fontWeight: FontWeight.w700,
                    ),
                  ),
                ],
              ),
            ),
            Padding(
              padding: const EdgeInsets.fromLTRB(20, 0, 20, 12),
              child: Text(
                'Drop the pin where the rider should find you — your door, '
                'gate, or a landmark nearby.',
                style: textTheme.bodySmall?.copyWith(
                  color: AppColors.onSurfaceMuted,
                ),
              ),
            ),
            Expanded(
              child: Padding(
                padding: const EdgeInsets.symmetric(horizontal: 20),
                child: DeliveryPinMap(
                  pin: pin,
                  onPin: (newPin) => setState(() => _pin = newPin),
                  // The same GPS seam as everywhere: injectable, so
                  // tests and desktop take their documented paths.
                  locate: ref.read(acquireLocationProvider),
                ),
              ),
            ),
            SafeArea(
              top: false,
              child: Padding(
                padding: const EdgeInsets.fromLTRB(20, 12, 20, 12),
                child: FilledButton(
                  onPressed: pin == null ? null : _save,
                  style: FilledButton.styleFrom(
                    minimumSize: const Size.fromHeight(52),
                    disabledBackgroundColor:
                        AppColors.onSurface.withValues(alpha: 0.15),
                  ),
                  child: Text(
                    pin == null ? 'Tap the map to drop the pin' : 'Save location',
                    style: const TextStyle(
                      fontSize: 16,
                      fontWeight: FontWeight.w700,
                    ),
                  ),
                ),
              ),
            ),
          ],
        ),
      ),
    );
  }
}
