import 'package:flutter/material.dart';

import 'package:tuma_app/core/theme/app_colors.dart';
import 'package:tuma_app/features/home/home_screen.dart';
import 'package:tuma_app/features/profile/profile_screen.dart';

/// The authenticated shell: bottom bar with Home • Profile (blueprint 7.1,
/// minus Search/Orders until those features exist — destinations slot in
/// without restructuring). The body is an IndexedStack so the home feed
/// stays alive while the profile tab is open.
class AppShell extends StatefulWidget {
  const AppShell({super.key});

  @override
  State<AppShell> createState() => _AppShellState();
}

class _AppShellState extends State<AppShell> {
  int _tab = 0;

  @override
  Widget build(BuildContext context) {
    final textTheme = Theme.of(context).textTheme;
    return Scaffold(
      body: IndexedStack(
        index: _tab,
        children: const [HomeScreen(), ProfileScreen()],
      ),
      bottomNavigationBar: DecoratedBox(
        decoration: const BoxDecoration(
          border: Border(top: BorderSide(color: AppColors.surfaceBorder)),
        ),
        child: NavigationBarTheme(
          data: NavigationBarThemeData(
            backgroundColor: AppColors.surfaceAlt,
            indicatorColor: AppColors.primary.withValues(alpha: 0.12),
            height: 68,
            iconTheme: WidgetStateProperty.resolveWith((states) {
              final selected = states.contains(WidgetState.selected);
              return IconThemeData(
                color: selected ? AppColors.primary : AppColors.onSurfaceMuted,
                size: 24,
              );
            }),
            labelTextStyle: WidgetStateProperty.resolveWith((states) {
              final selected = states.contains(WidgetState.selected);
              return (textTheme.labelMedium ?? const TextStyle()).copyWith(
                fontWeight: selected ? FontWeight.w600 : FontWeight.w500,
                color: selected ? AppColors.primary : AppColors.onSurfaceMuted,
              );
            }),
          ),
          child: NavigationBar(
            elevation: 0,
            selectedIndex: _tab,
            onDestinationSelected: (index) => setState(() => _tab = index),
            destinations: const [
              NavigationDestination(
                icon: Icon(Icons.storefront_outlined),
                selectedIcon: Icon(Icons.storefront),
                label: 'Home',
              ),
              NavigationDestination(
                icon: Icon(Icons.person_outline),
                selectedIcon: Icon(Icons.person),
                label: 'Profile',
              ),
            ],
          ),
        ),
      ),
    );
  }
}
