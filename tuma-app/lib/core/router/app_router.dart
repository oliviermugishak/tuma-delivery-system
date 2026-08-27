import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:go_router/go_router.dart';

import 'package:tuma_app/core/auth/auth_controller.dart';
import 'package:tuma_app/features/auth/name_screen.dart';
import 'package:tuma_app/features/auth/otp_screen.dart';
import 'package:tuma_app/features/auth/phone_screen.dart';
import 'package:tuma_app/features/auth/splash_screen.dart';
import 'package:tuma_app/features/home/home_shell.dart';

final GoRouter appRouter = GoRouter(
  initialLocation: '/',
  redirect: (context, state) {
    final container = ProviderScope.containerOf(context, listen: false);
    final session = container.read(sessionProvider);
    final sub = state.matchedLocation;

    // The OTP and name screens carry their arguments in `extra`; a cold
    // start on them has none — bounce back to the start of the flow.
    if ((sub == '/auth/otp' || sub == '/auth/name') && state.extra == null) {
      return '/auth/phone';
    }

    return session.when(
      data: (snapshot) {
        // Loading: stay put — the splash is still working.
        if (snapshot is SessionLoading) return null;
        final onAuthFlow = sub.startsWith('/auth');
        if (snapshot is SessionUser) {
          return onAuthFlow || sub == '/' ? '/home' : null;
        }
        // Anonymous: the auth flow is the only place to be.
        return onAuthFlow ? null : '/auth/phone';
      },
      loading: () => null,
      error: (_, _) => '/auth/phone',
    );
  },
  refreshListenable: sessionRouterRefresher,
  routes: [
    GoRoute(path: '/', builder: (_, _) => const SplashScreen()),
    GoRoute(path: '/auth/phone', builder: (_, _) => const PhoneScreen()),
    GoRoute(
      path: '/auth/otp',
      builder: (_, state) => OtpScreen(phone: state.extra! as String),
    ),
    GoRoute(
      path: '/auth/name',
      builder: (_, state) => NameScreen(args: state.extra! as NameScreenArgs),
    ),
    GoRoute(path: '/home', builder: (_, _) => const HomeShell()),
  ],
);
