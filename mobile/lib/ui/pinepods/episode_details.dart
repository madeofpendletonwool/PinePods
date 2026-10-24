// lib/ui/pinepods/episode_details.dart
import 'dart:async';
import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:share_plus/share_plus.dart';
import 'package:pinepods_mobile/bloc/settings/settings_bloc.dart';
import 'package:pinepods_mobile/services/pinepods/pinepods_service.dart';
import 'package:pinepods_mobile/services/pinepods/pinepods_audio_service.dart';
import 'package:pinepods_mobile/services/audio/audio_player_service.dart';
import 'package:pinepods_mobile/entities/pinepods_episode.dart';
import 'package:pinepods_mobile/entities/pending_action.dart';
import 'package:pinepods_mobile/entities/pinepods_search.dart';
import 'package:pinepods_mobile/entities/people.dart';
import 'package:pinepods_mobile/ui/widgets/podcast_html.dart';
import 'package:pinepods_mobile/ui/widgets/episode_description.dart';
import 'package:pinepods_mobile/ui/pinepods/podcast_details.dart';
import 'package:pinepods_mobile/ui/pinepods/podcast_nav.dart';
import 'package:pinepods_mobile/ui/pinepods/episode_ai_section.dart';
import 'package:pinepods_mobile/ui/pinepods/people/add_people_sheet.dart';
import 'package:pinepods_mobile/ui/pinepods/people/people_widgets.dart';
import 'package:pinepods_mobile/ui/utils/player_utils.dart';
import 'package:pinepods_mobile/ui/utils/local_download_utils.dart';
import 'package:pinepods_mobile/ui/utils/action_guard.dart';
import 'package:pinepods_mobile/ui/utils/artwork_colors.dart';
import 'package:pinepods_mobile/ui/widgets/lazy_network_image.dart';
import 'package:pinepods_mobile/ui/widgets/soft_ui.dart';
import 'package:pinepods_mobile/ui/widgets/pinepods_episode_card.dart' show humanDuration;
import 'package:pinepods_mobile/services/global_services.dart';
import 'package:provider/provider.dart';
import 'package:pinepods_mobile/services/audio/audio_player_service.dart';
import 'package:pinepods_mobile/ui/icons/phosphor_icons.dart';

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
  List<CreditedPerson> _people = [];
  // True once people loaded from a server that supports adding them.
  bool _canAddPeople = false;
  bool _isDownloadedLocally = false;
  int? _userId; // set once credentials are established in _loadEpisodeDetails

  // Play-button loading feedback: true while we've kicked off playback for this
  // episode but the audio system hasn't yet acknowledged it as now-playing.
  bool _isStartingPlayback = false;
  Color? _artColor;
  AudioPlayerService? _audioPlayerService;
  StreamSubscription? _episodeSub;
  StreamSubscription? _stateSub;

  /// Guards the action buttons (play, save, queue, download, complete, local
  /// download) against re-entrant taps. Each of those handlers awaits at
  /// least one network call with no immediate visual feedback, so without
  /// this a slow response invites the user to tap again - firing a second,
  /// overlapping request that can race with the first (e.g. double-toggling
  /// a save/queue state).
  final ActionGuard _actionGuard = ActionGuard();

  @override
  void initState() {
    super.initState();
    _episode = widget.initialEpisode;
    _loadEpisodeDetails();
    _checkLocalDownloadStatus();
  }

  @override
  void didChangeDependencies() {
    super.didChangeDependencies();
    if (_audioPlayerService != null) return;
    _resolveArtColor();
    final aps = Provider.of<AudioPlayerService>(context, listen: false);
    _audioPlayerService = aps;

    // Clear the loading state once the audio system reports this episode as the
    // now-playing one (mirrors PinepodsEpisodeCard's behaviour).
    _episodeSub = aps.episodeEvent?.listen((episode) {
      if (!mounted) return;
      if (episode != null &&
          _episode != null &&
          episode.guid == _episode!.episodeUrl) {
        if (_isStartingPlayback) {
          setState(() => _isStartingPlayback = false);
        }
      }
    });

    // Safety: never leave the spinner stuck if playback errors out.
    _stateSub = aps.playingState?.listen((state) {
      if (!mounted) return;
      if (state == AudioState.error && _isStartingPlayback) {
        setState(() => _isStartingPlayback = false);
      }
    });
  }

  PinepodsAudioService? get _audioService => GlobalServices.pinepodsAudioService;

  /// Run [action] only if no other guarded action is currently in flight.
  Future<void> _runGuarded(Future<void> Function() action) {
    return _actionGuard.run(action, onChange: () {
      if (mounted) setState(() {});
    });
  }

  /// Enqueue an interaction in the offline outbox so it syncs when back online,
  /// used when a direct server call fails (e.g. the device is offline). Returns
  /// true if it was queued.
  Future<bool> _enqueueOffline(PendingActionType type, int userId) async {
    final queue = GlobalServices.offlineActionQueue;
    if (queue == null) return false;
    await queue.enqueueSimple(type, _episode!.episodeId, userId, _episode!.isYoutube);
    return true;
  }

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

  Future<void> _shareEpisode() async {
    if (_episode == null) return;

    final settings = Provider.of<SettingsBloc>(context, listen: false).currentSettings;
    if (settings.pinepodsServer == null || settings.pinepodsApiKey == null) {
      _showSnackBar('Not connected to a server', Colors.red);
      return;
    }

    _pinepodsService.setCredentials(settings.pinepodsServer!, settings.pinepodsApiKey!);
    _showSnackBar('Creating share link…', Colors.blueGrey);

    try {
      final urlKey = await _pinepodsService.createShareLink(_episode!.episodeId);
      final shareUrl = '${settings.pinepodsServer}/shared_episode/$urlKey';
      if (!mounted) return;
      await _showShareDialog(shareUrl);
    } catch (e) {
      if (mounted) _showSnackBar('Failed to create share link: $e', Colors.red);
    }
  }

  Future<void> _showShareDialog(String shareUrl) async {
    await showDialog<void>(
      context: context,
      builder: (dialogContext) => AlertDialog(
        title: const Text('Share Episode'),
        content: Column(
          mainAxisSize: MainAxisSize.min,
          crossAxisAlignment: CrossAxisAlignment.start,
          children: [
            const Text(
              'Anyone with this link can listen to the episode. The link expires in 60 days.',
            ),
            const SizedBox(height: 12),
            SelectableText(
              shareUrl,
              style: Theme.of(dialogContext).textTheme.bodySmall,
            ),
          ],
        ),
        actions: [
          TextButton.icon(
            onPressed: () {
              Clipboard.setData(ClipboardData(text: shareUrl));
              _showSnackBar('Link copied', Colors.green);
            },
            icon: const Icon(PhosphorIconsRegular.copy),
            label: const Text('Copy'),
          ),
          TextButton.icon(
            onPressed: () {
              SharePlus.instance.share(
                ShareParams(text: shareUrl, subject: _episode?.episodeTitle),
              );
            },
            icon: const Icon(PhosphorIconsRegular.export),
            label: const Text('Share'),
          ),
          TextButton(
            onPressed: () => Navigator.of(dialogContext).pop(),
            child: const Text('Close'),
          ),
        ],
      ),
    );
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
        // No server connection: still show the episode we were given (e.g. a
        // local download opened while offline) rather than an error page.
        setState(() {
          _isLoading = false;
        });
        return;
      }

      _pinepodsService.setCredentials(settings.pinepodsServer!, settings.pinepodsApiKey!);
      GlobalServices.setCredentials(settings.pinepodsServer!, settings.pinepodsApiKey!);
      final userId = settings.pinepodsUserId!;
      _userId = userId;

      // People load on their own: they're a quick database read, while the
      // metadata can wait on the server.
      unawaited(_loadPeople(userId));

      final episodeDetails = await _pinepodsService.getEpisodeMetadata(
        _episode!.episodeId,
        userId,
        isYoutube: _episode!.isYoutube,
        personEpisode: false, // Adjust if needed
      );

      if (episodeDetails != null) {
        setState(() {
          _episode = episodeDetails;
          _isLoading = false;
        });
        _resolveArtColor();
      } else {
        // Metadata unavailable: fall back to the episode passed in so the page
        // (and playback of a local download) still works.
        setState(() {
          _isLoading = false;
        });
      }
    } catch (e) {
      // Likely offline. Keep the initial episode so local downloads remain
      // playable; only surface an error if we have nothing to show.
      setState(() {
        if (_episode == null) {
          _errorMessage = 'Error loading episode details: ${e.toString()}';
        }
        _isLoading = false;
      });
    }
  }

  Future<void> _loadPeople(int userId) async {
    final episode = _episode;
    if (episode == null || episode.episodeId <= 0 || episode.isYoutube) return;
    try {
      final people = await _pinepodsService.getEpisodePeople(episode.episodeId);
      if (people != null) {
        if (mounted) {
          setState(() {
            _people = people;
            _canAddPeople = true;
          });
        }
        return;
      }
      // Servers before the people endpoints: read-only, from the 2.0 data.
      final data = await _pinepodsService.fetchPodcasting2Data(episode.episodeId, userId);
      final legacy = (data?['people'] as List<dynamic>? ?? const [])
          .map((e) => CreditedPerson.fromJson(e as Map<String, dynamic>))
          .where((p) => p.name.isNotEmpty && p.name != 'Unknown Host')
          .toList();
      if (mounted) setState(() => _people = legacy);
    } catch (e) {
      debugPrint('Error loading episode people: $e');
    }
  }

  Future<void> _addPeople() async {
    final podcastId = _episode?.podcastId;
    if (podcastId == null || podcastId <= 0) return;
    final added = await showAddPeopleSheet(context, podcastId: podcastId, episodeId: _episode!.episodeId);
    if (added != null && mounted) setState(() => _people = added);
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
        // Start playing this episode. Show loading feedback until the audio
        // system acknowledges it (cleared by the episodeEvent subscription).
        setState(() => _isStartingPlayback = true);
        await playPinepodsEpisodeWithOptionalFullScreen(
          context,
          _audioService!,
          _episode!,
          resume: _episode!.isStarted,
        );
      }
    } catch (e) {
      if (mounted) {
        setState(() => _isStartingPlayback = false);
      }
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

  // Re-supply the native player with this episode's skip segments after a
  // review/detect change — but only if it is the one currently playing, so we
  // never push a different episode's ad ranges onto the active player.
  Future<void> _resupplySkipSegmentsIfPlaying() async {
    final aps = _audioPlayerService ??
        Provider.of<AudioPlayerService>(context, listen: false);
    final current = aps.nowPlaying;
    if (current == null || current.guid != _episode!.episodeUrl) return;
    final podcastId = _episode!.podcastId;
    if (podcastId == null || _userId == null) return;
    await _audioService?.refreshSkipSegments(
        _episode!.episodeId, _userId!, podcastId);
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
      if (await _enqueueOffline(PendingActionType.saveEpisode, userId)) {
        setState(() => _episode = _updateEpisodeProperty(_episode!, saved: true));
        _showSnackBar('Saved — will sync when online', Colors.blueGrey);
      } else {
        _showSnackBar('Error saving episode: $e', Colors.red);
      }
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
      if (await _enqueueOffline(PendingActionType.removeSaved, userId)) {
        setState(() => _episode = _updateEpisodeProperty(_episode!, saved: false));
        _showSnackBar('Removed — will sync when online', Colors.blueGrey);
      } else {
        _showSnackBar('Error removing saved episode: $e', Colors.red);
      }
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
      final wasCompleted = _episode!.completed;
      final type = wasCompleted ? PendingActionType.markUncompleted : PendingActionType.markCompleted;
      if (await _enqueueOffline(type, userId)) {
        setState(() => _episode = _updateEpisodeProperty(_episode!, completed: !wasCompleted));
        _showSnackBar(
          wasCompleted ? 'Marked incomplete — will sync when online' : 'Marked complete — will sync when online',
          Colors.blueGrey,
        );
      } else {
        _showSnackBar('Error updating completion: $e', Colors.red);
      }
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

    await navigateToPodcastById(
      context,
      _episode!.podcastId!,
      fallbackTitle: _episode!.podcastName,
      fallbackArtwork: _episode!.episodeArtwork,
    );
  }

  @override
  Widget build(BuildContext context) {
    // No loading screen: the episode we were handed renders immediately and
    // the full metadata fills in when it arrives.
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
                  PhosphorIconsRegular.warningCircle,
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

    final theme = Theme.of(context);
    // Eases the header between colours when fuller artwork arrives.
    return TweenAnimationBuilder<Color?>(
      tween: ColorTween(end: _accentColor(theme)),
      duration: const Duration(milliseconds: 350),
      builder: (context, accent, _) => _buildPage(theme, accent!),
    );
  }

  Widget _buildPage(ThemeData theme, Color accent) {
    final bg = theme.scaffoldBackgroundColor;
    final headerTint = Color.alphaBlend(accent.withOpacity(0.18), bg);

    return Scaffold(
      appBar: AppBar(
        backgroundColor: headerTint,
        surfaceTintColor: Colors.transparent,
        elevation: 0,
        scrolledUnderElevation: 0,
        actions: [
          IconButton(
            tooltip: 'Share',
            icon: const Icon(PhosphorIconsRegular.shareNetwork),
            onPressed: _shareEpisode,
          ),
        ],
      ),
      body: SafeArea(
        top: false,
        child: Column(
          children: [
            Expanded(
              child: SingleChildScrollView(
                child: Column(
                  crossAxisAlignment: CrossAxisAlignment.stretch,
                  children: [
                    _buildHeader(theme, accent, headerTint, bg),
                    Padding(
                      padding: const EdgeInsets.fromLTRB(16, 4, 16, 16),
                      child: Column(
                        crossAxisAlignment: CrossAxisAlignment.stretch,
                        children: [
                          _buildPlayButton(accent),
                          const SizedBox(height: 12),
                          _buildActionStrip(theme, accent),
                          if (_people.isNotEmpty || _canAddPeople) ...[
                            const SizedBox(height: 20),
                            PeopleStrip(
                              title: 'In This Episode',
                              people: _people,
                              onAdd: _canAddPeople && (_episode!.podcastId ?? 0) > 0 ? _addPeople : null,
                              emptyPrompt: 'Know who\'s in this episode? Add the hosts and guests.',
                            ),
                          ],
                          const SizedBox(height: 24),
                          Text(
                            'Description',
                            style: theme.textTheme.titleMedium!.copyWith(fontWeight: FontWeight.bold),
                          ),
                          const SizedBox(height: 8),
                          if (_isLoading && _episode!.episodeDescription.isEmpty)
                            const Padding(
                              padding: EdgeInsets.symmetric(vertical: 24),
                              child: Center(
                                child: SizedBox(
                                  width: 22,
                                  height: 22,
                                  child: CircularProgressIndicator(strokeWidth: 2.5),
                                ),
                              ),
                            )
                          else
                            EpisodeDescription(
                              content: _episode!.episodeDescription,
                              onTimestampTap: _handleTimestampTap,
                            ),
                          // AI features (#726 transcript / #790 ad-block). Self-hides unless
                          // the server reports the AI sidecar as available.
                          if (_userId != null)
                            EpisodeAiSection(
                              episode: _episode!,
                              userId: _userId!,
                              pinepodsService: _pinepodsService,
                              onSeek: _handleTimestampTap,
                              onSegmentsChanged: _resupplySkipSegmentsIfPlaying,
                            ),
                        ],
                      ),
                    ),
                  ],
                ),
              ),
            ),
          ],
        ),
      ),
    );
  }

  /// Artwork-derived accent when that setting is on, else the theme's.
  Color _accentColor(ThemeData theme) {
    final enabled = Provider.of<SettingsBloc>(context, listen: false).currentSettings.artworkAccentColors;
    return (enabled ? _artColor : null) ?? theme.primaryColor;
  }

  void _resolveArtColor() {
    final settings = Provider.of<SettingsBloc>(context, listen: false).currentSettings;
    if (!settings.artworkAccentColors || _episode == null) return;
    final brightness = ThemeData.estimateBrightnessForColor(Theme.of(context).scaffoldBackgroundColor);
    final url = _episode!.episodeArtwork;
    _artColor = ArtworkColors.cached(url, null, brightness);
    if (_artColor != null) return;
    ArtworkColors.resolve(url, null, brightness).then((color) {
      if (!mounted || color == null || url != _episode?.episodeArtwork) return;
      setState(() => _artColor = color);
    });
  }

  Widget _buildHeader(ThemeData theme, Color accent, Color headerTint, Color bg) {
    final muted = theme.hintColor;
    final meta = [
      _episode!.formattedPubDate,
      if (_episode!.episodeDuration > 0) humanDuration(_episode!.episodeDuration),
    ].join('  ·  ');

    return DecoratedBox(
      decoration: BoxDecoration(
        gradient: LinearGradient(
          begin: Alignment.topCenter,
          end: Alignment.bottomCenter,
          colors: [headerTint, bg],
        ),
      ),
      child: Padding(
        padding: const EdgeInsets.fromLTRB(24, 8, 24, 16),
        child: Column(
          children: [
            DecoratedBox(
              decoration: BoxDecoration(
                borderRadius: BorderRadius.circular(16),
                boxShadow: [
                  BoxShadow(color: Colors.black.withOpacity(0.25), blurRadius: 24, offset: const Offset(0, 8)),
                ],
              ),
              child: LazyNetworkImage(
                imageUrl: _episode!.episodeArtwork,
                width: 200,
                height: 200,
                borderRadius: BorderRadius.circular(16),
              ),
            ),
            const SizedBox(height: 20),
            InkWell(
              onTap: _navigateToPodcast,
              borderRadius: BorderRadius.circular(8),
              child: Padding(
                padding: const EdgeInsets.symmetric(horizontal: 6, vertical: 2),
                child: Row(
                  mainAxisSize: MainAxisSize.min,
                  children: [
                    Flexible(
                      child: Text(
                        _episode!.podcastName,
                        maxLines: 1,
                        overflow: TextOverflow.ellipsis,
                        style: theme.textTheme.titleSmall!.copyWith(color: accent, fontWeight: FontWeight.w600),
                      ),
                    ),
                    const SizedBox(width: 2),
                    Icon(PhosphorIconsBold.caretRight, size: 14, color: accent),
                  ],
                ),
              ),
            ),
            const SizedBox(height: 6),
            Text(
              _episode!.episodeTitle,
              textAlign: TextAlign.center,
              maxLines: 4,
              overflow: TextOverflow.ellipsis,
              style: theme.textTheme.titleLarge!.copyWith(fontWeight: FontWeight.bold, height: 1.25),
            ),
            const SizedBox(height: 8),
            Text(meta, style: theme.textTheme.bodyMedium!.copyWith(color: muted)),
          ],
        ),
      ),
    );
  }

  Widget _buildPlayButton(Color accent) {
    final onAccent = ThemeData.estimateBrightnessForColor(accent) == Brightness.dark ? Colors.white : Colors.black;

    return StreamBuilder<AudioState>(
      stream: Provider.of<AudioPlayerService>(context, listen: false).playingState,
      builder: (context, snapshot) {
        final isCurrentEpisode = _isCurrentEpisodePlaying();
        final state = snapshot.data;
        // Treat buffering/starting as active so the button reads correctly
        // through the whole start sequence.
        final isActive = isCurrentEpisode &&
            (state == AudioState.playing || state == AudioState.buffering || state == AudioState.starting);
        // Spinner while we've started this episode but the audio system
        // hasn't acknowledged it yet.
        final showSpinner = _isStartingPlayback && !isCurrentEpisode;
        final started = _episode!.isStarted && !_episode!.completed;

        final IconData icon;
        final String label;
        if (isActive) {
          icon = PhosphorIconsFill.pause;
          label = 'Pause';
        } else if (_episode!.completed) {
          icon = PhosphorIconsBold.arrowCounterClockwise;
          label = 'Play again';
        } else if (started) {
          icon = PhosphorIconsFill.play;
          final remaining = _episode!.episodeDuration - (_episode!.listenDuration ?? 0);
          label = 'Resume  ·  ${humanDuration(remaining)} left';
        } else {
          icon = PhosphorIconsFill.play;
          label = 'Play';
        }

        return ClipRRect(
          borderRadius: BorderRadius.circular(26),
          child: Stack(
            children: [
              SizedBox(
                width: double.infinity,
                height: 52,
                child: FilledButton(
                  style: FilledButton.styleFrom(
                    backgroundColor: accent,
                    foregroundColor: onAccent,
                    disabledBackgroundColor: accent.withOpacity(0.5),
                    disabledForegroundColor: onAccent.withOpacity(0.7),
                    shape: const StadiumBorder(),
                    textStyle: const TextStyle(fontSize: 16, fontWeight: FontWeight.w600),
                  ),
                  onPressed: _actionGuard.inProgress ? null : () => _runGuarded(_togglePlayPause),
                  child: AnimatedSwitcher(
                    duration: const Duration(milliseconds: 200),
                    child: showSpinner
                        ? SizedBox(
                            key: const ValueKey('loading'),
                            width: 22,
                            height: 22,
                            child: CircularProgressIndicator(strokeWidth: 2.5, color: onAccent),
                          )
                        : Row(
                            key: ValueKey(label),
                            mainAxisSize: MainAxisSize.min,
                            children: [
                              Icon(icon, size: 22),
                              const SizedBox(width: 8),
                              Text(label),
                            ],
                          ),
                  ),
                ),
              ),
              if (started)
                Positioned(
                  left: 0,
                  right: 0,
                  bottom: 0,
                  child: LinearProgressIndicator(
                    value: (_episode!.progressPercentage / 100).clamp(0.0, 1.0),
                    minHeight: 3,
                    backgroundColor: onAccent.withOpacity(0.15),
                    valueColor: AlwaysStoppedAnimation<Color>(onAccent.withOpacity(0.6)),
                  ),
                ),
            ],
          ),
        );
      },
    );
  }

  Widget _buildActionStrip(ThemeData theme, Color accent) {
    final busy = _actionGuard.inProgress;
    final e = _episode!;
    return Row(
      children: [
        _EpisodeAction(
          icon: e.saved ? PhosphorIconsFill.bookmarkSimple : PhosphorIconsRegular.bookmarkSimple,
          label: e.saved ? 'Saved' : 'Save',
          active: e.saved,
          accent: accent,
          onTap: busy ? null : () => _runGuarded(e.saved ? _removeSavedEpisode : _saveEpisode),
        ),
        _EpisodeAction(
          icon: e.queued ? PhosphorIconsFill.queue : PhosphorIconsRegular.queue,
          label: e.queued ? 'Queued' : 'Queue',
          active: e.queued,
          accent: accent,
          onTap: busy ? null : () => _runGuarded(_toggleQueue),
        ),
        _EpisodeAction(
          icon: _isDownloadedLocally ? PhosphorIconsFill.checkCircle : PhosphorIconsRegular.downloadSimple,
          label: _isDownloadedLocally ? 'On device' : 'Download',
          active: _isDownloadedLocally,
          accent: accent,
          onTap: busy ? null : () => _runGuarded(_isDownloadedLocally ? _confirmDeleteLocalDownload : _localDownloadEpisode),
        ),
        _EpisodeAction(
          icon: e.completed ? PhosphorIconsFill.checkCircle : PhosphorIconsRegular.checkCircle,
          label: 'Played',
          active: e.completed,
          accent: accent,
          onTap: busy ? null : () => _runGuarded(_toggleComplete),
        ),
        _EpisodeAction(
          icon: PhosphorIconsRegular.dotsThree,
          label: 'More',
          active: false,
          accent: accent,
          onTap: _showMoreSheet,
        ),
      ],
    );
  }

  Future<void> _confirmDeleteLocalDownload() async {
    final confirmed = await showDialog<bool>(
      context: context,
      builder: (context) => AlertDialog(
        title: const Text('Remove download?'),
        content: const Text('This deletes the copy stored on this device.'),
        actions: [
          TextButton(onPressed: () => Navigator.pop(context, false), child: const Text('Cancel')),
          TextButton(onPressed: () => Navigator.pop(context, true), child: const Text('Remove')),
        ],
      ),
    );
    if (confirmed == true) await _deleteLocalDownload();
  }

  void _showMoreSheet() {
    final e = _episode!;
    showModalBottomSheet<void>(
      context: context,
      showDragHandle: true,
      builder: (sheetContext) {
        void run(VoidCallback action) {
          Navigator.pop(sheetContext);
          action();
        }

        return SafeArea(
          child: Column(
            mainAxisSize: MainAxisSize.min,
            children: [
              ListTile(
                leading: Icon(e.downloaded ? PhosphorIconsRegular.cloudSlash : PhosphorIconsRegular.cloudArrowDown),
                title: Text(e.downloaded ? 'Remove server download' : 'Download to server'),
                subtitle: const Text('Stored on your PinePods server'),
                enabled: !_actionGuard.inProgress,
                onTap: () => run(() => _runGuarded(_toggleDownload)),
              ),
              ListTile(
                leading: const Icon(PhosphorIconsRegular.microphoneStage),
                title: const Text('Go to podcast'),
                onTap: () => run(_navigateToPodcast),
              ),
              ListTile(
                leading: const Icon(PhosphorIconsRegular.shareNetwork),
                title: const Text('Share episode'),
                onTap: () => run(_shareEpisode),
              ),
            ],
          ),
        );
      },
    );
  }

  @override
  void dispose() {
    _episodeSub?.cancel();
    _stateSub?.cancel();
    // Don't dispose global audio service - it should persist across pages
    super.dispose();
  }
}

/// Icon-over-label action used in the episode page's action strip.
class _EpisodeAction extends StatelessWidget {
  final IconData icon;
  final String label;
  final bool active;
  final Color accent;
  final VoidCallback? onTap;

  const _EpisodeAction({
    required this.icon,
    required this.label,
    required this.active,
    required this.accent,
    required this.onTap,
  });

  @override
  Widget build(BuildContext context) {
    final theme = Theme.of(context);
    final base = active ? accent : softNeutral(theme).withOpacity(0.8);
    final color = onTap == null ? base.withOpacity(0.4) : base;
    return Expanded(
      child: Semantics(
        button: true,
        label: label,
        child: InkWell(
          onTap: onTap,
          borderRadius: BorderRadius.circular(12),
          child: Padding(
            padding: const EdgeInsets.symmetric(vertical: 8),
            child: Column(
              mainAxisSize: MainAxisSize.min,
              children: [
                Icon(icon, size: 26, color: color),
                const SizedBox(height: 4),
                Text(
                  label,
                  maxLines: 1,
                  overflow: TextOverflow.ellipsis,
                  style: TextStyle(color: color, fontSize: 11.5, fontWeight: FontWeight.w500, letterSpacing: 0),
                ),
              ],
            ),
          ),
        ),
      ),
    );
  }
}
