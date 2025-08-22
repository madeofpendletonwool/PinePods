// lib/ui/auth/pinepods_startup_login.dart
import 'package:flutter/material.dart';
import 'package:pinepods_mobile/bloc/settings/settings_bloc.dart';
import 'package:pinepods_mobile/services/pinepods/login_service.dart';
import 'package:pinepods_mobile/services/pinepods/oidc_service.dart';
import 'package:provider/provider.dart';
import 'dart:math';
import 'dart:async';

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
  bool _isLoadingOidc = false;
  String _errorMessage = '';
  String? _tempServerUrl;
  String? _tempUsername;
  int? _tempUserId;
  String? _tempMfaSessionToken;
  List<OidcProvider> _oidcProviders = [];
  bool _hasCheckedOidc = false;
  Timer? _oidcCheckTimer;

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
    
    // Listen for server URL changes to check OIDC providers
    _serverController.addListener(_onServerUrlChanged);
  }

  void _onServerUrlChanged() {
    final serverUrl = _serverController.text.trim();
    
    // Cancel any existing timer
    _oidcCheckTimer?.cancel();
    
    // Reset OIDC state
    setState(() {
      _oidcProviders.clear();
      _hasCheckedOidc = false;
      _isLoadingOidc = false;
    });
    
    // Only check if URL looks complete and valid
    if (serverUrl.isNotEmpty && 
        (serverUrl.startsWith('http://') || serverUrl.startsWith('https://')) &&
        _isValidUrl(serverUrl)) {
      
      // Debounce the API call - wait 1 second after user stops typing
      _oidcCheckTimer = Timer(const Duration(seconds: 1), () {
        _checkOidcProviders(serverUrl);
      });
    }
  }

  bool _isValidUrl(String url) {
    try {
      final uri = Uri.parse(url);
      // Check if it has a proper host (not just protocol)
      return uri.hasScheme && 
             uri.host.isNotEmpty && 
             uri.host.contains('.') && // Must have at least one dot for domain
             uri.host.length > 3; // Minimum reasonable length
    } catch (e) {
      return false;
    }
  }

  Future<void> _checkOidcProviders(String serverUrl) async {
    // Allow rechecking if server URL changed
    final currentUrl = _serverController.text.trim();
    if (currentUrl != serverUrl) return; // URL changed while we were waiting
    
    setState(() {
      _isLoadingOidc = true;
    });

    try {
      final providers = await OidcService.getPublicProviders(serverUrl);
      // Double-check the URL hasn't changed during the API call
      if (mounted && _serverController.text.trim() == serverUrl) {
        setState(() {
          _oidcProviders = providers;
          _hasCheckedOidc = true;
          _isLoadingOidc = false;
        });
      }
    } catch (e) {
      // Only update state if URL hasn't changed
      if (mounted && _serverController.text.trim() == serverUrl) {
        setState(() {
          _oidcProviders.clear();
          _hasCheckedOidc = true;
          _isLoadingOidc = false;
        });
      }
    }
  }

  // Manual retry when user focuses on other fields (like username)
  void _retryOidcCheck() {
    final serverUrl = _serverController.text.trim();
    if (serverUrl.isNotEmpty && 
        _isValidUrl(serverUrl) && 
        !_hasCheckedOidc && 
        !_isLoadingOidc) {
      _checkOidcProviders(serverUrl);
    }
  }

  Future<void> _handleOidcLogin(OidcProvider provider) async {
    final serverUrl = _serverController.text.trim();
    if (serverUrl.isEmpty) {
      setState(() {
        _errorMessage = 'Please enter a server URL first';
      });
      return;
    }

    setState(() {
      _isLoading = true;
      _errorMessage = '';
    });

    try {
      // Generate PKCE and state parameters for security
      final pkce = OidcService.generatePkce();
      final state = OidcService.generateState();
      
      // Store server URL for callback handling
      final settingsBloc = Provider.of<SettingsBloc>(context, listen: false);
      settingsBloc.setPinepodsServer(serverUrl); // Store temporarily for OIDC completion
      
      // Launch OIDC authentication
      final success = await OidcService.initiateOidcLogin(
        provider: provider,
        serverUrl: serverUrl,
        state: state,
        pkce: pkce,
      );

      if (!success) {
        setState(() {
          _errorMessage = 'Failed to launch OIDC authentication. Please check if you have a browser installed.';
          _isLoading = false;
        });
      } else {
        // Successfully launched browser, show a helpful message
        setState(() {
          _errorMessage = 'Authentication opened in browser. Please complete login and return to the app.';
          _isLoading = false;
        });
      }
      
    } catch (e) {
      setState(() {
        _errorMessage = 'OIDC login error: ${e.toString()}';
        _isLoading = false;
      });
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

  /// Parse hex color string to Color object
  Color _parseColor(String hexColor) {
    try {
      final hex = hexColor.replaceAll('#', '');
      if (hex.length == 6) {
        return Color(int.parse('FF$hex', radix: 16));
      } else if (hex.length == 8) {
        return Color(int.parse(hex, radix: 16));
      }
    } catch (e) {
      // Fallback to default color on parsing error
    }
    return Theme.of(context).primaryColor;
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
                        Focus(
                          onFocusChange: (hasFocus) {
                            if (hasFocus) {
                              // User focused on username field, retry OIDC check if needed
                              _retryOidcCheck();
                            }
                          },
                          child: TextFormField(
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

                        // OIDC Providers Section
                        if (_oidcProviders.isNotEmpty && !_showMfaField) ...[
                          // Divider
                          Row(
                            children: [
                              const Expanded(child: Divider()),
                              Padding(
                                padding: const EdgeInsets.symmetric(horizontal: 16),
                                child: Text(
                                  'Or continue with',
                                  style: Theme.of(context).textTheme.bodySmall?.copyWith(
                                    color: Colors.grey[600],
                                  ),
                                ),
                              ),
                              const Expanded(child: Divider()),
                            ],
                          ),
                          const SizedBox(height: 16),
                          
                          // OIDC Provider Buttons
                          ..._oidcProviders.map((provider) => Padding(
                            padding: const EdgeInsets.only(bottom: 8),
                            child: SizedBox(
                              width: double.infinity,
                              child: ElevatedButton(
                                onPressed: _isLoading ? null : () => _handleOidcLogin(provider),
                                style: ElevatedButton.styleFrom(
                                  backgroundColor: _parseColor(provider.buttonColorHex),
                                  foregroundColor: _parseColor(provider.buttonTextColorHex),
                                  padding: const EdgeInsets.all(16),
                                  shape: RoundedRectangleBorder(
                                    borderRadius: BorderRadius.circular(12),
                                  ),
                                ),
                                child: Row(
                                  mainAxisAlignment: MainAxisAlignment.center,
                                  children: [
                                    if (provider.iconSvg != null && provider.iconSvg!.isNotEmpty)
                                      Container(
                                        width: 20,
                                        height: 20,
                                        margin: const EdgeInsets.only(right: 8),
                                        child: const Icon(Icons.account_circle, size: 20),
                                      ),
                                    Text(
                                      provider.displayText,
                                      style: const TextStyle(fontSize: 16),
                                    ),
                                  ],
                                ),
                              ),
                            ),
                          )),
                          
                          const SizedBox(height: 16),
                        ],
                        
                        // Loading indicator for OIDC discovery
                        if (_isLoadingOidc) ...[
                          const SizedBox(
                            height: 20,
                            width: 20,
                            child: CircularProgressIndicator(strokeWidth: 2),
                          ),
                          const SizedBox(height: 16),
                        ],

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
    _oidcCheckTimer?.cancel();
    _serverController.removeListener(_onServerUrlChanged);
    _serverController.dispose();
    _usernameController.dispose();
    _passwordController.dispose();
    _mfaController.dispose();
    super.dispose();
  }
}