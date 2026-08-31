import 'dart:async';
import 'dart:math';

import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:go_router/go_router.dart';

import 'package:tuma_app/core/api/api_client.dart';
import 'package:tuma_app/core/api/models/order.dart';
import 'package:tuma_app/core/auth/auth_controller.dart';
import 'package:tuma_app/core/theme/app_colors.dart';
import 'package:tuma_app/core/utils/format_rwf.dart';
import 'package:tuma_app/features/cart/cart_notifier.dart';
import 'package:tuma_app/features/home/app_shell.dart' show shellTabProvider;
import 'package:tuma_app/features/location/customer_location.dart';
import 'package:tuma_app/features/location/delivery_pin_map.dart';

/// Checkout — one checkout no matter how many stores are in the cart. The
/// customer reviews the grouped summary, enters a delivery address, and
/// places the order; the server splits it into one store order per store,
/// computes every number, and takes one cash payment. Cash-on-delivery
/// only (MoMo lands later).
///
/// The bottom bar is hidden because this route sits outside the AppShell.
class CheckoutScreen extends ConsumerStatefulWidget {
  const CheckoutScreen({super.key});

  @override
  ConsumerState<CheckoutScreen> createState() => _CheckoutScreenState();
}

class _CheckoutScreenState extends ConsumerState<CheckoutScreen> {
  final _addressController = TextEditingController();
  bool _placing = false;
  String? _error;
  /// The delivery pin from the map (or GPS). Seeds itself from the
  /// persisted customer location; every choice re-persists, so Home's
  /// distances sharpen after the first checkout too.
  CustomerLocation? _pin;
  /// Generated once per checkout attempt and kept until the order lands,
  /// so a retry (timeout, back button, re-tap) can never place twice.
  String? _idempotencyKey;

  @override
  void initState() {
    super.initState();
    unawaited(_initPin());
  }

  Future<void> _initPin() async {
    final persisted = await ref.read(customerLocationProvider.future);
    if (!mounted || _pin != null) return;
    setState(() => _pin = persisted);
  }

  Future<void> _choosePin(CustomerLocation pin) async {
    setState(() => _pin = pin);
    await ref.read(customerLocationProvider.notifier).setPin(pin);
  }

  @override
  void dispose() {
    _addressController.dispose();
    super.dispose();
  }

  /// A reasonably unique key without a uuid dependency: time + random.
  String _newIdempotencyKey() {
    final random = Random.secure();
    final suffix =
        List.generate(8, (_) => random.nextInt(16).toRadixString(16)).join();
    return 'mob-${DateTime.now().microsecondsSinceEpoch.toRadixString(36)}-$suffix';
  }

  Future<void> _placeOrder() async {
    if (_addressController.text.trim().isEmpty) {
      setState(() => _error = 'Please enter a delivery address.');
      return;
    }
    setState(() {
      _placing = true;
      _error = null;
    });
    try {
      final cart = ref.read(cartProvider);
      final state = cart.maybeWhen(
        data: (v) => v,
        orElse: () => const CartState(),
      );
      if (state.isEmpty) {
        if (!mounted) return;
        setState(() {
          _placing = false;
          _error = 'Your cart is empty.';
        });
        return;
      }
      final lines = <CheckoutLine>[
        for (final bucket in state.buckets)
          ...bucket.items.map(
            (item) => CheckoutLine(
              storeProductId: item.storeProductId,
              quantity: item.quantity,
            ),
          ),
      ];
      final request = CheckoutRequest(
        addressText: _addressController.text.trim(),
        addressLat: _pin?.lat,
        addressLng: _pin?.lng,
        idempotencyKey: _idempotencyKey ??= _newIdempotencyKey(),
        items: lines,
      );
      await ref.read(orderApiProvider).checkout(request);
      if (!mounted) return;
      // The checkout is server-side truth now — clear the cart and the
      // one-shot key with it.
      _idempotencyKey = null;
      await ref.read(cartProvider.notifier).clear();
      if (!mounted) return;
      // Never pop here: the dead checkout under this screen is exactly the
      // strand. The order detail is reached from the Orders list (with the
      // app bar), so back goes to history — not to an emptied checkout.
      ref.read(shellTabProvider.notifier).select(2);
      context.go('/home');
    } on ApiBadRequest catch (e) {
      if (!mounted) return;
      setState(() {
        _placing = false;
        _error = e.message;
      });
    } on ApiConflict catch (e) {
      // The world moved between cart and checkout: a store closed, an
      // item vanished, stock ran out. The message names the offenders.
      if (!mounted) return;
      setState(() {
        _placing = false;
        _error = e.message;
      });
    } catch (e) {
      if (!mounted) return;
      setState(() {
        _placing = false;
        _error = 'Could not place the order. Please try again.';
      });
    }
  }

  @override
  Widget build(BuildContext context) {
    final cart = ref.watch(cartProvider);
    final state = cart.maybeWhen(
      data: (v) => v,
      orElse: () => const CartState(),
    );

    if (state.isEmpty) {
      return Scaffold(
        body: SafeArea(
          child: Center(
            child: Column(
              mainAxisSize: MainAxisSize.min,
              children: [
                Text(
                  'Nothing to checkout.',
                  style: Theme.of(context).textTheme.titleMedium?.copyWith(
                        color: AppColors.onSurfaceMuted,
                      ),
                  textAlign: TextAlign.center,
                ),
                const SizedBox(height: 16),
                // Just-fit, not the global full-width stretch — a centered
                // nudge, not a banner. Back to the cart tab in the shell,
                // not pop(): dead history here is how the strand happens.
                FilledButton(
                  onPressed: () {
                    ref.read(shellTabProvider.notifier).select(3);
                    context.go('/home');
                  },
                  style: FilledButton.styleFrom(
                    minimumSize: const Size(0, 52),
                    padding: const EdgeInsets.symmetric(horizontal: 24),
                  ),
                  child: const Text('Back to cart'),
                ),
              ],
            ),
          ),
        ),
      );
    }

    final textTheme = Theme.of(context).textTheme;
    final storeCount = state.storeCount;

    return Scaffold(
      body: ListView(
        padding: const EdgeInsets.only(bottom: 120),
        children: [
          // Header
          Padding(
            padding: const EdgeInsets.fromLTRB(20, 20, 20, 16),
            child: Row(
              children: [
                IconButton(
                  icon: const Icon(Icons.arrow_back_ios_new_rounded, size: 18),
                  onPressed: () => context.pop(),
                ),
                const SizedBox(width: 4),
                Text(
                  'Checkout',
                  style: textTheme.titleLarge?.copyWith(
                    fontWeight: FontWeight.w700,
                  ),
                ),
              ],
            ),
          ),
          if (storeCount > 1)
            Padding(
              padding: const EdgeInsets.fromLTRB(20, 0, 20, 4),
              child: Text(
                '$storeCount stores are fulfilling this order — they\'ll arrive as separate deliveries.',
                style: textTheme.bodySmall?.copyWith(
                  color: AppColors.onSurfaceMuted,
                ),
              ),
            ),
          // Order summary, grouped by store exactly as it will be fulfilled.
          _SectionTitle('Order summary'),
          Padding(
            padding: const EdgeInsets.symmetric(horizontal: 20),
            child: Column(
              children: [
                for (final bucket in state.buckets) ...[
                  Padding(
                    padding: const EdgeInsets.only(top: 6, bottom: 2),
                    child: Row(
                      children: [
                        const Icon(Icons.storefront_rounded,
                            size: 14, color: AppColors.primary),
                        const SizedBox(width: 6),
                        Expanded(
                          child: Text(
                            bucket.storeName,
                            maxLines: 1,
                            overflow: TextOverflow.ellipsis,
                            style: textTheme.bodyMedium?.copyWith(
                              fontWeight: FontWeight.w700,
                            ),
                          ),
                        ),
                      ],
                    ),
                  ),
                  ...bucket.items.map(
                    (item) => Padding(
                      padding: const EdgeInsets.symmetric(vertical: 6),
                      child: Row(
                        children: [
                          Expanded(
                            child: Text(
                              '${item.name} × ${item.quantity}',
                              style: textTheme.bodyMedium,
                              maxLines: 1,
                              overflow: TextOverflow.ellipsis,
                            ),
                          ),
                          Text(
                            formatRwf(item.lineTotal),
                            style: textTheme.bodyMedium?.copyWith(
                              fontWeight: FontWeight.w600,
                            ),
                          ),
                        ],
                      ),
                    ),
                  ),
                  _TotalsRow(
                    label: 'Store subtotal',
                    value: bucket.subtotal,
                  ),
                  _TotalsRow(
                    label: 'Delivery',
                    value: bucket.deliveryFee,
                  ),
                  const SizedBox(height: 8),
                  const _Divider(),
                ],
                _TotalsRow(label: 'Subtotal', value: state.subtotal),
                _TotalsRow(
                  label: 'Delivery (all stores)',
                  value: state.deliveryTotal,
                ),
                const _Divider(),
                _TotalsRow(
                  label: 'Total',
                  value: state.total,
                  bold: true,
                ),
              ],
            ),
          ),
          const SizedBox(height: 24),
          // Delivery location — the real pin the rider will navigate to.
          // Tap the map to drop it; the human-readable label below stays
          // the address text.
          _SectionTitle('Delivery location'),
          Padding(
            padding: const EdgeInsets.symmetric(horizontal: 20),
            child: SizedBox(
              height: 220,
              child: DeliveryPinMap(
                pin: _pin,
                onPin: _choosePin,
                locate: ref.read(acquireLocationProvider),
              ),
            ),
          ),
          const SizedBox(height: 16),
          // Delivery address — one address for the whole purchase.
          _SectionTitle('Delivery address'),
          Padding(
            padding: const EdgeInsets.symmetric(horizontal: 20),
            child: TextField(
              controller: _addressController,
              maxLines: 2,
              decoration: InputDecoration(
                hintText: 'Street, building, landmark…',
                hintStyle: textTheme.bodyMedium?.copyWith(
                  color: AppColors.onSurfaceMuted,
                ),
                border: OutlineInputBorder(
                  borderRadius: BorderRadius.circular(12),
                  borderSide: BorderSide(color: AppColors.surfaceBorder),
                ),
                focusedBorder: OutlineInputBorder(
                  borderRadius: BorderRadius.circular(12),
                  borderSide:
                      const BorderSide(color: AppColors.primary, width: 1.5),
                ),
                errorBorder: OutlineInputBorder(
                  borderRadius: BorderRadius.circular(12),
                  borderSide:
                      const BorderSide(color: AppColors.error, width: 1.5),
                ),
                contentPadding:
                    const EdgeInsets.symmetric(horizontal: 14, vertical: 12),
              ),
              style: textTheme.bodyMedium,
            ),
          ),
          if (_error != null && !_error!.contains('order')) ...[
            Padding(
              padding: const EdgeInsets.fromLTRB(20, 6, 20, 0),
              child: Text(
                _error!,
                style: textTheme.bodySmall?.copyWith(color: AppColors.error),
              ),
            ),
          ],
          const SizedBox(height: 24),
          // Payment method — single option, non-interactive.
          _SectionTitle('Payment'),
          Padding(
            padding: const EdgeInsets.symmetric(horizontal: 20),
            child: Container(
              padding: const EdgeInsets.all(14),
              decoration: BoxDecoration(
                color: AppColors.primary.withValues(alpha: 0.08),
                borderRadius: BorderRadius.circular(12),
                border: Border.all(color: AppColors.primary.withValues(alpha: 0.2)),
              ),
              child: Row(
                children: [
                  Container(
                    padding: const EdgeInsets.all(8),
                    decoration: BoxDecoration(
                      color: AppColors.primary,
                      borderRadius: BorderRadius.circular(8),
                    ),
                    child: const Icon(
                      Icons.payment_rounded,
                      size: 18,
                      color: AppColors.onPrimary,
                    ),
                  ),
                  const SizedBox(width: 12),
                  Expanded(
                    child: Column(
                      crossAxisAlignment: CrossAxisAlignment.start,
                      children: [
                        Text(
                          'Cash on delivery',
                          style: textTheme.bodyMedium?.copyWith(
                            fontWeight: FontWeight.w600,
                          ),
                        ),
                        Text(
                          storeCount > 1
                              ? 'Pay the full total when the last delivery arrives.'
                              : 'Pay when your order arrives.',
                          style: textTheme.bodySmall?.copyWith(
                            color: AppColors.onSurfaceMuted,
                          ),
                        ),
                      ],
                    ),
                  ),
                  Container(
                    padding: const EdgeInsets.symmetric(horizontal: 8, vertical: 4),
                    decoration: BoxDecoration(
                      color: AppColors.primary,
                      borderRadius: BorderRadius.circular(999),
                    ),
                    child: const Icon(
                      Icons.check_rounded,
                      size: 16,
                      color: AppColors.onPrimary,
                    ),
                  ),
                ],
              ),
            ),
          ),
        ],
      ),
      bottomSheet: SafeArea(
        child: Padding(
          padding: const EdgeInsets.symmetric(horizontal: 20, vertical: 12),
          child: Column(
            mainAxisSize: MainAxisSize.min,
            children: [
              if (_error != null && _error!.contains('order'))
                Padding(
                  padding: const EdgeInsets.only(bottom: 8),
                  child: Text(
                    _error!,
                    style: textTheme.bodySmall?.copyWith(color: AppColors.error),
                  ),
                ),
              FilledButton(
                onPressed: _placing ? null : _placeOrder,
                style: FilledButton.styleFrom(
                  minimumSize: const Size.fromHeight(52),
                  disabledBackgroundColor: AppColors.onSurface.withValues(alpha: 0.15),
                ),
                child: _placing
                    ? const SizedBox(
                        height: 20,
                        width: 20,
                        child: CircularProgressIndicator(
                          strokeWidth: 2,
                          color: AppColors.onPrimary,
                        ),
                      )
                    : Text(
                        storeCount > 1
                            ? 'Place order · ${formatRwf(state.total)}'
                            : 'Place order',
                        style: const TextStyle(
                            fontSize: 16, fontWeight: FontWeight.w700),
                      ),
              ),
            ],
          ),
        ),
      ),
    );
  }
}

class _SectionTitle extends StatelessWidget {
  const _SectionTitle(this.title);

  final String title;

  @override
  Widget build(BuildContext context) {
    return Padding(
      padding: const EdgeInsets.fromLTRB(20, 0, 20, 8),
      child: Text(
        title,
        style: Theme.of(context).textTheme.titleSmall?.copyWith(
              fontWeight: FontWeight.w700,
            ),
      ),
    );
  }
}

class _Divider extends StatelessWidget {
  const _Divider();

  @override
  Widget build(BuildContext context) {
    return const Padding(
      padding: EdgeInsets.symmetric(vertical: 8),
      child: Divider(color: AppColors.surfaceBorder),
    );
  }
}

class _TotalsRow extends StatelessWidget {
  const _TotalsRow({
    required this.label,
    required this.value,
    this.bold = false,
  });

  final String label;
  final int value;
  final bool bold;

  @override
  Widget build(BuildContext context) {
    return Padding(
      padding: const EdgeInsets.symmetric(vertical: 4),
      child: Row(
        mainAxisAlignment: MainAxisAlignment.spaceBetween,
        children: [
          Text(
            label,
            style: Theme.of(context).textTheme.bodyMedium?.copyWith(
                  fontWeight: bold ? FontWeight.w700 : FontWeight.w500,
                  color: bold ? AppColors.onSurface : AppColors.onSurfaceMuted,
                ),
          ),
          Text(
            formatRwf(value),
            style: Theme.of(context).textTheme.bodyMedium?.copyWith(
                  fontWeight: bold ? FontWeight.w700 : FontWeight.w600,
                  color: bold ? AppColors.primary : AppColors.onSurface,
                ),
          ),
        ],
      ),
    );
  }
}
