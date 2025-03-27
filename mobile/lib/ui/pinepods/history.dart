// lib/ui/pinepods/history.dart
import 'package:flutter/material.dart';
import 'package:pinepods_mobile/bloc/settings/settings_bloc.dart';
import 'package:provider/provider.dart';

class PinepodsHistory extends StatefulWidget {
  // Constructor with optional key parameter
  const PinepodsHistory({Key? key}) : super(key: key);

  @override
  State<PinepodsHistory> createState() => _PinepodsHistoryState();
}

class _PinepodsHistoryState extends State<PinepodsHistory> {
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
      _loadHistory();
    }
  }

  Future<void> _loadHistory() async {
    setState(() {
      _isLoading = true;
      _errorMessage = '';
    });

    // Here you would fetch listening history from your PinePods server
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
                'Listening History',
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
                // Placeholder for history items
                ListView.builder(
                  shrinkWrap: true,
                  physics: const NeverScrollableScrollPhysics(),
                  itemCount: 10, // Sample count
                  itemBuilder: (context, index) {
                    // Calculate a fake date based on index (more recent items first)
                    final now = DateTime.now();
                    final date = now.subtract(Duration(days: index));

                    return Card(
                      margin: const EdgeInsets.only(bottom: 12.0),
                      child: ListTile(
                        leading: const Icon(Icons.history),
                        title: Text('History Item ${index + 1}'),
                        subtitle: Text('Listened on ${date.toString().substring(0, 10)}'),
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