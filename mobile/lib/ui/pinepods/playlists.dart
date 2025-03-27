// lib/ui/pinepods/playlists.dart
import 'package:flutter/material.dart';
import 'package:pinepods_mobile/bloc/settings/settings_bloc.dart';
import 'package:provider/provider.dart';

class PinepodsPlaylists extends StatefulWidget {
  // Constructor with optional key parameter
  const PinepodsPlaylists({Key? key}) : super(key: key);

  @override
  State<PinepodsPlaylists> createState() => _PinepodsPlaylistsState();
}

class _PinepodsPlaylistsState extends State<PinepodsPlaylists> {
  bool _isLoading = false;
  String _errorMessage = '';

  @override
  void initState() {
    super.initState();
    _checkConnection();
  }

  void _checkConnection() {
    var settingsBloc = Provider.of<SettingsBloc>(context, listen: false);

    if (settingsBloc.currentSettings.pinepodsServer == null ||
        settingsBloc.currentSettings.pinepodsApiKey == null) {
      setState(() {
        _errorMessage = 'Not connected to PinePods server. Please connect in Settings.';
      });
    } else {
      _loadPlaylists();
    }
  }

  Future<void> _loadPlaylists() async {
    setState(() {
      _isLoading = true;
      _errorMessage = '';
    });

    // Here you would fetch playlists from your PinePods server
    await Future.delayed(const Duration(seconds: 1));

    setState(() {
      _isLoading = false;
    });
  }

  @override
  Widget build(BuildContext context) {
    return SliverList(
      delegate: SliverChildListDelegate([
        Padding(
          padding: const EdgeInsets.all(16.0),
          child: Column(
            crossAxisAlignment: CrossAxisAlignment.start,
            children: [
              Row(
                mainAxisAlignment: MainAxisAlignment.spaceBetween,
                children: [
                  const Text(
                    'PinePods Playlists',
                    style: TextStyle(
                      fontSize: 24,
                      fontWeight: FontWeight.bold,
                    ),
                  ),
                  IconButton(
                    icon: const Icon(Icons.add),
                    onPressed: () {
                      // Add new playlist functionality
                    },
                    tooltip: 'Create new playlist',
                  ),
                ],
              ),
              if (_errorMessage.isNotEmpty)
                Padding(
                  padding: const EdgeInsets.only(top: 16.0),
                  child: Text(
                    _errorMessage,
                    style: TextStyle(
                      color: Theme.of(context).colorScheme.error,
                    ),
                  ),
                ),
              if (_isLoading)
                const Padding(
                  padding: EdgeInsets.only(top: 16.0),
                  child: Center(
                    child: CircularProgressIndicator(),
                  ),
                ),
              if (!_isLoading && _errorMessage.isEmpty) ...[
                const SizedBox(height: 16),
                // Placeholder for playlists
                ListView.builder(
                  shrinkWrap: true,
                  physics: const NeverScrollableScrollPhysics(),
                  itemCount: 3, // Sample count
                  itemBuilder: (context, index) {
                    return Card(
                      margin: const EdgeInsets.only(bottom: 12.0),
                      child: ListTile(
                        leading: const Icon(Icons.playlist_play),
                        title: Text('Playlist ${index + 1}'),
                        subtitle: Text('${(index + 1) * 5} episodes'),
                        trailing: const Icon(Icons.arrow_forward_ios, size: 16),
                        onTap: () {
                          // Open playlist
                        },
                      ),
                    );
                  },
                ),
              ],
            ],
          ),
        ),
      ]),
    );
  }
}