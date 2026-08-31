import 'package:flutter/foundation.dart';
import 'package:flutter_test/flutter_test.dart';

/// A widget test pinned to desktop: `defaultTargetPlatform` reads linux
/// for the body, so map surfaces take their documented honest paths (the
/// data placeholder, the paste-in pin field) — no Google Maps platform
/// channels exist in the test binding. The override is scoped (the
/// framework asserts foundation debug vars are unset after each test)
/// and restored even when the body throws. On the Linux dev host this is
/// a no-op override; on any other host it pins the map surfaces off.
void testDesktop(
    String description, Future<void> Function(WidgetTester) callback) {
  testWidgets(description, (tester) async {
    debugDefaultTargetPlatformOverride = TargetPlatform.linux;
    try {
      await callback(tester);
    } finally {
      debugDefaultTargetPlatformOverride = null;
    }
  });
}
