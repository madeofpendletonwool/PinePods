// lib/ui/settings/pinepods_login.dart
import 'package:flutter/material.dart';
import 'package:pinepods_mobile/bloc/settings/settings_bloc.dart';
import 'package:pinepods_mobile/l10n/L.dart';
import 'package:pinepods_mobile/services/pinepods/pinepods_service.dart';
import 'package:pinepods_mobile/services/pinepods/login_service.dart';
import 'package:pinepods_mobile/bloc/podcast/audio_bloc.dart';
import 'package:pinepods_mobile/ui/utils/local_download_utils.dart';
import 'package:pinepods_mobile/ui/settings/settings_section_label.dart';
import 'package:provider/provider.dart';
import 'package:http/http.dart' as http;
import 'dart:convert';
import 'package:pinepods_mobile/ui/icons/phosphor_icons.dart';

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
        _errorMessage = L.of(context)!.fill_in_all_fields;
      });
      return;
    }

    if (_showMfaField && _mfaController.text.isEmpty) {
      setState(() {
        _errorMessage = L.of(context)!.enter_mfa_code;
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
            _errorMessage = result.errorMessage ?? L.of(context)!.mfa_verification_failed;
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
            _errorMessage = L.of(context)!.enter_mfa_code;
          });
        } else {
          setState(() {
            _errorMessage = result.errorMessage ?? L.of(context)!.login_failed;
            _isLoading = false;
          });
        }
      }
    } catch (e) {
      setState(() {
        _errorMessage = L.of(context)!.error_with_detail(e.toString());
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

  void _logOut() {
    final settingsBloc = Provider.of<SettingsBloc>(context, listen: false);
    final navigator = Navigator.of(context);

    // Nothing from this account should keep playing or reporting.
    Provider.of<AudioBloc>(context, listen: false).transitionState(TransitionState.stop);
    LocalDownloadUtils.clearCache();

    // AuthWrapper swaps the home route to the login screen as soon as these
    // clear (the now-playing socket also disconnects off the same stream).
    settingsBloc.setPinepodsServer(null);
    settingsBloc.setPinepodsApiKey(null);
    settingsBloc.setPinepodsUserId(null);
    settingsBloc.setPinepodsUsername(null);
    settingsBloc.setPinepodsEmail(null);

    // Settings is pushed above home; drop back to the root so the login screen
    // is what's actually showing.
    navigator.popUntil((route) => route.isFirst);
  }

  @override
  Widget build(BuildContext context) {
    // Add a divider label for the PinePods section
    return Column(
      crossAxisAlignment: CrossAxisAlignment.start,
      children: [
        SettingsDividerLabel(label: L.of(context)!.pinepods_server),
        const Divider(),
        if (_isLoggedIn) ...[
          // Show connected status
          ListTile(
            title: Text(L.of(context)!.pinepods_connection),
            subtitle: Text(_connectedServer ?? ''),
            trailing: TextButton(
              onPressed: _logOut,
              child: Text(L.of(context)!.log_out),
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
                  decoration: InputDecoration(
                    labelText: L.of(context)!.server_url,
                    hintText: 'https://your-pinepods-server.com',
                  ),
                ),
                const SizedBox(height: 16),
                TextField(
                  controller: _usernameController,
                  decoration: InputDecoration(
                    labelText: L.of(context)!.username,
                  ),
                ),
                const SizedBox(height: 16),
                TextField(
                  controller: _passwordController,
                  decoration: InputDecoration(
                    labelText: L.of(context)!.password,
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
                      labelText: L.of(context)!.mfa_code,
                      hintText: L.of(context)!.mfa_code_hint,
                      suffixIcon: IconButton(
                        icon: const Icon(PhosphorIconsRegular.x),
                        onPressed: _resetMfa,
                        tooltip: L.of(context)!.cancel_mfa,
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
                        : Text(_showMfaField ? 'Verify MFA Code' : L.of(context)!.connect),
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