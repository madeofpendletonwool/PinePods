// lib/ui/pinepods/episode_details.dart
import 'package:flutter/material.dart';
import 'package:pinepods_mobile/bloc/settings/settings_bloc.dart';
import 'package:pinepods_mobile/services/pinepods/pinepods_service.dart';
import 'package:pinepods_mobile/services/pinepods/pinepods_audio_service.dart';
import 'package:pinepods_mobile/services/audio/audio_player_service.dart';
import 'package:pinepods_mobile/services/audio/default_audio_player_service.dart';
import 'package:pinepods_mobile/entities/pinepods_episode.dart';
import 'package:pinepods_mobile/entities/pinepods_search.dart';
import 'package:pinepods_mobile/entities/person.dart';
import 'package:pinepods_mobile/ui/widgets/podcast_html.dart';
import 'package:pinepods_mobile/ui/widgets/episode_description.dart';
import 'package:pinepods_mobile/ui/widgets/podcast_image.dart';
import 'package:pinepods_mobile/ui/pinepods/podcast_details.dart';
import 'package:pinepods_mobile/ui/podcast/mini_player.dart';
import 'package:pinepods_mobile/ui/utils/player_utils.dart';
import 'package:pinepods_mobile/ui/utils/local_download_utils.dart';
import 'package:pinepods_mobile/services/global_services.dart';
import 'package:provider/provider.dart';
import 'package:pinepods_mobile/services/audio/audio_player_service.dart';

class PinepodsEpisodeDetails extends StatefulWidget {
  final PinepodsEpisode initialEpisode;

  const PinepodsEpisodeDetails({
    Key? key,
    required this.initialEpisode,
  }) : super(key: key);

  @override
  State<PinepodsEpisodeDetails> createState() => _PinepodsEpisodeDetailsState();
}

class _PinepodsEpisodeDetailsState extends State<PinepodsEpisodeDetails> {
  final PinepodsService _pinepodsService = PinepodsService();
  // Use global audio service instead of creating local instance
  PinepodsEpisode? _episode;
  bool _isLoading = true;
  String _errorMessage = '';
  List<Person> _persons = [];
  bool _isDownloadedLocally = false;

  @override
  void initState() {
    super.initState();
    _episode = widget.initialEpisode;
    _loadEpisodeDetails();
    _checkLocalDownloadStatus();
  }

  PinepodsAudioService? get _audioService => GlobalServices.pinepodsAudioService;

  Future<void> _checkLocalDownloadStatus() async {
    if (_episode == null) return;
    
    final isDownloaded = await LocalDownloadUtils.isEpisodeDownloadedLocally(context, _episode!);
    
    if (mounted) {
      setState(() {
        _isDownloadedLocally = isDownloaded;
      });
    }
  }

  Future<void> _localDownloadEpisode() async {
    if (_episode == null) return;
    
    final success = await LocalDownloadUtils.localDownloadEpisode(context, _episode!);
    
    if (success) {
      LocalDownloadUtils.showSnackBar(context, 'Episode download started', Colors.green);
      await _checkLocalDownloadStatus(); // Update button state
    } else {
      LocalDownloadUtils.showSnackBar(context, 'Failed to start download', Colors.red);
    }
  }

  Future<void> _deleteLocalDownload() async {
    if (_episode == null) return;
    
    final deletedCount = await LocalDownloadUtils.deleteLocalDownload(context, _episode!);
    
    if (deletedCount > 0) {
      LocalDownloadUtils.showSnackBar(
        context, 
        'Deleted $deletedCount local download${deletedCount > 1 ? 's' : ''}', 
        Colors.orange
      );
      await _checkLocalDownloadStatus(); // Update button state
    } else {
      LocalDownloadUtils.showSnackBar(context, 'Local download not found', Colors.red);
    }
  }

  Future<void> _loadEpisodeDetails() async {
    setState(() {
      _isLoading = true;
      _errorMessage = '';
    });

    try {
      final settingsBloc = Provider.of<SettingsBloc>(context, listen: false);
      final settings = settingsBloc.currentSettings;

      if (settings.pinepodsServer == null || 
          settings.pinepodsApiKey == null || 
          settings.pinepodsUserId == null) {
        setState(() {
          _errorMessage = 'Not connected to PinePods server. Please login first.';
          _isLoading = false;
        });
        return;
      }

      _pinepodsService.setCredentials(settings.pinepodsServer!, settings.pinepodsApiKey!);
      GlobalServices.setCredentials(settings.pinepodsServer!, settings.pinepodsApiKey!);
      final userId = settings.pinepodsUserId!;

      final episodeDetails = await _pinepodsService.getEpisodeMetadata(
        _episode!.episodeId,
        userId,
        isYoutube: _episode!.isYoutube,
        personEpisode: false, // Adjust if needed
      );

      if (episodeDetails != null) {
        // Fetch podcast 2.0 data for persons information
        final podcast2Data = await _pinepodsService.fetchPodcasting2Data(
          episodeDetails.episodeId,
          userId,
        );
        
        List<Person> persons = [];
        if (podcast2Data != null) {
          final personsData = podcast2Data['people'] as List<dynamic>?;
          if (personsData != null) {
            try {
              persons = personsData.map((personData) {
                return Person(
                  name: personData['name'] ?? '',
                  role: personData['role'] ?? '',
                  group: personData['group'] ?? '',
                  image: personData['img'],
                  link: personData['href'],
                );
              }).toList();
              print('Loaded ${persons.length} persons from episode 2.0 data');
            } catch (e) {
              print('Error parsing persons data: $e');
            }
          }
        }
        
        setState(() {
          _episode = episodeDetails;
          _persons = persons;
          _isLoading = false;
        });
      } else {
        setState(() {
          _errorMessage = 'Failed to load episode details';
          _isLoading = false;
        });
      }
    } catch (e) {
      setState(() {
        _errorMessage = 'Error loading episode details: ${e.toString()}';
        _isLoading = false;
      });
    }
  }

  bool _isCurrentEpisodePlaying() {
    try {
      final audioPlayerService = Provider.of<AudioPlayerService>(context, listen: false);
      final currentEpisode = audioPlayerService.nowPlaying;
      return currentEpisode != null && currentEpisode.guid == _episode!.episodeUrl;
    } catch (e) {
      return false;
    }
  }

  bool _isAudioPlaying() {
    try {
      final audioPlayerService = Provider.of<AudioPlayerService>(context, listen: false);
      // This method is no longer needed since we're using StreamBuilder
      return false;
    } catch (e) {
      return false;
    }
  }

  Future<void> _togglePlayPause() async {
    
    if (_audioService == null) {
      _showSnackBar('Audio service not available', Colors.red);
      return;
    }

    try {
      final audioPlayerService = Provider.of<AudioPlayerService>(context, listen: false);
      
      // Check if this episode is currently playing
      if (_isCurrentEpisodePlaying()) {
        // This episode is loaded, check current state and toggle
        final currentState = audioPlayerService.playingState;
        if (currentState != null) {
          // Listen to the current state
          final state = await currentState.first;
          if (state == AudioState.playing) {
            await audioPlayerService.pause();
          } else {
            await audioPlayerService.play();
          }
        } else {
          await audioPlayerService.play();
        }
      } else {
        // Start playing this episode
        await playPinepodsEpisodeWithOptionalFullScreen(
          context,
          _audioService!,
          _episode!,
          resume: _episode!.isStarted,
        );
      }
    } catch (e) {
      _showSnackBar('Failed to control playback: ${e.toString()}', Colors.red);
    }
  }

  Future<void> _handleTimestampTap(Duration timestamp) async {
    
    if (_audioService == null) {
      _showSnackBar('Audio service not available', Colors.red);
      return;
    }

    try {
      final audioPlayerService = Provider.of<AudioPlayerService>(context, listen: false);
      
      // Check if this episode is currently playing
      final currentEpisode = audioPlayerService.nowPlaying;
      final isCurrentEpisode = currentEpisode != null && 
          currentEpisode.guid == _episode!.episodeUrl;
      
      if (!isCurrentEpisode) {
        // Start playing the episode first
        await playPinepodsEpisodeWithOptionalFullScreen(
          context,
          _audioService!,
          _episode!,
          resume: false, // Start from beginning initially
        );
        
        // Wait a moment for the episode to start loading
        await Future.delayed(const Duration(milliseconds: 500));
      }
      
      // Seek to the timestamp (convert Duration to seconds as int)
      await audioPlayerService.seek(position: timestamp.inSeconds);
      
    } catch (e) {
      _showSnackBar('Failed to jump to timestamp: ${e.toString()}', Colors.red);
    }
  }

  String _formatDuration(Duration duration) {
    final hours = duration.inHours;
    final minutes = duration.inMinutes.remainder(60);
    final seconds = duration.inSeconds.remainder(60);
    
    if (hours > 0) {
      return '${hours.toString().padLeft(1, '0')}:${minutes.toString().padLeft(2, '0')}:${seconds.toString().padLeft(2, '0')}';
    } else {
      return '${minutes.toString().padLeft(1, '0')}:${seconds.toString().padLeft(2, '0')}';
    }
  }

  Future<void> _saveEpisode() async {
    final settingsBloc = Provider.of<SettingsBloc>(context, listen: false);
    final settings = settingsBloc.currentSettings;
    final userId = settings.pinepodsUserId;

    if (userId == null) {
      _showSnackBar('Not logged in', Colors.red);
      return;
    }

    try {
      final success = await _pinepodsService.saveEpisode(
        _episode!.episodeId,
        userId,
        _episode!.isYoutube,
      );

      if (success) {
        setState(() {
          _episode = _updateEpisodeProperty(_episode!, saved: true);
        });
        _showSnackBar('Episode saved!', Colors.green);
      } else {
        _showSnackBar('Failed to save episode', Colors.red);
      }
    } catch (e) {
      _showSnackBar('Error saving episode: $e', Colors.red);
    }
  }

  Future<void> _removeSavedEpisode() async {
    final settingsBloc = Provider.of<SettingsBloc>(context, listen: false);
    final settings = settingsBloc.currentSettings;
    final userId = settings.pinepodsUserId;

    if (userId == null) {
      _showSnackBar('Not logged in', Colors.red);
      return;
    }

    try {
      final success = await _pinepodsService.removeSavedEpisode(
        _episode!.episodeId,
        userId,
        _episode!.isYoutube,
      );

      if (success) {
        setState(() {
          _episode = _updateEpisodeProperty(_episode!, saved: false);
        });
        _showSnackBar('Removed from saved episodes', Colors.orange);
      } else {
        _showSnackBar('Failed to remove saved episode', Colors.red);
      }
    } catch (e) {
      _showSnackBar('Error removing saved episode: $e', Colors.red);
    }
  }

  Future<void> _toggleQueue() async {
    final settingsBloc = Provider.of<SettingsBloc>(context, listen: false);
    final settings = settingsBloc.currentSettings;
    final userId = settings.pinepodsUserId;

    if (userId == null) {
      _showSnackBar('Not logged in', Colors.red);
      return;
    }

    try {
      bool success;
      if (_episode!.queued) {
        success = await _pinepodsService.removeQueuedEpisode(
          _episode!.episodeId,
          userId,
          _episode!.isYoutube,
        );
        if (success) {
          setState(() {
            _episode = _updateEpisodeProperty(_episode!, queued: false);
          });
          _showSnackBar('Removed from queue', Colors.orange);
        }
      } else {
        success = await _pinepodsService.queueEpisode(
          _episode!.episodeId,
          userId,
          _episode!.isYoutube,
        );
        if (success) {
          setState(() {
            _episode = _updateEpisodeProperty(_episode!, queued: true);
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
  }

  Future<void> _toggleDownload() async {
    final settingsBloc = Provider.of<SettingsBloc>(context, listen: false);
    final settings = settingsBloc.currentSettings;
    final userId = settings.pinepodsUserId;

    if (userId == null) {
      _showSnackBar('Not logged in', Colors.red);
      return;
    }

    try {
      bool success;
      if (_episode!.downloaded) {
        success = await _pinepodsService.deleteEpisode(
          _episode!.episodeId,
          userId,
          _episode!.isYoutube,
        );
        if (success) {
          setState(() {
            _episode = _updateEpisodeProperty(_episode!, downloaded: false);
          });
          _showSnackBar('Episode deleted from server', Colors.orange);
        }
      } else {
        success = await _pinepodsService.downloadEpisode(
          _episode!.episodeId,
          userId,
          _episode!.isYoutube,
        );
        if (success) {
          setState(() {
            _episode = _updateEpisodeProperty(_episode!, downloaded: true);
          });
          _showSnackBar('Episode download queued!', Colors.green);
        }
      }

      if (!success) {
        _showSnackBar('Failed to update download', Colors.red);
      }
    } catch (e) {
      _showSnackBar('Error updating download: $e', Colors.red);
    }
  }

  Future<void> _toggleComplete() async {
    final settingsBloc = Provider.of<SettingsBloc>(context, listen: false);
    final settings = settingsBloc.currentSettings;
    final userId = settings.pinepodsUserId;

    if (userId == null) {
      _showSnackBar('Not logged in', Colors.red);
      return;
    }

    try {
      bool success;
      if (_episode!.completed) {
        success = await _pinepodsService.markEpisodeUncompleted(
          _episode!.episodeId,
          userId,
          _episode!.isYoutube,
        );
        if (success) {
          setState(() {
            _episode = _updateEpisodeProperty(_episode!, completed: false);
          });
          _showSnackBar('Marked as incomplete', Colors.orange);
        }
      } else {
        success = await _pinepodsService.markEpisodeCompleted(
          _episode!.episodeId,
          userId,
          _episode!.isYoutube,
        );
        if (success) {
          setState(() {
            _episode = _updateEpisodeProperty(_episode!, completed: true);
          });
          _showSnackBar('Marked as complete!', Colors.green);
        }
      }

      if (!success) {
        _showSnackBar('Failed to update completion status', Colors.red);
      }
    } catch (e) {
      _showSnackBar('Error updating completion: $e', Colors.red);
    }
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

  void _showSnackBar(String message, Color backgroundColor) {
    ScaffoldMessenger.of(context).showSnackBar(
      SnackBar(
        content: Text(message),
        backgroundColor: backgroundColor,
        duration: const Duration(seconds: 2),
      ),
    );
  }

  Future<void> _navigateToPodcast() async {
    if (_episode!.podcastId == null) {
      _showSnackBar('Podcast ID not available', Colors.orange);
      return;
    }

    try {
      final settingsBloc = Provider.of<SettingsBloc>(context, listen: false);
      final settings = settingsBloc.currentSettings;
      final userId = settings.pinepodsUserId;

      if (userId == null) {
        _showSnackBar('Not logged in', Colors.red);
        return;
      }

      // Fetch the actual podcast details to get correct episode count
      final podcastDetails = await _pinepodsService.getPodcastDetailsById(_episode!.podcastId!, userId);
      
      final podcast = UnifiedPinepodsPodcast(
        id: _episode!.podcastId!,
        indexId: 0,
        title: _episode!.podcastName,
        url: podcastDetails?['feedurl'] ?? '',
        originalUrl: podcastDetails?['feedurl'] ?? '',
        link: podcastDetails?['websiteurl'] ?? '',
        description: podcastDetails?['description'] ?? '',
        author: podcastDetails?['author'] ?? '',
        ownerName: podcastDetails?['author'] ?? '',
        image: podcastDetails?['artworkurl'] ?? _episode!.episodeArtwork,
        artwork: podcastDetails?['artworkurl'] ?? _episode!.episodeArtwork,
        lastUpdateTime: 0,
        explicit: podcastDetails?['explicit'] ?? false,
        episodeCount: podcastDetails?['episodecount'] ?? 0,
      );
      
      // Navigate to podcast details - same as podcast tile does  
      Navigator.push(
        context,
        MaterialPageRoute<void>(
          settings: const RouteSettings(name: 'pinepods_podcast_details'),
          builder: (context) => PinepodsPodcastDetails(
            podcast: podcast,
            isFollowing: true, // Assume following since we have a podcast ID
          ),
        ),
      );
    } catch (e) {
      _showSnackBar('Error navigating to podcast: $e', Colors.red);
    }
  }

  @override
  Widget build(BuildContext context) {
    if (_isLoading) {
      return Scaffold(
        appBar: AppBar(
          title: const Text('Episode Details'),
        ),
        body: const Center(
          child: Column(
            mainAxisAlignment: MainAxisAlignment.center,
            children: [
              CircularProgressIndicator(),
              SizedBox(height: 16),
              Text('Loading episode details...'),
            ],
          ),
        ),
      );
    }

    if (_errorMessage.isNotEmpty) {
      return Scaffold(
        appBar: AppBar(
          title: const Text('Episode Details'),
        ),
        body: Center(
          child: Padding(
            padding: const EdgeInsets.all(16.0),
            child: Column(
              mainAxisAlignment: MainAxisAlignment.center,
              children: [
                Icon(
                  Icons.error_outline,
                  color: Theme.of(context).colorScheme.error,
                  size: 48,
                ),
                const SizedBox(height: 16),
                Text(
                  _errorMessage,
                  style: TextStyle(
                    color: Theme.of(context).colorScheme.error,
                  ),
                  textAlign: TextAlign.center,
                ),
                const SizedBox(height: 16),
                ElevatedButton(
                  onPressed: _loadEpisodeDetails,
                  child: const Text('Retry'),
                ),
              ],
            ),
          ),
        ),
      );
    }

    return Scaffold(
      appBar: AppBar(
        title: Text(_episode!.podcastName),
        elevation: 0,
      ),
      body: Column(
        children: [
          Expanded(
            child: SingleChildScrollView(
              padding: const EdgeInsets.all(16.0),
              child: Column(
                crossAxisAlignment: CrossAxisAlignment.start,
                children: [
            // Episode artwork and basic info
            Row(
              crossAxisAlignment: CrossAxisAlignment.start,
              children: [
                // Episode artwork
                ClipRRect(
                  borderRadius: BorderRadius.circular(8),
                  child: _episode!.episodeArtwork.isNotEmpty
                      ? Image.network(
                          _episode!.episodeArtwork,
                          width: 120,
                          height: 120,
                          fit: BoxFit.cover,
                          errorBuilder: (context, error, stackTrace) {
                            return Container(
                              width: 120,
                              height: 120,
                              decoration: BoxDecoration(
                                color: Colors.grey[300],
                                borderRadius: BorderRadius.circular(8),
                              ),
                              child: const Icon(
                                Icons.music_note,
                                color: Colors.grey,
                                size: 48,
                              ),
                            );
                          },
                        )
                      : Container(
                          width: 120,
                          height: 120,
                          decoration: BoxDecoration(
                            color: Colors.grey[300],
                            borderRadius: BorderRadius.circular(8),
                          ),
                          child: const Icon(
                            Icons.music_note,
                            color: Colors.grey,
                            size: 48,
                          ),
                        ),
                ),
                const SizedBox(width: 16),
                // Episode info
                Expanded(
                  child: Column(
                    crossAxisAlignment: CrossAxisAlignment.start,
                    children: [
                      // Clickable podcast name
                      GestureDetector(
                        onTap: () => _navigateToPodcast(),
                        child: Text(
                          _episode!.podcastName,
                          style: Theme.of(context).textTheme.titleMedium!.copyWith(
                            color: Theme.of(context).primaryColor,
                            fontWeight: FontWeight.w500,
                            decoration: TextDecoration.underline,
                            decorationColor: Theme.of(context).primaryColor,
                          ),
                          maxLines: 2,
                          overflow: TextOverflow.ellipsis,
                        ),
                      ),
                      const SizedBox(height: 4),
                      Text(
                        _episode!.episodeTitle,
                        style: Theme.of(context).textTheme.titleLarge!.copyWith(
                          fontWeight: FontWeight.bold,
                        ),
                        maxLines: 3,
                        overflow: TextOverflow.ellipsis,
                      ),
                      const SizedBox(height: 8),
                      Text(
                        _episode!.formattedDuration,
                        style: Theme.of(context).textTheme.bodyMedium!.copyWith(
                          color: Colors.grey[600],
                        ),
                      ),
                      const SizedBox(height: 4),
                      Text(
                        _episode!.formattedPubDate,
                        style: Theme.of(context).textTheme.bodyMedium!.copyWith(
                          color: Colors.grey[600],
                        ),
                      ),
                      if (_episode!.isStarted) ...[
                        const SizedBox(height: 8),
                        Text(
                          'Listened: ${_episode!.formattedListenDuration}',
                          style: Theme.of(context).textTheme.bodySmall!.copyWith(
                            color: Theme.of(context).primaryColor,
                          ),
                        ),
                        const SizedBox(height: 4),
                        LinearProgressIndicator(
                          value: _episode!.progressPercentage / 100,
                          backgroundColor: Colors.grey[300],
                          valueColor: AlwaysStoppedAnimation<Color>(
                            Theme.of(context).primaryColor,
                          ),
                        ),
                      ],
                    ],
                  ),
                ),
              ],
            ),
            
            const SizedBox(height: 24),
            
            // Action buttons
            Column(
              children: [
                // First row: Play, Save, Queue (3 buttons, each 1/3 width)
                Row(
                  children: [
                    // Play/Pause button
                    Expanded(
                      child: StreamBuilder<AudioState>(
                        stream: Provider.of<AudioPlayerService>(context, listen: false).playingState,
                        builder: (context, snapshot) {
                          final isCurrentEpisode = _isCurrentEpisodePlaying();
                          final isPlaying = snapshot.data == AudioState.playing;
                          final isCurrentlyPlaying = isCurrentEpisode && isPlaying;
                          
                          IconData icon;
                          String label;
                          
                          if (_episode!.completed) {
                            icon = Icons.replay;
                            label = 'Replay';
                          } else if (isCurrentlyPlaying) {
                            icon = Icons.pause;
                            label = 'Pause';
                          } else {
                            icon = Icons.play_arrow;
                            label = 'Play';
                          }
                          
                          return OutlinedButton.icon(
                            onPressed: _togglePlayPause,
                            icon: Icon(icon),
                            label: Text(label),
                          );
                        },
                      ),
                    ),
                    const SizedBox(width: 8),
                    
                    // Save/Unsave button
                    Expanded(
                      child: OutlinedButton.icon(
                        onPressed: _episode!.saved ? _removeSavedEpisode : _saveEpisode,
                        icon: Icon(
                          _episode!.saved ? Icons.bookmark : Icons.bookmark_outline,
                          color: _episode!.saved ? Colors.orange : null,
                        ),
                        label: Text(_episode!.saved ? 'Saved' : 'Save'),
                      ),
                    ),
                    const SizedBox(width: 8),
                    
                    // Queue button
                    Expanded(
                      child: OutlinedButton.icon(
                        onPressed: _toggleQueue,
                        icon: Icon(
                          _episode!.queued ? Icons.queue_music : Icons.queue_music_outlined,
                          color: _episode!.queued ? Colors.purple : null,
                        ),
                        label: Text(_episode!.queued ? 'Queued' : 'Queue'),
                      ),
                    ),
                  ],
                ),
                
                const SizedBox(height: 8),
                
                // Second row: Download, Complete (2 buttons, each 1/2 width)
                Row(
                  children: [
                    // Download button
                    Expanded(
                      child: OutlinedButton.icon(
                        onPressed: _toggleDownload,
                        icon: Icon(
                          _episode!.downloaded ? Icons.download_done : Icons.download_outlined,
                          color: _episode!.downloaded ? Colors.blue : null,
                        ),
                        label: Text(_episode!.downloaded ? 'Downloaded' : 'Download'),
                      ),
                    ),
                    const SizedBox(width: 8),
                    
                    // Complete button
                    Expanded(
                      child: OutlinedButton.icon(
                        onPressed: _toggleComplete,
                        icon: Icon(
                          _episode!.completed ? Icons.check_circle : Icons.check_circle_outline,
                          color: _episode!.completed ? Colors.green : null,
                        ),
                        label: Text(_episode!.completed ? 'Complete' : 'Mark Complete'),
                      ),
                    ),
                  ],
                ),
                
                const SizedBox(height: 8),
                
                // Third row: Local Download (full width)
                Row(
                  children: [
                    Expanded(
                      child: OutlinedButton.icon(
                        onPressed: _isDownloadedLocally ? _deleteLocalDownload : _localDownloadEpisode,
                        icon: Icon(
                          _isDownloadedLocally ? Icons.delete_forever_outlined : Icons.file_download_outlined,
                          color: _isDownloadedLocally ? Colors.red : Colors.green,
                        ),
                        label: Text(_isDownloadedLocally ? 'Delete Local Download' : 'Download Locally'),
                        style: OutlinedButton.styleFrom(
                          side: BorderSide(
                            color: _isDownloadedLocally ? Colors.red : Colors.green,
                          ),
                        ),
                      ),
                    ),
                  ],
                ),
              ],
            ),
            
            // Hosts/Guests section
            if (_persons.isNotEmpty) ...[
              const SizedBox(height: 24),
              Align(
                alignment: Alignment.centerLeft,
                child: Text(
                  'Hosts & Guests',
                  style: Theme.of(context).textTheme.titleMedium!.copyWith(
                    fontWeight: FontWeight.bold,
                  ),
                ),
              ),
              const SizedBox(height: 12),
              SizedBox(
                height: 80,
                child: ListView.builder(
                  scrollDirection: Axis.horizontal,
                  itemCount: _persons.length,
                  itemBuilder: (context, index) {
                    final person = _persons[index];
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
                            child: person.image != null && person.image!.isNotEmpty
                                ? ClipRRect(
                                    borderRadius: BorderRadius.circular(25),
                                    child: PodcastImage(
                                      url: person.image!,
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
                            person.name,
                            style: Theme.of(context).textTheme.bodySmall,
                            textAlign: TextAlign.center,
                            maxLines: 1,
                            overflow: TextOverflow.ellipsis,
                          ),
                        ],
                      ),
                    );
                  },
                ),
              ),
            ],
            
            const SizedBox(height: 32),
            
            // Episode description
            Text(
              'Description',
              style: Theme.of(context).textTheme.titleMedium!.copyWith(
                fontWeight: FontWeight.bold,
              ),
            ),
            const SizedBox(height: 12),
            EpisodeDescription(
              content: _episode!.episodeDescription,
              onTimestampTap: _handleTimestampTap,
            ),
                  ],
                ),
              ),
            ),
            const MiniPlayer(),
          ],
        ),
    );
  }

  @override
  void dispose() {
    // Don't dispose global audio service - it should persist across pages
    super.dispose();
  }
}