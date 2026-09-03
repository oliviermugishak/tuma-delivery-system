import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';

import 'package:tuma_app/core/auth/auth_controller.dart';
import 'package:tuma_app/core/router/app_router.dart';
import 'package:tuma_app/core/theme/app_theme.dart';

void main() {
  runApp(const ProviderScope(child: TumaApp()));
}

class TumaApp extends ConsumerStatefulWidget {
  const TumaApp({super.key});

  @override
  ConsumerState<TumaApp> createState() => _TumaAppState();
}

class _TumaAppState extends ConsumerState<TumaApp> {
  @override
  void initState() {
    super.initState();
    // Hydrate the session once the provider tree is alive. The router's
    // redirect observes the session and reroutes to home or splash.
    WidgetsBinding.instance.addPostFrameCallback((_) {
      ref.read(sessionProvider.notifier).bootstrap();
    });
  }

  @override
  Widget build(BuildContext context) {
    return AnnotatedRegion<SystemUiOverlayStyle>(
      value: SystemUiOverlayStyle.dark,
      child: MaterialApp.router(
        title: 'Tuma',
        debugShowCheckedModeBanner: false,
        theme: AppTheme.light(),
        routerConfig: appRouter,
      ),
    );
  }
}
