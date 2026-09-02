import 'package:flutter_test/flutter_test.dart';

import 'package:tuma_app/shared/widgets/show_app_snack.dart';

/// Review P24: `tel:` URIs are built from server-provided strings. A
/// crafted number carrying DTMF/pause characters (`,` `;` `#` `*`) could
/// dial extra digits once it reaches the platform dialer — the
/// sanitizer keeps digits and a leading `+` only.
void main() {
  test('sanitizer keeps digits and plus, strips everything else', () {
    expect(sanitizeTelPhone('+250788123456'), '+250788123456');
    expect(sanitizeTelPhone('0788 123 456'), '0788123456');
    // The attack shapes: pause/DTMF that could dial extensions.
    expect(sanitizeTelPhone('+250788123456,,123#'), '+250788123456123');
    expect(sanitizeTelPhone('+250788123456;ext=9'), '+2507881234569');
    expect(sanitizeTelPhone('+250788123456*9#'), '+2507881234569');
    // Non-phone garbage becomes nothing — the launch helper refuses.
    expect(sanitizeTelPhone('hello'), '');
    expect(sanitizeTelPhone(''), '');
    // Letters interleaved with digits: digits survive, letters go.
    expect(sanitizeTelPhone('call-0788x123'), '0788123');
  });
}
