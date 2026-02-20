// Copyright 2020 Ben Hills and the project contributors. All rights reserved.
// Use of this source code is governed by a BSD-style license that can be
// found in the LICENSE file.

import 'dart:async';

import 'package:pinepods_mobile/api/podcast/mobile_podcast_api.dart';
import 'package:pinepods_mobile/api/podcast/podcast_api.dart';
import 'package:pinepods_mobile/bloc/podcast/audio_bloc.dart';
import 'package:pinepods_mobile/bloc/podcast/episode_bloc.dart';
import 'package:pinepods_mobile/bloc/podcast/podcast_bloc.dart';
import 'package:pinepods_mobile/bloc/podcast/queue_bloc.dart';
import 'package:pinepods_mobile/bloc/search/search_bloc.dart';
import 'package:pinepods_mobile/bloc/settings/settings_bloc.dart';
import 'package:pinepods_mobile/bloc/ui/pager_bloc.dart';
import 'package:pinepods_mobile/core/environment.dart';
import 'package:pinepods_mobile/entities/app_settings.dart';
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
import 'package:pinepods_mobile/services/podcast/mobile_podcast_service.dart';
import 'package:pinepods_mobile/services/podcast/podcast_service.dart';
import 'package:pinepods_mobile/services/pinepods/pinepods_service.dart';
import 'package:pinepods_mobile/services/pinepods/pinepods_audio_service.dart';
import 'package:pinepods_mobile/services/pinepods/oidc_service.dart';
import 'package:pinepods_mobile/services/pinepods/login_service.dart';
import 'package:pinepods_mobile/services/auth_notifier.dart';
import 'package:pinepods_mobile/services/settings/mobile_settings_service.dart';
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
import 'package:pinepods_mobile/ui/pinepods/podcast_details.dart';
import 'package:pinepods_mobile/entities/pinepods_search.dart';
import 'package:pinepods_mobile/services/pinepods/pinepods_service.dart';
import 'package:pinepods_mobile/services/podcast/mobile_podcast_service.dart';
import 'package:pinepods_mobile/api/podcast/mobile_podcast_api.dart';
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
import 'package:pinepods_mobile/services/global_services.dart';

var theme = Themes.lightTheme().themeData;

/// PinePods is a Podcast player. You can search and subscribe to podcasts,
/// download and stream episodes and view the latest podcast charts.
// ignore: must_be_immutable
class PinepodsPodcastApp extends StatefulWidget {
  final Repository repository;
  late PodcastApi podcastApi;
  late DownloadService downloadService;
  late AudioPlayerService audioPlayerService;
  PodcastService? podcastService;
  SettingsBloc? settingsBloc;
  MobileSettingsService mobileSettingsService;
  List<int> certificateAuthorityBytes;
  late PinepodsAudioService pinepodsAudioService;
  late PinepodsService pinepodsService;

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

    // Create and connect PinepodsAudioService for listen duration tracking
    pinepodsService = PinepodsService();
    pinepodsAudioService = PinepodsAudioService(
      audioPlayerService!,
      pinepodsService,
      settingsBloc!,
    );

    // Connect the services for listen duration recording
    (audioPlayerService as DefaultAudioPlayerService).setPinepodsAudioService(
      pinepodsAudioService,
    );

    // Initialize global services for app-wide access
    GlobalServices.initialize(
      pinepodsAudioService: pinepodsAudioService,
      pinepodsService: pinepodsService,
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
          create: (_) => SearchBloc(podcastService: widget.podcastService!),
          dispose: (_, value) => value.dispose(),
        ),
        Provider<EpisodeBloc>(
          create: (_) => EpisodeBloc(
            podcastService: widget.podcastService!,
            audioPlayerService: widget.audioPlayerService,
          ),
          dispose: (_, value) => value.dispose(),
        ),
        Provider<PodcastBloc>(
          create: (_) => PodcastBloc(
            podcastService: widget.podcastService!,
            audioPlayerService: widget.audioPlayerService,
            downloadService: widget.downloadService,
            settingsService: widget.mobileSettingsService,
          ),
          dispose: (_, value) => value.dispose(),
        ),
        Provider<PagerBloc>(
          create: (_) => PagerBloc(),
          dispose: (_, value) => value.dispose(),
        ),
        Provider<AudioBloc>(
          create: (_) =>
              AudioBloc(audioPlayerService: widget.audioPlayerService),
          dispose: (_, value) => value.dispose(),
        ),
        Provider<SettingsBloc?>(
          create: (_) => widget.settingsBloc,
          dispose: (_, value) => value!.dispose(),
        ),
        Provider<QueueBloc>(
          create: (_) => QueueBloc(
            audioPlayerService: widget.audioPlayerService,
            podcastService: widget.podcastService!,
          ),
          dispose: (_, value) => value.dispose(),
        ),
        Provider<AudioPlayerService>(create: (_) => widget.audioPlayerService),
        Provider<PodcastService>(create: (_) => widget.podcastService!),
      ],
      child: MaterialApp(
        debugShowCheckedModeBanner: false,
        showSemanticsDebugger: false,
        title: 'Pinepods Podcast Client',
        navigatorObservers: [NavigationRouteObserver()],
        localizationsDelegates: const <LocalizationsDelegate<Object>>[
          PinepodsLocalisationsDelegate(),
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
          child: PinepodsHomePage(title: 'PinePods Podcast Player'),
        ),
      ),
    );
  }
}

class PinepodsHomePage extends StatefulWidget {
  final String? title;
  final bool topBarVisible;

  const PinepodsHomePage({super.key, this.title, this.topBarVisible = true});

  @override
  State<PinepodsHomePage> createState() => _PinepodsHomePageState();
}

class _PinepodsHomePageState extends State<PinepodsHomePage>
    with WidgetsBindingObserver {
  StreamSubscription<Uri>? deepLinkSubscription;

  final log = Logger('_PinepodsHomePageState');
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
  /// to a web page that supports 'Open with Pinepods'.
  void _setupLinkListener() async {
    print('Deep Link: Setting up link listener...');
    final appLinks = AppLinks(); // AppLinks is singleton

    // Handle initial link if app was launched by one (cold start)
    try {
      final initialUri = await appLinks.getInitialLink();
      if (initialUri != null) {
        print('Deep Link: App launched with initial link: $initialUri');
        _handleLinkEvent(initialUri);
      } else {
        print('Deep Link: No initial link found');
      }
    } catch (e) {
      print('Deep Link: Error getting initial link: $e');
    }

    // Subscribe to all events (further links while app is running)
    print('Deep Link: Setting up stream listener...');
    deepLinkSubscription = appLinks.uriLinkStream.listen((uri) {
      print('Deep Link: App received link while running: $uri');
      _handleLinkEvent(uri);
    }, onError: (err) {
      print('Deep Link: Stream error: $err');
    });
    
    print('Deep Link: Link listener setup complete');
  }

  /// This method handles the actual link supplied from [uni_links], either
  /// at app startup or during running.
  void _handleLinkEvent(Uri uri) async {
    print('Deep Link: Received link: $uri');
    print('Deep Link: Scheme: ${uri.scheme}, Host: ${uri.host}, Path: ${uri.path}');
    print('Deep Link: Query: ${uri.query}');
    print('Deep Link: QueryParameters: ${uri.queryParameters}');
    
    // Handle OIDC authentication callback - be more flexible with path matching
    if (uri.scheme == 'pinepods' && uri.host == 'auth') {
      print('Deep Link: OIDC callback detected (flexible match)');
      await _handleOidcCallback(uri);
      return;
    }
    
    // Handle OIDC authentication callback - strict match
    if (uri.scheme == 'pinepods' && uri.host == 'auth' && uri.path == '/callback') {
      print('Deep Link: OIDC callback detected (strict match)');
      await _handleOidcCallback(uri);
      return;
    }
    
    // Handle podcast subscription links
    if ((uri.scheme == 'pinepods-subscribe' || uri.scheme == 'https') &&
        (uri.query.startsWith('uri=') || uri.query.startsWith('url='))) {
      var path = uri.query.substring(4);
      var loadPodcastBloc = Provider.of<PodcastBloc>(context, listen: false);
      var routeName = NavigationRouteObserver().top!.settings.name;

      /// If we are currently on the podcast details page, we can simply request (via
      /// the BLoC) that we load this new URL. If not, we pop the stack until we are
      /// back at root and then load the podcast details page.
      if (routeName != null && routeName == 'podcastdetails') {
        loadPodcastBloc.load(
          Feed(
            podcast: Podcast.fromUrl(url: path),
            backgroundFresh: false,
            silently: false,
          ),
        );
      } else {
        /// Pop back to route.
        Navigator.of(context).popUntil((route) {
          var currentRouteName = NavigationRouteObserver().top!.settings.name;

          return currentRouteName == null ||
              currentRouteName == '' ||
              currentRouteName == '/';
        });

        /// Once we have reached the root route, push podcast details.
        await Navigator.push(
          context,
          MaterialPageRoute<void>(
            fullscreenDialog: true,
            settings: const RouteSettings(name: 'podcastdetails'),
            builder: (context) =>
                PodcastDetails(Podcast.fromUrl(url: path), loadPodcastBloc),
          ),
        );
      }
    }
  }

  /// Handle OIDC authentication callback
  Future<void> _handleOidcCallback(Uri uri) async {
    try {
      print('OIDC Callback: Received callback URL: $uri');
      
      // Parse the callback result
      final callbackResult = OidcService.parseCallback(uri.toString());
      
      if (!callbackResult.isSuccess) {
        print('OIDC Callback: Authentication failed: ${callbackResult.error}');
        if (context.mounted) {
          ScaffoldMessenger.of(context).showSnackBar(
            SnackBar(
              content: Text('OIDC authentication failed: ${callbackResult.error}'),
              backgroundColor: Colors.red,
            ),
          );
        }
        return;
      }

      // Check if we have an API key directly from the callback
      if (callbackResult.hasApiKey) {
        print('OIDC Callback: Found API key in callback, completing login');
        await _completeOidcLogin(callbackResult.apiKey!);
      } else {
        print('OIDC Callback: No API key found, traditional OAuth flow not implemented yet');
        if (context.mounted) {
          ScaffoldMessenger.of(context).showSnackBar(
            const SnackBar(
              content: Text('OIDC callback received but no API key found'),
              backgroundColor: Colors.orange,
            ),
          );
        }
      }
      
    } catch (e) {
      print('OIDC Callback: Error processing callback: $e');
      if (context.mounted) {
        ScaffoldMessenger.of(context).showSnackBar(
          SnackBar(
            content: Text('Error processing OIDC callback: $e'),
            backgroundColor: Colors.red,
          ),
        );
      }
    }
  }

  /// Complete OIDC login with the provided API key
  Future<void> _completeOidcLogin(String apiKey) async {
    try {
      print('OIDC Callback: Completing login with API key');
      
      // We need to get the server URL - we can get it from the current settings
      // since the user would have entered it during the initial OIDC flow
      final settingsBloc = Provider.of<SettingsBloc>(context, listen: false);
      final settings = settingsBloc.currentSettings;
      
      // Check if we have a server URL from a previous attempt
      String? serverUrl = settings.pinepodsServer;
      
      if (serverUrl == null || serverUrl.isEmpty) {
        throw Exception('No server URL available for OIDC completion');
      }
      
      // Verify the API key works and get user details
      // Verify API key
      final isValidKey = await PinepodsLoginService.verifyApiKey(serverUrl, apiKey);
      if (!isValidKey) {
        throw Exception('API key verification failed');
      }

      // Get user ID
      final userId = await PinepodsLoginService.getUserId(serverUrl, apiKey);
      if (userId == null) {
        throw Exception('Failed to get user ID');
      }

      // Get user details  
      final userDetails = await PinepodsLoginService.getUserDetails(serverUrl, apiKey, userId);
      if (userDetails == null) {
        throw Exception('Failed to get user details');
      }

      // Save the authentication details
      settingsBloc.setPinepodsServer(serverUrl);
      settingsBloc.setPinepodsApiKey(apiKey);
      settingsBloc.setPinepodsUserId(userId);
      
      // Set additional user details if available
      if (userDetails.username != null) {
        settingsBloc.setPinepodsUsername(userDetails.username!);
      }
      if (userDetails.email != null) {
        settingsBloc.setPinepodsEmail(userDetails.email!);  
      }

      // Fetch theme from server
      await settingsBloc.fetchThemeFromServer();

      print('OIDC Callback: Login completed successfully');
      
      if (context.mounted) {
        ScaffoldMessenger.of(context).showSnackBar(
          const SnackBar(
            content: Text('OIDC authentication successful!'),
            backgroundColor: Colors.green,
          ),
        );
        
        // Log current settings state for debugging
        final currentSettings = settingsBloc.currentSettings;
        print('OIDC Callback: Current settings after update:');
        print('  Server: ${currentSettings.pinepodsServer}');
        print('  API Key: ${currentSettings.pinepodsApiKey != null ? '[SET]' : '[NOT SET]'}');
        print('  User ID: ${currentSettings.pinepodsUserId}');
        print('  Username: ${currentSettings.pinepodsUsername}');
        
        // Notify login success globally
        AuthNotifier.notifyLoginSuccess();
      }
      
    } catch (e) {
      print('OIDC Callback: Error completing login: $e');
      if (context.mounted) {
        ScaffoldMessenger.of(context).showSnackBar(
          SnackBar(
            content: Text('Failed to complete OIDC login: $e'),
            backgroundColor: Colors.red,
          ),
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
    print('Deep Link: App lifecycle state changed to: $state');
    final audioBloc = Provider.of<AudioBloc>(context, listen: false);

    switch (state) {
      case AppLifecycleState.resumed:
        print('Deep Link: App resumed - checking for pending deep links...');
        audioBloc.transitionLifecycleState(LifecycleState.resume);
        
        // Check for any pending deep links when app resumes
        try {
          final appLinks = AppLinks();
          final initialUri = await appLinks.getInitialLink();
          if (initialUri != null) {
            print('Deep Link: Found pending link on resume: $initialUri');
            _handleLinkEvent(initialUri);
          }
        } catch (e) {
          print('Deep Link: Error checking for pending links on resume: $e');
        }
        break;
      case AppLifecycleState.paused:
        print('Deep Link: App paused');
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
                      title: ExcludeSemantics(child: TitleWidget()),
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
                                  appBar: AppBar(title: const Text('Queue')),
                                  body: const Column(
                                    children: [
                                      Expanded(
                                        child: CustomScrollView(
                                          slivers: [PinepodsQueue()],
                                        ),
                                      ),
                                      MiniPlayer(),
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
                                      settings: const RouteSettings(
                                        name: 'pinepods_search',
                                      ),
                                      builder: (context) =>
                                          const PinepodsSearch(),
                                    )
                                  : SlideRightRoute(
                                      widget: const PinepodsSearch(),
                                      settings: const RouteSettings(
                                        name: 'pinepods_search',
                                      ),
                                    ),
                            );
                          },
                        ),
                        PopupMenuButton<String>(
                          onSelected: _menuSelect,
                          icon: const Icon(Icons.more_vert),
                          itemBuilder: (BuildContext context) {
                            return <PopupMenuEntry<String>>[
                              if (feedbackUrl.isNotEmpty)
                                PopupMenuItem<String>(
                                  textStyle: Theme.of(
                                    context,
                                  ).textTheme.titleMedium,
                                  value: 'feedback',
                                  child: Row(
                                    crossAxisAlignment:
                                        CrossAxisAlignment.center,
                                    children: [
                                      const Padding(
                                        padding: EdgeInsets.only(right: 8.0),
                                        child: Icon(
                                          Icons.feedback_outlined,
                                          size: 18.0,
                                        ),
                                      ),
                                      Text(
                                        L.of(context)!.feedback_menu_item_label,
                                      ),
                                    ],
                                  ),
                                ),
                              PopupMenuItem<String>(
                                textStyle: Theme.of(
                                  context,
                                ).textTheme.titleMedium,
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
                                textStyle: Theme.of(
                                  context,
                                ).textTheme.titleMedium,
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
                            ];
                          },
                        ),
                      ],
                    ),
                  ),
                  StreamBuilder<int>(
                    stream: pager.currentPage,
                    builder:
                        (BuildContext context, AsyncSnapshot<int> snapshot) {
                          return _fragment(snapshot.data, searchBloc);
                        },
                  ),
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

            return StreamBuilder<AppSettings>(
              stream: Provider.of<SettingsBloc>(context).settings,
              builder:
                  (
                    BuildContext context,
                    AsyncSnapshot<AppSettings> settingsSnapshot,
                  ) {
                    final bottomBarOrder =
                        settingsSnapshot.data?.bottomBarOrder ??
                        [
                          'Home',
                          'Feed',
                          'Saved',
                          'Podcasts',
                          'Downloads',
                          'History',
                          'Playlists',
                          'Search',
                        ];

                    // Create a map of all available nav items
                    final Map<String, BottomNavItem> allNavItems = {
                      'Home': BottomNavItem(
                        icon: Icons.home,
                        label: 'Home',
                        isSelected: false,
                      ),
                      'Feed': BottomNavItem(
                        icon: Icons.rss_feed,
                        label: 'Feed',
                        isSelected: false,
                      ),
                      'Saved': BottomNavItem(
                        icon: Icons.bookmark,
                        label: 'Saved',
                        isSelected: false,
                      ),
                      'Podcasts': BottomNavItem(
                        icon: Icons.podcasts,
                        label: 'Podcasts',
                        isSelected: false,
                      ),
                      'Downloads': BottomNavItem(
                        icon: Icons.download,
                        label: 'Downloads',
                        isSelected: false,
                      ),
                      'History': BottomNavItem(
                        icon: Icons.history,
                        label: 'History',
                        isSelected: false,
                      ),
                      'Playlists': BottomNavItem(
                        icon: Icons.playlist_play,
                        label: 'Playlists',
                        isSelected: false,
                      ),
                      'Search': BottomNavItem(
                        icon: Icons.search,
                        label: 'Search',
                        isSelected: false,
                      ),
                    };

                    // Create the ordered nav items based on settings
                    final List<BottomNavItem> navItems = bottomBarOrder.map((
                      label,
                    ) {
                      final baseItem = allNavItems[label]!;
                      final itemIndex = bottomBarOrder.indexOf(label);
                      return BottomNavItem(
                        icon: index == itemIndex
                            ? _getSelectedIcon(label)
                            : _getUnselectedIcon(label),
                        label: label,
                        isSelected: index == itemIndex,
                      );
                    }).toList();

                    // Calculate if all icons fit in the current screen width
                    final screenWidth = MediaQuery.of(context).size.width;
                    final iconWidth = 80.0;
                    final totalIconsWidth = navItems.length * iconWidth;
                    final isLandscape = MediaQuery.of(context).orientation == Orientation.landscape;
                    final shouldCenterInPortrait = !isLandscape && totalIconsWidth <= screenWidth;

                    return Container(
                      height: 70 + MediaQuery.of(context).padding.bottom,
                      decoration: BoxDecoration(
                        color: Theme.of(context).bottomAppBarTheme.color,
                        border: Border(
                          top: BorderSide(
                            color: Theme.of(context).dividerColor,
                            width: 0.5,
                          ),
                        ),
                      ),
                      child: (isLandscape || shouldCenterInPortrait)
                          ? Padding(
                              padding: EdgeInsets.only(
                                bottom: MediaQuery.of(context).padding.bottom,
                              ),
                              child: Center(
                                child: Row(
                                  mainAxisSize: MainAxisSize.min,
                                  children: navItems.asMap().entries.map((entry) {
                                    int itemIndex = entry.key;
                                    BottomNavItem item = entry.value;

                                    return GestureDetector(
                                      onTap: () => pager.changePage(itemIndex),
                                      child: Container(
                                        width: 80,
                                        padding: const EdgeInsets.symmetric(
                                          vertical: 8,
                                        ),
                                        child: Column(
                                          mainAxisSize: MainAxisSize.min,
                                          children: [
                                            Icon(
                                              item.icon,
                                              color: item.isSelected
                                                  ? Theme.of(
                                                      context,
                                                    ).iconTheme.color
                                                  : HSLColor.fromColor(
                                                          Theme.of(context)
                                                              .bottomAppBarTheme
                                                              .color!,
                                                        )
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
                                                    ? Theme.of(
                                                        context,
                                                      ).iconTheme.color
                                                    : HSLColor.fromColor(
                                                            Theme.of(context)
                                                                .bottomAppBarTheme
                                                                .color!,
                                                          )
                                                          .withLightness(0.8)
                                                          .toColor(),
                                                fontWeight: item.isSelected
                                                    ? FontWeight.w600
                                                    : FontWeight.normal,
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
                            )
                          : Padding(
                              padding: EdgeInsets.only(
                                bottom: MediaQuery.of(context).padding.bottom,
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
                                        padding: const EdgeInsets.symmetric(
                                          vertical: 8,
                                        ),
                                        child: Column(
                                          mainAxisSize: MainAxisSize.min,
                                          children: [
                                            Icon(
                                              item.icon,
                                              color: item.isSelected
                                                  ? Theme.of(
                                                      context,
                                                    ).iconTheme.color
                                                  : HSLColor.fromColor(
                                                          Theme.of(context)
                                                              .bottomAppBarTheme
                                                              .color!,
                                                        )
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
                                                    ? Theme.of(
                                                        context,
                                                      ).iconTheme.color
                                                    : HSLColor.fromColor(
                                                            Theme.of(context)
                                                                .bottomAppBarTheme
                                                                .color!,
                                                          )
                                                          .withLightness(0.8)
                                                          .toColor(),
                                                fontWeight: item.isSelected
                                                    ? FontWeight.w600
                                                    : FontWeight.normal,
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
                            ),
                    );
                  },
            );
          },
        ),
      ),
    );
  }

  Widget _fragment(int? index, EpisodeBloc searchBloc) {
    final settingsBloc = Provider.of<SettingsBloc>(context, listen: false);
    final bottomBarOrder = settingsBloc.currentSettings.bottomBarOrder;

    if (index == null || index < 0 || index >= bottomBarOrder.length) {
      return const PinepodsHome(); // Default to Home
    }

    final pageLabel = bottomBarOrder[index];

    switch (pageLabel) {
      case 'Home':
        return const PinepodsHome();
      case 'Feed':
        return const PinepodsFeed();
      case 'Saved':
        return const PinepodsSaved();
      case 'Podcasts':
        return const PinepodsPodcasts();
      case 'Downloads':
        return const Downloads();
      case 'History':
        return const PinepodsHistory();
      case 'Playlists':
        return const PinepodsPlaylists();
      case 'Search':
        return const EpisodeSearchPage();
      default:
        return const PinepodsHome(); // Default to Home
    }
  }

  IconData _getSelectedIcon(String label) {
    switch (label) {
      case 'Home':
        return Icons.home;
      case 'Feed':
        return Icons.rss_feed;
      case 'Saved':
        return Icons.bookmark;
      case 'Podcasts':
        return Icons.podcasts;
      case 'Downloads':
        return Icons.download;
      case 'History':
        return Icons.history;
      case 'Playlists':
        return Icons.playlist_play;
      case 'Search':
        return Icons.search;
      default:
        return Icons.home;
    }
  }

  IconData _getUnselectedIcon(String label) {
    switch (label) {
      case 'Home':
        return Icons.home_outlined;
      case 'Feed':
        return Icons.rss_feed_outlined;
      case 'Saved':
        return Icons.bookmark_outline;
      case 'Podcasts':
        return Icons.podcasts_outlined;
      case 'Downloads':
        return Icons.download_outlined;
      case 'History':
        return Icons.history_outlined;
      case 'Playlists':
        return Icons.playlist_play_outlined;
      case 'Search':
        return Icons.search_outlined;
      default:
        return Icons.home_outlined;
    }
  }

  void _menuSelect(String choice) async {
    var textFieldController = TextEditingController();
    var podcastBloc = Provider.of<PodcastBloc>(context, listen: false);
    final theme = Theme.of(context);
    var url = '';

    switch (choice) {
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
                title: ActionText(L.of(context)!.cancel_button_label),
                onPressed: () {
                  Navigator.pop(context);
                },
              ),
              BasicDialogAction(
                title: ActionText(L.of(context)!.ok_button_label),
                iosIsDefaultAction: true,
                onPressed: () async {
                  Navigator.of(context).pop(); // Close the dialog first

                  // Show loading indicator
                  showDialog(
                    context: context,
                    barrierDismissible: false,
                    builder: (context) =>
                        const Center(child: CircularProgressIndicator()),
                  );

                  try {
                    await _handleRssUrl(url);
                  } catch (e) {
                    if (mounted) {
                      Navigator.of(context).pop(); // Close loading dialog
                      ScaffoldMessenger.of(context).showSnackBar(
                        SnackBar(
                          content: Text('Failed to load podcast: $e'),
                          backgroundColor: Colors.red,
                        ),
                      );
                    }
                  }
                },
              ),
            ],
          ),
        );
        break;
    }
  }

  Future<void> _handleRssUrl(String url) async {
    try {
      // Get services
      final podcastApi = MobilePodcastApi();
      final pinepodsService = PinepodsService();

      // Load podcast feed from RSS
      final podcast = await podcastApi.loadFeed(url);

      // Create UnifiedPinepodsPodcast from the loaded feed
      final unifiedPodcast = UnifiedPinepodsPodcast(
        id: 0,
        indexId: 0,
        title: podcast.title ?? 'Unknown Podcast',
        url: url,
        originalUrl: url,
        link: podcast.link ?? url,
        description: podcast.description ?? '',
        author: podcast.copyright ?? '',
        ownerName: podcast.copyright ?? '',
        image: podcast.image ?? '',
        artwork: podcast.image ?? '',
        lastUpdateTime: 0,
        categories: null,
        explicit: false,
        episodeCount: podcast.episodes?.length ?? 0,
      );

      // Check if podcast is already followed
      bool isFollowing = false;
      final settingsBloc = Provider.of<SettingsBloc>(context, listen: false);
      final settings = settingsBloc.currentSettings;
      final userId = settings.pinepodsUserId;

      if (userId != null) {
        try {
          isFollowing = await pinepodsService.checkPodcastExists(
            podcast.title ?? 'Unknown Podcast',
            url,
            userId,
          );
        } catch (e) {
          print('Failed to check if podcast exists: $e');
        }
      }

      if (mounted) {
        Navigator.of(context).pop(); // Close loading dialog

        // Navigate to podcast details page
        Navigator.push(
          context,
          MaterialPageRoute<void>(
            settings: const RouteSettings(name: 'pinepodspodcastdetails'),
            builder: (context) => PinepodsPodcastDetails(
              podcast: unifiedPodcast,
              isFollowing: isFollowing,
              onFollowChanged: (following) {
                // Handle follow state change if needed
              },
            ),
          ),
        );
      }
    } catch (e) {
      rethrow;
    }
  }

  void _launchFeedback() async {
    final uri = Uri.parse(feedbackUrl);

    if (!await launchUrl(uri, mode: LaunchMode.externalApplication)) {
      throw Exception('Could not launch $uri');
    }
  }

  void _launchEmail() async {
    final uri = Uri.parse('mailto:mobile-support@pinepods.online');

    if (await canLaunchUrl(uri)) {
      await launchUrl(uri);
    } else {
      throw 'Could not launch $uri';
    }
  }
}

class TitleWidget extends StatelessWidget {
  TitleWidget({super.key});

  String _generateGravatarUrl(String email, {int size = 40}) {
    final hash = md5
        .convert(utf8.encode(email.toLowerCase().trim()))
        .toString();
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
                      color: Theme.of(context).brightness == Brightness.light
                          ? Colors.black
                          : Colors.white,
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
                      color: Theme.of(context).brightness == Brightness.light
                          ? Colors.black
                          : Colors.white,
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
