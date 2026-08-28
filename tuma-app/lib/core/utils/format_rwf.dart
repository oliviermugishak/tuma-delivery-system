/// Integer RWF as people see it: `3500` → `"3,500 RWF"`.
///
/// Money on Tuma is whole francs and server-owned — this only formats,
/// never computes. Matches the platform's `formatRwf` (en-US grouping).
/// Hand-rolled so no formatting dependency earns a place just for commas.
String formatRwf(int amount) {
  final negative = amount < 0;
  final digits = (negative ? -amount : amount).toString();
  final grouped = StringBuffer();
  for (var i = 0; i < digits.length; i++) {
    if (i > 0 && (digits.length - i) % 3 == 0) {
      grouped.write(',');
    }
    grouped.write(digits[i]);
  }
  return '${negative ? '-' : ''}$grouped RWF';
}
