/// Environment-driven app configuration.
///
/// Keep this tiny: the API base URL is the only meaningful runtime knob.
/// Override at build with:
///   flutter run --dart-define=TUMA_API_BASE_URL=http://10.0.2.2:8080
class AppConfig {
  AppConfig._({required this.apiBaseUrl});

  /// Tuma API. Default targets the local server; override per-device with
  /// `--dart-define=TUMA_API_BASE_URL=http://...` (e.g. the Android emulator
  /// uses `http://10.0.2.2:8080` to reach the host loopback).
  static final AppConfig instance = AppConfig._(
    apiBaseUrl: const String.fromEnvironment(
      'TUMA_API_BASE_URL',
      defaultValue: 'http://localhost:8080',
    ),
  );

  final String apiBaseUrl;

  /// Root for the versioned business API.
  String get apiV1 => '$apiBaseUrl/api/v1';
}
