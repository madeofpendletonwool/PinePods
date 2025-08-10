// lib/ui/settings/pinepods_login.dart
import 'package:flutter/material.dart';
import 'package:pinepods_mobile/bloc/settings/settings_bloc.dart';
import 'package:pinepods_mobile/l10n/L.dart';
import 'package:pinepods_mobile/services/pinepods/pinepods_service.dart';
import 'package:pinepods_mobile/services/pinepods/login_service.dart';
import 'package:pinepods_mobile/ui/widgets/restart_widget.dart';
import 'package:provider/provider.dart';
import 'package:http/http.dart' as http;
import 'dart:convert';

class PinepodsLoginWidget extends StatefulWidget {
  const PinepodsLoginWidget({Key? key}) : super(key: key);

  @override
  State<PinepodsLoginWidget> createState() => _PinepodsLoginWidgetState();
}

class _PinepodsLoginWidgetState extends State<PinepodsLoginWidget> {
  final _serverController = TextEditingController();
  final _usernameController = TextEditingController();
  final _passwordController = TextEditingController();
  final _mfaController = TextEditingController();

  bool _isLoading = false;
  bool _showMfaField = false;
  String _errorMessage = '';
  bool _isLoggedIn = false;
  String? _connectedServer;
  String? _tempServerUrl;
  String? _tempUsername;
  int? _tempUserId;
  String? _tempMfaSessionToken;

  @override
  void initState() {
    super.initState();
    // Initialize UI based on saved settings
    _loadSavedSettings();
  }

  void _loadSavedSettings() {
    var settingsBloc = Provider.of<SettingsBloc>(context, listen: false);
    var settings = settingsBloc.currentSettings;

    // Check if we have PinePods settings
    setState(() {
      _isLoggedIn = false;
      _connectedServer = null;

      // We'll add these properties to AppSettings in the next step
      if (settings.pinepodsServer != null &&
          settings.pinepodsServer!.isNotEmpty &&
          settings.pinepodsApiKey != null &&
          settings.pinepodsApiKey!.isNotEmpty) {
        _isLoggedIn = true;
        _connectedServer = settings.pinepodsServer;
      }
    });
  }

  Future<void> _connectToPinepods() async {
    if (!_showMfaField && (_serverController.text.isEmpty ||
        _usernameController.text.isEmpty ||
        _passwordController.text.isEmpty)) {
      setState(() {
        _errorMessage = 'Please fill in all fields';
      });
      return;
    }

    if (_showMfaField && _mfaController.text.isEmpty) {
      setState(() {
        _errorMessage = 'Please enter your MFA code';
      });
      return;
    }

    setState(() {
      _isLoading = true;
      _errorMessage = '';
    });

    try {
      if (_showMfaField && _tempMfaSessionToken != null) {
        // Complete MFA login flow
        final mfaCode = _mfaController.text.trim();
        final result = await PinepodsLoginService.completeMfaLogin(
          serverUrl: _tempServerUrl!,
          username: _tempUsername!,
          mfaSessionToken: _tempMfaSessionToken!,
          mfaCode: mfaCode,
        );
        
        if (result.isSuccess) {
          // Save the connection details including user ID
          var settingsBloc = Provider.of<SettingsBloc>(context, listen: false);
          settingsBloc.setPinepodsServer(result.serverUrl!);
          settingsBloc.setPinepodsApiKey(result.apiKey!);
          settingsBloc.setPinepodsUserId(result.userId!);

          setState(() {
            _isLoggedIn = true;
            _connectedServer = _tempServerUrl;
            _showMfaField = false;
            _tempServerUrl = null;
            _tempUsername = null;
            _tempUserId = null;
            _tempMfaSessionToken = null;
            _isLoading = false;
          });
        } else {
          setState(() {
            _errorMessage = result.errorMessage ?? 'MFA verification failed';
            _isLoading = false;
          });
        }
      } else {
        // Initial login flow
        final serverUrl = _serverController.text.trim();
        final username = _usernameController.text.trim();
        final password = _passwordController.text;

        final result = await PinepodsLoginService.login(
          serverUrl,
          username,
          password,
        );

        if (result.isSuccess) {
          // Save the connection details including user ID
          var settingsBloc = Provider.of<SettingsBloc>(context, listen: false);
          settingsBloc.setPinepodsServer(result.serverUrl!);
          settingsBloc.setPinepodsApiKey(result.apiKey!);
          settingsBloc.setPinepodsUserId(result.userId!);

          setState(() {
            _isLoggedIn = true;
            _connectedServer = serverUrl;
            _isLoading = false;
          });
        } else if (result.requiresMfa) {
          // Store MFA session info and show MFA field
          setState(() {
            _tempServerUrl = result.serverUrl;
            _tempUsername = result.username;
            _tempUserId = result.userId;
            _tempMfaSessionToken = result.mfaSessionToken;
            _showMfaField = true;
            _isLoading = false;
            _errorMessage = 'Please enter your MFA code';
          });
        } else {
          setState(() {
            _errorMessage = result.errorMessage ?? 'Login failed';
            _isLoading = false;
          });
        }
      }
    } catch (e) {
      setState(() {
        _errorMessage = 'Error: ${e.toString()}';
        _isLoading = false;
      });
    }
  }

  void _resetMfa() {
    setState(() {
      _showMfaField = false;
      _tempServerUrl = null;
      _tempUsername = null;
      _tempUserId = null;
      _tempMfaSessionToken = null;
      _mfaController.clear();
      _errorMessage = '';
    });
  }

  void _logOut() async {
    var settingsBloc = Provider.of<SettingsBloc>(context, listen: false);

    // Clear all PinePods user data
    settingsBloc.setPinepodsServer(null);
    settingsBloc.setPinepodsApiKey(null);
    settingsBloc.setPinepodsUserId(null);
    settingsBloc.setPinepodsUsername(null);
    settingsBloc.setPinepodsEmail(null);

    setState(() {
      _isLoggedIn = false;
      _connectedServer = null;
    });

    // Wait for the settings to be processed and then restart the app
    WidgetsBinding.instance.addPostFrameCallback((_) async {
      await Future.delayed(const Duration(milliseconds: 100));
      if (mounted) {
        // Restart the entire app to reset all state
        RestartWidget.restartApp(context);
      }
    });
  }

  @override
  Widget build(BuildContext context) {
    // Add a divider label for the PinePods section
    return Column(
      crossAxisAlignment: CrossAxisAlignment.start,
      children: [
        const Padding(
          padding: EdgeInsets.only(left: 16.0, top: 16.0, bottom: 8.0),
          child: Text(
            'PinePods Server',
            style: TextStyle(
              fontSize: 14.0,
              fontWeight: FontWeight.bold,
            ),
          ),
        ),
        const Divider(),
        if (_isLoggedIn) ...[
          // Show connected status
          ListTile(
            title: const Text('PinePods Connection'),
            subtitle: Text(_connectedServer ?? ''),
            trailing: TextButton(
              onPressed: _logOut,
              child: const Text('Log Out'),
            ),
          ),
        ] else ...[
          // Show login form
          Padding(
            padding: const EdgeInsets.all(16.0),
            child: Column(
              crossAxisAlignment: CrossAxisAlignment.start,
              children: [
                TextField(
                  controller: _serverController,
                  decoration: const InputDecoration(
                    labelText: 'Server URL',
                    hintText: 'https://your-pinepods-server.com',
                  ),
                ),
                const SizedBox(height: 16),
                TextField(
                  controller: _usernameController,
                  decoration: const InputDecoration(
                    labelText: 'Username',
                  ),
                ),
                const SizedBox(height: 16),
                TextField(
                  controller: _passwordController,
                  decoration: const InputDecoration(
                    labelText: 'Password',
                  ),
                  obscureText: true,
                  enabled: !_showMfaField,
                ),
                // MFA Field (shown when MFA is required)
                if (_showMfaField) ...[
                  const SizedBox(height: 16),
                  TextField(
                    controller: _mfaController,
                    decoration: InputDecoration(
                      labelText: 'MFA Code',
                      hintText: 'Enter 6-digit code',
                      suffixIcon: IconButton(
                        icon: const Icon(Icons.close),
                        onPressed: _resetMfa,
                        tooltip: 'Cancel MFA',
                      ),
                    ),
                    keyboardType: TextInputType.number,
                    maxLength: 6,
                  ),
                ],
                if (_errorMessage.isNotEmpty) ...[
                  const SizedBox(height: 16),
                  Text(
                    _errorMessage,
                    style: TextStyle(color: Theme.of(context).colorScheme.error),
                  ),
                ],
                const SizedBox(height: 16),
                SizedBox(
                  width: double.infinity,
                  child: ElevatedButton(
                    onPressed: _isLoading ? null : _connectToPinepods,
                    child: _isLoading
                        ? const SizedBox(
                      height: 20,
                      width: 20,
                      child: CircularProgressIndicator(
                        strokeWidth: 2,
                      ),
                    )
                        : Text(_showMfaField ? 'Verify MFA Code' : 'Connect'),
                  ),
                ),
              ],
            ),
          ),
        ],
      ],
    );
  }

  @override
  void dispose() {
    _serverController.dispose();
    _usernameController.dispose();
    _passwordController.dispose();
    _mfaController.dispose();
    super.dispose();
  }
}