// lib/ui/widgets/pinepods_episode_card.dart
import 'dart:async';

import 'package:flutter/material.dart';
import 'package:pinepods_mobile/bloc/podcast/audio_bloc.dart';
import 'package:pinepods_mobile/entities/episode.dart';
import 'package:pinepods_mobile/entities/pinepods_episode.dart';
import 'package:pinepods_mobile/services/audio/audio_player_service.dart';
import 'package:pinepods_mobile/ui/utils/live_progress.dart';
import 'package:pinepods_mobile/ui/widgets/lazy_network_image.dart';
import 'package:provider/provider.dart';

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

  @override
  void didChangeDependencies() {
    super.didChangeDependencies();
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
  void dispose() {
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
    final accent = theme.primaryColor;
    final muted = theme.hintColor;

    final statusIcons = <Widget>[
      if (widget.isLocalDownload) Icon(Icons.offline_pin, size: 14, color: Colors.green[600]),
      if (widget.episode.saved) Icon(Icons.bookmark, size: 14, color: Colors.orange[600]),
      if (widget.episode.downloaded) Icon(Icons.download_done, size: 14, color: Colors.green[600]),
      if (widget.episode.queued) Icon(Icons.queue_music, size: 14, color: Colors.blue[400]),
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
            if (_showProgress)
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
              _isPlaying ? Icons.graphic_eq : Icons.pause,
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

    final Color fg;
    final Color bg;
    final IconData icon;
    if (_isPlaying) {
      bg = accent;
      fg = ThemeData.estimateBrightnessForColor(accent) == Brightness.dark ? Colors.white : Colors.black;
      icon = Icons.pause_rounded;
    } else if (completed) {
      bg = Colors.green.withOpacity(0.15);
      fg = Colors.green[600]!;
      icon = Icons.check_rounded;
    } else {
      bg = accent.withOpacity(0.14);
      fg = accent;
      icon = Icons.play_arrow_rounded;
    }

    return Tooltip(
      message: _isPlaying ? 'Pause' : 'Play',
      child: Material(
        color: bg,
        shape: const CircleBorder(),
        clipBehavior: Clip.antiAlias,
        child: InkWell(
          onTap: _onButtonTap,
          child: SizedBox(
            width: 44,
            height: 44,
            child: AnimatedSwitcher(
              duration: const Duration(milliseconds: 180),
              child: showSpinner
                  ? Padding(
                      key: const ValueKey('loading'),
                      padding: const EdgeInsets.all(12),
                      child: CircularProgressIndicator(strokeWidth: 2.2, color: fg),
                    )
                  : Icon(icon, key: ValueKey(icon), color: fg, size: 28),
            ),
          ),
        ),
      ),
    );
  }
}

/// Compact human duration: "1h 5m", "45m", "<1m".
String humanDuration(int seconds) {
  if (seconds <= 0) return '0m';
  final h = seconds ~/ 3600;
  final m = (seconds % 3600) ~/ 60;
  if (h > 0) return m > 0 ? '${h}h ${m}m' : '${h}h';
  return m > 0 ? '${m}m' : '<1m';
}
