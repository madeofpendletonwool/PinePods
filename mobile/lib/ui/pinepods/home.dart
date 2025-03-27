// lib/ui/pinepods/home.dart
import 'package:flutter/material.dart';
import 'package:pinepods_mobile/bloc/settings/settings_bloc.dart';
import 'package:provider/provider.dart';

class PinepodsHome extends StatefulWidget {
  const PinepodsHome({Key? key}) : super(key: key);

  @override
  State<PinepodsHome> createState() => _PinepodsHomeState();
}

class _PinepodsHomeState extends State<PinepodsHome> {
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
      _loadHomeContent();
    }
  }

  Future<void> _loadHomeContent() async {
    setState(() {
      _isLoading = true;
      _errorMessage = '';
    });

    // Here you would fetch content from your PinePods server
    // For now, we'll just simulate a delay
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
                'PinePods Home',
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
                Card(
                  child: Padding(
                    padding: const EdgeInsets.all(16.0),
                    child: Column(
                      crossAxisAlignment: CrossAxisAlignment.start,
                      children: [
                        const Text(
                          'Recently Played',
                          style: TextStyle(
                            fontSize: 18,
                            fontWeight: FontWeight.bold,
                          ),
                        ),
                        const SizedBox(height: 8),
                        // Placeholder for recently played episodes
                        Container(
                          height: 120,
                          color: Colors.grey.shade200,
                          child: const Center(
                            child: Text('Recently played episodes will appear here'),
                          ),
                        ),
                      ],
                    ),
                  ),
                ),
                const SizedBox(height: 16),
                Card(
                  child: Padding(
                    padding: const EdgeInsets.all(16.0),
                    child: Column(
                      crossAxisAlignment: CrossAxisAlignment.start,
                      children: [
                        const Text(
                          'New Episodes',
                          style: TextStyle(
                            fontSize: 18,
                            fontWeight: FontWeight.bold,
                          ),
                        ),
                        const SizedBox(height: 8),
                        // Placeholder for new episodes
                        Container(
                          height: 120,
                          color: Colors.grey.shade200,
                          child: const Center(
                            child: Text('New episodes from your subscriptions will appear here'),
                          ),
                        ),
                      ],
                    ),
                  ),
                ),
                const SizedBox(height: 16),
                Card(
                  child: Padding(
                    padding: const EdgeInsets.all(16.0),
                    child: Column(
                      crossAxisAlignment: CrossAxisAlignment.start,
                      children: [
                        const Text(
                          'Popular on PinePods',
                          style: TextStyle(
                            fontSize: 18,
                            fontWeight: FontWeight.bold,
                          ),
                        ),
                        const SizedBox(height: 8),
                        // Placeholder for popular podcasts
                        Container(
                          height: 120,
                          color: Colors.grey.shade200,
                          child: const Center(
                            child: Text('Popular podcasts will appear here'),
                          ),
                        ),
                      ],
                    ),
                  ),
                ),
              ],
            ],
          ),
        ),
      ]),
    );
  }
}