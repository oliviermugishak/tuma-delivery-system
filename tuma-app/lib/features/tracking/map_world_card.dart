import 'dart:async';

import 'package:flutter/material.dart';
import 'package:url_launcher/url_launcher.dart';

import 'package:tuma_app/core/api/models/tracking.dart';
import 'package:tuma_app/core/theme/app_colors.dart';
import 'package:tuma_app/features/tracking/delivery_map.dart';

/// The Moving world's card (tracking doc §1/§2): the map takes over, the
/// timeline collapses to a slim progress strip, and the ETA decays against
/// the server-provided target — presenting real data smoothly, never
/// manufacturing it. The truth ladder (§2) lives here: Live → Lagging →
/// Ended changes presentation and the poll cadence, never data — no
/// auto-delivered, ever. Fallbacks are mechanical (doc §3): no
/// destination pin → no map, honest text; pin but no rider check-in yet →
/// the cached route plus "you'll see the rider the moment their phone
/// checks in".
class MapWorldCard extends StatefulWidget {
  const MapWorldCard({
    super.key,
    required this.tracking,
    this.destinationLat,
    this.destinationLng,
  });

  final DeliveryTracking tracking;
  final double? destinationLat;
  final double? destinationLng;

  @override
  State<MapWorldCard> createState() => _MapWorldCardState();
}

/// The truth ladder's rungs (tracking doc §2). `ended` stops the poll.
enum _Ladder { live, lagging, ended }

class _MapWorldCardState extends State<MapWorldCard>
    with WidgetsBindingObserver {
  /// The ticker re-renders the decaying ETA and re-evaluates the ladder
  /// against absolute timestamps. It renders minutes, so 1s was 60× the
  /// honest cost — 5s matches what the eye can see. It is
  /// lifecycle-aware (backgrounded, it stops) and goes to sleep entirely
  /// once the ladder reaches `ended`, where the presentation is static.
  Timer? _clock;

  /// The grace period past the ETA before "Taking longer than expected"
  /// appears (the doc's ~15 min).
  static const _overdueGrace = Duration(minutes: 15);

  /// Past ETA + 1h the badge is firmly Ended territory (poll stops at
  /// ~24h — the order detail owns that rule; here it's presentation).
  static const _endedAfter = Duration(hours: 24);

  @override
  void initState() {
    super.initState();
    WidgetsBinding.instance.addObserver(this);
    _clock = Timer.periodic(const Duration(seconds: 5), (_) {
      if (mounted && _ladder != _Ladder.ended) setState(() {});
    });
  }

  @override
  void didChangeAppLifecycleState(AppLifecycleState state) {
    if (state == AppLifecycleState.paused ||
        state == AppLifecycleState.hidden) {
      _clock?.cancel();
      _clock = null;
    } else if (state == AppLifecycleState.resumed && _clock == null) {
      _clock = Timer.periodic(const Duration(seconds: 5), (_) {
        if (mounted && _ladder != _Ladder.ended) setState(() {});
      });
    }
  }

  @override
  void dispose() {
    WidgetsBinding.instance.removeObserver(this);
    _clock?.cancel();
    super.dispose();
  }

  /// The rung for this delivery right now: fresh signal and not past the
  /// ETA → Live; stale signal or past the grace → Lagging; ~24h past ETA
  /// with no delivery → Ended. Computed from real timestamps only.
  _Ladder get _ladder {
    final tracking = widget.tracking;
    final overdue = tracking.overdueBy;
    if (overdue != null && overdue >= _endedAfter) return _Ladder.ended;
    final age = tracking.signalAgeMinutes;
    if (age != null && age >= 5) return _Ladder.lagging;
    if (overdue != null && overdue >= _overdueGrace) return _Ladder.lagging;
    return _Ladder.live;
  }

  @override
  Widget build(BuildContext context) {
    final tracking = widget.tracking;
    // §3: the destination pin decides the map — a guessed geocode is
    // never invented for an address the customer didn't pin.
    final hasPin =
        widget.destinationLat != null && widget.destinationLng != null;
    final textTheme = Theme.of(context).textTheme;
    final ladder = _ladder;

    return Container(
      padding: const EdgeInsets.all(16),
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
                child: switch (ladder) {
                  // The decaying ETA, or the honest no-ETA line.
                  _Ladder.live || _Ladder.lagging => tracking.etaTarget !=
                          null
                      ? _EtaLine(
                          target: tracking.etaTarget!,
                          overdue: tracking.overdueBy != null &&
                              tracking.overdueBy! >= _overdueGrace,
                          storePhone: tracking.storeContactPhone,
                        )
                      : Text(
                          'Your delivery is on the way.',
                          style: textTheme.titleSmall
                              ?.copyWith(fontWeight: FontWeight.w700),
                        ),
                  // Ended: the truth about the pipeline, the way out.
                  _Ladder.ended => Text(
                      'This delivery is taking much longer than expected.',
                      style: textTheme.titleSmall
                          ?.copyWith(fontWeight: FontWeight.w700),
                    ),
                },
              ),
              const SizedBox(width: 8),
              _LadderBadge(ladder: ladder, ageMinutes: tracking.signalAgeMinutes),
            ],
          ),
          // Past the grace or Ended: the honest line plus the way out —
          // call the store (the delivery's `store_contact_phone`).
          if (tracking.storeContactPhone != null &&
              (ladder == _Ladder.ended ||
                  tracking.overdueBy != null &&
                      tracking.overdueBy! >= _overdueGrace))
            _CallTheStore(phone: tracking.storeContactPhone!),
          const SizedBox(height: 12),
          // §3: no destination pin → no map, never a guessed geocode.
          if (hasPin) ...[
            DeliveryMap(
              tracking: tracking,
              destinationLat: widget.destinationLat,
              destinationLng: widget.destinationLng,
            ),
            const SizedBox(height: 12),
          ] else ...[
            Text(
              'The store has your address — add a map pin at checkout next time and watch the rider move.',
              style: textTheme.bodySmall
                  ?.copyWith(color: AppColors.onSurfaceMuted),
            ),
          ],
          if (hasPin && !tracking.hasRiderPosition && ladder != _Ladder.ended) ...[
            const SizedBox(height: 8),
            Row(
              children: [
                const Icon(Icons.notifications_none_rounded,
                    size: 16, color: AppColors.onSurfaceMuted),
                const SizedBox(width: 6),
                Expanded(
                  child: Text(
                    "You'll see the rider the moment their phone checks in.",
                    style: textTheme.bodySmall
                        ?.copyWith(color: AppColors.onSurfaceMuted),
                  ),
                ),
              ],
            ),
          ],
        ],
      ),
    );
  }
}

/// The truth ladder's badge (doc §2): Live green, Lagging amber with the
/// last-signal age, Ended grey. Presentation only — the data underneath
/// is always the last real snapshot.
class _LadderBadge extends StatelessWidget {
  const _LadderBadge({required this.ladder, this.ageMinutes});

  final _Ladder ladder;
  final int? ageMinutes;

  @override
  Widget build(BuildContext context) {
    final (color, label) = switch (ladder) {
      _Ladder.live => (AppColors.success, 'Live'),
      _Ladder.lagging => (
          const Color(0xFFF59E0B),
          ageMinutes == null ? 'Lagging' : 'Lagging · $ageMinutes min ago',
        ),
      _Ladder.ended => (AppColors.onSurfaceMuted, 'Ended'),
    };
    return Container(
      padding: const EdgeInsets.symmetric(horizontal: 8, vertical: 5),
      decoration: BoxDecoration(
        color: color.withValues(alpha: 0.12),
        borderRadius: BorderRadius.circular(999),
      ),
      child: Text(
        label,
        style: Theme.of(context).textTheme.labelSmall?.copyWith(
              color: color,
              fontWeight: FontWeight.w700,
            ),
      ),
    );
  }
}

/// Past the ETA + grace: the honest line plus the way out — call the
/// store (the number is the delivery's `store_contact_phone`).
class _CallTheStore extends StatelessWidget {
  const _CallTheStore({required this.phone});

  final String phone;

  @override
  Widget build(BuildContext context) {
    return Padding(
      padding: const EdgeInsets.only(top: 6),
      child: Row(
        children: [
          const Icon(Icons.storefront_rounded,
              size: 16, color: AppColors.error),
          const SizedBox(width: 6),
          Expanded(
            child: Text(
              'Something may be wrong — call the store to check on it.',
              style: Theme.of(context).textTheme.bodySmall?.copyWith(
                    color: AppColors.onSurfaceMuted,
                  ),
            ),
          ),
          GestureDetector(
            onTap: () => unawaited(
              launchUrl(Uri(scheme: 'tel', path: phone)),
            ),
            child: Text(
              'Call',
              style: Theme.of(context).textTheme.labelMedium?.copyWith(
                    color: AppColors.primary,
                    fontWeight: FontWeight.w700,
                  ),
            ),
          ),
        ],
      ),
    );
  }
}

/// The decaying ETA: "Arriving ~14:35 · 12 min", floored at "Arriving
/// now" — or, past the grace period, "Taking longer than expected" (the
/// call-the-store action renders below). It counts down against the
/// server's absolute timestamp — smooth without lying (tracking doc §4).
class _EtaLine extends StatelessWidget {
  const _EtaLine({
    required this.target,
    required this.overdue,
    this.storePhone,
  });

  final DateTime target;
  final bool overdue;
  final String? storePhone;

  @override
  Widget build(BuildContext context) {
    final remaining = target.difference(DateTime.now());
    final textTheme = Theme.of(context).textTheme;
    final label = overdue
        ? 'Taking longer than expected'
        : remaining.isNegative
            ? 'Arriving now'
            : 'Arriving ${_clockTime(target)} · ${remaining.inMinutes + (remaining.inSeconds > 0 ? 1 : 0)} min';

    return Row(
      children: [
        Icon(
          Icons.schedule_rounded,
          size: 18,
          color: overdue ? AppColors.error : AppColors.primary,
        ),
        const SizedBox(width: 8),
        Expanded(
          child: Text(
            label,
            style: textTheme.titleSmall?.copyWith(
              fontWeight: FontWeight.w700,
              color: overdue ? AppColors.error : null,
            ),
          ),
        ),
      ],
    );
  }

  String _clockTime(DateTime time) {
    final local = time.toLocal();
    final hour = local.hour.toString().padLeft(2, '0');
    final minute = local.minute.toString().padLeft(2, '0');
    return '~$hour:$minute';
  }
}

/// The slim progress strip that replaces the six-step timeline while the
/// delivery moves (doc §1: the map leads, the timeline collapses). The
/// whole strip is one state — picked_up — so it renders as a single gold
/// segment the customer watches.
class SlimProgressStrip extends StatelessWidget {
  const SlimProgressStrip({super.key});

  @override
  Widget build(BuildContext context) {
    return Row(
      children: [
        Expanded(
          child: Container(
            height: 4,
            decoration: BoxDecoration(
              color: AppColors.primary,
              borderRadius: BorderRadius.circular(2),
            ),
          ),
        ),
        const SizedBox(width: 8),
        const Icon(Icons.motorcycle_rounded,
            size: 16, color: AppColors.primary),
        const SizedBox(width: 8),
        Text(
          'Out for delivery',
          style: Theme.of(context).textTheme.labelSmall?.copyWith(
                color: AppColors.primary,
                fontWeight: FontWeight.w700,
              ),
        ),
      ],
    );
  }
}
