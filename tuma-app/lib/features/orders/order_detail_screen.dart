import 'dart:async';

import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:go_router/go_router.dart';
import 'package:url_launcher/url_launcher.dart';

import 'package:tuma_app/core/api/api_client.dart';
import 'package:tuma_app/core/api/models/order.dart';
import 'package:tuma_app/core/api/models/tracking.dart';
import 'package:tuma_app/core/auth/auth_controller.dart';
import 'package:tuma_app/core/theme/app_colors.dart';
import 'package:tuma_app/core/theme/app_theme.dart';
import 'package:tuma_app/features/tracking/delivery_map.dart';
import 'package:tuma_app/shared/widgets/design_system.dart';
import 'package:tuma_app/core/utils/format_rwf.dart';
import 'package:tuma_app/shared/widgets/error_state.dart';
import 'package:tuma_app/shared/widgets/store_contact_sheet.dart';

/// Order group detail — the one purchase the customer placed, rendered as
/// one section per fulfilling store. Each store has its own status
/// timeline and can be cancelled independently while it's still on the
/// premises. While anything is still in flight the screen polls the
/// tracking endpoint every few seconds, echoing `changed_at` back as
/// `since`: the server answers 204 when nothing moved and the screen does
/// nothing at all — no rebuild, no work (battery is the contract). When a
/// snapshot does land, its statuses merge over the group detail (the
/// header chip and payment line follow `group_status`; each store section
/// follows its own delivery status), and a picked-up delivery's section
/// becomes the map world. Polling stops the moment the group settles.
class OrderDetailScreen extends ConsumerStatefulWidget {
  const OrderDetailScreen({super.key, required this.orderId});

  final String orderId;

  @override
  ConsumerState<OrderDetailScreen> createState() => _OrderDetailScreenState();
}

/// How often the in-flight group re-fetches while everything is Live.
/// The truth ladder decays this (15s lagging, 60s ended) — see
/// [_OrderDetailScreenState._pollInterval].
const _livePollInterval = Duration(seconds: 5);

/// The statuses a customer can still cancel from (P10: cancel
/// disappears once the order is picked up).
const _cancellableStatuses = {'placed', 'accepted', 'preparing'};

class _OrderDetailScreenState extends ConsumerState<OrderDetailScreen>
    with WidgetsBindingObserver {
  OrderGroup? _order;
  GroupTracking? _tracking;
  String? _error;
  Timer? _pollTimer;

  /// The freshest group status: tracking's when a snapshot has landed,
  /// the group detail's before that.
  String get _effectiveGroupStatus {
    final order = _order;
    if (order == null) return '';
    return _tracking?.groupStatus ?? order.status;
  }

  /// Effective payment state — cash flips to collected on delivery, and
  /// the receipt line must show that without a full refetch.
  String get _effectivePaymentStatus {
    final order = _order;
    if (order == null) return '';
    return _tracking?.paymentStatus ?? order.paymentStatus;
  }

  bool get _effectiveInFlight =>
      _effectiveGroupStatus == 'in_progress' ||
      _effectiveGroupStatus == 'partially_fulfilled';

  /// The first-load path: everything resets to a spinner. Pull-to-refresh
  /// takes [_silentRefresh] — a failed refresh keeps the order on screen
  /// (flashing back to a spinner, or worse wiping loaded content to an
  /// error state, is jank, not honesty).
  Future<void> _load() async {
    setState(() {
      _error = null;
      _order = null;
      _tracking = null;
    });
    await _fetch();
  }

  Future<void> _silentRefresh() async {
    try {
      final api = ref.read(orderApiProvider);
      final order = await api.getGroup(widget.orderId);
      if (!mounted) return;
      setState(() => _order = order);
      _syncPolling();
    } on Object {
      // The rendered order stays; the next pull retries.
    }
  }

  Future<void> _fetch() async {
    try {
      final api = ref.read(orderApiProvider);
      final order = await api.getGroup(widget.orderId);
      if (!mounted) return;
      setState(() => _order = order);
      // The tracking snapshot is additive: the detail renders with the
      // group's own statuses if it fails, and the poll retries.
      try {
        final tracking = await api.trackGroup(widget.orderId);
        if (!mounted || tracking == null) return;
        setState(() => _tracking = tracking);
      } on Object {
        // Tracking is optional at first paint; the poll recovers it.
      }
      _syncPolling();
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
  /// not a stream. The cadence follows the truth ladder (tracking doc
  /// §2): 5s while Live, 15s once lagging, 60s deep past the ETA — a
  /// restart only happens when the honest interval actually changed.
  void _syncPolling() {
    if (!_effectiveInFlight) {
      _pollTimer?.cancel();
      _pollTimer = null;
      _activeInterval = null;
      return;
    }
    final interval = _pollInterval;
    if (_pollTimer == null) {
      _activeInterval = interval;
      _pollTimer = Timer.periodic(interval, (_) => unawaited(_refresh()));
    } else if (_activeInterval != interval) {
      _pollTimer!.cancel();
      _activeInterval = interval;
      _pollTimer = Timer.periodic(interval, (_) => unawaited(_refresh()));
    }
  }

  /// The interval currently programmed into the timer (null = none).
  Duration? _activeInterval;

  /// The ladder's poll cadence: the slowest-moving delivery on the group
  /// decides — any lagging delivery drops the group to 15s, any ended
  /// delivery to 60s. Fresh-and-on-time keeps the 5s live poll.
  Duration get _pollInterval {
    final tracking = _tracking;
    if (tracking == null) return _livePollInterval;
    var interval = _livePollInterval;
    for (final delivery in tracking.deliveries) {
      if (delivery.status != 'picked_up') continue;
      final overdue = delivery.overdueBy;
      final age = delivery.signalAgeMinutes;
      if (overdue != null && overdue >= const Duration(hours: 24)) {
        return const Duration(minutes: 1);
      }
      if (age != null && age >= 5 ||
          overdue != null && overdue >= const Duration(minutes: 15)) {
        interval = const Duration(seconds: 15);
      }
    }
    return interval;
  }

  /// A quiet re-fetch: echo the last `changed_at` as `since`. A 204 means
  /// nothing moved — the screen does nothing at all (the battery win is
  /// the point). A snapshot merges its statuses over the detail; items and
  /// totals never change after placement, so the group detail is fetched
  /// once and only statuses are tracked.
  Future<void> _refresh() async {
    try {
      final tracking = await ref
          .read(orderApiProvider)
          .trackGroup(widget.orderId, since: _tracking?.changedAt);
      if (!mounted) return;
      if (tracking == null) return; // 204 — nothing moved, no rebuild.
      setState(() => _tracking = tracking);
      _syncPolling();
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
      // A cancel changed real state: refetch the detail (items don't
      // change, but the group may have settled) and freshen tracking.
      final api = ref.read(orderApiProvider);
      final fresh = await api.getGroup(widget.orderId);
      if (!mounted) return;
      setState(() {
        _order = fresh;
        _tracking = null;
      });
      try {
        final tracking = await api.trackGroup(widget.orderId);
        if (!mounted || tracking == null) return;
        setState(() => _tracking = tracking);
      } on Object {
        // The poll recovers it.
      }
      _syncPolling();
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
      if (_effectiveInFlight) {
        unawaited(_refresh());
        _syncPolling();
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
          'Track order #${order.number}',
          style: const TextStyle(fontSize: 16, fontWeight: FontWeight.w600),
          overflow: TextOverflow.ellipsis,
        ),
      ),
      body: RefreshIndicator(
        onRefresh: _silentRefresh,
        color: AppColors.primary,
        child: CustomScrollView(
          slivers: [
            // The HERO STATUS (P11): one story narrating the lifecycle —
            // icon, hero words, the promise subline. One signal at a time.
            SliverToBoxAdapter(
              child: Padding(
                padding: const EdgeInsets.fromLTRB(16, 8, 16, 0),
                child: Row(
                  crossAxisAlignment: CrossAxisAlignment.start,
                  children: [
                    const Icon(
                      Icons.two_wheeler_rounded,
                      size: 26,
                      color: AppColors.primary,
                    ),
                    const SizedBox(width: 10),
                    Expanded(
                      child: Column(
                        crossAxisAlignment: CrossAxisAlignment.start,
                        children: [
                          Text(_heroTitle(), style: AppTheme.d2(textTheme)),
                          const SizedBox(height: 2),
                          Text(
                            _heroSubline(),
                            style: AppTheme.bd(textTheme)
                                .copyWith(color: AppColors.onSurfaceMuted),
                          ),
                        ],
                      ),
                    ),
                  ],
                ),
              ),
            ),
            // The ladder's warning line: amber on lagging, the store
            // call when it's genuinely late (P14: delay is amber).
            ..._ladderWarningSliver(),
            // The stepper — position in the story, labeled.
            SliverToBoxAdapter(
              child: Padding(
                padding: const EdgeInsets.fromLTRB(16, 14, 16, 0),
                child: OrderStepper.lifecycle(
                  index: _stepperIndex(),
                  completed: _effectiveGroupStatus == 'completed',
                ),
              ),
            ),
            // ONE DELIVERY SECTION PER STORE — each store's own status,
            // rider, and (while moving) its own map. A different driver
            // per store means each driver gets their own story.
            ..._deliverySectionSlivers(order),
            // The DELIVER-TO row with the actions beside it (P10: cancel
            // disappears once picked up — it's no longer reversible).
            SliverToBoxAdapter(
              child: Padding(
                padding: const EdgeInsets.fromLTRB(16, 10, 16, 0),
                child: Container(
                  padding:
                      const EdgeInsets.symmetric(horizontal: 14, vertical: 12),
                  decoration: BoxDecoration(
                    color: AppColors.surfaceAlt,
                    borderRadius: BorderRadius.circular(16),
                    border: Border.all(color: AppColors.surfaceBorder),
                  ),
                  child: Row(
                    children: [
                      const Icon(Icons.location_on_rounded,
                          size: 20, color: AppColors.primary),
                      const SizedBox(width: 12),
                      Expanded(
                        child: Text(
                          order.addressText,
                          maxLines: 2,
                          overflow: TextOverflow.ellipsis,
                          style: AppTheme.bd(textTheme)
                              .copyWith(fontWeight: FontWeight.w600),
                        ),
                      ),
                    ],
                  ),
                ),
              ),
            ),
            // The ORDER SUMMARY — collapsed behind a micro-label with an
            // expand affordance (the receipt, one place, P1).
            SliverToBoxAdapter(
              child: Padding(
                padding: const EdgeInsets.fromLTRB(16, 10, 16, 0),
                child: _CollapsibleSummary(
                  order: order,
                  effectivePaymentStatus: _effectivePaymentStatus,
                ),
              ),
            ),
            // ACTIONS per status (P10, P14): cancel only while reversible;
            // Reorder as the settled primary.
            SliverToBoxAdapter(
              child: Padding(
                padding: const EdgeInsets.fromLTRB(16, 12, 16, 0),
                child: _actionsRow(order),
              ),
            ),
            const SliverToBoxAdapter(child: SizedBox(height: 24)),
          ],
        ),
      ),
    );
  }

  /// The hero words, from the effective statuses (P11: story, not
  /// sticker). One fact: position in the lifecycle + the promise.
  /// The ladder's rung for the moving delivery (tracking doc §2): fresh
  /// signal + not past ETA → live; stale signal or past the 15-min grace
  /// → lagging; ~24h past ETA → ended.
  ({String rung, DateTime? soonestEta, bool stale}) _ladder() {
    const endedAfter = Duration(hours: 24);
    const grace = Duration(minutes: 15);
    final deliveries = _tracking?.deliveries ?? const <DeliveryTracking>[];
    final moving = deliveries.where((d) => d.status == 'picked_up');
    var stale = false;
    DateTime? soonest;
    for (final d in moving) {
      final age = d.signalAgeMinutes;
      if (age != null && age >= 5) stale = true;
      final eta = d.etaTarget;
      if (eta != null && (soonest == null || eta.isBefore(soonest))) {
        soonest = eta;
      }
    }
    final overdue = soonest == null
        ? null
        : DateTime.now().difference(soonest);
    if (overdue != null && overdue >= endedAfter) {
      return (rung: 'ended', soonestEta: soonest, stale: stale);
    }
    if (stale || (overdue != null && overdue >= grace)) {
      return (rung: 'lagging', soonestEta: soonest, stale: stale);
    }
    return (rung: 'live', soonestEta: soonest, stale: stale);
  }

  String _heroTitle() {
    if (_effectiveGroupStatus == 'cancelled') return 'Cancelled';
    if (_effectiveGroupStatus == 'completed') return 'Delivered';
    final deliveries = _tracking?.deliveries ?? const <DeliveryTracking>[];
    final anyMoving = deliveries.any((d) => d.status == 'picked_up');
    if (anyMoving) {
      if (_ladder().rung == 'ended') {
        return 'Taking much longer than expected';
      }
      if (_ladder().rung == 'lagging') return 'Running late';
      return 'Out for delivery';
    }
    return 'Preparing your order';
  }

  String _heroSubline() {
    if (_effectiveGroupStatus == 'cancelled') {
      return 'This order was cancelled — nothing was charged.';
    }
    if (_effectiveGroupStatus == 'completed') {
      final at = _tracking?.deliveries
          .map((d) => d.updatedAt)
          .fold<DateTime?>(null, (max, t) => max == null || t.isAfter(max) ? t : max);
      final when = at != null ? 'Today at ${_clockTime(at)}' : 'Today';
      return '$when · ${_order?.addressText ?? ''}';
    }
    // The ladder's promise (P11: a delay warning ALWAYS pairs with the
    // revised ETA — even when the revised answer is "now").
    final etas = _tracking?.deliveries
        .map((d) => d.etaTarget)
        .whereType<DateTime>()
        .toList();
    if (etas != null && etas.isNotEmpty) {
      final soonest = etas.reduce((a, b) => a.isBefore(b) ? a : b);
      final remaining = soonest.difference(DateTime.now());
      if (_ladder().rung == 'ended') {
        return 'This needs a human — call the store below.';
      }
      if (remaining.isNegative) return 'Now arriving';
      if (_ladder().rung == 'lagging') return 'Now arriving ~${remaining.inMinutes + (remaining.inSeconds > 0 ? 1 : 0)} min';
      final minutes = remaining.inMinutes + (remaining.inSeconds > 0 ? 1 : 0);
      return 'Arriving in about $minutes min';
    }
    return 'Cash on delivery — ${formatRwf(_order?.grandTotal ?? 0)} when it arrives';
  }

  int _stepperIndex() {
    final deliveries = _tracking?.deliveries ?? const [];
    // Delivered wins over picked_up: a completed stop must land on the
    // last dot, not the "On the way" one (the checks are ordered by the
    // ladder's progress, not by whichever status happens to appear).
    if (deliveries.any((d) => d.status == 'delivered')) return 4;
    if (deliveries.any((d) => d.status == 'picked_up')) return 3;
    if (deliveries.any((d) => d.status == 'preparing')) return 1;
    return 0;
  }

  String _clockTime(DateTime time) {
    final local = time.toLocal();
    final hour = local.hour.toString().padLeft(2, '0');
    final minute = local.minute.toString().padLeft(2, '0');
    return '$hour:$minute';
  }

  /// The lagging/ended warning: amber dot row + the store-call escape.
  List<Widget> _ladderWarningSliver() {
    if (_effectiveGroupStatus != 'in_progress' &&
        _effectiveGroupStatus != 'partially_fulfilled') {
      return const [];
    }
    final ladder = _ladder();
    if (ladder.rung == 'live') return const [];
    final phone = _tracking?.deliveries
        .map((d) => d.storeContactPhone)
        .whereType<String>()
        .firstWhere((_) => true, orElse: () => '');
    return [
      SliverToBoxAdapter(
        child: Padding(
          padding: const EdgeInsets.fromLTRB(16, 8, 16, 0),
          child: Row(
            children: [
              // Flexible: on a tiny screen the long "Ended" label and the
              // call action must share, not collide (neither can shrink
              // in a bare Row).
              Flexible(
                child: StatusRow(
                  color: AppColors.warning,
                  label: ladder.rung == 'ended'
                      ? 'Ended · contact the store'
                      : 'Running late',
                  pulsing: false,
                ),
              ),
              const Spacer(),
              if (phone!.isNotEmpty)
                GestureDetector(
                  onTap: () =>
                      unawaited(launchUrl(Uri(scheme: 'tel', path: phone))),
                  child: Text(
                    'Call store',
                    style: Theme.of(context).textTheme.titleSmall?.copyWith(
                          fontSize: 14,
                          color: AppColors.primary,
                          fontWeight: FontWeight.w600,
                        ),
                  ),
                ),
            ],
          ),
        ),
      ),
    ];
  }

  /// One DELIVERY SECTION per store order (the multi-store rule: one
  /// rider per store, so one story per store). Each section carries the
  /// store's own status, its rider card, and — while that store's
  /// delivery is moving and the order has a pin — its OWN map. A
  /// two-store order with two different riders shows both, each with
  /// their map, in fulfillment order.
  List<Widget> _deliverySectionSlivers(OrderGroup order) {
    final tracking = _tracking;
    final hasPin = order.addressLat != null && order.addressLng != null;
    return [
      for (final storeOrder in order.storeOrders)
        _StoreDeliverySection(
          storeOrder: storeOrder,
          tracking: tracking?.forStoreOrder(storeOrder.id),
          orderNumber: order.number,
          hasPin: hasPin,
          destinationLat: order.addressLat,
          destinationLng: order.addressLng,
          cancellable: _cancellableStatuses.contains(storeOrder.status),
          onCancel: () => unawaited(_cancelStoreOrder(storeOrder)),
        ),
    ];
  }

  /// The actions row: what this status allows (P10).
  Widget _actionsRow(OrderGroup order) {
    // Cancel lives INSIDE each store's delivery section now (per-store
    // cancel — the second store's order is reachable too). This row is
    // the whole-purchase actions: reach the store always, reorder when
    // done. Get help NEVER disappears — delivered orders still need the
    // store's contact.
    final settled =
        _effectiveGroupStatus == 'completed' || _effectiveGroupStatus == 'cancelled';
    return Column(
      children: [
        if (settled) ...[
          SizedBox(
            width: double.infinity,
            child: FilledButton.icon(
              onPressed: () => context.go('/home'),
              icon: const Icon(Icons.replay_rounded, size: 20),
              label: const Text(
                'Reorder',
                style: TextStyle(fontSize: 15, fontWeight: FontWeight.w600),
              ),
            ),
          ),
          const SizedBox(height: 10),
        ],
        OutlinedButton(
          onPressed: () => unawaited(_showContactSheet(order)),
          child: const Text('Get help'),
        ),
      ],
    );
  }

  /// Get help = reach the store directly: one contact card per
  /// participating store, with the Call/Email actions (the founder's
  /// replacement for the old "accounts era" placeholder).
  Future<void> _showContactSheet(OrderGroup order) async {
    final tracking = _tracking;
    final entries = order.storeOrders.map((storeOrder) {
      // Tracking's contact phone is the richer source when present; the
      // detail's own fields carry the rest (and cover cancelled orders,
      // which never reach tracking).
      final phone =
          tracking?.forStoreOrder(storeOrder.id)?.storeContactPhone ??
              storeOrder.storeContactPhone;
      return StoreContactEntry(
        name: storeOrder.storeName,
        address: order.addressText,
        phone: phone,
        email: storeOrder.storeContactEmail,
      );
    }).toList();
    await showStoreContactSheet(
      context,
      entries: entries,
      title: 'Order #${order.number} — contact',
    );
  }
}



/// The rider card: avatar, one status line that always tells the truth
/// about where the rider is, the vehicle line, and the optional
/// call-the-store circle (the store phone rides on the tracking entry).
class _RiderCard extends StatelessWidget {
  const _RiderCard({
    required this.avatarText,
    required this.title,
    this.subtitle,
    this.phone,
  });

  final String avatarText;
  final String title;
  final String? subtitle;
  final String? phone;

  @override
  Widget build(BuildContext context) {
    return Container(
      padding: const EdgeInsets.symmetric(horizontal: 14, vertical: 12),
      decoration: BoxDecoration(
        color: AppColors.surfaceAlt,
        borderRadius: BorderRadius.circular(16),
        border: Border.all(color: AppColors.surfaceBorder),
      ),
      child: Row(
        children: [
          AccentAvatar(text: avatarText, size: 44),
          const SizedBox(width: 12),
          Expanded(
            child: Column(
              crossAxisAlignment: CrossAxisAlignment.start,
              children: [
                Text(title,
                    style: AppTheme.bd(Theme.of(context).textTheme)
                        .copyWith(fontWeight: FontWeight.w600)),
                if (subtitle != null && subtitle!.isNotEmpty)
                  Text(subtitle!,
                      style: AppTheme.sub(Theme.of(context).textTheme)),
              ],
            ),
          ),
          if (phone != null)
            GestureDetector(
              onTap: () => unawaited(
                launchUrl(Uri(scheme: 'tel', path: phone!)),
              ),
              child: Container(
                width: 40,
                height: 40,
                decoration: BoxDecoration(
                  shape: BoxShape.circle,
                  border: Border.all(
                    color: AppColors.primary.withValues(alpha: 0.4),
                  ),
                ),
                child: const Icon(Icons.call_rounded,
                    size: 19, color: AppColors.primary),
              ),
            ),
        ],
      ),
    );
  }
}

/// The collapsed receipt (P1: money in one place; the sheet keeps the
/// hero focused). Tap the label row to expand.
class _CollapsibleSummary extends StatefulWidget {
  const _CollapsibleSummary({
    required this.order,
    required this.effectivePaymentStatus,
  });

  final OrderGroup order;

  /// Tracking's payment status when a snapshot landed, the group
  /// detail's otherwise — cash flips to collected on delivery and the
  /// receipt must show that without a refetch.
  final String effectivePaymentStatus;

  @override
  State<_CollapsibleSummary> createState() => _CollapsibleSummaryState();
}

class _CollapsibleSummaryState extends State<_CollapsibleSummary> {
  bool _expanded = false;

  @override
  Widget build(BuildContext context) {
    final order = widget.order;
    final textTheme = Theme.of(context).textTheme;
    return Container(
      padding: const EdgeInsets.all(14),
      decoration: BoxDecoration(
        color: AppColors.surfaceAlt,
        borderRadius: BorderRadius.circular(16),
        border: Border.all(color: AppColors.surfaceBorder),
      ),
      child: Column(
        children: [
          InkWell(
            onTap: () => setState(() => _expanded = !_expanded),
            borderRadius: BorderRadius.circular(8),
            child: Row(
              children: [
                const Expanded(child: MicroLabel('Order summary')),
                AnimatedRotation(
                  turns: _expanded ? 0.5 : 0,
                  duration: const Duration(milliseconds: 200),
                  child: const Icon(
                    Icons.expand_more_rounded,
                    size: 18,
                    color: AppColors.onSurfaceMuted,
                  ),
                ),
              ],
            ),
          ),
          if (_expanded) ...[
            const SizedBox(height: 9),
            for (final storeOrder in order.storeOrders)
              for (final item in storeOrder.items)
                Padding(
                  padding: const EdgeInsets.only(bottom: 5),
                  child: Row(
                    children: [
                      Expanded(
                        child: Text(
                          '${item.productName} × ${item.quantity}',
                          style: AppTheme.bd(textTheme)
                              .copyWith(color: AppColors.onSurfaceMuted),
                          maxLines: 1,
                          overflow: TextOverflow.ellipsis,
                        ),
                      ),
                      Text(formatRwf(item.lineTotal),
                          style: AppTheme.bd(textTheme)),
                    ],
                  ),
                ),
            Padding(
              padding: const EdgeInsets.only(bottom: 5),
              child: Row(
                mainAxisAlignment: MainAxisAlignment.spaceBetween,
                children: [
                  Text('Delivery fee',
                      style: AppTheme.bd(textTheme)
                          .copyWith(color: AppColors.onSurfaceMuted)),
                  Text(formatRwf(order.deliveryTotal),
                      style: AppTheme.bd(textTheme)),
                ],
              ),
            ),
            const Divider(color: AppColors.surfaceBorder),
          ],
          Row(
            mainAxisAlignment: MainAxisAlignment.spaceBetween,
            children: [
              Expanded(
                child: Text(
                  widget.effectivePaymentStatus == 'pending'
                      ? 'Total · pay cash on delivery'
                      : 'Total · paid',
                  style: AppTheme.bd(textTheme).copyWith(
                    fontWeight: FontWeight.w600,
                  ),
                ),
              ),
              Text(
                formatRwf(order.grandTotal),
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
    );
  }
}

/// One store's delivery section — the multi-store rule made visible: one
/// rider per store, so one story per store. The header carries the
/// store's name, its total, and its own status dot; a moving delivery
/// with a pin gets ITS OWN map (a different driver per store means each
/// driver shows on their own map); the rider card tells the truth about
/// that rider; the closeness line ("your driver is close") comes from the
/// server's straight-line meters; cancel lives here while reversible.
class _StoreDeliverySection extends StatelessWidget {
  const _StoreDeliverySection({
    required this.storeOrder,
    required this.tracking,
    required this.orderNumber,
    required this.hasPin,
    required this.destinationLat,
    required this.destinationLng,
    required this.cancellable,
    required this.onCancel,
  });

  final StoreOrder storeOrder;
  final DeliveryTracking? tracking;
  final int orderNumber;
  final bool hasPin;
  final double? destinationLat;
  final double? destinationLng;
  final bool cancellable;
  final VoidCallback onCancel;

  /// The store-order's own status story — same shape as the group one
  /// but scoped to THIS store's delivery.
  ({Color color, String label, bool pulsing}) get _status {
    final status = tracking?.status ?? storeOrder.status;
    return switch (status) {
      'picked_up' => (
        color: AppColors.success,
        label: 'On the way',
        pulsing: true,
      ),
      'delivered' => (
        color: AppColors.success,
        label: 'Delivered',
        pulsing: false,
      ),
      'cancelled' => (
        color: AppColors.error,
        label: 'Cancelled',
        pulsing: false,
      ),
      'preparing' => (
        color: AppColors.primary,
        label: 'Preparing',
        pulsing: false,
      ),
      'accepted' => (
        color: AppColors.primary,
        label: 'Accepted',
        pulsing: false,
      ),
      _ => (
        color: AppColors.primary,
        label: 'Placed',
        pulsing: false,
      ),
    };
  }

  @override
  Widget build(BuildContext context) {
    final textTheme = Theme.of(context).textTheme;
    final delivery = tracking;
    final moving = delivery?.status == 'picked_up';
    final riderName = (delivery?.riderName?.isNotEmpty ?? false)
        ? delivery!.riderName!
        : null;

    return SliverToBoxAdapter(
      child: Padding(
        padding: const EdgeInsets.fromLTRB(16, 14, 16, 0),
        child: Container(
          decoration: BoxDecoration(
            color: AppColors.surfaceAlt,
            borderRadius: BorderRadius.circular(16),
            border: Border.all(color: AppColors.surfaceBorder),
          ),
          child: Column(
            crossAxisAlignment: CrossAxisAlignment.start,
            children: [
              // The header: who is fulfilling this slice, for how much,
              // and where it stands.
              Padding(
                padding: const EdgeInsets.fromLTRB(14, 12, 14, 0),
                child: Row(
                  children: [
                    Expanded(
                      child: Text(
                        storeOrder.storeName,
                        maxLines: 1,
                        overflow: TextOverflow.ellipsis,
                        style: textTheme.titleSmall?.copyWith(
                          fontSize: 14.5,
                          fontWeight: FontWeight.w700,
                        ),
                      ),
                    ),
                    const SizedBox(width: 10),
                    Text(
                      formatRwf(storeOrder.total),
                      style: textTheme.titleSmall?.copyWith(
                        fontSize: 14,
                        color: AppColors.primary,
                        fontWeight: FontWeight.w700,
                      ),
                    ),
                  ],
                ),
              ),
              Padding(
                padding: const EdgeInsets.fromLTRB(14, 5, 14, 0),
                child: StatusRow(
                  color: _status.color,
                  label: _status.label,
                  pulsing: _status.pulsing,
                ),
              ),
              // THE MAP — this store's own delivery, its own map while
              // moving. No pin → the quiet nothing (P2), with the reason
              // said out loud only while the story is alive.
              if (moving) ...[
                if (hasPin && destinationLat != null && destinationLng != null)
                  Padding(
                    padding: const EdgeInsets.fromLTRB(14, 10, 14, 0),
                    child: DeliveryMap(
                      tracking: delivery!,
                      destinationLat: destinationLat,
                      destinationLng: destinationLng,
                    ),
                  )
                else
                  Padding(
                    padding: const EdgeInsets.fromLTRB(14, 10, 14, 0),
                    child: Text(
                      'Your delivery is on the way — add a delivery location next order to watch it move.',
                      style: AppTheme.sub(textTheme),
                    ),
                  ),
                // The closeness line: the server's straight-line meters,
                // never client math. "Delivery started" until it's close.
                Padding(
                  padding: const EdgeInsets.fromLTRB(14, 8, 14, 0),
                  child: Row(
                    children: [
                      Icon(
                        (delivery?.riderDistanceM ?? 1 << 62) <= 500
                            ? Icons.near_me_rounded
                            : Icons.local_shipping_rounded,
                        size: 15,
                        color: AppColors.success,
                      ),
                      const SizedBox(width: 6),
                      Expanded(
                        child: Text(
                          _closenessLine(),
                          style: AppTheme.sub(textTheme)
                              .copyWith(color: AppColors.success),
                        ),
                      ),
                    ],
                  ),
                ),
              ],
              // The rider card — THIS store's rider only.
              if (riderName != null)
                Padding(
                  padding: const EdgeInsets.fromLTRB(14, 10, 14, 0),
                  child: _RiderCard(
                    avatarText: riderName,
                    title: delivery!.status == 'delivered'
                        ? 'Delivered · ${_clockTimeOf(delivery)} by ${riderName.split(' ').first}'
                        : '${riderName.split(' ').first} is on the way',
                    subtitle: [
                      ?delivery.riderVehicle,
                      ?delivery.riderPlate,
                    ].join(' · '),
                    phone: delivery.storeContactPhone,
                  ),
                )
              else if (moving || storeOrder.status == 'placed' || storeOrder.status == 'accepted' || storeOrder.status == 'preparing')
                Padding(
                  padding: const EdgeInsets.fromLTRB(14, 10, 14, 0),
                  child: Row(
                    children: [
                      const Icon(Icons.person_outline_rounded,
                          size: 18, color: AppColors.onSurfaceMuted),
                      const SizedBox(width: 10),
                      Expanded(
                        child: Text(
                          'A rider will be assigned when the store hands your order over.',
                          style: AppTheme.sub(textTheme),
                        ),
                      ),
                    ],
                  ),
                ),
              // Cancel — per store, while the store's order is reversible.
              if (cancellable)
                Padding(
                  padding: const EdgeInsets.fromLTRB(14, 12, 14, 12),
                  child: SizedBox(
                    width: double.infinity,
                    child: OutlinedButton(
                      onPressed: onCancel,
                      style: OutlinedButton.styleFrom(
                        foregroundColor: AppColors.error,
                        side: BorderSide(
                          color: AppColors.error.withValues(alpha: 0.35),
                        ),
                      ),
                      child: Text(
                        'Cancel ${storeOrder.storeName.split(' ').first} order',
                        overflow: TextOverflow.ellipsis,
                      ),
                    ),
                  ),
                )
              else
                const SizedBox(height: 12),
            ],
          ),
        ),
      ),
    );
  }

  String _closenessLine() {
    final meters = tracking?.riderDistanceM;
    if (meters == null) return 'Delivery started — you will see the rider move once their phone checks in.';
    if (meters <= 500) return 'Your driver is close — about $meters m away.';
    return 'Delivery started — about ${(meters / 1000).toStringAsFixed(1)} km away.';
  }

  String _clockTimeOf(DeliveryTracking delivery) {
    final local = delivery.updatedAt.toLocal();
    final hour = local.hour.toString().padLeft(2, '0');
    final minute = local.minute.toString().padLeft(2, '0');
    return '$hour:$minute';
  }
}
