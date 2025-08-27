// Copyright 2020 Ben Hills and the project contributors. All rights reserved.
// Use of this source code is governed by a BSD-style license that can be
// found in the LICENSE file.

import 'dart:async';

import 'package:pinepods_mobile/bloc/podcast/podcast_bloc.dart';
import 'package:pinepods_mobile/bloc/settings/settings_bloc.dart';
import 'package:pinepods_mobile/entities/episode.dart';
import 'package:pinepods_mobile/entities/feed.dart';
import 'package:pinepods_mobile/entities/podcast.dart';
import 'package:pinepods_mobile/l10n/L.dart';
import 'package:pinepods_mobile/state/bloc_state.dart';
import 'package:pinepods_mobile/ui/podcast/funding_menu.dart';
import 'package:pinepods_mobile/ui/podcast/playback_error_listener.dart';
import 'package:pinepods_mobile/ui/podcast/podcast_context_menu.dart';
import 'package:pinepods_mobile/ui/podcast/podcast_episode_list.dart';
import 'package:pinepods_mobile/ui/widgets/action_text.dart';
import 'package:pinepods_mobile/ui/widgets/delayed_progress_indicator.dart';
import 'package:pinepods_mobile/ui/widgets/episode_filter_selector.dart';
import 'package:pinepods_mobile/ui/widgets/episode_sort_selector.dart';
import 'package:pinepods_mobile/ui/widgets/placeholder_builder.dart';
import 'package:pinepods_mobile/ui/widgets/platform_back_button.dart';
import 'package:pinepods_mobile/ui/widgets/platform_progress_indicator.dart';
import 'package:pinepods_mobile/ui/widgets/podcast_html.dart';
import 'package:pinepods_mobile/ui/widgets/podcast_image.dart';
import 'package:pinepods_mobile/ui/widgets/sync_spinner.dart';
import 'package:pinepods_mobile/ui/podcast/mini_player.dart';
import 'package:pinepods_mobile/ui/pinepods/podcast_details.dart';
import 'package:pinepods_mobile/entities/pinepods_search.dart';
import 'package:pinepods_mobile/services/pinepods/pinepods_service.dart';
import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:flutter_dialogs/flutter_dialogs.dart';
import 'package:flutter_html/flutter_html.dart';
import 'package:logging/logging.dart';
import 'package:provider/provider.dart';

/// This Widget takes a search result and builds a list of currently available podcasts.
///
/// From here a user can option to subscribe/unsubscribe or play a podcast directly
/// from a search result.
class PodcastDetails extends StatefulWidget {
  final Podcast podcast;
  final PodcastBloc _podcastBloc;

  const PodcastDetails(
    this.podcast,
    this._podcastBloc, {
    super.key,
  });

  @override
  State<PodcastDetails> createState() => _PodcastDetailsState();
}

class _PodcastDetailsState extends State<PodcastDetails> {
  final log = Logger('PodcastDetails');
  final scaffoldMessengerKey = GlobalKey<ScaffoldMessengerState>();
  final ScrollController _sliverScrollController = ScrollController();
  var brightness = Brightness.dark;
  bool toolbarCollapsed = false;
  SystemUiOverlayStyle? _systemOverlayStyle;

  @override
  void initState() {
    super.initState();

    // Load the details of the Podcast specified in the URL
    log.fine('initState() - load feed');

    widget._podcastBloc.load(Feed(
      podcast: widget.podcast,
      backgroundFresh: true,
      silently: true,
    ));

    // We only want to display the podcast title when the toolbar is in a
    // collapsed state. Add a listener and set toollbarCollapsed variable
    // as required. The text display property is then based on this boolean.
    _sliverScrollController.addListener(() {
      if (!toolbarCollapsed &&
          _sliverScrollController.hasClients &&
          _sliverScrollController.offset > (300 - kToolbarHeight)) {
        setState(() {
          toolbarCollapsed = true;
          _updateSystemOverlayStyle();
        });
      } else if (toolbarCollapsed &&
          _sliverScrollController.hasClients &&
          _sliverScrollController.offset < (300 - kToolbarHeight)) {
        setState(() {
          toolbarCollapsed = false;
          _updateSystemOverlayStyle();
        });
      }
    });

    widget._podcastBloc.backgroundLoading.where((event) => event is BlocPopulatedState<void>).listen((event) {
      if (mounted) {
        /// If we have not scrolled (save a few pixels) just refresh the episode list;
        /// otherwise prompt the user to prevent unexpected list jumping
        if (_sliverScrollController.offset < 20) {
          widget._podcastBloc.podcastEvent(PodcastEvent.refresh);
        } else {
          scaffoldMessengerKey.currentState!.showSnackBar(SnackBar(
            content: Text(L.of(context)!.new_episodes_label),
            behavior: SnackBarBehavior.floating,
            action: SnackBarAction(
              label: L.of(context)!.new_episodes_view_now_label,
              onPressed: () {
                _sliverScrollController.animateTo(100,
                    duration: const Duration(milliseconds: 500), curve: Curves.easeInOut);
                widget._podcastBloc.podcastEvent(PodcastEvent.refresh);
              },
            ),
            duration: const Duration(seconds: 5),
          ));
        }
      }
    });
  }

  @override
  void didChangeDependencies() {
    _systemOverlayStyle = SystemUiOverlayStyle(
      statusBarIconBrightness: Theme.of(context).brightness == Brightness.light ? Brightness.dark : Brightness.light,
      statusBarColor: Theme.of(context).appBarTheme.backgroundColor!.withOpacity(toolbarCollapsed ? 1.0 : 0.5),
    );
    super.didChangeDependencies();
  }

  @override
  void dispose() {
    super.dispose();
  }

  Future<void> _handleRefresh() async {
    log.fine('_handleRefresh');

    widget._podcastBloc.load(Feed(
      podcast: widget.podcast,
      refresh: true,
    ));
  }

  void _resetSystemOverlayStyle() {
    setState(() {
      _systemOverlayStyle = SystemUiOverlayStyle(
        statusBarIconBrightness: Theme.of(context).brightness == Brightness.light ? Brightness.dark : Brightness.light,
        statusBarColor: Colors.transparent,
      );
    });
  }

  void _updateSystemOverlayStyle() {
    setState(() {
      _systemOverlayStyle = SystemUiOverlayStyle(
        statusBarIconBrightness: Theme.of(context).brightness == Brightness.light ? Brightness.dark : Brightness.light,
        statusBarColor: Theme.of(context).appBarTheme.backgroundColor!.withOpacity(toolbarCollapsed ? 1.0 : 0.5),
      );
    });
  }

  /// TODO: This really needs a refactor. There are too many nested streams on this now and it needs simplifying.
  @override
  Widget build(BuildContext context) {
    final podcastBloc = Provider.of<PodcastBloc>(context, listen: false);
    final placeholderBuilder = PlaceholderBuilder.of(context);

    return Semantics(
      header: false,
      label: L.of(context)!.semantics_podcast_details_header,
      child: PopScope(
        canPop: true,
        onPopInvokedWithResult: (didPop, result) {
          _resetSystemOverlayStyle();
          podcastBloc.podcastSearchEvent('');
        },
        child: ScaffoldMessenger(
          key: scaffoldMessengerKey,
          child: Scaffold(
            backgroundColor: Theme.of(context).scaffoldBackgroundColor,
            body: Column(
              children: [
                Expanded(
                  child: RefreshIndicator(
                    displacement: 60.0,
                    onRefresh: _handleRefresh,
                    child: CustomScrollView(
                physics: const AlwaysScrollableScrollPhysics(),
                controller: _sliverScrollController,
                slivers: <Widget>[
                  SliverAppBar(
                      systemOverlayStyle: _systemOverlayStyle,
                      title: AnimatedOpacity(
                          opacity: toolbarCollapsed ? 1.0 : 0.0,
                          duration: const Duration(milliseconds: 500),
                          child: Text(widget.podcast.title)),
                      leading: PlatformBackButton(
                        iconColour: toolbarCollapsed && Theme.of(context).brightness == Brightness.light
                            ? Theme.of(context).appBarTheme.foregroundColor!
                            : Colors.white,
                        decorationColour: toolbarCollapsed ? const Color(0x00000000) : const Color(0x22000000),
                        onPressed: () {
                          _resetSystemOverlayStyle();
                          Navigator.pop(context);
                        },
                      ),
                      expandedHeight: 300.0,
                      floating: false,
                      pinned: true,
                      snap: false,
                      flexibleSpace: FlexibleSpaceBar(
                        background: Hero(
                          key: Key('detailhero${widget.podcast.imageUrl}:${widget.podcast.link}'),
                          tag: '${widget.podcast.imageUrl}:${widget.podcast.link}',
                          child: ExcludeSemantics(
                            child: StreamBuilder<BlocState<Podcast>>(
                                initialData: BlocEmptyState<Podcast>(),
                                stream: podcastBloc.details,
                                builder: (context, snapshot) {
                                  final state = snapshot.data;
                                  Podcast? podcast = widget.podcast;

                                  if (state is BlocLoadingState<Podcast>) {
                                    podcast = state.data;
                                  }

                                  if (state is BlocPopulatedState<Podcast>) {
                                    podcast = state.results;
                                  }

                                  return PodcastHeaderImage(
                                    podcast: podcast!,
                                    placeholderBuilder: placeholderBuilder,
                                  );
                                }),
                          ),
                        ),
                      )),
                  StreamBuilder<BlocState<Podcast>>(
                      initialData: BlocEmptyState<Podcast>(),
                      stream: podcastBloc.details,
                      builder: (context, snapshot) {
                        final state = snapshot.data;

                        if (state is BlocLoadingState) {
                          return const SliverToBoxAdapter(
                            child: Padding(
                              padding: EdgeInsets.all(24.0),
                              child: Column(
                                children: <Widget>[
                                  PlatformProgressIndicator(),
                                ],
                              ),
                            ),
                          );
                        }

                        if (state is BlocErrorState) {
                          return SliverFillRemaining(
                            hasScrollBody: false,
                            child: Padding(
                              padding: const EdgeInsets.all(32.0),
                              child: Column(
                                mainAxisAlignment: MainAxisAlignment.center,
                                crossAxisAlignment: CrossAxisAlignment.center,
                                children: <Widget>[
                                  const Icon(
                                    Icons.error_outline,
                                    size: 50,
                                  ),
                                  Text(
                                    L.of(context)!.no_podcast_details_message,
                                    style: Theme.of(context).textTheme.bodyMedium,
                                    textAlign: TextAlign.center,
                                  ),
                                ],
                              ),
                            ),
                          );
                        }

                        if (state is BlocPopulatedState<Podcast>) {
                          return SliverToBoxAdapter(
                              child: PlaybackErrorListener(
                            child: Column(
                              crossAxisAlignment: CrossAxisAlignment.start,
                              children: <Widget>[
                                PodcastTitle(state.results!),
                                const Divider(),
                              ],
                            ),
                          ));
                        }

                        return const SliverToBoxAdapter(
                          child: SizedBox(
                            width: 0.0,
                            height: 0.0,
                          ),
                        );
                      }),
                  StreamBuilder<BlocState<Podcast>>(
                      initialData: BlocEmptyState<Podcast>(),
                      stream: podcastBloc.details,
                      builder: (context1, snapshot1) {
                        final state = snapshot1.data;

                        if (state is BlocPopulatedState<Podcast>) {
                          return StreamBuilder<List<Episode?>?>(
                              stream: podcastBloc.episodes,
                              builder: (context, snapshot) {
                                if (snapshot.hasData) {
                                  return snapshot.data!.isNotEmpty
                                      ? PodcastEpisodeList(
                                          episodes: snapshot.data!,
                                          play: true,
                                          download: true,
                                        )
                                      : const SliverToBoxAdapter(child: NoEpisodesFound());
                                } else {
                                  return const SliverToBoxAdapter(
                                      child: SizedBox(
                                    height: 200,
                                    width: 200,
                                  ));
                                }
                              });
                        } else {
                          return const SliverToBoxAdapter(
                              child: SizedBox(
                            height: 200,
                            width: 200,
                          ));
                        }
                      }),
                ],
                    ),
                  ),
                ),
                const MiniPlayer(),
              ],
            ),
          ),
        ),
      ),
    );
  }
}

/// Renders the podcast or episode image.
class PodcastHeaderImage extends StatelessWidget {
  const PodcastHeaderImage({
    super.key,
    required this.podcast,
    required this.placeholderBuilder,
  });

  final Podcast podcast;
  final PlaceholderBuilder? placeholderBuilder;

  @override
  Widget build(BuildContext context) {
    if (podcast.imageUrl == null || podcast.imageUrl!.isEmpty) {
      return const SizedBox(
        height: 560,
        width: 560,
      );
    }

    return PodcastBannerImage(
      key: Key('details${podcast.imageUrl}'),
      url: podcast.imageUrl!,
      fit: BoxFit.cover,
      placeholder:
          placeholderBuilder != null ? placeholderBuilder?.builder()(context) : DelayedCircularProgressIndicator(),
      errorPlaceholder: placeholderBuilder != null
          ? placeholderBuilder?.errorBuilder()(context)
          : const Image(image: AssetImage('assets/images/favicon.png')),
    );
  }
}

/// Renders the podcast title, copyright, description, follow/unfollow and
/// overflow button.
///
/// If the episode description is fairly long, an overflow icon is also shown
/// and a portion of the episode description is shown. Tapping the overflow
/// icons allows the user to expand and collapse the text.
///
/// Description is rendered by [PodcastDescription].
/// Follow/Unfollow button rendered by [FollowButton].
class PodcastTitle extends StatefulWidget {
  final Podcast podcast;

  const PodcastTitle(this.podcast, {super.key});

  @override
  State<PodcastTitle> createState() => _PodcastTitleState();
}

class _PodcastTitleState extends State<PodcastTitle> with SingleTickerProviderStateMixin {
  final GlobalKey descriptionKey = GlobalKey();
  final maxHeight = 100.0;
  PodcastHtml? description;
  bool showOverflow = false;
  bool showEpisodeSearch = false;
  final StreamController<bool> isDescriptionExpandedStream = StreamController<bool>.broadcast();
  final _episodeSearchController = TextEditingController();
  final _searchFocus = FocusNode();

  late final AnimationController _controller = AnimationController(
    duration: const Duration(milliseconds: 200),
    vsync: this,
  );
  late final Animation<double> _animation = CurvedAnimation(
    parent: _controller,
    curve: Curves.fastOutSlowIn,
  );

  @override
  Widget build(BuildContext context) {
    final textTheme = Theme.of(context).textTheme;
    final settings = Provider.of<SettingsBloc>(context).currentSettings;
    final podcastBloc = Provider.of<PodcastBloc>(context, listen: false);

    return Padding(
      padding: const EdgeInsets.fromLTRB(8.0, 8.0, 8.0, 0.0),
      child: Column(
        mainAxisAlignment: MainAxisAlignment.start,
        crossAxisAlignment: CrossAxisAlignment.start,
        children: <Widget>[
          Row(
            mainAxisAlignment: MainAxisAlignment.start,
            crossAxisAlignment: CrossAxisAlignment.start,
            children: [
              Expanded(
                child: MergeSemantics(
                  child: Column(
                    mainAxisAlignment: MainAxisAlignment.start,
                    crossAxisAlignment: CrossAxisAlignment.start,
                    children: [
                      Padding(
                        padding: const EdgeInsets.fromLTRB(8.0, 8.0, 8.0, 2.0),
                        child: Text(widget.podcast.title, style: textTheme.titleLarge),
                      ),
                      Padding(
                        padding: const EdgeInsets.fromLTRB(8, 0, 8, 4),
                        child: Text(widget.podcast.copyright ?? '', style: textTheme.bodySmall),
                      ),
                    ],
                  ),
                ),
              ),
              StreamBuilder<bool>(
                  stream: isDescriptionExpandedStream.stream,
                  initialData: false,
                  builder: (context, snapshot) {
                    final expanded = snapshot.data!;
                    return Visibility(
                      visible: showOverflow,
                      child: SizedBox(
                        height: 48.0,
                        width: 48.0,
                        child: expanded
                            ? TextButton(
                                style: const ButtonStyle(
                                  visualDensity: VisualDensity.compact,
                                ),
                                child: Icon(
                                  Icons.expand_less,
                                  semanticLabel: L.of(context)!.semantics_collapse_podcast_description,
                                ),
                                onPressed: () {
                                  isDescriptionExpandedStream.add(false);
                                },
                              )
                            : TextButton(
                                style: const ButtonStyle(visualDensity: VisualDensity.compact),
                                child: Icon(
                                  Icons.expand_more,
                                  semanticLabel: L.of(context)!.semantics_expand_podcast_description,
                                ),
                                onPressed: () {
                                  isDescriptionExpandedStream.add(true);
                                },
                              ),
                      ),
                    );
                  })
            ],
          ),
          PodcastDescription(
            key: descriptionKey,
            content: description,
            isDescriptionExpandedStream: isDescriptionExpandedStream,
          ),
          Padding(
            padding: const EdgeInsets.only(left: 8.0, right: 8.0),
            child: Row(
              mainAxisAlignment: MainAxisAlignment.start,
              mainAxisSize: MainAxisSize.min,
              children: <Widget>[
                FollowButton(widget.podcast),
                PodcastContextMenu(widget.podcast),
                IconButton(
                  onPressed: () {
                    setState(() {
                      if (showEpisodeSearch) {
                        _controller.reverse();
                      } else {
                        _controller.forward();
                        _searchFocus.requestFocus();
                      }
                      showEpisodeSearch = !showEpisodeSearch;
                    });
                  },
                  icon: Icon(
                    Icons.search,
                    semanticLabel: L.of(context)!.search_episodes_label,
                  ),
                  visualDensity: VisualDensity.compact,
                ),
                SortButton(widget.podcast),
                FilterButton(widget.podcast),
                settings.showFunding
                    ? FundingMenu(widget.podcast.funding)
                    : const SizedBox(
                        width: 0.0,
                        height: 0.0,
                      ),
                const Expanded(
                    child: Align(
                  alignment: Alignment.centerRight,
                  child: SyncSpinner(),
                )),
              ],
            ),
          ),
          SizeTransition(
            sizeFactor: _animation,
            child: Padding(
                padding: const EdgeInsets.all(7.0),
                child: TextField(
                    focusNode: _searchFocus,
                    controller: _episodeSearchController,
                    decoration: InputDecoration(
                      contentPadding: const EdgeInsets.all(0.0),
                      prefixIcon: const Icon(Icons.search),
                      suffixIcon: IconButton(
                        icon: Icon(
                          Icons.close,
                          semanticLabel: L.of(context)!.clear_search_button_label,
                        ),
                        onPressed: () {
                          _episodeSearchController.clear();
                          podcastBloc.podcastSearchEvent('');
                        },
                      ),
                      isDense: true,
                      filled: true,
                      border: const OutlineInputBorder(
                        borderRadius: BorderRadius.all(Radius.circular(8.0)),
                        borderSide: BorderSide.none,
                        gapPadding: 0.0,
                      ),
                      hintText: L.of(context)!.search_episodes_label,
                    ),
                    onChanged: ((search) {
                      podcastBloc.podcastSearchEvent(search);
                    }),
                    onSubmitted: ((search) {
                      podcastBloc.podcastSearchEvent(search);
                    }),
                    onTapOutside: (event) => _searchFocus.unfocus())),
          ),
        ],
      ),
    );
  }

  @override
  void initState() {
    super.initState();

    description = PodcastHtml(
      content: widget.podcast.description!,
      fontSize: FontSize.medium,
    );

    WidgetsBinding.instance.addPostFrameCallback((timeStamp) {
      if (descriptionKey.currentContext!.size!.height == maxHeight) {
        setState(() {
          showOverflow = true;
        });
      }
    });
  }

  @override
  void dispose() {
    _episodeSearchController.dispose();
    _searchFocus.dispose();
    super.dispose();
  }
}

/// This class wraps the description in an expandable box.
///
/// This handles the common case whereby the description is very long and, without
/// this constraint, would require the use to always scroll before reaching the
/// podcast episodes.
///
/// TODO: Animate between the two states.
class PodcastDescription extends StatelessWidget {
  final PodcastHtml? content;
  final StreamController<bool>? isDescriptionExpandedStream;
  static const maxHeight = 100.0;
  static const padding = 4.0;

  const PodcastDescription({
    super.key,
    this.content,
    this.isDescriptionExpandedStream,
  });

  @override
  Widget build(BuildContext context) {
    return Padding(
      padding: const EdgeInsets.only(bottom: PodcastDescription.padding),
      child: StreamBuilder<bool>(
          stream: isDescriptionExpandedStream!.stream,
          initialData: false,
          builder: (context, snapshot) {
            final expanded = snapshot.data!;
            return AnimatedSize(
              duration: const Duration(milliseconds: 150),
              curve: Curves.fastOutSlowIn,
              alignment: Alignment.topCenter,
              child: Container(
                constraints: expanded
                    ? const BoxConstraints()
                    : BoxConstraints.loose(const Size(double.infinity, maxHeight - padding)),
                child: expanded
                    ? content
                    : ShaderMask(
                        shaderCallback: LinearGradient(
                          colors: [Colors.white, Colors.white.withAlpha(0)],
                          begin: Alignment.topCenter,
                          end: Alignment.bottomCenter,
                          stops: const [0.9, 1],
                        ).createShader,
                        child: content),
              ),
            );
          }),
    );
  }
}

class NoEpisodesFound extends StatelessWidget {
  const NoEpisodesFound({super.key});

  @override
  Widget build(BuildContext context) {
    return Padding(
      padding: const EdgeInsets.all(16.0),
      child: Column(
        mainAxisSize: MainAxisSize.min,
        mainAxisAlignment: MainAxisAlignment.center,
        crossAxisAlignment: CrossAxisAlignment.center,
        children: [
          Text(
            L.of(context)!.episode_filter_no_episodes_title_label,
            style: Theme.of(context).textTheme.titleLarge,
          ),
          Padding(
            padding: const EdgeInsets.fromLTRB(64.0, 24.0, 64.0, 64.0),
            child: Text(
              L.of(context)!.episode_filter_no_episodes_title_description,
              style: Theme.of(context).textTheme.titleSmall,
              textAlign: TextAlign.center,
            ),
          ),
        ],
      ),
    );
  }
}

class FollowButton extends StatefulWidget {
  final Podcast podcast;

  const FollowButton(this.podcast, {super.key});

  @override
  State<FollowButton> createState() => _FollowButtonState();
}

class _FollowButtonState extends State<FollowButton> {
  bool _isLoading = false;

  @override
  Widget build(BuildContext context) {
    final bloc = Provider.of<PodcastBloc>(context);

    // If we're in loading state, show loading button immediately
    if (_isLoading) {
      print('Follow button: Showing loading spinner - _isLoading=$_isLoading');
      return Semantics(
        liveRegion: true,
        child: OutlinedButton.icon(
          style: OutlinedButton.styleFrom(
            padding: const EdgeInsets.fromLTRB(10.0, 4.0, 10.0, 4.0),
            shape: RoundedRectangleBorder(borderRadius: BorderRadius.circular(8.0)),
          ),
          icon: const SizedBox(
            width: 20,
            height: 20,
            child: CircularProgressIndicator(
              strokeWidth: 3.0,
              valueColor: AlwaysStoppedAnimation<Color>(Colors.blue),
            ),
          ),
          label: Text(L.of(context)!.subscribe_label),
          onPressed: null,
        ),
      );
    }

    return StreamBuilder<BlocState<Podcast>>(
        stream: bloc.details,
        builder: (context, snapshot) {
          var ready = false;
          var subscribed = false;

          // To prevent jumpy UI, we always need to display the follow/unfollow button.
          // Display a disabled follow button until the full state it loaded.
          if (snapshot.hasData) {
            final state = snapshot.data;

            if (state is BlocLoadingState<Podcast>) {
              ready = false;
              subscribed = state.data?.subscribed ?? false;
              print('Follow button: BlocLoadingState - ready=$ready, subscribed=$subscribed, _isLoading=$_isLoading');
            } else if (state is BlocPopulatedState<Podcast>) {
              ready = true;
              subscribed = state.results!.subscribed;
              print('Follow button: BlocPopulatedState - ready=$ready, subscribed=$subscribed, _isLoading=$_isLoading');
              
              // Reset loading state when we get populated data
              if (_isLoading) {
                print('Follow button: Resetting loading state');
                WidgetsBinding.instance.addPostFrameCallback((_) {
                  if (mounted) {
                    setState(() {
                      _isLoading = false;
                    });
                    print('Follow button: Loading state reset to false');
                  }
                });
              }
            }
          }
          print('Follow button: Rendering normal UI - ready=$ready, subscribed=$subscribed, _isLoading=$_isLoading');
          
          return Semantics(
            liveRegion: true,
            child: subscribed
                ? OutlinedButton.icon(
                    style: OutlinedButton.styleFrom(
                      padding: const EdgeInsets.fromLTRB(10.0, 4.0, 10.0, 4.0),
                      shape: RoundedRectangleBorder(borderRadius: BorderRadius.circular(8.0)),
                    ),
                    icon: const Icon(
                      Icons.delete_outline,
                    ),
                    label: Text(L.of(context)!.unsubscribe_label),
                    onPressed: ready
                        ? () {
                            showPlatformDialog<void>(
                              context: context,
                              useRootNavigator: false,
                              builder: (_) => BasicDialogAlert(
                                title: Text(L.of(context)!.unsubscribe_label),
                                content: Text(L.of(context)!.unsubscribe_message),
                                actions: <Widget>[
                                  BasicDialogAction(
                                    title: ActionText(
                                      L.of(context)!.cancel_button_label,
                                    ),
                                    onPressed: () {
                                      Navigator.pop(context);
                                    },
                                  ),
                                  BasicDialogAction(
                                    title: ActionText(
                                      L.of(context)!.unsubscribe_button_label,
                                    ),
                                    iosIsDefaultAction: true,
                                    iosIsDestructiveAction: true,
                                    onPressed: () {
                                      bloc.podcastEvent(PodcastEvent.unsubscribe);

                                      Navigator.pop(context);
                                      Navigator.pop(context);
                                    },
                                  ),
                                ],
                              ),
                            );
                          }
                        : null,
                  )
                : OutlinedButton.icon(
                    style: OutlinedButton.styleFrom(
                      padding: const EdgeInsets.fromLTRB(10.0, 4.0, 10.0, 4.0),
                      shape: RoundedRectangleBorder(borderRadius: BorderRadius.circular(8.0)),
                    ),
                    icon: const Icon(
                      Icons.add,
                    ),
                    label: Text(L.of(context)!.subscribe_label),
                    onPressed: ready && !_isLoading
                        ? () async {
                            print('Follow button: CLICKED - Setting loading to true');
                            setState(() {
                              _isLoading = true;
                            });
                            print('Follow button: Loading state set to: $_isLoading');
                            
                            bloc.podcastEvent(PodcastEvent.subscribe);
                            
                            // Show loading indicator for a minimum time to be visible
                            await Future.delayed(const Duration(milliseconds: 300));
                            
                            // After successful subscription, check if we should switch to PinePods context
                            await _handlePostSubscriptionContextSwitch(context, bloc);
                          }
                        : null,
                  ),
          );
        });
  }

  Future<void> _handlePostSubscriptionContextSwitch(BuildContext context, PodcastBloc bloc) async {
    print('Follow button: Starting context switch check');
    // Wait a short moment for subscription to complete, then check if we should context switch
    await Future.delayed(const Duration(milliseconds: 500));
    
    if (!mounted) {
      print('Follow button: Widget not mounted, skipping context switch');
      return;
    }
    
    // Check if we're in PinePods environment and should switch contexts
    final settingsBloc = Provider.of<SettingsBloc>(context, listen: false);
    final settings = settingsBloc.currentSettings;
    
    if (settings.pinepodsServer != null && 
        settings.pinepodsApiKey != null && 
        settings.pinepodsUserId != null) {
      
      // Check if the podcast is now subscribed to PinePods
      final pinepodsService = PinepodsService();
      pinepodsService.setCredentials(settings.pinepodsServer!, settings.pinepodsApiKey!);
      
      try {
        final isSubscribed = await pinepodsService.checkPodcastExists(
          widget.podcast.title,
          widget.podcast.url ?? '',
          settings.pinepodsUserId!
        );
        
        if (isSubscribed && mounted) {
          print('Follow button: Podcast is subscribed, switching to PinePods context');
          
          // Reset loading state before context switch
          setState(() {
            _isLoading = false;
          });
          
          // Create unified podcast object for PinePods context
          final unifiedPodcast = UnifiedPinepodsPodcast(
            id: 0, // Will be fetched by PinePods component
            indexId: 0, // Default for subscribed podcasts
            title: widget.podcast.title,
            url: widget.podcast.url ?? '',
            originalUrl: widget.podcast.url ?? '',
            link: widget.podcast.link ?? '',
            description: widget.podcast.description ?? '',
            author: widget.podcast.copyright ?? '',
            ownerName: widget.podcast.copyright ?? '',
            image: widget.podcast.imageUrl ?? '',
            artwork: widget.podcast.imageUrl ?? '',
            lastUpdateTime: 0,
            explicit: false,
            episodeCount: 0, // Will be loaded
          );
          
          // Replace current route with PinePods podcast details
          Navigator.pushReplacement(
            context,
            MaterialPageRoute<void>(
              settings: const RouteSettings(name: 'pinepodspodcastdetails'),
              builder: (context) => PinepodsPodcastDetails(
                podcast: unifiedPodcast,
                isFollowing: true,
              ),
            ),
          );
        } else {
          print('Follow button: Podcast not subscribed or widget not mounted, staying in current context');
          // Reset loading state if not switching contexts
          if (mounted) {
            setState(() {
              _isLoading = false;
            });
          }
        }
      } catch (e) {
        print('Error checking post-subscription status: $e');
        // Reset loading state on error
        if (mounted) {
          setState(() {
            _isLoading = false;
          });
        }
      }
    } else {
      print('Follow button: Not in PinePods environment, staying in RSS context');
      // Reset loading state if not in PinePods environment
      if (mounted) {
        setState(() {
          _isLoading = false;
        });
      }
    }
  }
}

class FilterButton extends StatelessWidget {
  final Podcast podcast;

  const FilterButton(this.podcast, {super.key});

  @override
  Widget build(BuildContext context) {
    final bloc = Provider.of<PodcastBloc>(context);

    return StreamBuilder<BlocState<Podcast>>(
        stream: bloc.details,
        builder: (context, snapshot) {
          Podcast? podcast;

          if (snapshot.hasData) {
            final state = snapshot.data;

            if (state is BlocPopulatedState<Podcast>) {
              podcast = state.results!;
            }
          }

          return EpisodeFilterSelectorWidget(
            podcast: podcast,
          );
        });
  }
}

class SortButton extends StatelessWidget {
  final Podcast podcast;

  const SortButton(this.podcast, {super.key});

  @override
  Widget build(BuildContext context) {
    final bloc = Provider.of<PodcastBloc>(context);

    return StreamBuilder<BlocState<Podcast>>(
        stream: bloc.details,
        builder: (context, snapshot) {
          Podcast? podcast;

          if (snapshot.hasData) {
            final state = snapshot.data;

            if (state is BlocPopulatedState<Podcast>) {
              podcast = state.results!;
            }
          }

          return EpisodeSortSelectorWidget(
            podcast: podcast,
          );
        });
  }
}
