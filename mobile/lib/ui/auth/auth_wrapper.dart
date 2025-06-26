// lib/ui/auth/auth_wrapper.dart
import 'package:flutter/material.dart';
import 'package:pinepods_mobile/bloc/settings/settings_bloc.dart';
import 'package:pinepods_mobile/entities/app_settings.dart';
import 'package:pinepods_mobile/ui/auth/pinepods_startup_login.dart';
import 'package:provider/provider.dart';

class AuthWrapper extends StatelessWidget {
  final Widget child;

  const AuthWrapper({
    Key? key,
    required this.child,
  }) : super(key: key);

  @override
  Widget build(BuildContext context) {
    return Consumer<SettingsBloc>(
      builder: (context, settingsBloc, _) {
        return StreamBuilder<AppSettings>(
          stream: settingsBloc.settings,
          initialData: settingsBloc.currentSettings,
          builder: (context, snapshot) {
            if (!snapshot.hasData) {
              return const Scaffold(
                body: Center(
                  child: CircularProgressIndicator(),
                ),
              );
            }

            final settings = snapshot.data!;

            // Check if PinePods server is configured
            final hasServer = settings.pinepodsServer != null &&
                settings.pinepodsServer!.isNotEmpty;
            final hasApiKey = settings.pinepodsApiKey != null &&
                settings.pinepodsApiKey!.isNotEmpty;

            if (hasServer && hasApiKey) {
              // User is logged in, show main app
              return child;
            } else {
              // User needs to login, show startup login
              return PinepodsStartupLogin(
                onLoginSuccess: () {
                  // Force rebuild to check auth state again
                  // The StreamBuilder will automatically rebuild when settings change
                },
              );
            }
          },
        );
      },
    );
  }
}

// Alternative version if you want more explicit control
class AuthChecker extends StatefulWidget {
  final Widget authenticatedChild;
  final Widget? unauthenticatedChild;

  const AuthChecker({
    Key? key,
    required this.authenticatedChild,
    this.unauthenticatedChild,
  }) : super(key: key);

  @override
  State<AuthChecker> createState() => _AuthCheckerState();
}

class _AuthCheckerState extends State<AuthChecker> {
  bool _isCheckingAuth = true;
  bool _isAuthenticated = false;

  @override
  void initState() {
    super.initState();
    _checkAuthStatus();
  }

  void _checkAuthStatus() {
    final settingsBloc = Provider.of<SettingsBloc>(context, listen: false);
    final settings = settingsBloc.currentSettings;

    final hasServer = settings.pinepodsServer != null &&
        settings.pinepodsServer!.isNotEmpty;
    final hasApiKey = settings.pinepodsApiKey != null &&
        settings.pinepodsApiKey!.isNotEmpty;

    setState(() {
      _isAuthenticated = hasServer && hasApiKey;
      _isCheckingAuth = false;
    });
  }

  void _onLoginSuccess() {
    setState(() {
      _isAuthenticated = true;
    });
  }

  @override
  Widget build(BuildContext context) {
    if (_isCheckingAuth) {
      return const Scaffold(
        body: Center(
          child: CircularProgressIndicator(),
        ),
      );
    }

    if (_isAuthenticated) {
      return widget.authenticatedChild;
    } else {
      return widget.unauthenticatedChild ??
          PinepodsStartupLogin(onLoginSuccess: _onLoginSuccess);
    }
  }
}

// Simple authentication status provider
class AuthStatus extends InheritedWidget {
  final bool isAuthenticated;
  final VoidCallback? onAuthChanged;

  const AuthStatus({
    Key? key,
    required this.isAuthenticated,
    this.onAuthChanged,
    required Widget child,
  }) : super(key: key, child: child);

  static AuthStatus? of(BuildContext context) {
    return context.dependOnInheritedWidgetOfExactType<AuthStatus>();
  }

  @override
  bool updateShouldNotify(AuthStatus oldWidget) {
    return isAuthenticated != oldWidget.isAuthenticated;
  }
}