// lib/ui/pinepods/saved.dart
import 'package:flutter/material.dart';
import 'package:pinepods_mobile/bloc/settings/settings_bloc.dart';
import 'package:pinepods_mobile/services/pinepods/pinepods_service.dart';
import 'package:pinepods_mobile/services/pinepods/pinepods_audio_service.dart';
import 'package:pinepods_mobile/services/audio/audio_player_service.dart';
import 'package:pinepods_mobile/entities/pinepods_episode.dart';
import 'package:pinepods_mobile/ui/widgets/episode_context_menu.dart';
import 'package:pinepods_mobile/ui/pinepods/podcast_nav.dart';
import 'package:pinepods_mobile/ui/widgets/pinepods_episode_card.dart';
import 'package:pinepods_mobile/ui/pinepods/episode_details.dart';
import 'package:pinepods_mobile/ui/utils/local_download_utils.dart';
import 'package:pinepods_mobile/ui/utils/player_utils.dart';
import 'package:pinepods_mobile/ui/utils/position_utils.dart';
import 'package:pinepods_mobile/ui/widgets/server_error_page.dart';
import 'package:pinepods_mobile/services/error_handling_service.dart';
import 'package:pinepods_mobile/services/global_services.dart';
import 'package:provider/provider.dart';
import 'package:shared_preferences/shared_preferences.dart';
import 'package:sliver_tools/sliver_tools.dart';
import 'package:pinepods_mobile/ui/widgets/soft_ui.dart';
import 'package:pinepods_mobile/ui/icons/phosphor_icons.dart';
import 'package:pinepods_mobile/l10n/L.dart';
import 'package:pinepods_mobile/ui/icons/collection_icons.dart';
import 'package:pinepods_mobile/ui/pinepods/collection_sheets.dart';
import 'package:pinepods_mobile/entities/pinepods_collection.dart';

/// Sort direction options for saved episodes
enum SavedSortDirection {
  newestFirst,
  oldestFirst,
  shortestFirst,
  longestFirst,
  titleAZ,
  titleZA,
}

/// Episode filter options
enum EpisodeFilter {
  all,
  completed,
  inProgress,
}

class PinepodsSaved extends StatefulWidget {
  const PinepodsSaved({Key? key}) : super(key: key);

  @override
  State<PinepodsSaved> createState() => _PinepodsSavedState();
}

class _PinepodsSavedState extends State<PinepodsSaved> {
  bool _isLoading = false;
  String _errorMessage = '';
  List<PinepodsEpisode> _episodes = [];
  List<PinepodsEpisode> _filteredEpisodes = [];
  // Position of each episode in [_episodes], for the context menu.
  Map<PinepodsEpisode, int> _originalIndex = Map<PinepodsEpisode, int>.identity();
  final PinepodsService _pinepodsService = PinepodsService();
  // Use global audio service instead of creating local instance
  int? _contextMenuEpisodeIndex;
  final TextEditingController _searchController = TextEditingController();
  String _searchQuery = '';

  // Pagination state
  int _offset = 0;
  int _total = 0;
  bool _isLoadingMore = false;

  // Sort and filter state
  SavedSortDirection _sortDirection = SavedSortDirection.newestFirst;
  EpisodeFilter _activeFilter = EpisodeFilter.all;
  static const String _sortPreferenceKey = 'saved_sort_direction';

  // Favorites-only filter (podcast-level favorite state from return_pods).
  bool _favoritesOnly = false;
  Set<int> _favoritePodcastIds = {};

  // Collections other than Saved; [_active] is the one shown (null = Saved).
  List<PinepodsCollection> _collections = [];
  PinepodsCollection? _active;

  @override
  void initState() {
    super.initState();
    _loadSortPreference();
    _loadSavedEpisodes();
    _loadCollections();
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

  Future<void> _saveSortPreference(SavedSortDirection direction) async {
    final prefs = await SharedPreferences.getInstance();
    await prefs.setString(_sortPreferenceKey, _sortDirectionToString(direction));
  }

  String _sortDirectionToString(SavedSortDirection direction) {
    switch (direction) {
      case SavedSortDirection.newestFirst:
        return 'newest';
      case SavedSortDirection.oldestFirst:
        return 'oldest';
      case SavedSortDirection.shortestFirst:
        return 'shortest';
      case SavedSortDirection.longestFirst:
        return 'longest';
      case SavedSortDirection.titleAZ:
        return 'title_az';
      case SavedSortDirection.titleZA:
        return 'title_za';
    }
  }

  SavedSortDirection _sortDirectionFromString(String value) {
    switch (value) {
      case 'oldest':
        return SavedSortDirection.oldestFirst;
      case 'shortest':
        return SavedSortDirection.shortestFirst;
      case 'longest':
        return SavedSortDirection.longestFirst;
      case 'title_az':
        return SavedSortDirection.titleAZ;
      case 'title_za':
        return SavedSortDirection.titleZA;
      case 'newest':
      default:
        return SavedSortDirection.newestFirst;
    }
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
    // Sort and status filter are handled server-side; only apply search locally
    var filtered = List<PinepodsEpisode>.from(_episodes);
    if (_searchQuery.isNotEmpty) {
      filtered = filtered.where((episode) {
        return episode.episodeTitle.toLowerCase().contains(_searchQuery.toLowerCase()) ||
               episode.podcastName.toLowerCase().contains(_searchQuery.toLowerCase()) ||
               episode.episodeDescription.toLowerCase().contains(_searchQuery.toLowerCase());
      }).toList();
    }
    if (_favoritesOnly) {
      filtered = filtered
          .where((e) =>
              e.podcastId != null && _favoritePodcastIds.contains(e.podcastId))
          .toList();
    }
    _filteredEpisodes = filtered;
    _originalIndex = Map<PinepodsEpisode, int>.identity()
      ..addEntries([for (var i = 0; i < _episodes.length; i++) MapEntry(_episodes[i], i)]);
  }

  Future<void> _loadFavoritePodcastIds(int userId) async {
    try {
      final podcasts = await _pinepodsService.getUserPodcasts(userId);
      if (!mounted) return;
      setState(() {
        _favoritePodcastIds = podcasts
            .where((p) => p.isFavorite && p.id != null)
            .map((p) => p.id!)
            .toSet();
        _filterEpisodes();
      });
    } catch (e) {
      // Non-fatal: favorites filter just won't have data.
    }
  }

  void _toggleFavoritesFilter() {
    setState(() {
      _favoritesOnly = !_favoritesOnly;
      _filterEpisodes();
    });
    if (_favoritesOnly) {
      final userId =
          Provider.of<SettingsBloc>(context, listen: false)
              .currentSettings
              .pinepodsUserId;
      if (userId != null) {
        _loadFavoritePodcastIds(userId);
      }
    }
  }

  String _sortByParam(SavedSortDirection direction) {
    switch (direction) {
      case SavedSortDirection.shortestFirst:
      case SavedSortDirection.longestFirst:
        return 'duration';
      case SavedSortDirection.titleAZ:
      case SavedSortDirection.titleZA:
        return 'title';
      default:
        return 'date';
    }
  }

  String _sortOrderParam(SavedSortDirection direction) {
    switch (direction) {
      case SavedSortDirection.oldestFirst:
      case SavedSortDirection.shortestFirst:
      case SavedSortDirection.titleAZ:
        return 'asc';
      default:
        return 'desc';
    }
  }

  String _filterParam(EpisodeFilter filter) {
    switch (filter) {
      case EpisodeFilter.completed: return 'completed';
      case EpisodeFilter.inProgress: return 'in_progress';
      case EpisodeFilter.all: return 'all';
    }
  }

  void _setSortDirection(SavedSortDirection direction) {
    setState(() {
      _sortDirection = direction;
      _episodes = [];
      _filteredEpisodes = [];
      _offset = 0;
      _total = 0;
    });
    _saveSortPreference(direction);
    _loadSavedEpisodes();
  }

  void _setFilter(EpisodeFilter filter) {
    setState(() {
      _activeFilter = filter;
      _episodes = [];
      _filteredEpisodes = [];
      _offset = 0;
      _total = 0;
    });
    _loadSavedEpisodes();
  }

  void _clearAllFilters() {
    setState(() {
      _activeFilter = EpisodeFilter.all;
      _favoritesOnly = false;
      _searchController.clear();
      _searchQuery = '';
      _episodes = [];
      _filteredEpisodes = [];
      _offset = 0;
      _total = 0;
    });
    _loadSavedEpisodes();
  }

  PinepodsAudioService? get _audioService => GlobalServices.pinepodsAudioService;

  Future<void> _loadCollections() async {
    final client = collectionsClient(context);
    if (client == null) return;
    try {
      final cols = await client.$1.getCollections(client.$2);
      if (!mounted) return;
      setState(() {
        _collections = cols.where((c) => !c.isDefault).toList();
        // Keep the shown collection current (name/icon edits), or fall back to Saved.
        if (_active != null) {
          _active = _collections.where((c) => c.id == _active!.id).firstOrNull;
          if (_active == null) _loadSavedEpisodes();
        }
      });
    } catch (_) {
      // Non-fatal: the page still shows Saved.
    }
  }

  void _selectCollection(PinepodsCollection? collection) {
    if (collection?.id == _active?.id) return;
    setState(() {
      _active = collection;
      _searchController.clear();
      _searchQuery = '';
    });
    _loadSavedEpisodes();
  }

  Future<void> _newCollection() async {
    final created = await showCollectionEditor(context);
    if (created == null || !mounted) return;
    await _loadCollections();
    final match = _collections.where((c) => c.id == created.id).firstOrNull;
    if (match != null) _selectCollection(match);
  }

  Future<void> _editActiveCollection() async {
    final active = _active;
    if (active == null) return;
    final result = await showCollectionEditor(context, existing: active);
    if (result == null || !mounted) return;
    if (result.deleted) {
      setState(() => _active = null);
      _loadSavedEpisodes();
    }
    _loadCollections();
  }

  Future<void> _removeFromActiveCollection(int episodeIndex) async {
    final active = _active;
    final client = collectionsClient(context);
    if (active == null || client == null) return;
    final episode = _episodes[episodeIndex];
    try {
      await client.$1.setEpisodeInCollection(active.id, client.$2, episode.episodeId, episode.isYoutube, false);
      if (!mounted) return;
      setState(() {
        _episodes.removeAt(episodeIndex);
        _total = (_total - 1).clamp(0, _total);
        _offset = (_offset - 1).clamp(0, _offset);
        _filterEpisodes();
      });
      _showSnackBar(L.current.removed_from_collection(active.name), Colors.orange);
      _loadCollections();
    } catch (e) {
      _showSnackBar(L.current.failed_to_remove_error(e.toString()), Colors.red);
    }
  }

  /// One page of the shown list: Saved, or the active collection.
  Future<EpisodePage> _fetchPage(int userId, int offset) {
    final sortBy = _sortByParam(_sortDirection);
    final sortOrder = _sortOrderParam(_sortDirection);
    final filter = _filterParam(_activeFilter);
    final active = _active;
    if (active != null) {
      return _pinepodsService.getCollectionEpisodesPaged(active.id,
          limit: 50, offset: offset, sortBy: sortBy, sortOrder: sortOrder, filter: filter);
    }
    return _pinepodsService.getSavedEpisodesPaged(userId,
        limit: 50, offset: offset, sortBy: sortBy, sortOrder: sortOrder, filter: filter);
  }

  Future<void> _loadSavedEpisodes() async {
    setState(() {
      _isLoading = true;
      _errorMessage = '';
      _offset = 0;
      _total = 0;
      _episodes = [];
      _filteredEpisodes = [];
    });

    try {
      final settingsBloc = Provider.of<SettingsBloc>(context, listen: false);
      final settings = settingsBloc.currentSettings;

      if (settings.pinepodsServer == null ||
          settings.pinepodsApiKey == null ||
          settings.pinepodsUserId == null) {
        setState(() {
          _errorMessage = L.of(context)!.not_connected_please_login;
          _isLoading = false;
        });
        return;
      }

      _pinepodsService.setCredentials(settings.pinepodsServer!, settings.pinepodsApiKey!);
      GlobalServices.setCredentials(settings.pinepodsServer!, settings.pinepodsApiKey!);
      final userId = settings.pinepodsUserId!;

      final page = await _fetchPage(userId, 0);

      final enrichedEpisodes = await PositionUtils.enrichEpisodesWithBestPositions(
        context,
        _pinepodsService,
        page.episodes,
        userId,
      );

      setState(() {
        _episodes = enrichedEpisodes;
        _total = page.total;
        _offset = enrichedEpisodes.length;
        _filterEpisodes();
        _isLoading = false;
      });

      await LocalDownloadUtils.loadLocalDownloadStatuses(context, enrichedEpisodes);
      await _loadFavoritePodcastIds(userId);
    } catch (e) {
      setState(() {
        _errorMessage = L.current.failed_to_load_saved_episodes_error(e.toString());
        _isLoading = false;
      });
    }
  }

  Future<void> _loadMoreEpisodes() async {
    if (_isLoadingMore || _offset >= _total) return;

    final settingsBloc = Provider.of<SettingsBloc>(context, listen: false);
    final settings = settingsBloc.currentSettings;

    if (settings.pinepodsServer == null ||
        settings.pinepodsApiKey == null ||
        settings.pinepodsUserId == null) {
      return;
    }

    final userId = settings.pinepodsUserId!;
    setState(() => _isLoadingMore = true);

    try {
      final page = await _fetchPage(userId, _offset);

      final enrichedEpisodes = await PositionUtils.enrichEpisodesWithBestPositions(
        context,
        _pinepodsService,
        page.episodes,
        userId,
      );

      setState(() {
        _episodes.addAll(enrichedEpisodes);
        _total = page.total;
        _offset += enrichedEpisodes.length;
        _filterEpisodes();
        _isLoadingMore = false;
      });

      await LocalDownloadUtils.loadLocalDownloadStatuses(context, enrichedEpisodes);
    } catch (e) {
      setState(() => _isLoadingMore = false);
    }
  }

  Future<void> _refresh() async {
    LocalDownloadUtils.clearCache();
    _loadCollections();
    await _loadSavedEpisodes();
  }

  Future<void> _playEpisode(PinepodsEpisode episode) async {
    
    if (_audioService == null) {
      ScaffoldMessenger.of(context).showSnackBar(
        SnackBar(
          content: Text(L.of(context)!.audio_service_not_available),
          backgroundColor: Colors.red,
        ),
      );
      return;
    }

    try {
      await _audioService!.playPinepodsEpisode(
        pinepodsEpisode: episode,
        resume: episode.isStarted,
      );
    } catch (e) {
      ScaffoldMessenger.of(context).showSnackBar(
        SnackBar(
          content: Text(L.current.failed_to_play_episode(e.toString())),
          backgroundColor: Colors.red,
          duration: const Duration(seconds: 3),
        ),
      );
    }
  }

  Future<void> _showContextMenu(int episodeIndex) async {
    final episode = _episodes[episodeIndex];
    final isDownloadedLocally = await LocalDownloadUtils.isEpisodeDownloadedLocally(context, episode);

    if (!mounted) return;

    final pageContext = context;
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
          _toggleQueueEpisode(episodeIndex);
        },
        onQueueAt: (position) {
          Navigator.of(context).pop();
          _toggleQueueEpisode(episodeIndex, position: position);
        },
        onMarkComplete: () {
          Navigator.of(context).pop();
          _toggleMarkComplete(episodeIndex);
        },
        onDismiss: () {
          Navigator.of(context).pop();
        },
        onRemoveFromCollection: _active == null
            ? null
            : () {
                Navigator.of(context).pop();
                _removeFromActiveCollection(episodeIndex);
              },
        onPodcastTap: () {
          Navigator.of(context).pop();
          navigateToPodcastById(
            pageContext,
            episode.podcastId,
            fallbackTitle: episode.podcastName,
            fallbackArtwork: episode.episodeArtwork,
          );
        },
      ),
    );
  }

  Future<void> _localDownloadEpisode(int episodeIndex) async {
    final episode = _episodes[episodeIndex];
    
    final success = await LocalDownloadUtils.localDownloadEpisode(context, episode);
    
    if (success) {
      LocalDownloadUtils.showSnackBar(context, L.current.episode_download_started, Colors.green);
    } else {
      LocalDownloadUtils.showSnackBar(context, L.current.failed_to_start_download, Colors.red);
    }
  }

  Future<void> _deleteLocalDownload(int episodeIndex) async {
    final episode = _episodes[episodeIndex];
    
    final deletedCount = await LocalDownloadUtils.deleteLocalDownload(context, episode);
    
    if (deletedCount > 0) {
      LocalDownloadUtils.showSnackBar(
        context, 
        L.current.deleted_local_downloads(deletedCount), 
        Colors.orange
      );
    } else {
      LocalDownloadUtils.showSnackBar(context, L.current.local_download_not_found, Colors.red);
    }
  }

  void _hideContextMenu() {
    setState(() {
      _contextMenuEpisodeIndex = null;
    });
  }

  Future<void> _saveEpisode(int episodeIndex) async {
    // This shouldn't be called since all episodes here are already saved
    // But just in case, we'll handle it
    final episode = _episodes[episodeIndex];
    final settingsBloc = Provider.of<SettingsBloc>(context, listen: false);
    final settings = settingsBloc.currentSettings;
    final userId = settings.pinepodsUserId;

    if (userId == null) {
      _showSnackBar(L.of(context)!.not_logged_in, Colors.red);
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
          _episodes[episodeIndex] = _updateEpisodeProperty(_episodes[episodeIndex], saved: true);
        });
        _showSnackBar(L.current.episode_saved, Colors.green);
      } else {
        _showSnackBar(L.current.failed_to_save_episode, Colors.red);
      }
    } catch (e) {
      _showSnackBar(L.current.error_saving_episode(e.toString()), Colors.red);
    }

    _hideContextMenu();
  }

  Future<void> _removeSavedEpisode(int episodeIndex) async {
    final episode = _episodes[episodeIndex];
    final settingsBloc = Provider.of<SettingsBloc>(context, listen: false);
    final settings = settingsBloc.currentSettings;
    final userId = settings.pinepodsUserId;

    if (userId == null) {
      _showSnackBar(L.of(context)!.not_logged_in, Colors.red);
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
          if (_active == null) {
            // Off the Saved list now; a collection still holds it.
            _episodes.removeAt(episodeIndex);
          } else {
            _episodes[episodeIndex] = _updateEpisodeProperty(episode, saved: false);
          }
          _filterEpisodes();
        });
        _showSnackBar(L.current.removed_from_saved_episodes, Colors.orange);
      } else {
        _showSnackBar(L.current.failed_to_remove_saved_episode, Colors.red);
      }
    } catch (e) {
      _showSnackBar(L.current.error_removing_saved_episode(e.toString()), Colors.red);
    }

    _hideContextMenu();
  }

  Future<void> _downloadEpisode(int episodeIndex) async {
    final episode = _episodes[episodeIndex];
    final settingsBloc = Provider.of<SettingsBloc>(context, listen: false);
    final settings = settingsBloc.currentSettings;
    final userId = settings.pinepodsUserId;

    if (userId == null) {
      _showSnackBar(L.of(context)!.not_logged_in, Colors.red);
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
        });
        _showSnackBar(L.current.episode_download_queued, Colors.green);
      } else {
        _showSnackBar(L.current.failed_to_queue_download, Colors.red);
      }
    } catch (e) {
      _showSnackBar(L.current.error_downloading_episode(e.toString()), Colors.red);
    }

    _hideContextMenu();
  }

  Future<void> _deleteEpisode(int episodeIndex) async {
    final episode = _episodes[episodeIndex];
    final settingsBloc = Provider.of<SettingsBloc>(context, listen: false);
    final settings = settingsBloc.currentSettings;
    final userId = settings.pinepodsUserId;

    if (userId == null) {
      _showSnackBar(L.of(context)!.not_logged_in, Colors.red);
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
        });
        _showSnackBar(L.current.episode_deleted_from_server, Colors.orange);
      } else {
        _showSnackBar(L.current.failed_to_delete_episode, Colors.red);
      }
    } catch (e) {
      _showSnackBar(L.current.error_deleting_episode(e.toString()), Colors.red);
    }

    _hideContextMenu();
  }

  Future<void> _toggleQueueEpisode(int episodeIndex, {String? position}) async {
    final episode = _episodes[episodeIndex];
    final settingsBloc = Provider.of<SettingsBloc>(context, listen: false);
    final settings = settingsBloc.currentSettings;
    final userId = settings.pinepodsUserId;

    if (userId == null) {
      _showSnackBar(L.of(context)!.not_logged_in, Colors.red);
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
          });
          _showSnackBar(L.current.removed_from_queue, Colors.orange);
        }
      } else {
        success = await _pinepodsService.queueEpisode(
          episode.episodeId,
          userId,
          episode.isYoutube,
          position: position,
        );
        if (success) {
          setState(() {
            _episodes[episodeIndex] = _updateEpisodeProperty(episode, queued: true);
          });
          _showSnackBar(L.current.added_to_queue_excl, Colors.green);
        }
      }

      if (!success) {
        _showSnackBar(L.current.failed_to_update_queue, Colors.red);
      }
    } catch (e) {
      _showSnackBar(L.current.error_updating_queue(e.toString()), Colors.red);
    }

    _hideContextMenu();
  }

  Future<void> _toggleMarkComplete(int episodeIndex) async {
    final episode = _episodes[episodeIndex];
    final settingsBloc = Provider.of<SettingsBloc>(context, listen: false);
    final settings = settingsBloc.currentSettings;
    final userId = settings.pinepodsUserId;

    if (userId == null) {
      _showSnackBar(L.of(context)!.not_logged_in, Colors.red);
      return;
    }

    _pinepodsService.setCredentials(settings.pinepodsServer!, settings.pinepodsApiKey!);

    try {
      bool success;
      if (episode.completed) {
        success = await _pinepodsService.markEpisodeUncompleted(
          episode.episodeId,
          userId,
          episode.isYoutube,
        );
        if (success) {
          setState(() {
            _episodes[episodeIndex] = _updateEpisodeProperty(episode, completed: false);
          });
          _showSnackBar(L.current.marked_as_incomplete, Colors.orange);
        }
      } else {
        success = await _pinepodsService.markEpisodeCompleted(
          episode.episodeId,
          userId,
          episode.isYoutube,
        );
        if (success) {
          setState(() {
            _episodes[episodeIndex] = _updateEpisodeProperty(episode, completed: true);
          });
          _showSnackBar(L.current.marked_as_complete, Colors.green);
        }
      }

      if (!success) {
        _showSnackBar(L.current.failed_to_update_completion, Colors.red);
      }
    } catch (e) {
      _showSnackBar(L.current.error_updating_completion(e.toString()), Colors.red);
    }

    _hideContextMenu();
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
      saveDate: episode.saveDate,
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

  @override
  Widget build(BuildContext context) {
    final title = _active?.name ?? L.of(context)!.saved;
    final header = <Widget>[
      SliverToBoxAdapter(
        child: SoftSectionHeader(
          title: title,
          count: _isLoading || _errorMessage.isNotEmpty ? null : _filteredEpisodes.length,
          trailing: [
            if (_active != null)
              IconButton(
                tooltip: L.of(context)!.edit_collection,
                icon: const Icon(PhosphorIconsRegular.pencilSimple),
                onPressed: _editActiveCollection,
              ),
            IconButton(
              tooltip: L.of(context)!.refresh,
              icon: const Icon(PhosphorIconsRegular.arrowClockwise),
              onPressed: _refresh,
            ),
          ],
        ),
      ),
      SliverToBoxAdapter(child: _buildCollectionChips()),
    ];

    if (_isLoading) {
      return MultiSliver(children: [
        ...header,
        const SliverFillRemaining(
          hasScrollBody: false,
          child: Center(child: CircularProgressIndicator()),
        ),
      ]);
    }

    if (_errorMessage.isNotEmpty) {
      return SliverServerErrorPage(
        errorMessage: _errorMessage.isServerConnectionError 
          ? null 
          : _errorMessage,
        onRetry: _refresh,
        title: L.of(context)!.saved_episodes_unavailable,
        subtitle: _errorMessage.isServerConnectionError
          ? L.of(context)!.unable_to_connect_server
          : L.of(context)!.failed_to_load_saved_episodes,
      );
    }

    if (_episodes.isEmpty && _activeFilter == EpisodeFilter.all) {
      return MultiSliver(children: [
        ...header,
        SliverFillRemaining(
          hasScrollBody: false,
          child: Center(
            child: _active == null
                ? SoftEmptyState(
                    icon: PhosphorIconsRegular.bookmarkSimple,
                    title: L.of(context)!.no_saved_episodes,
                    body: L.of(context)!.saved_empty_body,
                  )
                : SoftEmptyState(
                    icon: collectionIcon(_active!.icon),
                    title: L.of(context)!.collection_empty_title(_active!.name),
                    body: L.of(context)!.collection_empty_body,
                  ),
          ),
        ),
      ]);
    }

    return MultiSliver(
      children: [
        ...header,
        ListToolbar.sliver(context, _buildToolbar()),
        _buildEpisodesList(),
      ],
    );
  }

  /// Saved, then each collection, then "New".
  Widget _buildCollectionChips() {
    final pills = <Widget>[
      FilterPill(
        label: L.of(context)!.saved,
        icon: PhosphorIconsRegular.bookmarkSimple,
        selected: _active == null,
        onTap: () => _selectCollection(null),
      ),
      for (final c in _collections)
        FilterPill(
          label: c.name,
          icon: collectionIcon(c.icon),
          selected: _active?.id == c.id,
          onTap: () => _selectCollection(c),
        ),
      FilterPill(
        label: L.of(context)!.new_collection_chip,
        icon: PhosphorIconsRegular.plus,
        selected: false,
        onTap: _newCollection,
      ),
    ];
    return SizedBox(
      height: 46,
      child: ListView.separated(
        scrollDirection: Axis.horizontal,
        padding: const EdgeInsets.fromLTRB(16, 6, 16, 4),
        itemCount: pills.length,
        separatorBuilder: (_, __) => const SizedBox(width: 8),
        itemBuilder: (_, i) => pills[i],
      ),
    );
  }

  ListToolbar<SavedSortDirection> _buildToolbar() {
    return ListToolbar<SavedSortDirection>(
      searchController: _searchController,
      searchHint: L.of(context)!.search_episodes_label,
      filters: [
        ToolbarFilter(
          label: L.of(context)!.filter_all,
          selected: _activeFilter == EpisodeFilter.all && !_favoritesOnly,
          onTap: _clearAllFilters,
        ),
        ToolbarFilter(
          label: L.of(context)!.filter_in_progress,
          selected: _activeFilter == EpisodeFilter.inProgress,
          onTap: () => _setFilter(
              _activeFilter == EpisodeFilter.inProgress ? EpisodeFilter.all : EpisodeFilter.inProgress),
        ),
        ToolbarFilter(
          label: L.of(context)!.completed,
          selected: _activeFilter == EpisodeFilter.completed,
          onTap: () => _setFilter(
              _activeFilter == EpisodeFilter.completed ? EpisodeFilter.all : EpisodeFilter.completed),
        ),
        ToolbarFilter(
          label: L.of(context)!.favorites,
          icon: _favoritesOnly ? PhosphorIconsFill.star : PhosphorIconsRegular.star,
          selected: _favoritesOnly,
          onTap: _toggleFavoritesFilter,
        ),
      ],
      sortOptions: {
        SavedSortDirection.newestFirst: L.of(context)!.sort_newest_first,
        SavedSortDirection.oldestFirst: L.of(context)!.sort_oldest_first,
        SavedSortDirection.shortestFirst: L.of(context)!.sort_shortest_first,
        SavedSortDirection.longestFirst: L.of(context)!.sort_longest_first,
        SavedSortDirection.titleAZ: L.of(context)!.sort_title_az,
        SavedSortDirection.titleZA: L.of(context)!.sort_title_za,
      },
      sortValue: _sortDirection,
      onSort: _setSortDirection,
    );
  }

  Widget _buildEpisodesList() {
    final hasActiveSearch = _searchQuery.isNotEmpty;
    if (_filteredEpisodes.isEmpty &&
        !_isLoadingMore &&
        (hasActiveSearch || _favoritesOnly || _activeFilter != EpisodeFilter.all)) {
      return SliverToBoxAdapter(
        child: SoftEmptyState(
          icon: PhosphorIconsRegular.funnelSimpleX,
          title: L.of(context)!.no_episodes_found,
          body: _getNoResultsMessage(),
          action: TextButton(onPressed: _clearAllFilters, child: Text(L.of(context)!.clear_filters)),
        ),
      );
    }

    final showFooter = _isLoadingMore || _offset < _total;

    return SliverList(
      delegate: SliverChildBuilderDelegate(
        (context, index) {
          final episodeIndex = index;

          // Footer: triggers load more when it becomes visible
          if (showFooter && episodeIndex == _filteredEpisodes.length) {
            if (!_isLoadingMore) {
              WidgetsBinding.instance.addPostFrameCallback((_) => _loadMoreEpisodes());
            }
            return const Padding(
              padding: EdgeInsets.symmetric(vertical: 16.0),
              child: Center(child: CircularProgressIndicator()),
            );
          }

          if (episodeIndex >= _filteredEpisodes.length) return null;
          final episode = _filteredEpisodes[episodeIndex];
          final originalIndex = _originalIndex[episode] ?? -1;
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
            onLongPress: originalIndex >= 0 ? () => _showContextMenu(originalIndex) : null,
            onPlayPressed: () => _playEpisode(episode),
          );
        },
        childCount: _filteredEpisodes.length + (showFooter ? 1 : 0),
      ),
    );
  }

  String _getNoResultsMessage() {
    final l = L.of(context)!;
    final hasQuery = _searchQuery.isNotEmpty;
    if (_activeFilter == EpisodeFilter.completed) {
      return hasQuery ? l.no_completed_episodes_match_query(_searchQuery) : l.no_completed_episodes;
    }
    if (_activeFilter == EpisodeFilter.inProgress) {
      return hasQuery ? l.no_in_progress_episodes_match_query(_searchQuery) : l.no_in_progress_episodes;
    }
    return hasQuery ? l.no_episodes_match_query(_searchQuery) : l.no_episodes_match_filters;
  }
}