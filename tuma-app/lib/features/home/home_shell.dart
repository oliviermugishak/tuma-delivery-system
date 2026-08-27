import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';

import 'package:tuma_app/core/api/api_client.dart';
import 'package:tuma_app/core/auth/auth_controller.dart';
import 'package:tuma_app/core/theme/app_colors.dart';

/// Empty authenticated home shell — the honest placeholder until stores
/// land. Sign-out flips the session to anonymous and the router redirect
/// does the navigating.
class HomeShell extends ConsumerWidget {
  const HomeShell({super.key});

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final session = ref.watch(sessionProvider).asData?.value;
    final user = switch (session) {
      SessionUser s => s.user,
      _ => null,
    };
    final textTheme = Theme.of(context).textTheme;

    return Scaffold(
      appBar: AppBar(
        title: Text('Tuma', style: textTheme.titleLarge),
        actions: [
          IconButton(
            tooltip: 'Sign out',
            icon: const Icon(Icons.logout),
            onPressed: () async {
              await ref.read(sessionProvider.notifier).signOut();
            },
          ),
        ],
      ),
      body: Padding(
        padding: const EdgeInsets.all(24),
        child: Column(
          crossAxisAlignment: CrossAxisAlignment.start,
          children: [
            Text(
              user?.name != null && user!.name!.isNotEmpty
                  ? 'Hi, ${user.name} 👋'
                  : 'Welcome to Tuma 👋',
              style: textTheme.headlineSmall?.copyWith(
                color: AppColors.onSurface,
                fontWeight: FontWeight.w800,
              ),
            ),
            const SizedBox(height: 8),
            Text(
              'You are signed in as ${user?.phone ?? user?.email ?? 'customer'}.',
              style: textTheme.bodyMedium?.copyWith(
                color: AppColors.onSurfaceMuted,
              ),
            ),
            const SizedBox(height: 32),
            Card(
              child: Padding(
                padding: const EdgeInsets.all(20),
                child: Column(
                  crossAxisAlignment: CrossAxisAlignment.start,
                  children: [
                    Text(
                      'Stores are next.',
                      style: textTheme.titleMedium?.copyWith(
                        fontWeight: FontWeight.w700,
                      ),
                    ),
                    const SizedBox(height: 8),
                    Text(
                      'Once stores land, this is where you will find a place to eat.',
                      style: textTheme.bodyMedium?.copyWith(
                        color: AppColors.onSurfaceMuted,
                      ),
                    ),
                  ],
                ),
              ),
            ),
            const Spacer(),
            const _DebugConnectionProbe(),
            const SizedBox(height: 12),
          ],
        ),
      ),
    );
  }
}

/// Sits at the bottom of the home shell. Confirms the wire is live without
/// lying: a 200 says "good", anything else is shown verbatim. Removed when
/// the real home feed arrives.
class _DebugConnectionProbe extends ConsumerStatefulWidget {
  const _DebugConnectionProbe();

  @override
  ConsumerState<_DebugConnectionProbe> createState() => _DebugConnectionProbeState();
}

class _DebugConnectionProbeState extends ConsumerState<_DebugConnectionProbe> {
  String? _status;
  bool _busy = false;

  Future<void> _probe() async {
    setState(() {
      _busy = true;
      _status = null;
    });
    try {
      final api = ref.read(apiClientProvider);
      await api.get('/openapi.json');
      setState(() => _status = 'api reachable');
    } on ApiError catch (error) {
      setState(() => _status = '${error.runtimeType}: ${error.message}');
    } on Object catch (error) {
      setState(() => _status = error.toString());
    } finally {
      if (mounted) setState(() => _busy = false);
    }
  }

  @override
  Widget build(BuildContext context) {
    final textTheme = Theme.of(context).textTheme;
    return Row(
      mainAxisAlignment: MainAxisAlignment.center,
      children: [
        TextButton.icon(
          onPressed: _busy ? null : _probe,
          icon: const Icon(Icons.wifi_tethering),
          label: Text(_busy ? 'Checking…' : 'Check API'),
        ),
        if (_status != null) ...[
          const SizedBox(width: 8),
          Flexible(
            child: Text(
              _status!,
              style: textTheme.bodySmall?.copyWith(
                color: AppColors.onSurfaceMuted,
              ),
              overflow: TextOverflow.ellipsis,
            ),
          ),
        ],
      ],
    );
  }
}
