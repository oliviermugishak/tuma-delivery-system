import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';

import 'package:tuma_app/core/auth/auth_controller.dart';
import 'package:tuma_app/core/auth/token_storage.dart';
import 'package:tuma_app/main.dart';

/// Test double: no stored token, and no platform channel involved.
class _FakeTokenStorage implements TokenStorage {
  @override
  Future<String?> readToken() async => null;

  @override
  Future<void> writeToken(String token) async {}

  @override
  Future<void> clear() async {}
}

void main() {
  testWidgets('app boots through the splash and lands on the phone screen',
      (tester) async {
    await tester.pumpWidget(
      ProviderScope(
        overrides: [tokenStorageProvider.overrideWithValue(_FakeTokenStorage())],
        child: const TumaApp(),
      ),
    );
    await tester.pump();

    // Splash while the session hydrates.
    expect(find.text('Tuma'), findsOneWidget);
    expect(find.text('Everything you crave, delivered.'), findsOneWidget);

    // Past the minimum splash duration: no stored token → anonymous → the
    // router redirect lands on the phone screen.
    await tester.pump(const Duration(milliseconds: 1300));
    await tester.pump();
    await tester.pump();

    expect(find.text('Welcome to Tuma'), findsOneWidget);
    expect(find.text('Continue'), findsOneWidget);
  });
}
