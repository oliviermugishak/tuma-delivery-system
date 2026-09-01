import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';

import 'package:tuma_app/core/api/api_client.dart';
import 'package:tuma_app/core/api/models/authenticated_user.dart';
import 'package:tuma_app/core/auth/auth_controller.dart';
import 'package:tuma_app/core/theme/app_colors.dart';
import 'package:tuma_app/shared/widgets/primary_button.dart';

/// Everything the name screen needs to finish the flow: the token minted by
/// the OTP verify and the user it belongs to. Carried via GoRouter `extra`.
class NameScreenArgs {
  const NameScreenArgs({required this.token, required this.user});

  final String token;
  final AuthenticatedUser user;
}

/// Third screen, first-time users only: capture a display name. Skippable —
/// the home shell greets anonymously until a name is set.
class NameScreen extends ConsumerStatefulWidget {
  const NameScreen({super.key, required this.args});

  final NameScreenArgs args;

  @override
  ConsumerState<NameScreen> createState() => _NameScreenState();
}

class _NameScreenState extends ConsumerState<NameScreen> {
  final TextEditingController _nameController = TextEditingController();
  String? _error;
  bool _submitting = false;

  @override
  void dispose() {
    _nameController.dispose();
    super.dispose();
  }

  Future<void> _finish(AuthenticatedUser user) async {
    await ref
        .read(sessionProvider.notifier)
        .accept(token: widget.args.token, user: user);
    // Router redirect → /home.
  }

  Future<void> _continue() async {
    if (_submitting) return;
    final name = _nameController.text.trim();
    if (name.isEmpty) {
      setState(() => _error = 'Tell us what to call you');
      return;
    }
    setState(() {
      _submitting = true;
      _error = null;
    });
    try {
      final updated = await ref
          .read(authApiProvider)
          .updateName(name, token: widget.args.token);
      if (!mounted) return;
      await _finish(updated);
    } on ApiError catch (error) {
      if (!mounted) return;
      setState(() => _error = error.message);
    } finally {
      if (mounted) setState(() => _submitting = false);
    }
  }

  Future<void> _skip() async {
    if (_submitting) return;
    setState(() => _submitting = true);
    try {
      await _finish(widget.args.user);
    } finally {
      // A storage failure inside _finish must not leave the screen
      // permanently disabled — the failure surfaces wherever it throws.
      if (mounted) setState(() => _submitting = false);
    }
  }

  @override
  Widget build(BuildContext context) {
    final textTheme = Theme.of(context).textTheme;
    return Scaffold(
      backgroundColor: AppColors.surface,
      body: SafeArea(
        child: Padding(
          padding: const EdgeInsets.symmetric(horizontal: 24),
          child: Column(
            crossAxisAlignment: CrossAxisAlignment.stretch,
            children: [
              const SizedBox(height: 72),
              Text(
                'What should we call you?',
                style: textTheme.headlineSmall?.copyWith(
                  color: AppColors.onSurface,
                  fontWeight: FontWeight.w800,
                ),
              ),
              const SizedBox(height: 8),
              Text(
                'So shops and riders know who they are serving.',
                style: textTheme.bodyMedium?.copyWith(color: AppColors.onSurfaceMuted),
              ),
              const SizedBox(height: 32),
              TextField(
                controller: _nameController,
                autofocus: true,
                textCapitalization: TextCapitalization.words,
                style: textTheme.bodyLarge?.copyWith(color: AppColors.onSurface),
                decoration: const InputDecoration(
                  hintText: 'Your name',
                  counterText: '',
                ),
                maxLength: 100,
                onChanged: (_) {
                  if (_error != null) setState(() => _error = null);
                },
                onSubmitted: (_) => _continue(),
              ),
              if (_error != null) ...[
                const SizedBox(height: 8),
                Text(
                  _error!,
                  style: textTheme.bodySmall?.copyWith(color: AppColors.error),
                ),
              ],
              const Spacer(),
              PrimaryButton(
                label: 'Continue',
                loading: _submitting,
                onPressed: _submitting ? null : _continue,
              ),
              const SizedBox(height: 8),
              TextButton(
                onPressed: _submitting ? null : _skip,
                child: Text(
                  'Not now',
                  style: textTheme.bodyMedium?.copyWith(color: AppColors.onSurfaceMuted),
                ),
              ),
              const SizedBox(height: 16),
            ],
          ),
        ),
      ),
    );
  }
}
