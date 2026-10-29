// lib/ui/pinepods/search.dart

import 'dart:io';
import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:pinepods_mobile/bloc/settings/settings_bloc.dart';
import 'package:pinepods_mobile/entities/pinepods_search.dart';
import 'package:pinepods_mobile/entities/youtube_channel.dart';
import 'package:pinepods_mobile/services/pinepods/pinepods_service.dart';
import 'package:pinepods_mobile/services/search_history_service.dart';
import 'package:pinepods_mobile/ui/pinepods/podcast_details.dart';
import 'package:pinepods_mobile/ui/pinepods/youtube_channel.dart';
import 'package:pinepods_mobile/ui/widgets/platform_progress_indicator.dart';
import 'package:pinepods_mobile/ui/widgets/server_error_page.dart';
import 'package:pinepods_mobile/services/error_handling_service.dart';
import 'package:provider/provider.dart';
import 'package:pinepods_mobile/ui/icons/phosphor_icons.dart';
import 'package:pinepods_mobile/ui/widgets/lazy_network_image.dart';
import 'package:pinepods_mobile/ui/widgets/soft_ui.dart';

class PinepodsSearch extends StatefulWidget {
  final String? searchTerm;

  const PinepodsSearch({
    super.key,
    this.searchTerm,
  });

  @override
  State<PinepodsSearch> createState() => _PinepodsSearchState();
}

class _PinepodsSearchState extends State<PinepodsSearch> {
  late TextEditingController _searchController;
  late FocusNode _searchFocusNode;
  final PinepodsService _pinepodsService = PinepodsService();
  final SearchHistoryService _searchHistoryService = SearchHistoryService();

  SearchProvider _selectedProvider = SearchProvider.podcastIndex;
  bool _isLoading = false;
  bool _showHistory = false;
  String? _errorMessage;
  List<UnifiedPinepodsPodcast> _searchResults = [];
  List<YouTubeChannelResult> _channelResults = [];
  // Feed URL to podcast ID of each subscribed channel in [_channelResults].
  final Map<String, int> _subscribedChannels = {};
  final Set<String> _channelsBusy = {};
  List<String> _searchHistory = [];
  Set<String> _addedPodcastUrls = {};

  @override
  void initState() {
    super.initState();
    
    _searchFocusNode = FocusNode();
    _searchController = TextEditingController();

    if (widget.searchTerm != null) {
      _searchController.text = widget.searchTerm!;
      _performSearch(widget.searchTerm!);
    } else {
      _loadSearchHistory();
    }

    _initializeCredentials();
    _searchController.addListener(_onSearchChanged);
  }

  void _initializeCredentials() {
    final settingsBloc = Provider.of<SettingsBloc>(context, listen: false);
    final settings = settingsBloc.currentSettings;
    
    if (settings.pinepodsServer != null && settings.pinepodsApiKey != null) {
      _pinepodsService.setCredentials(
        settings.pinepodsServer!,
        settings.pinepodsApiKey!,
      );
    }
  }

  @override
  void dispose() {
    _searchFocusNode.dispose();
    _searchController.dispose();
    super.dispose();
  }

  Future<void> _loadSearchHistory() async {
    final history = await _searchHistoryService.getPodcastSearchHistory();
    if (mounted) {
      setState(() {
        _searchHistory = history;
        _showHistory = _searchController.text.isEmpty && history.isNotEmpty;
      });
    }
  }

  void _onSearchChanged() {
    final query = _searchController.text.trim();
    setState(() {
      _showHistory = query.isEmpty && _searchHistory.isNotEmpty;
    });
  }

  bool get _isYoutube => _selectedProvider == SearchProvider.youtube;

  bool get _hasResults => _isYoutube ? _channelResults.isNotEmpty : _searchResults.isNotEmpty;

  void _clearResults() {
    _searchResults = [];
    _channelResults = [];
  }

  void _selectHistoryItem(String searchTerm) {
    _searchController.text = searchTerm;
    _performSearch(searchTerm);
  }

  Future<void> _removeHistoryItem(String searchTerm) async {
    await _searchHistoryService.removePodcastSearchTerm(searchTerm);
    await _loadSearchHistory();
  }

  Future<void> _performSearch(String query) async {
    if (query.trim().isEmpty) {
      setState(() {
        _clearResults();
        _errorMessage = null;
        _showHistory = _searchHistory.isNotEmpty;
      });
      return;
    }

    setState(() {
      _isLoading = true;
      _errorMessage = null;
      _showHistory = false;
    });

    // Save search term to history
    await _searchHistoryService.addPodcastSearchTerm(query);
    await _loadSearchHistory();

    final provider = _selectedProvider;
    try {
      if (provider == SearchProvider.youtube) {
        final channels = await _pinepodsService.searchYouTubeChannels(query);
        if (!mounted || provider != _selectedProvider) return;
        setState(() {
          _clearResults();
          _channelResults = channels;
          _isLoading = false;
        });
        await _checkSubscribedChannels();
        return;
      }

      final result = await _pinepodsService.searchPodcasts(query, provider);
      final podcasts = result.getUnifiedPodcasts();
      if (!mounted || provider != _selectedProvider) return;

      setState(() {
        _clearResults();
        _searchResults = podcasts;
        _isLoading = false;
      });

      // Check which podcasts are already added
      await _checkAddedPodcasts();
    } catch (e) {
      if (!mounted || provider != _selectedProvider) return;
      setState(() {
        _errorMessage = 'Search failed: $e';
        _isLoading = false;
        _clearResults();
      });
    }
  }

  Future<void> _checkSubscribedChannels() async {
    final userId = Provider.of<SettingsBloc>(context, listen: false).currentSettings.pinepodsUserId;
    if (userId == null) return;
    try {
      final followed = await _pinepodsService.getFollowedFeedIds(userId);
      if (!mounted) return;
      setState(() {
        _subscribedChannels.clear();
        for (final channel in _channelResults) {
          final id = followed[channel.url];
          if (id != null) _subscribedChannels[channel.url] = id;
        }
      });
    } catch (e) {
      debugPrint('Failed to check subscribed channels: $e');
    }
  }

  void _setChannelSubscription(YouTubeChannelResult channel, int? podcastId) {
    setState(() {
      if (podcastId == null) {
        _subscribedChannels.remove(channel.url);
      } else {
        _subscribedChannels[channel.url] = podcastId;
      }
    });
  }

  Future<void> _toggleChannel(YouTubeChannelResult channel) async {
    if (_channelsBusy.contains(channel.url)) return;
    setState(() => _channelsBusy.add(channel.url));
    if (_subscribedChannels.containsKey(channel.url)) {
      if (await unsubscribeFromChannel(context, channel) && mounted) _setChannelSubscription(channel, null);
    } else {
      final id = await subscribeToChannel(context, channel);
      if (id != null && mounted) _setChannelSubscription(channel, id);
    }
    if (mounted) setState(() => _channelsBusy.remove(channel.url));
  }

  Future<void> _checkAddedPodcasts() async {
    final settingsBloc = Provider.of<SettingsBloc>(context, listen: false);
    final settings = settingsBloc.currentSettings;
    final userId = settings.pinepodsUserId;

    if (userId == null) return;

    for (final podcast in _searchResults) {
      try {
        final exists = await _pinepodsService.checkPodcastExists(
          podcast.title,
          podcast.url,
          userId,
        );
        if (exists) {
          setState(() {
            _addedPodcastUrls.add(podcast.url);
          });
        }
      } catch (e) {
        // Ignore individual check failures
        print('Failed to check podcast ${podcast.title}: $e');
      }
    }
  }

  Future<void> _togglePodcast(UnifiedPinepodsPodcast podcast) async {
    final settingsBloc = Provider.of<SettingsBloc>(context, listen: false);
    final settings = settingsBloc.currentSettings;
    final userId = settings.pinepodsUserId;

    if (userId == null) {
      _showSnackBar('Not logged in to PinePods server', Colors.red);
      return;
    }

    final isAdded = _addedPodcastUrls.contains(podcast.url);

    try {
      bool success;
      if (isAdded) {
        success = await _pinepodsService.removePodcast(
          podcast.title,
          podcast.url,
          userId,
        );
        if (success) {
          setState(() {
            _addedPodcastUrls.remove(podcast.url);
          });
          _showSnackBar('Podcast removed', Colors.orange);
        }
      } else {
        success = await _pinepodsService.addPodcast(podcast, userId);
        if (success) {
          setState(() {
            _addedPodcastUrls.add(podcast.url);
          });
          _showSnackBar('Podcast added', Colors.green);
        }
      }

      if (!success) {
        _showSnackBar('Failed to ${isAdded ? 'remove' : 'add'} podcast', Colors.red);
      }
    } catch (e) {
      _showSnackBar('Error: $e', Colors.red);
    }
  }

  void _showSnackBar(String message, Color backgroundColor) {
    ScaffoldMessenger.of(context).showSnackBar(
      SnackBar(
        content: Text(message),
        backgroundColor: backgroundColor,
        duration: const Duration(seconds: 2),
      ),
    );
  }

  Widget _buildSearchHistorySliver() {
    if (_searchHistory.isEmpty) {
      return const SliverFillRemaining(
        hasScrollBody: false,
        child: Center(
          child: SoftEmptyState(
            icon: PhosphorIconsRegular.magnifyingGlass,
            title: 'Search for podcasts',
            body: 'Find new shows to follow by name, topic or host',
          ),
        ),
      );
    }

    final theme = Theme.of(context);
    return SliverList(
      delegate: SliverChildListDelegate([
        SoftSectionHeader(
          title: 'Recent searches',
          trailing: [
            TextButton(
              onPressed: () async {
                await _searchHistoryService.clearPodcastSearchHistory();
                await _loadSearchHistory();
              },
              child: const Text('Clear'),
            ),
          ],
        ),
        for (final searchTerm in _searchHistory.take(10))
          ListTile(
            contentPadding: const EdgeInsets.only(left: 16, right: 4),
            leading: Icon(PhosphorIconsRegular.clockCounterClockwise, color: theme.hintColor, size: 20),
            title: Text(searchTerm, maxLines: 1, overflow: TextOverflow.ellipsis),
            trailing: IconButton(
              tooltip: 'Remove',
              icon: Icon(PhosphorIconsRegular.x, size: 18, color: theme.hintColor),
              onPressed: () => _removeHistoryItem(searchTerm),
            ),
            onTap: () => _selectHistoryItem(searchTerm),
          ),
      ]),
    );
  }

  Widget _buildPodcastCard(UnifiedPinepodsPodcast podcast) {
    final theme = Theme.of(context);
    final isAdded = _addedPodcastUrls.contains(podcast.url);
    final meta = [
      if (podcast.episodeCount > 0) '${podcast.episodeCount} episode${podcast.episodeCount != 1 ? 's' : ''}',
      if (podcast.explicit) 'Explicit',
    ].join('  ·  ');

    return Card(
      margin: const EdgeInsets.symmetric(horizontal: 12.0, vertical: 4.0),
      child: InkWell(
        onTap: () {
          Navigator.push(
            context,
            MaterialPageRoute(
              builder: (context) => PinepodsPodcastDetails(
                podcast: podcast,
                isFollowing: isAdded,
                onFollowChanged: (following) {
                  setState(() {
                    if (following) {
                      _addedPodcastUrls.add(podcast.url);
                    } else {
                      _addedPodcastUrls.remove(podcast.url);
                    }
                  });
                },
              ),
            ),
          );
        },
        child: Padding(
          padding: const EdgeInsets.fromLTRB(10, 10, 10, 10),
          child: Row(
            crossAxisAlignment: CrossAxisAlignment.start,
            children: [
              LazyNetworkImage(
                imageUrl: podcast.artwork,
                width: 72,
                height: 72,
                borderRadius: BorderRadius.circular(12),
              ),
              const SizedBox(width: 12),
              Expanded(
                child: Column(
                  crossAxisAlignment: CrossAxisAlignment.start,
                  children: [
                    Text(
                      podcast.title,
                      style: const TextStyle(fontSize: 15, fontWeight: FontWeight.w600, height: 1.25),
                      maxLines: 2,
                      overflow: TextOverflow.ellipsis,
                    ),
                    if (podcast.author.isNotEmpty) ...[
                      const SizedBox(height: 2),
                      Text(
                        podcast.author,
                        style: TextStyle(fontSize: 12.5, fontWeight: FontWeight.w600, color: theme.primaryColor),
                        maxLines: 1,
                        overflow: TextOverflow.ellipsis,
                      ),
                    ],
                    if (podcast.description.isNotEmpty) ...[
                      const SizedBox(height: 4),
                      Text(
                        podcast.description,
                        style: TextStyle(fontSize: 12.5, color: theme.hintColor, height: 1.3),
                        maxLines: 2,
                        overflow: TextOverflow.ellipsis,
                      ),
                    ],
                    if (meta.isNotEmpty) ...[
                      const SizedBox(height: 4),
                      Text(meta, style: TextStyle(fontSize: 12, color: theme.hintColor)),
                    ],
                  ],
                ),
              ),
              const SizedBox(width: 8),
              CircleAction(
                size: 40,
                tooltip: isAdded ? 'Unfollow' : 'Follow',
                onTap: () => _togglePodcast(podcast),
                icon: Icon(
                  isAdded ? PhosphorIconsBold.check : PhosphorIconsBold.plus,
                  size: 18,
                  color: isAdded ? theme.primaryColor : softNeutral(theme).withOpacity(0.8),
                ),
              ),
            ],
          ),
        ),
      ),
    );
  }

  Widget _buildChannelCard(YouTubeChannelResult channel) {
    final theme = Theme.of(context);
    final podcastId = _subscribedChannels[channel.url];
    final subscribed = podcastId != null;
    final busy = _channelsBusy.contains(channel.url);
    final videos = channel.recentVideos.length;

    return Card(
      margin: const EdgeInsets.symmetric(horizontal: 12.0, vertical: 4.0),
      child: InkWell(
        onTap: () {
          Navigator.push(
            context,
            MaterialPageRoute(
              builder: (context) => YouTubeChannelPage(
                channel: channel,
                podcastId: podcastId,
                onSubscriptionChanged: (id) => _setChannelSubscription(channel, id),
              ),
            ),
          );
        },
        child: Padding(
          padding: const EdgeInsets.all(10),
          child: Row(
            crossAxisAlignment: CrossAxisAlignment.start,
            children: [
              ClipOval(child: LazyNetworkImage(imageUrl: channel.thumbnailUrl, width: 72, height: 72)),
              const SizedBox(width: 12),
              Expanded(
                child: Column(
                  crossAxisAlignment: CrossAxisAlignment.start,
                  children: [
                    Text(
                      channel.name,
                      style: const TextStyle(fontSize: 15, fontWeight: FontWeight.w600, height: 1.25),
                      maxLines: 2,
                      overflow: TextOverflow.ellipsis,
                    ),
                    if (channel.description.isNotEmpty) ...[
                      const SizedBox(height: 4),
                      Text(
                        channel.description,
                        style: TextStyle(fontSize: 12.5, color: theme.hintColor, height: 1.3),
                        maxLines: 2,
                        overflow: TextOverflow.ellipsis,
                      ),
                    ],
                    if (videos > 0) ...[
                      const SizedBox(height: 4),
                      Text(
                        'Latest: ${channel.recentVideos.first.title}',
                        style: TextStyle(fontSize: 12, color: theme.hintColor),
                        maxLines: 1,
                        overflow: TextOverflow.ellipsis,
                      ),
                    ],
                  ],
                ),
              ),
              const SizedBox(width: 8),
              CircleAction(
                size: 40,
                tooltip: subscribed ? 'Unsubscribe' : 'Subscribe',
                onTap: busy ? null : () => _toggleChannel(channel),
                icon: busy
                    ? SizedBox(
                        width: 16,
                        height: 16,
                        child: CircularProgressIndicator(strokeWidth: 2, color: theme.primaryColor),
                      )
                    : Icon(
                        subscribed ? PhosphorIconsBold.check : PhosphorIconsBold.plus,
                        size: 18,
                        color: subscribed ? theme.primaryColor : softNeutral(theme).withOpacity(0.8),
                      ),
              ),
            ],
          ),
        ),
      ),
    );
  }

  @override
  Widget build(BuildContext context) {
    final theme = Theme.of(context);
    return Scaffold(
      body: CustomScrollView(
        slivers: <Widget>[
          SliverAppBar(
            leading: IconButton(
              tooltip: 'Back',
              icon: Platform.isAndroid
                  ? Icon(PhosphorIconsRegular.arrowLeft, color: theme.appBarTheme.foregroundColor)
                  : const Icon(PhosphorIconsRegular.caretLeft),
              onPressed: () => Navigator.pop(context),
            ),
            titleSpacing: 0,
            title: Padding(
              padding: const EdgeInsets.only(right: 16),
              child: TextField(
                controller: _searchController,
                focusNode: _searchFocusNode,
                autofocus: widget.searchTerm != null ? false : true,
                keyboardType: TextInputType.text,
                textInputAction: TextInputAction.search,
                onTap: () {
                  setState(() {
                    _showHistory = _searchController.text.isEmpty && _searchHistory.isNotEmpty;
                  });
                },
                decoration: InputDecoration(
                  hintText: _isYoutube ? 'Search YouTube channels' : 'Search podcasts',
                  isDense: true,
                  contentPadding: const EdgeInsets.symmetric(vertical: 10),
                  border: OutlineInputBorder(borderRadius: BorderRadius.circular(22), borderSide: BorderSide.none),
                  enabledBorder:
                      OutlineInputBorder(borderRadius: BorderRadius.circular(22), borderSide: BorderSide.none),
                  focusedBorder:
                      OutlineInputBorder(borderRadius: BorderRadius.circular(22), borderSide: BorderSide.none),
                  prefixIcon: const Icon(PhosphorIconsRegular.magnifyingGlass, size: 20),
                  suffixIcon: ValueListenableBuilder<TextEditingValue>(
                    valueListenable: _searchController,
                    builder: (context, value, _) => value.text.isEmpty
                        ? const SizedBox.shrink()
                        : IconButton(
                            tooltip: 'Clear search',
                            icon: const Icon(PhosphorIconsRegular.x, size: 18),
                            onPressed: () {
                              _searchController.clear();
                              setState(() {
                                _clearResults();
                                _errorMessage = null;
                                _showHistory = _searchHistory.isNotEmpty;
                              });
                              FocusScope.of(context).requestFocus(_searchFocusNode);
                              SystemChannels.textInput.invokeMethod<String>('TextInput.show');
                            },
                          ),
                  ),
                ),
                style: const TextStyle(fontSize: 16),
                onSubmitted: _performSearch,
              ),
            ),
            floating: false,
            pinned: true,
            snap: false,
            bottom: PreferredSize(
              preferredSize: const Size.fromHeight(52),
              child: SizedBox(
                height: 52,
                child: ListView(
                  scrollDirection: Axis.horizontal,
                  padding: const EdgeInsets.fromLTRB(16, 8, 16, 10),
                  children: [
                    for (final provider in SearchProvider.values)
                      Padding(
                        padding: const EdgeInsets.only(right: 8),
                        child: FilterPill(
                          label: provider.name,
                          icon: provider == SearchProvider.youtube ? PhosphorIconsFill.youtubeLogo : null,
                          selected: provider == _selectedProvider,
                          onTap: () {
                            if (provider == _selectedProvider) return;
                            setState(() {
                              _selectedProvider = provider;
                              _clearResults();
                            });
                            // Re-search with the new provider if there's a current search
                            if (_searchController.text.isNotEmpty) {
                              _performSearch(_searchController.text);
                            }
                          },
                        ),
                      ),
                  ],
                ),
              ),
            ),
          ),

          // Search results or history
          if (_showHistory)
            _buildSearchHistorySliver()
          else if (_isLoading)
            const SliverFillRemaining(
              hasScrollBody: false,
              child: Center(child: PlatformProgressIndicator()),
            )
          else if (_errorMessage != null)
            SliverServerErrorPage(
              errorMessage: _errorMessage!.isServerConnectionError
                ? null
                : _errorMessage,
              onRetry: () => _performSearch(_searchController.text),
              title: 'Search Unavailable',
              subtitle: _errorMessage!.isServerConnectionError
                ? 'Unable to connect to the PinePods server'
                : _isYoutube
                  ? 'Failed to search YouTube'
                  : 'Failed to search for podcasts',
            )
          else if (!_hasResults && _searchController.text.isNotEmpty)
            SliverFillRemaining(
              hasScrollBody: false,
              child: Center(
                child: SoftEmptyState(
                  icon: PhosphorIconsRegular.magnifyingGlassMinus,
                  title: _isYoutube ? 'No channels found' : 'No podcasts found',
                  body: 'Try different keywords or another search provider',
                ),
              ),
            )
          else if (!_hasResults)
            _buildSearchHistorySliver()
          else if (_isYoutube)
            SliverList(
              delegate: SliverChildBuilderDelegate(
                (context, index) => _buildChannelCard(_channelResults[index]),
                childCount: _channelResults.length,
              ),
            )
          else
            SliverList(
              delegate: SliverChildBuilderDelegate(
                (context, index) {
                  return _buildPodcastCard(_searchResults[index]);
                },
                childCount: _searchResults.length,
              ),
            ),
        ],
      ),
    );
  }
}
