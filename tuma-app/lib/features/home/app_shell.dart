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
/// checkout lands on the Orders tab, the empty cart routes to the Cart
/// tab, and so on.
class ShellTabNotifier extends Notifier<int> {
  @override
  int build() => 0;

  void select(int index) => state = index;
}

final shellTabProvider =
    NotifierProvider<ShellTabNotifier, int>(ShellTabNotifier.new);

/// The five-tab shell, the redesign's exact chrome: pill-highlighted
/// icons (the active tab's icon sits in a surface-high pill), labels
/// under, and the cart's live accent badge. Tabs live in one
/// IndexedStack — switching keeps every tab's scroll and state alive.
class AppShell extends ConsumerWidget {
  const AppShell({super.key});

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final tab = ref.watch(shellTabProvider);
    return Scaffold(
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
          color: AppColors.navSurface,
          border: Border(top: BorderSide(color: AppColors.surfaceBorder)),
        ),
        child: SafeArea(
          top: false,
          child: Padding(
            padding: const EdgeInsets.fromLTRB(6, 10, 6, 18),
            child: Row(
              mainAxisAlignment: MainAxisAlignment.spaceAround,
              crossAxisAlignment: CrossAxisAlignment.start,
              children: [
                _NavItem(
                  icon: Icons.storefront_rounded,
                  label: 'Home',
                  selected: tab == 0,
                  onTap: () => ref.read(shellTabProvider.notifier).select(0),
                ),
                _NavItem(
                  icon: Icons.search_rounded,
                  label: 'Search',
                  selected: tab == 1,
                  onTap: () => ref.read(shellTabProvider.notifier).select(1),
                ),
                _NavItem(
                  icon: Icons.receipt_long_rounded,
                  label: 'Orders',
                  selected: tab == 2,
                  onTap: () => ref.read(shellTabProvider.notifier).select(2),
                ),
                _CartNavItem(
                  selected: tab == 3,
                  onTap: () => ref.read(shellTabProvider.notifier).select(3),
                ),
                _NavItem(
                  icon: Icons.person_rounded,
                  label: 'Profile',
                  selected: tab == 4,
                  onTap: () => ref.read(shellTabProvider.notifier).select(4),
                ),
              ],
            ),
          ),
        ),
      ),
    );
  }
}

class _NavItem extends StatelessWidget {
  const _NavItem({
    required this.icon,
    required this.label,
    required this.selected,
    required this.onTap,
    this.badge,
  });

  final IconData icon;
  final String label;
  final bool selected;
  final VoidCallback onTap;
  final String? badge;

  @override
  Widget build(BuildContext context) {
    final color =
        selected ? AppColors.primary : AppColors.onSurfaceMuted;
    return InkWell(
      onTap: onTap,
      borderRadius: BorderRadius.circular(999),
      child: Column(
        mainAxisSize: MainAxisSize.min,
        children: [
          Container(
            padding: const EdgeInsets.symmetric(horizontal: 14, vertical: 6),
            decoration: BoxDecoration(
              color: selected ? AppColors.surfaceHigh : Colors.transparent,
              borderRadius: BorderRadius.circular(999),
            ),
            child: Stack(
              clipBehavior: Clip.none,
              children: [
                Icon(icon, size: 24, color: color),
                if (badge != null)
                  Positioned(
                    top: -4,
                    right: -8,
                    child: Container(
                      constraints:
                          const BoxConstraints(minWidth: 16, minHeight: 16),
                      padding: const EdgeInsets.symmetric(horizontal: 4),
                      decoration: const BoxDecoration(
                        color: AppColors.primary,
                        shape: BoxShape.circle,
                      ),
                      alignment: Alignment.center,
                      child: Text(
                        badge!,
                        style: const TextStyle(
                          color: AppColors.onPrimary,
                          fontSize: 10,
                          fontWeight: FontWeight.w700,
                          height: 16 / 10,
                        ),
                        textAlign: TextAlign.center,
                      ),
                    ),
                  ),
              ],
            ),
          ),
          const SizedBox(height: 5),
          Text(
            label,
            style: TextStyle(
              fontSize: 11,
              fontWeight: FontWeight.w600,
              color: color,
            ),
          ),
        ],
      ),
    );
  }
}

/// The Cart tab item with the live item-count badge (the redesign's
/// yellow cbadge).
class _CartNavItem extends ConsumerWidget {
  const _CartNavItem({required this.selected, required this.onTap});

  final bool selected;
  final VoidCallback onTap;

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final cart = ref.watch(cartProvider);
    final count = cart.maybeWhen(
      data: (v) => v.itemCount,
      orElse: () => 0,
    );
    return _NavItem(
      icon: Icons.shopping_cart_rounded,
      label: 'Cart',
      selected: selected,
      onTap: onTap,
      badge: count > 0 ? (count > 99 ? '99+' : '$count') : null,
    );
  }
}
