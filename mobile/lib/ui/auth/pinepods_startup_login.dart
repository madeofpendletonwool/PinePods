// lib/ui/auth/pinepods_startup_login.dart
import 'package:flutter/material.dart';
import 'package:pinepods_mobile/bloc/settings/settings_bloc.dart';
import 'package:pinepods_mobile/services/pinepods/login_service.dart';
import 'package:provider/provider.dart';
import 'dart:math';

class PinepodsStartupLogin extends StatefulWidget {
  final VoidCallback? onLoginSuccess;

  const PinepodsStartupLogin({
    Key? key,
    this.onLoginSuccess,
  }) : super(key: key);

  @override
  State<PinepodsStartupLogin> createState() => _PinepodsStartupLoginState();
}

class _PinepodsStartupLoginState extends State<PinepodsStartupLogin> {
  final _serverController = TextEditingController();
  final _usernameController = TextEditingController();
  final _passwordController = TextEditingController();
  final _mfaController = TextEditingController();
  final _formKey = GlobalKey<FormState>();

  bool _isLoading = false;
  bool _showMfaField = false;
  String _errorMessage = '';
  String? _tempServerUrl;
  String? _tempApiKey;
  int? _tempUserId;

  // List of background images - you can add your own images to assets/images/
  final List<String> _backgroundImages = [
    'assets/images/1.jpg',
    'assets/images/2.jpg',
    'assets/images/3.jpg',
    'assets/images/4.jpg',
    'assets/images/5.jpg',
    'assets/images/6.jpg',
    'assets/images/7.jpg',
    'assets/images/8.jpg',
    'assets/images/9.jpg',
  ];

  late String _selectedBackground;

  @override
  void initState() {
    super.initState();
    // Select a random background image
    final random = Random();
    _selectedBackground = _backgroundImages[random.nextInt(_backgroundImages.length)];
  }

  Future<void> _connectToPinepods() async {
    if (!_formKey.currentState!.validate()) {
      return;
    }

    setState(() {
      _isLoading = true;
      _errorMessage = '';
    });

    try {
      final serverUrl = _serverController.text.trim();
      final username = _usernameController.text.trim();
      final password = _passwordController.text;
      final mfaCode = _showMfaField ? _mfaController.text.trim() : null;

      final result = await PinepodsLoginService.login(
        serverUrl,
        username,
        password,
        mfaCode: mfaCode,
      );

      if (result.isSuccess) {
        // Save the connection details including user ID
        final settingsBloc = Provider.of<SettingsBloc>(context, listen: false);
        settingsBloc.setPinepodsServer(result.serverUrl!);
        settingsBloc.setPinepodsApiKey(result.apiKey!);
        settingsBloc.setPinepodsUserId(result.userId!);

        // Fetch theme from server after successful login
        await settingsBloc.fetchThemeFromServer();

        setState(() {
          _isLoading = false;
        });

        // Call success callback
        if (widget.onLoginSuccess != null) {
          widget.onLoginSuccess!();
        }
      } else if (result.requiresMfa) {
        // Store temporary credentials and show MFA field
        setState(() {
          _tempServerUrl = result.serverUrl;
          _tempApiKey = result.apiKey;
          _tempUserId = result.userId;
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
      _tempApiKey = null;
      _tempUserId = null;
      _mfaController.clear();
      _errorMessage = '';
    });
  }


  @override
  Widget build(BuildContext context) {
    return Scaffold(
      body: Container(
        decoration: BoxDecoration(
          image: DecorationImage(
            image: AssetImage(_selectedBackground),
            fit: BoxFit.cover,
            colorFilter: ColorFilter.mode(
              Colors.black.withOpacity(0.6),
              BlendMode.darken,
            ),
          ),
        ),
        child: SafeArea(
          child: Center(
            child: SingleChildScrollView(
              padding: const EdgeInsets.all(24.0),
              child: Card(
                elevation: 8,
                shape: RoundedRectangleBorder(
                  borderRadius: BorderRadius.circular(16),
                ),
                child: Padding(
                  padding: const EdgeInsets.all(32.0),
                  child: Form(
                    key: _formKey,
                    child: Column(
                      mainAxisSize: MainAxisSize.min,
                      crossAxisAlignment: CrossAxisAlignment.stretch,
                      children: [
                        // App Logo/Title
                        Container(
                          width: 80,
                          height: 80,
                          decoration: BoxDecoration(
                            borderRadius: BorderRadius.circular(16),
                            boxShadow: [
                              BoxShadow(
                                color: Colors.black.withOpacity(0.1),
                                blurRadius: 8,
                                offset: const Offset(0, 4),
                              ),
                            ],
                          ),
                          child: ClipRRect(
                            borderRadius: BorderRadius.circular(16),
                            child: Image.asset(
                              'assets/images/favicon.png',
                              fit: BoxFit.cover,
                              errorBuilder: (context, error, stackTrace) {
                                return Container(
                                  decoration: BoxDecoration(
                                    color: Theme.of(context).primaryColor,
                                    borderRadius: BorderRadius.circular(16),
                                  ),
                                  child: Icon(
                                    Icons.headset,
                                    size: 48,
                                    color: Colors.white,
                                  ),
                                );
                              },
                            ),
                          ),
                        ),
                        const SizedBox(height: 16),
                        Text(
                          'Welcome to PinePods',
                          style: Theme.of(context).textTheme.headlineSmall?.copyWith(
                            fontWeight: FontWeight.bold,
                          ),
                          textAlign: TextAlign.center,
                        ),
                        const SizedBox(height: 8),
                        Text(
                          'Connect to your PinePods server to get started',
                          style: Theme.of(context).textTheme.bodyMedium?.copyWith(
                            color: Colors.grey[600],
                          ),
                          textAlign: TextAlign.center,
                        ),
                        const SizedBox(height: 32),

                        // Server URL Field
                        TextFormField(
                          controller: _serverController,
                          decoration: InputDecoration(
                            labelText: 'Server URL',
                            hintText: 'https://your-pinepods-server.com',
                            prefixIcon: const Icon(Icons.dns),
                            border: OutlineInputBorder(
                              borderRadius: BorderRadius.circular(12),
                            ),
                          ),
                          validator: (value) {
                            if (value == null || value.isEmpty) {
                              return 'Please enter a server URL';
                            }
                            if (!value.startsWith('http://') && !value.startsWith('https://')) {
                              return 'URL must start with http:// or https://';
                            }
                            return null;
                          },
                          textInputAction: TextInputAction.next,
                        ),
                        const SizedBox(height: 16),

                        // Username Field
                        TextFormField(
                          controller: _usernameController,
                          decoration: InputDecoration(
                            labelText: 'Username',
                            prefixIcon: const Icon(Icons.person),
                            border: OutlineInputBorder(
                              borderRadius: BorderRadius.circular(12),
                            ),
                          ),
                          validator: (value) {
                            if (value == null || value.isEmpty) {
                              return 'Please enter your username';
                            }
                            return null;
                          },
                          textInputAction: TextInputAction.next,
                        ),
                        const SizedBox(height: 16),

                        // Password Field
                        TextFormField(
                          controller: _passwordController,
                          decoration: InputDecoration(
                            labelText: 'Password',
                            prefixIcon: const Icon(Icons.lock),
                            border: OutlineInputBorder(
                              borderRadius: BorderRadius.circular(12),
                            ),
                          ),
                          obscureText: true,
                          validator: (value) {
                            if (value == null || value.isEmpty) {
                              return 'Please enter your password';
                            }
                            return null;
                          },
                          textInputAction: _showMfaField ? TextInputAction.next : TextInputAction.done,
                          onFieldSubmitted: (_) {
                            if (!_showMfaField) {
                              _connectToPinepods();
                            }
                          },
                          enabled: !_showMfaField,
                        ),
                        
                        // MFA Field (shown when MFA is required)
                        if (_showMfaField) ...[
                          const SizedBox(height: 16),
                          TextFormField(
                            controller: _mfaController,
                            decoration: InputDecoration(
                              labelText: 'MFA Code',
                              hintText: 'Enter 6-digit code',
                              prefixIcon: const Icon(Icons.security),
                              border: OutlineInputBorder(
                                borderRadius: BorderRadius.circular(12),
                              ),
                              suffixIcon: IconButton(
                                icon: const Icon(Icons.close),
                                onPressed: _resetMfa,
                                tooltip: 'Cancel MFA',
                              ),
                            ),
                            keyboardType: TextInputType.number,
                            maxLength: 6,
                            validator: (value) {
                              if (_showMfaField && (value == null || value.isEmpty)) {
                                return 'Please enter your MFA code';
                              }
                              if (_showMfaField && value!.length != 6) {
                                return 'MFA code must be 6 digits';
                              }
                              return null;
                            },
                            textInputAction: TextInputAction.done,
                            onFieldSubmitted: (_) => _connectToPinepods(),
                          ),
                        ],

                        // Error Message
                        if (_errorMessage.isNotEmpty) ...[
                          const SizedBox(height: 16),
                          Container(
                            padding: const EdgeInsets.all(12),
                            decoration: BoxDecoration(
                              color: Colors.red.shade50,
                              borderRadius: BorderRadius.circular(8),
                              border: Border.all(color: Colors.red.shade200),
                            ),
                            child: Row(
                              children: [
                                Icon(Icons.error_outline, color: Colors.red.shade700),
                                const SizedBox(width: 8),
                                Expanded(
                                  child: Text(
                                    _errorMessage,
                                    style: TextStyle(color: Colors.red.shade700),
                                  ),
                                ),
                              ],
                            ),
                          ),
                        ],

                        const SizedBox(height: 24),

                        // Connect Button
                        ElevatedButton(
                          onPressed: _isLoading ? null : _connectToPinepods,
                          style: ElevatedButton.styleFrom(
                            padding: const EdgeInsets.all(16),
                            shape: RoundedRectangleBorder(
                              borderRadius: BorderRadius.circular(12),
                            ),
                          ),
                          child: _isLoading
                              ? const SizedBox(
                            height: 20,
                            width: 20,
                            child: CircularProgressIndicator(
                              strokeWidth: 2,
                            ),
                          )
                              : Text(
                            _showMfaField ? 'Verify MFA Code' : 'Connect to PinePods',
                            style: const TextStyle(fontSize: 16),
                          ),
                        ),

                        const SizedBox(height: 16),

                        // Additional Info
                        Text(
                          'Don\'t have a PinePods server? Visit pinepods.online to learn more.',
                          style: Theme.of(context).textTheme.bodySmall?.copyWith(
                            color: Colors.grey[600],
                          ),
                          textAlign: TextAlign.center,
                        ),
                      ],
                    ),
                  ),
                ),
              ),
            ),
          ),
        ),
      ),
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