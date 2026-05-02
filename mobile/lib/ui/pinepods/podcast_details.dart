// lib/ui/pinepods/podcast_details.dart

import 'dart:convert';
import 'package:flutter/material.dart';
import 'package:http/http.dart' as http;
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
import 'package:pinepods_mobile/ui/podcast/mini_player.dart';
import 'package:pinepods_mobile/ui/utils/player_utils.dart';
import 'package:pinepods_mobile/ui/utils/local_download_utils.dart';
import 'package:provider/provider.dart';
import 'package:shared_preferences/shared_preferences.dart';
import 'package:sliver_tools/sliver_tools.dart';

// Sort direction for episodes within a podcast
enum EpisodeSortDirection {
  newestFirst,
  oldestFirst,
  shortestFirst,
  longestFirst,
  titleAZ,
  titleZA,
}

// 3-state completed filter
enum CompletedFilter {
  showAll,
  showOnly,
  hide,
}

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
  bool _isFollowButtonLoading = false;
  String? _errorMessage;
  List<PinepodsEpisode> _episodes = [];
  List<PinepodsEpisode> _filteredEpisodes = [];
  int? _contextMenuEpisodeIndex;
  // Use global audio service instead of creating local instance
  final TextEditingController _searchController = TextEditingController();
  String _searchQuery = '';
  List<Person> _hosts = [];

  // Sort and filter state
  EpisodeSortDirection _sortDirection = EpisodeSortDirection.newestFirst;
  CompletedFilter _completedFilter = CompletedFilter.showAll;
  bool _showInProgress = false;
  bool _isAutoDownloadEnabled = false;
  bool _isAutoPlayNextEnabled = false;

  @override
  void initState() {
    super.initState();
    _isFollowing = widget.isFollowing;
    _initializeCredentials();
    _loadSortPreference();
    _loadAutoDownloadPreference();
    _loadAutoPlayNextPreference();
    _checkFollowStatus();
    _searchController.addListener(_onSearchChanged);
  }

  Future<void> _loadSortPreference() async {
    final prefs = await SharedPreferences.getInstance();
    // Use podcast URL as key for per-podcast sort preference
    final key = 'episode_sort_${widget.podcast.url.hashCode}';
    final savedSort = prefs.getString(key);
    if (savedSort != null && mounted) {
      setState(() {
        switch (savedSort) {
          case 'newest':
            _sortDirection = EpisodeSortDirection.newestFirst;
            break;
          case 'oldest':
            _sortDirection = EpisodeSortDirection.oldestFirst;
            break;
          case 'shortest':
            _sortDirection = EpisodeSortDirection.shortestFirst;
            break;
          case 'longest':
            _sortDirection = EpisodeSortDirection.longestFirst;
            break;
          case 'title_az':
            _sortDirection = EpisodeSortDirection.titleAZ;
            break;
          case 'title_za':
            _sortDirection = EpisodeSortDirection.titleZA;
            break;
        }
        _filterEpisodes();
      });
    }
  }

  Future<void> _loadAutoDownloadPreference() async {
    final podcastId = widget.podcast.id;
    if (podcastId <= 0) return;
    final prefs = await SharedPreferences.getInstance();
    if (mounted) {
      setState(() {
        _isAutoDownloadEnabled = prefs.getBool('auto_download_podcast_$podcastId') ?? false;
      });
    }
  }

  Future<void> _toggleAutoDownload() async {
    final podcastId = widget.podcast.id;
    if (podcastId <= 0) return;
    final prefs = await SharedPreferences.getInstance();
    final newValue = !_isAutoDownloadEnabled;
    await prefs.setBool('auto_download_podcast_$podcastId', newValue);
    if (newValue) {
      final lastCheckKey = 'auto_download_last_check_$podcastId';
      if (prefs.getString(lastCheckKey) == null) {
        await prefs.setString(lastCheckKey, DateTime.now().toIso8601String());
      }
    }
    if (mounted) {
      setState(() {
        _isAutoDownloadEnabled = newValue;
      });
      ScaffoldMessenger.of(context).showSnackBar(
        SnackBar(
          content: Text(newValue ? 'Auto-download enabled' : 'Auto-download disabled'),
          duration: const Duration(seconds: 2),
        ),
      );
    }
  }

  Future<void> _loadAutoPlayNextPreference() async {
    final podcastId = widget.podcast.id;
    if (podcastId <= 0) return;

    try {
      final settingsBloc = Provider.of<SettingsBloc>(context, listen: false);
      final settings = settingsBloc.currentSettings;
      if (settings.pinepodsServer != null && settings.pinepodsApiKey != null && settings.pinepodsUserId != null) {
        _pinepodsService.setCredentials(settings.pinepodsServer!, settings.pinepodsApiKey!);
        final status = await _pinepodsService.getAutoPlayNextStatus(podcastId, settings.pinepodsUserId!);
        if (mounted) {
          setState(() {
            _isAutoPlayNextEnabled = status;
          });
        }
      }
    } catch (e) {
      debugPrint('Error loading auto-play-next preference: $e');
    }
  }

  Future<void> _toggleAutoPlayNext() async {
    final podcastId = widget.podcast.id;
    if (podcastId <= 0) return;

    final newValue = !_isAutoPlayNextEnabled;

    try {
      final settingsBloc = Provider.of<SettingsBloc>(context, listen: false);
      final settings = settingsBloc.currentSettings;
      if (settings.pinepodsServer != null && settings.pinepodsApiKey != null && settings.pinepodsUserId != null) {
        _pinepodsService.setCredentials(settings.pinepodsServer!, settings.pinepodsApiKey!);

        final url = Uri.parse('${settings.pinepodsServer}/api/data/enable_auto_play_next');
        final response = await http.post(
          url,
          headers: {
            'Api-Key': settings.pinepodsApiKey!,
            'Content-Type': 'application/json',
          },
          body: jsonEncode({
            'podcast_id': podcastId,
            'user_id': settings.pinepodsUserId,
            'auto_play_next': newValue,
          }),
        );

        if (response.statusCode == 200 && mounted) {
          setState(() {
            _isAutoPlayNextEnabled = newValue;
          });
          ScaffoldMessenger.of(context).showSnackBar(
            SnackBar(
              content: Text(newValue ? 'Auto-play next enabled' : 'Auto-play next disabled'),
              duration: const Duration(seconds: 2),
            ),
          );
        }
      }
    } catch (e) {
      debugPrint('Error toggling auto-play-next: $e');
    }
  }

  Future<void> _saveSortPreference() async {
    final prefs = await SharedPreferences.getInstance();
    final key = 'episode_sort_${widget.podcast.url.hashCode}';
    String value;
    switch (_sortDirection) {
      case EpisodeSortDirection.newestFirst:
        value = 'newest';
        break;
      case EpisodeSortDirection.oldestFirst:
        value = 'oldest';
        break;
      case EpisodeSortDirection.shortestFirst:
        value = 'shortest';
        break;
      case EpisodeSortDirection.longestFirst:
        value = 'longest';
        break;
      case EpisodeSortDirection.titleAZ:
        value = 'title_az';
        break;
      case EpisodeSortDirection.titleZA:
        value = 'title_za';
        break;
    }
    await prefs.setString(key, value);
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
    // Start with all episodes
    List<PinepodsEpisode> filtered = List.from(_episodes);

    // Apply search filter (search both title and description)
    if (_searchQuery.isNotEmpty) {
      final query = _searchQuery.toLowerCase();
      filtered = filtered.where((episode) {
        return episode.episodeTitle.toLowerCase().contains(query) ||
            episode.episodeDescription.toLowerCase().contains(query);
      }).toList();
    }

    // Apply completed filter (3-state)
    if (_showInProgress) {
      // In Progress: not completed but has some listen duration
      filtered = filtered.where((episode) {
        return !episode.completed && (episode.listenDuration ?? 0) > 0;
      }).toList();
    } else {
      switch (_completedFilter) {
        case CompletedFilter.showOnly:
          filtered = filtered.where((episode) => episode.completed).toList();
          break;
        case CompletedFilter.hide:
          filtered = filtered.where((episode) => !episode.completed).toList();
          break;
        case CompletedFilter.showAll:
          // No filtering
          break;
      }
    }

    // Apply sorting
    filtered.sort((a, b) {
      switch (_sortDirection) {
        case EpisodeSortDirection.newestFirst:
          return _compareDates(b.episodePubDate, a.episodePubDate);
        case EpisodeSortDirection.oldestFirst:
          return _compareDates(a.episodePubDate, b.episodePubDate);
        case EpisodeSortDirection.shortestFirst:
          return (a.episodeDuration ?? 0).compareTo(b.episodeDuration ?? 0);
        case EpisodeSortDirection.longestFirst:
          return (b.episodeDuration ?? 0).compareTo(a.episodeDuration ?? 0);
        case EpisodeSortDirection.titleAZ:
          return a.episodeTitle.toLowerCase().compareTo(b.episodeTitle.toLowerCase());
        case EpisodeSortDirection.titleZA:
          return b.episodeTitle.toLowerCase().compareTo(a.episodeTitle.toLowerCase());
      }
    });

    _filteredEpisodes = filtered;
  }

  int _compareDates(String dateA, String dateB) {
    try {
      final a = DateTime.parse(dateA);
      final b = DateTime.parse(dateB);
      return a.compareTo(b);
    } catch (e) {
      return 0;
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
            print('Loaded ${episodes.length} episodes from server for podcastId: $podcastId');
            
            // If server has no episodes, this podcast may need episode sync
            if (episodes.isEmpty) {
              print('Server has no episodes for subscribed podcast. This should not happen.');
              print('Podcast ID: $podcastId, Title: ${widget.podcast.title}');
              
              // For subscribed podcasts, we should NOT fall back to RSS
              // The server should have episodes. This indicates a server-side sync issue.
              // Fall back to RSS ONLY as emergency backup, but episodes won't be clickable
              try {
                final podcastService = Provider.of<PodcastService>(context, listen: false);
                final rssPodcast = Podcast.fromUrl(url: widget.podcast.url);
                
                final loadedPodcast = await podcastService.loadPodcast(podcast: rssPodcast);
                
                if (loadedPodcast != null && loadedPodcast.episodes.isNotEmpty) {
                  episodes = loadedPodcast.episodes.map(_convertEpisodeToPinepodsEpisode).toList();
                  print('Emergency RSS fallback: Loaded ${episodes.length} episodes (NOT CLICKABLE)');
                }
              } catch (e) {
                print('Emergency RSS fallback also failed: $e');
              }
            }
            
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
    // Prevent concurrent operations
    if (_isFollowButtonLoading) {
      print('PinePods Follow button: Already processing, ignoring click');
      return;
    }

    final settingsBloc = Provider.of<SettingsBloc>(context, listen: false);
    final settings = settingsBloc.currentSettings;
    final userId = settings.pinepodsUserId;

    if (userId == null) {
      _showSnackBar('Not logged in to PinePods server', Colors.red);
      return;
    }

    // Show confirmation dialog if unfollowing
    if (_isFollowing) {
      final confirmed = await showDialog<bool>(
        context: context,
        barrierDismissible: false, // Prevent dismissing by tapping outside
        builder: (context) => AlertDialog(
          title: const Text('Unfollow Podcast'),
          content: Text(
            'Are you sure you want to unfollow "${widget.podcast.title}"?\n\nThis will remove the podcast and all episode history from your library.',
          ),
          actions: [
            TextButton(
              onPressed: () => Navigator.of(context).pop(false),
              child: const Text('Cancel'),
            ),
            TextButton(
              onPressed: () => Navigator.of(context).pop(true),
              style: TextButton.styleFrom(
                foregroundColor: Colors.red,
              ),
              child: const Text('Unfollow'),
            ),
          ],
        ),
      );

      if (confirmed != true) {
        print('PinePods Follow button: User cancelled unfollow');
        return;
      }
    }

    print('PinePods Follow button: CLICKED - Setting loading to true');
    setState(() {
      _isFollowButtonLoading = true;
    });

    try {
      bool success;
      final oldFollowingState = _isFollowing;

      if (oldFollowingState) {
        print('PinePods: Attempting to remove podcast');
        success = await _pinepodsService.removePodcast(
          widget.podcast.title,
          widget.podcast.url,
          userId,
        );
        print('PinePods: Remove podcast result: $success');
        if (success) {
          setState(() {
            _isFollowing = false;
          });
          widget.onFollowChanged?.call(false);
          _showSnackBar('Podcast removed', Colors.orange);
        }
      } else {
        print('PinePods: Attempting to add podcast');
        success = await _pinepodsService.addPodcast(widget.podcast, userId);
        print('PinePods: Add podcast result: $success');
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
        print('PinePods: Reloading podcast feed after ${oldFollowingState ? 'unfollow' : 'follow'}');
        await _loadPodcastFeed();
      } else {
        // Revert state change if the operation failed
        setState(() {
          _isFollowing = oldFollowingState;
        });
        _showSnackBar('Failed to ${oldFollowingState ? 'remove' : 'add'} podcast', Colors.red);
      }
    } catch (e) {
      print('PinePods: Error in _toggleFollow: $e');
      _showSnackBar('Error: $e', Colors.red);
    } finally {
      // Always reset loading state
      if (mounted) {
        setState(() {
          _isFollowButtonLoading = false;
        });
      }
      print('PinePods Follow button: Loading state reset to false');
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
              if (_isFollowing && widget.podcast.id > 0)
                IconButton(
                  onPressed: _toggleAutoDownload,
                  icon: Icon(
                    _isAutoDownloadEnabled
                        ? Icons.download_for_offline
                        : Icons.download_for_offline_outlined,
                    color: _isAutoDownloadEnabled ? Colors.blue[300] : Colors.white,
                  ),
                  tooltip: _isAutoDownloadEnabled ? 'Disable auto-download' : 'Enable auto-download',
                ),
              if (_isFollowing && widget.podcast.id > 0)
                IconButton(
                  onPressed: _toggleAutoPlayNext,
                  icon: Icon(
                    _isAutoPlayNextEnabled
                        ? Icons.skip_next
                        : Icons.skip_next_outlined,
                    color: _isAutoPlayNextEnabled ? Colors.green[300] : Colors.white,
                  ),
                  tooltip: _isAutoPlayNextEnabled ? 'Disable auto-play next' : 'Enable auto-play next',
                ),
              IconButton(
                onPressed: _isFollowButtonLoading ? null : _toggleFollow,
                icon: _isFollowButtonLoading
                  ? const SizedBox(
                      width: 24,
                      height: 24,
                      child: CircularProgressIndicator(
                        strokeWidth: 2.0,
                        valueColor: AlwaysStoppedAnimation<Color>(Colors.white),
                      ),
                    )
                  : Icon(
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
                        onPressed: _isFollowButtonLoading ? null : _toggleFollow,
                        icon: _isFollowButtonLoading 
                          ? const SizedBox(
                              width: 16,
                              height: 16,
                              child: CircularProgressIndicator(
                                strokeWidth: 2.0,
                                valueColor: AlwaysStoppedAnimation<Color>(Colors.white),
                              ),
                            )
                          : Icon(
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
                  
                  _ExpandableDescription(description: widget.podcast.description),
                  
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
                        '${_episodes.length} episode${_episodes.length != 1 ? 's' : ''}',
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
                      ElevatedButton.icon(
                        onPressed: _isFollowing ? _loadPodcastFeed : _toggleFollow,
                        icon: Icon(_isFollowing ? Icons.refresh : Icons.add),
                        label: Text(_isFollowing ? 'Retry' : 'Follow'),
                      ),
                    ],
                  ),
                ),
              ),
            )
          else
            MultiSliver(
              children: [
                _buildSearchAndFilterBar(),
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

  Widget _buildSearchAndFilterBar() {
    return SliverToBoxAdapter(
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          // Search bar with sort dropdown
          Padding(
            padding: const EdgeInsets.fromLTRB(16.0, 16.0, 16.0, 8.0),
            child: Row(
              children: [
                // Search field
                Expanded(
                  child: TextField(
                    controller: _searchController,
                    decoration: InputDecoration(
                      hintText: 'Search episodes...',
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
                  padding: const EdgeInsets.symmetric(horizontal: 12),
                  decoration: BoxDecoration(
                    color: Theme.of(context).cardColor,
                    borderRadius: BorderRadius.circular(12),
                    border: Border.all(color: Theme.of(context).dividerColor),
                  ),
                  child: DropdownButtonHideUnderline(
                    child: DropdownButton<EpisodeSortDirection>(
                      value: _sortDirection,
                      icon: const Icon(Icons.sort),
                      items: const [
                        DropdownMenuItem(
                          value: EpisodeSortDirection.newestFirst,
                          child: Text('Newest'),
                        ),
                        DropdownMenuItem(
                          value: EpisodeSortDirection.oldestFirst,
                          child: Text('Oldest'),
                        ),
                        DropdownMenuItem(
                          value: EpisodeSortDirection.shortestFirst,
                          child: Text('Shortest'),
                        ),
                        DropdownMenuItem(
                          value: EpisodeSortDirection.longestFirst,
                          child: Text('Longest'),
                        ),
                        DropdownMenuItem(
                          value: EpisodeSortDirection.titleAZ,
                          child: Text('Title A-Z'),
                        ),
                        DropdownMenuItem(
                          value: EpisodeSortDirection.titleZA,
                          child: Text('Title Z-A'),
                        ),
                      ],
                      onChanged: (value) {
                        if (value != null) {
                          setState(() {
                            _sortDirection = value;
                            _filterEpisodes();
                          });
                          _saveSortPreference();
                        }
                      },
                    ),
                  ),
                ),
              ],
            ),
          ),
          // Filter chips
          Padding(
            padding: const EdgeInsets.fromLTRB(16.0, 0, 16.0, 8.0),
            child: SingleChildScrollView(
              scrollDirection: Axis.horizontal,
              child: Row(
                children: [
                  // Clear all filters chip
                  _buildFilterChip(
                    label: 'Clear All',
                    icon: Icons.clear_all,
                    isActive: false,
                    onTap: () {
                      setState(() {
                        _completedFilter = CompletedFilter.showAll;
                        _showInProgress = false;
                        _filterEpisodes();
                      });
                    },
                  ),
                  const SizedBox(width: 8),
                  // Completed filter chip (3-state)
                  _buildFilterChip(
                    label: _getCompletedFilterLabel(),
                    icon: _getCompletedFilterIcon(),
                    isActive: _completedFilter != CompletedFilter.showAll,
                    isAlert: _completedFilter == CompletedFilter.hide,
                    onTap: () {
                      setState(() {
                        // Cycle through states: showAll -> showOnly -> hide -> showAll
                        switch (_completedFilter) {
                          case CompletedFilter.showAll:
                            _completedFilter = CompletedFilter.showOnly;
                            _showInProgress = false;
                            break;
                          case CompletedFilter.showOnly:
                            _completedFilter = CompletedFilter.hide;
                            _showInProgress = false;
                            break;
                          case CompletedFilter.hide:
                            _completedFilter = CompletedFilter.showAll;
                            break;
                        }
                        _filterEpisodes();
                      });
                    },
                  ),
                  const SizedBox(width: 8),
                  // In Progress filter chip
                  _buildFilterChip(
                    label: 'In Progress',
                    icon: Icons.play_circle_outline,
                    isActive: _showInProgress,
                    onTap: () {
                      setState(() {
                        _showInProgress = !_showInProgress;
                        if (_showInProgress) {
                          _completedFilter = CompletedFilter.showAll;
                        }
                        _filterEpisodes();
                      });
                    },
                  ),
                ],
              ),
            ),
          ),
        ],
      ),
    );
  }

  String _getCompletedFilterLabel() {
    switch (_completedFilter) {
      case CompletedFilter.showAll:
        return 'Completed';
      case CompletedFilter.showOnly:
        return 'Completed Only';
      case CompletedFilter.hide:
        return 'Hide Completed';
    }
  }

  IconData _getCompletedFilterIcon() {
    switch (_completedFilter) {
      case CompletedFilter.showAll:
        return Icons.circle_outlined;
      case CompletedFilter.showOnly:
        return Icons.check_circle;
      case CompletedFilter.hide:
        return Icons.cancel;
    }
  }

  Widget _buildFilterChip({
    required String label,
    required IconData icon,
    required bool isActive,
    bool isAlert = false,
    required VoidCallback onTap,
  }) {
    final theme = Theme.of(context);
    final Color backgroundColor;
    final Color foregroundColor;

    if (isAlert) {
      backgroundColor = Colors.orange.withOpacity(0.2);
      foregroundColor = Colors.orange;
    } else if (isActive) {
      backgroundColor = theme.primaryColor.withOpacity(0.2);
      foregroundColor = theme.primaryColor;
    } else {
      backgroundColor = theme.cardColor;
      foregroundColor = theme.textTheme.bodyMedium?.color ?? Colors.grey;
    }

    return Material(
      color: backgroundColor,
      borderRadius: BorderRadius.circular(20),
      child: InkWell(
        borderRadius: BorderRadius.circular(20),
        onTap: onTap,
        child: Container(
          padding: const EdgeInsets.symmetric(horizontal: 12, vertical: 8),
          decoration: BoxDecoration(
            borderRadius: BorderRadius.circular(20),
            border: Border.all(
              color: isActive || isAlert ? foregroundColor : theme.dividerColor,
            ),
          ),
          child: Row(
            mainAxisSize: MainAxisSize.min,
            children: [
              Icon(icon, size: 18, color: foregroundColor),
              const SizedBox(width: 6),
              Text(
                label,
                style: TextStyle(
                  color: foregroundColor,
                  fontWeight: isActive || isAlert ? FontWeight.w600 : FontWeight.normal,
                ),
              ),
            ],
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
          final bool hasValidServerEpisodeId = episode.episodeId > 0;
          
          if (!hasValidServerEpisodeId) {
            print('Episode "${episode.episodeTitle}" has no server ID (RSS fallback) - disabling episode details navigation');
          }
          
          return PinepodsEpisodeCard(
            episode: episode,
            onTap: _isFollowing && hasValidServerEpisodeId ? () {
              // Navigate to episode details only if following AND has valid server episode ID
              Navigator.push(
                context,
                MaterialPageRoute(
                  builder: (context) => PinepodsEpisodeDetails(
                    initialEpisode: episode,
                  ),
                ),
              );
            } : null, // Disable tap if not following or no valid episode ID
            onLongPress: _isFollowing && hasValidServerEpisodeId ? () {
              _showEpisodeContextMenu(originalIndex);
            } : null, // Disable long press if not following or no valid episode ID  
            onPlayPressed: _isFollowing ? () {
              _playEpisode(episode);
            } : null, // Allow play for RSS episodes since it uses direct URL
          );
        },
        childCount: _filteredEpisodes.length,
      ),
    );
  }
}

class _ExpandableDescription extends StatefulWidget {
  final String description;
  const _ExpandableDescription({required this.description});

  @override
  State<_ExpandableDescription> createState() => _ExpandableDescriptionState();
}

class _ExpandableDescriptionState extends State<_ExpandableDescription> {
  bool _expanded = false;

  @override
  Widget build(BuildContext context) {
    return Column(
      crossAxisAlignment: CrossAxisAlignment.start,
      children: [
        Text(
          widget.description,
          style: const TextStyle(fontSize: 14),
          maxLines: _expanded ? null : 3,
          overflow: _expanded ? TextOverflow.visible : TextOverflow.ellipsis,
        ),
        GestureDetector(
          onTap: () => setState(() => _expanded = !_expanded),
          child: Padding(
            padding: const EdgeInsets.only(top: 4),
            child: Text(
              _expanded ? 'Show less' : 'Show more',
              style: TextStyle(
                fontSize: 13,
                color: Theme.of(context).colorScheme.primary,
                fontWeight: FontWeight.w500,
              ),
            ),
          ),
        ),
      ],
    );
  }
}