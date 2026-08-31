import 'dart:async';

import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:url_launcher/url_launcher.dart';
import 'package:wakelock_plus/wakelock_plus.dart';

import 'package:tuma_app/core/api/api_client.dart';
import 'package:tuma_app/core/api/models/authenticated_user.dart';
import 'package:tuma_app/core/api/models/rider.dart';
import 'package:tuma_app/core/auth/auth_controller.dart';
import 'package:tuma_app/core/theme/app_colors.dart';
import 'package:tuma_app/features/location/customer_location.dart';
import 'package:tuma_app/features/rider/rider_map.dart';
import 'package:tuma_app/shared/widgets/initials_tile.dart';

/// The delivery kiosk (tracking doc §5): the rider's whole working day on
/// one screen. A store hands an order over by typing the rider number —
/// the job appears here with its destination, the cached road route, and
/// the customer's `tel:` link. Start delivering turns the screen-on
/// wakelock on and pushes ONE real GPS fix every 5 seconds to EVERY
/// active delivery (the rider is in one place; each delivery gets the
/// same real breadcrumb). Delivered is the handover — food given, cash
/// received, one real event that settles the money. No background
/// service: pushing runs only while the screen is on and the app is
/// foregrounded, exactly like the doc says.
class RiderScreen extends ConsumerStatefulWidget {
  const RiderScreen({super.key});

  @override
  ConsumerState<RiderScreen> createState() => _RiderScreenState();
}

/// How often the kiosk pushes a breadcrumb while delivering.
const _pushInterval = Duration(seconds: 5);

/// How often the work list refreshes (new handoffs, stray re-routes).
const _listInterval = Duration(seconds: 15);

class _RiderScreenState extends ConsumerState<RiderScreen>
    with WidgetsBindingObserver {
  List<RiderDelivery>? _deliveries;
  String? _loadError;
  bool _delivering = false;
  bool _starting = false;
  DateTime? _lastPushAt;
  bool _pushError = false;
  double? _riderLat;
  double? _riderLng;
  String? _finishingId;
  Timer? _pushTimer;
  Timer? _listTimer;

  /// The bike-mount wakelock, best-effort: a platform-channel failure
  /// (desktop dev, test binding) must never break the run itself — the
  /// screen just won't stay on.
  Future<void> _keepScreenOn(bool on) async {
    try {
      if (on) {
        await WakelockPlus.enable();
      } else {
        await WakelockPlus.disable();
      }
    } on Object {
      // The push loop is the feature; the wakelock is comfort.
    }
  }

  @override
  void initState() {
    super.initState();
    WidgetsBinding.instance.addObserver(this);
    unawaited(_refreshList());
    _listTimer = Timer.periodic(_listInterval, (_) => unawaited(_refreshList()));
  }

  @override
  void dispose() {
    WidgetsBinding.instance.removeObserver(this);
    _pushTimer?.cancel();
    _listTimer?.cancel();
    if (_delivering) unawaited(_keepScreenOn(false));
    super.dispose();
  }

  @override
  void didChangeAppLifecycleState(AppLifecycleState state) {
    // Foreground-only pushing: backgrounded, the kiosk stops the loop and
    // the wakelock (the screen is off anyway); resumed, delivering picks
    // up where it was.
    if (state == AppLifecycleState.paused ||
        state == AppLifecycleState.hidden) {
      _pushTimer?.cancel();
      _pushTimer = null;
      if (_delivering) unawaited(_keepScreenOn(false));
    } else if (state == AppLifecycleState.resumed && _delivering) {
      unawaited(_pushOnce());
      _pushTimer = Timer.periodic(_pushInterval, (_) => unawaited(_pushOnce()));
      unawaited(_keepScreenOn(true));
    }
  }

  /// A quiet work-list refresh: new handoffs appear, stray re-routes
  /// replace the drawn route, delivered jobs fall off.
  Future<void> _refreshList() async {
    try {
      final deliveries =
          await ref.read(riderApiProvider).listActiveDeliveries();
      if (!mounted) return;
      setState(() {
        _deliveries = deliveries;
        _loadError = null;
      });
      // The last job just left the list: delivering has nothing to push
      // to — stop honestly instead of looping on an empty run.
      if (_delivering && deliveries.isEmpty) {
        await _stopDelivering();
      }
    } on ApiError catch (e) {
      if (!mounted) return;
      // First load failure is a visible error; a poll failure keeps the
      // last good list.
      if (_deliveries == null) setState(() => _loadError = e.message);
    } on Object {
      if (!mounted || _deliveries != null) return;
      setState(() => _loadError = 'Could not reach the server.');
    }
  }

  Future<void> _startDelivering() async {
    if (_starting || _delivering) return;
    // The fix the loop will use — and the permission prompt, if this is
    // the first time. GPS acquisition can take seconds, so the button
    // shows "Locating…" until the first fix lands. Every failure is an
    // honest on-screen state.
    setState(() => _starting = true);
    try {
      final fix = await ref.read(acquireLocationProvider)();
      if (!mounted) return;
      setState(() {
        _starting = false;
        _delivering = true;
        _pushError = false;
        _riderLat = fix.lat;
        _riderLng = fix.lng;
      });
      unawaited(_keepScreenOn(true)); // screen-on on the bike mount.
      await _pushOnce();
      _pushTimer = Timer.periodic(_pushInterval, (_) => unawaited(_pushOnce()));
    } on Object {
      if (!mounted) return;
      setState(() => _starting = false);
      ScaffoldMessenger.of(context).showSnackBar(
        SnackBar(
          content: const Text(
            'Location is off or permission was denied — delivering needs your GPS.',
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

  Future<void> _stopDelivering() async {
    _pushTimer?.cancel();
    _pushTimer = null;
    unawaited(_keepScreenOn(false));
    if (!mounted) return;
    setState(() {
      _delivering = false;
      _pushError = false;
    });
  }

  /// One tick: one GPS fix, pushed to every active delivery. Failures are
  /// quiet — the next tick retries; the status line tells the truth.
  Future<void> _pushOnce() async {
    final deliveries = _deliveries;
    if (deliveries == null || deliveries.isEmpty) return;
    try {
      final fix = await ref.read(acquireLocationProvider)();
      if (!mounted) return;
      setState(() {
        _riderLat = fix.lat;
        _riderLng = fix.lng;
      });
      var anySucceeded = false;
      for (final delivery in deliveries) {
        try {
          await ref
              .read(riderApiProvider)
              .pushLocation(delivery.deliveryId, fix.lat, fix.lng);
          anySucceeded = true;
        } on Object {
          // One delivery failing must not starve the others.
        }
      }
      if (!mounted) return;
      setState(() {
        _lastPushAt = DateTime.now();
        _pushError = !anySucceeded;
      });
    } on Object {
      if (!mounted) return;
      setState(() => _pushError = true);
    }
  }

  Future<void> _markDelivered(RiderDelivery delivery) async {
    final confirmed = await showDialog<bool>(
      context: context,
      builder: (dialogContext) => AlertDialog(
        backgroundColor: AppColors.surfaceAlt,
        title: Text('Deliver to ${delivery.destinationAddress}?'),
        content: const Text(
          'Confirm the handover: the food is with the customer and the '
          'cash is in your hand. This settles the payment.',
        ),
        actions: [
          TextButton(
            onPressed: () => Navigator.of(dialogContext).pop(false),
            child: const Text('Not yet'),
          ),
          FilledButton(
            onPressed: () => Navigator.of(dialogContext).pop(true),
            child: const Text('Delivered'),
          ),
        ],
      ),
    );
    if (confirmed != true) return;

    setState(() => _finishingId = delivery.deliveryId);
    try {
      await ref.read(riderApiProvider).markDelivered(delivery.deliveryId);
      if (!mounted) return;
      ScaffoldMessenger.of(context).hideCurrentSnackBar();
      ScaffoldMessenger.of(context).showSnackBar(
        SnackBar(
          content: const Text(
            'Delivered — cash received. The store sees it too.',
            style: TextStyle(color: AppColors.onSurface),
          ),
          behavior: SnackBarBehavior.floating,
          backgroundColor: AppColors.surfaceAlt,
          shape: RoundedRectangleBorder(
            borderRadius: BorderRadius.circular(12),
          ),
        ),
      );
      await _refreshList();
    } on ApiError catch (e) {
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
      await _refreshList();
    } on Object {
      if (!mounted) return;
      ScaffoldMessenger.of(context).showSnackBar(
        SnackBar(
          content: const Text(
            'Could not reach the server — try again.',
            style: TextStyle(color: AppColors.onSurface),
          ),
          behavior: SnackBarBehavior.floating,
          backgroundColor: AppColors.surfaceAlt,
          shape: RoundedRectangleBorder(
            borderRadius: BorderRadius.circular(12),
          ),
        ),
      );
    } finally {
      if (mounted) setState(() => _finishingId = null);
    }
  }

  Future<void> _call(String phone) async {
    final uri = Uri(scheme: 'tel', path: phone);
    if (await canLaunchUrl(uri)) {
      await launchUrl(uri);
    } else if (mounted) {
      ScaffoldMessenger.of(context).showSnackBar(
        SnackBar(
          content: Text(
            'Could not open the dialer — the number is $phone',
            style: const TextStyle(color: AppColors.onSurface),
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

  Future<void> _navigateTo(RiderDelivery delivery) async {
    final uri = Uri.parse(
      'https://www.google.com/maps/dir/?api=1'
      '&destination=${delivery.destinationLat},${delivery.destinationLng}'
      '&travelmode=driving',
    );
    await launchUrl(uri, mode: LaunchMode.externalApplication);
  }

  Future<void> _signOut() async {
    await _stopDelivering();
    await ref.read(sessionProvider.notifier).signOut();
  }

  @override
  Widget build(BuildContext context) {
    final session = ref.watch(sessionProvider).asData?.value;
    final user = session is SessionUser ? session.user : null;
    final rider = user?.rider;

    return Scaffold(
      backgroundColor: AppColors.surface,
      body: SafeArea(
        child: ListView(
          padding: const EdgeInsets.fromLTRB(20, 24, 20, 24),
          children: [
            Text(
              'Rider mode',
              style: Theme.of(context).textTheme.headlineMedium?.copyWith(
                    fontWeight: FontWeight.w700,
                  ),
            ),
            const SizedBox(height: 24),
            _IdentityCard(name: user?.displayName ?? 'Rider', phone: user?.phone),
            const SizedBox(height: 16),
            _RiderNumberCard(rider: rider),
            const SizedBox(height: 24),
            _buildJobs(context),
            const SizedBox(height: 24),
            _StatusCard(rider: rider),
            const SizedBox(height: 16),
            _SignOutRow(onSignOut: _signOut),
          ],
        ),
      ),
    );
  }

  Widget _buildJobs(BuildContext context) {
    final deliveries = _deliveries;
    if (_loadError != null) {
      return _JobsCard(
        child: Column(
          crossAxisAlignment: CrossAxisAlignment.start,
          children: [
            Text(
              _loadError!,
              style: Theme.of(context).textTheme.bodyMedium,
            ),
            const SizedBox(height: 12),
            OutlinedButton.icon(
              onPressed: () => unawaited(_refreshList()),
              icon: const Icon(Icons.refresh_rounded, size: 18),
              label: const Text('Try again'),
            ),
          ],
        ),
      );
    }
    if (deliveries == null) {
      return const _JobsCard(
        child: Center(child: CircularProgressIndicator()),
      );
    }
    if (deliveries.isEmpty) {
      return _JobsCard(
        child: Text(
          'No deliveries yet. When a store hands you an order with your '
          'rider number, it appears here.',
          style: Theme.of(context).textTheme.bodyMedium?.copyWith(
                color: AppColors.onSurfaceMuted,
              ),
        ),
      );
    }
    return Column(
      crossAxisAlignment: CrossAxisAlignment.start,
      children: [
        _StartStopCard(
          delivering: _delivering,
          starting: _starting,
          lastPushAt: _lastPushAt,
          pushError: _pushError,
          onStart: () => unawaited(_startDelivering()),
          onStop: () => unawaited(_stopDelivering()),
        ),
        const SizedBox(height: 16),
        for (final delivery in deliveries) ...[
          _DeliveryCard(
            delivery: delivery,
            riderLat: _riderLat,
            riderLng: _riderLng,
            finishing: _finishingId == delivery.deliveryId,
            onDelivered: () => unawaited(_markDelivered(delivery)),
            onCall: () => unawaited(_call(delivery.customerPhone!)),
            onNavigate: () => unawaited(_navigateTo(delivery)),
          ),
          const SizedBox(height: 16),
        ],
      ],
    );
  }
}

/// The section container for job states.
class _JobsCard extends StatelessWidget {
  const _JobsCard({required this.child});

  final Widget child;

  @override
  Widget build(BuildContext context) {
    return Container(
      width: double.infinity,
      padding: const EdgeInsets.all(20),
      decoration: BoxDecoration(
        color: AppColors.surfaceAlt,
        borderRadius: BorderRadius.circular(20),
        border: Border.all(color: AppColors.surfaceBorder),
      ),
      child: child,
    );
  }
}

/// The one dominant action: Start/Stop delivering, with the honest push
/// status line ("Pushing · last 3s ago" / "reconnecting…").
class _StartStopCard extends StatelessWidget {
  const _StartStopCard({
    required this.delivering,
    required this.starting,
    required this.lastPushAt,
    required this.pushError,
    required this.onStart,
    required this.onStop,
  });

  final bool delivering;
  final bool starting;
  final DateTime? lastPushAt;
  final bool pushError;
  final VoidCallback onStart;
  final VoidCallback onStop;

  @override
  Widget build(BuildContext context) {
    return Container(
      padding: const EdgeInsets.all(16),
      decoration: BoxDecoration(
        color: AppColors.surfaceAlt,
        borderRadius: BorderRadius.circular(20),
        border: Border.all(
          color: delivering
              ? AppColors.success.withValues(alpha: 0.5)
              : AppColors.surfaceBorder,
        ),
      ),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          SizedBox(
            width: double.infinity,
            child: FilledButton.icon(
              onPressed: delivering ? onStop : (starting ? null : onStart),
              style: FilledButton.styleFrom(
                minimumSize: const Size.fromHeight(52),
                backgroundColor:
                    delivering ? AppColors.error : AppColors.primary,
              ),
              icon: starting
                  ? const SizedBox(
                      width: 18,
                      height: 18,
                      child: CircularProgressIndicator(
                        strokeWidth: 2,
                        color: AppColors.onPrimary,
                      ),
                    )
                  : Icon(
                      delivering
                          ? Icons.stop_circle_rounded
                          : Icons.play_circle_rounded,
                    ),
              label: Text(
                delivering
                    ? 'Stop delivering'
                    : starting
                        ? 'Locating…'
                        : 'Start delivering',
                style: const TextStyle(
                  fontSize: 16,
                  fontWeight: FontWeight.w700,
                ),
              ),
            ),
          ),
          const SizedBox(height: 10),
          Row(
            children: [
              Icon(
                delivering
                    ? (pushError
                        ? Icons.wifi_off_rounded
                        : Icons.my_location_rounded)
                    : Icons.pause_circle_outline_rounded,
                size: 16,
                color: delivering
                    ? (pushError ? AppColors.error : AppColors.success)
                    : AppColors.onSurfaceMuted,
              ),
              const SizedBox(width: 8),
              Expanded(
                child: Text(
                  !delivering
                      ? 'Your position is only pushed while you deliver — '
                          'nothing runs in the background.'
                      : pushError
                          ? 'Reconnecting — the next fix retries.'
                          : _pushLine(),
                  style: Theme.of(context).textTheme.bodySmall?.copyWith(
                        color: pushError && delivering
                            ? AppColors.error
                            : AppColors.onSurfaceMuted,
                      ),
                ),
              ),
            ],
          ),
        ],
      ),
    );
  }

  String _pushLine() {
    final at = lastPushAt;
    if (at == null) return 'Pushing your position every 5 seconds…';
    final seconds = DateTime.now().difference(at).inSeconds;
    return 'Pushing · last signal ${seconds}s ago';
  }
}

/// One job: the store it came from, where it goes, who receives it, the
/// map, and the Delivered action.
class _DeliveryCard extends StatelessWidget {
  const _DeliveryCard({
    required this.delivery,
    required this.finishing,
    required this.onDelivered,
    required this.onCall,
    required this.onNavigate,
    this.riderLat,
    this.riderLng,
  });

  final RiderDelivery delivery;
  final bool finishing;
  final VoidCallback onDelivered;
  final VoidCallback onCall;
  final VoidCallback onNavigate;
  final double? riderLat;
  final double? riderLng;

  @override
  Widget build(BuildContext context) {
    final textTheme = Theme.of(context).textTheme;
    return Container(
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
                  delivery.storeName,
                  maxLines: 1,
                  overflow: TextOverflow.ellipsis,
                  style: textTheme.titleSmall?.copyWith(
                    fontWeight: FontWeight.w700,
                  ),
                ),
              ),
              Container(
                padding:
                    const EdgeInsets.symmetric(horizontal: 8, vertical: 4),
                decoration: BoxDecoration(
                  color: AppColors.success.withValues(alpha: 0.12),
                  borderRadius: BorderRadius.circular(999),
                ),
                child: Text(
                  'Out for delivery',
                  style: textTheme.labelSmall?.copyWith(
                    color: AppColors.success,
                    fontWeight: FontWeight.w700,
                  ),
                ),
              ),
            ],
          ),
          const SizedBox(height: 8),
          Text(
            delivery.destinationAddress,
            style: textTheme.bodyMedium,
          ),
          if (delivery.customerName != null) ...[
            const SizedBox(height: 2),
            Text(
              'For ${delivery.customerName}',
              style: textTheme.bodySmall?.copyWith(
                color: AppColors.onSurfaceMuted,
              ),
            ),
          ],
          if (delivery.etaTarget != null) ...[
            const SizedBox(height: 2),
            Text(
              'Arrive ~${_clockTime(delivery.etaTarget!)}',
              style: textTheme.bodySmall?.copyWith(
                color: AppColors.primary,
                fontWeight: FontWeight.w600,
              ),
            ),
          ],
          const SizedBox(height: 12),
          RiderMap(
            delivery: delivery,
            riderLat: riderLat,
            riderLng: riderLng,
          ),
          const SizedBox(height: 12),
          Row(
            children: [
              if (delivery.customerPhone != null)
                OutlinedButton.icon(
                  onPressed: onCall,
                  style: OutlinedButton.styleFrom(
                    foregroundColor: AppColors.primary,
                    side: BorderSide(
                      color: AppColors.primary.withValues(alpha: 0.4),
                    ),
                    shape: RoundedRectangleBorder(
                      borderRadius: BorderRadius.circular(10),
                    ),
                  ),
                  icon: const Icon(Icons.call_rounded, size: 18),
                  label: const Text('Call'),
                ),
              if (delivery.destinationLat != null &&
                  delivery.destinationLng != null) ...[
                const SizedBox(width: 8),
                OutlinedButton.icon(
                  onPressed: onNavigate,
                  style: OutlinedButton.styleFrom(
                    foregroundColor: AppColors.primary,
                    side: BorderSide(
                      color: AppColors.primary.withValues(alpha: 0.4),
                    ),
                    shape: RoundedRectangleBorder(
                      borderRadius: BorderRadius.circular(10),
                    ),
                  ),
                  icon: const Icon(Icons.navigation_rounded, size: 18),
                  label: const Text('Navigate'),
                ),
              ],
            ],
          ),
          const SizedBox(height: 10),
          // The handover is the dominant action — full width, like every
          // primary button in the app's theme.
          SizedBox(
            width: double.infinity,
            child: FilledButton(
              onPressed: finishing ? null : onDelivered,
              style: FilledButton.styleFrom(
                backgroundColor: AppColors.success,
              ),
              child: Text(finishing ? '…' : 'Delivered'),
            ),
          ),
        ],
      ),
    );
  }

  String _clockTime(DateTime time) {
    final local = time.toLocal();
    final hour = local.hour.toString().padLeft(2, '0');
    final minute = local.minute.toString().padLeft(2, '0');
    return '$hour:$minute';
  }
}

class _IdentityCard extends StatelessWidget {
  const _IdentityCard({required this.name, required this.phone});

  final String name;
  final String? phone;

  @override
  Widget build(BuildContext context) {
    return Container(
      padding: const EdgeInsets.all(16),
      decoration: BoxDecoration(
        color: AppColors.surfaceAlt,
        borderRadius: BorderRadius.circular(20),
        border: Border.all(color: AppColors.surfaceBorder),
      ),
      child: Row(
        children: [
          InitialsTile(text: name),
          const SizedBox(width: 14),
          Expanded(
            child: Column(
              crossAxisAlignment: CrossAxisAlignment.start,
              children: [
                Text(
                  name,
                  style: Theme.of(context).textTheme.titleMedium?.copyWith(
                        fontWeight: FontWeight.w600,
                      ),
                ),
                if (phone != null) ...[
                  const SizedBox(height: 2),
                  Text(
                    phone!,
                    style: Theme.of(context).textTheme.bodySmall?.copyWith(
                          color: AppColors.onSurfaceMuted,
                        ),
                  ),
                ],
              ],
            ),
          ),
        ],
      ),
    );
  }
}

/// The rider number, displayed large: it is the whole interface of the
/// handoff design — the merchant types this number when handing the
/// delivery over.
class _RiderNumberCard extends StatelessWidget {
  const _RiderNumberCard({required this.rider});

  final RiderInfo? rider;

  @override
  Widget build(BuildContext context) {
    final number = rider?.riderNumber;
    return Container(
      padding: const EdgeInsets.all(20),
      decoration: BoxDecoration(
        color: AppColors.surfaceAlt,
        borderRadius: BorderRadius.circular(20),
        border: Border.all(color: AppColors.surfaceBorder),
      ),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          Text(
            'Your rider number',
            style: Theme.of(context).textTheme.labelMedium?.copyWith(
                  color: AppColors.onSurfaceMuted,
                ),
          ),
          const SizedBox(height: 8),
          Text(
            number != null ? '#$number' : '—',
            style: Theme.of(context).textTheme.displaySmall?.copyWith(
                  color: AppColors.primary,
                  fontWeight: FontWeight.w800,
                  letterSpacing: 2,
                ),
          ),
          const SizedBox(height: 6),
          Text(
            'Show this number to the store at handoff — they type it to assign you the delivery.',
            style: Theme.of(context).textTheme.bodySmall?.copyWith(
                  color: AppColors.onSurfaceMuted,
                ),
          ),
        ],
      ),
    );
  }
}

class _StatusCard extends StatelessWidget {
  const _StatusCard({required this.rider});

  final RiderInfo? rider;

  @override
  Widget build(BuildContext context) {
    final active = rider?.isActive ?? true;
    return Container(
      padding: const EdgeInsets.symmetric(horizontal: 16, vertical: 14),
      decoration: BoxDecoration(
        color: AppColors.surfaceAlt,
        borderRadius: BorderRadius.circular(16),
        border: Border.all(color: AppColors.surfaceBorder),
      ),
      child: Row(
        children: [
          Icon(
            active
                ? Icons.check_circle_rounded
                : Icons.pause_circle_outline_rounded,
            color: active ? AppColors.success : AppColors.onSurfaceMuted,
            size: 22,
          ),
          const SizedBox(width: 12),
          Expanded(
            child: Text(
              active
                  ? 'Active — you can be assigned deliveries.'
                  : 'Inactive — you can\'t be assigned deliveries. Contact Tuma to be reactivated.',
              style: Theme.of(context).textTheme.bodySmall?.copyWith(
                    color: AppColors.onSurfaceMuted,
                  ),
            ),
          ),
        ],
      ),
    );
  }
}

class _SignOutRow extends StatelessWidget {
  const _SignOutRow({required this.onSignOut});

  final VoidCallback onSignOut;

  @override
  Widget build(BuildContext context) {
    return TextButton.icon(
      onPressed: onSignOut,
      icon: const Icon(Icons.logout_rounded, size: 20),
      label: const Text('Sign out'),
      style: TextButton.styleFrom(
        foregroundColor: AppColors.error,
        padding: const EdgeInsets.symmetric(horizontal: 12, vertical: 10),
      ),
    );
  }
}
