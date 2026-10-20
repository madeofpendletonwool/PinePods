// Now-playing sessions + cross-device remote control (mobile client).
//
// Holds a single persistent, authenticated WebSocket per install that reports
// this device's playback to the server (a best-effort mirror) and receives the
// user's device list plus advisory remote-control commands. The local player
// stays authoritative — inbound commands are applied to it, never the other way
// around, and a dead socket only ever costs remote control (HTTP position
// reporting remains the fallback). Mirrors web/src/requests/now_playing.rs.

import 'dart:async';
import 'dart:convert';
import 'dart:io';
import 'dart:math';

import 'package:device_info_plus/device_info_plus.dart';
import 'package:flutter/foundation.dart';
import 'package:flutter_secure_storage/flutter_secure_storage.dart';
import 'package:logging/logging.dart';
import 'package:web_socket_channel/web_socket_channel.dart';
import 'package:web_socket_channel/status.dart' as ws_status;

import 'package:pinepods_mobile/services/audio/audio_player_service.dart';
import 'package:pinepods_mobile/services/pinepods/pinepods_audio_service.dart';
import 'package:pinepods_mobile/services/pinepods/pinepods_service.dart';

/// Mirror of the server's `NowPlayingSnapshot` (see rust-api models.rs).
class NowPlayingDevice {
  final String deviceId;
  final String deviceName;
  final String deviceType;
  final int episodeId;
  final bool isYoutube;
  final String title;
  final String artworkUrl;
  final double positionSec;
  final double durationSec;
  final bool playing;
  final double speed;
  final int updatedAt;

  /// Actions the device says it applies. Empty: it predates the `capabilities`
  /// connect parameter, so it handles [legacyActions].
  final List<String> capabilities;

  /// What every client applied before devices could list their actions.
  static const legacyActions = [
    'play',
    'pause',
    'seek',
    'skip_forward',
    'skip_back',
    'set_speed',
    'play_episode',
  ];

  const NowPlayingDevice({
    required this.deviceId,
    required this.deviceName,
    required this.deviceType,
    required this.episodeId,
    required this.isYoutube,
    required this.title,
    required this.artworkUrl,
    required this.positionSec,
    required this.durationSec,
    required this.playing,
    required this.speed,
    required this.updatedAt,
    this.capabilities = const [],
  });

  /// Whether this device applies [action], so the UI can hide controls it lacks.
  bool supports(String action) => capabilities.isEmpty
      ? legacyActions.contains(action)
      : capabilities.contains(action);

  static double _toDouble(dynamic v) => (v is num) ? v.toDouble() : 0.0;
  static int _toInt(dynamic v) => (v is num) ? v.toInt() : 0;

  factory NowPlayingDevice.fromJson(Map<String, dynamic> json) {
    return NowPlayingDevice(
      deviceId: json['device_id'] as String? ?? '',
      deviceName: json['device_name'] as String? ?? '',
      deviceType: json['device_type'] as String? ?? '',
      episodeId: _toInt(json['episode_id']),
      isYoutube: json['is_youtube'] as bool? ?? false,
      title: json['title'] as String? ?? '',
      artworkUrl: json['artwork_url'] as String? ?? '',
      positionSec: _toDouble(json['position_sec']),
      durationSec: _toDouble(json['duration_sec']),
      playing: json['playing'] as bool? ?? false,
      speed: _toDouble(json['speed']),
      updatedAt: _toInt(json['updated_at']),
      capabilities:
          (json['capabilities'] as List?)?.whereType<String>().toList() ??
              const [],
    );
  }
}

/// Manages the now-playing socket, exposes the user's other devices to the UI
/// (via [devices] / [connected]), and applies inbound remote commands to the
/// local player. Best-effort throughout: playback never blocks on this service.
class NowPlayingService {
  NowPlayingService({
    required AudioPlayerService audioPlayerService,
    required PinepodsService pinepodsService,
    required PinepodsAudioService pinepodsAudioService,
  })  : _audioPlayerService = audioPlayerService,
        _pinepodsService = pinepodsService,
        _pinepodsAudioService = pinepodsAudioService;

  final log = Logger('NowPlayingService');

  final AudioPlayerService _audioPlayerService;
  final PinepodsService _pinepodsService;
  final PinepodsAudioService _pinepodsAudioService;

  static const _storage = FlutterSecureStorage();
  static const _deviceIdKey = 'pinepods_device_id';

  /// The user's other active devices (this device is excluded by the server).
  final ValueNotifier<List<NowPlayingDevice>> devices =
      ValueNotifier<List<NowPlayingDevice>>(const []);

  /// Whether the socket is currently open.
  final ValueNotifier<bool> connected = ValueNotifier<bool>(false);

  /// Last command the server couldn't deliver (target offline), for the UI to
  /// surface; null when there's nothing to report.
  final ValueNotifier<String?> lastCommandError = ValueNotifier<String?>(null);

  /// Close code the server sends when a newer connection took over our
  /// device_id (rust-api handlers/nowplaying.rs `CLOSE_REPLACED`).
  static const _closeReplaced = 4000;

  // What this device last reported, so "Send to" can hand it off.
  int? _reportedEpisodeId;
  bool _reportedIsYoutube = false;
  double _reportedPositionSec = 0;

  // cmd_id of an in-flight "Send to" handoff; we pause locally only once the
  // server acks delivery, so an offline target doesn't strand playback.
  String? _pendingHandoffCmdId;

  int _cmdSeq = 0;

  WebSocketChannel? _channel;
  StreamSubscription? _channelSub;
  Timer? _reconnectTimer;

  String? _deviceId;
  String? _deviceName;
  String _deviceType = 'Mobile';

  /// The actions [_applyCommand] handles, announced on connect.
  static const _capabilities =
      'play,pause,seek,skip_forward,skip_back,set_speed,play_episode';

  // Credentials of the currently-connected session; reused for `play_episode`.
  String? _server;
  String? _apiKey;
  int? _userId;

  /// Set true by [disconnect] so an intentional close doesn't trigger a reconnect.
  bool _closing = false;

  /// Bumped on every [connect]/[_teardown] so an in-flight connect whose async
  /// setup was superseded by a newer call bails out instead of leaking a socket.
  int _connectGen = 0;

  String? get selfDeviceId => _deviceId;

  /// Stable per-install device id, persisted in secure storage.
  Future<String> _getOrCreateDeviceId() async {
    if (_deviceId != null) return _deviceId!;
    try {
      final existing = await _storage.read(key: _deviceIdKey);
      if (existing != null && existing.isNotEmpty) {
        _deviceId = existing;
        return existing;
      }
    } catch (e) {
      log.fine('Could not read device id from secure storage: $e');
    }
    final generated = _generateId();
    try {
      await _storage.write(key: _deviceIdKey, value: generated);
    } catch (e) {
      log.fine('Could not persist device id (continuing): $e');
    }
    _deviceId = generated;
    return generated;
  }

  /// Take a fresh device id. Used when the server says another live connection
  /// holds ours (e.g. secure storage restored onto a second phone), so the two
  /// installs stop evicting each other.
  Future<void> _rotateDeviceId() async {
    final generated = _generateId();
    try {
      await _storage.write(key: _deviceIdKey, value: generated);
    } catch (e) {
      log.fine('Could not persist rotated device id (continuing): $e');
    }
    _deviceId = generated;
  }

  String _generateId() {
    final rand = Random.secure();
    final bytes = List<int>.generate(16, (_) => rand.nextInt(256));
    return bytes.map((b) => b.toRadixString(16).padLeft(2, '0')).join();
  }

  Future<void> _resolveDeviceInfo() async {
    _deviceType = Platform.isIOS ? 'iOS' : 'Android';
    if (_deviceName != null) return;
    try {
      final info = DeviceInfoPlugin();
      if (Platform.isIOS) {
        final ios = await info.iosInfo;
        _deviceName = ios.name.isNotEmpty ? ios.name : ios.utsname.machine;
      } else {
        final android = await info.androidInfo;
        _deviceName = android.model.isNotEmpty ? android.model : 'Android';
      }
    } catch (e) {
      log.fine('Could not read device info (continuing): $e');
      _deviceName = _deviceType;
    }
  }

  /// Open (or re-target) the now-playing socket for the given session. Idempotent:
  /// a no-op when already connected to the same server/user. Reconnection is
  /// best-effort — a dropped socket costs remote control, never local playback.
  Future<void> connect({
    required String server,
    required String apiKey,
    required int userId,
  }) async {
    final normalizedServer = server.trim().replaceAll(RegExp(r'/$'), '');

    // Already connected to this exact session — nothing to do.
    if (_channel != null &&
        _server == normalizedServer &&
        _apiKey == apiKey &&
        _userId == userId) {
      return;
    }

    // Switching sessions (or first connect): tear down any existing socket.
    _teardown();
    _closing = false;
    final gen = ++_connectGen;

    _server = normalizedServer;
    _apiKey = apiKey;
    _userId = userId;

    final deviceId = await _getOrCreateDeviceId();
    await _resolveDeviceInfo();

    // A newer connect/teardown superseded us while awaiting the setup above.
    if (_closing || gen != _connectGen) return;

    final clean = normalizedServer
        .replaceFirst('https://', '')
        .replaceFirst('http://', '');
    final proto = normalizedServer.startsWith('https://') ? 'wss' : 'ws';
    final uri = Uri.parse(
      '$proto://$clean/ws/api/nowplaying/$userId'
      '?api_key=${Uri.encodeComponent(apiKey)}'
      '&device_id=${Uri.encodeComponent(deviceId)}'
      '&device_name=${Uri.encodeComponent(_deviceName ?? _deviceType)}'
      '&device_type=${Uri.encodeComponent(_deviceType)}'
      '&capabilities=${Uri.encodeComponent(_capabilities)}',
    );

    try {
      final channel = WebSocketChannel.connect(uri);
      _channel = channel;
      _channelSub = channel.stream.listen(
        _handleInbound,
        onError: (e) {
          log.fine('Now-playing socket error: $e');
          _onDisconnected();
        },
        onDone: () => _onDisconnected(closeCode: channel.closeCode),
        cancelOnError: true,
      );
      connected.value = true;
      log.info('Now-playing socket connecting to $proto://$clean (device $deviceId)');
    } catch (e) {
      log.fine('Now-playing socket failed to open (continuing): $e');
      _onDisconnected();
    }
  }

  void _onDisconnected({int? closeCode}) {
    connected.value = false;
    devices.value = const [];
    _channelSub?.cancel();
    _channelSub = null;
    _channel = null;
    if (_closing) return;
    _reconnectTimer?.cancel();
    final server = _server;
    final apiKey = _apiKey;
    final userId = _userId;
    if (server == null || apiKey == null || userId == null) return;
    // Another live connection took over our id: rotate it and reconnect promptly,
    // rather than reclaiming the old id and evicting the other one in a loop.
    final replaced = closeCode == _closeReplaced;
    if (replaced) log.info('Now-playing device id taken over elsewhere; rotating');
    _reconnectTimer = Timer(Duration(seconds: replaced ? 1 : 10), () async {
      if (_closing) return;
      if (replaced) await _rotateDeviceId();
      connect(server: server, apiKey: apiKey, userId: userId);
    });
  }

  void _teardown() {
    _connectGen++;
    _reconnectTimer?.cancel();
    _reconnectTimer = null;
    _channelSub?.cancel();
    _channelSub = null;
    final channel = _channel;
    _channel = null;
    if (channel != null) {
      try {
        channel.sink.close(ws_status.normalClosure);
      } catch (_) {}
    }
    connected.value = false;
    devices.value = const [];
  }

  /// Intentionally close the socket (e.g. on logout). Cancels reconnection.
  void disconnect() {
    _closing = true;
    _teardown();
    _server = null;
    _apiKey = null;
    _userId = null;
  }

  void _send(Map<String, dynamic> msg) {
    final channel = _channel;
    if (channel == null) return;
    try {
      channel.sink.add(jsonEncode(msg));
    } catch (e) {
      log.fine('Now-playing send failed (continuing): $e');
    }
  }

  /// Report this device's current playback state (best-effort). Called from the
  /// audio service's periodic tick; also refreshes the snapshot TTL while paused.
  void reportNowPlaying({
    required int episodeId,
    required bool isYoutube,
    required String title,
    required String artworkUrl,
    required double positionSec,
    required double durationSec,
    required bool playing,
    required double speed,
  }) {
    _reportedEpisodeId = episodeId;
    _reportedIsYoutube = isYoutube;
    _reportedPositionSec = positionSec;
    _send({
      'type': 'report',
      'episode_id': episodeId,
      'is_youtube': isYoutube,
      'title': title,
      'artwork_url': artworkUrl,
      'position_sec': positionSec,
      'duration_sec': durationSec,
      'playing': playing,
      'speed': speed,
    });
  }

  void sendHeartbeat() => _send({'type': 'heartbeat'});

  /// Send an advisory remote-control command to another device. If the server
  /// reports the target offline, [lastCommandError] is set.
  /// Returns the correlation id the server echoes back in its `ack`.
  String sendCommand(String targetDeviceId, String action,
      [Map<String, dynamic> args = const {}]) {
    lastCommandError.value = null;
    final cmdId = '${++_cmdSeq}';
    _send({
      'type': 'command',
      'target_device_id': targetDeviceId,
      'action': action,
      'args': args,
      'cmd_id': cmdId,
    });
    return cmdId;
  }

  /// "Play on ▸": start an episode on another device. With no position the
  /// target resumes from the server-stored listen position.
  void playOnDevice(String targetDeviceId, int episodeId, bool isYoutube) {
    sendCommand(targetDeviceId, 'play_episode', {
      'episode_id': episodeId,
      'is_youtube': isYoutube,
    });
  }

  /// Whether something is loaded here that [sendPlaybackTo] could hand off.
  bool get hasLocalPlayback =>
      _reportedEpisodeId != null && _audioPlayerService.nowPlaying != null;

  /// "Send to ▸": hand off what this device is playing to another device at the
  /// live position — the push mirror of [playHere]. The local player pauses once
  /// the server acks delivery. Returns false when nothing is playing here.
  bool sendPlaybackTo(String targetDeviceId) {
    final episodeId = _reportedEpisodeId;
    if (episodeId == null || _audioPlayerService.nowPlaying == null) return false;
    final live = _audioPlayerService.playPosition?.valueOrNull;
    final positionSec =
        live?.position.inSeconds.toDouble() ?? _reportedPositionSec;
    _pendingHandoffCmdId = sendCommand(targetDeviceId, 'play_episode', {
      'episode_id': episodeId,
      'is_youtube': _reportedIsYoutube,
      'position_sec': positionSec,
    });
    return true;
  }

  void _handleInbound(dynamic raw) {
    if (raw is! String) return;
    Map<String, dynamic> msg;
    try {
      final decoded = jsonDecode(raw);
      if (decoded is! Map) return;
      msg = decoded.cast<String, dynamic>();
    } catch (_) {
      return;
    }

    switch (msg['type'] as String?) {
      case 'devices':
        final list = (msg['devices'] as List?) ?? const [];
        devices.value = list
            .whereType<Map>()
            .map((d) => NowPlayingDevice.fromJson(d.cast<String, dynamic>()))
            .where((d) => d.deviceId != _deviceId)
            .toList();
        break;
      case 'command':
        final action = msg['action'] as String?;
        if (action == null) break;
        final args = (msg['args'] is Map)
            ? (msg['args'] as Map).cast<String, dynamic>()
            : <String, dynamic>{};
        // Advisory: apply to our local, authoritative player.
        unawaited(_applyCommand(action, args));
        break;
      case 'ack':
        final cmdId = msg['cmd_id'] as String?;
        final handoff = cmdId != null && cmdId == _pendingHandoffCmdId;
        if (handoff) _pendingHandoffCmdId = null;
        if (msg['ok'] == true && handoff) {
          unawaited(_applyCommand('pause', const {}));
        }
        if (msg['ok'] == false) {
          final targetId = msg['target_device_id'] as String?;
          final match = devices.value.where((d) => d.deviceId == targetId);
          final name = match.isNotEmpty && match.first.deviceName.isNotEmpty
              ? match.first.deviceName
              : 'That device';
          lastCommandError.value = '$name is offline — command not delivered';
        }
        break;
    }
  }

  /// Apply an inbound remote command to the local player. Transport commands act
  /// on the current episode; `play_episode` fetches the target and starts it here.
  Future<void> _applyCommand(String action, Map<String, dynamic> args) async {
    try {
      switch (action) {
        case 'play':
          await _audioPlayerService.play();
          break;
        case 'pause':
          await _audioPlayerService.pause();
          break;
        case 'seek':
          final sec = (args['position_sec'] as num?)?.toDouble();
          if (sec != null) {
            await _audioPlayerService.seek(position: sec.clamp(0, double.infinity).round());
          }
          break;
        case 'skip_forward':
          final by = (args['seconds'] as num?)?.toDouble() ?? 30.0;
          await _seekRelative(by);
          break;
        case 'skip_back':
          final by = (args['seconds'] as num?)?.toDouble() ?? 15.0;
          await _seekRelative(-by);
          break;
        case 'set_speed':
          final speed = (args['speed'] as num?)?.toDouble();
          if (speed != null) await _audioPlayerService.setPlaybackSpeed(speed);
          break;
        case 'play_episode':
          final id = (args['episode_id'] as num?)?.toInt();
          if (id != null) {
            // A handoff carries the sender's live position; otherwise resume
            // from the server-stored one.
            await _playEpisodeById(
              id,
              args['is_youtube'] == true,
              positionSec: (args['position_sec'] as num?)?.toDouble(),
            );
          }
          break;
        default:
          log.fine('Ignoring unknown remote command: $action');
      }
    } catch (e) {
      log.warning('Failed to apply remote command "$action" (playback continues): $e');
    }
  }

  Future<void> _seekRelative(double deltaSec) async {
    // valueOrNull: playPosition is an unseeded BehaviorSubject; .value throws
    // until the first position event (e.g. a skip command right after connect).
    final pos = _audioPlayerService.playPosition?.valueOrNull;
    if (pos == null) return;
    final target = (pos.position.inSeconds + deltaSec).clamp(0, double.infinity);
    await _audioPlayerService.seek(position: target.round());
  }

  /// Take over another device's episode on *this* device ("Play here" in the
  /// UI): pause the source if it's playing (advisory) so it doesn't keep going in
  /// two places, and resume at its live position rather than the staler
  /// server-stored one. Mirrors web remote_devices.rs.
  Future<void> playHere(NowPlayingDevice source) async {
    if (source.playing && source.supports('pause')) {
      sendCommand(source.deviceId, 'pause');
    }
    await _playEpisodeById(
      source.episodeId,
      source.isYoutube,
      positionSec: source.positionSec,
    );
  }

  Future<void> _playEpisodeById(int episodeId, bool isYoutube,
      {double? positionSec}) async {
    final server = _server;
    final apiKey = _apiKey;
    final userId = _userId;
    if (server == null || apiKey == null || userId == null) return;

    _pinepodsService.setCredentials(server, apiKey);
    final episode = await _pinepodsService.getEpisodeMetadata(
      episodeId,
      userId,
      isYoutube: isYoutube,
    );
    if (episode == null) {
      log.fine('Remote play_episode: metadata not found for $episodeId');
      return;
    }
    final resumeAt = (positionSec != null && positionSec.isFinite && positionSec > 0)
        ? episode.withListenDuration(positionSec.round())
        : episode;
    await _pinepodsAudioService.playPinepodsEpisode(
      pinepodsEpisode: resumeAt,
      resume: true,
    );
  }

  void dispose() {
    disconnect();
    devices.dispose();
    connected.dispose();
    lastCommandError.dispose();
  }
}
