import 'dart:async';

import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';

import 'package:tuma_app/core/api/api_client.dart';
import 'package:tuma_app/core/api/models/rider.dart';
import 'package:tuma_app/core/auth/auth_controller.dart';
import 'package:tuma_app/core/theme/app_colors.dart';
import 'package:tuma_app/core/theme/app_theme.dart';
import 'package:tuma_app/core/utils/format_rwf.dart';
import 'package:tuma_app/shared/widgets/design_system.dart';
import 'package:tuma_app/shared/widgets/error_state.dart';

/// The rider's profile page — the kiosk's second surface (the founder's
/// call: riders get a profile like everyone else). Identity, the rider
/// number that IS the handoff interface, today's tally, the full
/// delivered history, and the sign-out that left the kiosk's main
/// screen. One tap back to the jobs.
class RiderProfileScreen extends ConsumerStatefulWidget {
  const RiderProfileScreen({super.key});

  @override
  ConsumerState<RiderProfileScreen> createState() =>
      _RiderProfileScreenState();
}

class _RiderProfileScreenState extends ConsumerState<RiderProfileScreen> {
  List<RiderHistoryEntry>? _history;
  RiderTally? _tally;
  String? _error;
  bool _signingOut = false;

  @override
  void initState() {
    super.initState();
    unawaited(_load());
  }

  Future<void> _load() async {
    setState(() => _error = null);
    // The tally is a garnish on this page (the kiosk carries it too) —
    // its failure must not blank the history.
    unawaited(
      ref.read(riderApiProvider).today().then((tally) {
        if (mounted) setState(() => _tally = tally);
      }).catchError((_) {}),
    );
    try {
      final history = await ref.read(riderApiProvider).history();
      if (!mounted) return;
      setState(() => _history = history);
    } on ApiError catch (e) {
      if (!mounted) return;
      setState(() => _error = e.message);
    } on Object {
      if (!mounted) return;
      setState(() => _error = 'Could not reach the server.');
    }
  }

  Future<void> _signOut() async {
    setState(() => _signingOut = true);
    try {
      await ref.read(sessionProvider.notifier).signOut();
    } finally {
      if (mounted) setState(() => _signingOut = false);
    }
  }

  @override
  Widget build(BuildContext context) {
    final session = ref.watch(sessionProvider).asData?.value;
    final user = session is SessionUser ? session.user : null;
    final rider = user?.rider;
    final textTheme = Theme.of(context).textTheme;

    return Scaffold(
      backgroundColor: AppColors.surface,
      appBar: AppBar(
        backgroundColor: AppColors.surface,
        title: Text('Profile', style: AppTheme.d2(textTheme)),
      ),
      body: ListView(
        padding: const EdgeInsets.fromLTRB(16, 8, 16, 24),
        children: [
          // Identity.
          Container(
            padding: const EdgeInsets.all(16),
            decoration: BoxDecoration(
              color: AppColors.surfaceAlt,
              borderRadius: BorderRadius.circular(16),
              border: Border.all(color: AppColors.surfaceBorder),
            ),
            child: Row(
              children: [
                AccentAvatar(text: user?.displayName ?? 'Rider', size: 52),
                const SizedBox(width: 14),
                Expanded(
                  child: Column(
                    crossAxisAlignment: CrossAxisAlignment.start,
                    children: [
                      Text(user?.displayName ?? 'Rider',
                          style: AppTheme.d2(textTheme)),
                      if (user?.phone != null &&
                          user!.phone!.isNotEmpty) ...[
                        const SizedBox(height: 2),
                        Text(user.phone!, style: AppTheme.sub(textTheme)),
                      ],
                    ],
                  ),
                ),
              ],
            ),
          ),
          const SizedBox(height: 12),
          // The rider number — the whole handoff interface.
          Container(
            padding: const EdgeInsets.all(16),
            decoration: BoxDecoration(
              color: AppColors.primary.withValues(alpha: 0.10),
              borderRadius: BorderRadius.circular(16),
              border: Border.all(
                color: AppColors.primary.withValues(alpha: 0.28),
              ),
            ),
            child: Row(
              children: [
                const Icon(Icons.badge_rounded,
                    size: 26, color: AppColors.primary),
                const SizedBox(width: 12),
                // Expanded: the number never overflows the card on a
                // narrow screen (the trailing caption is gone — everyone
                // knows what the rider number is for).
                Expanded(
                  child: Column(
                    crossAxisAlignment: CrossAxisAlignment.start,
                    children: [
                      const MicroLabel('Your rider number'),
                      const SizedBox(height: 2),
                      Text(
                        '#${rider?.riderNumber ?? '—'}',
                        style: textTheme.titleMedium?.copyWith(
                          fontSize: 22,
                          fontWeight: FontWeight.w800,
                          color: AppColors.primary,
                        ),
                      ),
                    ],
                  ),
                ),
              ],
            ),
          ),
          const SizedBox(height: 12),
          // Today's tally — the same numbers the waiting card shows.
          if (_tally != null) ...[
            Container(
              padding: const EdgeInsets.all(16),
              decoration: BoxDecoration(
                color: AppColors.surfaceAlt,
                borderRadius: BorderRadius.circular(16),
                border: Border.all(color: AppColors.surfaceBorder),
              ),
              child: Row(
                children: [
                  Expanded(
                    child: Column(
                      crossAxisAlignment: CrossAxisAlignment.start,
                      children: [
                        const MicroLabel('Today'),
                        const SizedBox(height: 3),
                        Text(
                          '${_tally!.deliveries} deliver${_tally!.deliveries == 1 ? 'y' : 'ies'}'
                          ' · ${formatRwf(_tally!.collected)} collected',
                          style: AppTheme.bd(textTheme)
                              .copyWith(fontWeight: FontWeight.w600),
                        ),
                      ],
                    ),
                  ),
                  const Icon(Icons.payments_rounded,
                      size: 20, color: AppColors.onSurfaceMuted),
                ],
              ),
            ),
            const SizedBox(height: 18),
          ],
          const MicroLabel('Delivered history'),
          const SizedBox(height: 8),
          if (_error != null)
            ErrorState(message: _error!, onRetry: _load)
          else if (_history == null)
            const Center(
              child: Padding(
                padding: EdgeInsets.all(24),
                child: CircularProgressIndicator(),
              ),
            )
          else if (_history!.isEmpty)
            Padding(
              padding: const EdgeInsets.symmetric(vertical: 32),
              child: Text(
                'Nothing delivered yet — your completed runs land here.',
                style: AppTheme.sub(textTheme),
              ),
            )
          else
            Container(
              decoration: BoxDecoration(
                color: AppColors.surfaceAlt,
                borderRadius: BorderRadius.circular(16),
                border: Border.all(color: AppColors.surfaceBorder),
              ),
              child: Column(
                children: [
                  for (var i = 0; i < _history!.length; i++) ...[
                    if (i > 0)
                      Container(
                        height: 1,
                        margin: const EdgeInsets.symmetric(horizontal: 14),
                        color: AppColors.surfaceBorder,
                      ),
                    _HistoryRow(entry: _history![i]),
                  ],
                ],
              ),
            ),
          const SizedBox(height: 24),
          // Sign out lives HERE now — off the kiosk's main screen (the
          // founder's call). Quiet red text, the profile-page pattern.
          Center(
            child: TextButton(
              onPressed: _signingOut ? null : _signOut,
              style: TextButton.styleFrom(
                foregroundColor: AppColors.error,
                textStyle: const TextStyle(
                  fontSize: 14,
                  fontWeight: FontWeight.w600,
                ),
              ),
              child: const Text('Sign out'),
            ),
          ),
        ],
      ),
    );
  }
}

/// One delivered run: order number + store, the destination line, and
/// the cash it collected on the right.
class _HistoryRow extends StatelessWidget {
  const _HistoryRow({required this.entry});

  final RiderHistoryEntry entry;

  @override
  Widget build(BuildContext context) {
    final textTheme = Theme.of(context).textTheme;
    final local = entry.deliveredAt.toLocal();
    final when =
        '${local.year}-${local.month.toString().padLeft(2, '0')}-${local.day.toString().padLeft(2, '0')}'
        ' · ${local.hour.toString().padLeft(2, '0')}:${local.minute.toString().padLeft(2, '0')}';
    return Padding(
      padding: const EdgeInsets.symmetric(horizontal: 14, vertical: 12),
      child: Row(
        children: [
          Expanded(
            child: Column(
              crossAxisAlignment: CrossAxisAlignment.start,
              children: [
                Text(
                  'Order #${entry.number} · ${entry.storeName}',
                  maxLines: 1,
                  overflow: TextOverflow.ellipsis,
                  style: AppTheme.bd(textTheme)
                      .copyWith(fontWeight: FontWeight.w600),
                ),
                const SizedBox(height: 2),
                Text(
                  '${entry.destinationAddress} · $when',
                  maxLines: 1,
                  overflow: TextOverflow.ellipsis,
                  style: AppTheme.sub(textTheme),
                ),
              ],
            ),
          ),
          const SizedBox(width: 10),
          Text(
            formatRwf(entry.total),
            style: textTheme.titleSmall?.copyWith(
              fontSize: 14,
              color: AppColors.success,
              fontWeight: FontWeight.w700,
            ),
          ),
        ],
      ),
    );
  }
}
