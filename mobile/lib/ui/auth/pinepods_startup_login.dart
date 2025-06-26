// lib/ui/auth/pinepods_startup_login.dart
import 'package:flutter/material.dart';
import 'package:pinepods_mobile/bloc/settings/settings_bloc.dart';
import 'package:provider/provider.dart';
import 'package:http/http.dart' as http;
import 'dart:convert';
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
  final _formKey = GlobalKey<FormState>();

  bool _isLoading = false;
  String _errorMessage = '';

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

  Future<bool> _verifyPinepodsInstance(String serverUrl) async {
    final normalizedUrl = serverUrl.trim().replaceAll(RegExp(r'/$'), '');
    final url = Uri.parse('$normalizedUrl/api/pinepods_check');

    try {
      final response = await http.get(url);

      if (response.statusCode == 200) {
        final data = jsonDecode(response.body);
        return data['pinepods_instance'] == true;
      }
      return false;
    } catch (e) {
      return false;
    }
  }

  Future<String?> _login(String serverUrl, String username, String password) async {
    final normalizedUrl = serverUrl.trim().replaceAll(RegExp(r'/$'), '');
    final credentials = base64Encode(utf8.encode('$username:$password'));
    final authHeader = 'Basic $credentials';
    final url = Uri.parse('$normalizedUrl/api/data/get_key');

    try {
      final response = await http.get(
        url,
        headers: {'Authorization': authHeader},
      );

      if (response.statusCode == 200) {
        final data = jsonDecode(response.body);
        return data['retrieved_key'];
      }
      return null;
    } catch (e) {
      return null;
    }
  }

  Future<bool> _verifyApiKey(String serverUrl, String apiKey) async {
    final url = Uri.parse('$serverUrl/api/data/verify_key');

    try {
      final response = await http.get(
        url,
        headers: {'Api-Key': apiKey},
      );

      if (response.statusCode == 200) {
        final data = jsonDecode(response.body);
        return data['status'] == 'success';
      }
      return false;
    } catch (e) {
      return false;
    }
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

      // Step 1: Verify this is a PinePods server
      final isPinepods = await _verifyPinepodsInstance(serverUrl);
      if (!isPinepods) {
        setState(() {
          _errorMessage = 'Not a valid PinePods server';
          _isLoading = false;
        });
        return;
      }

      // Step 2: Get API key
      final apiKey = await _login(
        serverUrl,
        _usernameController.text.trim(),
        _passwordController.text,
      );

      if (apiKey == null) {
        setState(() {
          _errorMessage = 'Login failed. Check your credentials.';
          _isLoading = false;
        });
        return;
      }

      // Step 3: Verify API key
      final isValidKey = await _verifyApiKey(serverUrl, apiKey);
      if (!isValidKey) {
        setState(() {
          _errorMessage = 'API key verification failed';
          _isLoading = false;
        });
        return;
      }

      // Step 4: Save the connection details
      var settingsBloc = Provider.of<SettingsBloc>(context, listen: false);
      settingsBloc.setPinepodsServer(serverUrl);
      settingsBloc.setPinepodsApiKey(apiKey);

      setState(() {
        _isLoading = false;
      });

      // Call success callback
      if (widget.onLoginSuccess != null) {
        widget.onLoginSuccess!();
      }
    } catch (e) {
      setState(() {
        _errorMessage = 'Error: ${e.toString()}';
        _isLoading = false;
      });
    }
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
                        Icon(
                          Icons.headset,
                          size: 64,
                          color: Theme.of(context).primaryColor,
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
                          textInputAction: TextInputAction.done,
                          onFieldSubmitted: (_) => _connectToPinepods(),
                        ),

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
                              : const Text(
                            'Connect to PinePods',
                            style: TextStyle(fontSize: 16),
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
    super.dispose();
  }
}