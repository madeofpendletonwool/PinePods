// lib/ui/pinepods/saved.dart
import 'package:flutter/material.dart';
import 'package:pinepods_mobile/bloc/settings/settings_bloc.dart';
import 'package:provider/provider.dart';

class PinepodsSaved extends StatefulWidget {
  // Constructor with optional key parameter
  const PinepodsSaved({Key? key}) : super(key: key);

  @override
  State<PinepodsSaved> createState() => _PinepodsSavedState();
}

class _PinepodsSavedState extends State<PinepodsSaved> {
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
      _loadSavedEpisodes();
    }
  }

  Future<void> _loadSavedEpisodes() async {
    setState(() {
      _isLoading = true;
      _errorMessage = '';
    });

    // Here you would fetch saved episodes from your PinePods server
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
              const Text(
                'Saved Episodes',
                style: TextStyle(
                  fontSize: 24,
                  fontWeight: FontWeight.bold,
                ),
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
                // Placeholder for saved episodes
                ListView.builder(
                  shrinkWrap: true,
                  physics: const NeverScrollableScrollPhysics(),
                  itemCount: 5, // Sample count
                  itemBuilder: (context, index) {
                    return Card(
                      margin: const EdgeInsets.only(bottom: 12.0),
                      child: ListTile(
                        leading: const Icon(Icons.bookmark),
                        title: Text('Saved Episode ${index + 1}'),
                        subtitle: Text('Saved on ${DateTime.now().toString().substring(0, 10)}'),
                        trailing: IconButton(
                          icon: const Icon(Icons.play_arrow),
                          onPressed: () {
                            // Play episode
                          },
                        ),
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