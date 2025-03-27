// Copyright 2020 Ben Hills and the project contributors. All rights reserved.
// Use of this source code is governed by a BSD-style license that can be
// found in the LICENSE file.

import 'dart:async';

import 'package:pinepods_mobile/bloc/podcast/audio_bloc.dart';
import 'package:pinepods_mobile/entities/episode.dart';
import 'package:pinepods_mobile/l10n/L.dart';
import 'package:pinepods_mobile/services/audio/audio_player_service.dart';
import 'package:pinepods_mobile/ui/podcast/chapter_selector.dart';
import 'package:pinepods_mobile/ui/podcast/dot_decoration.dart';
import 'package:pinepods_mobile/ui/podcast/now_playing_floating_player.dart';
import 'package:pinepods_mobile/ui/podcast/now_playing_options.dart';
import 'package:pinepods_mobile/ui/podcast/person_avatar.dart';
import 'package:pinepods_mobile/ui/podcast/playback_error_listener.dart';
import 'package:pinepods_mobile/ui/podcast/player_position_controls.dart';
import 'package:pinepods_mobile/ui/podcast/player_transport_controls.dart';
import 'package:pinepods_mobile/ui/widgets/delayed_progress_indicator.dart';
import 'package:pinepods_mobile/ui/widgets/placeholder_builder.dart';
import 'package:pinepods_mobile/ui/widgets/podcast_html.dart';
import 'package:pinepods_mobile/ui/widgets/podcast_image.dart';
import 'package:auto_size_text/auto_size_text.dart';
import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:provider/provider.dart';
import 'package:url_launcher/url_launcher.dart';

/// This is the full-screen player Widget which is invoked by touching the mini player.
///
/// This is the parent widget of the now playing screen(s). If we are running on a mobile in
/// portrait mode, we display the episode details, controls and additional options
/// as a draggable view. For tablets in portrait or on desktop, we display a split
/// screen. The main details and controls appear in one pane with the additional
/// controls in another.
///
/// TODO: The fade in/out transition applied when scrolling the queue is the first implementation.
/// Using [Opacity] is a very inefficient way of achieving this effect, but will do as a place
/// holder until a better animation can be achieved.
class NowPlaying extends StatefulWidget {
  const NowPlaying({
    super.key,
  });

  @override
  State<NowPlaying> createState() => _NowPlayingState();
}

class _NowPlayingState extends State<NowPlaying> with WidgetsBindingObserver {
  late StreamSubscription<AudioState> playingStateSubscription;
  var textGroup = AutoSizeGroup();
  double scrollPos = 0.0;
  double opacity = 0.0;

  @override
  void initState() {
    super.initState();
    WidgetsBinding.instance.addObserver(this);

    final audioBloc = Provider.of<AudioBloc>(context, listen: false);
    var popped = false;

    // If the episode finishes we can close.
    playingStateSubscription =
        audioBloc.playingState!.where((state) => state == AudioState.stopped).listen((playingState) async {
      // Prevent responding to multiple stop events after we've popped and lost context.
      if (!popped) {
        popped = true;
        if (mounted) {
          Navigator.of(context).pop();
        }
      }
    });
  }

  @override
  void dispose() {
    WidgetsBinding.instance.removeObserver(this);
    playingStateSubscription.cancel();

    super.dispose();
  }

  bool isMobilePortrait(BuildContext context) {
    final query = MediaQuery.of(context);
    return (query.orientation == Orientation.portrait || query.size.width <= 1000);
  }

  @override
  Widget build(BuildContext context) {
    final audioBloc = Provider.of<AudioBloc>(context, listen: false);
    final playerBuilder = PlayerControlsBuilder.of(context);

    return Semantics(
      header: false,
      label: L.of(context)!.semantics_main_player_header,
      explicitChildNodes: true,
      child: StreamBuilder<Episode?>(
          stream: audioBloc.nowPlaying,
          builder: (context, snapshot) {
            if (!snapshot.hasData) {
              return Container();
            }

            var duration = snapshot.data == null ? 0 : snapshot.data!.duration;
            final WidgetBuilder? transportBuilder = playerBuilder?.builder(duration);

            return isMobilePortrait(context)
                ? NotificationListener<DraggableScrollableNotification>(
                    onNotification: (notification) {
                      setState(() {
                        if (notification.extent > (notification.minExtent)) {
                          opacity = 1 - (notification.maxExtent - notification.extent);
                          scrollPos = 1.0;
                        } else {
                          opacity = 0.0;
                          scrollPos = 0.0;
                        }
                      });

                      return true;
                    },
                    child: Stack(
                      fit: StackFit.expand,
                      children: [
                        // We need to hide the main player when the floating player is visible to prevent
                        // screen readers from reading both parts of the stack.
                        Visibility(
                          visible: opacity < 1,
                          child: NowPlayingTabs(
                            episode: snapshot.data!,
                            transportBuilder: transportBuilder,
                          ),
                        ),
                        SizedBox.expand(
                            child: SafeArea(
                          child: Column(
                            children: [
                              /// Sized boxes without a child are 'invisible' so they do not prevent taps below
                              /// the stack but are still present in the layout. We have a sized box here to stop
                              /// the draggable panel from jumping as you start to pull it up. I am really looking
                              /// forward to the Dart team fixing the nested scroll issues with [DraggableScrollableSheet]
                              SizedBox(
                                height: 64.0,
                                child: scrollPos == 1
                                    ? Opacity(
                                        opacity: opacity,
                                        child: const FloatingPlayer(),
                                      )
                                    : null,
                              ),
                              if (MediaQuery.of(context).orientation == Orientation.portrait)
                                const Expanded(
                                  child: NowPlayingOptionsSelector(),
                                ),
                            ],
                          ),
                        )),
                      ],
                    ),
                  )
                : Row(
                    mainAxisSize: MainAxisSize.max,
                    children: [
                      Expanded(
                        flex: 1,
                        child: NowPlayingTabs(episode: snapshot.data!, transportBuilder: transportBuilder),
                      ),
                      const Expanded(
                        flex: 1,
                        child: NowPlayingOptionsSelectorWide(),
                      ),
                    ],
                  );
          }),
    );
  }
}

/// This widget displays the episode logo, episode title and current
/// chapter if available.
///
/// If running in portrait this will be in a vertical format; if in
/// landscape this will be in a horizontal format. The actual displaying
/// of the episode text is handed off to [NowPlayingEpisodeDetails].
class NowPlayingEpisode extends StatelessWidget {
  final String? imageUrl;
  final Episode episode;
  final AutoSizeGroup? textGroup;

  const NowPlayingEpisode({
    super.key,
    required this.imageUrl,
    required this.episode,
    required this.textGroup,
  });

  @override
  Widget build(BuildContext context) {
    final placeholderBuilder = PlaceholderBuilder.of(context);

    return OrientationBuilder(
      builder: (context, orientation) {
        return Padding(
          padding: const EdgeInsets.all(8.0),
          child: MediaQuery.of(context).orientation == Orientation.portrait || MediaQuery.of(context).size.width >= 1000
              ? Column(
                  mainAxisAlignment: MainAxisAlignment.start,
                  children: [
                    Expanded(
                      flex: 7,
                      child: Semantics(
                        label: L.of(context)!.semantic_podcast_artwork_label,
                        child: PodcastImage(
                          key: Key('nowplaying$imageUrl'),
                          url: imageUrl!,
                          width: MediaQuery.of(context).size.width * .75,
                          height: MediaQuery.of(context).size.height * .75,
                          fit: BoxFit.contain,
                          borderRadius: 6.0,
                          placeholder: placeholderBuilder != null
                              ? placeholderBuilder.builder()(context)
                              : DelayedCircularProgressIndicator(),
                          errorPlaceholder: placeholderBuilder != null
                              ? placeholderBuilder.errorBuilder()(context)
                              : const Image(image: AssetImage('assets/images/favicon.png')),
                        ),
                      ),
                    ),
                    Expanded(
                      flex: 3,
                      child: NowPlayingEpisodeDetails(
                        episode: episode,
                        textGroup: textGroup,
                      ),
                    ),
                  ],
                )
              : Row(
                  mainAxisAlignment: MainAxisAlignment.spaceBetween,
                  children: [
                    Expanded(
                      flex: 2,
                      child: Padding(
                        padding: const EdgeInsets.only(
                          left: 8.0,
                          bottom: 8.0,
                        ),
                        child: PodcastImage(
                          key: Key('nowplaying$imageUrl'),
                          url: imageUrl!,
                          height: 280,
                          width: 280,
                          fit: BoxFit.contain,
                          borderRadius: 8.0,
                          placeholder: placeholderBuilder != null
                              ? placeholderBuilder.builder()(context)
                              : DelayedCircularProgressIndicator(),
                          errorPlaceholder: placeholderBuilder != null
                              ? placeholderBuilder.errorBuilder()(context)
                              : const Image(image: AssetImage('assets/images/favicon.png')),
                        ),
                      ),
                    ),
                    Expanded(
                      flex: 5,
                      child: NowPlayingEpisodeDetails(
                        episode: episode,
                        textGroup: textGroup,
                      ),
                    ),
                  ],
                ),
        );
      },
    );
  }
}

/// This widget is responsible for displaying the main episode details.
///
/// This displays the current episode title and, if available, the
/// current chapter title and optional link.
class NowPlayingEpisodeDetails extends StatelessWidget {
  final Episode? episode;
  final AutoSizeGroup? textGroup;
  static const minFontSize = 14.0;

  const NowPlayingEpisodeDetails({
    super.key,
    this.episode,
    this.textGroup,
  });

  @override
  Widget build(BuildContext context) {
    final chapterTitle = episode?.currentChapter?.title ?? '';
    final chapterUrl = episode?.currentChapter?.url ?? '';

    return Column(
      children: [
        Expanded(
          flex: 5,
          child: Padding(
            padding: const EdgeInsets.fromLTRB(8.0, 16.0, 8.0, 0.0),
            child: Semantics(
              container: true,
              child: AutoSizeText(
                episode?.title ?? '',
                group: textGroup,
                textAlign: TextAlign.center,
                overflow: TextOverflow.ellipsis,
                minFontSize: minFontSize,
                style: const TextStyle(
                  fontWeight: FontWeight.bold,
                  fontSize: 24.0,
                ),
                maxLines: episode!.hasChapters ? 3 : 4,
              ),
            ),
          ),
        ),
        if (episode!.hasChapters)
          Expanded(
            flex: 4,
            child: Padding(
              padding: const EdgeInsets.fromLTRB(8.0, 0.0, 0.0, 8.0),
              child: Row(
                mainAxisAlignment: MainAxisAlignment.center,
                crossAxisAlignment: CrossAxisAlignment.center,
                children: [
                  Flexible(
                    child: Semantics(
                      label: L.of(context)!.semantic_current_chapter_label,
                      container: true,
                      child: AutoSizeText(
                        chapterTitle,
                        group: textGroup,
                        minFontSize: minFontSize,
                        textAlign: TextAlign.center,
                        overflow: TextOverflow.ellipsis,
                        style: TextStyle(
                          color: Colors.grey[300],
                          fontWeight: FontWeight.normal,
                          fontSize: 16.0,
                        ),
                        maxLines: 2,
                      ),
                    ),
                  ),
                  chapterUrl.isEmpty
                      ? const SizedBox(
                          height: 0,
                          width: 0,
                        )
                      : Semantics(
                          label: L.of(context)!.semantic_chapter_link_label,
                          container: true,
                          child: IconButton(
                              padding: EdgeInsets.zero,
                              icon: const Icon(
                                Icons.link,
                              ),
                              color: Theme.of(context).primaryIconTheme.color,
                              onPressed: () {
                                _chapterLink(chapterUrl);
                              }),
                        ),
                ],
              ),
            ),
          ),
      ],
    );
  }

  void _chapterLink(String url) async {
    final uri = Uri.parse(url);

    if (await canLaunchUrl(uri)) {
      await launchUrl(uri);
    } else {
      throw 'Could not launch chapter link: $url';
    }
  }
}

/// This widget handles the displaying of the episode show notes.
///
/// This consists of title, show notes and person details
/// (where available).
class NowPlayingShowNotes extends StatelessWidget {
  final Episode? episode;

  const NowPlayingShowNotes({
    super.key,
    required this.episode,
  });

  @override
  Widget build(BuildContext context) {
    return SizedBox.expand(
      child: SingleChildScrollView(
        scrollDirection: Axis.vertical,
        child: Column(
          children: [
            Align(
              alignment: Alignment.centerLeft,
              child: Padding(
                padding: const EdgeInsets.only(
                  left: 16.0,
                  right: 16.0,
                  bottom: 16.0,
                ),
                child: Text(
                  episode!.title!,
                  style: Theme.of(context).textTheme.titleLarge!.copyWith(
                        fontWeight: FontWeight.bold,
                      ),
                ),
              ),
            ),
            if (episode!.persons.isNotEmpty)
              SizedBox(
                height: 120.0,
                child: ListView.builder(
                  itemCount: episode!.persons.length,
                  scrollDirection: Axis.horizontal,
                  itemBuilder: (BuildContext context, int index) {
                    return PersonAvatar(person: episode!.persons[index]);
                  },
                ),
              ),
            Padding(
              padding: const EdgeInsets.only(
                top: 8.0,
                left: 8.0,
                right: 8.0,
              ),
              child: PodcastHtml(content: episode?.content ?? episode?.description ?? ''),
            ),
          ],
        ),
      ),
    );
  }
}

/// Widget for rendering main episode tabs.
///
/// This will be episode details and show notes. If the episode supports chapters
/// this will be included also. This is the parent widget. The tabs are
/// rendered via [EpisodeTabBar] and the tab contents via. [EpisodeTabBarView].
class NowPlayingTabs extends StatelessWidget {
  const NowPlayingTabs({
    super.key,
    required this.transportBuilder,
    required this.episode,
  });

  final WidgetBuilder? transportBuilder;
  final Episode episode;

  @override
  Widget build(BuildContext context) {
    return DefaultTabController(
        length: episode.hasChapters ? 3 : 2,
        initialIndex: episode.hasChapters ? 1 : 0,
        child: AnnotatedRegion<SystemUiOverlayStyle>(
          value: Theme.of(context)
              .appBarTheme
              .systemOverlayStyle!
              .copyWith(systemNavigationBarColor: Theme.of(context).secondaryHeaderColor),
          child: Scaffold(
            appBar: AppBar(
              backgroundColor: Theme.of(context).scaffoldBackgroundColor,
              elevation: 0.0,
              leading: IconButton(
                tooltip: L.of(context)!.minimise_player_window_button_label,
                icon: Icon(
                  Icons.keyboard_arrow_down,
                  color: Theme.of(context).primaryIconTheme.color,
                ),
                onPressed: () => {
                  Navigator.pop(context),
                },
              ),
              flexibleSpace: PlaybackErrorListener(
                child: Column(
                  mainAxisAlignment: MainAxisAlignment.end,
                  children: <Widget>[
                    EpisodeTabBar(
                      chapters: episode.hasChapters,
                    ),
                  ],
                ),
              ),
            ),
            body: Column(
              children: [
                Expanded(
                  flex: 5,
                  child: EpisodeTabBarView(
                    episode: episode,
                    chapters: episode.hasChapters,
                  ),
                ),
                transportBuilder != null
                    ? transportBuilder!(context)
                    : const SizedBox(
                        height: 148.0,
                        child: NowPlayingTransport(),
                      ),
                if (MediaQuery.of(context).orientation == Orientation.portrait)
                  const Expanded(
                    flex: 1,
                    child: NowPlayingOptionsScaffold(),
                  ),
              ],
            ),
          ),
        ));
  }
}

/// This class is responsible for rendering the tab selection at the top of the screen.
///
/// It displays two or three tabs depending upon whether the current episode supports
/// (and contains) chapters.
class EpisodeTabBar extends StatelessWidget {
  final bool chapters;

  const EpisodeTabBar({
    super.key,
    this.chapters = false,
  });

  @override
  Widget build(BuildContext context) {
    return TabBar(
      isScrollable: true,
      indicatorSize: TabBarIndicatorSize.tab,
      indicator: DotDecoration(colour: Theme.of(context).primaryColor),
      tabs: [
        if (chapters)
          Tab(
            child: Align(
              alignment: Alignment.center,
              child: Text(L.of(context)!.chapters_label),
            ),
          ),
        Tab(
          child: Align(
            alignment: Alignment.center,
            child: Text(L.of(context)!.episode_label),
          ),
        ),
        Tab(
          child: Align(
            alignment: Alignment.center,
            child: Text(L.of(context)!.notes_label),
          ),
        ),
      ],
    );
  }
}

/// This class is responsible for rendering the tab bodies.
///
/// This includes the chapter selection view (if the episode supports chapters),
/// the episode details (image and description) and the show notes view.
class EpisodeTabBarView extends StatelessWidget {
  final Episode? episode;
  final AutoSizeGroup? textGroup;
  final bool chapters;

  const EpisodeTabBarView({
    super.key,
    this.episode,
    this.textGroup,
    this.chapters = false,
  });

  @override
  Widget build(BuildContext context) {
    final audioBloc = Provider.of<AudioBloc>(context);

    return TabBarView(
      children: [
        if (chapters)
          ChapterSelector(
            episode: episode!,
          ),
        StreamBuilder<Episode?>(
            stream: audioBloc.nowPlaying,
            builder: (context, snapshot) {
              final e = snapshot.hasData ? snapshot.data! : episode!;

              return NowPlayingEpisode(
                episode: e,
                imageUrl: e.positionalImageUrl,
                textGroup: textGroup,
              );
            }),
        NowPlayingShowNotes(episode: episode),
      ],
    );
  }
}

/// This is the parent widget for the episode position and transport
/// controls.
class NowPlayingTransport extends StatelessWidget {
  const NowPlayingTransport({super.key});

  @override
  Widget build(BuildContext context) {
    return const Column(
      children: <Widget>[
        Divider(
          height: 0.0,
        ),
        PlayerPositionControls(),
        PlayerTransportControls(),
      ],
    );
  }
}

/// This widget allows users to inject their own transport controls
/// into the app.
///
/// When rendering the controls, Pinepods will check if a PlayerControlsBuilder
/// is in the tree. If so, it will use the builder rather than its own
/// transport controls.
class PlayerControlsBuilder extends InheritedWidget {
  final WidgetBuilder Function(int duration) builder;

  const PlayerControlsBuilder({
    super.key,
    required this.builder,
    required super.child,
  });

  static PlayerControlsBuilder? of(BuildContext context) {
    return context.dependOnInheritedWidgetOfExactType<PlayerControlsBuilder>();
  }

  @override
  bool updateShouldNotify(PlayerControlsBuilder oldWidget) {
    return builder != oldWidget.builder;
  }
}
