import 'dart:async';

import 'package:flutter/material.dart';
import 'package:url_launcher/url_launcher.dart';

import 'package:tuma_app/core/theme/app_colors.dart';

/// The app's one snack bar (review P19): floating, the white card, ink
/// text, green action. A dozen copies of this styling had drifted across
/// the screens; every surface shows a toast through here.
///
/// [duration] defaults to Material's 4s; call sites that had a shorter
/// one pass it through.
void showAppSnack(
  BuildContext context,
  String message, {
  Duration duration = const Duration(seconds: 4),
}) {
  ScaffoldMessenger.of(context).showSnackBar(
    SnackBar(
      content: Text(message, style: const TextStyle(color: AppColors.onSurface)),
      behavior: SnackBarBehavior.floating,
      duration: duration,
      backgroundColor: AppColors.surface,
      elevation: 3,
      shape: RoundedRectangleBorder(borderRadius: BorderRadius.circular(12)),
    ),
  );
}

/// The dialer-safe form of a server-provided phone string (review P24):
/// digits and a leading `+` only. A crafted number carrying `,`/`#`/`;`
/// pauses or DTMF could dial extra digits once it reaches the platform
/// dialer; everything else is stripped before the `tel:` URI is built.
String sanitizeTelPhone(String phone) =>
    phone.replaceAll(RegExp(r'[^0-9+]'), '');

/// Open [phone] in the platform dialer. False (nothing handles `tel:`)
/// and thrown errors both land on [onFail] — an uncaught async error
/// from a contact tap is never the customer's problem to see.
Future<void> launchDialer(
  String rawPhone, {
  required void Function(String sanitized) onFail,
}) async {
  final phone = sanitizeTelPhone(rawPhone);
  if (phone.isEmpty) {
    onFail(phone);
    return;
  }
  try {
    final launched = await launchUrl(Uri(scheme: 'tel', path: phone));
    if (!launched) onFail(phone);
  } on Object {
    onFail(phone);
  }
}

/// Open [email] in the platform mail app, same failure discipline as
/// [launchDialer].
Future<void> launchMail(
  String email, {
  required void Function() onFail,
}) async {
  try {
    final launched = await launchUrl(Uri(scheme: 'mailto', path: email));
    if (!launched) onFail();
  } on Object {
    onFail();
  }
}
