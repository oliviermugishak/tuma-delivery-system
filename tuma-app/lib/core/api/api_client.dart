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

/// Lightweight HTTP client. Always goes through the versioned `/api/v1`
/// prefix; tests can swap the constructor for a mock.
class ApiClient {
  ApiClient({
    http.Client? httpClient,
    AppConfig? config,
    required TokenProvider tokenProvider,
  })  : _http = httpClient ?? http.Client(),
        _config = config ?? AppConfig.instance,
        // ignore: prefer_initializing_formals — public parameter, private field.
        _tokenProvider = tokenProvider;

  final http.Client _http;
  final AppConfig _config;
  final TokenProvider _tokenProvider;

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
          response = await _http.get(url, headers: headers);
        case 'POST':
          response = await _http.post(
            url,
            headers: headers,
            body: body == null ? null : jsonEncode(body),
          );
        case 'PATCH':
          response = await _http.patch(
            url,
            headers: headers,
            body: body == null ? null : jsonEncode(body),
          );
        default:
          throw ArgumentError('unsupported method $method');
      }
    } on Object catch (error) {
      // Network errors, timeouts, anything outside the HTTP response path.
      throw ApiNetwork(error.toString());
    }
    return _decode(response);
  }

  dynamic _decode(http.Response response) {
    if (response.statusCode >= 200 && response.statusCode < 300) {
      if (response.body.isEmpty) return null;
      return jsonDecode(response.body);
    }
    final errorCode = _readErrorCode(response);
    final message = _readErrorMessage(response, errorCode);
    final fields = _readFieldErrors(response);
    switch (response.statusCode) {
      case 400:
        throw ApiBadRequest(message, fieldErrors: fields);
      case 401:
        throw const ApiUnauthorized();
      case 403:
        throw ApiForbidden(message);
      case 404:
        throw ApiNotFound(message);
      case 409:
        throw ApiConflict(message, fieldErrors: fields);
      case 422:
        throw ApiBadRequest(message, fieldErrors: fields);
      default:
        throw ApiServer(message);
    }
  }

  String? _readErrorCode(http.Response response) {
    if (response.body.isEmpty) return null;
    try {
      final decoded = jsonDecode(response.body);
      if (decoded is Map<String, dynamic>) return decoded['error'] as String?;
    } on Object {
      // fall through
    }
    return null;
  }

  String _readErrorMessage(http.Response response, String? errorCode) {
    if (response.body.isEmpty) return response.reasonPhrase ?? 'Request failed';
    try {
      final decoded = jsonDecode(response.body);
      if (decoded is Map<String, dynamic> && decoded['message'] is String) {
        return decoded['message'] as String;
      }
    } on Object {
      // fall through
    }
    return response.reasonPhrase ?? 'Request failed';
  }

  List<ApiFieldError>? _readFieldErrors(http.Response response) {
    if (response.body.isEmpty) return null;
    try {
      final decoded = jsonDecode(response.body);
      if (decoded is Map<String, dynamic> && decoded['details'] is List) {
        return (decoded['details'] as List)
            .whereType<Map<String, dynamic>>()
            .map(ApiFieldError.fromJson)
            .toList();
      }
    } on Object {
      // fall through
    }
    return null;
  }
}
