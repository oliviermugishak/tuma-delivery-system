import 'dart:async';

import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:go_router/go_router.dart';

import 'package:tuma_app/core/api/api_client.dart';
import 'package:tuma_app/core/api/models/order.dart';
import 'package:tuma_app/core/auth/auth_controller.dart';
import 'package:tuma_app/core/theme/app_colors.dart';
import 'package:tuma_app/core/utils/format_rwf.dart';
import 'package:tuma_app/shared/widgets/error_state.dart';

/// Order history — one card per checkout (an order group), newest first.
/// A group can span several stores; the card shows who is fulfilling it
/// and the derived overall state. Tapping opens the group detail.
class OrdersScreen extends ConsumerStatefulWidget {
  const OrdersScreen({super.key});

  @override
  ConsumerState<OrdersScreen> createState() => _OrdersScreenState();
}

class _OrdersScreenState extends ConsumerState<OrdersScreen> {
  List<GroupSummary>? _orders;
  String? _error;

  Future<void> _load() async {
    setState(() {
      _error = null;
      _orders = null;
    });
    try {
      final orders = await ref.read(orderApiProvider).listGroups();
      if (!mounted) return;
      setState(() => _orders = orders);
    } on ApiError catch (e) {
      if (!mounted) return;
      setState(() => _error = e.message);
    }
  }

  @override
  void initState() {
    super.initState();
    unawaited(_load());
  }

  @override
  Widget build(BuildContext context) {
    final textTheme = Theme.of(context).textTheme;
    return Scaffold(
      body: SafeArea(
        child: Column(
          crossAxisAlignment: CrossAxisAlignment.start,
          children: [
            // Page title, always visible — same voice as Home and Profile.
            Padding(
              padding: const EdgeInsets.fromLTRB(24, 24, 24, 8),
              child: Text(
                'Your Orders',
                style: textTheme.headlineSmall?.copyWith(
                  color: AppColors.onSurface,
                  fontWeight: FontWeight.w800,
                ),
              ),
            ),
            Expanded(child: _body()),
          ],
        ),
      ),
    );
  }

  Widget _body() {
    if (_orders == null && _error == null) {
      return const Center(child: CircularProgressIndicator());
    }
    if (_orders == null && _error != null) {
      return ErrorState(
        message: _error!,
        onRetry: () => unawaited(_load()),
      );
    }
    if (_orders?.isEmpty ?? false) {
      return const _EmptyOrders();
    }

    return RefreshIndicator(
      onRefresh: () async => _load(),
      child: ListView.separated(
        padding: const EdgeInsets.only(top: 8),
        itemCount: _orders!.length,
        separatorBuilder: (ctx, i) =>
            Divider(height: 1, thickness: 1 / MediaQuery.of(context).devicePixelRatio),
        itemBuilder: (context, index) => _OrderGroupCard(
          group: _orders![index],
          onTap: () => context.push('/orders/${_orders![index].id}'),
        ),
      ),
    );
  }
}

class _EmptyOrders extends StatelessWidget {
  const _EmptyOrders();

  @override
  Widget build(BuildContext context) {
    return Center(
      child: Column(
        mainAxisSize: MainAxisSize.min,
        children: [
          Container(
            width: 56,
            height: 56,
            decoration: BoxDecoration(
              color: AppColors.primary.withValues(alpha: 0.1),
              borderRadius: BorderRadius.circular(16),
            ),
            child: const Icon(
              Icons.receipt_long_rounded,
              size: 28,
              color: AppColors.primary,
            ),
          ),
          const SizedBox(height: 20),
          Text(
            'No orders yet',
            style: Theme.of(context).textTheme.titleMedium?.copyWith(
                  fontWeight: FontWeight.w700,
                ),
          ),
          const SizedBox(height: 8),
          Text(
            'Your order history will appear here.',
            style: Theme.of(context).textTheme.bodyMedium?.copyWith(
                  color: AppColors.onSurfaceMuted,
                ),
            textAlign: TextAlign.center,
          ),
        ],
      ),
    );
  }
}

/// Overall group state — colored pill. The server derives it from the
/// store orders; the app only renders it.
Color _groupColor(String status) {
  switch (status) {
    case 'in_progress':
      return AppColors.primary;
    case 'partially_fulfilled':
      return const Color(0xFF8B5CF6); // purple
    case 'completed':
      return AppColors.success;
    case 'cancelled':
      return AppColors.error;
    default:
      return AppColors.onSurfaceMuted;
  }
}

String _groupLabel(String status) {
  switch (status) {
    case 'in_progress':
      return 'In progress';
    case 'partially_fulfilled':
      return 'Partially fulfilled';
    case 'completed':
      return 'Completed';
    case 'cancelled':
      return 'Cancelled';
    default:
      return status;
  }
}

class _OrderGroupCard extends StatelessWidget {
  const _OrderGroupCard({required this.group, required this.onTap});

  final GroupSummary group;
  final VoidCallback onTap;

  @override
  Widget build(BuildContext context) {
    final textTheme = Theme.of(context).textTheme;
    final color = _groupColor(group.status);
    final storesLabel = group.stores.join(' · ');

    return Material(
      color: Colors.transparent,
      child: InkWell(
        onTap: onTap,
        child: Padding(
          padding: const EdgeInsets.symmetric(horizontal: 20, vertical: 14),
          child: Row(
            children: [
              // Overall status chip
              Container(
                padding: const EdgeInsets.symmetric(horizontal: 8, vertical: 4),
                decoration: BoxDecoration(
                  color: color.withValues(alpha: 0.12),
                  borderRadius: BorderRadius.circular(999),
                ),
                child: Text(
                  _groupLabel(group.status),
                  style: textTheme.labelSmall?.copyWith(
                    color: color,
                    fontWeight: FontWeight.w700,
                  ),
                ),
              ),
              const SizedBox(width: 14),
              // Stores + total
              Expanded(
                child: Column(
                  crossAxisAlignment: CrossAxisAlignment.start,
                  children: [
                    Text(
                      storesLabel,
                      maxLines: 1,
                      overflow: TextOverflow.ellipsis,
                      style: textTheme.bodyMedium?.copyWith(
                        fontWeight: FontWeight.w600,
                      ),
                    ),
                    Text(
                      formatRwf(group.grandTotal),
                      style: textTheme.labelMedium?.copyWith(
                        color: AppColors.primary,
                        fontWeight: FontWeight.w700,
                      ),
                    ),
                  ],
                ),
              ),
              // Date (compact)
              Text(
                _formatDate(group.createdAt),
                style: textTheme.labelSmall?.copyWith(
                  color: AppColors.onSurfaceMuted,
                ),
              ),
              const SizedBox(width: 8),
              Icon(
                Icons.chevron_right_rounded,
                size: 20,
                color: AppColors.onSurfaceMuted,
              ),
            ],
          ),
        ),
      ),
    );
  }
}

/// Format an RFC-3339 date string into a compact relative or absolute form.
/// V1 keeps it simple — "Today"/"Yesterday" and short dates otherwise.
String _formatDate(String rfc3339) {
  try {
    final dt = DateTime.parse(rfc3339);
    final now = DateTime.now();
    final diff = now.difference(dt);
    if (diff.inDays == 0) return 'Today';
    if (diff.inDays == 1) return 'Yesterday';
    if (diff.inDays < 7) return '${diff.inDays}d ago';
    return '${dt.month}/${dt.day}';
  } on Object {
    return rfc3339;
  }
}
