// lib/ui/pinepods/playlists.dart
import 'package:flutter/material.dart';
import 'package:pinepods_mobile/bloc/settings/settings_bloc.dart';
import 'package:pinepods_mobile/services/pinepods/pinepods_service.dart';
import 'package:pinepods_mobile/ui/widgets/platform_progress_indicator.dart';
import 'package:pinepods_mobile/ui/widgets/server_error_page.dart';
import 'package:pinepods_mobile/services/error_handling_service.dart';
import 'package:pinepods_mobile/ui/pinepods/playlist_episodes.dart';
import 'package:pinepods_mobile/ui/pinepods/create_playlist.dart';
import 'package:provider/provider.dart';

class PinepodsPlaylists extends StatefulWidget {
  const PinepodsPlaylists({Key? key}) : super(key: key);

  @override
  State<PinepodsPlaylists> createState() => _PinepodsPlaylistsState();
}

class _PinepodsPlaylistsState extends State<PinepodsPlaylists> {
  final PinepodsService _pinepodsService = PinepodsService();
  List<PlaylistData>? _playlists;
  bool _isLoading = true;
  String? _errorMessage;
  Set<int> _selectedPlaylists = {};
  bool _isSelectionMode = false;

  @override
  void initState() {
    super.initState();
    _loadPlaylists();
  }

  /// Calculate responsive cross axis count for playlist grid
  int _getPlaylistCrossAxisCount(BuildContext context) {
    final screenWidth = MediaQuery.of(context).size.width;
    if (screenWidth > 1200) return 4;      // Very wide screens (large tablets, desktop)
    if (screenWidth > 800) return 3;       // Wide tablets like iPad
    if (screenWidth > 500) return 2;       // Standard phones and small tablets
    return 1;                              // Very small phones (< 500px)
  }

  /// Calculate responsive aspect ratio for playlist cards
  double _getPlaylistAspectRatio(BuildContext context) {
    final screenWidth = MediaQuery.of(context).size.width;
    if (screenWidth <= 500) {
      // Single column on small screens - generous height for multi-line descriptions + padding
      return 1.8; // Allows space for title + 2-3 lines of description + proper padding
    }
    return 1.1; // Standard aspect ratio for multi-column layouts
  }

  Future<void> _loadPlaylists() async {
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
      _pinepodsService.setCredentials(
        settings.pinepodsServer!,
        settings.pinepodsApiKey!,
      );
      
      final playlists = await _pinepodsService.getUserPlaylists(settings.pinepodsUserId!);
      
      setState(() {
        _playlists = playlists;
        _isLoading = false;
      });
    } catch (e) {
      setState(() {
        _errorMessage = e.toString();
        _isLoading = false;
      });
    }
  }

  void _toggleSelectionMode() {
    setState(() {
      _isSelectionMode = !_isSelectionMode;
      if (!_isSelectionMode) {
        _selectedPlaylists.clear();
      }
    });
  }

  void _togglePlaylistSelection(int playlistId) {
    setState(() {
      if (_selectedPlaylists.contains(playlistId)) {
        _selectedPlaylists.remove(playlistId);
      } else {
        _selectedPlaylists.add(playlistId);
      }
    });
  }

  Future<void> _deleteSelectedPlaylists() async {
    if (_selectedPlaylists.isEmpty) return;

    final confirmed = await showDialog<bool>(
      context: context,
      builder: (context) => AlertDialog(
        title: const Text('Delete Playlists'),
        content: Text('Are you sure you want to delete ${_selectedPlaylists.length} playlist${_selectedPlaylists.length == 1 ? '' : 's'}?'),
        actions: [
          TextButton(
            onPressed: () => Navigator.of(context).pop(false),
            child: const Text('Cancel'),
          ),
          TextButton(
            onPressed: () => Navigator.of(context).pop(true),
            child: const Text('Delete'),
          ),
        ],
      ),
    );

    if (confirmed != true) return;

    final settingsBloc = Provider.of<SettingsBloc>(context, listen: false);
    final settings = settingsBloc.currentSettings;

    try {
      for (final playlistId in _selectedPlaylists) {
        await _pinepodsService.deletePlaylist(settings.pinepodsUserId!, playlistId);
      }
      
      setState(() {
        _selectedPlaylists.clear();
        _isSelectionMode = false;
      });
      
      _loadPlaylists(); // Refresh the list
      
      if (mounted) {
        ScaffoldMessenger.of(context).showSnackBar(
          const SnackBar(content: Text('Playlists deleted successfully')),
        );
      }
    } catch (e) {
      if (mounted) {
        ScaffoldMessenger.of(context).showSnackBar(
          SnackBar(content: Text('Error deleting playlists: $e')),
        );
      }
    }
  }

  Future<void> _deletePlaylist(PlaylistData playlist) async {
    final confirmed = await showDialog<bool>(
      context: context,
      builder: (context) => AlertDialog(
        title: const Text('Delete Playlist'),
        content: Text('Are you sure you want to delete "${playlist.name}"?'),
        actions: [
          TextButton(
            onPressed: () => Navigator.of(context).pop(false),
            child: const Text('Cancel'),
          ),
          TextButton(
            onPressed: () => Navigator.of(context).pop(true),
            child: const Text('Delete'),
          ),
        ],
      ),
    );

    if (confirmed != true) return;

    final settingsBloc = Provider.of<SettingsBloc>(context, listen: false);
    final settings = settingsBloc.currentSettings;

    try {
      await _pinepodsService.deletePlaylist(settings.pinepodsUserId!, playlist.playlistId);
      _loadPlaylists(); // Refresh the list
      
      if (mounted) {
        ScaffoldMessenger.of(context).showSnackBar(
          const SnackBar(content: Text('Playlist deleted successfully')),
        );
      }
    } catch (e) {
      if (mounted) {
        ScaffoldMessenger.of(context).showSnackBar(
          SnackBar(content: Text('Error deleting playlist: $e')),
        );
      }
    }
  }

  void _openPlaylist(PlaylistData playlist) {
    Navigator.push(
      context,
      MaterialPageRoute(
        builder: (context) => PlaylistEpisodesPage(playlist: playlist),
      ),
    );
  }

  void _createPlaylist() async {
    final result = await Navigator.push(
      context,
      MaterialPageRoute(
        builder: (context) => const CreatePlaylistPage(),
      ),
    );
    
    if (result == true) {
      _loadPlaylists(); // Refresh the list
    }
  }

  IconData _getPlaylistIcon(String? iconName) {
    if (iconName == null) return Icons.playlist_play;
    
    // Map common icon names to Material icons
    switch (iconName) {
      case 'ph-playlist':
        return Icons.playlist_play;
      case 'ph-music-notes':
        return Icons.music_note;
      case 'ph-play-circle':
        return Icons.play_circle;
      case 'ph-headphones':
        return Icons.headphones;
      case 'ph-star':
        return Icons.star;
      case 'ph-heart':
        return Icons.favorite;
      case 'ph-bookmark':
        return Icons.bookmark;
      case 'ph-clock':
        return Icons.access_time;
      case 'ph-calendar':
        return Icons.calendar_today;
      case 'ph-timer':
        return Icons.timer;
      case 'ph-shuffle':
        return Icons.shuffle;
      case 'ph-repeat':
        return Icons.repeat;
      case 'ph-microphone':
        return Icons.mic;
      case 'ph-queue':
        return Icons.queue_music;
      default:
        return Icons.playlist_play;
    }
  }

  @override
  Widget build(BuildContext context) {
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
        onRetry: _loadPlaylists,
        title: 'Playlists Unavailable',
        subtitle: _errorMessage!.isServerConnectionError
          ? 'Unable to connect to the PinePods server'
          : 'Failed to load your playlists',
      );
    }

    if (_playlists == null || _playlists!.isEmpty) {
      return SliverFillRemaining(
        hasScrollBody: false,
        child: Padding(
          padding: const EdgeInsets.all(32.0),
          child: Column(
            mainAxisAlignment: MainAxisAlignment.center,
            crossAxisAlignment: CrossAxisAlignment.center,
            children: <Widget>[
              Icon(
                Icons.playlist_play,
                size: 75,
                color: Theme.of(context).primaryColor,
              ),
              const SizedBox(height: 16),
              Text(
                'No playlists found',
                style: Theme.of(context).textTheme.titleLarge,
                textAlign: TextAlign.center,
              ),
              const SizedBox(height: 8),
              Text(
                'Create a smart playlist to get started!',
                style: Theme.of(context).textTheme.bodyMedium,
                textAlign: TextAlign.center,
              ),
              const SizedBox(height: 16),
              ElevatedButton.icon(
                onPressed: _createPlaylist,
                icon: const Icon(Icons.add),
                label: const Text('Create Playlist'),
              ),
            ],
          ),
        ),
      );
    }

    return SliverList(
      delegate: SliverChildListDelegate([
        Padding(
          padding: const EdgeInsets.all(16.0),
          child: Column(
            crossAxisAlignment: CrossAxisAlignment.start,
            children: [
              // Header with action buttons
              Row(
                mainAxisAlignment: MainAxisAlignment.spaceBetween,
                children: [
                  const Text(
                    'Smart Playlists',
                    style: TextStyle(
                      fontSize: 24,
                      fontWeight: FontWeight.bold,
                    ),
                  ),
                  Row(
                    children: [
                      if (_isSelectionMode) ...[
                        IconButton(
                          icon: const Icon(Icons.close),
                          onPressed: _toggleSelectionMode,
                          tooltip: 'Cancel',
                        ),
                        IconButton(
                          icon: const Icon(Icons.delete),
                          onPressed: _selectedPlaylists.isNotEmpty ? _deleteSelectedPlaylists : null,
                          tooltip: 'Delete selected (${_selectedPlaylists.length})',
                        ),
                      ] else ...[
                        IconButton(
                          icon: const Icon(Icons.select_all),
                          onPressed: _toggleSelectionMode,
                          tooltip: 'Select multiple',
                        ),
                        IconButton(
                          icon: const Icon(Icons.add),
                          onPressed: _createPlaylist,
                          tooltip: 'Create playlist',
                        ),
                      ],
                    ],
                  ),
                ],
              ),
              
              // Info banner for selection mode
              if (_isSelectionMode)
                Container(
                  margin: const EdgeInsets.only(top: 8, bottom: 16),
                  padding: const EdgeInsets.all(12),
                  decoration: BoxDecoration(
                    color: Theme.of(context).primaryColor.withOpacity(0.1),
                    borderRadius: BorderRadius.circular(8),
                    border: Border.all(
                      color: Theme.of(context).primaryColor.withOpacity(0.3),
                    ),
                  ),
                  child: Row(
                    children: [
                      Icon(
                        Icons.info_outline,
                        color: Theme.of(context).primaryColor,
                      ),
                      const SizedBox(width: 8),
                      const Expanded(
                        child: Text(
                          'System playlists cannot be deleted.',
                          style: TextStyle(fontSize: 14),
                        ),
                      ),
                    ],
                  ),
                ),
              
              const SizedBox(height: 8),
              
              // Playlists grid
              GridView.builder(
                shrinkWrap: true,
                physics: const NeverScrollableScrollPhysics(),
                gridDelegate: SliverGridDelegateWithFixedCrossAxisCount(
                  crossAxisCount: _getPlaylistCrossAxisCount(context),
                  crossAxisSpacing: 12,
                  mainAxisSpacing: 12,
                  childAspectRatio: _getPlaylistAspectRatio(context),
                ),
                itemCount: _playlists!.length,
                itemBuilder: (context, index) {
                  final playlist = _playlists![index];
                  final isSelected = _selectedPlaylists.contains(playlist.playlistId);
                  final canSelect = _isSelectionMode && !playlist.isSystemPlaylist;
                  
                  return GestureDetector(
                    onTap: () {
                      if (_isSelectionMode && !playlist.isSystemPlaylist) {
                        _togglePlaylistSelection(playlist.playlistId);
                      } else if (!_isSelectionMode) {
                        _openPlaylist(playlist);
                      }
                    },
                    child: Card(
                      elevation: isSelected ? 8 : 2,
                      shape: RoundedRectangleBorder(
                        borderRadius: BorderRadius.circular(12),
                      ),
                      color: isSelected 
                          ? Theme.of(context).primaryColor.withOpacity(0.1)
                          : null,
                      child: Stack(
                        children: [
                          Padding(
                            padding: const EdgeInsets.all(16.0),
                            child: Column(
                              crossAxisAlignment: CrossAxisAlignment.start,
                              children: [
                                Row(
                                  children: [
                                    Icon(
                                      _getPlaylistIcon(playlist.iconName),
                                      size: 32,
                                      color: Theme.of(context).primaryColor,
                                    ),
                                    const Spacer(),
                                    if (playlist.isSystemPlaylist)
                                      Container(
                                        padding: const EdgeInsets.symmetric(
                                          horizontal: 6,
                                          vertical: 2,
                                        ),
                                        decoration: BoxDecoration(
                                          color: Theme.of(context).colorScheme.secondary.withOpacity(0.2),
                                          borderRadius: BorderRadius.circular(8),
                                        ),
                                        child: Text(
                                          'System',
                                          style: TextStyle(
                                            fontSize: 10,
                                            color: Theme.of(context).colorScheme.secondary,
                                          ),
                                        ),
                                      ),
                                  ],
                                ),
                                const SizedBox(height: 12),
                                Text(
                                  playlist.name,
                                  style: const TextStyle(
                                    fontSize: 16,
                                    fontWeight: FontWeight.bold,
                                  ),
                                  maxLines: 2,
                                  overflow: TextOverflow.ellipsis,
                                ),
                                const SizedBox(height: 4),
                                Text(
                                  '${playlist.episodeCount ?? 0} episodes',
                                  style: TextStyle(
                                    fontSize: 12,
                                    color: Theme.of(context).textTheme.bodyMedium?.color,
                                  ),
                                ),
                                if (playlist.description != null && playlist.description!.isNotEmpty) ...[
                                  const SizedBox(height: 4),
                                  Text(
                                    playlist.description!,
                                    style: TextStyle(
                                      fontSize: 11,
                                      color: Theme.of(context).textTheme.bodySmall?.color,
                                    ),
                                    maxLines: 2,
                                    overflow: TextOverflow.ellipsis,
                                  ),
                                ],
                              ],
                            ),
                          ),
                          
                          // Selection checkbox
                          if (canSelect)
                            Positioned(
                              top: 8,
                              left: 8,
                              child: Checkbox(
                                value: isSelected,
                                onChanged: (value) {
                                  _togglePlaylistSelection(playlist.playlistId);
                                },
                              ),
                            ),
                          
                          // Delete button for non-system playlists (when not in selection mode)
                          if (!_isSelectionMode && !playlist.isSystemPlaylist)
                            Positioned(
                              top: 8,
                              right: 8,
                              child: IconButton(
                                icon: const Icon(Icons.delete_outline, size: 20),
                                onPressed: () => _deletePlaylist(playlist),
                                color: Theme.of(context).colorScheme.error.withOpacity(0.7),
                              ),
                            ),
                        ],
                      ),
                    ),
                  );
                },
              ),
            ],
          ),
        ),
      ]),
    );
  }
}