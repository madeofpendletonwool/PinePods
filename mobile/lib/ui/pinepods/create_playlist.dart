// lib/ui/pinepods/create_playlist.dart
import 'package:flutter/material.dart';
import 'package:pinepods_mobile/bloc/settings/settings_bloc.dart';
import 'package:pinepods_mobile/services/pinepods/pinepods_service.dart';
import 'package:provider/provider.dart';

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
  String _sortOrder = 'newest_first';
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
        const SnackBar(content: Text('Not connected to PinePods server')),
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
            const SnackBar(content: Text('Playlist created successfully!')),
          );
        }
      } else {
        if (mounted) {
          ScaffoldMessenger.of(context).showSnackBar(
            const SnackBar(content: Text('Failed to create playlist')),
          );
        }
      }
    } catch (e) {
      if (mounted) {
        ScaffoldMessenger.of(context).showSnackBar(
          SnackBar(content: Text('Error creating playlist: $e')),
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
        title: const Text('Create Playlist'),
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
              child: const Text('Create'),
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
              decoration: const InputDecoration(
                labelText: 'Playlist Name',
                hintText: 'Enter playlist name',
              ),
              validator: (value) {
                if (value == null || value.trim().isEmpty) {
                  return 'Please enter a playlist name';
                }
                return null;
              },
            ),
            
            const SizedBox(height: 16),
            
            // Description field
            TextFormField(
              controller: _descriptionController,
              decoration: const InputDecoration(
                labelText: 'Description (Optional)',
                hintText: 'Enter playlist description',
              ),
              maxLines: 3,
            ),
            
            const SizedBox(height: 16),
            
            // Icon selector
            Text(
              'Icon',
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
              'Episode Filters',
              style: Theme.of(context).textTheme.titleMedium,
            ),
            
            const SizedBox(height: 8),
            
            // Episode filters
            CheckboxListTile(
              title: const Text('Include Unplayed'),
              value: _includeUnplayed,
              onChanged: (value) {
                setState(() {
                  _includeUnplayed = value ?? true;
                });
              },
            ),
            CheckboxListTile(
              title: const Text('Include Partially Played'),
              value: _includePartiallyPlayed,
              onChanged: (value) {
                setState(() {
                  _includePartiallyPlayed = value ?? true;
                });
              },
            ),
            CheckboxListTile(
              title: const Text('Include Played'),
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
              'Duration Range (minutes)',
              style: Theme.of(context).textTheme.titleMedium,
            ),
            const SizedBox(height: 8),
            Row(
              children: [
                Expanded(
                  child: TextFormField(
                    decoration: const InputDecoration(
                      labelText: 'Min',
                      hintText: 'Any',
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
                    decoration: const InputDecoration(
                      labelText: 'Max',
                      hintText: 'Any',
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
              decoration: const InputDecoration(
                labelText: 'Sort Order',
              ),
              items: const [
                DropdownMenuItem(value: 'newest_first', child: Text('Newest First')),
                DropdownMenuItem(value: 'oldest_first', child: Text('Oldest First')),
                DropdownMenuItem(value: 'shortest_first', child: Text('Shortest First')),
                DropdownMenuItem(value: 'longest_first', child: Text('Longest First')),
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
              decoration: const InputDecoration(
                labelText: 'Max Episodes (Optional)',
                hintText: 'Leave blank for no limit',
              ),
              keyboardType: TextInputType.number,
              onChanged: (value) {
                _maxEpisodes = value;
              },
            ),
            
            const SizedBox(height: 16),
            
            // Group by podcast
            CheckboxListTile(
              title: const Text('Group by Podcast'),
              subtitle: const Text('Group episodes by their podcast'),
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