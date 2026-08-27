import 'dart:async';

import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:go_router/go_router.dart';

import 'package:tuma_app/core/api/api_client.dart';
import 'package:tuma_app/core/auth/auth_controller.dart';
import 'package:tuma_app/core/theme/app_brand.dart';
import 'package:tuma_app/core/theme/app_colors.dart';
import 'package:tuma_app/shared/widgets/primary_button.dart';

class _Country {
  const _Country(this.name, this.flag, this.dialCode, {this.nationalDigits});

  final String name;
  final String flag;
  final String dialCode;

  /// Exact national-number length when known; otherwise 8–12 digits pass.
  final int? nationalDigits;
}

const List<_Country> _countries = [
  _Country('Rwanda', '🇷🇼', '+250', nationalDigits: 9),
  _Country('Kenya', '🇰🇪', '+254'),
  _Country('Uganda', '🇺🇬', '+256'),
  _Country('Tanzania', '🇹🇿', '+255'),
  _Country('Burundi', '🇧🇮', '+257'),
  _Country('DR Congo', '🇨🇩', '+243'),
];

/// First screen of the auth flow. Register and login are one flow: the
/// server upserts the customer when the OTP verifies.
class PhoneScreen extends ConsumerStatefulWidget {
  const PhoneScreen({super.key});

  @override
  ConsumerState<PhoneScreen> createState() => _PhoneScreenState();
}

class _PhoneScreenState extends ConsumerState<PhoneScreen> {
  final TextEditingController _phoneController = TextEditingController();
  _Country _country = _countries.first;
  String? _error;
  bool _submitting = false;

  @override
  void dispose() {
    _phoneController.dispose();
    super.dispose();
  }

  String? _validate(String national) {
    final expected = _country.nationalDigits;
    if (expected != null) {
      if (national.length != expected) return 'Enter a $expected-digit phone number';
    } else if (national.length < 8 || national.length > 12) {
      return 'Enter a valid phone number';
    }
    return null;
  }

  Future<void> _continue() async {
    if (_submitting) return;
    final national = _phoneController.text.replaceAll(RegExp(r'\D'), '');
    final problem = _validate(national);
    if (problem != null) {
      setState(() => _error = problem);
      return;
    }
    setState(() {
      _submitting = true;
      _error = null;
    });
    final phone = '${_country.dialCode}$national';
    try {
      await ref.read(authApiProvider).requestOtp(phone);
      if (!mounted) return;
      unawaited(context.push('/auth/otp', extra: phone));
    } on ApiError catch (error) {
      if (!mounted) return;
      setState(() => _error = error.message);
    } finally {
      if (mounted) setState(() => _submitting = false);
    }
  }

  Future<void> _pickCountry() async {
    final picked = await showModalBottomSheet<_Country>(
      context: context,
      backgroundColor: AppColors.surfaceAlt,
      shape: const RoundedRectangleBorder(
        borderRadius: BorderRadius.vertical(top: Radius.circular(20)),
      ),
      builder: (sheetContext) => SafeArea(
        child: Column(
          mainAxisSize: MainAxisSize.min,
          children: [
            for (final country in _countries)
              ListTile(
                leading: Text(country.flag, style: const TextStyle(fontSize: 22)),
                title: Text(country.name),
                trailing: country == _country
                    ? const Icon(Icons.check_circle, color: AppColors.primary)
                    : Text(
                        country.dialCode,
                        style: TextStyle(color: AppColors.onSurfaceMuted),
                      ),
                onTap: () => Navigator.of(sheetContext).pop(country),
              ),
            const SizedBox(height: 8),
          ],
        ),
      ),
    );
    if (picked != null) setState(() => _country = picked);
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
              const SizedBox(height: 48),
              const Center(child: BrandMark(size: 72, borderRadius: 16)),
              const SizedBox(height: 24),
              Text(
                'Welcome to Tuma',
                textAlign: TextAlign.center,
                style: textTheme.headlineSmall?.copyWith(
                  color: AppColors.onSurface,
                  fontWeight: FontWeight.w800,
                ),
              ),
              const SizedBox(height: 8),
              Text(
                'Everything you crave, delivered.',
                textAlign: TextAlign.center,
                style: textTheme.bodyMedium?.copyWith(color: AppColors.onSurfaceMuted),
              ),
              const SizedBox(height: 48),
              Text(
                'Phone number',
                style: textTheme.labelLarge?.copyWith(color: AppColors.onSurface),
              ),
              const SizedBox(height: 8),
              Row(
                children: [
                  _CountryChip(country: _country, onTap: _pickCountry),
                  const SizedBox(width: 8),
                  Expanded(
                    child: TextField(
                      controller: _phoneController,
                      keyboardType: TextInputType.phone,
                      autofocus: true,
                      style: textTheme.bodyLarge?.copyWith(color: AppColors.onSurface),
                      decoration: const InputDecoration(hintText: '7XX XXX XXX'),
                      onChanged: (_) {
                        if (_error != null) setState(() => _error = null);
                      },
                      onSubmitted: (_) => _continue(),
                    ),
                  ),
                ],
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
              const SizedBox(height: 16),
              Text(
                'By continuing you agree to Tuma\'s Terms and Privacy Policy.',
                textAlign: TextAlign.center,
                style: textTheme.bodySmall?.copyWith(color: AppColors.onSurfaceMuted),
              ),
              const SizedBox(height: 16),
            ],
          ),
        ),
      ),
    );
  }
}

class _CountryChip extends StatelessWidget {
  const _CountryChip({required this.country, required this.onTap});

  final _Country country;
  final VoidCallback onTap;

  @override
  Widget build(BuildContext context) {
    return Material(
      color: AppColors.surfaceAlt,
      borderRadius: BorderRadius.circular(12),
      child: InkWell(
        borderRadius: BorderRadius.circular(12),
        onTap: onTap,
        child: Padding(
          padding: const EdgeInsets.symmetric(horizontal: 12, vertical: 14),
          child: Row(
            mainAxisSize: MainAxisSize.min,
            children: [
              Text(country.flag, style: const TextStyle(fontSize: 18)),
              const SizedBox(width: 6),
              Text(
                country.dialCode,
                style: Theme.of(context)
                    .textTheme
                    .bodyLarge
                    ?.copyWith(color: AppColors.onSurface),
              ),
              const SizedBox(width: 4),
              const Icon(
                Icons.keyboard_arrow_down,
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
