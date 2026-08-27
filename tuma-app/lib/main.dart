import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';

import 'package:tuma_app/core/router/app_router.dart';
import 'package:tuma_app/core/theme/app_theme.dart';

void main() {
  runApp(const ProviderScope(child: TumaApp()));
}

class TumaApp extends ConsumerWidget {
  const TumaApp({super.key});

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    return MaterialApp.router(
      title: 'Tuma',
      debugShowCheckedModeBanner: false,
      theme: AppTheme.light(),
      routerConfig: appRouter,
    );
  }
}
