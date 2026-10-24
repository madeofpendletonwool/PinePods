// Copyright 2020 Ben Hills and the project contributors. All rights reserved.
// Use of this source code is governed by a BSD-style license that can be
// found in the LICENSE file.

import 'package:pinepods_mobile/bloc/podcast/podcast_bloc.dart';
import 'package:pinepods_mobile/bloc/settings/settings_bloc.dart';
import 'package:pinepods_mobile/entities/app_settings.dart';
import 'package:pinepods_mobile/entities/podcast.dart';
import 'package:pinepods_mobile/l10n/L.dart';
import 'package:pinepods_mobile/ui/widgets/platform_progress_indicator.dart';
import 'package:pinepods_mobile/ui/widgets/pinepods_podcast_grid_tile.dart';
import 'package:pinepods_mobile/ui/widgets/pinepods_podcast_tile.dart';
import 'package:pinepods_mobile/ui/widgets/layout_selector.dart';
import 'package:pinepods_mobile/services/pinepods/pinepods_service.dart';
import 'package:pinepods_mobile/ui/widgets/server_error_page.dart';
import 'package:pinepods_mobile/services/error_handling_service.dart';
import 'package:flutter/material.dart';
import 'package:provider/provider.dart';
import 'package:shared_preferences/shared_preferences.dart';
import 'package:sliver_tools/sliver_tools.dart';
import 'package:pinepods_mobile/ui/icons/phosphor_icons.dart';
import 'package:pinepods_mobile/ui/widgets/soft_ui.dart';

/// Sort direction options for podcasts
enum PodcastSortDirection {
  titleAZ,
  titleZA,
  mostEpisodes,
  leastEpisodes,
  newestSubscribed,
  oldestSubscribed,
}

/// This class displays the list of podcasts the user is subscribed to on the PinePods server.
class PinepodsPodcasts extends StatefulWidget {
  const PinepodsPodcasts({
    super.key,
  });

  @override
  State<PinepodsPodcasts> createState() => _PinepodsPodcastsState();
}

class _PinepodsPodcastsState extends State<PinepodsPodcasts> {
  List<Podcast>? _podcasts;
  List<Podcast>? _filteredPodcasts;
  bool _isLoading = true;
  String? _errorMessage;
  final PinepodsService _pinepodsService = PinepodsService();
  final TextEditingController _searchController = TextEditingController();
  String _searchQuery = '';

  // Sort and filter state
  PodcastSortDirection _sortDirection = PodcastSortDirection.titleAZ;
  String? _selectedCategory;
  List<String> _availableCategories = [];
  Map<int, List<String>> _podcastCategories = {}; // podcastId -> categories
  static const String _sortPreferenceKey = 'podcasts_sort_direction';

  @override
  void initState() {
    super.initState();
    _loadSortPreference();
    _loadPodcasts();
    _searchController.addListener(_onSearchChanged);
  }

  Future<void> _loadSortPreference() async {
    final prefs = await SharedPreferences.getInstance();
    final savedSort = prefs.getString(_sortPreferenceKey);
    if (savedSort != null) {
      setState(() {
        _sortDirection = _sortDirectionFromString(savedSort);
      });
    }
  }

  Future<void> _saveSortPreference(PodcastSortDirection direction) async {
    final prefs = await SharedPreferences.getInstance();
    await prefs.setString(_sortPreferenceKey, _sortDirectionToString(direction));
  }

  String _sortDirectionToString(PodcastSortDirection direction) {
    switch (direction) {
      case PodcastSortDirection.titleAZ:
        return 'title_az';
      case PodcastSortDirection.titleZA:
        return 'title_za';
      case PodcastSortDirection.mostEpisodes:
        return 'most_episodes';
      case PodcastSortDirection.leastEpisodes:
        return 'least_episodes';
      case PodcastSortDirection.newestSubscribed:
        return 'newest_subscribed';
      case PodcastSortDirection.oldestSubscribed:
        return 'oldest_subscribed';
    }
  }

  PodcastSortDirection _sortDirectionFromString(String value) {
    switch (value) {
      case 'title_za':
        return PodcastSortDirection.titleZA;
      case 'most_episodes':
        return PodcastSortDirection.mostEpisodes;
      case 'least_episodes':
        return PodcastSortDirection.leastEpisodes;
      case 'newest_subscribed':
        return PodcastSortDirection.newestSubscribed;
      case 'oldest_subscribed':
        return PodcastSortDirection.oldestSubscribed;
      case 'title_az':
      default:
        return PodcastSortDirection.titleAZ;
    }
  }

  void _setSortDirection(PodcastSortDirection direction) {
    setState(() {
      _sortDirection = direction;
      _filterPodcasts();
    });
    _saveSortPreference(direction);
  }

  void _setCategory(String? category) {
    setState(() {
      _selectedCategory = category;
      _filterPodcasts();
    });
  }

  void _clearAllFilters() {
    setState(() {
      _selectedCategory = null;
      _searchController.clear();
      _searchQuery = '';
      _filterPodcasts();
    });
  }

  @override
  void dispose() {
    _searchController.dispose();
    super.dispose();
  }

  void _onSearchChanged() {
    setState(() {
      _searchQuery = _searchController.text;
      _filterPodcasts();
    });
  }

  void _filterPodcasts() {
    if (_podcasts == null) {
      _filteredPodcasts = null;
      return;
    }

    var filtered = List<Podcast>.from(_podcasts!);

    // Apply search filter
    if (_searchQuery.isNotEmpty) {
      filtered = filtered.where((podcast) {
        return podcast.title.toLowerCase().contains(_searchQuery.toLowerCase()) ||
               (podcast.description?.toLowerCase().contains(_searchQuery.toLowerCase()) ?? false);
      }).toList();
    }

    // Apply category filter
    if (_selectedCategory != null) {
      filtered = filtered.where((podcast) {
        final categories = _podcastCategories[podcast.id] ?? [];
        return categories.contains(_selectedCategory);
      }).toList();
    }

    // Apply sorting
    filtered.sort((a, b) {
      switch (_sortDirection) {
        case PodcastSortDirection.titleAZ:
          return a.title.toLowerCase().compareTo(b.title.toLowerCase());
        case PodcastSortDirection.titleZA:
          return b.title.toLowerCase().compareTo(a.title.toLowerCase());
        case PodcastSortDirection.mostEpisodes:
          return b.episodes.length.compareTo(a.episodes.length);
        case PodcastSortDirection.leastEpisodes:
          return a.episodes.length.compareTo(b.episodes.length);
        case PodcastSortDirection.newestSubscribed:
          final aDate = a.subscribedDate ?? DateTime(1970);
          final bDate = b.subscribedDate ?? DateTime(1970);
          return bDate.compareTo(aDate);
        case PodcastSortDirection.oldestSubscribed:
          final aDate = a.subscribedDate ?? DateTime(1970);
          final bDate = b.subscribedDate ?? DateTime(1970);
          return aDate.compareTo(bDate);
      }
    });

    _filteredPodcasts = filtered;
  }

  Future<void> _loadPodcasts() async {
    final settingsBloc = Provider.of<SettingsBloc>(context, listen: false);
    final settings = settingsBloc.currentSettings;

    if (settings.pinepodsServer == null ||
        settings.pinepodsApiKey == null ||
        settings.pinepodsUserId == null) {
      setState(() {
        _errorMessage = 'Not connected to PinePods server. Please connect in Settings.';
        _isLoading = false;
      });
      return;
    }

    setState(() {
      _isLoading = true;
      _errorMessage = null;
    });

    try {
      // Initialize the service with the stored credentials
      _pinepodsService.setCredentials(
        settings.pinepodsServer!,
        settings.pinepodsApiKey!,
      );

      // Load podcasts with categories
      final result = await _pinepodsService.getUserPodcastsWithCategories(settings.pinepodsUserId!);
      final podcasts = result['podcasts'] as List<Podcast>;
      final categories = result['categories'] as Map<int, List<String>>;

      // Extract unique categories
      final allCategories = <String>{};
      // Older servers can return blank categories; never show an empty chip.
      for (final cats in categories.values) {
        allCategories.addAll(cats.map((c) => c.trim()).where((c) => c.isNotEmpty));
      }

      setState(() {
        _podcasts = podcasts;
        _podcastCategories = categories;
        _availableCategories = allCategories.toList()..sort();
        _filterPodcasts(); // Initialize filtered list
        _isLoading = false;
      });
    } catch (e) {
      setState(() {
        _errorMessage = e.toString();
        _isLoading = false;
      });
    }
  }

  List<Widget> _buildHeaderSlivers() {
    return [
      SliverToBoxAdapter(
        child: SoftSectionHeader(
          title: 'Podcasts',
          count: _filteredPodcasts?.length,
          trailing: [
            IconButton(
              tooltip: 'Layout',
              icon: const Icon(PhosphorIconsRegular.squaresFour),
              onPressed: () => showModalBottomSheet<void>(
                context: context,
                barrierLabel: L.of(context)!.scrim_layout_selector,
                builder: (context) => const LayoutSelectorWidget(),
              ),
            ),
          ],
        ),
      ),
      ListToolbar.sliver(
        context,
        ListToolbar<PodcastSortDirection>(
          searchController: _searchController,
          searchHint: 'Search podcasts',
          filters: [
            ToolbarFilter(
              label: 'All',
              selected: _selectedCategory == null,
              onTap: () => _setCategory(null),
            ),
            for (final category in _availableCategories)
              ToolbarFilter(
                label: category,
                selected: _selectedCategory == category,
                onTap: () => _setCategory(_selectedCategory == category ? null : category),
              ),
          ],
          sortOptions: const {
            PodcastSortDirection.titleAZ: 'Title A–Z',
            PodcastSortDirection.titleZA: 'Title Z–A',
            PodcastSortDirection.mostEpisodes: 'Most episodes',
            PodcastSortDirection.leastEpisodes: 'Fewest episodes',
            PodcastSortDirection.newestSubscribed: 'Recently followed',
            PodcastSortDirection.oldestSubscribed: 'First followed',
          },
          sortValue: _sortDirection,
          onSort: _setSortDirection,
        ),
      ),
    ];
  }

  Widget _buildPodcastList(AppSettings settings) {
    final podcasts = _filteredPodcasts ?? [];
    final hasActiveFilters = _searchQuery.isNotEmpty || _selectedCategory != null;

    if (podcasts.isEmpty && hasActiveFilters) {
      return SliverToBoxAdapter(
        child: SoftEmptyState(
          icon: PhosphorIconsRegular.funnelSimpleX,
          title: 'No podcasts found',
          body: _getNoResultsMessage(),
          action: TextButton(onPressed: _clearAllFilters, child: const Text('Clear filters')),
        ),
      );
    }

    var mode = settings.layout;
    var size = mode == 1 ? 100.0 : 160.0;

    if (mode == 0) {
      // List view
      return SliverList(
        delegate: SliverChildBuilderDelegate(
          (BuildContext context, int index) {
            return PinepodsPodcastTile(podcast: podcasts[index]);
          },
          childCount: podcasts.length,
          addAutomaticKeepAlives: false,
        ),
      );
    }

    // Grid view
    return SliverGrid(
      gridDelegate: SliverGridDelegateWithMaxCrossAxisExtent(
        maxCrossAxisExtent: size,
        mainAxisSpacing: 10.0,
        crossAxisSpacing: 10.0,
      ),
      delegate: SliverChildBuilderDelegate(
        (BuildContext context, int index) {
          return PinepodsPodcastGridTile(podcast: podcasts[index]);
        },
        childCount: podcasts.length,
      ),
    );
  }

  @override
  Widget build(BuildContext context) {
    final settingsBloc = Provider.of<SettingsBloc>(context);

    if (_isLoading) {
      return const SliverFillRemaining(
        hasScrollBody: false,
        child: Column(
          mainAxisAlignment: MainAxisAlignment.center,
          crossAxisAlignment: CrossAxisAlignment.center,
          children: <Widget>[
            PlatformProgressIndicator(),
          ],
        ),
      );
    }

    if (_errorMessage != null) {
      return SliverServerErrorPage(
        errorMessage: _errorMessage!.isServerConnectionError 
          ? null 
          : _errorMessage,
        onRetry: _loadPodcasts,
        title: 'Podcasts Unavailable',
        subtitle: _errorMessage!.isServerConnectionError
          ? 'Unable to connect to the PinePods server'
          : 'Failed to load your podcasts',
      );
    }

    if (_podcasts == null || _podcasts!.isEmpty) {
      return SliverFillRemaining(
        hasScrollBody: false,
        child: Padding(
          padding: const EdgeInsets.all(32.0),
          child: Column(
            mainAxisAlignment: MainAxisAlignment.center,
            crossAxisAlignment: CrossAxisAlignment.center,
            children: <Widget>[
              Icon(
                PhosphorIconsRegular.microphoneStage,
                size: 75,
                color: Theme.of(context).primaryColor,
              ),
              const SizedBox(height: 16),
              Text(
                'No podcasts found',
                style: Theme.of(context).textTheme.titleLarge,
                textAlign: TextAlign.center,
              ),
              const SizedBox(height: 8),
              Text(
                'You haven\'t subscribed to any podcasts yet. Search for podcasts to get started!',
                style: Theme.of(context).textTheme.bodyMedium,
                textAlign: TextAlign.center,
              ),
            ],
          ),
        ),
      );
    }

    return StreamBuilder<AppSettings>(
      stream: settingsBloc.settings,
      builder: (context, settingsSnapshot) {
        if (settingsSnapshot.hasData) {
          return MultiSliver(
            children: [
              ..._buildHeaderSlivers(),
              _buildPodcastList(settingsSnapshot.data!),
            ],
          );
        } else {
          return const SliverFillRemaining(
            hasScrollBody: false,
            child: SizedBox(
              height: 0,
              width: 0,
            ),
          );
        }
      },
    );
  }

  String _getNoResultsMessage() {
    final parts = <String>[];

    if (_searchQuery.isNotEmpty) {
      parts.add('matching "$_searchQuery"');
    }

    if (_selectedCategory != null) {
      parts.add('in category "$_selectedCategory"');
    }

    if (parts.isEmpty) {
      return 'No podcasts match your filters';
    }

    return 'No podcasts ${parts.join(' ')}';
  }
}