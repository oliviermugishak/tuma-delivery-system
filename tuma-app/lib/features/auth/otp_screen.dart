import 'dart:async';
import 'dart:math' as math;

import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:go_router/go_router.dart';

import 'package:tuma_app/core/api/api_client.dart';
import 'package:tuma_app/core/auth/auth_controller.dart';
import 'package:tuma_app/core/theme/app_colors.dart';
import 'package:tuma_app/features/auth/name_screen.dart';

/// Second screen of the auth flow: the 6-digit code. Submits automatically
/// when the code is complete; a wrong code shakes, clears, and retries.
class OtpScreen extends ConsumerStatefulWidget {
  const OtpScreen({super.key, required this.phone});

  final String phone;

  @override
  ConsumerState<OtpScreen> createState() => _OtpScreenState();
}

class _OtpScreenState extends ConsumerState<OtpScreen> with TickerProviderStateMixin {
  static const int _codeLength = 6;
  static const int _resendCooldownSecs = 60; // mirrors the server cooldown

  final TextEditingController _codeController = TextEditingController();
  final FocusNode _focusNode = FocusNode();
  late final AnimationController _shake = AnimationController(
    vsync: this,
    duration: const Duration(milliseconds: 450),
  );
  Timer? _countdownTimer;
  int _secondsLeft = _resendCooldownSecs;
  String? _error;
  bool _submitting = false;
  bool _resending = false;

  @override
  void initState() {
    super.initState();
    _focusNode.addListener(() => setState(() {}));
    _startCountdown();
  }

  @override
  void dispose() {
    _countdownTimer?.cancel();
    _shake.dispose();
    _codeController.dispose();
    _focusNode.dispose();
    super.dispose();
  }

  void _startCountdown() {
    _countdownTimer?.cancel();
    setState(() => _secondsLeft = _resendCooldownSecs);
    _countdownTimer = Timer.periodic(const Duration(seconds: 1), (timer) {
      if (_secondsLeft <= 1) {
        timer.cancel();
        setState(() => _secondsLeft = 0);
      } else {
        setState(() => _secondsLeft--);
      }
    });
  }

  void _onCodeChanged(String value) {
    if (_error != null) setState(() => _error = null);
    if (value.length == _codeLength) unawaited(_verify(value));
  }

  Future<void> _verify(String code) async {
    if (_submitting) return;
    setState(() => _submitting = true);
    try {
      final result =
          await ref.read(authApiProvider).verifyOtp(phone: widget.phone, code: code);
      if (!mounted) return;
      final name = result.user.displayName;
      if (name == null || name.trim().isEmpty) {
        // First-time user: capture the name before settling into home.
        unawaited(
          context.push(
            '/auth/name',
            extra: NameScreenArgs(token: result.token, user: result.user),
          ),
        );
      } else {
        await ref
            .read(sessionProvider.notifier)
            .accept(token: result.token, user: result.user);
        // The router redirect takes it from here → /home.
      }
    } on ApiError catch (error) {
      if (!mounted) return;
      setState(() {
        _error = error.message;
        _codeController.clear();
      });
      _shake.forward(from: 0);
      _focusNode.requestFocus();
    } finally {
      if (mounted) setState(() => _submitting = false);
    }
  }

  Future<void> _resend() async {
    if (_resending || _secondsLeft > 0) return;
    setState(() {
      _resending = true;
      _error = null;
    });
    try {
      await ref.read(authApiProvider).requestOtp(widget.phone);
      if (!mounted) return;
      _startCountdown();
    } on ApiError catch (error) {
      if (!mounted) return;
      setState(() => _error = error.message);
    } finally {
      if (mounted) setState(() => _resending = false);
    }
  }

  Widget _buildBoxes(TextTheme textTheme) {
    final code = _codeController.text;
    return Row(
      children: List.generate(_codeLength, (index) {
        final filled = index < code.length;
        final isCurrent = index == code.length && _focusNode.hasFocus && !_submitting;
        final borderColor = _error != null
            ? AppColors.error
            : isCurrent
                ? AppColors.primary
                : AppColors.surfaceBorder;
        return Expanded(
          child: Padding(
            padding: const EdgeInsets.symmetric(horizontal: 4),
            child: AspectRatio(
              aspectRatio: 0.78,
              child: AnimatedContainer(
                duration: const Duration(milliseconds: 150),
                decoration: BoxDecoration(
                  color: AppColors.surfaceAlt,
                  borderRadius: BorderRadius.circular(12),
                  border: Border.all(
                    color: borderColor,
                    width: _error != null || isCurrent ? 1.5 : 1,
                  ),
                ),
                child: Center(
                  child: Text(
                    filled ? code[index] : '',
                    style: textTheme.titleLarge?.copyWith(
                      color: AppColors.onSurface,
                      fontWeight: FontWeight.w700,
                    ),
                  ),
                ),
              ),
            ),
          ),
        );
      }),
    );
  }

  @override
  Widget build(BuildContext context) {
    final textTheme = Theme.of(context).textTheme;
    return Scaffold(
      backgroundColor: AppColors.surface,
      // Scroll-safe, same as the phone screen: the keyboard must never
      // stripe the fixed content on a small phone.
      body: SafeArea(
        child: SingleChildScrollView(
          child: ConstrainedBox(
            constraints: BoxConstraints(
              minHeight: MediaQuery.sizeOf(context).height -
                  MediaQuery.paddingOf(context).vertical,
            ),
            child: Padding(
              padding: const EdgeInsets.symmetric(horizontal: 24),
              child: Column(
            crossAxisAlignment: CrossAxisAlignment.stretch,
            children: [
              Align(
                alignment: Alignment.centerLeft,
                child: IconButton(
                  tooltip: 'Change number',
                  icon: const Icon(Icons.arrow_back, color: AppColors.onSurface),
                  onPressed: () => context.pop(),
                ),
              ),
              const SizedBox(height: 8),
              Text(
                'Enter the code',
                style: textTheme.headlineSmall?.copyWith(
                  color: AppColors.onSurface,
                  fontWeight: FontWeight.w800,
                ),
              ),
              const SizedBox(height: 8),
              Text.rich(
                TextSpan(
                  text: 'We sent a 6-digit code to ',
                  children: [
                    TextSpan(
                      text: widget.phone,
                      style: textTheme.bodyMedium?.copyWith(
                        color: AppColors.onSurface,
                        fontWeight: FontWeight.w600,
                      ),
                    ),
                  ],
                ),
                style: textTheme.bodyMedium?.copyWith(color: AppColors.onSurfaceMuted),
              ),
              const SizedBox(height: 40),
              AnimatedBuilder(
                animation: _shake,
                builder: (context, child) {
                  final progress = _shake.value;
                  final dx = math.sin(progress * math.pi * 3) * 10 * (1 - progress);
                  return Transform.translate(offset: Offset(dx, 0), child: child);
                },
                child: Stack(
                  children: [
                    IgnorePointer(child: _buildBoxes(textTheme)),
                    // Invisible full-size field: taps anywhere open the
                    // keyboard, digits land in one controller, boxes render it.
                    Positioned.fill(
                      child: Opacity(
                        opacity: 0,
                        child: TextField(
                          controller: _codeController,
                          focusNode: _focusNode,
                          autofocus: true,
                          enabled: !_submitting,
                          keyboardType: TextInputType.number,
                          inputFormatters: [
                            FilteringTextInputFormatter.digitsOnly,
                            LengthLimitingTextInputFormatter(_codeLength),
                          ],
                          decoration: const InputDecoration(
                            filled: false,
                            counterText: '',
                            border: InputBorder.none,
                            enabledBorder: InputBorder.none,
                            focusedBorder: InputBorder.none,
                            contentPadding: EdgeInsets.zero,
                          ),
                          onChanged: _onCodeChanged,
                        ),
                      ),
                    ),
                  ],
                ),
              ),
              const SizedBox(height: 12),
              SizedBox(
                height: 20,
                child: _error == null
                    ? null
                    : Text(
                        _error!,
                        textAlign: TextAlign.center,
                        style: textTheme.bodySmall?.copyWith(color: AppColors.error),
                      ),
              ),
              const SizedBox(height: 16),
              Center(
                child: _secondsLeft > 0
                    ? Text(
                        'Resend code in ${_secondsLeft}s',
                        style:
                            textTheme.bodyMedium?.copyWith(color: AppColors.onSurfaceMuted),
                      )
                    : TextButton(
                        onPressed: _resending ? null : _resend,
                        child: Text(_resending ? 'Sending…' : 'Resend code'),
                      ),
              ),
              // The flexible tail the Spacer owned — scrolls away under
              // the keyboard instead of striping.
              const SizedBox(height: 48),
              if (_submitting)
                const Padding(
                  padding: EdgeInsets.only(bottom: 32),
                  child: Center(
                    child: SizedBox(
                      width: 24,
                      height: 24,
                      child: CircularProgressIndicator(
                        strokeWidth: 2.5,
                        valueColor: AlwaysStoppedAnimation(AppColors.primary),
                      ),
                    ),
                  ),
                ),
              const SizedBox(height: 16),
            ],
          ),
        ),
          ),
        ),
      ),
    );
  }
}
