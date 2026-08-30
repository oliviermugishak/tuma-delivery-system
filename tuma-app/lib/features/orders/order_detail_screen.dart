import 'dart:async';

import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:go_router/go_router.dart';

import 'package:tuma_app/core/api/api_client.dart';
import 'package:tuma_app/core/api/models/order.dart';
import 'package:tuma_app/core/auth/auth_controller.dart';
import 'package:tuma_app/core/theme/app_colors.dart';
import 'package:tuma_app/core/utils/format_rwf.dart';
import 'package:tuma_app/shared/widgets/error_state.dart';

/// Order group detail — the one purchase the customer placed, rendered as
/// one section per fulfilling store. Each store has its own status
/// timeline and can be cancelled independently while it's still on the
/// premises. While anything is still in flight the screen polls the
/// server every few seconds (V1's honest realtime: pull + gentle polling,
/// no simulation) and stops the moment the group settles.
class OrderDetailScreen extends ConsumerStatefulWidget {
  const OrderDetailScreen({super.key, required this.orderId});

  final String orderId;

  @override
  ConsumerState<OrderDetailScreen> createState() => _OrderDetailScreenState();
}

/// How often the in-flight group re-fetches its state.
const _pollInterval = Duration(seconds: 5);

class _OrderDetailScreenState extends ConsumerState<OrderDetailScreen>
    with WidgetsBindingObserver {
  OrderGroup? _order;
  String? _error;
  Timer? _pollTimer;

  Future<void> _load() async {
    setState(() {
      _error = null;
      _order = null;
    });
    try {
      final order = await ref.read(orderApiProvider).getGroup(widget.orderId);
      if (!mounted) return;
      setState(() => _order = order);
      _syncPolling(order);
    } on ApiNotFound {
      if (!mounted) return;
      setState(() => _error = 'not_found');
      _pollTimer?.cancel();
    } on ApiError catch (e) {
      if (!mounted) return;
      setState(() => _error = e.message);
    }
  }

  /// Poll only while the purchase is alive; a settled group is a fact,
  /// not a stream.
  void _syncPolling(OrderGroup order) {
    if (order.isInFlight) {
      _pollTimer ??= Timer.periodic(_pollInterval, (_) => unawaited(_refresh()));
    } else {
      _pollTimer?.cancel();
      _pollTimer = null;
    }
  }

  /// A quiet re-fetch: no loading spinner, just the freshest state.
  Future<void> _refresh() async {
    try {
      final order = await ref.read(orderApiProvider).getGroup(widget.orderId);
      if (!mounted) return;
      setState(() => _order = order);
      _syncPolling(order);
    } on Object {
      // A failed poll keeps the last good state; the next tick retries.
    }
  }

  Future<void> _cancelStoreOrder(StoreOrder order) async {
    final confirmed = await showDialog<bool>(
      context: context,
      builder: (dialogContext) => AlertDialog(
        backgroundColor: AppColors.surfaceAlt,
        title: const Text('Cancel this order?'),
        content: Text(
          'Your order from ${order.storeName} will be cancelled. '
          'The other stores in this purchase are unaffected.',
        ),
        actions: [
          TextButton(
            onPressed: () => Navigator.of(dialogContext).pop(false),
            child: const Text('Keep it'),
          ),
          TextButton(
            onPressed: () => Navigator.of(dialogContext).pop(true),
            style: TextButton.styleFrom(foregroundColor: AppColors.error),
            child: const Text('Cancel order'),
          ),
        ],
      ),
    );
    if (confirmed != true) return;

    try {
      await ref
          .read(orderApiProvider)
          .cancelStoreOrder(widget.orderId, order.id);
      await _refresh();
      if (!mounted) return;
      ScaffoldMessenger.of(context).hideCurrentSnackBar();
      ScaffoldMessenger.of(context).showSnackBar(
        SnackBar(
          content: Text(
            'Your ${order.storeName} order was cancelled.',
            style: const TextStyle(color: AppColors.onSurface),
          ),
          behavior: SnackBarBehavior.floating,
          duration: const Duration(seconds: 2),
          backgroundColor: AppColors.surfaceAlt,
          shape: RoundedRectangleBorder(
            borderRadius: BorderRadius.circular(12),
          ),
        ),
      );
    } on ApiBadRequest catch (e) {
      if (!mounted) return;
      ScaffoldMessenger.of(context).showSnackBar(
        SnackBar(
          content: Text(e.message, style: const TextStyle(color: AppColors.onSurface)),
          behavior: SnackBarBehavior.floating,
          backgroundColor: AppColors.surfaceAlt,
          shape: RoundedRectangleBorder(
            borderRadius: BorderRadius.circular(12),
          ),
        ),
      );
      await _refresh();
    } on Object {
      if (!mounted) return;
      ScaffoldMessenger.of(context).showSnackBar(
        SnackBar(
          content: const Text(
            'Could not cancel right now. Try again.',
            style: TextStyle(color: AppColors.onSurface),
          ),
          behavior: SnackBarBehavior.floating,
          backgroundColor: AppColors.surfaceAlt,
          shape: RoundedRectangleBorder(
            borderRadius: BorderRadius.circular(12),
          ),
        ),
      );
    }
  }

  @override
  void initState() {
    super.initState();
    WidgetsBinding.instance.addObserver(this);
    unawaited(_load());
  }

  @override
  void didChangeAppLifecycleState(AppLifecycleState state) {
    // A backgrounded screen must not keep polling — that is someone
    // else's battery. Pause cancels the timer; resume re-fetches and
    // restarts it only while the group is still in flight.
    if (state == AppLifecycleState.paused ||
        state == AppLifecycleState.hidden) {
      _pollTimer?.cancel();
      _pollTimer = null;
    } else if (state == AppLifecycleState.resumed) {
      final order = _order;
      if (order != null && order.isInFlight) {
        unawaited(_refresh());
        _syncPolling(order);
      }
    }
  }

  @override
  void dispose() {
    WidgetsBinding.instance.removeObserver(this);
    _pollTimer?.cancel();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    if (_order == null && _error == null) {
      return const Scaffold(body: Center(child: CircularProgressIndicator()));
    }
    if (_order == null && _error == 'not_found') {
      return Scaffold(
        body: SafeArea(
          child: Center(
            child: Column(
              mainAxisSize: MainAxisSize.min,
              children: [
                const Icon(Icons.error_outline, size: 48, color: AppColors.onSurfaceMuted),
                const SizedBox(height: 16),
                Text(
                  'Order not found',
                  style: Theme.of(context).textTheme.titleMedium?.copyWith(
                        fontWeight: FontWeight.w700,
                      ),
                ),
                const SizedBox(height: 8),
                Text(
                  'This order may have been removed or you don\'t have access to it.',
                  style: Theme.of(context).textTheme.bodyMedium?.copyWith(
                        color: AppColors.onSurfaceMuted,
                      ),
                  textAlign: TextAlign.center,
                ),
                const SizedBox(height: 24),
                FilledButton(
                  onPressed: () => context.go('/home'),
                  child: const Text('Back to home'),
                ),
              ],
            ),
          ),
        ),
      );
    }
    if (_order == null && _error != null) {
      return Scaffold(
        body: ErrorState(
          message: _error!,
          onRetry: () => unawaited(_load()),
        ),
      );
    }
    final order = _order!;
    final textTheme = Theme.of(context).textTheme;
    final storeCount = order.storeOrders.length;

    return Scaffold(
      // A plain pinned AppBar — no collapsible layers to overlap — with an
      // explicit way home: whatever the navigation stack looks like (fresh
      // checkout push, deep link, restored session), there is always a way
      // out of this screen.
      appBar: AppBar(
        backgroundColor: AppColors.surfaceAlt,
        surfaceTintColor: Colors.transparent,
        elevation: 0,
        scrolledUnderElevation: 0,
        leading: IconButton(
          icon: const Icon(Icons.arrow_back_rounded,
              color: AppColors.onSurface),
          onPressed: () {
            if (context.canPop()) {
              context.pop();
            } else {
              context.go('/home');
            }
          },
        ),
        title: Text(
          'Order #${order.number}',
          style: const TextStyle(fontWeight: FontWeight.w700),
          overflow: TextOverflow.ellipsis,
        ),
        actions: [
          IconButton(
            tooltip: 'Home',
            icon: const Icon(Icons.home_outlined, color: AppColors.onSurface),
            onPressed: () => context.go('/home'),
          ),
          const SizedBox(width: 4),
        ],
      ),
      body: RefreshIndicator(
        onRefresh: () async => _load(),
        color: AppColors.primary,
        child: CustomScrollView(
          slivers: [
            // Overall state + payment.
            SliverToBoxAdapter(
              child: Padding(
                padding: const EdgeInsets.fromLTRB(20, 8, 20, 0),
                child: Column(
                  crossAxisAlignment: CrossAxisAlignment.start,
                  children: [
                    Row(
                      children: [
                        _GroupStatusChip(status: order.status),
                        const SizedBox(width: 8),
                        if (order.isInFlight)
                          const _LiveBadge(),
                      ],
                    ),
                    const SizedBox(height: 6),
                    Text(
                      storeCount > 1
                          ? '$storeCount stores are fulfilling this order.'
                          : '${order.storeOrders.first.storeName} is fulfilling this order.',
                      style: textTheme.bodySmall?.copyWith(
                        color: AppColors.onSurfaceMuted,
                      ),
                    ),
                    const SizedBox(height: 4),
                    Text(
                      order.paymentStatus == 'pending'
                          ? 'Cash on delivery — pay ${formatRwf(order.grandTotal)} when it arrives.'
                          : 'Payment: ${order.paymentStatus} · ${formatRwf(order.grandTotal)}',
                      style: textTheme.bodySmall?.copyWith(
                        color: AppColors.onSurfaceMuted,
                      ),
                    ),
                  ],
                ),
              ),
            ),
            // One section per store: identity, timeline, items, totals.
            for (final storeOrder in order.storeOrders) ...[
              const SliverToBoxAdapter(child: SizedBox(height: 20)),
              SliverToBoxAdapter(
                child: _StoreOrderSection(
                  storeOrder: storeOrder,
                  onCancel: () => unawaited(_cancelStoreOrder(storeOrder)),
                ),
              ),
            ],
            const SliverToBoxAdapter(child: SizedBox(height: 16)),
            // The purchase's totals.
            SliverToBoxAdapter(
              child: _SectionTitle('Your purchase'),
            ),
            SliverToBoxAdapter(
              child: Padding(
                padding: const EdgeInsets.symmetric(horizontal: 20),
                child: Column(
                  children: [
                    _TotalsRow(label: 'Subtotal', value: order.subtotal),
                    _TotalsRow(
                      label: storeCount > 1
                          ? 'Delivery (all stores)'
                          : 'Delivery fee',
                      value: order.deliveryTotal,
                    ),
                    const _Divider(),
                    _TotalsRow(
                      label: 'Total',
                      value: order.grandTotal,
                      bold: true,
                    ),
                  ],
                ),
              ),
            ),
            // Delivery address
            if (order.addressText.isNotEmpty) ...[
              const SliverToBoxAdapter(child: SizedBox(height: 16)),
              const SliverToBoxAdapter(child: _SectionTitle('Delivery address')),
              SliverToBoxAdapter(
                child: Padding(
                  padding: const EdgeInsets.symmetric(horizontal: 20),
                  child: Container(
                    padding: const EdgeInsets.all(14),
                    decoration: BoxDecoration(
                      color: AppColors.surface.withValues(alpha: 0.5),
                      borderRadius: BorderRadius.circular(12),
                    ),
                    child: Row(
                      children: [
                        const Icon(Icons.location_on_rounded,
                            color: AppColors.primary, size: 20),
                        const SizedBox(width: 10),
                        Expanded(
                          child: Text(
                            order.addressText,
                            style: Theme.of(context).textTheme.bodyMedium,
                          ),
                        ),
                      ],
                    ),
                  ),
                ),
              ),
            ],
            // Wayfinding: a settled purchase always offers the way back to
            // shopping — nobody ends on a dead screen.
            if (!order.isInFlight) ...[
              const SliverToBoxAdapter(child: SizedBox(height: 8)),
              SliverToBoxAdapter(
                child: Padding(
                  padding: const EdgeInsets.symmetric(horizontal: 20),
                  child: FilledButton.icon(
                    onPressed: () => context.go('/home'),
                    style: FilledButton.styleFrom(
                      minimumSize: const Size.fromHeight(52),
                    ),
                    icon: const Icon(Icons.storefront_rounded),
                    label: const Text(
                      'Continue shopping',
                      style: TextStyle(
                          fontSize: 16, fontWeight: FontWeight.w700),
                    ),
                  ),
                ),
              ),
            ],
            // Footer note
            SliverToBoxAdapter(
              child: Padding(
                padding: const EdgeInsets.fromLTRB(20, 24, 20, 32),
                child: Text(
                  order.isInFlight
                      ? 'This screen updates itself — or pull to refresh now.'
                      : 'Pull to refresh any time.',
                  style: Theme.of(context).textTheme.labelSmall?.copyWith(
                        color: AppColors.onSurfaceMuted,
                      ),
                  textAlign: TextAlign.center,
                ),
              ),
            ),
          ],
        ),
      ),
    );
  }
}

/// The overall group state chip.
class _GroupStatusChip extends StatelessWidget {
  const _GroupStatusChip({required this.status});

  final String status;

  Color get _color {
    switch (status) {
      case 'in_progress':
        return AppColors.primary;
      case 'partially_fulfilled':
        return const Color(0xFF8B5CF6); // purple
      case 'completed':
        return AppColors.success;
      case 'cancelled':
        return AppColors.error;
      default:
        return AppColors.onSurfaceMuted;
    }
  }

  String get _label {
    switch (status) {
      case 'in_progress':
        return 'In progress';
      case 'partially_fulfilled':
        return 'Partially fulfilled';
      case 'completed':
        return 'Completed';
      case 'cancelled':
        return 'Cancelled';
      default:
        return status;
    }
  }

  @override
  Widget build(BuildContext context) {
    return Container(
      padding: const EdgeInsets.symmetric(horizontal: 10, vertical: 5),
      decoration: BoxDecoration(
        color: _color.withValues(alpha: 0.12),
        borderRadius: BorderRadius.circular(999),
      ),
      child: Text(
        _label,
        style: Theme.of(context).textTheme.labelSmall?.copyWith(
              color: _color,
              fontWeight: FontWeight.w700,
            ),
      ),
    );
  }
}

/// A quiet pulse: the group is still moving and the screen is watching it.
class _LiveBadge extends StatelessWidget {
  const _LiveBadge();

  @override
  Widget build(BuildContext context) {
    return Container(
      padding: const EdgeInsets.symmetric(horizontal: 8, vertical: 5),
      decoration: BoxDecoration(
        color: AppColors.success.withValues(alpha: 0.1),
        borderRadius: BorderRadius.circular(999),
      ),
      child: Row(
        mainAxisSize: MainAxisSize.min,
        children: [
          Container(
            width: 6,
            height: 6,
            decoration: const BoxDecoration(
              color: AppColors.success,
              shape: BoxShape.circle,
            ),
          ),
          const SizedBox(width: 5),
          Text(
            'Live',
            style: Theme.of(context).textTheme.labelSmall?.copyWith(
                  color: AppColors.success,
                  fontWeight: FontWeight.w700,
                ),
          ),
        ],
      ),
    );
  }
}

/// One store's slice: header, status timeline, items, money, and the
/// cancel affordance while cancelling is still possible.
class _StoreOrderSection extends StatelessWidget {
  const _StoreOrderSection({required this.storeOrder, required this.onCancel});

  final StoreOrder storeOrder;
  final VoidCallback onCancel;

  bool get _cancellable =>
      storeOrder.status == 'placed' ||
      storeOrder.status == 'accepted' ||
      storeOrder.status == 'preparing';

  @override
  Widget build(BuildContext context) {
    final textTheme = Theme.of(context).textTheme;
    return Padding(
      padding: const EdgeInsets.symmetric(horizontal: 20),
      child: Container(
        padding: const EdgeInsets.all(14),
        decoration: BoxDecoration(
          color: AppColors.surfaceAlt,
          borderRadius: BorderRadius.circular(16),
          border: Border.all(color: AppColors.surfaceBorder),
        ),
        child: Column(
          crossAxisAlignment: CrossAxisAlignment.start,
          children: [
            Row(
              children: [
                Expanded(
                  child: Text(
                    storeOrder.storeName,
                    maxLines: 1,
                    overflow: TextOverflow.ellipsis,
                    style: textTheme.titleSmall?.copyWith(
                      fontWeight: FontWeight.w700,
                    ),
                  ),
                ),
                _StoreStatusChip(status: storeOrder.status),
              ],
            ),
            const SizedBox(height: 12),
            _StatusTimeline(status: storeOrder.status),
            const SizedBox(height: 12),
            ...storeOrder.items.map(
              (item) => Padding(
                padding: const EdgeInsets.symmetric(vertical: 4),
                child: Row(
                  children: [
                    Expanded(
                      child: Text(
                        '${item.productName} × ${item.quantity}',
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
            const SizedBox(height: 6),
            const Divider(height: 1, color: AppColors.surfaceBorder),
            const SizedBox(height: 6),
            _MiniTotalsRow(label: 'Subtotal', value: storeOrder.subtotal),
            _MiniTotalsRow(label: 'Delivery', value: storeOrder.deliveryFee),
            _MiniTotalsRow(
              label: 'Total',
              value: storeOrder.total,
              bold: true,
            ),
            if (_cancellable) ...[
              const SizedBox(height: 10),
              SizedBox(
                width: double.infinity,
                child: OutlinedButton.icon(
                  onPressed: onCancel,
                  style: OutlinedButton.styleFrom(
                    foregroundColor: AppColors.error,
                    side: BorderSide(
                      color: AppColors.error.withValues(alpha: 0.4),
                    ),
                    shape: RoundedRectangleBorder(
                      borderRadius: BorderRadius.circular(10),
                    ),
                  ),
                  icon: const Icon(Icons.cancel_outlined, size: 18),
                  label: const Text('Cancel this order'),
                ),
              ),
            ],
          ],
        ),
      ),
    );
  }
}

class _MiniTotalsRow extends StatelessWidget {
  const _MiniTotalsRow({
    required this.label,
    required this.value,
    this.bold = false,
  });

  final String label;
  final int value;
  final bool bold;

  @override
  Widget build(BuildContext context) {
    final textTheme = Theme.of(context).textTheme;
    return Padding(
      padding: const EdgeInsets.symmetric(vertical: 2),
      child: Row(
        mainAxisAlignment: MainAxisAlignment.spaceBetween,
        children: [
          Text(
            label,
            style: textTheme.bodySmall?.copyWith(
              fontWeight: bold ? FontWeight.w700 : FontWeight.w500,
              color: bold ? AppColors.onSurface : AppColors.onSurfaceMuted,
            ),
          ),
          Text(
            formatRwf(value),
            style: textTheme.bodySmall?.copyWith(
              fontWeight: bold ? FontWeight.w700 : FontWeight.w600,
              color: bold ? AppColors.primary : AppColors.onSurface,
            ),
          ),
        ],
      ),
    );
  }
}

/// Store-order status chip — colored pill matching the state.
class _StoreStatusChip extends StatelessWidget {
  const _StoreStatusChip({required this.status});

  final String status;

  Color get _color {
    switch (status) {
      case 'placed':
        return AppColors.primary;
      case 'accepted':
      case 'preparing':
        return const Color(0xFF0EA5E9); // sky blue
      case 'picked_up':
        return const Color(0xFF8B5CF6); // purple
      case 'delivered':
        return AppColors.success;
      case 'cancelled':
        return AppColors.error;
      default:
        return AppColors.onSurfaceMuted;
    }
  }

  String get _label {
    if (status.isEmpty) return 'Placed';
    return status
        .split('_')
        .map((w) => w.isEmpty ? '' : w[0].toUpperCase() + w.substring(1))
        .join(' ');
  }

  @override
  Widget build(BuildContext context) {
    return Container(
      padding: const EdgeInsets.symmetric(horizontal: 8, vertical: 4),
      decoration: BoxDecoration(
        color: _color.withValues(alpha: 0.12),
        borderRadius: BorderRadius.circular(999),
      ),
      child: Text(
        _label,
        style: Theme.of(context).textTheme.labelSmall?.copyWith(
              color: _color,
              fontWeight: FontWeight.w700,
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
      padding: const EdgeInsets.symmetric(horizontal: 20),
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
      padding: EdgeInsets.symmetric(vertical: 6),
      child: Divider(
        height: 1,
        color: AppColors.surfaceBorder,
      ),
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
      padding: const EdgeInsets.symmetric(vertical: 3),
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

/// Six-step vertical timeline showing a store order's progression.
/// Completed steps are teal; the current step is gold; future muted;
/// cancelled renders as a red terminal state.
class _StatusTimeline extends StatelessWidget {
  const _StatusTimeline({required this.status});

  final String status;

  static const _steps = [
    _Step('placed', 'Placed'),
    _Step('accepted', 'Accepted'),
    _Step('preparing', 'Preparing'),
    _Step('picked_up', 'Picked up'),
    _Step('delivered', 'Delivered'),
  ];

  @override
  Widget build(BuildContext context) {
    final currentIndex = _steps.indexWhere((s) => s.key == status);
    final textTheme = Theme.of(context).textTheme;
    final cancelled = status == 'cancelled';

    final visibleSteps = cancelled
        ? const [_Step('placed', 'Placed'), _Step('cancelled', 'Cancelled')]
        : _steps;
    final visibleIndex =
        cancelled ? 1 : currentIndex;

    return Column(
      crossAxisAlignment: CrossAxisAlignment.start,
      children: List.generate(visibleSteps.length, (i) {
        final step = visibleSteps[i];
        final completed = !cancelled && i < visibleIndex;
        final current = !cancelled && i == visibleIndex;
        final isCancelledStep = step.key == 'cancelled';
        final color = isCancelledStep
            ? AppColors.error
            : completed
                ? AppColors.success
                : current
                    ? AppColors.primary
                    : AppColors.onSurfaceMuted;
        final icon = isCancelledStep
            ? Icons.cancel_rounded
            : completed
                ? Icons.check_rounded
                : current
                    ? Icons.schedule_rounded
                    : Icons.circle_outlined;

        return Padding(
          padding: const EdgeInsets.symmetric(vertical: 4),
          child: Row(
            crossAxisAlignment: CrossAxisAlignment.start,
            children: [
              SizedBox(
                width: 24,
                child: Icon(icon, size: 18, color: color),
              ),
              const SizedBox(width: 4),
              // Vertical line connector (except last)
              if (i < visibleSteps.length - 1)
                Container(
                  width: 2,
                  height: 26,
                  margin: const EdgeInsets.only(left: 11, top: 20),
                  color: completed
                      ? AppColors.success.withValues(alpha: 0.4)
                      : AppColors.surfaceBorder,
                ),
              Expanded(
                child: Text(
                  step.label,
                  style: textTheme.bodyMedium?.copyWith(
                    fontWeight: current || isCancelledStep
                        ? FontWeight.w700
                        : FontWeight.w500,
                    color: color,
                  ),
                ),
              ),
            ],
          ),
        );
      }),
    );
  }
}

class _Step {
  const _Step(this.key, this.label);
  final String key;
  final String label;
}
