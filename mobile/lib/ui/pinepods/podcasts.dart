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
      for (final cats in categories.values) {
        allCategories.addAll(cats);
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

  Widget _buildSearchAndFilterBar() {
    return SliverToBoxAdapter(
      child: Padding(
        padding: const EdgeInsets.all(16.0),
        child: Column(
          crossAxisAlignment: CrossAxisAlignment.start,
          children: [
            // Search, Sort, and Layout row
            Row(
              children: [
                // Search field
                Expanded(
                  child: TextField(
                    controller: _searchController,
                    decoration: InputDecoration(
                      hintText: 'Search podcasts...',
                      prefixIcon: const Icon(Icons.search),
                      suffixIcon: _searchQuery.isNotEmpty
                          ? IconButton(
                              icon: const Icon(Icons.clear),
                              onPressed: () {
                                _searchController.clear();
                              },
                            )
                          : null,
                      border: OutlineInputBorder(
                        borderRadius: BorderRadius.circular(12),
                      ),
                      filled: true,
                      fillColor: Theme.of(context).cardColor,
                      contentPadding: const EdgeInsets.symmetric(horizontal: 16, vertical: 12),
                    ),
                  ),
                ),
                const SizedBox(width: 8),
                // Sort dropdown
                Container(
                  decoration: BoxDecoration(
                    color: Theme.of(context).cardColor,
                    borderRadius: BorderRadius.circular(12),
                    border: Border.all(
                      color: Theme.of(context).dividerColor,
                    ),
                  ),
                  padding: const EdgeInsets.symmetric(horizontal: 8),
                  child: DropdownButtonHideUnderline(
                    child: DropdownButton<PodcastSortDirection>(
                      value: _sortDirection,
                      icon: const Icon(Icons.sort),
                      items: const [
                        DropdownMenuItem(
                          value: PodcastSortDirection.titleAZ,
                          child: Text('A-Z'),
                        ),
                        DropdownMenuItem(
                          value: PodcastSortDirection.titleZA,
                          child: Text('Z-A'),
                        ),
                        DropdownMenuItem(
                          value: PodcastSortDirection.mostEpisodes,
                          child: Text('Most Eps'),
                        ),
                        DropdownMenuItem(
                          value: PodcastSortDirection.leastEpisodes,
                          child: Text('Least Eps'),
                        ),
                        DropdownMenuItem(
                          value: PodcastSortDirection.newestSubscribed,
                          child: Text('Newest'),
                        ),
                        DropdownMenuItem(
                          value: PodcastSortDirection.oldestSubscribed,
                          child: Text('Oldest'),
                        ),
                      ],
                      onChanged: (value) {
                        if (value != null) {
                          _setSortDirection(value);
                        }
                      },
                    ),
                  ),
                ),
                const SizedBox(width: 8),
                // Layout selector button
                Material(
                  color: Theme.of(context).cardColor,
                  borderRadius: BorderRadius.circular(12),
                  child: InkWell(
                    borderRadius: BorderRadius.circular(12),
                    onTap: () async {
                      await showModalBottomSheet<void>(
                        context: context,
                        backgroundColor: Theme.of(context).secondaryHeaderColor,
                        barrierLabel: L.of(context)!.scrim_layout_selector,
                        shape: const RoundedRectangleBorder(
                          borderRadius: BorderRadius.only(
                            topLeft: Radius.circular(16.0),
                            topRight: Radius.circular(16.0),
                          ),
                        ),
                        builder: (context) => const LayoutSelectorWidget(),
                      );
                    },
                    child: Container(
                      width: 48,
                      height: 48,
                      decoration: BoxDecoration(
                        border: Border.all(
                          color: Theme.of(context).dividerColor,
                        ),
                        borderRadius: BorderRadius.circular(12),
                      ),
                      child: const Icon(
                        Icons.dashboard,
                        size: 20,
                      ),
                    ),
                  ),
                ),
              ],
            ),
            const SizedBox(height: 12),
            // Filter chips
            SingleChildScrollView(
              scrollDirection: Axis.horizontal,
              child: Row(
                children: [
                  // Clear all chip
                  _buildFilterChip(
                    label: 'Clear All',
                    icon: Icons.clear_all,
                    isActive: false,
                    onTap: _clearAllFilters,
                  ),
                  const SizedBox(width: 8),
                  // Category chips
                  ..._availableCategories.map((category) => Padding(
                    padding: const EdgeInsets.only(right: 8),
                    child: _buildFilterChip(
                      label: category,
                      icon: Icons.category,
                      isActive: _selectedCategory == category,
                      onTap: () {
                        _setCategory(_selectedCategory == category ? null : category);
                      },
                    ),
                  )),
                ],
              ),
            ),
          ],
        ),
      ),
    );
  }

  Widget _buildFilterChip({
    required String label,
    required IconData icon,
    required bool isActive,
    required VoidCallback onTap,
  }) {
    final theme = Theme.of(context);
    return Material(
      color: isActive ? theme.primaryColor : theme.cardColor,
      borderRadius: BorderRadius.circular(20),
      child: InkWell(
        onTap: onTap,
        borderRadius: BorderRadius.circular(20),
        child: Container(
          padding: const EdgeInsets.symmetric(horizontal: 12, vertical: 8),
          decoration: BoxDecoration(
            borderRadius: BorderRadius.circular(20),
            border: Border.all(
              color: isActive ? theme.primaryColor : theme.dividerColor,
            ),
          ),
          child: Row(
            mainAxisSize: MainAxisSize.min,
            children: [
              Icon(
                icon,
                size: 18,
                color: isActive ? Colors.white : theme.iconTheme.color,
              ),
              const SizedBox(width: 6),
              Text(
                label,
                style: TextStyle(
                  color: isActive ? Colors.white : theme.textTheme.bodyMedium?.color,
                  fontWeight: FontWeight.w500,
                ),
              ),
            ],
          ),
        ),
      ),
    );
  }

  Widget _buildPodcastList(AppSettings settings) {
    final podcasts = _filteredPodcasts ?? [];
    final hasActiveFilters = _searchQuery.isNotEmpty || _selectedCategory != null;

    if (podcasts.isEmpty && hasActiveFilters) {
      return SliverFillRemaining(
        hasScrollBody: false,
        child: Padding(
          padding: const EdgeInsets.all(32.0),
          child: Column(
            mainAxisAlignment: MainAxisAlignment.center,
            crossAxisAlignment: CrossAxisAlignment.center,
            children: <Widget>[
              Icon(
                Icons.filter_list_off,
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
                _getNoResultsMessage(),
                style: Theme.of(context).textTheme.bodyMedium,
                textAlign: TextAlign.center,
              ),
              const SizedBox(height: 16),
              TextButton.icon(
                onPressed: _clearAllFilters,
                icon: const Icon(Icons.clear_all),
                label: const Text('Clear Filters'),
              ),
            ],
          ),
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
                Icons.podcasts,
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
              _buildSearchAndFilterBar(),
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