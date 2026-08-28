import 'package:flutter_test/flutter_test.dart';

import 'package:tuma_app/core/utils/format_rwf.dart';

void main() {
  group('formatRwf', () {
    test('formats whole francs with en-US grouping', () {
      expect(formatRwf(0), '0 RWF');
      expect(formatRwf(500), '500 RWF');
      expect(formatRwf(3500), '3,500 RWF');
      expect(formatRwf(120000), '120,000 RWF');
      expect(formatRwf(1234567), '1,234,567 RWF');
    });
  });
}
