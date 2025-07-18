// lib/ui/pinepods/search.dart

import 'dart:io';
import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:pinepods_mobile/bloc/settings/settings_bloc.dart';
import 'package:pinepods_mobile/entities/pinepods_search.dart';
import 'package:pinepods_mobile/services/pinepods/pinepods_service.dart';
import 'package:pinepods_mobile/ui/pinepods/podcast_details.dart';
import 'package:pinepods_mobile/ui/widgets/platform_progress_indicator.dart';
import 'package:provider/provider.dart';

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

  SearchProvider _selectedProvider = SearchProvider.podcastIndex;
  bool _isLoading = false;
  String? _errorMessage;
  List<UnifiedPinepodsPodcast> _searchResults = [];
  Set<String> _addedPodcastUrls = {};

  @override
  void initState() {
    super.initState();
    
    _searchFocusNode = FocusNode();
    _searchController = TextEditingController();

    if (widget.searchTerm != null) {
      _searchController.text = widget.searchTerm!;
      _performSearch(widget.searchTerm!);
    }

    _initializeCredentials();
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

  Future<void> _performSearch(String query) async {
    if (query.trim().isEmpty) {
      setState(() {
        _searchResults = [];
        _errorMessage = null;
      });
      return;
    }

    setState(() {
      _isLoading = true;
      _errorMessage = null;
    });

    try {
      final result = await _pinepodsService.searchPodcasts(query, _selectedProvider);
      final podcasts = result.getUnifiedPodcasts();
      
      setState(() {
        _searchResults = podcasts;
        _isLoading = false;
      });

      // Check which podcasts are already added
      await _checkAddedPodcasts();
    } catch (e) {
      setState(() {
        _errorMessage = 'Search failed: $e';
        _isLoading = false;
        _searchResults = [];
      });
    }
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

  Widget _buildPodcastCard(UnifiedPinepodsPodcast podcast) {
    final isAdded = _addedPodcastUrls.contains(podcast.url);
    
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
        child: Column(
          children: [
            // Podcast image and info
            Padding(
              padding: const EdgeInsets.all(12.0),
              child: Row(
                crossAxisAlignment: CrossAxisAlignment.start,
                children: [
                  // Podcast artwork
                  ClipRRect(
                    borderRadius: BorderRadius.circular(8),
                    child: podcast.artwork.isNotEmpty
                        ? Image.network(
                            podcast.artwork,
                            width: 80,
                            height: 80,
                            fit: BoxFit.cover,
                            errorBuilder: (context, error, stackTrace) {
                              return Container(
                                width: 80,
                                height: 80,
                                decoration: BoxDecoration(
                                  color: Colors.grey[300],
                                  borderRadius: BorderRadius.circular(8),
                                ),
                                child: const Icon(
                                  Icons.music_note,
                                  color: Colors.grey,
                                  size: 32,
                                ),
                              );
                            },
                          )
                        : Container(
                            width: 80,
                            height: 80,
                            decoration: BoxDecoration(
                              color: Colors.grey[300],
                              borderRadius: BorderRadius.circular(8),
                            ),
                            child: const Icon(
                              Icons.music_note,
                              color: Colors.grey,
                              size: 32,
                            ),
                          ),
                  ),
                  const SizedBox(width: 12),
                  
                  // Podcast info
                  Expanded(
                    child: Column(
                      crossAxisAlignment: CrossAxisAlignment.start,
                      children: [
                        Text(
                          podcast.title,
                          style: const TextStyle(
                            fontSize: 16,
                            fontWeight: FontWeight.bold,
                          ),
                          maxLines: 2,
                          overflow: TextOverflow.ellipsis,
                        ),
                        const SizedBox(height: 4),
                        if (podcast.author.isNotEmpty)
                          Text(
                            'By ${podcast.author}',
                            style: TextStyle(
                              fontSize: 14,
                              color: Theme.of(context).primaryColor,
                            ),
                            maxLines: 1,
                            overflow: TextOverflow.ellipsis,
                          ),
                        const SizedBox(height: 4),
                        Text(
                          podcast.description,
                          style: TextStyle(
                            fontSize: 12,
                            color: Colors.grey[600],
                          ),
                          maxLines: 3,
                          overflow: TextOverflow.ellipsis,
                        ),
                        const SizedBox(height: 8),
                        Row(
                          children: [
                            Icon(
                              Icons.mic,
                              size: 16,
                              color: Colors.grey[600],
                            ),
                            const SizedBox(width: 4),
                            Text(
                              '${podcast.episodeCount} episode${podcast.episodeCount != 1 ? 's' : ''}',
                              style: TextStyle(
                                fontSize: 12,
                                color: Colors.grey[600],
                              ),
                            ),
                            const SizedBox(width: 16),
                            if (podcast.explicit)
                              Container(
                                padding: const EdgeInsets.symmetric(
                                  horizontal: 4,
                                  vertical: 2,
                                ),
                                decoration: BoxDecoration(
                                  color: Colors.red,
                                  borderRadius: BorderRadius.circular(4),
                                ),
                                child: const Text(
                                  'E',
                                  style: TextStyle(
                                    color: Colors.white,
                                    fontSize: 10,
                                    fontWeight: FontWeight.bold,
                                  ),
                                ),
                              ),
                          ],
                        ),
                      ],
                    ),
                  ),
                  
                  // Follow/Unfollow button
                  IconButton(
                    onPressed: () => _togglePodcast(podcast),
                    icon: Icon(
                      isAdded ? Icons.remove_circle : Icons.add_circle,
                      color: isAdded ? Colors.red : Colors.green,
                    ),
                    tooltip: isAdded ? 'Remove podcast' : 'Add podcast',
                  ),
                ],
              ),
            ),
          ],
        ),
      ),
    );
  }

  @override
  Widget build(BuildContext context) {
    return Scaffold(
      body: CustomScrollView(
        slivers: <Widget>[
          SliverAppBar(
            leading: IconButton(
              tooltip: 'Back',
              icon: Platform.isAndroid
                  ? Icon(Icons.arrow_back, color: Theme.of(context).appBarTheme.foregroundColor)
                  : const Icon(Icons.arrow_back_ios),
              onPressed: () => Navigator.pop(context),
            ),
            title: TextField(
              controller: _searchController,
              focusNode: _searchFocusNode,
              autofocus: widget.searchTerm != null ? false : true,
              keyboardType: TextInputType.text,
              textInputAction: TextInputAction.search,
              decoration: const InputDecoration(
                hintText: 'Search for podcasts',
                border: InputBorder.none,
              ),
              style: TextStyle(
                color: Theme.of(context).primaryIconTheme.color,
                fontSize: 18.0,
                decorationColor: Theme.of(context).scaffoldBackgroundColor,
              ),
              onSubmitted: _performSearch,
            ),
            floating: false,
            pinned: true,
            snap: false,
            actions: <Widget>[
              IconButton(
                tooltip: 'Clear search',
                icon: const Icon(Icons.clear),
                onPressed: () {
                  _searchController.clear();
                  setState(() {
                    _searchResults = [];
                    _errorMessage = null;
                  });
                  FocusScope.of(context).requestFocus(_searchFocusNode);
                  SystemChannels.textInput.invokeMethod<String>('TextInput.show');
                },
              ),
            ],
            bottom: PreferredSize(
              preferredSize: const Size.fromHeight(60),
              child: Container(
                padding: const EdgeInsets.all(12.0),
                child: Row(
                  children: [
                    const Text(
                      'Search Provider: ',
                      style: TextStyle(
                        fontSize: 14,
                        fontWeight: FontWeight.w500,
                      ),
                    ),
                    Expanded(
                      child: DropdownButton<SearchProvider>(
                        value: _selectedProvider,
                        isExpanded: true,
                        items: SearchProvider.values.map((provider) {
                          return DropdownMenuItem(
                            value: provider,
                            child: Text(provider.name),
                          );
                        }).toList(),
                        onChanged: (provider) {
                          if (provider != null) {
                            setState(() {
                              _selectedProvider = provider;
                            });
                            // Re-search with new provider if there's a current search
                            if (_searchController.text.isNotEmpty) {
                              _performSearch(_searchController.text);
                            }
                          }
                        },
                      ),
                    ),
                  ],
                ),
              ),
            ),
          ),
          
          // Search results
          if (_isLoading)
            const SliverFillRemaining(
              hasScrollBody: false,
              child: Center(child: PlatformProgressIndicator()),
            )
          else if (_errorMessage != null)
            SliverFillRemaining(
              hasScrollBody: false,
              child: Center(
                child: Column(
                  mainAxisAlignment: MainAxisAlignment.center,
                  children: [
                    Icon(
                      Icons.error_outline,
                      size: 64,
                      color: Colors.red[300],
                    ),
                    const SizedBox(height: 16),
                    Text(
                      _errorMessage!,
                      textAlign: TextAlign.center,
                      style: Theme.of(context).textTheme.bodyLarge,
                    ),
                    const SizedBox(height: 16),
                    ElevatedButton(
                      onPressed: () => _performSearch(_searchController.text),
                      child: const Text('Retry'),
                    ),
                  ],
                ),
              ),
            )
          else if (_searchResults.isEmpty && _searchController.text.isNotEmpty)
            SliverFillRemaining(
              hasScrollBody: false,
              child: Center(
                child: Column(
                  mainAxisAlignment: MainAxisAlignment.center,
                  children: [
                    Icon(
                      Icons.search_off,
                      size: 64,
                      color: Colors.grey[400],
                    ),
                    const SizedBox(height: 16),
                    Text(
                      'No podcasts found',
                      style: Theme.of(context).textTheme.headlineSmall,
                    ),
                    const SizedBox(height: 8),
                    Text(
                      'Try searching with different keywords or switch search provider',
                      style: Theme.of(context).textTheme.bodyMedium,
                      textAlign: TextAlign.center,
                    ),
                  ],
                ),
              ),
            )
          else if (_searchResults.isEmpty)
            SliverFillRemaining(
              hasScrollBody: false,
              child: Center(
                child: Column(
                  mainAxisAlignment: MainAxisAlignment.center,
                  children: [
                    Icon(
                      Icons.search,
                      size: 64,
                      color: Colors.grey[400],
                    ),
                    const SizedBox(height: 16),
                    Text(
                      'Search for podcasts',
                      style: Theme.of(context).textTheme.headlineSmall,
                    ),
                    const SizedBox(height: 8),
                    Text(
                      'Enter a search term to find podcasts',
                      style: Theme.of(context).textTheme.bodyMedium,
                    ),
                  ],
                ),
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