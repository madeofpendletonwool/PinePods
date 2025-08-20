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
import 'package:sliver_tools/sliver_tools.dart';

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

  @override
  void initState() {
    super.initState();
    _loadPodcasts();
    _searchController.addListener(_onSearchChanged);
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

    if (_searchQuery.isEmpty) {
      _filteredPodcasts = List.from(_podcasts!);
    } else {
      _filteredPodcasts = _podcasts!.where((podcast) {
        return podcast.title.toLowerCase().contains(_searchQuery.toLowerCase());
      }).toList();
    }
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
      
      final podcasts = await _pinepodsService.getUserPodcasts(settings.pinepodsUserId!);
      
      setState(() {
        _podcasts = podcasts;
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

  Widget _buildSearchBar() {
    return SliverToBoxAdapter(
      child: Padding(
        padding: const EdgeInsets.all(16.0),
        child: Row(
          children: [
            Expanded(
              child: TextField(
                controller: _searchController,
                decoration: InputDecoration(
                  hintText: 'Filter podcasts...',
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
                ),
              ),
            ),
            const SizedBox(width: 12),
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
      ),
    );
  }

  Widget _buildPodcastList(AppSettings settings) {
    final podcasts = _filteredPodcasts ?? [];
    
    if (podcasts.isEmpty && _searchQuery.isNotEmpty) {
      return SliverFillRemaining(
        hasScrollBody: false,
        child: Padding(
          padding: const EdgeInsets.all(32.0),
          child: Column(
            mainAxisAlignment: MainAxisAlignment.center,
            crossAxisAlignment: CrossAxisAlignment.center,
            children: <Widget>[
              Icon(
                Icons.search_off,
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
                'No podcasts match "$_searchQuery"',
                style: Theme.of(context).textTheme.bodyMedium,
                textAlign: TextAlign.center,
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
              _buildSearchBar(),
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
}