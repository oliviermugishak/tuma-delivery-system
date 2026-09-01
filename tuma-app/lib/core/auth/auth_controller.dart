import 'dart:async';

import 'package:flutter/foundation.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';

import 'package:tuma_app/core/api/api_client.dart';
import 'package:tuma_app/core/api/auth_api.dart';
import 'package:tuma_app/core/api/models/authenticated_user.dart';
import 'package:tuma_app/core/api/address_api.dart';
import 'package:tuma_app/core/api/geo_api.dart';
import 'package:tuma_app/core/api/rider_api.dart';
import 'package:tuma_app/core/api/store_api.dart';
import 'package:tuma_app/core/api/order_api.dart';
import 'package:tuma_app/core/auth/token_storage.dart';
import 'package:tuma_app/features/cart/cart_notifier.dart';
import 'package:tuma_app/features/location/customer_location.dart';
import 'package:tuma_app/features/search/search_screen.dart' as search;

/// Holds the in-memory bearer token. `null` = anonymous. Hydrated on
/// bootstrap from secure storage and cleared on logout.
class AuthTokenNotifier extends Notifier<String?> {
  @override
  String? build() => null;

  Future<void> set(String token) async {
    await ref.read(tokenStorageProvider).writeToken(token);
    state = token;
  }

  Future<void> clear() async {
    await ref.read(tokenStorageProvider).clear();
    state = null;
  }
}

final tokenStorageProvider = Provider<TokenStorage>((ref) => TokenStorage());

final authTokenProvider =
    NotifierProvider<AuthTokenNotifier, String?>(AuthTokenNotifier.new);

/// Api client that always reads the current token from the notifier —
/// and dies loudly on 401: an expired token clears the session (cart,
/// pin, recents) and the router lands on auth, instead of every screen
/// showing "You need to sign in" forever. The `dying` guard breaks the
/// loop: signOut's own logout() call re-enters here on the stale 401.
final apiClientProvider = Provider<ApiClient>((ref) {
  var dying = false;
  return ApiClient(
    tokenProvider: () => ref.read(authTokenProvider),
    onUnauthorized: () {
      if (dying) return;
      dying = true;
      unawaited(
        ref
            .read(sessionProvider.notifier)
            .signOut()
            .whenComplete(() => dying = false),
      );
    },
  );
});

final authApiProvider = Provider<AuthApi>(
  (ref) => AuthApi(ref.read(apiClientProvider)),
);

final storeApiProvider = Provider<StoreApi>(
  (ref) => StoreApi(ref.read(apiClientProvider)),
);

final orderApiProvider = Provider<OrderApi>(
  (ref) => OrderApi(ref.read(apiClientProvider)),
);

final riderApiProvider = Provider<RiderApi>(
  (ref) => RiderApi(ref.read(apiClientProvider)),
);

final geoApiProvider = Provider<GeoApi>(
  (ref) => GeoApi(ref.read(apiClientProvider)),
);

final addressApiProvider = Provider<AddressApi>(
  (ref) => AddressApi(ref.read(apiClientProvider)),
);

/// What we know about the current session.
sealed class SessionState {
  const SessionState();

  const factory SessionState.loading() = SessionLoading;
  const factory SessionState.anon() = SessionAnon;
  const factory SessionState.user(AuthenticatedUser user) = SessionUser;
}

class SessionLoading extends SessionState {
  const SessionLoading();
}

class SessionAnon extends SessionState {
  const SessionAnon();
}

class SessionUser extends SessionState {
  const SessionUser(this.user);
  final AuthenticatedUser user;
}

/// Bridges session changes to the router. GoRouter is a top-level global
/// (built outside the widget tree), so it cannot watch the ProviderScope
/// directly; [SessionNotifier] pokes this on every transition and the router
/// uses it as its `refreshListenable`.
class SessionRouterRefresher extends ChangeNotifier {
  void notify() => notifyListeners();
}

final SessionRouterRefresher sessionRouterRefresher = SessionRouterRefresher();

/// Hydrates the session on app boot.
class SessionNotifier extends AsyncNotifier<SessionState> {
  @override
  Future<SessionState> build() async => const SessionState.loading();

  void _set(SessionState value) {
    state = AsyncData(value);
    sessionRouterRefresher.notify();
  }

  Future<void> bootstrap() async {
    // Let the splash animation breathe even when hydration is instant.
    final minimumSplash = Future<void>.delayed(const Duration(milliseconds: 1200));

    SessionState outcome;
    final storage = ref.read(tokenStorageProvider);
    String? token;
    try {
      token = await storage.readToken();
    } on Object {
      // Secure storage can fail (keystore locked, libsecret missing).
      // Degrade to anonymous instead of hanging on the splash forever.
      token = null;
    }
    if (token == null) {
      outcome = const SessionState.anon();
    } else {
      await ref.read(authTokenProvider.notifier).set(token);
      try {
        final user = await ref.read(authApiProvider).me();
        outcome = SessionState.user(user);
      } on Object {
        await ref.read(authTokenProvider.notifier).clear();
        outcome = const SessionState.anon();
      }
    }
    await minimumSplash;
    _set(outcome);
  }

  Future<void> accept({required String token, required AuthenticatedUser user}) async {
    await ref.read(authTokenProvider.notifier).set(token);
    _set(SessionState.user(user));
  }

  /// Replaces the known user after a profile edit (PATCH /me) so every
  /// screen reading the session — the greeting, the identity card —
  /// reflects the change without a re-fetch.
  void updateUser(AuthenticatedUser user) => _set(SessionState.user(user));

  Future<void> signOut() async {
    try {
      await ref.read(authApiProvider).logout();
    } on Object {
      // Server logout is best-effort: a stale token is fine to drop locally.
    }
    // End-of-session hygiene: everything that belongs to one customer
    // goes with them (the docs on both promise this) — cart, delivery
    // pin, and the search recents that joined in the discovery slice.
    await ref.read(cartProvider.notifier).clear();
    await ref.read(customerLocationProvider.notifier).clear();
    await search.clearRecentSearches();
    await ref.read(authTokenProvider.notifier).clear();
    _set(const SessionState.anon());
  }
}

final sessionProvider =
    AsyncNotifierProvider<SessionNotifier, SessionState>(SessionNotifier.new);
