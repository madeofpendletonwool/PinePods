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
import 'package:pinepods_mobile/ui/icons/phosphor_icons.dart';
import 'package:pinepods_mobile/ui/widgets/soft_ui.dart';
import 'package:pinepods_mobile/l10n/L.dart';

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

  /// One column of rows on phones, more on tablets.
  int _getPlaylistCrossAxisCount(BuildContext context) {
    final screenWidth = MediaQuery.of(context).size.width;
    if (screenWidth > 1000) return 3;
    if (screenWidth > 640) return 2;
    return 1;
  }

  Future<void> _loadPlaylists() async {
    final settingsBloc = Provider.of<SettingsBloc>(context, listen: false);
    final settings = settingsBloc.currentSettings;

    if (settings.pinepodsServer == null || 
        settings.pinepodsApiKey == null || 
        settings.pinepodsUserId == null) {
      setState(() {
        _errorMessage = L.of(context)!.not_connected_connect_in_settings;
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
        title: Text(L.of(context)!.delete_playlists),
        content: Text(L.of(context)!.delete_playlists_confirm(_selectedPlaylists.length)),
        actions: [
          TextButton(
            onPressed: () => Navigator.of(context).pop(false),
            child: Text(L.of(context)!.cancel_button_label),
          ),
          TextButton(
            onPressed: () => Navigator.of(context).pop(true),
            child: Text(L.of(context)!.delete_label),
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
          SnackBar(content: Text(L.of(context)!.playlists_deleted)),
        );
      }
    } catch (e) {
      if (mounted) {
        ScaffoldMessenger.of(context).showSnackBar(
          SnackBar(content: Text(L.of(context)!.error_deleting_playlists(e.toString()))),
        );
      }
    }
  }

  Future<void> _deletePlaylist(PlaylistData playlist) async {
    final confirmed = await showDialog<bool>(
      context: context,
      builder: (context) => AlertDialog(
        title: Text(L.of(context)!.delete_playlist),
        content: Text(L.of(context)!.delete_playlist_confirm(playlist.name)),
        actions: [
          TextButton(
            onPressed: () => Navigator.of(context).pop(false),
            child: Text(L.of(context)!.cancel_button_label),
          ),
          TextButton(
            onPressed: () => Navigator.of(context).pop(true),
            child: Text(L.of(context)!.delete_label),
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
          SnackBar(content: Text(L.of(context)!.playlist_deleted)),
        );
      }
    } catch (e) {
      if (mounted) {
        ScaffoldMessenger.of(context).showSnackBar(
          SnackBar(content: Text(L.of(context)!.error_deleting_playlist(e.toString()))),
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
    if (iconName == null) return PhosphorIconsRegular.playlist;
    
    // Map common icon names to Material icons
    switch (iconName) {
      case 'ph-playlist':
        return PhosphorIconsRegular.playlist;
      case 'ph-music-notes':
        return PhosphorIconsRegular.musicNote;
      case 'ph-play-circle':
        return PhosphorIconsFill.playCircle;
      case 'ph-headphones':
        return PhosphorIconsRegular.headphones;
      case 'ph-star':
        return PhosphorIconsFill.star;
      case 'ph-heart':
        return PhosphorIconsFill.heart;
      case 'ph-bookmark':
        return PhosphorIconsFill.bookmarkSimple;
      case 'ph-clock':
        return PhosphorIconsRegular.clock;
      case 'ph-calendar':
        return PhosphorIconsRegular.calendarBlank;
      case 'ph-timer':
        return PhosphorIconsRegular.timer;
      case 'ph-shuffle':
        return PhosphorIconsRegular.shuffle;
      case 'ph-repeat':
        return PhosphorIconsRegular.repeat;
      case 'ph-microphone':
        return PhosphorIconsRegular.microphone;
      case 'ph-queue':
        return PhosphorIconsRegular.queue;
      default:
        return PhosphorIconsRegular.playlist;
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
        title: L.of(context)!.playlists_unavailable,
        subtitle: _errorMessage!.isServerConnectionError
          ? L.of(context)!.unable_to_connect_server
          : L.of(context)!.failed_to_load_playlists,
      );
    }

    if (_playlists == null || _playlists!.isEmpty) {
      return SliverFillRemaining(
        hasScrollBody: false,
        child: Center(
          child: SoftEmptyState(
            icon: PhosphorIconsRegular.playlist,
            title: L.of(context)!.no_playlists_yet,
            body: L.of(context)!.playlists_empty_body,
            action: FilledButton.icon(
              onPressed: _createPlaylist,
              icon: const Icon(PhosphorIconsRegular.plus),
              label: Text(L.of(context)!.create_playlist),
            ),
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
              SoftSectionHeader(
                title: L.of(context)!.playlists,
                count: _playlists!.length,
                padding: const EdgeInsets.only(bottom: 4),
                trailing: [
                  if (_isSelectionMode) ...[
                    IconButton(
                      icon: const Icon(PhosphorIconsRegular.x),
                      onPressed: _toggleSelectionMode,
                      tooltip: L.of(context)!.cancel_button_label,
                    ),
                    IconButton(
                      icon: const Icon(PhosphorIconsRegular.trash),
                      onPressed: _selectedPlaylists.isNotEmpty ? _deleteSelectedPlaylists : null,
                      tooltip: L.of(context)!.delete_selected_count(_selectedPlaylists.length),
                    ),
                  ] else ...[
                    IconButton(
                      icon: const Icon(PhosphorIconsRegular.selectionAll),
                      onPressed: _toggleSelectionMode,
                      tooltip: L.of(context)!.select_multiple,
                    ),
                    IconButton(
                      icon: const Icon(PhosphorIconsRegular.plus),
                      onPressed: _createPlaylist,
                      tooltip: L.of(context)!.create_playlist,
                    ),
                  ],
                ],
              ),

              // Info banner for selection mode
              if (_isSelectionMode)
                Container(
                  margin: const EdgeInsets.only(top: 8, bottom: 16),
                  padding: const EdgeInsets.all(12),
                  decoration: BoxDecoration(
                    color: Theme.of(context).primaryColor.withOpacity(0.1),
                    borderRadius: BorderRadius.circular(14),
                  ),
                  child: Row(
                    children: [
                      Icon(
                        PhosphorIconsRegular.info,
                        color: Theme.of(context).primaryColor,
                      ),
                      const SizedBox(width: 8),
                      Expanded(
                        child: Text(
                          L.of(context)!.system_playlists_cannot_be_deleted,
                          style: const TextStyle(fontSize: 14),
                        ),
                      ),
                    ],
                  ),
                ),
              
              const SizedBox(height: 8),
              
              // Playlists
              GridView.builder(
                shrinkWrap: true,
                padding: EdgeInsets.zero,
                physics: const NeverScrollableScrollPhysics(),
                gridDelegate: SliverGridDelegateWithFixedCrossAxisCount(
                  crossAxisCount: _getPlaylistCrossAxisCount(context),
                  crossAxisSpacing: 10,
                  mainAxisSpacing: 8,
                  mainAxisExtent: 76,
                ),
                itemCount: _playlists!.length,
                itemBuilder: (context, index) {
                  final playlist = _playlists![index];
                  return _PlaylistRow(
                    playlist: playlist,
                    icon: _getPlaylistIcon(playlist.iconName),
                    selectionMode: _isSelectionMode,
                    selected: _selectedPlaylists.contains(playlist.playlistId),
                    onTap: () {
                      if (_isSelectionMode && !playlist.isSystemPlaylist) {
                        _togglePlaylistSelection(playlist.playlistId);
                      } else if (!_isSelectionMode) {
                        _openPlaylist(playlist);
                      }
                    },
                    onDelete: () => _deletePlaylist(playlist),
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
class _PlaylistRow extends StatelessWidget {
  final PlaylistData playlist;
  final IconData icon;
  final bool selectionMode;
  final bool selected;
  final VoidCallback onTap;
  final VoidCallback onDelete;

  const _PlaylistRow({
    required this.playlist,
    required this.icon,
    required this.selectionMode,
    required this.selected,
    required this.onTap,
    required this.onDelete,
  });

  @override
  Widget build(BuildContext context) {
    final theme = Theme.of(context);
    final accent = theme.primaryColor;
    final count = playlist.episodeCount ?? 0;
    final description = playlist.description?.trim() ?? '';
    final subtitle = [
      L.of(context)!.episodes_count(count),
      if (description.isNotEmpty) description,
    ].join('  ·  ');

    final Widget trailing;
    if (selectionMode) {
      trailing = playlist.isSystemPlaylist
          ? const SizedBox(width: 48)
          : Checkbox(value: selected, onChanged: (_) => onTap());
    } else if (playlist.isSystemPlaylist) {
      trailing = Padding(
        padding: const EdgeInsets.only(right: 12),
        child: Container(
          padding: const EdgeInsets.symmetric(horizontal: 8, vertical: 3),
          decoration: BoxDecoration(
            color: theme.hintColor.withOpacity(0.12),
            borderRadius: BorderRadius.circular(20),
          ),
          child: Text(L.of(context)!.system, style: TextStyle(fontSize: 11, color: theme.hintColor)),
        ),
      );
    } else {
      trailing = PopupMenuButton<String>(
        tooltip: L.of(context)!.playlist_options,
        icon: Icon(PhosphorIconsRegular.dotsThreeVertical, color: theme.hintColor),
        onSelected: (_) => onDelete(),
        itemBuilder: (context) => [
          PopupMenuItem<String>(
            value: 'delete',
            child: ListTile(
              contentPadding: EdgeInsets.zero,
              leading: Icon(PhosphorIconsRegular.trash, color: theme.colorScheme.error),
              title: Text(L.of(context)!.delete_label, style: TextStyle(color: theme.colorScheme.error)),
            ),
          ),
        ],
      );
    }

    return Card(
      margin: EdgeInsets.zero,
      color: selected ? accent.withOpacity(0.12) : null,
      child: InkWell(
        onTap: onTap,
        child: Padding(
          padding: const EdgeInsets.only(left: 12),
          child: Row(
            children: [
              Container(
                width: 48,
                height: 48,
                decoration: BoxDecoration(
                  color: accent.withOpacity(0.14),
                  borderRadius: BorderRadius.circular(12),
                ),
                child: Icon(icon, color: accent, size: 26),
              ),
              const SizedBox(width: 14),
              Expanded(
                child: Column(
                  mainAxisAlignment: MainAxisAlignment.center,
                  crossAxisAlignment: CrossAxisAlignment.start,
                  children: [
                    Text(
                      playlist.name,
                      maxLines: 1,
                      overflow: TextOverflow.ellipsis,
                      style: const TextStyle(fontSize: 15, fontWeight: FontWeight.w600),
                    ),
                    const SizedBox(height: 3),
                    Text(
                      subtitle,
                      maxLines: 1,
                      overflow: TextOverflow.ellipsis,
                      style: TextStyle(fontSize: 12.5, color: theme.hintColor),
                    ),
                  ],
                ),
              ),
              trailing,
            ],
          ),
        ),
      ),
    );
  }
}
