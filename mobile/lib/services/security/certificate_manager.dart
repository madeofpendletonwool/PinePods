// Copyright 2025 the PinePods project contributors. All rights reserved.
// Use of this source code is governed by a BSD-style license that can be
// found in the LICENSE file.

import 'dart:convert';
import 'dart:io';

import 'package:crypto/crypto.dart';
import 'package:flutter/foundation.dart';
import 'package:flutter_secure_storage/flutter_secure_storage.dart';
import 'package:logging/logging.dart';

/// Snapshot of which user-supplied certificates are currently configured.
///
/// Used to drive the certificate UI on the login and settings screens.
class CertificateState {
  const CertificateState({this.serverCaName, this.clientCertName});

  /// Display name (original filename) of the imported server CA, or null.
  final String? serverCaName;

  /// Display name (original filename) of the imported client certificate, or null.
  final String? clientCertName;

  bool get hasServerCa => serverCaName != null;
  bool get hasClientCert => clientCertName != null;
}

/// Manages user-supplied TLS certificates and applies them process-wide.
///
/// Flutter/Dart's `http` package does **not** consult the OS/Android trust
/// store — it uses Dart's own bundled Mozilla roots via
/// [SecurityContext.defaultContext]. This manager lets a user import:
///
///  * a **server CA** (`.pem`/`.crt`/`.der`) so a private / self-signed server
///    certificate is trusted (fixes "not a valid PinePods server" for reverse
///    proxies behind Tailscale etc.), and
///  * a **client certificate** (`.p12`/`.pfx` + password) for **mTLS** — the
///    client presenting a certificate to the server (issue #800).
///
/// Everything is applied to the process-wide [SecurityContext.defaultContext].
/// This intentionally avoids a custom [HttpOverrides]: the top-level
/// `http.get`/`http.post` calls used across the app already build their client
/// from `defaultContext`, and the `podcast_search` feed path reassigns
/// `HttpOverrides.global` on every feed load but still creates its client from
/// `defaultContext` — so applying here covers all Dart-side network paths.
///
/// Note: `defaultContext` is a per-isolate singleton and is effectively
/// append-only — imported material cannot be *removed* from it until the app is
/// restarted. Removing a certificate clears it from secure storage (so it is
/// gone on next launch) and callers should surface a "restart to fully apply"
/// hint.
///
/// Out of scope (native, deferred): audio streaming (ExoPlayer/AVPlayer) and
/// downloads (`flutter_downloader`) use native HTTP stacks that bypass Dart TLS,
/// so mTLS-required playback/downloads need separate native work.
class CertificateManager {
  CertificateManager._();

  static final CertificateManager instance = CertificateManager._();

  // Secure-storage keys. Byte payloads are stored base64-encoded.
  static const String _serverCaKey = 'pinepods_server_ca_pem';
  static const String _serverCaNameKey = 'pinepods_server_ca_name';
  static const String _clientP12Key = 'pinepods_client_p12';
  static const String _clientP12PasswordKey = 'pinepods_client_p12_password';
  static const String _clientCertNameKey = 'pinepods_client_cert_name';

  final Logger _log = Logger('CertificateManager');
  // v10 uses custom ciphers on Android by default (Jetpack Security is
  // deprecated), so no special AndroidOptions are needed.
  final FlutterSecureStorage _storage = const FlutterSecureStorage();

  /// Current configured state, for UI. Rebuilds on import/remove.
  final ValueNotifier<CertificateState> state =
      ValueNotifier<CertificateState>(const CertificateState());

  // Fingerprints of material already applied to defaultContext this session,
  // to avoid redundant re-application.
  String? _appliedCaFingerprint;
  String? _appliedClientCertFingerprint;

  /// Read persisted certificates and apply them to [SecurityContext.defaultContext].
  /// Safe to call multiple times; call once at startup, and again after import.
  Future<void> init() async {
    await _refreshState();
    await _apply();
  }

  /// Import a server CA certificate (PEM or DER bytes) and apply it immediately.
  Future<void> importServerCa(List<int> bytes, String displayName) async {
    await _storage.write(key: _serverCaKey, value: base64Encode(bytes));
    await _storage.write(key: _serverCaNameKey, value: displayName);
    await _refreshState();
    await _apply();
  }

  /// Import a client certificate (PKCS#12 `.p12`/`.pfx` bytes) + password for
  /// mTLS, and apply it immediately. The password may be null/empty.
  ///
  /// Validate first with [validateClientCert]; this throws a [TlsException] if
  /// the bytes/password are rejected.
  Future<void> importClientCert(
    List<int> p12Bytes,
    String? password,
    String displayName,
  ) async {
    await _storage.write(key: _clientP12Key, value: base64Encode(p12Bytes));
    await _storage.write(key: _clientP12PasswordKey, value: password);
    await _storage.write(key: _clientCertNameKey, value: displayName);
    await _refreshState();
    await _apply();
  }

  /// Remove the imported server CA from storage. Takes full effect on restart.
  Future<void> removeServerCa() async {
    await _storage.delete(key: _serverCaKey);
    await _storage.delete(key: _serverCaNameKey);
    _appliedCaFingerprint = null;
    await _refreshState();
  }

  /// Remove the imported client certificate from storage. Takes full effect on restart.
  Future<void> removeClientCert() async {
    await _storage.delete(key: _clientP12Key);
    await _storage.delete(key: _clientP12PasswordKey);
    await _storage.delete(key: _clientCertNameKey);
    _appliedClientCertFingerprint = null;
    await _refreshState();
  }

  /// Validate that [p12Bytes] + [password] form a loadable PKCS#12 client
  /// certificate, without touching the shared default context. Returns null on
  /// success or a human-readable error message on failure.
  static String? validateClientCert(List<int> p12Bytes, String? password) {
    try {
      final ctx = SecurityContext(withTrustedRoots: false);
      ctx.useCertificateChainBytes(p12Bytes, password: password);
      ctx.usePrivateKeyBytes(p12Bytes, password: password);
      return null;
    } on TlsException catch (e) {
      return describeCertError(e, password: password);
    } catch (e) {
      return e.toString();
    }
  }

  /// Validate that [bytes] parse as one or more trusted certificates.
  /// Returns null on success or an error message on failure.
  static String? validateServerCa(List<int> bytes) {
    try {
      final ctx = SecurityContext(withTrustedRoots: false);
      ctx.setTrustedCertificatesBytes(bytes);
      return null;
    } on TlsException catch (e) {
      return describeCertError(e);
    } catch (e) {
      return e.toString();
    }
  }

  /// Map a [TlsException] from certificate loading to a friendlier message.
  static String describeCertError(TlsException e, {String? password}) {
    final msg = e.osError?.message ?? e.message;
    final lower = msg.toLowerCase();
    if (lower.contains('mac verify') ||
        lower.contains('password') ||
        lower.contains('decrypt')) {
      return password == null || password.isEmpty
          ? 'This certificate is password-protected. Enter its password.'
          : 'Incorrect certificate password.';
    }
    return 'Could not read certificate: $msg';
  }

  Future<void> _apply() async {
    // Server CA trust.
    final caBytes = await _readBytes(_serverCaKey);
    if (caBytes != null) {
      final fp = _fingerprint(caBytes);
      if (fp != _appliedCaFingerprint) {
        try {
          SecurityContext.defaultContext.setTrustedCertificatesBytes(caBytes);
          _appliedCaFingerprint = fp;
          _log.info('Applied user server CA to trust store');
        } catch (e) {
          // Duplicate adds can throw on some platforms; non-fatal.
          _log.warning('Could not apply server CA: $e');
        }
      }
    }

    // Client certificate (mTLS).
    final p12 = await _readBytes(_clientP12Key);
    if (p12 != null) {
      final fp = _fingerprint(p12);
      if (fp != _appliedClientCertFingerprint) {
        final pw = await _storage.read(key: _clientP12PasswordKey);
        try {
          SecurityContext.defaultContext
              .useCertificateChainBytes(p12, password: pw);
          SecurityContext.defaultContext.usePrivateKeyBytes(p12, password: pw);
          _appliedClientCertFingerprint = fp;
          _log.info('Applied client certificate for mTLS');
        } catch (e) {
          _log.warning('Could not apply client certificate: $e');
        }
      }
    }
  }

  Future<void> _refreshState() async {
    state.value = CertificateState(
      serverCaName: await _storage.read(key: _serverCaNameKey),
      clientCertName: await _storage.read(key: _clientCertNameKey),
    );
  }

  Future<List<int>?> _readBytes(String key) async {
    final b64 = await _storage.read(key: key);
    if (b64 == null || b64.isEmpty) return null;
    try {
      return base64Decode(b64);
    } catch (_) {
      return null;
    }
  }

  String _fingerprint(List<int> bytes) => sha256.convert(bytes).toString();
}
