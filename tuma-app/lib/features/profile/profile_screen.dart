import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:go_router/go_router.dart';

import 'package:tuma_app/core/api/api_client.dart';
import 'package:tuma_app/core/auth/auth_controller.dart';
import 'package:tuma_app/core/theme/app_colors.dart';
import 'package:tuma_app/core/theme/app_theme.dart';
import 'package:tuma_app/shared/widgets/design_system.dart';
import 'package:tuma_app/shared/widgets/show_app_snack.dart';

/// Profile tab — the founder's example layout: identity card with the
/// inline edit, rows to real destinations, sign out. Only rows backed by
/// a real feature ship; Payment Methods, Favorites, Settings/push
/// notifications and Support join when their slices land — no stub rows.
/// Name editing is an INLINE section (not a dialog): it's the first of
/// the profile fields — more editable facts join this screen later, each
/// as its own card in this column.
class ProfileScreen extends ConsumerStatefulWidget {
  const ProfileScreen({super.key});

  @override
  ConsumerState<ProfileScreen> createState() => _ProfileScreenState();
}

class _ProfileScreenState extends ConsumerState<ProfileScreen> {
  bool _signingOut = false;

  // The inline name editor — owned by this State, disposed with it (the
  // dialog version raced its exit animation: dispose ran while the route
  // transition still held the TextField's listeners → debug asserts).
  bool _editingName = false;
  bool _savingName = false;
  String? _nameError;
  late final TextEditingController _nameController = TextEditingController();
  String _currentName = '';

  Future<void> _signOut() async {
    setState(() => _signingOut = true);
    await ref.read(sessionProvider.notifier).signOut();
    // The router redirect does the navigating; re-arm only if we somehow
    // survive the transition.
    if (mounted) setState(() => _signingOut = false);
  }

  void _toggleNameEdit() {
    final session = ref.read(sessionProvider).asData?.value;
    final user = switch (session) {
      SessionUser s => s.user,
      _ => null,
    };
    if (user == null) return;
    setState(() {
      _currentName = user.displayName ?? '';
      _nameController.text = _currentName;
      _nameError = null;
      _editingName = !_editingName;
    });
  }

  /// Dirty state drives the Save button — typing re-evaluates it, and a
  /// session update from elsewhere (profile edits land here later too)
  /// re-seeds nothing while the editor is open.
  bool get _nameDirty =>
      _editingName && _nameController.text.trim() != _currentName;

  Future<void> _saveName() async {
    final name = _nameController.text.trim();
    if (name.isEmpty) {
      setState(() => _nameError = 'Enter a name.');
      return;
    }
    if (name == _currentName || _savingName) return;
    setState(() {
      _savingName = true;
      _nameError = null;
    });
    try {
      final updated = await ref.read(authApiProvider).updateName(name);
      ref.read(sessionProvider.notifier).updateUser(updated);
      if (!mounted) return;
      setState(() {
        _savingName = false;
        _editingName = false;
        _currentName = name;
      });
      showAppSnack(context, 'Name updated.', duration: const Duration(seconds: 1));
    } on ApiError catch (e) {
      if (!mounted) return;
      setState(() {
        _savingName = false;
        _nameError = e.message;
      });
    } on Object {
      if (!mounted) return;
      setState(() {
        _savingName = false;
        _nameError = 'Could not save. Try again.';
      });
    }
  }

  @override
  void dispose() {
    _nameController.dispose();
    super.dispose();
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
            Text('Profile', style: AppTheme.d1(textTheme)),
            const SizedBox(height: 14),
            // Identity card — who you are, with the edit affordance on
            // the right, exactly like the founder's example. The pencil
            // toggles the inline editor below (no dialog).
            Container(
              padding: const EdgeInsets.fromLTRB(20, 16, 8, 16),
              decoration: BoxDecoration(
                color: AppColors.surfaceAlt,
                borderRadius: BorderRadius.circular(16),
                border: Border.all(color: AppColors.surfaceBorder),
              ),
              child: Row(
                children: [
                  AccentAvatar(text: identity, size: 56),
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
                        if (phone != null) ...[
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
                  IconButton(
                    tooltip: _editingName ? 'Close editor' : 'Edit name',
                    icon: Icon(
                      _editingName
                          ? Icons.close_rounded
                          : Icons.edit_rounded,
                      size: 20,
                      color: AppColors.onSurfaceMuted,
                    ),
                    onPressed: _toggleNameEdit,
                  ),
                ],
              ),
            ),
            if (_editingName) ...[
              const SizedBox(height: 12),
              Container(
                padding: const EdgeInsets.all(16),
                decoration: BoxDecoration(
                  color: AppColors.surfaceAlt,
                  borderRadius: BorderRadius.circular(16),
                  border: Border.all(
                    color: AppColors.primary.withValues(alpha: 0.4),
                  ),
                ),
                child: Column(
                  crossAxisAlignment: CrossAxisAlignment.start,
                  children: [
                    TextField(
                      controller: _nameController,
                      autofocus: true,
                      textCapitalization: TextCapitalization.words,
                      onChanged: (_) => setState(() {}),
                      style: textTheme.bodyMedium,
                      decoration: InputDecoration(
                        hintText: 'How should we call you?',
                        hintStyle: textTheme.bodyMedium?.copyWith(
                          color: AppColors.onSurfaceMuted,
                        ),
                        counterText: '',
                      ),
                      maxLength: 60,
                    ),
                    if (_nameError != null) ...[
                      const SizedBox(height: 8),
                      Text(
                        _nameError!,
                        style: textTheme.bodySmall?.copyWith(
                          color: AppColors.error,
                        ),
                      ),
                    ],
                    const SizedBox(height: 12),
                    Row(
                      mainAxisAlignment: MainAxisAlignment.end,
                      children: [
                        TextButton(
                          onPressed:
                              _savingName ? null : _toggleNameEdit,
                          style: TextButton.styleFrom(
                            foregroundColor: AppColors.onSurfaceMuted,
                          ),
                          child: const Text('Cancel'),
                        ),
                        const SizedBox(width: 8),
                        FilledButton(
                          onPressed: !_nameDirty || _savingName
                              ? null
                              : _saveName,
                          // The theme's FilledButton is full-width
                          // (Size.fromHeight) — inside this Row that's
                          // infinite width. Just-fit, like the rider
                          // kiosk's row actions.
                          style: FilledButton.styleFrom(
                            minimumSize: const Size(0, 44),
                            disabledBackgroundColor: AppColors.onSurface
                                .withValues(alpha: 0.15),
                          ),
                          child: _savingName
                              ? const SizedBox(
                                  width: 18,
                                  height: 18,
                                  child: CircularProgressIndicator(
                                    strokeWidth: 2.5,
                                    color: AppColors.onPrimary,
                                  ),
                                )
                              : const Text('Save'),
                        ),
                      ],
                    ),
                  ],
                ),
              ),
            ],
            const SizedBox(height: 20),
            // ACCOUNT section (P15's grouped rhythm).
            const Padding(
              padding: EdgeInsets.fromLTRB(2, 0, 2, 8),
              child: MicroLabel('Account'),
            ),
            Container(
              decoration: BoxDecoration(
                color: AppColors.surfaceAlt,
                borderRadius: BorderRadius.circular(16),
                border: Border.all(color: AppColors.surfaceBorder),
              ),
              child: Column(
                children: [
                  _ProfileRow(
                    icon: Icons.location_on_rounded,
                    label: 'Delivery locations',
                    sub: 'Your pinned spot · Default',
                    onTap: () => context.push('/profile/location'),
                  ),
                  const Divider(height: 1, indent: 14, endIndent: 14),
                  _ProfileRow(
                    icon: Icons.credit_card_rounded,
                    label: 'Payment methods',
                    sub: 'Cash · MoMo (coming soon)',
                    onTap: () {},
                  ),
                ],
              ),
            ),
            const SizedBox(height: 20),
            // SUPPORT section.
            const Padding(
              padding: EdgeInsets.fromLTRB(2, 0, 2, 8),
              child: MicroLabel('Support'),
            ),
            Container(
              decoration: BoxDecoration(
                color: AppColors.surfaceAlt,
                borderRadius: BorderRadius.circular(16),
                border: Border.all(color: AppColors.surfaceBorder),
              ),
              child: Column(
                children: [
                  _ProfileRow(
                    icon: Icons.help_outline_rounded,
                    label: 'Help center',
                    onTap: () {},
                  ),
                  const Divider(height: 1, indent: 14, endIndent: 14),
                  _ProfileRow(
                    icon: Icons.description_rounded,
                    label: 'Terms & privacy',
                    onTap: () {},
                  ),
                ],
              ),
            ),
            const SizedBox(height: 26),
            // Sign out — quiet centered red TEXT (P14: red is vocabulary).
            Center(
              child: TextButton(
                onPressed: _signingOut ? null : _signOut,
                style: TextButton.styleFrom(
                  foregroundColor: AppColors.error,
                  textStyle: textTheme.titleSmall?.copyWith(
                    fontSize: 14,
                    fontWeight: FontWeight.w600,
                  ),
                ),
                child: Text(_signingOut ? 'Signing out…' : 'Sign out'),
              ),
            ),
            const SizedBox(height: 14),
            Center(
              child: Text('Tuma v1.0.0', style: AppTheme.sub(textTheme)),
            ),
          ],
        ),
      ),
    );
  }
}

/// One row of a grouped profile section: quiet icon, label, optional sub
/// line, chevron. Rows for features that don't exist yet are absent —
/// except the MoMo slot, whose disabled state is designed (P12).
class _ProfileRow extends StatelessWidget {
  const _ProfileRow({
    required this.icon,
    required this.label,
    required this.onTap,
    this.sub,
  });

  final IconData icon;
  final String label;
  final VoidCallback onTap;

  /// The quiet second line.
  final String? sub;

  @override
  Widget build(BuildContext context) {
    final textTheme = Theme.of(context).textTheme;
    return Material(
      color: Colors.transparent,
      child: InkWell(
        onTap: onTap,
        child: Padding(
          padding: const EdgeInsets.all(15),
          child: Row(
            children: [
              Icon(icon, size: 20, color: AppColors.onSurfaceMuted),
              const SizedBox(width: 12),
              Expanded(
                child: Column(
                  crossAxisAlignment: CrossAxisAlignment.start,
                  children: [
                    Text(
                      label,
                      style: textTheme.titleSmall?.copyWith(
                        fontSize: 14,
                        fontWeight: FontWeight.w600,
                      ),
                    ),
                    if (sub != null) ...[
                      const SizedBox(height: 2),
                      Text(
                        sub!,
                        maxLines: 1,
                        overflow: TextOverflow.ellipsis,
                        style: textTheme.bodySmall?.copyWith(
                          color: AppColors.onSurfaceMuted,
                        ),
                      ),
                    ],
                  ],
                ),
              ),
              const Icon(
                Icons.chevron_right_rounded,
                size: 18,
                color: AppColors.onSurfaceMuted,
              ),
            ],
          ),
        ),
      ),
    );
  }
}
