// lib/services/network/network_status.dart
import 'package:connectivity_plus/connectivity_plus.dart';

/// Small helper around [Connectivity] used to gate *automatic* downloads on the
/// user's network type. Manual, user-initiated downloads never consult this.
class NetworkStatus {
  /// Returns true when the device currently has a WiFi (or wired/ethernet)
  /// connection. `checkConnectivity` can report multiple active interfaces, so
  /// we treat any non-cellular link as "on WiFi" for download-gating purposes.
  static Future<bool> isOnWifi() async {
    try {
      final results = await Connectivity().checkConnectivity();
      return results.any((r) =>
          r == ConnectivityResult.wifi || r == ConnectivityResult.ethernet);
    } catch (_) {
      // If we cannot determine connectivity, fail open so downloads are not
      // silently blocked forever.
      return true;
    }
  }

  /// Returns true when automatic downloads are permitted given the WiFi-only
  /// [wifiOnly] preference and the current connection.
  static Future<bool> canAutoDownload({required bool wifiOnly}) async {
    if (!wifiOnly) return true;
    return isOnWifi();
  }
}
