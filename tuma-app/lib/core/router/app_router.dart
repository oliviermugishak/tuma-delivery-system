import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:go_router/go_router.dart';

import 'package:tuma_app/core/api/models/address.dart';
import 'package:tuma_app/core/auth/auth_controller.dart';
import 'package:tuma_app/features/auth/name_screen.dart';
import 'package:tuma_app/features/auth/otp_screen.dart';
import 'package:tuma_app/features/auth/phone_screen.dart';
import 'package:tuma_app/features/auth/splash_screen.dart';
import 'package:tuma_app/features/cart/cart_view.dart';
import 'package:tuma_app/features/cart/checkout_screen.dart';
import 'package:tuma_app/features/orders/success_screen.dart';
import 'package:tuma_app/features/home/app_shell.dart';
import 'package:tuma_app/features/orders/order_detail_screen.dart';
import 'package:tuma_app/features/profile/location_screen.dart';
import 'package:tuma_app/features/rider/rider_profile_screen.dart';
import 'package:tuma_app/features/rider/rider_screen.dart';
import 'package:tuma_app/features/store/store_screen.dart';

/// Build a fresh router. A factory so tests and hot-restarts never reuse a
/// router whose internal location has moved on (a singleton keeps its last
/// route forever, which breaks any fresh mount).
GoRouter buildRouter() => GoRouter(
      initialLocation: '/',
      redirect: (context, state) {
        final container = ProviderScope.containerOf(context, listen: false);
        final session = container.read(sessionProvider);
        final sub = state.matchedLocation;

        // The OTP and name screens carry their arguments in `extra`; a cold
        // start on them has none — bounce back to the start of the flow.
        // `/success` is the same class: no args = nothing to celebrate.
        if ((sub == '/auth/otp' || sub == '/auth/name' || sub == '/success') &&
            state.extra == null) {
          return sub == '/success' ? '/home' : '/auth/phone';
        }

        return session.when(
          data: (snapshot) {
            // Loading: stay put — the splash is still working.
            if (snapshot is SessionLoading) return null;
            final onAuthFlow = sub.startsWith('/auth');
            if (snapshot is SessionUser) {
              // Rider mode is the rider's surface: they start (and stay)
              // there — the kiosk and its profile page are the rider's
              // two places — and a customer never sees either.
              final user = snapshot.user;
              if (user.isRider) {
                return (sub == '/rider' || sub == '/rider/profile')
                    ? null
                    : '/rider';
              }
              if (sub.startsWith('/rider')) return '/home';
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
    GoRoute(path: '/home', builder: (_, _) => const AppShell()),
    GoRoute(
      path: '/stores/:id',
      builder: (_, state) =>
          StoreScreen(storeId: state.pathParameters['id']!),
    ),
    GoRoute(path: '/cart', builder: (_, _) => const CartScreen()),
    GoRoute(path: '/checkout', builder: (_, _) => const CheckoutScreen()),
    GoRoute(
      path: '/success',
      builder: (_, state) {
        final args = state.extra! as SuccessScreenArgs;
        return SuccessScreen(
          groupId: args.groupId,
          orderNumber: args.orderNumber,
          storeName: args.storeName,
          total: args.total,
          etaMinutes: args.etaMinutes,
        );
      },
    ),
    GoRoute(
      path: '/orders/:id',
      builder: (_, state) => OrderDetailScreen(
        orderId: state.pathParameters['id']!,
      ),
    ),
    GoRoute(
      path: '/profile/location',
      builder: (_, state) =>
          DeliveryLocationScreen(edit: state.extra as Address?),
    ),
    GoRoute(path: '/rider', builder: (_, _) => const RiderScreen()),
    GoRoute(
        path: '/rider/profile', builder: (_, _) => const RiderProfileScreen()),
      ],
    );

/// The app's router instance (see [buildRouter] for why it is a factory).
final GoRouter appRouter = buildRouter();
