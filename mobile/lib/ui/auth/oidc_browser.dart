import 'package:flutter/material.dart';
import 'package:webview_flutter/webview_flutter.dart';
import 'package:pinepods_mobile/services/pinepods/oidc_service.dart';

class OidcBrowser extends StatefulWidget {
  final String authUrl;
  final String serverUrl;
  final Function(String apiKey) onSuccess;
  final Function(String error) onError;

  const OidcBrowser({
    super.key,
    required this.authUrl,
    required this.serverUrl,
    required this.onSuccess,
    required this.onError,
  });

  @override
  State<OidcBrowser> createState() => _OidcBrowserState();
}

class _OidcBrowserState extends State<OidcBrowser> {
  late final WebViewController _controller;
  bool _isLoading = true;
  String _currentUrl = '';
  bool _callbackTriggered = false; // Prevent duplicate callbacks

  @override
  void initState() {
    super.initState();
    _initializeWebView();
  }

  void _initializeWebView() {
    _controller = WebViewController()
      ..setJavaScriptMode(JavaScriptMode.unrestricted)
      ..setNavigationDelegate(
        NavigationDelegate(
          onPageStarted: (String url) {
            setState(() {
              _currentUrl = url;
              _isLoading = true;
            });
            
            _checkForCallback(url);
          },
          onPageFinished: (String url) {
            setState(() {
              _isLoading = false;
            });
            
            _checkForCallback(url);
          },
          onNavigationRequest: (NavigationRequest request) {
            _checkForCallback(request.url);
            return NavigationDecision.navigate;
          },
        ),
      )
      ..loadRequest(Uri.parse(widget.authUrl));
  }

  void _checkForCallback(String url) {
    if (_callbackTriggered) return; // Prevent duplicate callbacks
    
    // Check if we've reached the callback URL with an API key
    final apiKey = OidcService.extractApiKeyFromUrl(url);
    if (apiKey != null) {
      _callbackTriggered = true; // Mark callback as triggered
      widget.onSuccess(apiKey);
      return;
    }
    
    // Check for error in callback URL
    final uri = Uri.tryParse(url);
    if (uri != null && uri.path.contains('/oauth/callback')) {
      final error = uri.queryParameters['error'];
      if (error != null) {
        _callbackTriggered = true; // Mark callback as triggered
        final errorDescription = uri.queryParameters['description'] ?? uri.queryParameters['details'] ?? 'Authentication failed';
        widget.onError('$error: $errorDescription');
        return;
      }
    }
  }

  @override
  Widget build(BuildContext context) {
    return Scaffold(
      appBar: AppBar(
        title: const Text('Sign In'),
        backgroundColor: Theme.of(context).primaryColor,
        foregroundColor: Colors.white,
        leading: IconButton(
          icon: const Icon(Icons.close),
          onPressed: () {
            widget.onError('User cancelled authentication');
          },
        ),
        actions: [
          if (_isLoading)
            const Padding(
              padding: EdgeInsets.all(16.0),
              child: SizedBox(
                width: 20,
                height: 20,
                child: CircularProgressIndicator(
                  strokeWidth: 2,
                  valueColor: AlwaysStoppedAnimation<Color>(Colors.white),
                ),
              ),
            ),
        ],
      ),
      body: Column(
        children: [
          // URL bar for debugging
          if (MediaQuery.of(context).size.height > 600)
            Container(
              padding: const EdgeInsets.all(8.0),
              color: Colors.grey[200],
              child: Row(
                children: [
                  const Icon(Icons.link, size: 16),
                  const SizedBox(width: 8),
                  Expanded(
                    child: Text(
                      _currentUrl,
                      style: const TextStyle(fontSize: 12),
                      overflow: TextOverflow.ellipsis,
                    ),
                  ),
                ],
              ),
            ),
          // WebView
          Expanded(
            child: WebViewWidget(
              controller: _controller,
            ),
          ),
        ],
      ),
    );
  }
}