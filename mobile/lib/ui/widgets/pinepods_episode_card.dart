// lib/ui/widgets/pinepods_episode_card.dart
import 'dart:async';

import 'package:flutter/material.dart';
import 'package:pinepods_mobile/bloc/podcast/audio_bloc.dart';
import 'package:pinepods_mobile/entities/episode.dart';
import 'package:pinepods_mobile/entities/pinepods_episode.dart';
import 'package:pinepods_mobile/services/audio/audio_player_service.dart';
import 'package:pinepods_mobile/ui/widgets/lazy_network_image.dart';
import 'package:provider/provider.dart';

class PinepodsEpisodeCard extends StatefulWidget {
  final PinepodsEpisode episode;
  final VoidCallback? onTap;
  final VoidCallback? onLongPress;
  final VoidCallback? onPlayPressed;

  /// When true the card shows an "offline" badge, indicating the episode is
  /// available as a local download and can be played without a connection.
  final bool isLocalDownload;

  const PinepodsEpisodeCard({
    Key? key,
    required this.episode,
    this.onTap,
    this.onLongPress,
    this.onPlayPressed,
    this.isLocalDownload = false,
  }) : super(key: key);

  @override
  State<PinepodsEpisodeCard> createState() => _PinepodsEpisodeCardState();
}

class _PinepodsEpisodeCardState extends State<PinepodsEpisodeCard> {
  bool _isLoading = false;
  AudioState _audioState = AudioState.none;
  Episode? _nowPlaying;
  AudioBloc? _audioBloc;
  StreamSubscription? _nowPlayingSub;
  StreamSubscription? _audioStateSub;

  @override
  void didChangeDependencies() {
    super.didChangeDependencies();
    final bloc = Provider.of<AudioBloc>(context, listen: false);
    if (_audioBloc != bloc) {
      _nowPlayingSub?.cancel();
      _audioStateSub?.cancel();
      _audioBloc = bloc;

      _nowPlayingSub = bloc.nowPlaying?.listen((episode) {
        if (mounted) {
          setState(() {
            _nowPlaying = episode;
            // Clear loading once the audio system acknowledges a new episode
            _isLoading = false;
          });
        }
      });

      _audioStateSub = bloc.playingState?.listen((state) {
        if (mounted) {
          setState(() {
            _audioState = state;
            if (state == AudioState.error) _isLoading = false;
          });
        }
      });
    }
  }

  @override
  void dispose() {
    _nowPlayingSub?.cancel();
    _audioStateSub?.cancel();
    super.dispose();
  }

  bool get _isCurrentEpisode =>
      widget.episode.episodeUrl.isNotEmpty &&
      _nowPlaying?.guid == widget.episode.episodeUrl;

  bool get _isPlaying =>
      _isCurrentEpisode &&
      (_audioState == AudioState.playing ||
          _audioState == AudioState.buffering ||
          _audioState == AudioState.starting);

  bool get _isPaused =>
      _isCurrentEpisode && _audioState == AudioState.pausing;

  void _onButtonTap() {
    final bloc = _audioBloc;
    if (bloc == null) return;

    if (_isPlaying) {
      // Pause the already-loaded episode — same as mini player pause button
      bloc.transitionState(TransitionState.pause);
    } else if (_isPaused) {
      // Resume the already-loaded episode — same as mini player play button
      bloc.transitionState(TransitionState.play);
    } else {
      // Episode not in player — full load
      if (_isLoading) return;
      setState(() => _isLoading = true);
      widget.onPlayPressed?.call();
    }
  }

  @override
  Widget build(BuildContext context) {
    final showSpinner = _isLoading && !_isCurrentEpisode;

    IconData playIcon;
    Color iconColor;
    if (_isPlaying) {
      playIcon = Icons.pause_circle;
      iconColor = Theme.of(context).primaryColor;
    } else if (widget.episode.completed && !_isPaused) {
      playIcon = Icons.check_circle;
      iconColor = Colors.green;
    } else if (widget.episode.listenDuration != null &&
        widget.episode.listenDuration! > 0) {
      playIcon = Icons.play_circle_filled;
      iconColor = Theme.of(context).primaryColor;
    } else {
      playIcon = Icons.play_circle_outline;
      iconColor = Theme.of(context).primaryColor;
    }

    return Card(
      margin: const EdgeInsets.symmetric(horizontal: 12.0, vertical: 4.0),
      elevation: 1,
      child: InkWell(
        onTap: widget.onTap,
        onLongPress: widget.onLongPress,
        borderRadius: BorderRadius.circular(8),
        child: Padding(
          padding: const EdgeInsets.all(12.0),
          child: Row(
            crossAxisAlignment: CrossAxisAlignment.start,
            children: [
              LazyNetworkImage(
                imageUrl: widget.episode.episodeArtwork,
                width: 50,
                height: 50,
                fit: BoxFit.cover,
                borderRadius: BorderRadius.circular(6),
              ),
              const SizedBox(width: 12),

              Expanded(
                child: Column(
                  crossAxisAlignment: CrossAxisAlignment.start,
                  children: [
                    Text(
                      widget.episode.episodeTitle,
                      style: TextStyle(
                        fontSize: 14,
                        fontWeight: FontWeight.w600,
                        color: _isCurrentEpisode
                            ? Theme.of(context).primaryColor
                            : null,
                      ),
                      maxLines: 2,
                      overflow: TextOverflow.ellipsis,
                    ),
                    const SizedBox(height: 2),
                    Text(
                      widget.episode.podcastName,
                      style: TextStyle(
                        fontSize: 12,
                        color: Theme.of(context).primaryColor,
                        fontWeight: FontWeight.w500,
                      ),
                      maxLines: 1,
                      overflow: TextOverflow.ellipsis,
                    ),
                    const SizedBox(height: 4),
                    Row(
                      children: [
                        Text(
                          widget.episode.formattedPubDate,
                          style: TextStyle(
                            fontSize: 11,
                            color: Colors.grey[600],
                          ),
                        ),
                        const SizedBox(width: 8),
                        Text(
                          widget.episode.formattedDuration,
                          style: TextStyle(
                            fontSize: 11,
                            color: Colors.grey[600],
                          ),
                        ),
                      ],
                    ),

                    if (widget.episode.isStarted) ...[
                      const SizedBox(height: 6),
                      LinearProgressIndicator(
                        value: widget.episode.progressPercentage / 100,
                        backgroundColor: Theme.of(context).colorScheme.surfaceVariant,
                        valueColor: AlwaysStoppedAnimation<Color>(
                          Theme.of(context).primaryColor,
                        ),
                        minHeight: 2,
                      ),
                    ],
                  ],
                ),
              ),

              // Status indicators and play button
              Column(
                children: [
                  if (widget.onPlayPressed != null)
                    SizedBox(
                      width: 32,
                      height: 32,
                      child: AnimatedSwitcher(
                        duration: const Duration(milliseconds: 200),
                        child: showSpinner
                            ? Padding(
                                key: const ValueKey('loading'),
                                padding: const EdgeInsets.all(4.0),
                                child: CircularProgressIndicator(
                                  strokeWidth: 2.5,
                                  valueColor: AlwaysStoppedAnimation<Color>(
                                    Theme.of(context).primaryColor,
                                  ),
                                ),
                              )
                            : GestureDetector(
                                key: ValueKey(playIcon),
                                behavior: HitTestBehavior.opaque,
                                onTap: _onButtonTap,
                                child: Icon(
                                  playIcon,
                                  color: iconColor,
                                  size: 28,
                                ),
                              ),
                      ),
                    ),

                  const SizedBox(height: 4),
                  Row(
                    mainAxisSize: MainAxisSize.min,
                    children: [
                      if (widget.isLocalDownload)
                        Padding(
                          padding: const EdgeInsets.only(left: 4),
                          child: Icon(
                            Icons.offline_pin,
                            size: 16,
                            color: Colors.green[600],
                          ),
                        ),
                      if (widget.episode.saved)
                        Icon(
                          Icons.bookmark,
                          size: 16,
                          color: Colors.orange[600],
                        ),
                      if (widget.episode.downloaded)
                        Padding(
                          padding: const EdgeInsets.only(left: 4),
                          child: Icon(
                            Icons.download_done,
                            size: 16,
                            color: Colors.green[600],
                          ),
                        ),
                      if (widget.episode.queued)
                        Padding(
                          padding: const EdgeInsets.only(left: 4),
                          child: Icon(
                            Icons.queue_music,
                            size: 16,
                            color: Colors.blue[600],
                          ),
                        ),
                    ],
                  ),
                ],
              ),
            ],
          ),
        ),
      ),
    );
  }
}
