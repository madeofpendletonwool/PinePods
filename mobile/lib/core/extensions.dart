// Copyright 2020 Ben Hills and the project contributors. All rights reserved.
// Use of this source code is governed by a BSD-style license that can be
// found in the LICENSE file.

import 'dart:math';

extension IterableExtensions<E> on Iterable<E> {
  Iterable<List<E>> chunk(int size) sync* {
    if (length <= 0) {
      yield [];
      return;
    }

    var skip = 0;

    while (skip < length) {
      final chunk = this.skip(skip).take(size);

      yield chunk.toList(growable: false);

      skip += size;

      if (chunk.length < size) {
        return;
      }
    }
  }
}

/// Which http URLs [ExtString.forceHttps] leaves alone.
class HttpsUpgrade {
  HttpsUpgrade._();

  /// Host of the user's own Pinepods server. The app already talks to it over
  /// the scheme the user chose, so its stream and artwork URLs are left as is.
  static String? serverHost;

  static void setServer(String? url) {
    final host = url == null ? null : Uri.tryParse(url.trim())?.host.toLowerCase();
    serverHost = (host == null || host.isEmpty) ? null : host;
  }

  static const _localSuffixes = ['.local', '.lan', '.home.arpa', '.internal', '.ts.net'];

  /// True for hosts that can't be public: loopback, private, link-local and
  /// CGNAT (Tailscale) addresses, single-label names and local-only domains.
  /// These often speak only http, and an https upgrade would break them.
  static bool isLocalHost(String rawHost) {
    var host = rawHost.toLowerCase();
    if (host.startsWith('[') && host.endsWith(']')) host = host.substring(1, host.length - 1);
    if (host.isEmpty) return false;
    if (host == serverHost) return true;
    if (host == 'localhost' || host.endsWith('.localhost')) return true;
    if (_localSuffixes.any(host.endsWith)) return true;

    if (host.contains(':')) return _isLocalIpv6(host);

    final octets = host.split('.');
    if (octets.length == 4 && octets.every((o) => int.tryParse(o) != null)) {
      final a = int.parse(octets[0]);
      final b = int.parse(octets[1]);
      return a == 10 ||
          a == 127 ||
          (a == 172 && b >= 16 && b <= 31) ||
          (a == 192 && b == 168) ||
          (a == 169 && b == 254) ||
          (a == 100 && b >= 64 && b <= 127);
    }

    // A single-label name (no dot) only resolves on a local network.
    return !host.contains('.');
  }

  static bool _isLocalIpv6(String host) {
    if (host == '::1') return true;
    final first = int.tryParse(host.split(':').first, radix: 16);
    if (first == null) return false;
    return (first & 0xfe00) == 0xfc00 || // fc00::/7 unique local
        (first & 0xffc0) == 0xfe80; // fe80::/10 link-local
  }
}

extension ExtString on String? {
  /// Upgrades a public http URL to https. Local and private hosts, and the
  /// user's own server, keep http.
  String get forceHttps {
    if (this != null) {
      final url = Uri.tryParse(this!);

      if (url == null || !url.isScheme('http')) return this!;
      if (HttpsUpgrade.isLocalHost(url.host)) return this!;

      return url.replace(scheme: 'https').toString();
    }

    return this ?? '';
  }
}

extension ExtDouble on double {
  double get toTenth {
    var mod = pow(10.0, 1).toDouble();
    return ((this * mod).round().toDouble() / mod);
  }
}
