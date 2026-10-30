// lib/ui/widgets/pinepods_episode_card.dart
import 'dart:async';
import 'dart:math' as math;

import 'package:flutter/material.dart';
import 'package:pinepods_mobile/bloc/podcast/audio_bloc.dart';
import 'package:pinepods_mobile/bloc/settings/settings_bloc.dart';
import 'package:pinepods_mobile/entities/app_settings.dart';
import 'package:pinepods_mobile/entities/episode.dart';
import 'package:pinepods_mobile/entities/pinepods_episode.dart';
import 'package:pinepods_mobile/services/audio/audio_player_service.dart';
import 'package:pinepods_mobile/ui/utils/artwork_colors.dart';
import 'package:pinepods_mobile/ui/utils/live_progress.dart';
import 'package:pinepods_mobile/ui/widgets/lazy_network_image.dart';
import 'package:provider/provider.dart';
import 'package:pinepods_mobile/ui/icons/phosphor_icons.dart';

class PinepodsEpisodeCard extends StatefulWidget {
  final PinepodsEpisode episode;
  final VoidCallback? onTap;
  final VoidCallback? onLongPress;

  /// Starts playback. Awaited so the button's loading state always clears,
  /// even when starting fails before the player hears about the episode.
  final FutureOr<void> Function()? onPlayPressed;

  /// When true the card shows an "offline" badge, indicating the episode is
  /// available as a local download and can be played without a connection.
  final bool isLocalDownload;

  final EdgeInsetsGeometry margin;

  /// Optional widget before the artwork, e.g. a drag handle.
  final Widget? leading;

  const PinepodsEpisodeCard({
    Key? key,
    required this.episode,
    this.onTap,
    this.onLongPress,
    this.onPlayPressed,
    this.isLocalDownload = false,
    this.margin = const EdgeInsets.symmetric(horizontal: 12.0, vertical: 4.0),
    this.leading,
  }) : super(key: key);

  @override
  State<PinepodsEpisodeCard> createState() => _PinepodsEpisodeCardState();
}

class _PinepodsEpisodeCardState extends State<PinepodsEpisodeCard> {
  static const double _artSize = 56;

  bool _isLoading = false;
  AudioState _audioState = AudioState.none;
  Episode? _nowPlaying;
  PositionState? _positionState;
  AudioBloc? _audioBloc;
  StreamSubscription? _nowPlayingSub;
  StreamSubscription? _audioStateSub;
  StreamSubscription? _positionSub;

  SettingsBloc? _settingsBloc;
  StreamSubscription<AppSettings>? _settingsSub;
  String _buttonStyle = 'ring';
  bool _artworkAccent = true;

  Color? _artColor;
  Brightness? _artBrightness;

  @override
  void didChangeDependencies() {
    super.didChangeDependencies();
    final settingsBloc = Provider.of<SettingsBloc>(context, listen: false);
    if (_settingsBloc != settingsBloc) {
      _settingsSub?.cancel();
      _settingsBloc = settingsBloc;
      _applySettings(settingsBloc.currentSettings, rebuild: false);
      _settingsSub = settingsBloc.settings.listen((s) => _applySettings(s, rebuild: true));
    }
    _resolveArtColor();

    final bloc = Provider.of<AudioBloc>(context, listen: false);
    if (_audioBloc != bloc) {
      _nowPlayingSub?.cancel();
      _audioStateSub?.cancel();
      _positionSub?.cancel();
      _audioBloc = bloc;

      // Seed from the current values so a freshly built card shows the right
      // state on its first frame instead of flashing "not playing".
      _nowPlaying = bloc.nowPlaying?.valueOrNull;
      _audioState = bloc.playingState?.valueOrNull ?? AudioState.none;

      _nowPlayingSub = bloc.nowPlaying?.listen((episode) {
        if (!mounted) return;
        setState(() {
          _nowPlaying = episode;
          // Clear loading once the audio system acknowledges a new episode
          _isLoading = false;
        });
      });

      // Only the playing card cares about state changes; the rest just track
      // the value so a whole list doesn't rebuild on every transition.
      _audioStateSub = bloc.playingState?.listen((state) {
        if (!mounted) return;
        if (_isCurrentEpisode || state == AudioState.error) {
          setState(() {
            _audioState = state;
            if (state == AudioState.error) _isLoading = false;
          });
        } else {
          _audioState = state;
        }
      });

      // Position ticks fire about once a second; only the playing card rebuilds.
      _positionSub = bloc.playPosition?.listen((state) {
        if (!mounted) return;
        if (state.episode?.guid != widget.episode.episodeUrl) return;
        setState(() => _positionState = state);
      });
    }
  }

  @override
  void didUpdateWidget(PinepodsEpisodeCard oldWidget) {
    super.didUpdateWidget(oldWidget);
    if (oldWidget.episode.episodeArtwork != widget.episode.episodeArtwork ||
        oldWidget.episode.localImagePath != widget.episode.localImagePath) {
      _artBrightness = null;
      _resolveArtColor();
    }
  }

  void _applySettings(AppSettings settings, {required bool rebuild}) {
    if (settings.playButtonStyle == _buttonStyle && settings.artworkAccentColors == _artworkAccent) return;
    _buttonStyle = settings.playButtonStyle;
    _artworkAccent = settings.artworkAccentColors;
    if (rebuild && mounted) {
      setState(() {});
      _resolveArtColor();
    }
  }

  /// Artwork colours are tuned against the card surface, so they're keyed on
  /// its brightness rather than the theme's (some themes mix the two).
  void _resolveArtColor() {
    if (!_artworkAccent) return;
    final theme = Theme.of(context);
    final brightness = ThemeData.estimateBrightnessForColor(theme.cardTheme.color ?? theme.cardColor);
    if (_artBrightness == brightness) return;
    _artBrightness = brightness;

    final url = widget.episode.episodeArtwork;
    final local = widget.episode.localImagePath;
    _artColor = ArtworkColors.cached(url, local, brightness);
    if (_artColor != null) return;
    ArtworkColors.resolve(url, local, brightness).then((color) {
      if (!mounted || color == null || _artBrightness != brightness) return;
      if (url != widget.episode.episodeArtwork) return;
      setState(() => _artColor = color);
    });
  }

  @override
  void dispose() {
    _settingsSub?.cancel();
    _nowPlayingSub?.cancel();
    _audioStateSub?.cancel();
    _positionSub?.cancel();
    super.dispose();
  }

  bool get _isCurrentEpisode =>
      widget.episode.episodeUrl.isNotEmpty &&
      _nowPlaying?.guid == widget.episode.episodeUrl;

  double get _displayProgressPercentage => LiveProgressResolver.percentage(
        isCurrentEpisode: _isCurrentEpisode,
        staticPercentage: widget.episode.progressPercentage,
        livePercentage: _positionState?.percentage,
      );

  bool get _showProgress =>
      !widget.episode.completed &&
      LiveProgressResolver.shouldShowProgress(
        isCurrentEpisode: _isCurrentEpisode,
        hasStaticProgress: widget.episode.isStarted,
      );

  bool get _isPlaying =>
      _isCurrentEpisode &&
      (_audioState == AudioState.playing ||
          _audioState == AudioState.buffering ||
          _audioState == AudioState.starting);

  bool get _isPaused =>
      _isCurrentEpisode && _audioState == AudioState.pausing;

  /// "45m", or "23m left" once started.
  String get _timeLabel {
    final total = widget.episode.episodeDuration;
    if (!_showProgress) return humanDuration(total);
    final live = _isCurrentEpisode ? _positionState : null;
    final remaining = live != null && live.length.inSeconds > 0
        ? live.length.inSeconds - live.position.inSeconds
        : total - (widget.episode.listenDuration ?? 0);
    return '${humanDuration(remaining)} left';
  }

  Future<void> _onButtonTap() async {
    final bloc = _audioBloc;
    if (bloc == null) return;

    if (_isPlaying) {
      bloc.transitionState(TransitionState.pause);
    } else if (_isPaused) {
      bloc.transitionState(TransitionState.play);
    } else {
      if (_isLoading) return;
      setState(() => _isLoading = true);
      try {
        await widget.onPlayPressed?.call();
      } finally {
        if (mounted) setState(() => _isLoading = false);
      }
    }
  }

  @override
  Widget build(BuildContext context) {
    final theme = Theme.of(context);
    final accent = (_artworkAccent ? _artColor : null) ?? theme.primaryColor;
    final ringStyle = _buttonStyle == 'ring' && widget.onPlayPressed != null;
    final muted = theme.hintColor;

    final statusIcons = <Widget>[
      if (widget.isLocalDownload) Icon(PhosphorIconsFill.checkCircle, size: 14, color: Colors.green[600]),
      if (widget.episode.saved) Icon(PhosphorIconsFill.bookmarkSimple, size: 14, color: Colors.orange[600]),
      if (widget.episode.downloaded) Icon(PhosphorIconsRegular.checkCircle, size: 14, color: Colors.green[600]),
      if (widget.episode.queued) Icon(PhosphorIconsRegular.queue, size: 14, color: Colors.blue[400]),
    ];

    return Card(
      margin: widget.margin,
      child: InkWell(
        onTap: widget.onTap,
        onLongPress: widget.onLongPress,
        child: Stack(
          children: [
            Padding(
              padding: const EdgeInsets.fromLTRB(10, 10, 6, 10),
              child: Row(
                children: [
                  if (widget.leading != null) widget.leading!,
                  _artwork(accent),
                  const SizedBox(width: 12),
                  Expanded(
                    child: Column(
                      mainAxisSize: MainAxisSize.min,
                      crossAxisAlignment: CrossAxisAlignment.start,
                      children: [
                        Text(
                          widget.episode.podcastName,
                          maxLines: 1,
                          overflow: TextOverflow.ellipsis,
                          style: TextStyle(fontSize: 11.5, fontWeight: FontWeight.w600, color: accent),
                        ),
                        const SizedBox(height: 2),
                        Text(
                          widget.episode.episodeTitle,
                          maxLines: 2,
                          overflow: TextOverflow.ellipsis,
                          style: TextStyle(
                            fontSize: 14.5,
                            height: 1.25,
                            fontWeight: FontWeight.w600,
                            color: _isCurrentEpisode ? accent : null,
                          ),
                        ),
                        const SizedBox(height: 4),
                        Row(
                          children: [
                            Flexible(
                              child: Text(
                                '${widget.episode.formattedPubDate}  ·  $_timeLabel',
                                maxLines: 1,
                                overflow: TextOverflow.ellipsis,
                                style: TextStyle(fontSize: 12, color: muted),
                              ),
                            ),
                            for (final icon in statusIcons)
                              Padding(padding: const EdgeInsets.only(left: 6), child: icon),
                          ],
                        ),
                      ],
                    ),
                  ),
                  if (widget.onPlayPressed != null) ...[
                    const SizedBox(width: 4),
                    _playButton(accent),
                  ],
                ],
              ),
            ),
            // Pinned to the bottom edge so showing progress never changes the
            // card's height.
            // The ring style draws progress around the button instead.
            if (_showProgress && !ringStyle)
              Positioned(
                left: 0,
                right: 0,
                bottom: 0,
                child: LinearProgressIndicator(
                  value: (_displayProgressPercentage / 100).clamp(0.0, 1.0),
                  minHeight: 3,
                  backgroundColor: accent.withOpacity(0.12),
                  valueColor: AlwaysStoppedAnimation<Color>(accent),
                ),
              ),
          ],
        ),
      ),
    );
  }

  Widget _artwork(Color accent) {
    final image = LazyNetworkImage(
      imageUrl: widget.episode.episodeArtwork,
      localImagePath: widget.episode.localImagePath,
      width: _artSize,
      height: _artSize,
      fit: BoxFit.cover,
      borderRadius: BorderRadius.circular(10),
    );
    if (!_isCurrentEpisode) return image;
    return Stack(
      children: [
        image,
        Positioned.fill(
          child: DecoratedBox(
            decoration: BoxDecoration(
              color: Colors.black.withOpacity(0.45),
              borderRadius: BorderRadius.circular(10),
            ),
            child: Icon(
              _isPlaying ? PhosphorIconsRegular.waveform : PhosphorIconsFill.pause,
              color: Colors.white,
              size: 26,
            ),
          ),
        ),
      ],
    );
  }

  Widget _playButton(Color accent) {
    final showSpinner = _isLoading && !_isCurrentEpisode;
    final completed = widget.episode.completed && !_isCurrentEpisode;
    final onAccent = ThemeData.estimateBrightnessForColor(accent) == Brightness.dark ? Colors.white : Colors.black;
    final done = Colors.green[600]!;
    final progress = _showProgress ? (_displayProgressPercentage / 100).clamp(0.0, 1.0) : 0.0;

    final IconData icon = _isPlaying
        ? PhosphorIconsFill.pause
        : completed
            ? PhosphorIconsBold.check
            : PhosphorIconsFill.play;

    Color? fill;
    Color fg;
    Widget? decoration;
    double iconSize = 22;

    switch (_buttonStyle) {
      case 'outline':
        fill = _isPlaying ? accent : null;
        fg = _isPlaying ? onAccent : (completed ? done : accent);
        decoration = _isPlaying
            ? null
            : DecoratedBox(
                decoration: BoxDecoration(
                  shape: BoxShape.circle,
                  border: Border.all(color: (completed ? done : accent).withOpacity(0.7), width: 1.5),
                ),
              );
        break;
      case 'bare':
        fg = completed ? done : accent;
        iconSize = 30;
        break;
      case 'tinted':
        fill = _isPlaying ? accent : (completed ? done.withOpacity(0.15) : accent.withOpacity(0.14));
        fg = _isPlaying ? onAccent : (completed ? done : accent);
        iconSize = 26;
        break;
      case 'ring':
      default:
        fg = completed ? done : accent;
        decoration = CustomPaint(
          painter: _ProgressRingPainter(
            progress: completed ? 1.0 : progress,
            color: completed ? done : accent,
            trackColor: (completed ? done : accent).withOpacity(0.2),
            fillColor: _isPlaying ? accent : null,
          ),
        );
        if (_isPlaying) fg = onAccent;
        iconSize = 20;
        break;
    }

    // Spinner sits on the ring itself for the ring style, inside otherwise.
    final spinner = _buttonStyle == 'ring' || _buttonStyle == 'outline'
        ? SizedBox.expand(child: CircularProgressIndicator(strokeWidth: 2.5, color: accent))
        : Padding(
            padding: const EdgeInsets.all(12),
            child: CircularProgressIndicator(strokeWidth: 2.2, color: fg),
          );

    // The play triangle's visual centre sits left of its box centre.
    final glyph = Icon(icon, color: fg, size: iconSize);
    final iconWidget = icon == PhosphorIconsFill.play
        ? Transform.translate(offset: Offset(iconSize * 0.06, 0), child: glyph)
        : glyph;

    return Tooltip(
      message: _isPlaying ? 'Pause' : 'Play',
      child: Material(
        color: fill ?? Colors.transparent,
        shape: const CircleBorder(),
        clipBehavior: Clip.antiAlias,
        child: InkWell(
          onTap: _onButtonTap,
          child: SizedBox(
            width: 44,
            height: 44,
            child: Stack(
              alignment: Alignment.center,
              children: [
                if (decoration != null && !(showSpinner && _buttonStyle == 'ring')) Positioned.fill(child: decoration),
                AnimatedSwitcher(
                  duration: const Duration(milliseconds: 180),
                  child: showSpinner
                      ? KeyedSubtree(key: const ValueKey('loading'), child: spinner)
                      : KeyedSubtree(key: ValueKey(icon), child: iconWidget),
                ),
              ],
            ),
          ),
        ),
      ),
    );
  }
}

class _ProgressRingPainter extends CustomPainter {
  final double progress;
  final Color color;
  final Color trackColor;
  final Color? fillColor;

  _ProgressRingPainter({
    required this.progress,
    required this.color,
    required this.trackColor,
    this.fillColor,
  });

  static const double _stroke = 2.5;

  @override
  void paint(Canvas canvas, Size size) {
    final center = size.center(Offset.zero);
    final radius = (math.min(size.width, size.height) - _stroke) / 2;

    if (fillColor != null) {
      canvas.drawCircle(center, radius - _stroke - 1.5, Paint()..color = fillColor!);
    }

    final track = Paint()
      ..style = PaintingStyle.stroke
      ..strokeWidth = _stroke
      ..color = trackColor;
    canvas.drawCircle(center, radius, track);

    if (progress <= 0) return;
    final arc = Paint()
      ..style = PaintingStyle.stroke
      ..strokeWidth = _stroke
      ..strokeCap = StrokeCap.round
      ..color = color;
    canvas.drawArc(
      Rect.fromCircle(center: center, radius: radius),
      -math.pi / 2,
      2 * math.pi * progress,
      false,
      arc,
    );
  }

  @override
  bool shouldRepaint(_ProgressRingPainter old) =>
      old.progress != progress || old.color != color || old.trackColor != trackColor || old.fillColor != fillColor;
}

/// Compact human duration: "1h 5m", "45m", "<1m".
String humanDuration(int seconds) {
  if (seconds <= 0) return '0m';
  final h = seconds ~/ 3600;
  final m = (seconds % 3600) ~/ 60;
  if (h > 0) return m > 0 ? '${h}h ${m}m' : '${h}h';
  return m > 0 ? '${m}m' : '<1m';
}
