import 'dart:async';
import 'dart:convert';

import 'package:http/http.dart' as http;

import 'package:tuma_app/core/config/app_config.dart';

/// Tuma API errors. Mapped to the server's stable error envelope
/// (see tuma-docs/Tuma_API_Architecture.md §4).
sealed class ApiError implements Exception {
  const ApiError(this.message, {this.fieldErrors});

  final String message;
  final List<ApiFieldError>? fieldErrors;

  @override
  String toString() => '$runtimeType($message)';
}

class ApiBadRequest extends ApiError {
  const ApiBadRequest(super.message, {super.fieldErrors});
}

class ApiUnauthorized extends ApiError {
  const ApiUnauthorized([super.message = 'You need to sign in.']);
}

class ApiForbidden extends ApiError {
  const ApiForbidden([super.message = 'You don\'t have access.']);
}

class ApiNotFound extends ApiError {
  const ApiNotFound([super.message = 'Not found.']);
}

class ApiConflict extends ApiError {
  const ApiConflict(super.message, {super.fieldErrors});
}

class ApiNetwork extends ApiError {
  const ApiNetwork([super.message = 'Can\'t reach the server. Check your connection.']);
}

class ApiServer extends ApiError {
  const ApiServer([super.message = 'Something went wrong. Please try again.']);
}

class ApiFieldError {
  ApiFieldError({required this.field, required this.message});

  final String field;
  final String message;

  factory ApiFieldError.fromJson(Map<String, dynamic> json) => ApiFieldError(
        field: json['field'] as String,
        message: json['message'] as String,
      );
}

/// Mutable Bearer token holder, read by the api client before each request.
typedef TokenProvider = String? Function();

/// Called once per 401 response — the session layer uses it to die loudly
/// (clear the token, return to auth) instead of leaving every screen
/// showing "You need to sign in" forever on an expired token.
typedef OnUnauthorized = void Function();

/// Lightweight HTTP client. Always goes through the versioned `/api/v1`
/// prefix; tests can swap the constructor for a mock. Every request is
/// bounded by a timeout — an unbounded request is how a splash screen
/// hangs forever and how poll loops pile up against a dead socket.
class ApiClient {
  ApiClient({
    http.Client? httpClient,
    AppConfig? config,
    required TokenProvider tokenProvider,
    OnUnauthorized? onUnauthorized,
  })  : _http = httpClient ?? http.Client(),
        _config = config ?? AppConfig.instance,
        _tokenProvider = tokenProvider, // ignore: prefer_initializing_formals
        _onUnauthorized = onUnauthorized; // ignore: prefer_initializing_formals

  /// The ceiling for one request. Generous on purpose — it bounds the
  /// disaster cases (dead network, hung socket), not normal latency.
  static const _timeout = Duration(seconds: 15);

  final http.Client _http;
  final AppConfig _config;
  final TokenProvider _tokenProvider;
  final OnUnauthorized? _onUnauthorized;

  /// Parse the error envelope ONCE — the code/message/field readers used
  /// to each jsonDecode the full body.
  ({String? code, String? message, List<ApiFieldError>? fields})? _parseError(
    http.Response response,
  ) {
    if (response.body.isEmpty) return null;
    try {
      final decoded = jsonDecode(response.body);
      if (decoded is! Map<String, dynamic>) return null;
      final fields = decoded['details'] is List
          ? (decoded['details'] as List)
              .whereType<Map<String, dynamic>>()
              .map(ApiFieldError.fromJson)
              .toList()
          : null;
      return (
        code: decoded['error'] as String?,
        message: decoded['message'] is String
            ? decoded['message'] as String
            : null,
        fields: fields,
      );
    } on Object {
      return null;
    }
  }

  Future<dynamic> get(String path, {Map<String, String>? query}) {
    return _send('GET', path, null, query: query);
  }

  Future<dynamic> post(String path, {Object? body}) async {
    return _send('POST', path, body);
  }

  /// [token] overrides the session token for this one request. The
  /// name-capture step needs it: it runs before `accept()` activates the
  /// token on the session, so the client would otherwise send no Bearer
  /// header and the server answers 401.
  Future<dynamic> patch(String path, {Object? body, String? token}) async {
    return _send('PATCH', path, body, token: token);
  }

  /// The decoded JSON of a successful response, or `null` when the body
  /// is empty (204s and friends). Throws [ApiError] otherwise. Callers
  /// that expect a JSON list should go through [getList] so contract
  /// drift surfaces as an [ApiError], not a CastError crash.
  Future<List<dynamic>> getList(String path, {Map<String, String>? query}) async {
    final decoded = await _send('GET', path, null, query: query);
    if (decoded is List) return decoded;
    throw ApiServer('Unexpected response from $path.');
  }

  Future<dynamic> _send(
    String method,
    String path,
    Object? body, {
    String? token,
    Map<String, String>? query,
  }) async {
    var url = Uri.parse('${_config.apiV1}$path');
    if (query != null && query.isNotEmpty) {
      url = url.replace(queryParameters: query);
    }
    final bearer = token ?? _tokenProvider();
    final headers = <String, String>{
      'Accept': 'application/json',
      if (body != null) 'Content-Type': 'application/json',
      if (bearer != null) 'Authorization': 'Bearer $bearer',
    };
    final http.Response response;
    try {
      switch (method) {
        case 'GET':
          response =
              await _http.get(url, headers: headers).timeout(_timeout);
        case 'POST':
          response = await _http
              .post(
                url,
                headers: headers,
                body: body == null ? null : jsonEncode(body),
              )
              .timeout(_timeout);
        case 'PATCH':
          response = await _http
              .patch(
                url,
                headers: headers,
                body: body == null ? null : jsonEncode(body),
              )
              .timeout(_timeout);
        default:
          throw ArgumentError('unsupported method $method');
      }
    } on TimeoutException {
      throw const ApiNetwork('The server took too long to answer.');
    } on Object catch (error) {
      // Network errors, anything outside the HTTP response path.
      throw ApiNetwork(error.toString());
    }
    return _decode(response);
  }

  dynamic _decode(http.Response response) {
    if (response.statusCode >= 200 && response.statusCode < 300) {
      if (response.body.isEmpty) return null;
      try {
        return jsonDecode(response.body);
      } on Object {
        // A 2xx with a non-JSON body is the server misbehaving, not the
        // caller's bug — it must surface as an ApiError like every other
        // failure, never as a raw FormatException out of every call.
        throw const ApiServer('The server sent something we could not read.');
      }
    }
    final parsed = _parseError(response);
    switch (response.statusCode) {
      case 400:
        throw ApiBadRequest(
          parsed?.message ?? response.reasonPhrase ?? 'Request failed',
          fieldErrors: parsed?.fields,
        );
      case 401:
        // One dead session, every screen told — the session layer clears
        // state and the router lands on auth.
        _onUnauthorized?.call();
        throw ApiUnauthorized(
          parsed?.message ?? 'You need to sign in.',
        );
      case 403:
        throw ApiForbidden(parsed?.message ?? response.reasonPhrase ?? 'Request failed');
      case 404:
        throw ApiNotFound(parsed?.message ?? response.reasonPhrase ?? 'Request failed');
      case 409:
        throw ApiConflict(
          parsed?.message ?? response.reasonPhrase ?? 'Request failed',
          fieldErrors: parsed?.fields,
        );
      case 422:
        throw ApiBadRequest(
          parsed?.message ?? response.reasonPhrase ?? 'Request failed',
          fieldErrors: parsed?.fields,
        );
      default:
        throw ApiServer(parsed?.message ?? 'Something went wrong. Please try again.');
    }
  }
}
