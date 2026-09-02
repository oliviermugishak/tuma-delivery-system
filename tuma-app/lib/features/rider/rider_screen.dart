import 'dart:async';

import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:geolocator/geolocator.dart';
import 'package:go_router/go_router.dart';
import 'package:url_launcher/url_launcher.dart';
import 'package:wakelock_plus/wakelock_plus.dart';

import 'package:tuma_app/core/api/api_client.dart';
import 'package:tuma_app/core/api/models/rider.dart';
import 'package:tuma_app/core/auth/auth_controller.dart';
import 'package:tuma_app/core/theme/app_colors.dart';
import 'package:tuma_app/core/theme/app_theme.dart';
import 'package:tuma_app/core/utils/format_rwf.dart';
import 'package:tuma_app/shared/widgets/design_system.dart';
import 'package:tuma_app/shared/widgets/show_app_snack.dart';
import 'package:tuma_app/features/location/customer_location.dart';

/// The honest line for a location failure — the rider must know whether
/// to fix their phone (permission, services, GPS signal) or their
/// connection. geolocator throws its own exception types; every one maps
/// to a named fix, never a generic "server" blame.
String gpsFailureReason(Object error) {
  if (error is PermissionDeniedException) {
    return 'Location permission is off — allow it in Settings, then try again.';
  }
  if (error is LocationServiceDisabledException) {
    return 'Location services are off — turn on GPS, then try again.';
  }
  if (error is TimeoutException) {
    return 'Getting your position took too long — step outside or try again.';
  }
  return 'Could not get your position — check location and connection, then try again.';
}

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
  String? _finishingId;
  Timer? _pushTimer;
  Timer? _listTimer;
  RiderTally? _tally;
  /// Consecutive empty work-list polls while delivering (review P25):
  /// the run ends only on the SECOND one — a single `[]` may be a
  /// server hiccup, not a finished run.
  int _emptyPolls = 0;

  /// Per-delivery stage: the rider CONFIRMS pickup on screen (the food
  /// is in hand) — the server status stays picked_up (six-value rule);
  /// this is presentation state, so a kiosk restart re-asks, which is
  /// honest: the number re-surfaces for the store.
  final Set<String> _confirmedPickups = {};

  /// Guards one push tick at a time: a GPS fix can take up to 10s while
  /// the 5s timer keeps firing — overlapping ticks would stack HTTP
  /// pushes and setState races. A late tick is skipped, not queued.
  bool _pushInFlight = false;

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
    // Foreground-only, for BOTH loops: backgrounded, the push loop stops,
    // the wakelock drops, and the work-list poll stops too (a poll with
    // the app "closed" is exactly the background service we promised not
    // to be). Resumed, whichever loop should run is re-programmed —
    // through _syncPushLoop, never a bare Timer.periodic (the old leak:
    // a resume could stack a second timer beside the running one).
    if (state == AppLifecycleState.paused ||
        state == AppLifecycleState.hidden) {
      _pushTimer?.cancel();
      _pushTimer = null;
      _listTimer?.cancel();
      _listTimer = null;
      if (_delivering) unawaited(_keepScreenOn(false));
    } else if (state == AppLifecycleState.resumed) {
      unawaited(_refreshList());
      _listTimer ??= Timer.periodic(_listInterval, (_) => unawaited(_refreshList()));
      if (_delivering) {
        unawaited(_pushOnce());
        _syncPushLoop(delivering: true);
        unawaited(_keepScreenOn(true));
      }
    }
  }

  /// (Re)arms or disarms the push loop to exactly match [delivering] —
  /// the lifecycle handler's and the toggle's single point of truth.
  void _syncPushLoop({required bool delivering}) {
    if (delivering) {
      _pushTimer ??= Timer.periodic(_pushInterval, (_) => unawaited(_pushOnce()));
    } else {
      _pushTimer?.cancel();
      _pushTimer = null;
    }
  }

  /// A quiet work-list refresh: new handoffs appear, stray re-routes
  /// replace the drawn route, delivered jobs fall off.
  Future<void> _refreshList() async {
    try {
      unawaited(
        ref.read(riderApiProvider).today().then((tally) {
          if (mounted) setState(() => _tally = tally);
        }).catchError((_) {}),
      );
      final deliveries =
          await ref.read(riderApiProvider).listActiveDeliveries();
      if (!mounted) return;
      setState(() {
        _deliveries = deliveries;
        _loadError = null;
      });
      // The work list went empty while delivering (review P25): ONE
      // empty poll is not proof — a server hiccup or a reassignment race
      // can return `[]` for a heartbeat. Only TWO consecutive empty
      // polls end the run; any non-empty poll resets the count.
      if (_delivering && deliveries.isEmpty) {
        _emptyPolls++;
        if (_emptyPolls >= 2) {
          _emptyPolls = 0;
          await _stopDelivering();
        }
      } else {
        _emptyPolls = 0;
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

  /// The handoff moment: confirming the pickup IS going to work — the
  /// food is in hand, so the GPS push loop, the wakelock, and the
  /// customer's live map arm themselves here. Idempotent: already
  /// delivering, it's a no-op; a GPS failure keeps the honest snackbar
  /// and the rider taps again.
  Future<void> _confirmPickup(String deliveryId) async {
    setState(() => _confirmedPickups.add(deliveryId));
    if (!_delivering && !_starting) {
      await _startDelivering();
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
      await ref.read(acquireLocationProvider)();
      if (!mounted) return;
      setState(() {
        _starting = false;
        _delivering = true;
        _pushError = false;
      });
      unawaited(_keepScreenOn(true)); // screen-on on the bike mount.
      await _pushOnce();
      _syncPushLoop(delivering: true);
    } on Object catch (error) {
      if (!mounted) return;
      setState(() => _starting = false);
      // The failure names itself. On a real phone geolocator throws its
      // OWN exception types — PermissionDeniedException,
      // LocationServiceDisabledException, TimeoutException — none of
      // which are StateError, so they all used to fall into a lying
      // "could not reach the server" (the founder's big red error with
      // no server call anywhere). Every type gets its honest line.
      final reason = switch (error) {
        StateError(:final message) => 'GPS: $message. Enable location and try again.',
        _ => gpsFailureReason(error),
      };
      showAppSnack(context, reason);
    }
  }

  Future<void> _stopDelivering() async {
    _syncPushLoop(delivering: false);
    unawaited(_keepScreenOn(false));
    if (!mounted) return;
    setState(() {
      _delivering = false;
      _pushError = false;
    });
  }

  /// One tick: one GPS fix, pushed to every active delivery. Failures are
  /// quiet — the next tick retries; the status line tells the truth. An
  /// in-flight tick skips the next one rather than stacking HTTP pushes.
  Future<void> _pushOnce() async {
    if (_pushInFlight) return;
    final deliveries = _deliveries;
    if (deliveries == null || deliveries.isEmpty) return;
    _pushInFlight = true;
    try {
      final fix = await ref.read(acquireLocationProvider)();
      if (!mounted) return;

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
    } finally {
      _pushInFlight = false;
    }
  }

  Future<void> _markDelivered(RiderDelivery delivery) async {
    // Screen 17 — the dialog exists for exactly one moment: the transfer
    // of cash. It restates the amount; the confirm verb is the real
    // event (P9, P10).
    final confirmed = await showDialog<bool>(
      context: context,
      builder: (dialogContext) => AlertDialog(
        backgroundColor: AppColors.surface,
        title: Row(
          children: [
            Container(
              width: 52,
              height: 52,
              decoration: BoxDecoration(
                color: AppColors.primary.withValues(alpha: 0.12),
                borderRadius: BorderRadius.circular(16),
              ),
              child: const Icon(Icons.payments_rounded,
                  size: 28, color: AppColors.primary),
            ),
          ],
        ),
        content: Column(
          mainAxisSize: MainAxisSize.min,
          crossAxisAlignment: CrossAxisAlignment.start,
          children: [
            Text(
              'Deliver to ${delivery.customerName ?? 'the customer'}?',
              style: Theme.of(dialogContext).textTheme.titleMedium?.copyWith(
                    fontSize: 17,
                    fontWeight: FontWeight.w700,
                  ),
            ),
            const SizedBox(height: 6),
            Text.rich(
              TextSpan(
                text: 'Confirm only after collecting ',
                children: [
                  TextSpan(
                    text: '${formatRwf(delivery.total)} cash',
                    style: TextStyle(
                      color: AppColors.onSurface,
                      fontWeight: FontWeight.w700,
                    ),
                  ),
                  const TextSpan(text: ' from the customer.'),
                ],
              ),
            ),
          ],
        ),
        actions: [
          Row(
            children: [
              Expanded(
                child: OutlinedButton(
                  onPressed: () => Navigator.of(dialogContext).pop(false),
                  child: const Text('Cancel'),
                ),
              ),
              const SizedBox(width: 10),
              Expanded(
                child: FilledButton(
                  onPressed: () => Navigator.of(dialogContext).pop(true),
                  child: const Text('Cash received'),
                ),
              ),
            ],
          ),
        ],
      ),
    );
    if (confirmed != true) return;
    // The dialog's await is a gap: the kiosk may have unmounted (rider
    // signed out, session dropped) while it was open — a setState here
    // would throw (review P15).
    if (!mounted) return;

    setState(() => _finishingId = delivery.deliveryId);
    try {
      await ref.read(riderApiProvider).markDelivered(delivery.deliveryId);
      if (!mounted) return;
      ScaffoldMessenger.of(context).hideCurrentSnackBar();
      showAppSnack(context, 'Delivered — cash received. The store sees it too.');
      await _refreshList();
    } on ApiError catch (e) {
      if (!mounted) return;
      showAppSnack(context, e.message);
      await _refreshList();
    } on Object {
      if (!mounted) return;
      showAppSnack(context, 'Could not reach the server — try again.');
      // A failed handover must not leave a stale card claiming a job the
      // server may or may not have settled — re-check the real list.
      await _refreshList();
    } finally {
      if (mounted) setState(() => _finishingId = null);
    }
  }

  Future<void> _call(String phone) async {
    // Sanitized + failure-caught (review P24): a crafted number must not
    // smuggle extra digits into the dialer, and a dead dialer lands on a
    // snack, not an uncaught async error.
    await launchDialer(
      phone,
      onFail: (sanitized) {
        if (mounted) {
          showAppSnack(
              context, 'Could not open the dialer — the number is $sanitized');
        }
      },
    );
  }

  Future<void> _navigateTo(RiderDelivery delivery) async {
    final uri = Uri.parse(
      'https://www.google.com/maps/dir/?api=1'
      '&destination=${delivery.destinationLat},${delivery.destinationLng}'
      '&travelmode=driving',
    );
    try {
      await launchUrl(uri, mode: LaunchMode.externalApplication);
    } on Object {
      if (mounted) {
        showAppSnack(context, 'Could not open Google Maps.');
      }
    }
  }

  String _pushLine() {
    final at = _lastPushAt;
    if (at == null) return 'Pushing your position every 5 seconds…';
    final seconds = DateTime.now().difference(at).inSeconds;
    return 'Pushing · last signal ${seconds}s ago';
  }

  @override
  Widget build(BuildContext context) {
    final session = ref.watch(sessionProvider).asData?.value;
    final user = session is SessionUser ? session.user : null;
    final online = _delivering;
    final textTheme = Theme.of(context).textTheme;
    return Scaffold(
      backgroundColor: AppColors.surface,
      body: SafeArea(
        child: ListView(
          padding: const EdgeInsets.fromLTRB(0, 10, 0, 24),
          children: [
            // The top chrome: identity + the Stop pill (the deliver
            // toggle lives here, always reachable) + the profile door —
            // sign-out and history live THERE, not on the kiosk.
            Padding(
              padding: const EdgeInsets.symmetric(horizontal: 16),
              child: Row(
                children: [
                  AccentAvatar(text: user?.displayName ?? 'Rider', size: 44),
                  const SizedBox(width: 12),
                  Expanded(
                    child: Column(
                      crossAxisAlignment: CrossAxisAlignment.start,
                      children: [
                        Text(user?.displayName ?? 'Rider',
                            style: AppTheme.bd(textTheme)
                                .copyWith(fontWeight: FontWeight.w700)),
                        Text(user?.phone ?? '',
                            style: AppTheme.sub(textTheme)),
                      ],
                    ),
                  ),
                  // The off switch, ONLY when on the clock. Offline, the
                  // bottom Start button is the way onto the clock — a
                  // grey fake Stop beside it was the founder's
                  // "irrelevant redundant buttons" complaint.
                  if (online)
                    GestureDetector(
                      onTap: () => unawaited(_stopDelivering()),
                      child: Container(
                        padding: const EdgeInsets.symmetric(
                            horizontal: 15, vertical: 7),
                        decoration: BoxDecoration(
                          borderRadius: BorderRadius.circular(999),
                          border: Border.all(
                            color: AppColors.error.withValues(alpha: 0.35),
                          ),
                        ),
                        child: const Text(
                          'Stop',
                          style: TextStyle(
                            fontSize: 12,
                            fontWeight: FontWeight.w700,
                            color: AppColors.error,
                          ),
                        ),
                      ),
                    ),
                  const SizedBox(width: 10),
                  GestureDetector(
                    onTap: () => context.push('/rider/profile'),
                    behavior: HitTestBehavior.opaque,
                    child: const Icon(
                      Icons.account_circle_rounded,
                      size: 28,
                      color: AppColors.onSurfaceMuted,
                    ),
                  ),
                ],
              ),
            ),
            const SizedBox(height: 8),
            // The online pill — pulsing teal when on the clock.
            Padding(
              padding: const EdgeInsets.symmetric(horizontal: 16),
              child: Align(
                alignment: Alignment.centerLeft,
                child: Container(
                  padding: const EdgeInsets.symmetric(
                      horizontal: 13, vertical: 7),
                  decoration: BoxDecoration(
                    color: AppColors.success.withValues(alpha: 0.12),
                    borderRadius: BorderRadius.circular(999),
                  ),
                  child: Row(
                    mainAxisSize: MainAxisSize.min,
                    children: [
                      PulsingDot(
                          color: AppColors.success,
                          pulsing: online,
                          size: 7),
                      const SizedBox(width: 6),
                      Text(
                        online ? 'Delivering' : 'Online',
                        style: TextStyle(
                          fontSize: 12,
                          fontWeight: FontWeight.w700,
                          color: AppColors.success,
                        ),
                      ),
                    ],
                  ),
                ),
              ),
            ),
            // The push loop's honest status line (P13): what the app is
            // doing with the rider's location, right now.
            if (online)
              Padding(
                padding: const EdgeInsets.fromLTRB(16, 8, 16, 0),
                child: Row(
                  children: [
                    Icon(
                      _pushError
                          ? Icons.wifi_off_rounded
                          : Icons.my_location_rounded,
                      size: 15,
                      color: _pushError
                          ? AppColors.error
                          : AppColors.onSurfaceMuted,
                    ),
                    const SizedBox(width: 6),
                    Expanded(
                      child: Text(
                        _pushError
                            ? 'Reconnecting — the next fix retries.'
                            : _pushLine(),
                        style: TextStyle(
                          fontSize: 12.5,
                          color: _pushError
                              ? AppColors.error
                              : AppColors.onSurfaceMuted,
                        ),
                      ),
                    ),
                  ],
                ),
              ),
            const SizedBox(height: 8),
            _buildJobs(context),
          ],
        ),
      ),
    );
  }

  Widget _buildJobs(BuildContext context) {
    final textTheme = Theme.of(context).textTheme;
    final deliveries = _deliveries;
    final session = ref.watch(sessionProvider).asData?.value;
    final rider = session is SessionUser ? session.user.rider : null;
    if (_loadError != null) {
      return _card(
        child: Column(
          crossAxisAlignment: CrossAxisAlignment.start,
          children: [
            Text(_loadError!, style: AppTheme.bd(textTheme)),
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
      return const Center(
          child: Padding(
        padding: EdgeInsets.all(40),
        child: CircularProgressIndicator(),
      ));
    }

    // ---- WAITING (screen 07): calm, the number explained, the tally ----
    if (deliveries.isEmpty) {
      final tally = _tally;
      return Column(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          _card(
            child: Column(
              children: [
                const SizedBox(height: 14),
                const PulsingDot(color: AppColors.success, pulsing: true, size: 16),
                const SizedBox(height: 16),
                Text("You're online", style: AppTheme.d2(textTheme)),
                const SizedBox(height: 5),
                Text('Waiting for new orders…', style: AppTheme.bd(textTheme)),
                const SizedBox(height: 18),
                // The rider number — the handoff interface — explained in
                // one sentence (P3).
                Container(
                  padding:
                      const EdgeInsets.symmetric(horizontal: 14, vertical: 11),
                  decoration: BoxDecoration(
                    color: AppColors.primary.withValues(alpha: 0.10),
                    borderRadius: BorderRadius.circular(12),
                    border: Border.all(
                      color: AppColors.primary.withValues(alpha: 0.28),
                    ),
                  ),
                  child: Row(
                    children: [
                      const Icon(Icons.badge_rounded,
                          size: 24, color: AppColors.primary),
                      const SizedBox(width: 12),
                      Column(
                        crossAxisAlignment: CrossAxisAlignment.start,
                        children: [
                          const MicroLabel('Your rider number'),
                          const SizedBox(height: 2),
                          Text(
                            '#${rider?.riderNumber ?? '—'}',
                            style: textTheme.titleMedium?.copyWith(
                              fontSize: 20,
                              fontWeight: FontWeight.w800,
                              color: AppColors.primary,
                            ),
                          ),
                        ],
                      ),
                    ],
                  ),
                ),
                const SizedBox(height: 12),
                Text(
                  'Give this number to the store — they use it to assign you deliveries.',
                  textAlign: TextAlign.center,
                  style: AppTheme.sub(textTheme).copyWith(height: 1.5),
                ),
                const SizedBox(height: 14),
              ],
            ),
          ),
          if (tally != null && (tally.deliveries > 0 || tally.collected > 0))
            _card(
              child: Row(
                children: [
                  Expanded(
                    child: Column(
                      crossAxisAlignment: CrossAxisAlignment.start,
                      children: [
                        const MicroLabel('Today'),
                        const SizedBox(height: 3),
                        Text.rich(
                          TextSpan(
                            children: [
                              TextSpan(
                                text:
                                    '${tally.deliveries} deliver${tally.deliveries == 1 ? 'y' : 'ies'}',
                                style: AppTheme.bd(textTheme)
                                    .copyWith(fontWeight: FontWeight.w600),
                              ),
                              TextSpan(
                                text:
                                    ' · ${formatRwf(tally.collected)} collected',
                                style: AppTheme.bd(textTheme).copyWith(
                                  fontWeight: FontWeight.w600,
                                  color: AppColors.success,
                                ),
                              ),
                            ],
                          ),
                        ),
                      ],
                    ),
                  ),
                  const Icon(Icons.payments_rounded,
                      size: 20, color: AppColors.onSurfaceMuted),
                ],
              ),
            ),
          const SizedBox(height: 14),
          Padding(
            padding: const EdgeInsets.symmetric(horizontal: 16),
            child: Row(
              children: [
                const Icon(Icons.location_on_rounded,
                    size: 15, color: AppColors.onSurfaceMuted),
                const SizedBox(width: 6),
                Expanded(
                  child: Text(
                    // The CONTRACT, not the code path (P3).
                    "Location is shared only while you're delivering.",
                    style: AppTheme.sub(textTheme),
                  ),
                ),
              ],
            ),
          ),
          const SizedBox(height: 16),
          // The one Start button in the whole kiosk: off the clock, the
          // waiting card carries it. Once delivering (or mid-locate), it
          // is GONE — the top chrome's Stop pill is the off switch; a
          // button that stays after its action lies (P10).
          if (!_delivering && !_starting)
            Center(
              child: _StartDeliveringButton(
                starting: _starting,
                onStart: () => unawaited(_startDelivering()),
              ),
            ),
        ],
      );
    }

    // ---- STAGES (screens 08/09): sequenced stops, the money strip ----
    // Confirming the first pickup starts delivering automatically (the
    // GPS loop arms itself there), so no Start button lives in this
    // branch — the top chrome's Stop pill is the off switch.
    return Column(
      crossAxisAlignment: CrossAxisAlignment.start,
      children: [
        for (var i = 0; i < deliveries.length; i++)
          _StageStop(
            stopNumber: i + 1,
            stopCount: deliveries.length,
            delivery: deliveries[i],
            pickedUp: _confirmedPickups.contains(deliveries[i].deliveryId),
            finishing: _finishingId == deliveries[i].deliveryId,
            onConfirmPickup: () =>
                unawaited(_confirmPickup(deliveries[i].deliveryId)),
            onNavigate: () => unawaited(_navigateTo(deliveries[i])),
            onCall: (phone) => unawaited(_call(phone)),
            onDelivered: () => unawaited(_markDelivered(deliveries[i])),
          ),
      ],
    );
  }

  Widget _card({required Widget child}) {
    return Container(
      width: double.infinity,
      margin: const EdgeInsets.symmetric(horizontal: 16),
      padding: const EdgeInsets.all(16),
      decoration: BoxDecoration(
        color: AppColors.surfaceAlt,
        borderRadius: BorderRadius.circular(16),
        border: Border.all(color: AppColors.surfaceBorder),
      ),
      child: child,
    );
  }
}

/// The section container for job states.

/// The one dominant action: Start/Stop delivering, with the honest push
/// status line ("Pushing · last 3s ago" / "reconnecting…").

/// One job: the store it came from, where it goes, who receives it, the
/// map, and the Delivered action.


/// The rider number, displayed large: it is the whole interface of the
/// handoff design — the merchant types this number when handing the
/// delivery over.



/// One STOP in the rider's run (screens 08/09): the stage decides the
/// card — PICK UP shows the store, the order check line, the rider
/// number, and Navigate; DELIVER shows the customer, the address + note,
/// the CASH STRIP, Navigate, and the guarded Mark delivered. A completed
/// pickup renders as a done row so the run reads as a story (P11).
class _StageStop extends StatelessWidget {
  const _StageStop({
    required this.stopNumber,
    required this.stopCount,
    required this.delivery,
    required this.pickedUp,
    required this.finishing,
    required this.onConfirmPickup,
    required this.onNavigate,
    required this.onCall,
    required this.onDelivered,
  });

  final int stopNumber;
  final int stopCount;
  final RiderDelivery delivery;
  final bool pickedUp;
  final bool finishing;
  final VoidCallback onConfirmPickup;
  final VoidCallback onNavigate;
  final ValueChanged<String> onCall;
  final VoidCallback onDelivered;

  @override
  Widget build(BuildContext context) {
    final textTheme = Theme.of(context).textTheme;
    final delivering = pickedUp;
    return Padding(
      padding: const EdgeInsets.only(bottom: 12),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          // The stat label — the stage transition marker (signature 5).
          Padding(
            padding: const EdgeInsets.symmetric(horizontal: 16),
            child: Row(
              mainAxisAlignment: MainAxisAlignment.spaceBetween,
              children: [
                StatLabel(delivering
                    ? 'Stop $stopNumber of $stopCount · Deliver'
                    : 'Stop $stopNumber of $stopCount · Pick up'),
                // The stage's call action: PICK UP calls the STORE (the
                // new work-list contact phone), DELIVER calls the
                // customer. A missing number renders the chip quiet —
                // never a dead button.
                GestureDetector(
                  onTap: delivering &&
                          (delivery.customerPhone ?? '').isNotEmpty
                      ? () => onCall(delivery.customerPhone!)
                      : !delivering &&
                              (delivery.storeContactPhone ?? '').isNotEmpty
                          ? () => onCall(delivery.storeContactPhone!)
                          : null,
                  child: Container(
                    padding: const EdgeInsets.symmetric(
                        horizontal: 13, vertical: 7),
                    decoration: BoxDecoration(
                      borderRadius: BorderRadius.circular(999),
                      border: Border.all(color: AppColors.line),
                    ),
                    child: Row(
                      mainAxisSize: MainAxisSize.min,
                      children: [
                        const Icon(Icons.call_rounded,
                            size: 15, color: AppColors.primary),
                        const SizedBox(width: 6),
                        Text(
                          delivering ? 'Customer' : 'Store',
                          style: const TextStyle(
                            fontSize: 12,
                            fontWeight: FontWeight.w600,
                            color: AppColors.onSurface,
                          ),
                        ),
                      ],
                    ),
                  ),
                ),
              ],
            ),
          ),
          const SizedBox(height: 9),
          Container(
            margin: const EdgeInsets.symmetric(horizontal: 16),
            padding: const EdgeInsets.all(16),
            decoration: BoxDecoration(
              color: AppColors.surfaceAlt,
              borderRadius: BorderRadius.circular(16),
              border: Border.all(color: AppColors.surfaceBorder),
            ),
            child: Column(
              crossAxisAlignment: CrossAxisAlignment.start,
              children: [
                if (delivering) ...[
                  Row(
                    children: [
                      const Icon(Icons.check_circle_rounded,
                          size: 20, color: AppColors.success),
                      const SizedBox(width: 12),
                      Expanded(
                        child: Text(
                          'Picked up · ${delivery.storeName}',
                          style: AppTheme.bd(textTheme).copyWith(
                            fontSize: 13,
                            color: AppColors.onSurfaceMuted,
                          ),
                        ),
                      ),
                      const MicroLabel('Done'),
                    ],
                  ),
                  const SizedBox(height: 13),
                ],
                Row(
                  children: [
                    delivering
                        ? AccentAvatar(
                            text: delivery.customerName ?? 'Customer',
                            size: 44,
                            tinted: true,
                          )
                        : Container(
                            width: 44,
                            height: 44,
                            decoration: BoxDecoration(
                              color: AppColors.primary
                                  .withValues(alpha: 0.12),
                              borderRadius: BorderRadius.circular(12),
                            ),
                            child: const Icon(Icons.storefront_rounded,
                                size: 20, color: AppColors.primary),
                          ),
                    const SizedBox(width: 12),
                    Expanded(
                      child: Column(
                        crossAxisAlignment: CrossAxisAlignment.start,
                        children: [
                          Text(
                            delivering
                                ? (delivery.customerName ?? 'Customer')
                                : delivery.storeName,
                            style: textTheme.titleSmall
                                ?.copyWith(fontSize: 16),
                          ),
                          const SizedBox(height: 2),
                          Text(
                            delivering
                                ? 'Customer · ${delivery.customerPhone ?? ''}'
                                : (delivery.storeAddress ??
                                    delivery.destinationAddress),
                            style: AppTheme.sub(textTheme),
                          ),
                        ],
                      ),
                    ),
                  ],
                ),
                const SizedBox(height: 13),
                Row(
                  crossAxisAlignment: CrossAxisAlignment.start,
                  children: [
                    const Icon(Icons.location_on_rounded,
                        size: 20, color: AppColors.primary),
                    const SizedBox(width: 12),
                    Expanded(
                      child: Column(
                        crossAxisAlignment: CrossAxisAlignment.start,
                        children: [
                          Text(
                            delivery.destinationAddress,
                            style: AppTheme.bd(textTheme)
                                .copyWith(fontWeight: FontWeight.w600),
                          ),
                          if (delivery.customerNote != null &&
                              delivery.customerNote!.isNotEmpty) ...[
                            const SizedBox(height: 2),
                            Text(
                              'Note: ${delivery.customerNote}',
                              style: AppTheme.sub(textTheme)
                                  .copyWith(fontSize: 11.5),
                            ),
                          ],
                        ],
                      ),
                    ),
                  ],
                ),
                if (delivering) ...[
                  const SizedBox(height: 13),
                  // THE CASH STRIP (P8): money is the risk; it's the hero.
                  AccentStrip(
                    icon: Icons.payments_rounded,
                    text: 'Collect ${formatRwf(delivery.total)} cash',
                  ),
                ],
                const SizedBox(height: 13),
                SizedBox(
                  width: double.infinity,
                  child: FilledButton.icon(
                    onPressed: delivery.destinationLat != null &&
                            delivery.destinationLng != null
                        ? onNavigate
                        : null,
                    icon: const Icon(Icons.navigation_rounded, size: 20),
                    label: Text(
                      delivering ? 'Navigate' : 'Navigate to store',
                      style: const TextStyle(
                        fontSize: 15,
                        fontWeight: FontWeight.w600,
                      ),
                    ),
                  ),
                ),
                const SizedBox(height: 10),
                SizedBox(
                  width: double.infinity,
                  child: delivering
                      ? OutlinedButton.icon(
                          onPressed: finishing ? null : onDelivered,
                          icon: const Icon(Icons.check_circle_rounded,
                              size: 19),
                          label: Text(finishing ? '…' : 'Mark delivered'),
                        )
                      : OutlinedButton.icon(
                          onPressed: onConfirmPickup,
                          style: OutlinedButton.styleFrom(
                            foregroundColor: AppColors.success,
                            side: BorderSide(
                              color:
                                  AppColors.success.withValues(alpha: 0.4),
                            ),
                          ),
                          icon: const Icon(Icons.check_circle_rounded,
                              size: 19),
                          label: const Text('Picked up'),
                        ),
                ),
              ],
            ),
          ),
          if (stopNumber < stopCount)
            Container(
              margin: const EdgeInsets.fromLTRB(16, 0, 16, 0),
              padding:
                  const EdgeInsets.symmetric(horizontal: 14, vertical: 13),
              decoration: BoxDecoration(
                color: AppColors.surfaceAlt,
                borderRadius: BorderRadius.circular(16),
                border: Border.all(color: AppColors.surfaceBorder),
              ),
              child: Row(
                children: [
                  Container(
                    width: 26,
                    height: 26,
                    decoration: BoxDecoration(
                      shape: BoxShape.circle,
                      border: Border.all(color: AppColors.line, width: 1.5),
                    ),
                    alignment: Alignment.center,
                    child: Text(
                      '${stopNumber + 1}',
                      style: const TextStyle(
                        fontSize: 12,
                        fontWeight: FontWeight.w700,
                        color: AppColors.onSurfaceMuted,
                      ),
                    ),
                  ),
                  const SizedBox(width: 12),
                  Expanded(
                    child: Text(
                      'Next stop queued — appears here after this one',
                      style: AppTheme.bd(textTheme).copyWith(
                        fontSize: 13,
                        color: AppColors.onSurfaceMuted,
                      ),
                    ),
                  ),
                  const Icon(Icons.chevron_right_rounded,
                      size: 18, color: AppColors.onSurfaceMuted),
                ],
              ),
            ),
        ],
      ),
    );
  }
}

/// The Start button for the Waiting state — one tap arms the GPS loop.
class _StartDeliveringButton extends StatelessWidget {
  const _StartDeliveringButton({required this.starting, required this.onStart});

  final bool starting;
  final VoidCallback onStart;

  @override
  Widget build(BuildContext context) {
    return SizedBox(
      width: MediaQuery.of(context).size.width - 32,
      child: FilledButton(
        onPressed: starting ? null : onStart,
        child: starting
            ? const SizedBox(
                width: 18,
                height: 18,
                child: CircularProgressIndicator(
                  strokeWidth: 2.5,
                  color: AppColors.onPrimary,
                ),
              )
            : const Text(
                'Start delivering',
                style: TextStyle(
                  fontSize: 15,
                  fontWeight: FontWeight.w600,
                ),
              ),
      ),
    );
  }
}
