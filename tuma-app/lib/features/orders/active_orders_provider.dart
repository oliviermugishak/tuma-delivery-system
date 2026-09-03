import 'dart:async';

import 'package:flutter/material.dart';
import 'package:flutter/widgets.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';

import 'package:tuma_app/core/api/api_client.dart';
import 'package:tuma_app/core/api/models/order.dart';
import 'package:tuma_app/core/auth/auth_controller.dart';
import 'package:tuma_app/core/theme/app_colors.dart';

/// How often the in-flight order surfaces re-fetch — the same live
/// cadence the tracking screen polls at.
const _liveInterval = Duration(seconds: 5);

/// The one realtime heartbeat for the customer's order surfaces (Home's
/// live card, the Orders tabs). It fetches the group list once on first
/// watch, then ticks every 5s WHILE anything is in flight and stops the
/// moment the last order settles — polling only exists when there is
/// something to watch.
///
/// Silent by design: a failed refresh keeps the last good list (no
/// spinner, no content wipe — a refresh that flashes is jank, not
/// honesty), and the app-backgrounded poller stops (this is not a
/// background service; it resumes with the app). Watching the session
/// means a sign-out/sign-in rebuilds it for the right account.
class ActiveOrdersNotifier extends AsyncNotifier<List<GroupSummary>>
    with WidgetsBindingObserver {
  Timer? _timer;
  bool _paused = false;

  @override
  Future<List<GroupSummary>> build() async {
    ref.onDispose(() {
      _timer?.cancel();
      _timer = null;
      WidgetsBinding.instance.removeObserver(this);
    });
    WidgetsBinding.instance.addObserver(this);
    // The session is the account boundary: a fresh sign-in (or a sign-out)
    // re-runs this build for whoever is actually holding the phone.
    ref.watch(sessionProvider);
    return _fetch();
  }

  bool _inFlight(List<GroupSummary> groups) => groups.any(
        (g) => g.status == 'in_progress' || g.status == 'partially_fulfilled',
      );

  /// (Re)arms or disarms the poll to exactly match [groups] + pause state
  /// — the single point of truth for the timer, like the rider kiosk's
  /// push loop.
  void _syncPoll(List<GroupSummary> groups) {
    if (!_paused && _inFlight(groups)) {
      _timer ??= Timer.periodic(_liveInterval, (_) => unawaited(_tick()));
    } else {
      _timer?.cancel();
      _timer = null;
    }
  }

  Future<List<GroupSummary>> _fetch() async {
    try {
      final groups = await ref.read(orderApiProvider).listGroups();
      _syncPoll(groups);
      return groups;
    } on ApiUnauthorized {
      // The session died mid-poll: stop, and let the global 401 handler
      // own the sign-out. A dead session must not keep knocking.
      _timer?.cancel();
      _timer = null;
      rethrow;
    }
  }

  /// One poll tick. A failure keeps the last good list and the cadence —
  /// the next tick retries. An UNCHANGED list is not written: every write
  /// rebuilds every watcher's card (review P18), so the tick compares
  /// ids + statuses + etaTargets first.
  Future<void> _tick() async {
    if (_timer == null) return;
    try {
      final groups = await ref.read(orderApiProvider).listGroups();
      if (!ref.mounted) return;
      final current = state.asData?.value;
      if (current != null && _sameList(current, groups)) {
        _syncPoll(groups);
        return;
      }
      state = AsyncData(groups);
      _syncPoll(groups);
    } on Object {
      // Keep polling; the rendered list stays honest-stale.
    }
  }

  /// Equality on what the cards actually render: identity, status, and
  /// the ETA the countdown decays against.
  bool _sameList(List<GroupSummary> a, List<GroupSummary> b) {
    if (a.length != b.length) return false;
    for (var i = 0; i < a.length; i++) {
      if (a[i].id != b[i].id ||
          a[i].status != b[i].status ||
          a[i].etaTarget != b[i].etaTarget) {
        return false;
      }
    }
    return true;
  }

  /// A silent refresh for events that create or change orders right now
  /// (just-placed checkout). Unlike [refresh], no loading state — the
  /// data swaps in place.
  Future<void> poke() async {
    try {
      final groups = await _fetch();
      if (!ref.mounted) return;
      state = AsyncData(groups);
    } on Object {
      // The next tick (or pull) retries.
    }
  }

  @override
  void didChangeAppLifecycleState(AppLifecycleState state) {
    // Foreground-only, the app's promise: backgrounded, the poll stops;
    // resumed, an immediate fetch re-arms it if anything is still moving.
    if (state == AppLifecycleState.paused ||
        state == AppLifecycleState.hidden) {
      _paused = true;
      _timer?.cancel();
      _timer = null;
    } else if (state == AppLifecycleState.resumed) {
      _paused = false;
      unawaited(poke());
    }
  }
}

final activeOrdersProvider = AsyncNotifierProvider<ActiveOrdersNotifier,
    List<GroupSummary>>(ActiveOrdersNotifier.new);

/// The active group's status story — color + words + progress for the
/// dot row and progress bar, derived from the server's own summary (the
/// ETA target means the delivery is actually moving). One derivation for
/// every surface (Home's live card, the Orders Active card), never two.
/// v3 adds the calm tier: preparing/confirmed get a neutral dot and
/// muted words — non-urgent states stop borrowing the action color.
({Color color, Color? textColor, String label, double progress, bool pulsing})
    activeOrderStory(GroupSummary group) {
  switch (group.status) {
    case 'partially_fulfilled':
      return (
        color: AppColors.primary,
        textColor: null,
        label: 'Out for delivery',
        progress: 0.65,
        pulsing: true,
      );
    case 'in_progress':
      if (group.etaTarget != null) {
        return (
          color: AppColors.primary,
          textColor: null,
          label: 'Out for delivery',
          progress: 0.65,
          pulsing: true,
        );
      }
      return (
        color: AppColors.dotNeutral,
        textColor: AppColors.onSurfaceMuted,
        label: 'Preparing',
        progress: 0.35,
        pulsing: false,
      );
    default:
      return (
        color: AppColors.dotNeutral,
        textColor: AppColors.onSurfaceMuted,
        label: 'Confirmed',
        progress: 0.15,
        pulsing: false,
      );
  }
}

/// True while the group is still moving — the poll's watch-set.
bool groupInFlight(GroupSummary group) =>
    group.status == 'in_progress' || group.status == 'partially_fulfilled';
