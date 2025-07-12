import 'dart:async';
import 'package:flutter/material.dart';
import 'package:pinepods_mobile/bloc/settings/settings_bloc.dart';
import 'package:pinepods_mobile/entities/pinepods_episode.dart';
import 'package:pinepods_mobile/services/pinepods/pinepods_service.dart';
import 'package:pinepods_mobile/services/pinepods/pinepods_audio_service.dart';
import 'package:pinepods_mobile/services/audio/audio_player_service.dart';
import 'package:pinepods_mobile/ui/widgets/pinepods_episode_card.dart';
import 'package:pinepods_mobile/ui/widgets/episode_context_menu.dart';
import 'package:pinepods_mobile/ui/pinepods/episode_details.dart';
import 'package:provider/provider.dart';

/// Episode search page for finding episodes in user's subscriptions
/// 
/// This page allows users to search through episodes in their subscribed podcasts
/// with debounced search input and animated loading states.
class EpisodeSearchPage extends StatefulWidget {
  const EpisodeSearchPage({Key? key}) : super(key: key);

  @override
  State<EpisodeSearchPage> createState() => _EpisodeSearchPageState();
}

class _EpisodeSearchPageState extends State<EpisodeSearchPage> with TickerProviderStateMixin {
  final PinepodsService _pinepodsService = PinepodsService();
  final TextEditingController _searchController = TextEditingController();
  final FocusNode _focusNode = FocusNode();
  Timer? _debounceTimer;
  
  List<SearchEpisodeResult> _searchResults = [];
  bool _isLoading = false;
  bool _hasSearched = false;
  String? _errorMessage;
  String _currentQuery = '';
  
  // Audio service and context menu state
  PinepodsAudioService? _audioService;
  int? _contextMenuEpisodeIndex;

  // Animation controllers
  late AnimationController _fadeAnimationController;
  late AnimationController _slideAnimationController;
  late Animation<double> _fadeAnimation;
  late Animation<Offset> _slideAnimation;

  @override
  void initState() {
    super.initState();
    _setupAnimations();
    _setupSearch();
  }

  void _setupAnimations() {
    // Fade animation for results
    _fadeAnimationController = AnimationController(
      duration: const Duration(milliseconds: 500),
      vsync: this,
    );
    _fadeAnimation = Tween<double>(
      begin: 0.0,
      end: 1.0,
    ).animate(CurvedAnimation(
      parent: _fadeAnimationController,
      curve: Curves.easeInOut,
    ));

    // Slide animation for search bar
    _slideAnimationController = AnimationController(
      duration: const Duration(milliseconds: 300),
      vsync: this,
    );
    _slideAnimation = Tween<Offset>(
      begin: const Offset(0, 0),
      end: const Offset(0, -0.2),
    ).animate(CurvedAnimation(
      parent: _slideAnimationController,
      curve: Curves.easeInOut,
    ));
  }

  void _setupSearch() {
    final settingsBloc = Provider.of<SettingsBloc>(context, listen: false);
    final settings = settingsBloc.currentSettings;

    if (settings.pinepodsServer != null && settings.pinepodsApiKey != null) {
      _pinepodsService.setCredentials(
        settings.pinepodsServer!,
        settings.pinepodsApiKey!,
      );
    }

    _searchController.addListener(_onSearchChanged);
  }

  void _initializeAudioService() {
    if (_audioService != null) return; // Already initialized
    
    try {
      final audioPlayerService = Provider.of<AudioPlayerService>(context, listen: false);
      final settingsBloc = Provider.of<SettingsBloc>(context, listen: false);
      
      _audioService = PinepodsAudioService(
        audioPlayerService,
        _pinepodsService,
        settingsBloc,
      );
    } catch (e) {
      // Provider not available - audio service will remain null
    }
  }

  Future<void> _playEpisode(PinepodsEpisode episode) async {
    // Try to initialize audio service if not already done
    _initializeAudioService();
    
    if (_audioService == null) {
      ScaffoldMessenger.of(context).showSnackBar(
        const SnackBar(
          content: Text('Audio service not available'),
          backgroundColor: Colors.red,
        ),
      );
      return;
    }

    try {
      await _audioService!.playPinepodsEpisode(pinepodsEpisode: episode);
      
      if (mounted) {
        ScaffoldMessenger.of(context).showSnackBar(
          SnackBar(
            content: Text('Playing ${episode.episodeTitle}'),
            backgroundColor: Colors.green,
          ),
        );
      }
    } catch (e) {
      if (mounted) {
        ScaffoldMessenger.of(context).showSnackBar(
          SnackBar(
            content: Text('Failed to play episode: $e'),
            backgroundColor: Colors.red,
          ),
        );
      }
    }
  }

  void _showContextMenu(int episodeIndex) {
    setState(() {
      _contextMenuEpisodeIndex = episodeIndex;
    });
  }

  void _hideContextMenu() {
    setState(() {
      _contextMenuEpisodeIndex = null;
    });
  }

  Future<void> _saveEpisode(int episodeIndex) async {
    final episode = _searchResults[episodeIndex].toPinepodsEpisode();
    final settingsBloc = Provider.of<SettingsBloc>(context, listen: false);
    final settings = settingsBloc.currentSettings;
    final userId = settings.pinepodsUserId;

    if (userId == null) {
      _showSnackBar('Not logged in', Colors.red);
      return;
    }

    try {
      final success = await _pinepodsService.saveEpisode(
        episode.episodeId,
        userId,
        episode.isYoutube,
      );

      if (success && mounted) {
        _showSnackBar('Episode saved', Colors.green);
        // Update local state
        setState(() {
          _searchResults[episodeIndex] = SearchEpisodeResult(
            podcastId: _searchResults[episodeIndex].podcastId,
            podcastName: _searchResults[episodeIndex].podcastName,
            artworkUrl: _searchResults[episodeIndex].artworkUrl,
            author: _searchResults[episodeIndex].author,
            categories: _searchResults[episodeIndex].categories,
            description: _searchResults[episodeIndex].description,
            episodeCount: _searchResults[episodeIndex].episodeCount,
            feedUrl: _searchResults[episodeIndex].feedUrl,
            websiteUrl: _searchResults[episodeIndex].websiteUrl,
            explicit: _searchResults[episodeIndex].explicit,
            userId: _searchResults[episodeIndex].userId,
            episodeId: _searchResults[episodeIndex].episodeId,
            episodeTitle: _searchResults[episodeIndex].episodeTitle,
            episodeDescription: _searchResults[episodeIndex].episodeDescription,
            episodePubDate: _searchResults[episodeIndex].episodePubDate,
            episodeArtwork: _searchResults[episodeIndex].episodeArtwork,
            episodeUrl: _searchResults[episodeIndex].episodeUrl,
            episodeDuration: _searchResults[episodeIndex].episodeDuration,
            completed: _searchResults[episodeIndex].completed,
            saved: true, // We just saved it
            queued: _searchResults[episodeIndex].queued,
            downloaded: _searchResults[episodeIndex].downloaded,
            isYoutube: _searchResults[episodeIndex].isYoutube,
            listenDuration: _searchResults[episodeIndex].listenDuration,
          );
        });
      } else if (mounted) {
        _showSnackBar('Failed to save episode', Colors.red);
      }
    } catch (e) {
      if (mounted) {
        _showSnackBar('Error saving episode: $e', Colors.red);
      }
    }
  }

  Future<void> _removeSavedEpisode(int episodeIndex) async {
    final episode = _searchResults[episodeIndex].toPinepodsEpisode();
    final settingsBloc = Provider.of<SettingsBloc>(context, listen: false);
    final settings = settingsBloc.currentSettings;
    final userId = settings.pinepodsUserId;

    if (userId == null) {
      _showSnackBar('Not logged in', Colors.red);
      return;
    }

    try {
      final success = await _pinepodsService.removeSavedEpisode(
        episode.episodeId,
        userId,
        episode.isYoutube,
      );

      if (success && mounted) {
        _showSnackBar('Episode removed from saved', Colors.orange);
      } else if (mounted) {
        _showSnackBar('Failed to remove saved episode', Colors.red);
      }
    } catch (e) {
      if (mounted) {
        _showSnackBar('Error removing saved episode: $e', Colors.red);
      }
    }
  }

  Future<void> _downloadEpisode(int episodeIndex) async {
    final episode = _searchResults[episodeIndex].toPinepodsEpisode();
    _showSnackBar('Download started for ${episode.episodeTitle}', Colors.blue);
    // Note: Actual download implementation would depend on download service integration
  }

  Future<void> _deleteEpisode(int episodeIndex) async {
    final episode = _searchResults[episodeIndex].toPinepodsEpisode();
    _showSnackBar('Delete requested for ${episode.episodeTitle}', Colors.orange);
    // Note: Actual delete implementation would depend on download service integration
  }

  Future<void> _localDownloadEpisode(int episodeIndex) async {
    final episode = _searchResults[episodeIndex].toPinepodsEpisode();
    _showSnackBar('Local download started for ${episode.episodeTitle}', Colors.blue);
    // Note: Actual local download implementation would depend on download service integration
  }

  Future<void> _toggleQueueEpisode(int episodeIndex) async {
    final episode = _searchResults[episodeIndex].toPinepodsEpisode();
    final settingsBloc = Provider.of<SettingsBloc>(context, listen: false);
    final settings = settingsBloc.currentSettings;
    final userId = settings.pinepodsUserId;

    if (userId == null) {
      _showSnackBar('Not logged in', Colors.red);
      return;
    }

    try {
      if (episode.queued) {
        final success = await _pinepodsService.removeQueuedEpisode(
          episode.episodeId,
          userId,
          episode.isYoutube,
        );
        
        if (success && mounted) {
          _showSnackBar('Episode removed from queue', Colors.orange);
        }
      } else {
        final success = await _pinepodsService.queueEpisode(
          episode.episodeId,
          userId,
          episode.isYoutube,
        );
        
        if (success && mounted) {
          _showSnackBar('Episode added to queue', Colors.green);
        }
      }
    } catch (e) {
      if (mounted) {
        _showSnackBar('Error updating queue: $e', Colors.red);
      }
    }
  }

  Future<void> _toggleMarkComplete(int episodeIndex) async {
    final episode = _searchResults[episodeIndex].toPinepodsEpisode();
    final settingsBloc = Provider.of<SettingsBloc>(context, listen: false);
    final settings = settingsBloc.currentSettings;
    final userId = settings.pinepodsUserId;

    if (userId == null) {
      _showSnackBar('Not logged in', Colors.red);
      return;
    }

    try {
      if (episode.completed) {
        final success = await _pinepodsService.markEpisodeUncompleted(
          episode.episodeId,
          userId,
          episode.isYoutube,
        );
        
        if (success && mounted) {
          _showSnackBar('Episode marked as incomplete', Colors.orange);
        }
      } else {
        final success = await _pinepodsService.markEpisodeCompleted(
          episode.episodeId,
          userId,
          episode.isYoutube,
        );
        
        if (success && mounted) {
          _showSnackBar('Episode marked as complete', Colors.green);
        }
      }
    } catch (e) {
      if (mounted) {
        _showSnackBar('Error updating completion status: $e', Colors.red);
      }
    }
  }

  void _showSnackBar(String message, Color backgroundColor) {
    if (mounted) {
      ScaffoldMessenger.of(context).showSnackBar(
        SnackBar(
          content: Text(message),
          backgroundColor: backgroundColor,
          duration: const Duration(seconds: 2),
        ),
      );
    }
  }

  void _onSearchChanged() {
    final query = _searchController.text.trim();
    
    if (_debounceTimer?.isActive ?? false) {
      _debounceTimer!.cancel();
    }

    _debounceTimer = Timer(const Duration(milliseconds: 500), () {
      if (query.isNotEmpty && query != _currentQuery) {
        _currentQuery = query;
        _performSearch(query);
      } else if (query.isEmpty) {
        _clearResults();
      }
    });
  }

  Future<void> _performSearch(String query) async {
    setState(() {
      _isLoading = true;
      _errorMessage = null;
    });

    // Animate search bar to top
    _slideAnimationController.forward();

    try {
      final settingsBloc = Provider.of<SettingsBloc>(context, listen: false);
      final settings = settingsBloc.currentSettings;
      final userId = settings.pinepodsUserId;

      if (userId == null) {
        throw Exception('Not logged in');
      }

      final results = await _pinepodsService.searchEpisodes(userId, query);

      setState(() {
        _searchResults = results;
        _isLoading = false;
        _hasSearched = true;
      });

      // Animate results in
      _fadeAnimationController.forward();
    } catch (e) {
      setState(() {
        _errorMessage = e.toString();
        _isLoading = false;
        _hasSearched = true;
        _searchResults = [];
      });
    }
  }

  void _clearResults() {
    setState(() {
      _searchResults = [];
      _hasSearched = false;
      _errorMessage = null;
      _currentQuery = '';
    });
    _fadeAnimationController.reset();
    _slideAnimationController.reverse();
  }

  Widget _buildSearchBar() {
    return SlideTransition(
      position: _slideAnimation,
      child: Container(
        padding: const EdgeInsets.all(16),
        child: Card(
          elevation: 4,
          shape: RoundedRectangleBorder(
            borderRadius: BorderRadius.circular(30),
          ),
          child: Container(
            decoration: BoxDecoration(
              borderRadius: BorderRadius.circular(30),
              gradient: LinearGradient(
                colors: [
                  Theme.of(context).primaryColor.withOpacity(0.1),
                  Theme.of(context).primaryColor.withOpacity(0.05),
                ],
                begin: Alignment.topLeft,
                end: Alignment.bottomRight,
              ),
            ),
            child: TextField(
              controller: _searchController,
              focusNode: _focusNode,
              style: Theme.of(context).textTheme.bodyLarge,
              decoration: InputDecoration(
                hintText: 'Search for episodes...',
                hintStyle: Theme.of(context).textTheme.bodyMedium?.copyWith(
                  color: Theme.of(context).hintColor,
                ),
                prefixIcon: Icon(
                  Icons.search,
                  color: Theme.of(context).primaryColor,
                ),
                suffixIcon: _searchController.text.isNotEmpty
                    ? IconButton(
                        icon: Icon(
                          Icons.clear,
                          color: Theme.of(context).primaryColor,
                        ),
                        onPressed: () {
                          _searchController.clear();
                          _clearResults();
                        },
                      )
                    : null,
                border: InputBorder.none,
                contentPadding: const EdgeInsets.symmetric(
                  horizontal: 20,
                  vertical: 16,
                ),
              ),
            ),
          ),
        ),
      ),
    );
  }

  Widget _buildLoadingIndicator() {
    return Container(
      padding: const EdgeInsets.all(64),
      child: Column(
        mainAxisAlignment: MainAxisAlignment.center,
        children: [
          const CircularProgressIndicator(),
          const SizedBox(height: 16),
          Text(
            'Searching...',
            style: Theme.of(context).textTheme.bodyLarge?.copyWith(
              color: Theme.of(context).primaryColor,
            ),
          ),
        ],
      ),
    );
  }

  Widget _buildEmptyState() {
    if (!_hasSearched) {
      return Container(
        padding: const EdgeInsets.all(32),
        child: Column(
          mainAxisAlignment: MainAxisAlignment.center,
          children: [
            Icon(
              Icons.search,
              size: 64,
              color: Theme.of(context).primaryColor.withOpacity(0.5),
            ),
            const SizedBox(height: 16),
            Text(
              'Search Your Episodes',
              style: Theme.of(context).textTheme.headlineSmall?.copyWith(
                color: Theme.of(context).primaryColor,
                fontWeight: FontWeight.bold,
              ),
            ),
            const SizedBox(height: 8),
            Text(
              'Find episodes from your subscribed podcasts',
              style: Theme.of(context).textTheme.bodyMedium?.copyWith(
                color: Theme.of(context).hintColor,
              ),
              textAlign: TextAlign.center,
            ),
          ],
        ),
      );
    }

    return Container(
      padding: const EdgeInsets.all(32),
      child: Column(
        mainAxisAlignment: MainAxisAlignment.center,
        children: [
          Icon(
            Icons.search_off,
            size: 64,
            color: Theme.of(context).hintColor,
          ),
          const SizedBox(height: 16),
          Text(
            'No Episodes Found',
            style: Theme.of(context).textTheme.headlineSmall,
          ),
          const SizedBox(height: 8),
          Text(
            'Try adjusting your search terms',
            style: Theme.of(context).textTheme.bodyMedium?.copyWith(
              color: Theme.of(context).hintColor,
            ),
          ),
        ],
      ),
    );
  }

  Widget _buildErrorState() {
    return Container(
      padding: const EdgeInsets.all(32),
      child: Column(
        mainAxisAlignment: MainAxisAlignment.center,
        children: [
          Icon(
            Icons.error_outline,
            size: 64,
            color: Theme.of(context).colorScheme.error,
          ),
          const SizedBox(height: 16),
          Text(
            'Search Error',
            style: Theme.of(context).textTheme.headlineSmall?.copyWith(
              color: Theme.of(context).colorScheme.error,
            ),
          ),
          const SizedBox(height: 8),
          Text(
            _errorMessage ?? 'Unknown error occurred',
            style: Theme.of(context).textTheme.bodyMedium,
            textAlign: TextAlign.center,
          ),
          const SizedBox(height: 16),
          ElevatedButton(
            onPressed: () {
              if (_currentQuery.isNotEmpty) {
                _performSearch(_currentQuery);
              }
            },
            child: const Text('Try Again'),
          ),
        ],
      ),
    );
  }

  Widget _buildResults() {
    return FadeTransition(
      opacity: _fadeAnimation,
      child: ListView.builder(
        shrinkWrap: true,
        physics: const NeverScrollableScrollPhysics(),
        itemCount: _searchResults.length,
        itemBuilder: (context, index) {
          final result = _searchResults[index];
          final episode = result.toPinepodsEpisode();
          
          return PinepodsEpisodeCard(
            episode: episode,
            onTap: () {
              Navigator.push(
                context,
                MaterialPageRoute(
                  builder: (context) => PinepodsEpisodeDetails(
                    initialEpisode: episode,
                  ),
                ),
              );
            },
            onLongPress: () => _showContextMenu(index),
            onPlayPressed: () => _playEpisode(episode),
          );
        },
      ),
    );
  }

  @override
  Widget build(BuildContext context) {
    // Show context menu as a modal overlay if needed
    if (_contextMenuEpisodeIndex != null) {
      final episodeIndex = _contextMenuEpisodeIndex!; // Store locally to avoid null issues
      final episode = _searchResults[episodeIndex].toPinepodsEpisode();
      WidgetsBinding.instance.addPostFrameCallback((_) {
        showDialog(
          context: context,
          barrierColor: Colors.black.withOpacity(0.3),
          builder: (context) => EpisodeContextMenu(
            episode: episode,
            onSave: () {
              Navigator.of(context).pop();
              _saveEpisode(episodeIndex);
            },
            onRemoveSaved: () {
              Navigator.of(context).pop();
              _removeSavedEpisode(episodeIndex);
            },
            onDownload: episode.downloaded 
              ? () {
                  Navigator.of(context).pop();
                  _deleteEpisode(episodeIndex);
                }
              : () {
                  Navigator.of(context).pop();
                  _downloadEpisode(episodeIndex);
                },
            onLocalDownload: () {
              Navigator.of(context).pop();
              _localDownloadEpisode(episodeIndex);
            },
            onQueue: () {
              Navigator.of(context).pop();
              _toggleQueueEpisode(episodeIndex);
            },
            onMarkComplete: () {
              Navigator.of(context).pop();
              _toggleMarkComplete(episodeIndex);
            },
            onDismiss: () {
              Navigator.of(context).pop();
              _hideContextMenu();
            },
          ),
        );
      });
      // Reset the context menu index after storing it locally
      _contextMenuEpisodeIndex = null;
    }

    return SliverFillRemaining(
      child: GestureDetector(
        onTap: () {
          // Dismiss keyboard when tapping outside
          FocusScope.of(context).unfocus();
        },
        child: Column(
          children: [
            _buildSearchBar(),
            Expanded(
              child: SingleChildScrollView(
                child: AnimatedSwitcher(
                  duration: const Duration(milliseconds: 300),
                  child: _isLoading
                      ? _buildLoadingIndicator()
                      : _errorMessage != null
                          ? _buildErrorState()
                          : _searchResults.isEmpty
                              ? _buildEmptyState()
                              : _buildResults(),
                ),
              ),
            ),
          ],
        ),
      ),
    );
  }

  @override
  void dispose() {
    _debounceTimer?.cancel();
    _searchController.dispose();
    _focusNode.dispose();
    _fadeAnimationController.dispose();
    _slideAnimationController.dispose();
    super.dispose();
  }
}