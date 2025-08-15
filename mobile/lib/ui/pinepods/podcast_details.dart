// lib/ui/pinepods/podcast_details.dart

import 'package:flutter/material.dart';
import 'package:pinepods_mobile/bloc/settings/settings_bloc.dart';
import 'package:pinepods_mobile/entities/pinepods_episode.dart';
import 'package:pinepods_mobile/entities/pinepods_search.dart';
import 'package:pinepods_mobile/entities/podcast.dart';
import 'package:pinepods_mobile/entities/episode.dart';
import 'package:pinepods_mobile/entities/person.dart';
import 'package:pinepods_mobile/services/pinepods/pinepods_service.dart';
import 'package:pinepods_mobile/services/podcast/podcast_service.dart';
import 'package:pinepods_mobile/services/global_services.dart';
import 'package:pinepods_mobile/ui/widgets/pinepods_episode_card.dart';
import 'package:pinepods_mobile/ui/widgets/platform_progress_indicator.dart';
import 'package:pinepods_mobile/ui/widgets/episode_context_menu.dart';
import 'package:pinepods_mobile/ui/widgets/podcast_image.dart';
import 'package:pinepods_mobile/ui/pinepods/episode_details.dart';
import 'package:pinepods_mobile/services/pinepods/pinepods_audio_service.dart';
import 'package:pinepods_mobile/services/audio/audio_player_service.dart';
import 'package:pinepods_mobile/ui/podcast/mini_player.dart';
import 'package:pinepods_mobile/ui/utils/player_utils.dart';
import 'package:pinepods_mobile/ui/utils/local_download_utils.dart';
import 'package:provider/provider.dart';
import 'package:sliver_tools/sliver_tools.dart';

class PinepodsPodcastDetails extends StatefulWidget {
  final UnifiedPinepodsPodcast podcast;
  final bool isFollowing;
  final Function(bool)? onFollowChanged;

  const PinepodsPodcastDetails({
    super.key,
    required this.podcast,
    required this.isFollowing,
    this.onFollowChanged,
  });

  @override
  State<PinepodsPodcastDetails> createState() => _PinepodsPodcastDetailsState();
}

class _PinepodsPodcastDetailsState extends State<PinepodsPodcastDetails> {
  final PinepodsService _pinepodsService = PinepodsService();
  bool _isLoading = false;
  bool _isFollowing = false;
  String? _errorMessage;
  List<PinepodsEpisode> _episodes = [];
  List<PinepodsEpisode> _filteredEpisodes = [];
  int? _contextMenuEpisodeIndex;
  // Use global audio service instead of creating local instance
  final TextEditingController _searchController = TextEditingController();
  String _searchQuery = '';
  List<Person> _hosts = [];

  @override
  void initState() {
    super.initState();
    _isFollowing = widget.isFollowing;
    _initializeCredentials();
    _checkFollowStatus();
    _searchController.addListener(_onSearchChanged);
  }

  @override
  void dispose() {
    _searchController.dispose();
    // Don't dispose global audio service - it should persist across pages
    super.dispose();
  }

  void _onSearchChanged() {
    setState(() {
      _searchQuery = _searchController.text;
      _filterEpisodes();
    });
  }

  void _filterEpisodes() {
    if (_searchQuery.isEmpty) {
      _filteredEpisodes = List.from(_episodes);
    } else {
      _filteredEpisodes = _episodes.where((episode) {
        return episode.episodeTitle.toLowerCase().contains(_searchQuery.toLowerCase());
      }).toList();
    }
  }

  void _initializeCredentials() {
    final settingsBloc = Provider.of<SettingsBloc>(context, listen: false);
    final settings = settingsBloc.currentSettings;
    
    if (settings.pinepodsServer != null && settings.pinepodsApiKey != null) {
      _pinepodsService.setCredentials(
        settings.pinepodsServer!,
        settings.pinepodsApiKey!,
      );
      GlobalServices.setCredentials(settings.pinepodsServer!, settings.pinepodsApiKey!);
    }
  }

  Future<void> _checkFollowStatus() async {
    final settingsBloc = Provider.of<SettingsBloc>(context, listen: false);
    final settings = settingsBloc.currentSettings;
    final userId = settings.pinepodsUserId;

    if (userId == null) {
      setState(() {
        _isFollowing = false;
      });
      _loadPodcastFeed();
      return;
    }

    try {
      // If we have a valid podcast ID (> 0), assume it's followed since we got it from episode metadata
      if (widget.podcast.id > 0 && widget.isFollowing) {
        print('Using podcast ID ${widget.podcast.id} - assuming followed');
        setState(() {
          _isFollowing = true;
        });
        _loadPodcastFeed();
        return;
      }

      print('Checking follow status for: ${widget.podcast.title}');
      final isFollowing = await _pinepodsService.checkPodcastExists(
        widget.podcast.title,
        widget.podcast.url,
        userId,
      );
      
      print('Follow status result: $isFollowing');
      setState(() {
        _isFollowing = isFollowing;
      });
      
      _loadPodcastFeed();
    } catch (e) {
      print('Error checking follow status: $e');
      // Use the passed value as fallback
      _loadPodcastFeed();
    }
  }

  // Convert Episode objects to PinepodsEpisode objects
  PinepodsEpisode _convertEpisodeToPinepodsEpisode(Episode episode) {
    return PinepodsEpisode(
      podcastName: episode.podcast ?? widget.podcast.title,
      episodeTitle: episode.title ?? '',
      episodePubDate: episode.publicationDate?.toIso8601String() ?? '',
      episodeDescription: episode.description ?? '',
      episodeArtwork: episode.imageUrl ?? widget.podcast.artwork,
      episodeUrl: episode.contentUrl ?? '',
      episodeDuration: episode.duration,
      listenDuration: 0, // RSS episodes don't have listen duration
      episodeId: 0, // RSS episodes don't have server IDs
      completed: false,
      saved: false,
      queued: false,
      downloaded: false,
      isYoutube: false,
    );
  }

  Future<void> _loadPodcastFeed() async {
    setState(() {
      _isLoading = true;
      _errorMessage = null;
    });

    try {
      final settingsBloc = Provider.of<SettingsBloc>(context, listen: false);
      final settings = settingsBloc.currentSettings;
      final userId = settings.pinepodsUserId;

      List<PinepodsEpisode> episodes = [];
      
      if (_isFollowing && userId != null) {
        try {
          print('Loading episodes for followed podcast: ${widget.podcast.title}');
          
          int? podcastId;
          
          // If we already have a podcast ID (from episode metadata), use it directly
          if (widget.podcast.id > 0) {
            podcastId = widget.podcast.id;
            print('Using existing podcast ID: $podcastId');
          } else {
            // Get the actual podcast ID using the dedicated endpoint
            podcastId = await _pinepodsService.getPodcastId(
              userId,
              widget.podcast.url,
              widget.podcast.title,
            );
            print('Got podcast ID from lookup: $podcastId');
          }

          if (podcastId != null && podcastId > 0) {
            // Get episodes from server
            episodes = await _pinepodsService.getPodcastEpisodes(userId, podcastId);
            print('Loaded ${episodes.length} episodes');
            
            // Fetch podcast 2.0 data for hosts information
            try {
              final podcastData = await _pinepodsService.fetchPodcasting2PodData(podcastId, userId);
              if (podcastData != null) {
                final personsData = podcastData['people'] as List<dynamic>?;
                if (personsData != null) {
                  final hosts = personsData.map((personData) {
                    return Person(
                      name: personData['name'] ?? '',
                      role: personData['role'] ?? '',
                      group: personData['group'] ?? '',
                      image: personData['img'],
                      link: personData['href'],
                    );
                  }).toList();
                  
                  setState(() {
                    _hosts = hosts;
                  });
                  print('Loaded ${hosts.length} hosts from podcast 2.0 data');
                }
              }
            } catch (e) {
              print('Error loading podcast 2.0 data: $e');
            }
          } else {
            print('No podcast ID found - podcast may not be properly added');
          }
        } catch (e) {
          print('Error loading episodes for followed podcast: $e');
          // Fall back to empty episodes list
          episodes = [];
        }
      } else {
        try {
          print('Loading episodes from RSS feed for non-followed podcast: ${widget.podcast.url}');
          
          // Use the existing podcast service to parse RSS feed
          final podcastService = Provider.of<PodcastService>(context, listen: false);
          final rssePodcast = Podcast.fromUrl(url: widget.podcast.url);
          
          final loadedPodcast = await podcastService.loadPodcast(podcast: rssePodcast);
          
          if (loadedPodcast != null && loadedPodcast.episodes.isNotEmpty) {
            // Convert Episode objects to PinepodsEpisode objects
            episodes = loadedPodcast.episodes.map(_convertEpisodeToPinepodsEpisode).toList();
            print('Loaded ${episodes.length} episodes from RSS feed');
          } else {
            print('No episodes found in RSS feed');
          }
        } catch (e) {
          print('Error loading episodes from RSS feed: $e');
          setState(() {
            _errorMessage = 'Failed to load podcast feed';
            _isLoading = false;
          });
          return;
        }
      }

      setState(() {
        _episodes = episodes;
        _filterEpisodes(); // Initialize filtered list
        _isLoading = false;
      });
    } catch (e) {
      print('Error in _loadPodcastFeed: $e');
      setState(() {
        _episodes = [];
        _isLoading = false;
        _errorMessage = 'Failed to load episodes';
      });
    }
  }

  Future<void> _toggleFollow() async {
    final settingsBloc = Provider.of<SettingsBloc>(context, listen: false);
    final settings = settingsBloc.currentSettings;
    final userId = settings.pinepodsUserId;

    if (userId == null) {
      _showSnackBar('Not logged in to PinePods server', Colors.red);
      return;
    }

    try {
      bool success;
      final oldFollowingState = _isFollowing;
      
      if (_isFollowing) {
        success = await _pinepodsService.removePodcast(
          widget.podcast.title,
          widget.podcast.url,
          userId,
        );
        if (success) {
          setState(() {
            _isFollowing = false;
          });
          widget.onFollowChanged?.call(false);
          _showSnackBar('Podcast removed', Colors.orange);
        }
      } else {
        success = await _pinepodsService.addPodcast(widget.podcast, userId);
        if (success) {
          setState(() {
            _isFollowing = true;
          });
          widget.onFollowChanged?.call(true);
          _showSnackBar('Podcast added', Colors.green);
        }
      }

      if (success) {
        // Always reload episodes when follow status changes
        // This will switch between server episodes (followed) and RSS episodes (unfollowed)
        await _loadPodcastFeed();
      } else {
        // Revert state change if the operation failed
        setState(() {
          _isFollowing = oldFollowingState;
        });
        _showSnackBar('Failed to ${oldFollowingState ? 'remove' : 'add'} podcast', Colors.red);
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


  Future<void> _showEpisodeContextMenu(int episodeIndex) async {
    final episode = _episodes[episodeIndex];
    final isDownloadedLocally = await LocalDownloadUtils.isEpisodeDownloadedLocally(context, episode);
    
    if (!mounted) return;
    
    showDialog(
      context: context,
      barrierColor: Colors.black.withOpacity(0.3),
      builder: (context) => EpisodeContextMenu(
        episode: episode,
        isDownloadedLocally: isDownloadedLocally,
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
        onDeleteLocalDownload: () {
          Navigator.of(context).pop();
          _deleteLocalDownload(episodeIndex);
        },
        onQueue: () {
          Navigator.of(context).pop();
          _queueEpisode(episodeIndex);
        },
        onMarkComplete: () {
          Navigator.of(context).pop();
          _markEpisodeComplete(episodeIndex);
        },
        onDismiss: () {
          Navigator.of(context).pop();
          _hideEpisodeContextMenu();
        },
      ),
    );
  }

  void _hideEpisodeContextMenu() {
    setState(() {
      _contextMenuEpisodeIndex = null;
    });
  }

  PinepodsAudioService? get _audioService => GlobalServices.pinepodsAudioService;

  Future<void> _playEpisode(PinepodsEpisode episode) async {
    if (_audioService == null) {
      _showSnackBar('Audio service not available', Colors.red);
      return;
    }

    try {
      await playPinepodsEpisodeWithOptionalFullScreen(
        context,
        _audioService!,
        episode,
        resume: episode.isStarted,
      );
    } catch (e) {
      _showSnackBar('Failed to play episode: $e', Colors.red);
    }
  }

  Future<void> _saveEpisode(int episodeIndex) async {
    final episode = _episodes[episodeIndex];
    final settingsBloc = Provider.of<SettingsBloc>(context, listen: false);
    final settings = settingsBloc.currentSettings;
    final userId = settings.pinepodsUserId;

    if (userId == null) {
      _showSnackBar('Not logged in', Colors.red);
      return;
    }

    _pinepodsService.setCredentials(settings.pinepodsServer!, settings.pinepodsApiKey!);

    try {
      final success = await _pinepodsService.saveEpisode(
        episode.episodeId,
        userId,
        episode.isYoutube,
      );

      if (success) {
        setState(() {
          _episodes[episodeIndex] = _updateEpisodeProperty(episode, saved: true);
          _filteredEpisodes = _episodes.where((e) => 
            e.episodeTitle.toLowerCase().contains(_searchController.text.toLowerCase())
          ).toList();
        });
        _showSnackBar('Episode saved!', Colors.green);
      } else {
        _showSnackBar('Failed to save episode', Colors.red);
      }
    } catch (e) {
      _showSnackBar('Error saving episode: $e', Colors.red);
    }

    _hideEpisodeContextMenu();
  }

  Future<void> _removeSavedEpisode(int episodeIndex) async {
    final episode = _episodes[episodeIndex];
    final settingsBloc = Provider.of<SettingsBloc>(context, listen: false);
    final settings = settingsBloc.currentSettings;
    final userId = settings.pinepodsUserId;

    if (userId == null) {
      _showSnackBar('Not logged in', Colors.red);
      return;
    }

    _pinepodsService.setCredentials(settings.pinepodsServer!, settings.pinepodsApiKey!);

    try {
      final success = await _pinepodsService.removeSavedEpisode(
        episode.episodeId,
        userId,
        episode.isYoutube,
      );

      if (success) {
        setState(() {
          _episodes[episodeIndex] = _updateEpisodeProperty(episode, saved: false);
          _filteredEpisodes = _episodes.where((e) => 
            e.episodeTitle.toLowerCase().contains(_searchController.text.toLowerCase())
          ).toList();
        });
        _showSnackBar('Removed from saved episodes', Colors.orange);
      } else {
        _showSnackBar('Failed to remove saved episode', Colors.red);
      }
    } catch (e) {
      _showSnackBar('Error removing saved episode: $e', Colors.red);
    }

    _hideEpisodeContextMenu();
  }

  Future<void> _downloadEpisode(int episodeIndex) async {
    final episode = _episodes[episodeIndex];
    final settingsBloc = Provider.of<SettingsBloc>(context, listen: false);
    final settings = settingsBloc.currentSettings;
    final userId = settings.pinepodsUserId;

    if (userId == null) {
      _showSnackBar('Not logged in', Colors.red);
      return;
    }

    _pinepodsService.setCredentials(settings.pinepodsServer!, settings.pinepodsApiKey!);

    try {
      final success = await _pinepodsService.downloadEpisode(
        episode.episodeId,
        userId,
        episode.isYoutube,
      );

      if (success) {
        setState(() {
          _episodes[episodeIndex] = _updateEpisodeProperty(episode, downloaded: true);
          _filteredEpisodes = _episodes.where((e) => 
            e.episodeTitle.toLowerCase().contains(_searchController.text.toLowerCase())
          ).toList();
        });
        _showSnackBar('Episode download started!', Colors.green);
      } else {
        _showSnackBar('Failed to download episode', Colors.red);
      }
    } catch (e) {
      _showSnackBar('Error downloading episode: $e', Colors.red);
    }

    _hideEpisodeContextMenu();
  }

  Future<void> _deleteEpisode(int episodeIndex) async {
    final episode = _episodes[episodeIndex];
    final settingsBloc = Provider.of<SettingsBloc>(context, listen: false);
    final settings = settingsBloc.currentSettings;
    final userId = settings.pinepodsUserId;

    if (userId == null) {
      _showSnackBar('Not logged in', Colors.red);
      return;
    }

    _pinepodsService.setCredentials(settings.pinepodsServer!, settings.pinepodsApiKey!);

    try {
      final success = await _pinepodsService.deleteEpisode(
        episode.episodeId,
        userId,
        episode.isYoutube,
      );

      if (success) {
        setState(() {
          _episodes[episodeIndex] = _updateEpisodeProperty(episode, downloaded: false);
          _filteredEpisodes = _episodes.where((e) => 
            e.episodeTitle.toLowerCase().contains(_searchController.text.toLowerCase())
          ).toList();
        });
        _showSnackBar('Episode deleted from server', Colors.orange);
      } else {
        _showSnackBar('Failed to delete episode', Colors.red);
      }
    } catch (e) {
      _showSnackBar('Error deleting episode: $e', Colors.red);
    }

    _hideEpisodeContextMenu();
  }

  Future<void> _queueEpisode(int episodeIndex) async {
    final episode = _episodes[episodeIndex];
    final settingsBloc = Provider.of<SettingsBloc>(context, listen: false);
    final settings = settingsBloc.currentSettings;
    final userId = settings.pinepodsUserId;

    if (userId == null) {
      _showSnackBar('Not logged in', Colors.red);
      return;
    }

    _pinepodsService.setCredentials(settings.pinepodsServer!, settings.pinepodsApiKey!);

    try {
      bool success;
      if (episode.queued) {
        success = await _pinepodsService.removeQueuedEpisode(
          episode.episodeId,
          userId,
          episode.isYoutube,
        );
        if (success) {
          setState(() {
            _episodes[episodeIndex] = _updateEpisodeProperty(episode, queued: false);
            _filteredEpisodes = _episodes.where((e) => 
              e.episodeTitle.toLowerCase().contains(_searchController.text.toLowerCase())
            ).toList();
          });
          _showSnackBar('Removed from queue', Colors.orange);
        }
      } else {
        success = await _pinepodsService.queueEpisode(
          episode.episodeId,
          userId,
          episode.isYoutube,
        );
        if (success) {
          setState(() {
            _episodes[episodeIndex] = _updateEpisodeProperty(episode, queued: true);
            _filteredEpisodes = _episodes.where((e) => 
              e.episodeTitle.toLowerCase().contains(_searchController.text.toLowerCase())
            ).toList();
          });
          _showSnackBar('Added to queue!', Colors.green);
        }
      }

      if (!success) {
        _showSnackBar('Failed to update queue', Colors.red);
      }
    } catch (e) {
      _showSnackBar('Error updating queue: $e', Colors.red);
    }

    _hideEpisodeContextMenu();
  }

  Future<void> _markEpisodeComplete(int episodeIndex) async {
    final episode = _episodes[episodeIndex];
    final settingsBloc = Provider.of<SettingsBloc>(context, listen: false);
    final settings = settingsBloc.currentSettings;
    final userId = settings.pinepodsUserId;

    if (userId == null) {
      _showSnackBar('Not logged in', Colors.red);
      return;
    }

    _pinepodsService.setCredentials(settings.pinepodsServer!, settings.pinepodsApiKey!);

    try {
      final success = await _pinepodsService.markEpisodeCompleted(
        episode.episodeId,
        userId,
        episode.isYoutube,
      );

      if (success) {
        setState(() {
          _episodes[episodeIndex] = _updateEpisodeProperty(episode, completed: true);
          _filteredEpisodes = _episodes.where((e) => 
            e.episodeTitle.toLowerCase().contains(_searchController.text.toLowerCase())
          ).toList();
        });
        _showSnackBar('Episode marked as complete', Colors.green);
      } else {
        _showSnackBar('Failed to mark episode complete', Colors.red);
      }
    } catch (e) {
      _showSnackBar('Error marking episode complete: $e', Colors.red);
    }

    _hideEpisodeContextMenu();
  }

  Future<void> _localDownloadEpisode(int episodeIndex) async {
    final episode = _episodes[episodeIndex];
    
    final success = await LocalDownloadUtils.localDownloadEpisode(context, episode);
    
    if (success) {
      _showSnackBar('Episode download started', Colors.green);
    } else {
      _showSnackBar('Failed to start download', Colors.red);
    }

    _hideEpisodeContextMenu();
  }

  Future<void> _deleteLocalDownload(int episodeIndex) async {
    final episode = _episodes[episodeIndex];
    
    final deletedCount = await LocalDownloadUtils.deleteLocalDownload(context, episode);
    
    if (deletedCount > 0) {
      _showSnackBar(
        'Deleted $deletedCount local download${deletedCount > 1 ? 's' : ''}', 
        Colors.orange
      );
    } else {
      _showSnackBar('Local download not found', Colors.red);
    }

    _hideEpisodeContextMenu();
  }

  PinepodsEpisode _updateEpisodeProperty(
    PinepodsEpisode episode, {
    bool? saved,
    bool? downloaded,
    bool? queued,
    bool? completed,
  }) {
    return PinepodsEpisode(
      podcastName: episode.podcastName,
      episodeTitle: episode.episodeTitle,
      episodePubDate: episode.episodePubDate,
      episodeDescription: episode.episodeDescription,
      episodeArtwork: episode.episodeArtwork,
      episodeUrl: episode.episodeUrl,
      episodeDuration: episode.episodeDuration,
      listenDuration: episode.listenDuration,
      episodeId: episode.episodeId,
      completed: completed ?? episode.completed,
      saved: saved ?? episode.saved,
      queued: queued ?? episode.queued,
      downloaded: downloaded ?? episode.downloaded,
      isYoutube: episode.isYoutube,
      podcastId: episode.podcastId,
    );
  }

  @override
  Widget build(BuildContext context) {
    return Scaffold(
      body: Column(
        children: [
          Expanded(
            child: CustomScrollView(
              slivers: [
          SliverAppBar(
            expandedHeight: 300,
            pinned: true,
            flexibleSpace: FlexibleSpaceBar(
              title: Text(
                widget.podcast.title,
                style: const TextStyle(
                  shadows: [
                    Shadow(
                      offset: Offset(0, 1),
                      blurRadius: 3,
                      color: Colors.black54,
                    ),
                  ],
                ),
              ),
              background: Stack(
                fit: StackFit.expand,
                children: [
                  widget.podcast.artwork.isNotEmpty
                      ? Image.network(
                          widget.podcast.artwork,
                          fit: BoxFit.cover,
                          errorBuilder: (context, error, stackTrace) {
                            return Container(
                              color: Colors.grey[300],
                              child: const Icon(
                                Icons.music_note,
                                size: 80,
                                color: Colors.grey,
                              ),
                            );
                          },
                        )
                      : Container(
                          color: Colors.grey[300],
                          child: const Icon(
                            Icons.music_note,
                            size: 80,
                            color: Colors.grey,
                          ),
                        ),
                  Container(
                    decoration: BoxDecoration(
                      gradient: LinearGradient(
                        begin: Alignment.topCenter,
                        end: Alignment.bottomCenter,
                        colors: [
                          Colors.transparent,
                          Colors.black.withOpacity(0.7),
                        ],
                      ),
                    ),
                  ),
                ],
              ),
            ),
            actions: [
              IconButton(
                onPressed: _toggleFollow,
                icon: Icon(
                  _isFollowing ? Icons.favorite : Icons.favorite_border,
                  color: _isFollowing ? Colors.red : Colors.white,
                ),
                tooltip: _isFollowing ? 'Unfollow' : 'Follow',
              ),
            ],
          ),
          
          SliverToBoxAdapter(
            child: Padding(
              padding: const EdgeInsets.all(16.0),
              child: Column(
                crossAxisAlignment: CrossAxisAlignment.start,
                children: [
                  // Podcast info with follow/unfollow button
                  Row(
                    children: [
                      Expanded(
                        child: Column(
                          crossAxisAlignment: CrossAxisAlignment.start,
                          children: [
                            if (widget.podcast.author.isNotEmpty)
                              Text(
                                'By ${widget.podcast.author}',
                                style: TextStyle(
                                  fontSize: 16,
                                  color: Theme.of(context).primaryColor,
                                  fontWeight: FontWeight.w500,
                                ),
                              ),
                          ],
                        ),
                      ),
                      ElevatedButton.icon(
                        onPressed: _toggleFollow,
                        icon: Icon(
                          _isFollowing ? Icons.remove : Icons.add,
                          size: 16,
                        ),
                        label: Text(_isFollowing ? 'Unfollow' : 'Follow'),
                        style: ElevatedButton.styleFrom(
                          backgroundColor: _isFollowing ? Colors.red : Colors.green,
                          foregroundColor: Colors.white,
                        ),
                      ),
                    ],
                  ),
                  
                  const SizedBox(height: 8),
                  
                  Text(
                    widget.podcast.description,
                    style: const TextStyle(fontSize: 14),
                  ),
                  
                  const SizedBox(height: 16),
                  
                  // Podcast stats
                  Row(
                    children: [
                      Icon(
                        Icons.mic,
                        size: 16,
                        color: Colors.grey[600],
                      ),
                      const SizedBox(width: 4),
                      Text(
                        '${widget.podcast.episodeCount} episode${widget.podcast.episodeCount != 1 ? 's' : ''}',
                        style: TextStyle(
                          fontSize: 14,
                          color: Colors.grey[600],
                        ),
                      ),
                      const SizedBox(width: 16),
                      if (widget.podcast.explicit)
                        Container(
                          padding: const EdgeInsets.symmetric(
                            horizontal: 6,
                            vertical: 2,
                          ),
                          decoration: BoxDecoration(
                            color: Colors.red,
                            borderRadius: BorderRadius.circular(4),
                          ),
                          child: const Text(
                            'Explicit',
                            style: TextStyle(
                              color: Colors.white,
                              fontSize: 12,
                              fontWeight: FontWeight.bold,
                            ),
                          ),
                        ),
                    ],
                  ),
                  
                  // Hosts section (filter out "Unknown Host" entries)
                  if (_hosts.where((host) => host.name != "Unknown Host").isNotEmpty) ...[
                    const SizedBox(height: 16),
                    Text(
                      'Hosts',
                      style: TextStyle(
                        fontSize: 16,
                        fontWeight: FontWeight.bold,
                        color: Colors.grey[800],
                      ),
                    ),
                    const SizedBox(height: 8),
                    SizedBox(
                      height: 80,
                      child: Builder(builder: (context) {
                        final actualHosts = _hosts.where((host) => host.name != "Unknown Host").toList();
                        return ListView.builder(
                          scrollDirection: Axis.horizontal,
                          itemCount: actualHosts.length,
                          itemBuilder: (context, index) {
                            final host = actualHosts[index];
                          return Container(
                            width: 70,
                            margin: const EdgeInsets.only(right: 12),
                            child: Column(
                              children: [
                                Container(
                                  width: 50,
                                  height: 50,
                                  decoration: BoxDecoration(
                                    shape: BoxShape.circle,
                                    color: Colors.grey[300],
                                  ),
                                  child: host.image != null && host.image!.isNotEmpty
                                      ? ClipRRect(
                                          borderRadius: BorderRadius.circular(25),
                                          child: PodcastImage(
                                            url: host.image!,
                                            width: 50,
                                            height: 50,
                                            fit: BoxFit.cover,
                                          ),
                                        )
                                      : const Icon(
                                          Icons.person,
                                          size: 30,
                                          color: Colors.grey,
                                        ),
                                ),
                                const SizedBox(height: 4),
                                Text(
                                  host.name,
                                  style: const TextStyle(fontSize: 12),
                                  textAlign: TextAlign.center,
                                  maxLines: 1,
                                  overflow: TextOverflow.ellipsis,
                                ),
                              ],
                            ),
                          );
                        },
                        );
                      }),
                    ),
                  ],
                  
                  const SizedBox(height: 24),
                  
                  // Episodes section header
                  Row(
                    children: [
                      const Text(
                        'Episodes',
                        style: TextStyle(
                          fontSize: 20,
                          fontWeight: FontWeight.bold,
                        ),
                      ),
                      const Spacer(),
                      if (_isLoading)
                        const SizedBox(
                          width: 20,
                          height: 20,
                          child: CircularProgressIndicator(strokeWidth: 2),
                        ),
                    ],
                  ),
                  
                  const SizedBox(height: 16),
                ],
              ),
            ),
          ),
          
          // Episodes list
          if (_isLoading)
            const SliverToBoxAdapter(
              child: Center(
                child: Padding(
                  padding: EdgeInsets.all(32.0),
                  child: PlatformProgressIndicator(),
                ),
              ),
            )
          else if (_errorMessage != null)
            SliverToBoxAdapter(
              child: Center(
                child: Padding(
                  padding: const EdgeInsets.all(32.0),
                  child: Column(
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
                        onPressed: _loadPodcastFeed,
                        child: const Text('Retry'),
                      ),
                    ],
                  ),
                ),
              ),
            )
          else if (_episodes.isEmpty)
            SliverToBoxAdapter(
              child: Center(
                child: Padding(
                  padding: const EdgeInsets.all(32.0),
                  child: Column(
                    children: [
                      Icon(
                        Icons.info_outline,
                        size: 64,
                        color: Colors.blue[300],
                      ),
                      const SizedBox(height: 16),
                      Text(
                        _isFollowing ? 'No episodes found' : 'Episodes available after following',
                        style: Theme.of(context).textTheme.headlineSmall,
                        textAlign: TextAlign.center,
                      ),
                      const SizedBox(height: 8),
                      Text(
                        _isFollowing 
                            ? 'Episodes from your PinePods library will appear here'
                            : 'Follow this podcast to add it to your library and view episodes',
                        style: Theme.of(context).textTheme.bodyMedium,
                        textAlign: TextAlign.center,
                      ),
                      const SizedBox(height: 16),
                      ElevatedButton(
                        onPressed: _toggleFollow,
                        child: Text(_isFollowing ? 'Unfollow' : 'Follow'),
                      ),
                    ],
                  ),
                ),
              ),
            )
          else
            MultiSliver(
              children: [
                _buildSearchBar(),
                _buildEpisodesList(),
              ],
            ),
              ],
            ),
          ),
          const MiniPlayer(),
        ],
      ),
    );
  }

  Widget _buildSearchBar() {
    return SliverToBoxAdapter(
      child: Padding(
        padding: const EdgeInsets.all(16.0),
        child: TextField(
          controller: _searchController,
          decoration: InputDecoration(
            hintText: 'Filter episodes...',
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
    );
  }

  Widget _buildEpisodesList() {
    // Check if search returned no results
    if (_filteredEpisodes.isEmpty && _searchQuery.isNotEmpty) {
      return SliverFillRemaining(
        child: Center(
          child: Column(
            mainAxisAlignment: MainAxisAlignment.center,
            children: [
              Icon(
                Icons.search_off,
                size: 64,
                color: Theme.of(context).primaryColor,
              ),
              const SizedBox(height: 16),
              Text(
                'No episodes found',
                style: Theme.of(context).textTheme.titleLarge,
                textAlign: TextAlign.center,
              ),
              const SizedBox(height: 8),
              Text(
                'No episodes match "$_searchQuery"',
                style: Theme.of(context).textTheme.bodyMedium,
                textAlign: TextAlign.center,
              ),
            ],
          ),
        ),
      );
    }

    return SliverList(
      delegate: SliverChildBuilderDelegate(
        (context, index) {
          final episode = _filteredEpisodes[index];
          // Find the original index for context menu operations
          final originalIndex = _episodes.indexOf(episode);
          return PinepodsEpisodeCard(
            episode: episode,
            onTap: _isFollowing ? () {
              // Navigate to episode details only if following
              Navigator.push(
                context,
                MaterialPageRoute(
                  builder: (context) => PinepodsEpisodeDetails(
                    initialEpisode: episode,
                  ),
                ),
              );
            } : null, // Disable tap if not following
            onLongPress: _isFollowing ? () {
              _showEpisodeContextMenu(originalIndex);
            } : null, // Disable long press if not following
            onPlayPressed: _isFollowing ? () {
              _playEpisode(episode);
            } : null, // Disable play if not following
          );
        },
        childCount: _filteredEpisodes.length,
      ),
    );
  }
}