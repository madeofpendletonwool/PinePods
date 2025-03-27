// lib/ui/pinepods/feed.dart
import 'package:flutter/material.dart';
import 'package:pinepods_mobile/bloc/settings/settings_bloc.dart';
import 'package:provider/provider.dart';

class PinepodsFeed extends StatefulWidget {
  // Constructor with optional key parameter
  const PinepodsFeed({Key? key}) : super(key: key);

  @override
  State<PinepodsFeed> createState() => _PinepodsFeedState();
}

class _PinepodsFeedState extends State<PinepodsFeed> {
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
      _loadFeedContent();
    }
  }

  Future<void> _loadFeedContent() async {
    setState(() {
      _isLoading = true;
      _errorMessage = '';
    });

    // Here you would fetch feed content from your PinePods server
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
                'PinePods Feed',
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
                // Placeholder for feed items
                ListView.builder(
                  shrinkWrap: true,
                  physics: const NeverScrollableScrollPhysics(),
                  itemCount: 5, // Sample count
                  itemBuilder: (context, index) {
                    return Card(
                      margin: const EdgeInsets.only(bottom: 12.0),
                      child: Padding(
                        padding: const EdgeInsets.all(16.0),
                        child: Column(
                          crossAxisAlignment: CrossAxisAlignment.start,
                          children: [
                            Text(
                              'Feed Item ${index + 1}',
                              style: const TextStyle(
                                fontSize: 16,
                                fontWeight: FontWeight.bold,
                              ),
                            ),
                            const SizedBox(height: 8),
                            Text('Sample feed content for item ${index + 1}'),
                          ],
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