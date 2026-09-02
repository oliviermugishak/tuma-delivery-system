import 'dart:async';
import 'dart:math';

import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:go_router/go_router.dart';

import 'package:tuma_app/core/api/api_client.dart';
import 'package:tuma_app/core/api/models/order.dart';
import 'package:tuma_app/core/auth/auth_controller.dart';
import 'package:tuma_app/core/api/models/address.dart';
import 'package:tuma_app/core/theme/app_colors.dart';
import 'package:tuma_app/core/theme/app_theme.dart';
import 'package:tuma_app/shared/widgets/design_system.dart';
import 'package:tuma_app/core/utils/format_rwf.dart';
import 'package:tuma_app/features/cart/cart_notifier.dart';
import 'package:tuma_app/features/orders/active_orders_provider.dart';
import 'package:tuma_app/features/orders/success_screen.dart';
import 'package:tuma_app/features/home/app_shell.dart' show shellTabProvider;
import 'package:tuma_app/features/location/customer_location.dart';

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
  List<Address>? _savedAddresses;
  Address? _selectedAddress;
  final TextEditingController _noteController = TextEditingController();

  /// Which slot renders [_error]: the form banner (field-level, e.g. a
  /// missing address) or the items-area banner (cart/server conflicts).
  bool _errorIsFormLevel = false;

  /// The delivery pin from the map (or GPS). Seeds itself from the
  /// persisted customer location; every choice re-persists, so Home's
  /// distances sharpen after the first checkout too.
  CustomerLocation? _pin;

  /// Generated once per checkout attempt and kept until the order lands,
  /// so a retry (timeout, back button, re-tap) can never place twice.
  String? _idempotencyKey;

  @override
  void initState() {
    unawaited(_loadAddresses());
    super.initState();
    unawaited(_initPin());
  }

  Future<void> _loadAddresses() async {
    try {
      final addresses = await ref.read(addressApiProvider).list();
      if (!mounted) return;
      setState(() {
        _savedAddresses = addresses;
        _selectedAddress = addresses.isNotEmpty
            ? addresses.firstWhere(
                (a) => a.isDefault,
                orElse: () => addresses.first,
              )
            : null;
        _seedFromSelectedAddress();
      });
    } on ApiError {
      // The location is simply not chosen yet — checkout requires it.
    }
  }

  /// A saved address drives the address field + pin (P3's answer:
  /// geocoding is the system's job; the address row already knows).
  /// Shared by the initial load and an explicit pick so both seed the
  /// form identically.
  void _seedFromSelectedAddress() {
    final selected = _selectedAddress;
    if (selected == null) return;
    _addressController.text = selected.addressText;
    // The rider note travels with the address — pre-fill it so the
    // rider always sees the gate detail (P8's loop closed).
    final note = selected.note;
    if (_noteController.text.isEmpty && note != null) {
      _noteController.text = note;
    }
    _pin = selected.lat != null && selected.lng != null
        ? CustomerLocation(lat: selected.lat!, lng: selected.lng!)
        : _pin;
  }

  /// Pick among saved addresses (>1): a bottom sheet of the book, plus
  /// the editor for a new pin. Returns the chosen address, or null when
  /// the sheet was dismissed.
  Future<Address?> _pickAddress(List<Address> saved) {
    final textTheme = Theme.of(context).textTheme;
    return showModalBottomSheet<Address>(
      context: context,
      backgroundColor: AppColors.surface,
      // A grown address book scrolls; fixed rows in a tight Column
      // overflowed the sheet (A31).
      isScrollControlled: true,
      shape: const RoundedRectangleBorder(
        borderRadius: BorderRadius.vertical(top: Radius.circular(24)),
      ),
      builder: (sheetContext) => SafeArea(
        child: ConstrainedBox(
          constraints: BoxConstraints(
            maxHeight: MediaQuery.of(context).size.height * 0.6,
          ),
          child: Column(
            mainAxisSize: MainAxisSize.min,
            crossAxisAlignment: CrossAxisAlignment.start,
            children: [
              const Padding(
                padding: EdgeInsets.fromLTRB(20, 16, 20, 4),
                child: MicroLabel('Deliver to'),
              ),
              Flexible(
                child: ListView(
                  shrinkWrap: true,
                  children: [
                    for (final address in saved) ...[
                      InkWell(
                        onTap: () => Navigator.of(sheetContext).pop(address),
                        child: Padding(
                          padding: const EdgeInsets.symmetric(
                            horizontal: 20,
                            vertical: 12,
                          ),
                          child: Row(
                            children: [
                              Icon(
                                switch (address.kind) {
                                  'home' => Icons.home_rounded,
                                  'work' => Icons.work_rounded,
                                  _ => Icons.place_rounded,
                                },
                                size: 20,
                                color: address.id == _selectedAddress?.id
                                    ? AppColors.primary
                                    : AppColors.onSurfaceMuted,
                              ),
                              const SizedBox(width: 12),
                              Expanded(
                                child: Column(
                                  crossAxisAlignment: CrossAxisAlignment.start,
                                  children: [
                                    Text(
                                      address.addressText,
                                      maxLines: 1,
                                      overflow: TextOverflow.ellipsis,
                                      style: AppTheme.bd(
                                        textTheme,
                                      ).copyWith(fontWeight: FontWeight.w600),
                                    ),
                                    const SizedBox(height: 2),
                                    Text(
                                      address.label,
                                      style: AppTheme.sub(textTheme),
                                    ),
                                  ],
                                ),
                              ),
                              if (address.id == _selectedAddress?.id)
                                const Icon(
                                  Icons.check_rounded,
                                  size: 18,
                                  color: AppColors.primary,
                                ),
                            ],
                          ),
                        ),
                      ),
                      const Divider(
                        height: 1,
                        indent: 20,
                        color: AppColors.surfaceBorder,
                      ),
                    ],
                  ],
                ),
              ),
            ],
          ),
        ),
      ),
    );
  }

  Future<void> _initPin() async {
    final persisted = await ref.read(customerLocationProvider.future);
    if (!mounted || _pin != null) return;
    setState(() => _pin = persisted);
  }

  @override
  void dispose() {
    _addressController.dispose();
    _noteController.dispose();
    super.dispose();
  }

  /// A reasonably unique key without a uuid dependency: time + random.
  String _newIdempotencyKey() {
    final random = Random.secure();
    final suffix = List.generate(
      8,
      (_) => random.nextInt(16).toRadixString(16),
    ).join();
    return 'mob-${DateTime.now().microsecondsSinceEpoch.toRadixString(36)}-$suffix';
  }

  Future<void> _placeOrder() async {
    if (_selectedAddress == null) {
      setState(() {
        _error =
            'Choose a delivery location — tap the card above and drop the pin.';
        _errorIsFormLevel = true;
      });
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
          _errorIsFormLevel = false;
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
        customerNote: _noteController.text.trim().isEmpty
            ? null
            : _noteController.text.trim(),
        idempotencyKey: _idempotencyKey ??= _newIdempotencyKey(),
        items: lines,
      );
      final placed = await ref.read(orderApiProvider).checkout(request);
      if (!mounted) return;
      // The checkout is server-side truth now — clear the cart and the
      // one-shot key with it.
      _idempotencyKey = null;
      await ref.read(cartProvider.notifier).clear();
      if (!mounted) return;
      // A just-placed order appears on Home and Orders the moment the
      // user looks — one silent poll refresh, no waiting for a tick.
      unawaited(ref.read(activeOrdersProvider.notifier).poke());
      // The success screen: celebration + one-tap tracking (P12, P17).
      // Never pop here: the dead checkout under this screen is exactly
      // the strand — success REPLACES this route.
      final firstStore = placed.storeOrders.isNotEmpty
          ? placed.storeOrders.first.storeName
          : 'Your order';
      // No local ETA preview: the placed response carries no coordinates
      // and the client doesn't geocode — the server's per-delivery ETA
      // rides the tracking endpoint instead (the success card stays
      // silent until then; P2: no invented numbers).
      if (!mounted) return;
      context.pushReplacement(
        '/success',
        extra: SuccessScreenArgs(
          groupId: placed.id,
          orderNumber: placed.number,
          storeName: firstStore,
          total: placed.grandTotal,
        ),
      );
    } on ApiBadRequest catch (e) {
      _showPlacedError(e.message);
    } on ApiConflict catch (e) {
      // The world moved between cart and checkout: a store closed, an
      // item vanished, stock ran out. The message names the offenders.
      _showPlacedError(e.message);
    } catch (e) {
      _showPlacedError('Could not place the order. Please try again.');
    }
  }

  /// A server-rejected placement renders in the items-area banner — these
  /// are cart conflicts, not field mistakes.
  void _showPlacedError(String message) {
    if (!mounted) return;
    setState(() {
      _placing = false;
      _error = message;
      _errorIsFormLevel = false;
    });
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
                  style: Theme.of(context).textTheme.titleMedium
                      ?.copyWith(color: AppColors.onSurfaceMuted),
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

    // No backing out while the placement POST is in flight (A28): the
    // popped State would take the checkout's idempotency key with it, so
    // the next placement books a second order. PopScope blocks the
    // system gesture/back button; go_router's imperative context.pop()
    // bypasses PopScope, so the app-bar back guards itself below.
    return PopScope(
      canPop: !_placing,
      child: Scaffold(
        body: ListView(
          padding: const EdgeInsets.only(bottom: 120),
          children: [
            // Header
            Padding(
              padding: const EdgeInsets.fromLTRB(16, 12, 16, 0),
              child: Row(
                children: [
                  IconButton(
                    icon: const Icon(Icons.arrow_back_rounded, size: 22),
                    // A cold start on /checkout has nothing to pop — fall
                    // home instead of throwing (review P16).
                    onPressed: () {
                      // While the placement POST is in flight, back is
                      // inert (A28): go_router's imperative pop bypasses
                      // PopScope, so the guard lives here too.
                      if (_placing) return;
                      if (context.canPop()) {
                        context.pop();
                      } else {
                        context.go('/home');
                      }
                    },
                  ),
                  const SizedBox(width: 8),
                  Text(
                    'Checkout',
                    style: AppTheme.d1(textTheme).copyWith(fontSize: 21),
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
            // DELIVER TO — saved-address-first (P3's answer: geocoding is
            // the system's job). The selected row drives the checkout's
            // address fields; Change swaps; dashed Add new creates. With no
            // saved addresses yet, an honest text field takes the row's
            // place (P12: the fallback is designed too) — the persisted pin
            // still carries the coordinates.
            Padding(
              padding: const EdgeInsets.fromLTRB(16, 16, 16, 8),
              child: MicroLabel('Deliver to'),
            ),
            Padding(
              padding: const EdgeInsets.symmetric(horizontal: 16),
              child: Container(
                padding: const EdgeInsets.symmetric(
                  horizontal: 14,
                  vertical: 12,
                ),
                decoration: BoxDecoration(
                  color: AppColors.surfaceAlt,
                  borderRadius: BorderRadius.circular(16),
                  border: Border.all(color: AppColors.surfaceBorder),
                ),
                child: _selectedAddress == null
                    ? InkWell(
                        onTap: () => unawaited(() async {
                          await context.push('/profile/location');
                          if (mounted) await _loadAddresses();
                        }()),
                        borderRadius: BorderRadius.circular(16),
                        child: Row(
                          children: [
                            const Icon(
                              Icons.location_searching_rounded,
                              size: 20,
                              color: AppColors.primary,
                            ),
                            const SizedBox(width: 12),
                            Expanded(
                              child: Column(
                                crossAxisAlignment: CrossAxisAlignment.start,
                                children: [
                                  Text(
                                    'Choose your delivery location',
                                    style: AppTheme.bd(textTheme).copyWith(
                                      fontWeight: FontWeight.w600,
                                      color: AppColors.primary,
                                    ),
                                  ),
                                  const SizedBox(height: 2),
                                  Text(
                                    'Required — pick on the map or use your GPS.',
                                    style: AppTheme.sub(textTheme),
                                  ),
                                ],
                              ),
                            ),
                            const Icon(
                              Icons.chevron_right_rounded,
                              size: 20,
                              color: AppColors.onSurfaceMuted,
                            ),
                          ],
                        ),
                      )
                    : Row(
                        children: [
                          const Icon(
                            Icons.location_on_rounded,
                            size: 20,
                            color: AppColors.primary,
                          ),
                          const SizedBox(width: 12),
                          Expanded(
                            child: Column(
                              crossAxisAlignment: CrossAxisAlignment.start,
                              children: [
                                Text(
                                  _addressController.text.trim().isEmpty
                                      ? 'Set your delivery address'
                                      : _addressController.text.trim(),
                                  maxLines: 1,
                                  overflow: TextOverflow.ellipsis,
                                  style: AppTheme.bd(textTheme)
                                      .copyWith(fontWeight: FontWeight.w600),
                                ),
                                const SizedBox(height: 2),
                                Text(
                                  // Honest labeling (A36): "Default" only
                                  // when the picked address IS the default.
                                  '${_selectedAddress!.label}'
                                  '${_selectedAddress!.isDefault ? ' · Default' : ''}'
                                  '${(_savedAddresses?.length ?? 0) > 1 ? ' · ${_savedAddresses!.length} saved' : ''}',
                                  style: AppTheme.sub(textTheme),
                                ),
                              ],
                            ),
                          ),
                          TextButton(
                            onPressed: () => unawaited(() async {
                              final saved = _savedAddresses ?? const [];
                              if (saved.length > 1) {
                                final picked = await _pickAddress(saved);
                                if (!mounted) return;
                                if (picked == null) return;
                                // Apply the pick FULLY — state, address
                                // field, and pin — exactly as the initial
                                // load seeds it. No reload here: one would
                                // re-select the default and throw the
                                // user's choice away (A27).
                                setState(() => _selectedAddress = picked);
                                _seedFromSelectedAddress();
                                return;
                              }
                              await context.push(
                                '/profile/location',
                                extra: _selectedAddress,
                              );
                              // The editor may have changed the book —
                              // reload (and re-select default) only here.
                              if (mounted) await _loadAddresses();
                            }()),
                            child: const Text('Change'),
                          ),
                        ],
                      ),
              ),
            ),
            const SizedBox(height: 10),
            Padding(
              padding: const EdgeInsets.symmetric(horizontal: 16),
              child: DashedAddRow(
                label: 'Add new address',
                // Await the editor and reload on return, exactly like the
                // no-addresses entry point above — a fresh save must show
                // up (A38).
                onTap: () => unawaited(() async {
                  await context.push('/profile/location');
                  if (mounted) await _loadAddresses();
                }()),
              ),
            ),
            if (_error != null && _errorIsFormLevel) ...[
              Padding(
                padding: const EdgeInsets.fromLTRB(16, 6, 16, 0),
                child: Text(
                  _error!,
                  style: textTheme.bodySmall?.copyWith(color: AppColors.error),
                ),
              ),
            ],
            // PAYMENT — cash selected, MoMo visible-but-disabled: showing
            // the roadmap is honest and sets the mental model (P12).
            const Padding(
              padding: EdgeInsets.fromLTRB(16, 22, 16, 8),
              child: MicroLabel('Payment'),
            ),
            Padding(
              padding: const EdgeInsets.symmetric(horizontal: 16),
              child: Container(
                decoration: BoxDecoration(
                  color: AppColors.surfaceAlt,
                  borderRadius: BorderRadius.circular(16),
                  border: Border.all(color: AppColors.surfaceBorder),
                ),
                child: Column(
                  children: [
                    Padding(
                      padding: const EdgeInsets.all(14),
                      child: Row(
                        children: [
                          const _PaymentTile(icon: Icons.payments_rounded),
                          const SizedBox(width: 12),
                          Expanded(
                            child: Column(
                              crossAxisAlignment: CrossAxisAlignment.start,
                              children: [
                                Text(
                                  'Cash on delivery',
                                  style: AppTheme.bd(textTheme)
                                      .copyWith(fontWeight: FontWeight.w600),
                                ),
                                Text(
                                  storeCount > 1
                                      ? 'Pay the full total when the last delivery arrives.'
                                      : 'Pay when your order arrives.',
                                  style: AppTheme.sub(textTheme),
                                ),
                              ],
                            ),
                          ),
                          const _RadioDot(selected: true),
                        ],
                      ),
                    ),
                    const Divider(height: 1, indent: 14, endIndent: 14),
                    Opacity(
                      opacity: 0.5,
                      child: Padding(
                        padding: const EdgeInsets.all(14),
                        child: Row(
                          children: [
                            Container(
                              width: 40,
                              height: 40,
                              decoration: BoxDecoration(
                                color: AppColors.surfaceHigh,
                                borderRadius: BorderRadius.circular(12),
                              ),
                              child: const Icon(
                                Icons.smartphone_rounded,
                                size: 20,
                                color: AppColors.onSurfaceMuted,
                              ),
                            ),
                            const SizedBox(width: 12),
                            Expanded(
                              child: Column(
                                crossAxisAlignment: CrossAxisAlignment.start,
                                children: [
                                  Text(
                                    'MoMo MTN',
                                    style: AppTheme.bd(textTheme)
                                        .copyWith(fontWeight: FontWeight.w600),
                                  ),
                                  Text(
                                    'Coming soon',
                                    style: AppTheme.sub(textTheme),
                                  ),
                                ],
                              ),
                            ),
                            const _RadioDot(selected: false),
                          ],
                        ),
                      ),
                    ),
                  ],
                ),
              ),
            ),
            // NOTE FOR RIDER — optional; the rider's Delivering card
            // displays it. The loop has a consumer, so here's the producer.
            const Padding(
              padding: EdgeInsets.fromLTRB(16, 22, 16, 8),
              child: MicroLabel('Note for rider · optional'),
            ),
            Padding(
              padding: const EdgeInsets.symmetric(horizontal: 16),
              child: TextField(
                controller: _noteController,
                maxLength: 140,
                style: AppTheme.bd(textTheme),
                decoration: const InputDecoration(
                  hintText: 'e.g. blue gate, ring the bell…',
                  counterText: '',
                ),
              ),
            ),
            // ORDER SUMMARY — one block (P1).
            const Padding(
              padding: EdgeInsets.fromLTRB(16, 22, 16, 8),
              child: MicroLabel('Order summary'),
            ),
            Padding(
              padding: const EdgeInsets.symmetric(horizontal: 16),
              child: Container(
                padding: const EdgeInsets.all(14),
                decoration: BoxDecoration(
                  color: AppColors.surfaceAlt,
                  borderRadius: BorderRadius.circular(16),
                  border: Border.all(color: AppColors.surfaceBorder),
                ),
                child: Column(
                  children: [
                    for (final bucket in state.buckets)
                      for (final item in bucket.items)
                        Padding(
                          padding: const EdgeInsets.only(bottom: 5),
                          child: Row(
                            children: [
                              Expanded(
                                child: Text(
                                  '${item.name} × ${item.quantity}',
                                  style: AppTheme.bd(
                                    textTheme,
                                  ).copyWith(color: AppColors.onSurfaceMuted),
                                  maxLines: 1,
                                  overflow: TextOverflow.ellipsis,
                                ),
                              ),
                              Text(
                                formatRwf(item.lineTotal),
                                style: AppTheme.bd(textTheme),
                              ),
                            ],
                          ),
                        ),
                    Padding(
                      padding: const EdgeInsets.only(bottom: 5),
                      child: Row(
                        mainAxisAlignment: MainAxisAlignment.spaceBetween,
                        children: [
                          Text(
                            'Delivery fee',
                            style: AppTheme.bd(textTheme)
                                .copyWith(color: AppColors.onSurfaceMuted),
                          ),
                          Text(
                            formatRwf(state.deliveryTotal),
                            style: AppTheme.bd(textTheme),
                          ),
                        ],
                      ),
                    ),
                    const Divider(color: AppColors.surfaceBorder),
                    Row(
                      mainAxisAlignment: MainAxisAlignment.spaceBetween,
                      children: [
                        Text(
                          'Total',
                          style: AppTheme.bd(textTheme)
                              .copyWith(fontWeight: FontWeight.w600),
                        ),
                        Text(
                          formatRwf(state.total),
                          style: textTheme.titleSmall?.copyWith(
                            fontSize: 15,
                            color: AppColors.primary,
                            fontWeight: FontWeight.w700,
                          ),
                        ),
                      ],
                    ),
                  ],
                ),
              ),
            ),
            const SizedBox(height: 24),
          ],
        ),
        bottomSheet: SafeArea(
          child: Padding(
            padding: const EdgeInsets.symmetric(horizontal: 16, vertical: 12),
            child: Column(
              mainAxisSize: MainAxisSize.min,
              children: [
                if (_error != null && !_errorIsFormLevel)
                  Padding(
                    padding: const EdgeInsets.only(bottom: 8),
                    child: Text(
                      _error!,
                      style: textTheme.bodySmall?.copyWith(
                        color: AppColors.error,
                      ),
                    ),
                  ),
                FilledButton(
                  onPressed: _placing ? null : _placeOrder,
                  style: FilledButton.styleFrom(
                    disabledBackgroundColor: AppColors.onSurface.withValues(
                      alpha: 0.15,
                    ),
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
                          'Place order · ${formatRwf(state.total)}',
                          style: const TextStyle(
                            fontSize: 15,
                            fontWeight: FontWeight.w600,
                          ),
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

/// The payment row's tile: the accent-well icon container.
class _PaymentTile extends StatelessWidget {
  const _PaymentTile({required this.icon});

  final IconData icon;

  @override
  Widget build(BuildContext context) {
    return Container(
      width: 40,
      height: 40,
      decoration: BoxDecoration(
        color: AppColors.primary.withValues(alpha: 0.12),
        borderRadius: BorderRadius.circular(12),
      ),
      child: Icon(icon, size: 20, color: AppColors.primary),
    );
  }
}

/// The radio dot — the spec's `.radio`: accent ring + accent fill when
/// selected.
class _RadioDot extends StatelessWidget {
  const _RadioDot({required this.selected});

  final bool selected;

  @override
  Widget build(BuildContext context) {
    return Container(
      width: 22,
      height: 22,
      decoration: BoxDecoration(
        shape: BoxShape.circle,
        border: Border.all(
          color: selected ? AppColors.primary : AppColors.line,
          width: 2,
        ),
      ),
      alignment: Alignment.center,
      child: selected
          ? Container(
              width: 11,
              height: 11,
              decoration: const BoxDecoration(
                color: AppColors.primary,
                shape: BoxShape.circle,
              ),
            )
          : null,
    );
  }
}
