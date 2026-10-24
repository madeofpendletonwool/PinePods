// The full-screen player, opened from the mini player.
//
// One surface: artwork, title, scrub bar, transport and a row of secondary
// actions. Up Next, chapters and show notes open as sheets above it. Each part
// listens only to the stream it shows, so a position tick repaints the scrub
// bar and nothing else.

import 'dart:async';
import 'dart:math' as math;

import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:pinepods_mobile/bloc/podcast/audio_bloc.dart';
import 'package:pinepods_mobile/bloc/settings/settings_bloc.dart';
import 'package:pinepods_mobile/entities/chapter.dart';
import 'package:pinepods_mobile/entities/episode.dart';
import 'package:pinepods_mobile/entities/sleep.dart';
import 'package:pinepods_mobile/l10n/L.dart';
import 'package:pinepods_mobile/services/audio/audio_player_service.dart';
import 'package:pinepods_mobile/services/global_services.dart';
import 'package:pinepods_mobile/ui/icons/phosphor_icons.dart';
import 'package:pinepods_mobile/ui/pinepods/podcast_nav.dart';
import 'package:pinepods_mobile/ui/podcast/person_avatar.dart';
import 'package:pinepods_mobile/ui/podcast/pinepods_up_next_view.dart';
import 'package:pinepods_mobile/ui/podcast/playback_error_listener.dart';
import 'package:pinepods_mobile/ui/podcast/remote_devices_button.dart';
import 'package:pinepods_mobile/ui/podcast/skip_interval_icon.dart';
import 'package:pinepods_mobile/ui/tab_navigator.dart';
import 'package:pinepods_mobile/ui/utils/artwork_colors.dart';
import 'package:pinepods_mobile/ui/widgets/placeholder_builder.dart';
import 'package:pinepods_mobile/ui/widgets/podcast_html.dart';
import 'package:pinepods_mobile/ui/widgets/podcast_image.dart';
import 'package:pinepods_mobile/ui/widgets/sleep_selector.dart';
import 'package:pinepods_mobile/ui/widgets/soft_ui.dart';
import 'package:pinepods_mobile/ui/widgets/speed_selector.dart';
import 'package:provider/provider.dart';

/// Opens the full-screen player above the app shell. Swipe down to close.
Future<void> showNowPlaying(BuildContext context) {
  // A modal sheet's content gets no top safe-area inset, so take the status
  // bar height from the screen opening it (or the window as a fallback).
  final view = View.of(context);
  final topInset = math.max(MediaQuery.paddingOf(context).top, view.padding.top / view.devicePixelRatio);

  return showModalBottomSheet<void>(
    context: context,
    useRootNavigator: true,
    isScrollControlled: true,
    backgroundColor: Colors.transparent,
    routeSettings: const RouteSettings(name: 'nowplaying'),
    builder: (_) => NowPlaying(topInset: topInset),
  );
}

class NowPlaying extends StatefulWidget {
  /// Height of the status bar area the player draws behind.
  final double topInset;

  const NowPlaying({super.key, this.topInset = 0});

  @override
  State<NowPlaying> createState() => _NowPlayingState();
}

class _NowPlayingState extends State<NowPlaying> {
  StreamSubscription<AudioState>? _stoppedSubscription;
  String? _artUrl;
  Color? _artColor;

  @override
  void initState() {
    super.initState();
    final audioBloc = Provider.of<AudioBloc>(context, listen: false);

    // Close once nothing is playing any more.
    _stoppedSubscription = audioBloc.playingState!.where((s) => s == AudioState.stopped).take(1).listen((_) {
      if (mounted) Navigator.of(context).maybePop();
    });
  }

  @override
  void dispose() {
    _stoppedSubscription?.cancel();
    super.dispose();
  }

  /// Artwork-derived accent when that setting is on, else the theme's.
  Color _accent(ThemeData theme, Episode episode) {
    final enabled = Provider.of<SettingsBloc>(context, listen: false).currentSettings.artworkAccentColors;
    if (!enabled) return theme.primaryColor;

    final url = episode.imageUrl;
    if (url != null && url.isNotEmpty && url != _artUrl) {
      _artUrl = url;
      final brightness = ThemeData.estimateBrightnessForColor(theme.scaffoldBackgroundColor);
      _artColor = ArtworkColors.cached(url, null, brightness);
      if (_artColor == null) {
        ArtworkColors.resolve(url, null, brightness).then((color) {
          if (mounted && color != null && url == _artUrl) setState(() => _artColor = color);
        });
      }
    }
    return _artColor ?? theme.primaryColor;
  }

  @override
  Widget build(BuildContext context) {
    final audioBloc = Provider.of<AudioBloc>(context, listen: false);
    final theme = Theme.of(context);

    return Semantics(
      label: L.of(context)!.semantics_main_player_header,
      explicitChildNodes: true,
      child: SizedBox(
        height: MediaQuery.sizeOf(context).height,
        child: StreamBuilder<Episode?>(
          stream: audioBloc.nowPlaying,
          initialData: audioBloc.nowPlaying?.valueOrNull,
          builder: (context, snapshot) {
            final episode = snapshot.data;
            if (episode == null) return const SizedBox.shrink();

            // Eases the background between colours when the episode changes.
            return TweenAnimationBuilder<Color?>(
              tween: ColorTween(end: _accent(theme, episode)),
              duration: const Duration(milliseconds: 350),
              builder: (context, accent, _) =>
                  _PlayerSurface(episode: episode, accent: accent!, topInset: widget.topInset),
            );
          },
        ),
      ),
    );
  }
}

class _PlayerSurface extends StatelessWidget {
  final Episode episode;
  final Color accent;
  final double topInset;

  const _PlayerSurface({required this.episode, required this.accent, required this.topInset});

  @override
  Widget build(BuildContext context) {
    final theme = Theme.of(context);
    final bg = theme.scaffoldBackgroundColor;
    final tint = Color.alphaBlend(accent.withOpacity(0.22), bg);

    return PlaybackErrorListener(
      child: ClipRRect(
        borderRadius: const BorderRadius.vertical(top: Radius.circular(20)),
        child: DecoratedBox(
          decoration: BoxDecoration(
            gradient: LinearGradient(
              begin: Alignment.topCenter,
              end: Alignment.bottomCenter,
              colors: [tint, bg],
              stops: const [0.0, 0.6],
            ),
          ),
          child: SafeArea(
            top: false,
            minimum: EdgeInsets.only(top: topInset),
            child: LayoutBuilder(builder: (context, constraints) {
              final wide = constraints.maxWidth > constraints.maxHeight && constraints.maxWidth >= 600;
              final controls = <Widget>[
                _TitleBlock(episode: episode, accent: accent),
                const SizedBox(height: 16),
                _ScrubBar(episode: episode, accent: accent),
                const SizedBox(height: 4),
                _Transport(episode: episode, accent: accent),
                const SizedBox(height: 16),
                _ActionRow(episode: episode, accent: accent),
              ];

              if (wide) {
                return Column(
                  children: [
                    _Header(episode: episode),
                    Expanded(
                      child: Row(
                        children: [
                          Expanded(
                            child: Padding(
                              padding: const EdgeInsets.fromLTRB(32, 8, 16, 24),
                              child: _Artwork(episode: episode),
                            ),
                          ),
                          Expanded(
                            child: Padding(
                              padding: const EdgeInsets.fromLTRB(16, 0, 32, 16),
                              child: Column(mainAxisAlignment: MainAxisAlignment.center, children: controls),
                            ),
                          ),
                        ],
                      ),
                    ),
                  ],
                );
              }

              return Column(
                children: [
                  _Header(episode: episode),
                  Expanded(
                    child: Padding(
                      padding: const EdgeInsets.fromLTRB(32, 8, 32, 24),
                      child: _Artwork(episode: episode),
                    ),
                  ),
                  Padding(
                    padding: const EdgeInsets.fromLTRB(24, 0, 24, 12),
                    child: Column(children: controls),
                  ),
                ],
              );
            }),
          ),
        ),
      ),
    );
  }
}

/// Close the player and open the playing episode's podcast in the app.
void _goToPodcast(BuildContext context, Episode episode) {
  final podcastId = GlobalServices.pinepodsAudioService?.currentPodcastId;
  Navigator.of(context).maybePop();
  final appContext = tabNavigatorKey.currentContext;
  if (appContext == null) return;
  navigateToPodcastById(
    appContext,
    podcastId,
    fallbackTitle: episode.podcast,
    fallbackArtwork: episode.imageUrl,
  );
}

class _Header extends StatelessWidget {
  final Episode episode;

  const _Header({required this.episode});

  @override
  Widget build(BuildContext context) {
    final theme = Theme.of(context);
    final neutral = softNeutral(theme);

    return Padding(
      padding: const EdgeInsets.fromLTRB(8, 4, 8, 0),
      child: Row(
        children: [
          IconButton(
            tooltip: L.of(context)!.minimise_player_window_button_label,
            icon: Icon(PhosphorIconsRegular.caretDown, color: neutral, size: 26),
            onPressed: () => Navigator.of(context).maybePop(),
          ),
          Expanded(
            child: InkWell(
              borderRadius: BorderRadius.circular(8),
              onTap: () => _goToPodcast(context, episode),
              child: Padding(
                padding: const EdgeInsets.symmetric(vertical: 4),
                child: Column(
                  mainAxisSize: MainAxisSize.min,
                  children: [
                    Text(
                      'NOW PLAYING',
                      style: theme.textTheme.labelSmall!.copyWith(color: theme.hintColor, letterSpacing: 1.2),
                    ),
                    const SizedBox(height: 2),
                    Text(
                      episode.podcast ?? '',
                      maxLines: 1,
                      overflow: TextOverflow.ellipsis,
                      textAlign: TextAlign.center,
                      style: theme.textTheme.labelLarge!.copyWith(fontWeight: FontWeight.w600, color: neutral),
                    ),
                  ],
                ),
              ),
            ),
          ),
          const RemoteDevicesButton(),
          PopupMenuButton<String>(
            tooltip: 'More',
            icon: Icon(PhosphorIconsRegular.dotsThree, color: neutral, size: 26),
            onSelected: (value) {
              switch (value) {
                case 'notes':
                  _showNotesSheet(context, episode);
                case 'podcast':
                  _goToPodcast(context, episode);
              }
            },
            itemBuilder: (context) => [
              PopupMenuItem(value: 'notes', child: Text(L.of(context)!.notes_label)),
              const PopupMenuItem(value: 'podcast', child: Text('Go to podcast')),
            ],
          ),
        ],
      ),
    );
  }
}

class _Artwork extends StatelessWidget {
  final Episode episode;

  const _Artwork({required this.episode});

  @override
  Widget build(BuildContext context) {
    final placeholderBuilder = PlaceholderBuilder.of(context);
    const fallback = Image(image: AssetImage('assets/images/favicon.png'));
    // Chapter artwork replaces the episode's while that chapter plays.
    final url = episode.positionalImageUrl;

    return LayoutBuilder(builder: (context, constraints) {
      final size = constraints.biggest.shortestSide;
      return Center(
        child: Semantics(
          label: L.of(context)!.semantic_podcast_artwork_label,
          child: DecoratedBox(
            decoration: BoxDecoration(
              borderRadius: BorderRadius.circular(20),
              boxShadow: [
                BoxShadow(color: Colors.black.withOpacity(0.25), blurRadius: 28, offset: const Offset(0, 10)),
              ],
            ),
            child: AnimatedSwitcher(
              duration: const Duration(milliseconds: 300),
              child: url == null || url.isEmpty
                  ? SizedBox(
                      key: const ValueKey('no-art'),
                      width: size,
                      height: size,
                      child: ClipRRect(borderRadius: BorderRadius.circular(20), child: fallback),
                    )
                  : PodcastImage(
                      key: ValueKey('nowplaying$url'),
                      url: url,
                      width: size,
                      height: size,
                      fit: BoxFit.cover,
                      borderRadius: 20,
                      placeholder: placeholderBuilder?.builder()(context) ?? fallback,
                      errorPlaceholder: placeholderBuilder?.errorBuilder()(context) ?? fallback,
                    ),
            ),
          ),
        ),
      );
    });
  }
}

class _TitleBlock extends StatelessWidget {
  final Episode episode;
  final Color accent;

  const _TitleBlock({required this.episode, required this.accent});

  @override
  Widget build(BuildContext context) {
    final theme = Theme.of(context);
    final chapter = episode.currentChapter;

    return Column(
      children: [
        Semantics(
          container: true,
          child: Text(
            episode.title ?? '',
            textAlign: TextAlign.center,
            maxLines: 2,
            overflow: TextOverflow.ellipsis,
            style: theme.textTheme.titleLarge!.copyWith(fontWeight: FontWeight.bold, height: 1.25),
          ),
        ),
        if (chapter != null && chapter.title.trim().isNotEmpty) ...[
          const SizedBox(height: 6),
          InkWell(
            borderRadius: BorderRadius.circular(8),
            onTap: () => _showChaptersSheet(context),
            child: Padding(
              padding: const EdgeInsets.symmetric(horizontal: 6, vertical: 2),
              child: Text(
                chapter.title.trim(),
                semanticsLabel: '${L.of(context)!.semantic_current_chapter_label} ${chapter.title}',
                maxLines: 1,
                overflow: TextOverflow.ellipsis,
                style: theme.textTheme.bodyMedium!.copyWith(color: accent, fontWeight: FontWeight.w600),
              ),
            ),
          ),
        ],
      ],
    );
  }
}

String _formatTime(Duration d) {
  final h = d.inHours;
  final m = d.inMinutes.remainder(60);
  final s = d.inSeconds.remainder(60).toString().padLeft(2, '0');
  return h > 0 ? '$h:${m.toString().padLeft(2, '0')}:$s' : '$m:$s';
}

/// Chapters shown in the table of contents, in order.
List<Chapter> _tocChapters(Episode episode) => episode.chapters.where((c) => c.toc).toList();

/// Scrub bar with the buffered range and chapter boundaries. Drag or tap to seek.
class _ScrubBar extends StatefulWidget {
  final Episode episode;
  final Color accent;

  const _ScrubBar({required this.episode, required this.accent});

  @override
  State<_ScrubBar> createState() => _ScrubBarState();
}

class _ScrubBarState extends State<_ScrubBar> {
  /// Where the finger is while dragging, as a fraction of the episode.
  double? _dragFraction;

  void _seekTo(AudioBloc audioBloc, double fraction, Duration length) {
    audioBloc.transitionPosition(length.inMilliseconds * fraction / 1000);
  }

  @override
  Widget build(BuildContext context) {
    final audioBloc = Provider.of<AudioBloc>(context, listen: false);
    final theme = Theme.of(context);
    final neutral = softNeutral(theme);
    final labels = theme.textTheme.bodySmall!.copyWith(
      color: theme.hintColor,
      fontFeatures: const [FontFeature.tabularFigures()],
    );

    return StreamBuilder<PositionState>(
      stream: audioBloc.playPosition,
      initialData: audioBloc.playPosition?.valueOrNull,
      builder: (context, snapshot) {
        final data = snapshot.data;
        final length = data?.length ?? Duration.zero;
        final lengthMs = length.inMilliseconds;
        final canSeek = lengthMs > 0;
        double fractionOf(Duration d) => canSeek ? (d.inMilliseconds / lengthMs).clamp(0.0, 1.0) : 0.0;

        final fraction = _dragFraction ?? fractionOf(data?.position ?? Duration.zero);
        final shown = Duration(milliseconds: (lengthMs * fraction).round());
        final remaining = length - shown;
        final chapterMarks = canSeek
            ? _tocChapters(widget.episode)
                .map((c) => c.startTime * 1000 / lengthMs)
                .where((f) => f > 0 && f < 1)
                .toList()
            : const <double>[];

        return Semantics(
          slider: true,
          label: L.of(context)!.now_playing_episode_position,
          value: _formatTime(shown),
          onIncrease: canSeek ? () => audioBloc.transitionState(TransitionState.fastforward) : null,
          onDecrease: canSeek ? () => audioBloc.transitionState(TransitionState.rewind) : null,
          child: Column(
            children: [
              LayoutBuilder(builder: (context, constraints) {
                final width = constraints.maxWidth;
                double at(Offset p) => (p.dx / width).clamp(0.0, 1.0);

                return GestureDetector(
                  behavior: HitTestBehavior.opaque,
                  onHorizontalDragStart: canSeek
                      ? (d) {
                          HapticFeedback.selectionClick();
                          setState(() => _dragFraction = at(d.localPosition));
                        }
                      : null,
                  onHorizontalDragUpdate: canSeek ? (d) => setState(() => _dragFraction = at(d.localPosition)) : null,
                  onHorizontalDragEnd: canSeek
                      ? (_) {
                          _seekTo(audioBloc, _dragFraction ?? fraction, length);
                          setState(() => _dragFraction = null);
                        }
                      : null,
                  onHorizontalDragCancel: () => setState(() => _dragFraction = null),
                  onTapUp: canSeek ? (d) => _seekTo(audioBloc, at(d.localPosition), length) : null,
                  child: SizedBox(
                    height: 32,
                    width: width,
                    child: CustomPaint(
                      painter: _ScrubPainter(
                        played: fraction,
                        buffered: fractionOf(data?.buffered ?? Duration.zero),
                        chapterMarks: chapterMarks,
                        dragging: _dragFraction != null,
                        accent: widget.accent,
                        track: neutral.withOpacity(0.12),
                        bufferedColor: neutral.withOpacity(0.24),
                        gap: theme.scaffoldBackgroundColor,
                      ),
                    ),
                  ),
                );
              }),
              Row(
                children: [
                  Text(_formatTime(shown), style: labels),
                  const Spacer(),
                  Text(
                    '-${_formatTime(remaining.isNegative ? Duration.zero : remaining)}',
                    semanticsLabel: '${L.of(context)!.now_playing_episode_time_remaining} ${_formatTime(remaining)}',
                    style: labels,
                  ),
                ],
              ),
            ],
          ),
        );
      },
    );
  }
}

class _ScrubPainter extends CustomPainter {
  final double played;
  final double buffered;
  final List<double> chapterMarks;
  final bool dragging;
  final Color accent;
  final Color track;
  final Color bufferedColor;
  final Color gap;

  _ScrubPainter({
    required this.played,
    required this.buffered,
    required this.chapterMarks,
    required this.dragging,
    required this.accent,
    required this.track,
    required this.bufferedColor,
    required this.gap,
  });

  @override
  void paint(Canvas canvas, Size size) {
    final height = dragging ? 7.0 : 4.0;
    final top = (size.height - height) / 2;
    final radius = Radius.circular(height / 2);
    RRect bar(double from, double to) =>
        RRect.fromLTRBR(size.width * from, top, size.width * to, top + height, radius);

    canvas.drawRRect(bar(0, 1), Paint()..color = track);
    if (buffered > played) canvas.drawRRect(bar(0, buffered), Paint()..color = bufferedColor);
    canvas.drawRRect(bar(0, played), Paint()..color = accent);

    // Chapter boundaries as small gaps in the bar.
    final gapPaint = Paint()..color = gap;
    for (final mark in chapterMarks) {
      final x = size.width * mark;
      canvas.drawRect(Rect.fromLTWH(x - 1, top, 2, height), gapPaint);
    }

    canvas.drawCircle(
      Offset(size.width * played, size.height / 2),
      dragging ? 9 : 6,
      Paint()..color = accent,
    );
  }

  @override
  bool shouldRepaint(_ScrubPainter old) =>
      old.played != played ||
      old.buffered != buffered ||
      old.dragging != dragging ||
      old.accent != accent ||
      old.track != track ||
      old.chapterMarks.length != chapterMarks.length;
}

class _Transport extends StatelessWidget {
  final Episode episode;
  final Color accent;

  const _Transport({required this.episode, required this.accent});

  /// Seek to the previous chapter, or back to the start of this one if it has
  /// been playing for more than a few seconds.
  void _previousChapter(AudioBloc audioBloc, List<Chapter> chapters) {
    final position = audioBloc.playPosition?.valueOrNull?.position.inMilliseconds ?? 0;
    final seconds = position / 1000;
    final current = chapters.lastIndexWhere((c) => c.startTime <= seconds);
    if (current < 0) return;
    final target = seconds - chapters[current].startTime > 3 || current == 0 ? current : current - 1;
    audioBloc.transitionPosition(chapters[target].startTime);
  }

  void _nextChapter(AudioBloc audioBloc, List<Chapter> chapters) {
    final position = audioBloc.playPosition?.valueOrNull?.position.inMilliseconds ?? 0;
    final next = chapters.indexWhere((c) => c.startTime > position / 1000);
    if (next >= 0) audioBloc.transitionPosition(chapters[next].startTime);
  }

  @override
  Widget build(BuildContext context) {
    final audioBloc = Provider.of<AudioBloc>(context, listen: false);
    final settings = Provider.of<SettingsBloc>(context, listen: false).currentSettings;
    final neutral = softNeutral(Theme.of(context));
    final chapters = _tocChapters(episode);

    return Row(
      mainAxisAlignment: MainAxisAlignment.spaceEvenly,
      children: [
        if (chapters.length > 1)
          IconButton(
            tooltip: 'Previous chapter',
            iconSize: 28,
            color: neutral,
            icon: const Icon(PhosphorIconsFill.skipBack),
            onPressed: () => _previousChapter(audioBloc, chapters),
          ),
        _SkipButton(
          icon: PhosphorIconsRegular.arrowCounterClockwise,
          seconds: settings.rewindInterval,
          tooltip: L.of(context)!.rewind_button_label,
          onTap: () => audioBloc.transitionState(TransitionState.rewind),
        ),
        _PlayButton(accent: accent),
        _SkipButton(
          icon: PhosphorIconsRegular.arrowClockwise,
          seconds: settings.fastForwardInterval,
          tooltip: L.of(context)!.fast_forward_button_label,
          onTap: () => audioBloc.transitionState(TransitionState.fastforward),
        ),
        if (chapters.length > 1)
          IconButton(
            tooltip: 'Next chapter',
            iconSize: 28,
            color: neutral,
            icon: const Icon(PhosphorIconsFill.skipForward),
            onPressed: () => _nextChapter(audioBloc, chapters),
          ),
      ],
    );
  }
}

/// Rewind / fast-forward with the interval written inside the arrow.
class _SkipButton extends StatelessWidget {
  final IconData icon;
  final int seconds;
  final String tooltip;
  final VoidCallback onTap;

  const _SkipButton({required this.icon, required this.seconds, required this.tooltip, required this.onTap});

  @override
  Widget build(BuildContext context) {
    final neutral = softNeutral(Theme.of(context));
    return Tooltip(
      message: tooltip,
      child: InkResponse(
        radius: 34,
        onTap: () {
          HapticFeedback.lightImpact();
          onTap();
        },
        child: SizedBox(
          width: 64,
          height: 64,
          child: Center(
            child: SkipIntervalIcon(icon: icon, seconds: seconds, color: neutral, semanticLabel: tooltip),
          ),
        ),
      ),
    );
  }
}

class _PlayButton extends StatelessWidget {
  final Color accent;

  const _PlayButton({required this.accent});

  @override
  Widget build(BuildContext context) {
    final audioBloc = Provider.of<AudioBloc>(context, listen: false);
    final fg = onColor(accent);

    return StreamBuilder<AudioState>(
      stream: audioBloc.playingState,
      initialData: audioBloc.playingState?.valueOrNull,
      builder: (context, snapshot) {
        final state = snapshot.data;
        final playing = state == AudioState.playing;
        final loading = state == AudioState.buffering || state == AudioState.starting;
        final failed = state == AudioState.error;
        final label = playing ? L.of(context)!.pause_button_label : L.of(context)!.play_button_label;

        return SizedBox(
          width: 88,
          height: 88,
          child: Stack(
            alignment: Alignment.center,
            children: [
              if (loading)
                SizedBox(
                  width: 86,
                  height: 86,
                  child: CircularProgressIndicator(strokeWidth: 3, color: accent),
                ),
              Material(
                color: accent,
                shape: const CircleBorder(),
                elevation: 2,
                child: InkWell(
                  customBorder: const CircleBorder(),
                  onTap: () {
                    HapticFeedback.mediumImpact();
                    audioBloc.transitionState(playing ? TransitionState.pause : TransitionState.play);
                  },
                  child: SizedBox(
                    width: 74,
                    height: 74,
                    child: Icon(
                      failed
                          ? PhosphorIconsBold.arrowClockwise
                          : playing
                              ? PhosphorIconsFill.pause
                              : PhosphorIconsFill.play,
                      size: 34,
                      color: fg,
                      semanticLabel: failed ? 'Retry' : label,
                    ),
                  ),
                ),
              ),
            ],
          ),
        );
      },
    );
  }
}

class _ActionRow extends StatelessWidget {
  final Episode episode;
  final Color accent;

  const _ActionRow({required this.episode, required this.accent});

  @override
  Widget build(BuildContext context) {
    final audioBloc = Provider.of<AudioBloc>(context, listen: false);
    final l = L.of(context)!;

    return Row(
      mainAxisAlignment: MainAxisAlignment.spaceEvenly,
      children: [
        StreamBuilder<double>(
          stream: audioBloc.currentSpeed,
          initialData: audioBloc.currentSpeed?.valueOrNull ?? 1.0,
          builder: (context, snapshot) {
            final speed = snapshot.data ?? 1.0;
            final text = speed == speed.roundToDouble() ? '${speed.toInt()}×' : '${speed.toStringAsFixed(1)}×';
            return _Action(
              label: l.playback_speed_label,
              semanticLabel: '${l.playback_speed_label} $text',
              active: speed != 1.0,
              accent: accent,
              icon: Text(text, style: const TextStyle(fontSize: 16, fontWeight: FontWeight.w700)),
              onTap: () => _showSheet(context, const SpeedSlider()),
            );
          },
        ),
        StreamBuilder<Sleep>(
          stream: audioBloc.sleepStream,
          initialData: Sleep(type: SleepType.none),
          builder: (context, snapshot) {
            final active = (snapshot.data?.type ?? SleepType.none) != SleepType.none;
            return _Action(
              label: l.sleep_timer_label,
              active: active,
              accent: accent,
              icon: Icon(active ? PhosphorIconsFill.moon : PhosphorIconsRegular.moon, size: 22),
              onTap: () => _showSheet(context, const SleepSlider(), scrollControlled: true),
            );
          },
        ),
        if (_tocChapters(episode).isNotEmpty)
          _Action(
            label: l.chapters_label,
            accent: accent,
            icon: const Icon(PhosphorIconsRegular.listNumbers, size: 22),
            onTap: () => _showChaptersSheet(context),
          ),
        _Action(
          label: l.up_next_queue_label,
          accent: accent,
          icon: const Icon(PhosphorIconsRegular.queue, size: 22),
          onTap: () => _showUpNextSheet(context),
        ),
      ],
    );
  }
}

class _Action extends StatelessWidget {
  final String label;
  final String? semanticLabel;
  final Widget icon;
  final VoidCallback onTap;
  final bool active;
  final Color accent;

  const _Action({
    required this.label,
    required this.icon,
    required this.onTap,
    required this.accent,
    this.semanticLabel,
    this.active = false,
  });

  @override
  Widget build(BuildContext context) {
    final theme = Theme.of(context);
    final color = active ? accent : softNeutral(theme).withOpacity(0.85);

    return Semantics(
      button: true,
      label: semanticLabel ?? label,
      excludeSemantics: true,
      child: InkWell(
        borderRadius: BorderRadius.circular(14),
        onTap: onTap,
        child: Padding(
          padding: const EdgeInsets.symmetric(horizontal: 8, vertical: 6),
          child: IconTheme.merge(
            data: IconThemeData(color: color),
            child: DefaultTextStyle.merge(
              style: TextStyle(color: color),
              child: Column(
                mainAxisSize: MainAxisSize.min,
                children: [
                  SizedBox(height: 26, child: Center(child: icon)),
                  const SizedBox(height: 4),
                  Text(label, style: theme.textTheme.labelSmall!.copyWith(color: color, letterSpacing: 0.2)),
                ],
              ),
            ),
          ),
        ),
      ),
    );
  }
}

// Sheets opened from the player.

Future<void> _showSheet(BuildContext context, Widget child, {bool scrollControlled = false}) {
  return showModalBottomSheet<void>(
    context: context,
    useRootNavigator: true,
    isScrollControlled: scrollControlled,
    backgroundColor: Theme.of(context).secondaryHeaderColor,
    builder: (_) => child,
  );
}

/// A sheet the user can drag between a partial and nearly full height, with
/// a handle and title; [builder] gets the sheet's scroll controller.
Future<void> _showDraggableSheet(
  BuildContext context, {
  required String title,
  required Widget Function(BuildContext context, ScrollController controller) builder,
}) {
  return showModalBottomSheet<void>(
    context: context,
    useRootNavigator: true,
    isScrollControlled: true,
    backgroundColor: Theme.of(context).secondaryHeaderColor,
    builder: (context) => DraggableScrollableSheet(
      expand: false,
      initialChildSize: 0.6,
      minChildSize: 0.35,
      maxChildSize: 0.92,
      builder: (context, controller) => Column(
        children: [
          const _SheetHandle(),
          Padding(
            padding: const EdgeInsets.fromLTRB(20, 4, 20, 8),
            child: Align(
              alignment: Alignment.centerLeft,
              child: Text(title, style: Theme.of(context).textTheme.titleLarge!.copyWith(fontWeight: FontWeight.bold)),
            ),
          ),
          Expanded(child: builder(context, controller)),
        ],
      ),
    ),
  );
}

class _SheetHandle extends StatelessWidget {
  const _SheetHandle();

  @override
  Widget build(BuildContext context) {
    return Padding(
      padding: const EdgeInsets.symmetric(vertical: 10),
      child: Container(
        width: 36,
        height: 4,
        decoration: BoxDecoration(
          color: softNeutral(Theme.of(context)).withOpacity(0.25),
          borderRadius: BorderRadius.circular(2),
        ),
      ),
    );
  }
}

void _showChaptersSheet(BuildContext context) {
  _showDraggableSheet(
    context,
    title: L.of(context)!.chapters_label,
    builder: (context, controller) => _ChapterList(controller: controller),
  );
}

void _showNotesSheet(BuildContext context, Episode episode) {
  _showDraggableSheet(
    context,
    title: L.of(context)!.notes_label,
    builder: (context, controller) => SingleChildScrollView(
      controller: controller,
      padding: const EdgeInsets.fromLTRB(12, 0, 12, 24),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          if (episode.persons.isNotEmpty)
            SizedBox(
              height: 120,
              child: ListView.builder(
                scrollDirection: Axis.horizontal,
                itemCount: episode.persons.length,
                itemBuilder: (context, index) => PersonAvatar(person: episode.persons[index]),
              ),
            ),
          PodcastHtml(content: episode.content ?? episode.description ?? ''),
        ],
      ),
    ),
  );
}

void _showUpNextSheet(BuildContext context) {
  showModalBottomSheet<void>(
    context: context,
    useRootNavigator: true,
    isScrollControlled: true,
    backgroundColor: Theme.of(context).secondaryHeaderColor,
    builder: (context) => SizedBox(
      height: MediaQuery.sizeOf(context).height * 0.7,
      child: const Column(
        children: [
          _SheetHandle(),
          Expanded(child: PinepodsUpNextView()),
        ],
      ),
    ),
  );
}

class _ChapterList extends StatefulWidget {
  final ScrollController controller;

  const _ChapterList({required this.controller});

  @override
  State<_ChapterList> createState() => _ChapterListState();
}

class _ChapterListState extends State<_ChapterList> {
  static const _rowHeight = 56.0;
  bool _scrolledToCurrent = false;

  @override
  Widget build(BuildContext context) {
    final audioBloc = Provider.of<AudioBloc>(context, listen: false);
    final theme = Theme.of(context);

    return StreamBuilder<Episode?>(
      stream: audioBloc.nowPlaying,
      initialData: audioBloc.nowPlaying?.valueOrNull,
      builder: (context, snapshot) {
        final episode = snapshot.data;
        final chapters = episode == null ? const <Chapter>[] : _tocChapters(episode);
        final current = chapters.indexWhere((c) => c == episode?.currentChapter);

        // Open with the current chapter in view.
        if (!_scrolledToCurrent && current > 1) {
          _scrolledToCurrent = true;
          WidgetsBinding.instance.addPostFrameCallback((_) {
            if (widget.controller.hasClients) widget.controller.jumpTo((current - 1) * _rowHeight);
          });
        }

        return ListView.builder(
          controller: widget.controller,
          itemExtent: _rowHeight,
          itemCount: chapters.length,
          itemBuilder: (context, index) {
            final chapter = chapters[index];
            final selected = index == current;
            final color = selected ? theme.primaryColor : softNeutral(theme);
            return InkWell(
              onTap: () => audioBloc.transitionPosition(chapter.startTime),
              child: Padding(
                padding: const EdgeInsets.symmetric(horizontal: 20),
                child: Row(
                  children: [
                    SizedBox(
                      width: 32,
                      child: Text('${index + 1}', style: theme.textTheme.bodyMedium!.copyWith(color: theme.hintColor)),
                    ),
                    Expanded(
                      child: Text(
                        chapter.title.trim(),
                        maxLines: 2,
                        overflow: TextOverflow.ellipsis,
                        style: theme.textTheme.bodyLarge!.copyWith(
                          color: color,
                          fontWeight: selected ? FontWeight.w700 : FontWeight.normal,
                        ),
                      ),
                    ),
                    const SizedBox(width: 12),
                    Text(
                      _formatTime(Duration(milliseconds: (chapter.startTime * 1000).round())),
                      style: theme.textTheme.bodySmall!.copyWith(
                        color: theme.hintColor,
                        fontFeatures: const [FontFeature.tabularFigures()],
                      ),
                    ),
                  ],
                ),
              ),
            );
          },
        );
      },
    );
  }
}
