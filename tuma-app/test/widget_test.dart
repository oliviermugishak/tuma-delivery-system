import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';

import 'package:tuma_app/main.dart';

void main() {
  testWidgets('app boots and shows the Tuma wordmark', (tester) async {
    await tester.pumpWidget(const ProviderScope(child: TumaApp()));
    await tester.pump();

    expect(find.text('Tuma'), findsOneWidget);
    expect(find.text('Everything you crave, delivered.'), findsOneWidget);
  });
}
