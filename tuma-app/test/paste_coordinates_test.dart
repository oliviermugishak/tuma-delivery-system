import 'package:flutter_test/flutter_test.dart';

import 'package:tuma_app/features/location/delivery_pin_map.dart';

void main() {
  test('raw "lat, lng" pairs parse with spaces and sign', () {
    final pin = parsePastedCoordinates('-1.9499, 30.0622');
    expect(pin, isNotNull);
    expect(pin!.lat, -1.9499);
    expect(pin.lng, 30.0622);
  });

  test('a Google Maps share-link prefers the precise place marker', () {
    // A real place link carries both the @viewport center and the !3d/!4d
    // pin — the pin wins, the viewport center would be a guess.
    final pin = parsePastedCoordinates(
      'https://www.google.com/maps/place/Kigali/@-1.9449,30.0619,13z'
      '/data=!4m5!3m4!1s0x0:0x0!8m2!3d-1.9501!4d30.0631',
    );
    expect(pin, isNotNull);
    expect(pin!.lat, -1.9501);
    expect(pin.lng, 30.0631);
  });

  test('a Google search link q= pair parses', () {
    final pin = parsePastedCoordinates(
      'https://www.google.com/maps?q=-1.9449,30.0619',
    );
    expect(pin, isNotNull);
    expect(pin!.lat, -1.9449);
    expect(pin.lng, 30.0619);
  });

  test('nonsense is rejected, not guessed', () {
    expect(parsePastedCoordinates('KN 4 Ave, Kigali'), isNull,
        reason: 'a comma is not a coordinate pair');
    expect(parsePastedCoordinates(''), isNull);
    expect(parsePastedCoordinates('999, 30'), isNull,
        reason: 'latitude out of range');
    expect(parsePastedCoordinates('-1.9, 999'), isNull,
        reason: 'longitude out of range');
  });
}
