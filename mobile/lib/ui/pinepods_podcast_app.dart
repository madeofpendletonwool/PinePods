// Copyright 2020 Ben Hills and the project contributors. All rights reserved.
// Use of this source code is governed by a BSD-style license that can be
// found in the LICENSE file.

import 'dart:async';

import 'package:pinepods_mobile/api/podcast/mobile_podcast_api.dart';
import 'package:pinepods_mobile/api/podcast/podcast_api.dart';
import 'package:pinepods_mobile/bloc/discovery/discovery_bloc.dart';
import 'package:pinepods_mobile/bloc/podcast/audio_bloc.dart';
import 'package:pinepods_mobile/bloc/podcast/episode_bloc.dart';
import 'package:pinepods_mobile/bloc/podcast/opml_bloc.dart';
import 'package:pinepods_mobile/bloc/podcast/podcast_bloc.dart';
import 'package:pinepods_mobile/bloc/podcast/queue_bloc.dart';
import 'package:pinepods_mobile/bloc/search/search_bloc.dart';
import 'package:pinepods_mobile/bloc/settings/settings_bloc.dart';
import 'package:pinepods_mobile/bloc/ui/pager_bloc.dart';
import 'package:pinepods_mobile/core/environment.dart';
import 'package:pinepods_mobile/entities/feed.dart';
import 'package:pinepods_mobile/entities/podcast.dart';
import 'package:pinepods_mobile/l10n/L.dart';
import 'package:pinepods_mobile/navigation/navigation_route_observer.dart';
import 'package:pinepods_mobile/repository/repository.dart';
import 'package:pinepods_mobile/repository/sembast/sembast_repository.dart';
import 'package:pinepods_mobile/services/audio/audio_player_service.dart';
import 'package:pinepods_mobile/services/audio/default_audio_player_service.dart';
import 'package:pinepods_mobile/services/download/download_service.dart';
import 'package:pinepods_mobile/services/download/mobile_download_manager.dart';
import 'package:pinepods_mobile/services/download/mobile_download_service.dart';
import 'package:pinepods_mobile/services/podcast/mobile_opml_service.dart';
import 'package:pinepods_mobile/services/podcast/mobile_podcast_service.dart';
import 'package:pinepods_mobile/services/podcast/opml_service.dart';
import 'package:pinepods_mobile/services/podcast/podcast_service.dart';
import 'package:pinepods_mobile/services/settings/mobile_settings_service.dart';
import 'package:pinepods_mobile/ui/library/discovery.dart';
import 'package:pinepods_mobile/ui/library/downloads.dart';
import 'package:pinepods_mobile/ui/library/library.dart';
import 'package:pinepods_mobile/ui/podcast/mini_player.dart';
import 'package:pinepods_mobile/ui/podcast/podcast_details.dart';
import 'package:pinepods_mobile/ui/search/search.dart';
import 'package:pinepods_mobile/ui/pinepods/search.dart';
import 'package:pinepods_mobile/ui/settings/settings.dart';
import 'package:pinepods_mobile/ui/themes.dart';
import 'package:pinepods_mobile/ui/widgets/action_text.dart';
import 'package:pinepods_mobile/ui/widgets/layout_selector.dart';
import 'package:pinepods_mobile/ui/widgets/search_slide_route.dart';
import 'package:pinepods_mobile/ui/pinepods/home.dart';
import 'package:pinepods_mobile/ui/pinepods/feed.dart';
import 'package:pinepods_mobile/ui/pinepods/saved.dart';
import 'package:pinepods_mobile/ui/pinepods/queue.dart';
import 'package:pinepods_mobile/ui/pinepods/history.dart';
import 'package:pinepods_mobile/ui/pinepods/playlists.dart';
import 'package:pinepods_mobile/ui/auth/auth_wrapper.dart';
import 'package:pinepods_mobile/ui/pinepods/user_stats.dart';
import 'package:pinepods_mobile/ui/pinepods/podcasts.dart';
import 'package:pinepods_mobile/ui/pinepods/episode_search.dart';
import 'package:app_links/app_links.dart';
import 'package:crypto/crypto.dart';
import 'dart:convert';
import 'package:flutter/foundation.dart';
import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:flutter_dialogs/flutter_dialogs.dart';
import 'package:flutter_localizations/flutter_localizations.dart';
import 'package:logging/logging.dart';
import 'package:provider/provider.dart';
import 'package:url_launcher/url_launcher.dart';

var theme = Themes.lightTheme().themeData;

/// PinePods is a Podcast player. You can search and subscribe to podcasts,
/// download and stream episodes and view the latest podcast charts.
// ignore: must_be_immutable
class PinepodsPodcastApp extends StatefulWidget {
  final Repository repository;
  late PodcastApi podcastApi;
  late DownloadService downloadService;
  late AudioPlayerService audioPlayerService;
  late OPMLService opmlService;
  PodcastService? podcastService;
  SettingsBloc? settingsBloc;
  MobileSettingsService mobileSettingsService;
  List<int> certificateAuthorityBytes;

  PinepodsPodcastApp({
    super.key,
    required this.mobileSettingsService,
    required this.certificateAuthorityBytes,
  }) : repository = SembastRepository() {
    podcastApi = MobilePodcastApi();

    podcastService = MobilePodcastService(
      api: podcastApi,
      repository: repository,
      settingsService: mobileSettingsService,
    );

    assert(podcastService != null);

    downloadService = MobileDownloadService(
      repository: repository,
      downloadManager: MobileDownloaderManager(),
      podcastService: podcastService!,
    );

    audioPlayerService = DefaultAudioPlayerService(
      repository: repository,
      settingsService: mobileSettingsService,
      podcastService: podcastService!,
    );

    settingsBloc = SettingsBloc(mobileSettingsService);

    opmlService = MobileOPMLService(
      podcastService: podcastService!,
      repository: repository,
    );

    podcastApi.addClientAuthorityBytes(certificateAuthorityBytes);
  }

  @override
  PinepodsPodcastAppState createState() => PinepodsPodcastAppState();
}

class PinepodsPodcastAppState extends State<PinepodsPodcastApp> {
  ThemeData? theme;

  @override
  void initState() {
    super.initState();

    /// Listen to theme change events from settings.
    widget.settingsBloc!.settings.listen((event) {
      setState(() {
        var newTheme = ThemeRegistry.getThemeData(event.theme);

        /// Only update the theme if it has changed.
        if (newTheme != theme) {
          theme = newTheme;
        }
      });
    });

    // Initialize theme from current settings
    theme = ThemeRegistry.getThemeData(widget.mobileSettingsService.theme);
  }

  @override
  Widget build(BuildContext context) {
    return MultiProvider(
      providers: [
        Provider<SearchBloc>(
          create: (_) => SearchBloc(
            podcastService: widget.podcastService!,
          ),
          dispose: (_, value) => value.dispose(),
        ),
        Provider<DiscoveryBloc>(
          create: (_) => DiscoveryBloc(
            podcastService: widget.podcastService!,
          ),
          dispose: (_, value) => value.dispose(),
        ),
        Provider<EpisodeBloc>(
          create: (_) =>
              EpisodeBloc(podcastService: widget.podcastService!, audioPlayerService: widget.audioPlayerService),
          dispose: (_, value) => value.dispose(),
        ),
        Provider<PodcastBloc>(
          create: (_) => PodcastBloc(
              podcastService: widget.podcastService!,
              audioPlayerService: widget.audioPlayerService,
              downloadService: widget.downloadService,
              settingsService: widget.mobileSettingsService),
          dispose: (_, value) => value.dispose(),
        ),
        Provider<PagerBloc>(
          create: (_) => PagerBloc(),
          dispose: (_, value) => value.dispose(),
        ),
        Provider<AudioBloc>(
          create: (_) => AudioBloc(audioPlayerService: widget.audioPlayerService),
          dispose: (_, value) => value.dispose(),
        ),
        Provider<SettingsBloc?>(
          create: (_) => widget.settingsBloc,
          dispose: (_, value) => value!.dispose(),
        ),
        Provider<OPMLBloc>(
          create: (_) => OPMLBloc(opmlService: widget.opmlService),
          dispose: (_, value) => value.dispose(),
        ),
        Provider<QueueBloc>(
          create: (_) => QueueBloc(
            audioPlayerService: widget.audioPlayerService,
            podcastService: widget.podcastService!,
          ),
          dispose: (_, value) => value.dispose(),
        ),
        Provider<AudioPlayerService>(
          create: (_) => widget.audioPlayerService,
        ),
        Provider<PodcastService>(
          create: (_) => widget.podcastService!,
        )
      ],
      child: MaterialApp(
        debugShowCheckedModeBanner: false,
        showSemanticsDebugger: false,
        title: 'Pinepods Podcast Client',
        navigatorObservers: [NavigationRouteObserver()],
        localizationsDelegates: const <LocalizationsDelegate<Object>>[
          AnytimeLocalisationsDelegate(),
          GlobalMaterialLocalizations.delegate,
          GlobalWidgetsLocalizations.delegate,
          GlobalCupertinoLocalizations.delegate,
        ],
        supportedLocales: const [
          Locale('en', ''),
          Locale('de', ''),
          Locale('it', ''),
        ],
        theme: theme,
        // Uncomment builder below to enable accessibility checker tool.
        // builder: (context, child) => AccessibilityTools(child: child),
        home: const AuthWrapper(
          child: AnytimeHomePage(title: 'PinePods Podcast Player'),
        ),
      ),
    );
  }
}

class AnytimeHomePage extends StatefulWidget {
  final String? title;
  final bool topBarVisible;

  const AnytimeHomePage({
    super.key,
    this.title,
    this.topBarVisible = true,
  });

  @override
  State<AnytimeHomePage> createState() => _AnytimeHomePageState();
}

class _AnytimeHomePageState extends State<AnytimeHomePage> with WidgetsBindingObserver {
  StreamSubscription<Uri>? deepLinkSubscription;

  final log = Logger('_AnytimeHomePageState');
  bool handledInitialLink = false;
  Widget? library;

  @override
  void initState() {
    super.initState();

    final audioBloc = Provider.of<AudioBloc>(context, listen: false);

    WidgetsBinding.instance.addObserver(this);

    audioBloc.transitionLifecycleState(LifecycleState.resume);

    /// Handle deep links
    _setupLinkListener();
  }

  /// We listen to external links from outside the app. For example, someone may navigate
  /// to a web page that supports 'Open with Anytime'.
  void _setupLinkListener() async {
    final appLinks = AppLinks(); // AppLinks is singleton

    // Subscribe to all events (initial link and further)
    deepLinkSubscription = appLinks.uriLinkStream.listen((uri) {
      // Do something (navigation, ...)
      _handleLinkEvent(uri);
    });
  }

  /// This method handles the actual link supplied from [uni_links], either
  /// at app startup or during running.
  void _handleLinkEvent(Uri uri) async {
    if ((uri.scheme == 'anytime-subscribe' || uri.scheme == 'https') &&
        (uri.query.startsWith('uri=') || uri.query.startsWith('url='))) {
      var path = uri.query.substring(4);
      var loadPodcastBloc = Provider.of<PodcastBloc>(context, listen: false);
      var routeName = NavigationRouteObserver().top!.settings.name;

      /// If we are currently on the podcast details page, we can simply request (via
      /// the BLoC) that we load this new URL. If not, we pop the stack until we are
      /// back at root and then load the podcast details page.
      if (routeName != null && routeName == 'podcastdetails') {
        loadPodcastBloc.load(Feed(
          podcast: Podcast.fromUrl(url: path),
          backgroundFresh: false,
          silently: false,
        ));
      } else {
        /// Pop back to route.
        Navigator.of(context).popUntil((route) {
          var currentRouteName = NavigationRouteObserver().top!.settings.name;

          return currentRouteName == null || currentRouteName == '' || currentRouteName == '/';
        });

        /// Once we have reached the root route, push podcast details.
        await Navigator.push(
          context,
          MaterialPageRoute<void>(
              fullscreenDialog: true,
              settings: const RouteSettings(name: 'podcastdetails'),
              builder: (context) => PodcastDetails(Podcast.fromUrl(url: path), loadPodcastBloc)),
        );
      }
    }
  }

  @override
  void dispose() {
    final audioBloc = Provider.of<AudioBloc>(context, listen: false);
    audioBloc.transitionLifecycleState(LifecycleState.pause);

    deepLinkSubscription?.cancel();

    WidgetsBinding.instance.removeObserver(this);
    super.dispose();
  }

  @override
  void didChangeAppLifecycleState(AppLifecycleState state) async {
    final audioBloc = Provider.of<AudioBloc>(context, listen: false);

    switch (state) {
      case AppLifecycleState.resumed:
        audioBloc.transitionLifecycleState(LifecycleState.resume);
        break;
      case AppLifecycleState.paused:
        audioBloc.transitionLifecycleState(LifecycleState.pause);
        break;
      default:
        break;
    }
  }

  @override
  Widget build(BuildContext context) {
    final pager = Provider.of<PagerBloc>(context);
    final searchBloc = Provider.of<EpisodeBloc>(context);
    final backgroundColour = Theme.of(context).scaffoldBackgroundColor;

    return AnnotatedRegion<SystemUiOverlayStyle>(
      value: Theme.of(context).appBarTheme.systemOverlayStyle!,
      child: Scaffold(
        backgroundColor: backgroundColour,
        body: Column(
          children: <Widget>[
            Expanded(
              child: CustomScrollView(
                slivers: <Widget>[
                  SliverVisibility(
                    visible: widget.topBarVisible,
                    sliver: SliverAppBar(
                      title: ExcludeSemantics(
                        child: TitleWidget(),
                      ),
                      backgroundColor: backgroundColour,
                      floating: false,
                      pinned: true,
                      snap: false,
                      actions: <Widget>[
                        IconButton(
                          tooltip: 'Queue',
                          icon: const Icon(Icons.queue_music),
                          onPressed: () async {
                            await Navigator.push(
                              context,
                              MaterialPageRoute<void>(
                                fullscreenDialog: false,
                                settings: const RouteSettings(name: 'queue'),
                                builder: (context) => Scaffold(
                                  appBar: AppBar(
                                    title: const Text('Queue'),
                                  ),
                                  body: const CustomScrollView(
                                    slivers: [
                                      PinepodsQueue(),
                                    ],
                                  ),
                                ),
                              ),
                            );
                          },
                        ),
                        IconButton(
                          tooltip: L.of(context)!.search_for_podcasts_hint,
                          icon: const Icon(Icons.search),
                          onPressed: () async {
                            await Navigator.push(
                              context,
                              defaultTargetPlatform == TargetPlatform.iOS
                                  ? MaterialPageRoute<void>(
                                      fullscreenDialog: false,
                                      settings: const RouteSettings(name: 'pinepods_search'),
                                      builder: (context) => const PinepodsSearch())
                                  : SlideRightRoute(
                                      widget: const PinepodsSearch(),
                                      settings: const RouteSettings(name: 'pinepods_search'),
                                    ),
                            );
                          },
                        ),
                        PopupMenuButton<String>(
                          onSelected: _menuSelect,
                          icon: const Icon(
                            Icons.more_vert,
                          ),
                          itemBuilder: (BuildContext context) {
                            return <PopupMenuEntry<String>>[
                              if (feedbackUrl.isNotEmpty)
                                PopupMenuItem<String>(
                                  textStyle: Theme.of(context).textTheme.titleMedium,
                                  value: 'feedback',
                                  child: Row(
                                    crossAxisAlignment: CrossAxisAlignment.center,
                                    children: [
                                      const Padding(
                                        padding: EdgeInsets.only(right: 8.0),
                                        child: Icon(Icons.feedback_outlined, size: 18.0),
                                      ),
                                      Text(L.of(context)!.feedback_menu_item_label),
                                    ],
                                  ),
                                ),
                              PopupMenuItem<String>(
                                textStyle: Theme.of(context).textTheme.titleMedium,
                                value: 'layout',
                                child: Row(
                                  crossAxisAlignment: CrossAxisAlignment.center,
                                  children: [
                                    const Padding(
                                      padding: EdgeInsets.only(right: 8.0),
                                      child: Icon(Icons.dashboard, size: 18.0),
                                    ),
                                    Text(L.of(context)!.layout_label),
                                  ],
                                ),
                              ),
                              PopupMenuItem<String>(
                                textStyle: Theme.of(context).textTheme.titleMedium,
                                value: 'rss',
                                child: Row(
                                  crossAxisAlignment: CrossAxisAlignment.center,
                                  children: [
                                    const Padding(
                                      padding: EdgeInsets.only(right: 8.0),
                                      child: Icon(Icons.rss_feed, size: 18.0),
                                    ),
                                    Text(L.of(context)!.add_rss_feed_option),
                                  ],
                                ),
                              ),
                              PopupMenuItem<String>(
                                textStyle: Theme.of(context).textTheme.titleMedium,
                                value: 'settings',
                                child: Row(
                                  children: [
                                    const Padding(
                                      padding: EdgeInsets.only(right: 8.0),
                                      child: Icon(Icons.settings, size: 18.0),
                                    ),
                                    Text(L.of(context)!.settings_label),
                                  ],
                                ),
                              ),
                              PopupMenuItem<String>(
                                textStyle: Theme.of(context).textTheme.titleMedium,
                                value: 'about',
                                child: Row(
                                  children: [
                                    const Padding(
                                      padding: EdgeInsets.only(right: 8.0),
                                      child: Icon(Icons.info_outline, size: 18.0),
                                    ),
                                    Text(L.of(context)!.about_label),
                                  ],
                                ),
                              ),
                            ];
                          },
                        ),
                      ],
                    ),
                  ),
                  StreamBuilder<int>(
                      stream: pager.currentPage,
                      builder: (BuildContext context, AsyncSnapshot<int> snapshot) {
                        return _fragment(snapshot.data, searchBloc);
                      }),
                ],
              ),
            ),
            const MiniPlayer(),
          ],
        ),
        bottomNavigationBar: StreamBuilder<int>(
            stream: pager.currentPage,
            initialData: 0,
            builder: (BuildContext context, AsyncSnapshot<int> snapshot) {
              int index = snapshot.data ?? 0;

              final List<BottomNavItem> navItems = [
                BottomNavItem(
                  icon: index == 0 ? Icons.home : Icons.home_outlined,
                  label: 'Home',
                  isSelected: index == 0,
                ),
                BottomNavItem(
                  icon: index == 1 ? Icons.rss_feed : Icons.rss_feed_outlined,
                  label: 'Feed',
                  isSelected: index == 1,
                ),
                BottomNavItem(
                  icon: index == 2 ? Icons.history : Icons.history_outlined,
                  label: 'History',
                  isSelected: index == 2,
                ),
                BottomNavItem(
                  icon: index == 3 ? Icons.bookmark : Icons.bookmark_outline,
                  label: 'Saved',
                  isSelected: index == 3,
                ),
                BottomNavItem(
                  icon: index == 4 ? Icons.download : Icons.download_outlined,
                  label: 'Downloads',
                  isSelected: index == 4,
                ),
                BottomNavItem(
                  icon: index == 5 ? Icons.playlist_play : Icons.playlist_play_outlined,
                  label: 'Playlists',
                  isSelected: index == 5,
                ),
                BottomNavItem(
                  icon: index == 6 ? Icons.podcasts : Icons.podcasts_outlined,
                  label: 'Podcasts',
                  isSelected: index == 6,
                ),
                BottomNavItem(
                  icon: index == 7 ? Icons.search : Icons.search_outlined,
                  label: 'Search',
                  isSelected: index == 7,
                ),
              ];

              return Container(
                height: 70,
                decoration: BoxDecoration(
                  color: Theme.of(context).bottomAppBarTheme.color,
                  border: Border(
                    top: BorderSide(
                      color: Theme.of(context).dividerColor,
                      width: 0.5,
                    ),
                  ),
                ),
                child: SingleChildScrollView(
                  scrollDirection: Axis.horizontal,
                  child: Row(
                    children: navItems.asMap().entries.map((entry) {
                      int itemIndex = entry.key;
                      BottomNavItem item = entry.value;
                      
                      return GestureDetector(
                        onTap: () => pager.changePage(itemIndex),
                        child: Container(
                          width: 80,
                          padding: const EdgeInsets.symmetric(vertical: 8),
                          child: Column(
                            mainAxisSize: MainAxisSize.min,
                            children: [
                              Icon(
                                item.icon,
                                color: item.isSelected
                                    ? Theme.of(context).iconTheme.color
                                    : HSLColor.fromColor(Theme.of(context).bottomAppBarTheme.color!)
                                        .withLightness(0.8)
                                        .toColor(),
                                size: 24,
                              ),
                              const SizedBox(height: 4),
                              Text(
                                item.label,
                                style: TextStyle(
                                  fontSize: 11,
                                  color: item.isSelected
                                      ? Theme.of(context).iconTheme.color
                                      : HSLColor.fromColor(Theme.of(context).bottomAppBarTheme.color!)
                                          .withLightness(0.8)
                                          .toColor(),
                                  fontWeight: item.isSelected ? FontWeight.w600 : FontWeight.normal,
                                ),
                                textAlign: TextAlign.center,
                              ),
                            ],
                          ),
                        ),
                      );
                    }).toList(),
                  ),
                ),
              );
            }),
      ),
    );
  }

  Widget _fragment(int? index, EpisodeBloc searchBloc) {
    switch (index) {
      case 0:
        return const PinepodsHome(); // Home
      case 1:
        return const PinepodsFeed(); // Feed
      case 2:
        return const PinepodsHistory(); // History 
      case 3:
        return const PinepodsSaved(); // Saved
      case 4:
        return const Downloads(); // Downloads
      case 5:
        return const PinepodsPlaylists(); // Playlists
      case 6:
        return const PinepodsPodcasts(); // Podcasts
      case 7:
        return const EpisodeSearchPage(); // Episode Search
      default:
        return const PinepodsHome(); // Default to Home
    }
  }

  void _menuSelect(String choice) async {
    var textFieldController = TextEditingController();
    var podcastBloc = Provider.of<PodcastBloc>(context, listen: false);
    final theme = Theme.of(context);
    var url = '';

    switch (choice) {
      case 'about':
        showAboutDialog(
          context: context,
          applicationName: 'PinePods Podcast Player',
          applicationVersion: 'v${Environment.projectVersion}',
          applicationIcon: Image.asset(
            'assets/images/pinepods-logo.png',
            width: 52.0,
            height: 52.0,
          ),
          children: <Widget>[
            const Text('Copyright © 2025 Gooseberry Development'),
            const SizedBox(height: 8.0),
            const Text(
              'The Pinepods Mobile App is an open-source podcast player adapted from the '
                  'Anytime Podcast Player (© 2020 Ben Hills). Portions of this application '
                  'retain the original BSD 3-Clause license.',
            ),
            GestureDetector(
              onTap: () {
                launchUrl(Uri.parse('https://github.com/amugofjava/anytime_podcast_player'));
              },
              child: Text(
                'View original project on GitHub',
                style: TextStyle(
                  decoration: TextDecoration.underline,
                  color: Theme.of(context).indicatorColor,
                ),
              ),
            ),
          ],
        );
        break;

      case 'settings':
        await Navigator.push(
          context,
          MaterialPageRoute<void>(
            fullscreenDialog: true,
            settings: const RouteSettings(name: 'settings'),
            builder: (context) => const Settings(),
          ),
        );
        break;
      case 'feedback':
        _launchFeedback();
        break;
      case 'layout':
        await showModalBottomSheet<void>(
          context: context,
          backgroundColor: theme.secondaryHeaderColor,
          barrierLabel: L.of(context)!.scrim_layout_selector,
          shape: const RoundedRectangleBorder(
            borderRadius: BorderRadius.only(
              topLeft: Radius.circular(16.0),
              topRight: Radius.circular(16.0),
            ),
          ),
          builder: (context) => const LayoutSelectorWidget(),
        );
        break;
      case 'rss':
        await showPlatformDialog<void>(
          context: context,
          useRootNavigator: false,
          builder: (_) => BasicDialogAlert(
            title: Text(L.of(context)!.add_rss_feed_option),
            content: Material(
              color: Colors.transparent,
              child: TextField(
                onChanged: (value) {
                  setState(() {
                    url = value;
                  });
                },
                controller: textFieldController,
                decoration: const InputDecoration(hintText: 'https://'),
              ),
            ),
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
                  L.of(context)!.ok_button_label,
                ),
                iosIsDefaultAction: true,
                onPressed: () {
                  Navigator.push(
                    context,
                    MaterialPageRoute<void>(
                        settings: const RouteSettings(name: 'podcastdetails'),
                        builder: (context) => PodcastDetails(Podcast.fromUrl(url: url), podcastBloc)),
                  ).then((value) {
                    if (mounted) {
                      Navigator.of(context).pop();
                    }
                  });
                },
              ),
            ],
          ),
        );
        break;
    }
  }

  void _launchFeedback() async {
    final uri = Uri.parse(feedbackUrl);

    if (!await launchUrl(
      uri,
      mode: LaunchMode.externalApplication,
    )) {
      throw Exception('Could not launch $uri');
    }
  }

  void _launchEmail() async {
    final uri = Uri.parse('mailto:hello@anytimeplayer.app');

    if (await canLaunchUrl(uri)) {
      await launchUrl(uri);
    } else {
      throw 'Could not launch $uri';
    }
  }
}

class TitleWidget extends StatelessWidget {
  TitleWidget({
    super.key,
  });

  String _generateGravatarUrl(String email, {int size = 40}) {
    final hash = md5.convert(utf8.encode(email.toLowerCase().trim())).toString();
    return 'https://www.gravatar.com/avatar/$hash?s=$size&d=identicon';
  }

  @override
  Widget build(BuildContext context) {
    return Consumer<SettingsBloc>(
      builder: (context, settingsBloc, child) {
        final settings = settingsBloc.currentSettings;
        final username = settings.pinepodsUsername;
        final email = settings.pinepodsEmail;

        if (username == null || username.isEmpty) {
          // Fallback to PinePods logo if no user is logged in - make it clickable
          return GestureDetector(
            onTap: () {
              Navigator.push(
                context,
                MaterialPageRoute(
                  builder: (context) => const PinepodsUserStats(),
                ),
              );
            },
            child: Padding(
              padding: const EdgeInsets.only(left: 2.0),
              child: Row(
                children: <Widget>[
                  Text(
                    'Pine',
                    style: TextStyle(
                      color: const Color(0xFF539e8a),
                      fontWeight: FontWeight.bold,
                      fontFamily: 'MontserratRegular',
                      fontSize: 18,
                    ),
                  ),
                  Text(
                    'Pods',
                    style: TextStyle(
                      color: Theme.of(context).brightness == Brightness.light ? Colors.black : Colors.white,
                      fontWeight: FontWeight.bold,
                      fontFamily: 'MontserratRegular',
                      fontSize: 18,
                    ),
                  ),
                ],
              ),
            ),
          );
        }

        return GestureDetector(
          onTap: () {
            Navigator.push(
              context,
              MaterialPageRoute(
                builder: (context) => const PinepodsUserStats(),
              ),
            );
          },
          child: Padding(
            padding: const EdgeInsets.only(left: 2.0),
            child: Row(
              children: [
                // User Avatar
                CircleAvatar(
                  radius: 18,
                  backgroundColor: Colors.grey[300],
                  child: email != null && email.isNotEmpty
                      ? ClipOval(
                          child: Image.network(
                            _generateGravatarUrl(email),
                            width: 36,
                            height: 36,
                            fit: BoxFit.cover,
                            errorBuilder: (context, error, stackTrace) {
                              return Image.asset(
                                'assets/images/pinepods-logo.png',
                                width: 36,
                                height: 36,
                                fit: BoxFit.cover,
                              );
                            },
                          ),
                        )
                      : Image.asset(
                          'assets/images/pinepods-logo.png',
                          width: 36,
                          height: 36,
                          fit: BoxFit.cover,
                        ),
                ),
                const SizedBox(width: 12),
                // Username
                Flexible(
                  child: Text(
                    username,
                    style: TextStyle(
                      color: Theme.of(context).brightness == Brightness.light ? Colors.black : Colors.white,
                      fontWeight: FontWeight.w600,
                      fontSize: 16,
                    ),
                    overflow: TextOverflow.ellipsis,
                  ),
                ),
              ],
            ),
          ),
        );
      },
    );
  }
}

class BottomNavItem {
  final IconData icon;
  final String label;
  final bool isSelected;

  BottomNavItem({
    required this.icon,
    required this.label,
    required this.isSelected,
  });
}
