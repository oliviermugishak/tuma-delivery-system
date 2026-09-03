import 'dart:async';

import 'package:flutter/material.dart';

import 'package:tuma_app/core/theme/app_colors.dart';
import 'package:tuma_app/core/theme/app_theme.dart';
import 'package:tuma_app/shared/widgets/show_app_snack.dart';

/// One store's contact surface, as the sheets show it: name, optional
/// address, and the Call / Email actions. A missing fact renders an
/// honest muted line — never a dead button.
class StoreContactEntry {
  const StoreContactEntry({
    required this.name,
    this.address,
    this.phone,
    this.email,
  });

  final String name;
  final String? address;
  final String? phone;
  final String? email;
}

/// The contact bottom sheet — the customer's way to reach a store
/// directly (the founder's Get-help replacement: "This should be Call").
/// One entry per participating store; actions launch the real dialer and
/// the real mail composer.
Future<void> showStoreContactSheet(
  BuildContext context, {
  required List<StoreContactEntry> entries,
  String title = 'Contact the store',
}) {
  return showModalBottomSheet<void>(
    context: context,
    backgroundColor: AppColors.canvas,
    shape: const RoundedRectangleBorder(
      borderRadius: BorderRadius.vertical(top: Radius.circular(20)),
    ),
    isScrollControlled: true,
    builder: (sheetContext) => SafeArea(
      child: Padding(
        padding: const EdgeInsets.fromLTRB(20, 14, 20, 18),
        child: Column(
          mainAxisSize: MainAxisSize.min,
          crossAxisAlignment: CrossAxisAlignment.start,
          children: [
            Center(
              child: Container(
                width: 36,
                height: 4,
                decoration: BoxDecoration(
                  color: AppColors.hairline,
                  borderRadius: BorderRadius.circular(2),
                ),
              ),
            ),
            const SizedBox(height: 14),
            Text(title, style: AppTheme.d2(Theme.of(sheetContext).textTheme)),
            const SizedBox(height: 14),
            for (final entry in entries)
              Padding(
                padding: const EdgeInsets.only(bottom: 10),
                child: _ContactCard(entry: entry),
              ),
          ],
        ),
      ),
    ),
  );
}

class _ContactCard extends StatelessWidget {
  const _ContactCard({required this.entry});

  final StoreContactEntry entry;

  @override
  Widget build(BuildContext context) {
    final textTheme = Theme.of(context).textTheme;
    return Container(
      padding: const EdgeInsets.symmetric(horizontal: 14, vertical: 12),
      decoration: BoxDecoration(
        color: AppColors.surface,
        borderRadius: BorderRadius.circular(16),
        border: Border.all(color: AppColors.hairline),
      ),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          Text(entry.name,
              style: AppTheme.bd(textTheme).copyWith(fontWeight: FontWeight.w600)),
          if (entry.address != null && entry.address!.isNotEmpty) ...[
            const SizedBox(height: 2),
            Text(entry.address!, style: AppTheme.sub(textTheme)),
          ],
          const SizedBox(height: 10),
          Row(
            children: [
              if (entry.phone != null && entry.phone!.isNotEmpty)
                Expanded(
                  child: _ContactAction(
                    icon: Icons.call_rounded,
                    label: entry.phone!,
                    onTap: () => unawaited(
                      launchDialer(
                        entry.phone!,
                        onFail: (sanitized) => showAppSnack(
                          context,
                          'Could not call $sanitized.',
                        ),
                      ),
                    ),
                  ),
                ),
              if (entry.phone != null &&
                  entry.phone!.isNotEmpty &&
                  entry.email != null &&
                  entry.email!.isNotEmpty)
                const SizedBox(width: 8),
              if (entry.email != null && entry.email!.isNotEmpty)
                Expanded(
                  child: _ContactAction(
                    icon: Icons.mail_outline_rounded,
                    label: entry.email!,
                    onTap: () => unawaited(
                      launchMail(
                        entry.email!,
                        onFail: () => showAppSnack(
                          context,
                          'Could not open a mail app for ${entry.email!}.',
                        ),
                      ),
                    ),
                  ),
                ),
              if ((entry.phone == null || entry.phone!.isEmpty) &&
                  (entry.email == null || entry.email!.isEmpty))
                Expanded(
                  child: Text(
                    'No contact details yet — the store has not added a phone or email.',
                    style: AppTheme.sub(textTheme),
                  ),
                ),
            ],
          ),
        ],
      ),
    );
  }
}

class _ContactAction extends StatelessWidget {
  const _ContactAction({
    required this.icon,
    required this.label,
    required this.onTap,
  });

  final IconData icon;
  final String label;
  final VoidCallback onTap;

  @override
  Widget build(BuildContext context) {
    return InkWell(
      onTap: onTap,
      borderRadius: BorderRadius.circular(12),
      child: Container(
        height: 44,
        padding: const EdgeInsets.symmetric(horizontal: 12),
        decoration: BoxDecoration(
          color: AppColors.fill,
          borderRadius: BorderRadius.circular(12),
        ),
        child: Row(
          children: [
            Icon(icon, size: 18, color: AppColors.primary),
            const SizedBox(width: 8),
            Expanded(
              child: Text(
                label,
                maxLines: 1,
                overflow: TextOverflow.ellipsis,
                style: AppTheme.sub(Theme.of(context).textTheme)
                    .copyWith(color: AppColors.primary, fontWeight: FontWeight.w600),
              ),
            ),
          ],
        ),
      ),
    );
  }
}
