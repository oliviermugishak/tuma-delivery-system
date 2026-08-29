import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';

import 'package:tuma_app/core/auth/auth_controller.dart';
import 'package:tuma_app/core/theme/app_colors.dart';
import 'package:tuma_app/shared/widgets/initials_tile.dart';

/// Profile tab — the honest V1 minimum: who you are and sign out.
/// Addresses, payment methods and favorites arrive with orders; no stub
/// rows until then. The inline name edit lands in S16.
class ProfileScreen extends ConsumerStatefulWidget {
  const ProfileScreen({super.key});

  @override
  ConsumerState<ProfileScreen> createState() => _ProfileScreenState();
}

class _ProfileScreenState extends ConsumerState<ProfileScreen> {
  bool _signingOut = false;

  Future<void> _signOut() async {
    setState(() => _signingOut = true);
    await ref.read(sessionProvider.notifier).signOut();
    // The router redirect does the navigating; re-arm only if we somehow
    // survive the transition.
    if (mounted) setState(() => _signingOut = false);
  }

  @override
  Widget build(BuildContext context) {
    final session = ref.watch(sessionProvider).asData?.value;
    final user = switch (session) {
      SessionUser s => s.user,
      _ => null,
    };
    final textTheme = Theme.of(context).textTheme;
    final name = user?.displayName;
    final phone = user?.phone;
    final identity = name ?? phone ?? 'Customer';

    return Scaffold(
      body: SafeArea(
        child: ListView(
          padding: const EdgeInsets.all(24),
          children: [
            Text(
              'Profile',
              style: textTheme.headlineSmall?.copyWith(
                color: AppColors.onSurface,
                fontWeight: FontWeight.w800,
              ),
            ),
            const SizedBox(height: 20),
            Card(
              child: Padding(
                padding: const EdgeInsets.all(20),
                child: Row(
                  children: [
                    InitialsTile(text: identity),
                    const SizedBox(width: 16),
                    Expanded(
                      child: Column(
                        crossAxisAlignment: CrossAxisAlignment.start,
                        children: [
                          Text(
                            identity,
                            maxLines: 1,
                            overflow: TextOverflow.ellipsis,
                            style: textTheme.titleMedium?.copyWith(
                              fontWeight: FontWeight.w700,
                            ),
                          ),
                          if (name != null && phone != null) ...[
                            const SizedBox(height: 4),
                            Text(
                              phone,
                              style: textTheme.bodySmall?.copyWith(
                                color: AppColors.onSurfaceMuted,
                              ),
                            ),
                          ],
                        ],
                      ),
                    ),
                  ],
                ),
              ),
            ),
            const SizedBox(height: 16),
            SizedBox(
              width: double.infinity,
              height: 52,
              child: OutlinedButton.icon(
                onPressed: _signingOut ? null : _signOut,
                style: OutlinedButton.styleFrom(
                  foregroundColor: AppColors.onSurface,
                  side: const BorderSide(color: AppColors.surfaceBorder),
                  shape: RoundedRectangleBorder(
                    borderRadius: BorderRadius.circular(12),
                  ),
                  textStyle: textTheme.labelLarge?.copyWith(
                    fontWeight: FontWeight.w600,
                  ),
                ),
                icon: _signingOut
                    ? const SizedBox(
                        width: 18,
                        height: 18,
                        child: CircularProgressIndicator(
                          strokeWidth: 2.5,
                          color: AppColors.onSurfaceMuted,
                        ),
                      )
                    : const Icon(Icons.logout_rounded, size: 20),
                label: Text(_signingOut ? 'Signing out…' : 'Sign out'),
              ),
            ),
          ],
        ),
      ),
    );
  }
}
