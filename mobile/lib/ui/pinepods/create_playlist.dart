// lib/ui/pinepods/create_playlist.dart
import 'package:flutter/material.dart';
import 'package:pinepods_mobile/bloc/settings/settings_bloc.dart';
import 'package:pinepods_mobile/services/pinepods/pinepods_service.dart';
import 'package:provider/provider.dart';
import 'package:pinepods_mobile/l10n/L.dart';

/// The sort orders the server accepts, in menu order.
const playlistSortOptions = ['date_desc', 'date_asc', 'duration_asc', 'duration_desc'];

String _playlistSortLabel(BuildContext context, String order) {
  final l = L.of(context)!;
  return switch (order) {
    'date_asc' => l.sort_oldest_first,
    'duration_asc' => l.sort_shortest_first,
    'duration_desc' => l.sort_longest_first,
    _ => l.sort_newest_first,
  };
}

class CreatePlaylistPage extends StatefulWidget {
  const CreatePlaylistPage({Key? key}) : super(key: key);

  @override
  State<CreatePlaylistPage> createState() => _CreatePlaylistPageState();
}

class _CreatePlaylistPageState extends State<CreatePlaylistPage> {
  final _formKey = GlobalKey<FormState>();
  final _nameController = TextEditingController();
  final _descriptionController = TextEditingController();
  final PinepodsService _pinepodsService = PinepodsService();
  
  bool _isLoading = false;
  String _selectedIcon = 'ph-playlist';
  bool _includeUnplayed = true;
  bool _includePartiallyPlayed = true;
  bool _includePlayed = false;
  String _minDuration = '';
  String _maxDuration = '';
  String _sortOrder = 'date_desc';
  bool _groupByPodcast = false;
  String _maxEpisodes = '';

  final List<Map<String, String>> _availableIcons = [
    {'name': 'ph-playlist', 'icon': '🎵'},
    {'name': 'ph-music-notes', 'icon': '🎶'},
    {'name': 'ph-play-circle', 'icon': '▶️'},
    {'name': 'ph-headphones', 'icon': '🎧'},
    {'name': 'ph-star', 'icon': '⭐'},
    {'name': 'ph-heart', 'icon': '❤️'},
    {'name': 'ph-bookmark', 'icon': '🔖'},
    {'name': 'ph-clock', 'icon': '⏰'},
    {'name': 'ph-calendar', 'icon': '📅'},
    {'name': 'ph-timer', 'icon': '⏲️'},
    {'name': 'ph-shuffle', 'icon': '🔀'},
    {'name': 'ph-repeat', 'icon': '🔁'},
    {'name': 'ph-microphone', 'icon': '🎤'},
    {'name': 'ph-queue', 'icon': '📋'},
    {'name': 'ph-fire', 'icon': '🔥'},
    {'name': 'ph-lightning', 'icon': '⚡'},
    {'name': 'ph-coffee', 'icon': '☕'},
    {'name': 'ph-moon', 'icon': '🌙'},
    {'name': 'ph-sun', 'icon': '☀️'},
    {'name': 'ph-rocket', 'icon': '🚀'},
  ];

  @override
  void dispose() {
    _nameController.dispose();
    _descriptionController.dispose();
    super.dispose();
  }

  Future<void> _createPlaylist() async {
    if (!_formKey.currentState!.validate()) {
      return;
    }

    final settingsBloc = Provider.of<SettingsBloc>(context, listen: false);
    final settings = settingsBloc.currentSettings;

    if (settings.pinepodsServer == null || 
        settings.pinepodsApiKey == null || 
        settings.pinepodsUserId == null) {
      ScaffoldMessenger.of(context).showSnackBar(
        SnackBar(content: Text(L.of(context)!.not_connected_to_pinepods)),
      );
      return;
    }

    setState(() {
      _isLoading = true;
    });

    try {
      _pinepodsService.setCredentials(
        settings.pinepodsServer!,
        settings.pinepodsApiKey!,
      );

      final request = CreatePlaylistRequest(
        userId: settings.pinepodsUserId!,
        name: _nameController.text.trim(),
        description: _descriptionController.text.trim().isNotEmpty 
            ? _descriptionController.text.trim() 
            : null,
        podcastIds: const [], // For now, we'll create without podcast filtering
        includeUnplayed: _includeUnplayed,
        includePartiallyPlayed: _includePartiallyPlayed,
        includePlayed: _includePlayed,
        minDuration: _minDuration.isNotEmpty ? int.tryParse(_minDuration) : null,
        maxDuration: _maxDuration.isNotEmpty ? int.tryParse(_maxDuration) : null,
        sortOrder: _sortOrder,
        groupByPodcast: _groupByPodcast,
        maxEpisodes: _maxEpisodes.isNotEmpty ? int.tryParse(_maxEpisodes) : null,
        iconName: _selectedIcon,
        playProgressMin: null, // Simplified for now
        playProgressMax: null,
        timeFilterHours: null,
      );

      final success = await _pinepodsService.createPlaylist(request);

      if (success) {
        if (mounted) {
          Navigator.of(context).pop(true); // Return true to indicate success
          ScaffoldMessenger.of(context).showSnackBar(
            SnackBar(content: Text(L.of(context)!.playlist_created)),
          );
        }
      } else {
        if (mounted) {
          ScaffoldMessenger.of(context).showSnackBar(
            SnackBar(content: Text(L.of(context)!.failed_to_create_playlist)),
          );
        }
      }
    } catch (e) {
      if (mounted) {
        ScaffoldMessenger.of(context).showSnackBar(
          SnackBar(content: Text(L.of(context)!.error_creating_playlist(e.toString()))),
        );
      }
    } finally {
      if (mounted) {
        setState(() {
          _isLoading = false;
        });
      }
    }
  }

  @override
  Widget build(BuildContext context) {
    return Scaffold(
      appBar: AppBar(
        title: Text(L.of(context)!.create_playlist_title),
        backgroundColor: Theme.of(context).scaffoldBackgroundColor,
        elevation: 0,
        actions: [
          if (_isLoading)
            const Padding(
              padding: EdgeInsets.all(16.0),
              child: SizedBox(
                width: 20,
                height: 20,
                child: CircularProgressIndicator(strokeWidth: 2),
              ),
            )
          else
            TextButton(
              onPressed: _createPlaylist,
              child: Text(L.of(context)!.create),
            ),
        ],
      ),
      body: Form(
        key: _formKey,
        child: ListView(
          padding: const EdgeInsets.all(16.0),
          children: [
            // Name field
            TextFormField(
              controller: _nameController,
              decoration: InputDecoration(
                labelText: L.of(context)!.playlist_name,
                hintText: L.of(context)!.playlist_name_hint,
              ),
              validator: (value) {
                if (value == null || value.trim().isEmpty) {
                  return L.of(context)!.playlist_name_required;
                }
                return null;
              },
            ),
            
            const SizedBox(height: 16),
            
            // Description field
            TextFormField(
              controller: _descriptionController,
              decoration: InputDecoration(
                labelText: L.of(context)!.description_optional,
                hintText: L.of(context)!.playlist_description_hint,
              ),
              maxLines: 3,
            ),
            
            const SizedBox(height: 16),
            
            // Icon selector
            Text(
              L.of(context)!.icon,
              style: Theme.of(context).textTheme.titleMedium,
            ),
            const SizedBox(height: 8),
            Container(
              height: 120,
              decoration: BoxDecoration(
                border: Border.all(color: Colors.grey.shade300),
                borderRadius: BorderRadius.circular(8),
              ),
              child: GridView.builder(
                padding: const EdgeInsets.all(8),
                gridDelegate: const SliverGridDelegateWithFixedCrossAxisCount(
                  crossAxisCount: 5,
                  crossAxisSpacing: 8,
                  mainAxisSpacing: 8,
                ),
                itemCount: _availableIcons.length,
                itemBuilder: (context, index) {
                  final icon = _availableIcons[index];
                  final isSelected = _selectedIcon == icon['name'];
                  
                  return GestureDetector(
                    onTap: () {
                      setState(() {
                        _selectedIcon = icon['name']!;
                      });
                    },
                    child: Container(
                      decoration: BoxDecoration(
                        color: isSelected 
                            ? Theme.of(context).primaryColor.withOpacity(0.2)
                            : null,
                        border: Border.all(
                          color: isSelected 
                              ? Theme.of(context).primaryColor
                              : Colors.grey.shade300,
                          width: isSelected ? 2 : 1,
                        ),
                        borderRadius: BorderRadius.circular(6),
                      ),
                      child: Center(
                        child: Text(
                          icon['icon']!,
                          style: const TextStyle(fontSize: 20),
                        ),
                      ),
                    ),
                  );
                },
              ),
            ),
            
            const SizedBox(height: 24),
            
            Text(
              L.of(context)!.episode_filters,
              style: Theme.of(context).textTheme.titleMedium,
            ),
            
            const SizedBox(height: 8),
            
            // Episode filters
            CheckboxListTile(
              title: Text(L.of(context)!.include_unplayed),
              value: _includeUnplayed,
              onChanged: (value) {
                setState(() {
                  _includeUnplayed = value ?? true;
                });
              },
            ),
            CheckboxListTile(
              title: Text(L.of(context)!.include_partially_played),
              value: _includePartiallyPlayed,
              onChanged: (value) {
                setState(() {
                  _includePartiallyPlayed = value ?? true;
                });
              },
            ),
            CheckboxListTile(
              title: Text(L.of(context)!.include_played),
              value: _includePlayed,
              onChanged: (value) {
                setState(() {
                  _includePlayed = value ?? false;
                });
              },
            ),
            
            const SizedBox(height: 16),
            
            // Duration range
            Text(
              L.of(context)!.duration_range_minutes,
              style: Theme.of(context).textTheme.titleMedium,
            ),
            const SizedBox(height: 8),
            Row(
              children: [
                Expanded(
                  child: TextFormField(
                    decoration: InputDecoration(
                      labelText: L.of(context)!.min,
                      hintText: L.of(context)!.any,
                    ),
                    keyboardType: TextInputType.number,
                    onChanged: (value) {
                      _minDuration = value;
                    },
                  ),
                ),
                const SizedBox(width: 16),
                Expanded(
                  child: TextFormField(
                    decoration: InputDecoration(
                      labelText: L.of(context)!.max,
                      hintText: L.of(context)!.any,
                    ),
                    keyboardType: TextInputType.number,
                    onChanged: (value) {
                      _maxDuration = value;
                    },
                  ),
                ),
              ],
            ),
            
            const SizedBox(height: 16),
            
            // Sort order
            DropdownButtonFormField<String>(
              value: _sortOrder,
              decoration: InputDecoration(
                labelText: L.of(context)!.sort_order,
              ),
              items: [
                for (final order in playlistSortOptions)
                  DropdownMenuItem(value: order, child: Text(_playlistSortLabel(context, order))),
              ],
              onChanged: (value) {
                setState(() {
                  _sortOrder = value!;
                });
              },
            ),
            
            const SizedBox(height: 16),
            
            // Max episodes
            TextFormField(
              decoration: InputDecoration(
                labelText: L.of(context)!.max_episodes_optional,
                hintText: L.of(context)!.leave_blank_no_limit,
              ),
              keyboardType: TextInputType.number,
              onChanged: (value) {
                _maxEpisodes = value;
              },
            ),
            
            const SizedBox(height: 16),
            
            // Group by podcast
            CheckboxListTile(
              title: Text(L.of(context)!.group_by_podcast),
              subtitle: Text(L.of(context)!.group_by_podcast_description),
              value: _groupByPodcast,
              onChanged: (value) {
                setState(() {
                  _groupByPodcast = value ?? false;
                });
              },
            ),
            
            const SizedBox(height: 32),
          ],
        ),
      ),
    );
  }
}