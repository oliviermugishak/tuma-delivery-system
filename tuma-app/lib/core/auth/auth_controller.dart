import 'package:flutter/foundation.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';

import 'package:tuma_app/core/api/api_client.dart';
import 'package:tuma_app/core/api/auth_api.dart';
import 'package:tuma_app/core/api/models/authenticated_user.dart';
import 'package:tuma_app/core/auth/token_storage.dart';

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

/// Api client that always reads the current token from the notifier.
final apiClientProvider = Provider<ApiClient>((ref) {
  return ApiClient(tokenProvider: () => ref.read(authTokenProvider));
});

final authApiProvider = Provider<AuthApi>(
  (ref) => AuthApi(ref.read(apiClientProvider)),
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

  Future<void> signOut() async {
    try {
      await ref.read(authApiProvider).logout();
    } on Object {
      // Server logout is best-effort: a stale token is fine to drop locally.
    }
    await ref.read(authTokenProvider.notifier).clear();
    _set(const SessionState.anon());
  }
}

final sessionProvider =
    AsyncNotifierProvider<SessionNotifier, SessionState>(SessionNotifier.new);
