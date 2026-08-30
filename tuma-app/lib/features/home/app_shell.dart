import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';

import 'package:tuma_app/core/theme/app_colors.dart';
import 'package:tuma_app/features/cart/cart_notifier.dart';
import 'package:tuma_app/features/cart/cart_view.dart';
import 'package:tuma_app/features/home/home_screen.dart';
import 'package:tuma_app/features/orders/orders_screen.dart';
import 'package:tuma_app/features/profile/profile_screen.dart';
import 'package:tuma_app/features/search/search_screen.dart';

/// Which tab the shell shows. Global so screens can navigate by tab —
/// the home search field is a door to the Search tab, not its own
/// controller.
class ShellTabNotifier extends Notifier<int> {
  @override
  int build() => 0;

  void select(int index) => state = index;
}

final shellTabProvider =
    NotifierProvider<ShellTabNotifier, int>(ShellTabNotifier.new);

/// The five-tab shell: browse (Home), discovery (Search), history
/// (Orders), the cart, and identity (Profile). Tabs live in one
/// IndexedStack — switching keeps every tab's scroll and state alive.
class AppShell extends ConsumerWidget {
  const AppShell({super.key});

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final tab = ref.watch(shellTabProvider);
    final textTheme = Theme.of(context).textTheme;
    return Scaffold(
      // Each tab owns its body; no shell appBar — the cart lives in the
      // bottom bar where the user actually looks for it.
      body: IndexedStack(
        index: tab,
        children: const [
          HomeScreen(),
          SearchScreen(),
          OrdersScreen(),
          CartView(),
          ProfileScreen(),
        ],
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
            selectedIndex: tab,
            onDestinationSelected: (index) =>
                ref.read(shellTabProvider.notifier).select(index),
            destinations: const [
              NavigationDestination(
                icon: Icon(Icons.storefront_outlined),
                selectedIcon: Icon(Icons.storefront),
                label: 'Home',
              ),
              NavigationDestination(
                icon: Icon(Icons.search_rounded),
                selectedIcon: Icon(Icons.search),
                label: 'Search',
              ),
              NavigationDestination(
                icon: Icon(Icons.receipt_long_rounded),
                selectedIcon: Icon(Icons.receipt_long),
                label: 'Orders',
              ),
              _CartDestination(),
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

/// The Cart tab icon with a live item-count badge — visible feedback that
/// something is in the cart, right where the user expects it.
class _CartDestination extends ConsumerWidget {
  const _CartDestination();

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final cart = ref.watch(cartProvider);
    final count = cart.maybeWhen(
      data: (v) => v.itemCount,
      orElse: () => 0,
    );
    return NavigationDestination(
      icon: _icon(Icons.shopping_cart_outlined, count),
      selectedIcon: _icon(Icons.shopping_cart, count),
      label: 'Cart',
    );
  }

  Widget _icon(IconData icon, int count) {
    return Stack(
      clipBehavior: Clip.none,
      children: [
        Icon(icon),
        if (count > 0)
          Positioned(
            right: -6,
            top: -6,
            child: Container(
              constraints: const BoxConstraints(minWidth: 16, minHeight: 16),
              padding: const EdgeInsets.symmetric(horizontal: 3),
              decoration: const BoxDecoration(
                color: AppColors.primary,
                shape: BoxShape.circle,
              ),
              alignment: Alignment.center,
              child: Text(
                count > 99 ? '99+' : '$count',
                style: const TextStyle(
                  color: AppColors.onPrimary,
                  fontSize: 9,
                  fontWeight: FontWeight.w700,
                ),
                textAlign: TextAlign.center,
              ),
            ),
          ),
      ],
    );
  }
}
