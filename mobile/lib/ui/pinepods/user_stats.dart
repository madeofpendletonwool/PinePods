// lib/ui/pinepods/user_stats.dart

import 'package:flutter/material.dart';
import 'package:pinepods_mobile/bloc/settings/settings_bloc.dart';
import 'package:pinepods_mobile/entities/user_stats.dart';
import 'package:pinepods_mobile/services/pinepods/pinepods_service.dart';
import 'package:pinepods_mobile/services/logging/app_logger.dart';
import 'package:pinepods_mobile/ui/widgets/platform_progress_indicator.dart';
import 'package:pinepods_mobile/core/environment.dart';
import 'package:provider/provider.dart';
import 'package:url_launcher/url_launcher.dart';

class PinepodsUserStats extends StatefulWidget {
  const PinepodsUserStats({super.key});

  @override
  State<PinepodsUserStats> createState() => _PinepodsUserStatsState();
}

class _PinepodsUserStatsState extends State<PinepodsUserStats> {
  final PinepodsService _pinepodsService = PinepodsService();
  UserStats? _userStats;
  String? _pinepodsVersion;
  bool _isLoading = true;
  String? _errorMessage;

  @override
  void initState() {
    super.initState();
    _initializeCredentials();
    _loadUserStats();
  }

  void _initializeCredentials() {
    final settingsBloc = Provider.of<SettingsBloc>(context, listen: false);
    final settings = settingsBloc.currentSettings;

    if (settings.pinepodsServer != null && settings.pinepodsApiKey != null) {
      _pinepodsService.setCredentials(
        settings.pinepodsServer!,
        settings.pinepodsApiKey!,
      );
    }
  }

  /// Calculate responsive cross axis count for stats grid
  int _getStatsCrossAxisCount(BuildContext context) {
    final screenWidth = MediaQuery.of(context).size.width;
    if (screenWidth > 1200) return 4;      // Very wide screens (large tablets, desktop)
    if (screenWidth > 800) return 3;       // Wide tablets like iPad
    if (screenWidth > 500) return 2;       // Standard phones and small tablets
    return 1;                              // Very small phones (< 500px)
  }

  /// Calculate responsive aspect ratio for stats cards
  double _getStatsAspectRatio(BuildContext context) {
    final screenWidth = MediaQuery.of(context).size.width;
    if (screenWidth <= 500) {
      // Single column on small screens - generous height for content + proper padding
      return 2.2; // Allows space for icon + title + value + padding, handles text wrapping
    }
    return 1.0; // Square aspect ratio for multi-column layouts
  }

  Future<void> _loadUserStats() async {
    final settingsBloc = Provider.of<SettingsBloc>(context, listen: false);
    final settings = settingsBloc.currentSettings;
    final userId = settings.pinepodsUserId;

    if (userId == null) {
      setState(() {
        _errorMessage = 'Not logged in';
        _isLoading = false;
      });
      return;
    }

    try {
      final futures = await Future.wait([
        _pinepodsService.getUserStats(userId),
        _pinepodsService.getPinepodsVersion(),
      ]);

      setState(() {
        _userStats = futures[0] as UserStats;
        _pinepodsVersion = futures[1] as String;
        _isLoading = false;
      });
    } catch (e) {
      setState(() {
        _errorMessage = 'Failed to load stats: $e';
        _isLoading = false;
      });
    }
  }

  Future<void> _launchUrl(String url) async {
    final logger = AppLogger();
    logger.info('UserStats', 'Attempting to launch URL: $url');
    
    try {
      final uri = Uri.parse(url);
      
      // Try to launch directly first (works better on Android)
      final launched = await launchUrl(
        uri, 
        mode: LaunchMode.externalApplication,
      );
      
      if (!launched) {
        logger.warning('UserStats', 'Direct URL launch failed, checking if URL can be launched');
        // If direct launch fails, check if URL can be launched
        final canLaunch = await canLaunchUrl(uri);
        if (!canLaunch) {
          throw Exception('No app available to handle this URL');
        }
      } else {
        logger.info('UserStats', 'Successfully launched URL: $url');
      }
    } catch (e) {
      logger.error('UserStats', 'Failed to launch URL: $url', e.toString());
      // Show error if URL can't be launched
      if (mounted) {
        ScaffoldMessenger.of(context).showSnackBar(
          SnackBar(
            content: Text('Could not open link: $url'),
            backgroundColor: Colors.red,
          ),
        );
      }
    }
  }

  Widget _buildStatCard(String label, String value, {IconData? icon, Color? iconColor}) {
    return Card(
      elevation: 2,
      shape: RoundedRectangleBorder(borderRadius: BorderRadius.circular(12)),
      child: Padding(
        padding: const EdgeInsets.all(16.0),
        child: Column(
          mainAxisAlignment: MainAxisAlignment.center,
          children: [
            if (icon != null) ...[
              Icon(
                icon,
                size: 32,
                color: iconColor ?? Theme.of(context).primaryColor,
              ),
              const SizedBox(height: 8),
            ],
            Text(
              label,
              style: TextStyle(
                fontSize: 14,
                fontWeight: FontWeight.w500,
                color: Colors.grey[600],
              ),
              textAlign: TextAlign.center,
            ),
            const SizedBox(height: 4),
            Text(
              value,
              style: const TextStyle(
                fontSize: 20,
                fontWeight: FontWeight.bold,
              ),
              textAlign: TextAlign.center,
            ),
          ],
        ),
      ),
    );
  }

  /// Build sync status card that fits in the grid with consistent styling
  Widget _buildSyncStatCard() {
    if (_userStats == null) return const SizedBox.shrink();

    final stats = _userStats!;
    final isNotSyncing = stats.podSyncType.toLowerCase() == 'none';

    return _buildStatCard(
      'Sync Status',
      stats.syncStatusDescription,
      icon: isNotSyncing ? Icons.sync_disabled : Icons.sync,
      iconColor: isNotSyncing ? Colors.grey : null,
    );
  }

  Widget _buildSyncStatusCard() {
    if (_userStats == null) return const SizedBox.shrink();

    final stats = _userStats!;
    final isNotSyncing = stats.podSyncType.toLowerCase() == 'none';

    return Card(
      elevation: 2,
      shape: RoundedRectangleBorder(borderRadius: BorderRadius.circular(12)),
      child: Padding(
        padding: const EdgeInsets.all(16.0),
        child: Column(
          children: [
            Icon(
              isNotSyncing ? Icons.sync_disabled : Icons.sync,
              size: 32,
              color: isNotSyncing ? Colors.grey : Theme.of(context).primaryColor,
            ),
            const SizedBox(height: 8),
            Text(
              'Podcast Sync Status',
              style: TextStyle(
                fontSize: 14,
                fontWeight: FontWeight.w500,
                color: Colors.grey[600],
              ),
              textAlign: TextAlign.center,
            ),
            const SizedBox(height: 4),
            Text(
              stats.syncStatusDescription,
              style: const TextStyle(
                fontSize: 18,
                fontWeight: FontWeight.bold,
              ),
              textAlign: TextAlign.center,
            ),
            if (!isNotSyncing && stats.gpodderUrl.isNotEmpty) ...[
              const SizedBox(height: 4),
              Text(
                stats.gpodderUrl,
                style: TextStyle(
                  fontSize: 12,
                  color: Colors.grey[600],
                ),
                textAlign: TextAlign.center,
              ),
            ],
          ],
        ),
      ),
    );
  }

  Widget _buildInfoCard() {
    return Card(
      elevation: 2,
      shape: RoundedRectangleBorder(borderRadius: BorderRadius.circular(12)),
      child: Padding(
        padding: const EdgeInsets.all(20.0),
        child: Column(
          children: [
            // PinePods Logo
            Container(
              width: 80,
              height: 80,
              decoration: BoxDecoration(
                borderRadius: BorderRadius.circular(16),
                image: const DecorationImage(
                  image: AssetImage('assets/images/pinepods-logo.png'),
                  fit: BoxFit.contain,
                ),
              ),
            ),
            const SizedBox(height: 16),

            Text(
              'App Version: v${Environment.projectVersion}',
              style: const TextStyle(
                fontSize: 16,
                fontWeight: FontWeight.w600,
              ),
              textAlign: TextAlign.center,
            ),
            const SizedBox(height: 8),
            Text(
              'Server Version: ${_pinepodsVersion ?? "Unknown"}',
              style: const TextStyle(
                fontSize: 16,
                fontWeight: FontWeight.w600,
              ),
              textAlign: TextAlign.center,
            ),
            const SizedBox(height: 12),

            Text(
              'Thanks for using PinePods! This app was born from a love for podcasts, of homelabs, and a desire to have a secure and central location to manage personal data.',
              style: TextStyle(
                fontSize: 14,
                color: Colors.grey[700],
                height: 1.4,
              ),
              textAlign: TextAlign.center,
            ),
            const SizedBox(height: 12),
            
            Text(
              'Copyright © 2025 Gooseberry Development',
              style: TextStyle(
                fontSize: 12,
                color: Colors.grey[600],
                fontWeight: FontWeight.w500,
              ),
              textAlign: TextAlign.center,
            ),
            const SizedBox(height: 8),
            
            Text(
              'The PinePods Mobile App is an open-source podcast player adapted from the Anytime Podcast Player (© 2020 Ben Hills). Portions of this application retain the original BSD 3-Clause license.',
              style: TextStyle(
                fontSize: 12,
                color: Colors.grey[600],
                height: 1.3,
              ),
              textAlign: TextAlign.center,
            ),
            const SizedBox(height: 8),
            
            GestureDetector(
              onTap: () => _launchUrl('https://github.com/amugofjava/anytime_podcast_player'),
              child: Text(
                'View original project on GitHub',
                style: TextStyle(
                  fontSize: 12,
                  decoration: TextDecoration.underline,
                  color: Theme.of(context).primaryColor,
                ),
                textAlign: TextAlign.center,
              ),
            ),
            const SizedBox(height: 20),

            // Buttons
            Column(
              children: [
                SizedBox(
                  width: double.infinity,
                  child: ElevatedButton.icon(
                    onPressed: () => _launchUrl('https://pinepods.online'),
                    icon: const Icon(Icons.description),
                    label: const Text('PinePods Documentation'),
                    style: ElevatedButton.styleFrom(
                      padding: const EdgeInsets.symmetric(vertical: 12),
                      shape: RoundedRectangleBorder(
                        borderRadius: BorderRadius.circular(8),
                      ),
                    ),
                  ),
                ),
                const SizedBox(height: 8),
                SizedBox(
                  width: double.infinity,
                  child: ElevatedButton.icon(
                    onPressed: () => _launchUrl('https://github.com/madeofpendletonwool/pinepods'),
                    icon: const Icon(Icons.code),
                    label: const Text('PinePods GitHub Repo'),
                    style: ElevatedButton.styleFrom(
                      padding: const EdgeInsets.symmetric(vertical: 12),
                      shape: RoundedRectangleBorder(
                        borderRadius: BorderRadius.circular(8),
                      ),
                    ),
                  ),
                ),
                const SizedBox(height: 8),
                SizedBox(
                  width: double.infinity,
                  child: ElevatedButton.icon(
                    onPressed: () => _launchUrl('https://www.buymeacoffee.com/collinscoffee'),
                    icon: const Icon(Icons.coffee),
                    label: const Text('Buy me a Coffee'),
                    style: ElevatedButton.styleFrom(
                      padding: const EdgeInsets.symmetric(vertical: 12),
                      shape: RoundedRectangleBorder(
                        borderRadius: BorderRadius.circular(8),
                      ),
                    ),
                  ),
                ),
                const SizedBox(height: 8),
                SizedBox(
                  width: double.infinity,
                  child: ElevatedButton.icon(
                    onPressed: () {
                      showLicensePage(context: context);
                    },
                    icon: const Icon(Icons.article_outlined),
                    label: const Text('Open Source Licenses'),
                    style: ElevatedButton.styleFrom(
                      padding: const EdgeInsets.symmetric(vertical: 12),
                      shape: RoundedRectangleBorder(
                        borderRadius: BorderRadius.circular(8),
                      ),
                    ),
                  ),
                ),
              ],
            ),
          ],
        ),
      ),
    );
  }

  @override
  Widget build(BuildContext context) {
    return Scaffold(
      appBar: AppBar(
        title: const Text('User Statistics'),
        centerTitle: true,
      ),
      body: _isLoading
          ? const Center(child: PlatformProgressIndicator())
          : _errorMessage != null
              ? Center(
                  child: Padding(
                    padding: const EdgeInsets.all(16.0),
                    child: Column(
                      mainAxisAlignment: MainAxisAlignment.center,
                      children: [
                        Icon(
                          Icons.error_outline,
                          size: 64,
                          color: Colors.red[300],
                        ),
                        const SizedBox(height: 16),
                        Text(
                          _errorMessage!,
                          textAlign: TextAlign.center,
                          style: Theme.of(context).textTheme.bodyLarge,
                        ),
                        const SizedBox(height: 16),
                        ElevatedButton(
                          onPressed: () {
                            setState(() {
                              _isLoading = true;
                              _errorMessage = null;
                            });
                            _loadUserStats();
                          },
                          child: const Text('Retry'),
                        ),
                      ],
                    ),
                  ),
                )
              : SingleChildScrollView(
                  padding: const EdgeInsets.all(16.0),
                  child: Column(
                    children: [
                      // Statistics Grid
                      GridView.count(
                        crossAxisCount: _getStatsCrossAxisCount(context),
                        shrinkWrap: true,
                        physics: const NeverScrollableScrollPhysics(),
                        childAspectRatio: _getStatsAspectRatio(context),
                        crossAxisSpacing: 12,
                        mainAxisSpacing: 12,
                        children: [
                          _buildStatCard(
                            'User Created',
                            _userStats?.formattedUserCreated ?? '',
                            icon: Icons.calendar_today,
                          ),
                          _buildStatCard(
                            'Podcasts Played',
                            _userStats?.podcastsPlayed.toString() ?? '',
                            icon: Icons.play_circle,
                          ),
                          _buildStatCard(
                            'Time Listened',
                            _userStats?.formattedTimeListened ?? '',
                            icon: Icons.access_time,
                          ),
                          _buildStatCard(
                            'Podcasts Added',
                            _userStats?.podcastsAdded.toString() ?? '',
                            icon: Icons.library_add,
                          ),
                          _buildStatCard(
                            'Episodes Saved',
                            _userStats?.episodesSaved.toString() ?? '',
                            icon: Icons.bookmark,
                          ),
                          _buildStatCard(
                            'Episodes Downloaded',
                            _userStats?.episodesDownloaded.toString() ?? '',
                            icon: Icons.download,
                          ),
                          // Add sync status as a stat card to maintain consistent layout
                          _buildSyncStatCard(),
                        ],
                      ),

                      const SizedBox(height: 16),

                      // Info Card
                      _buildInfoCard(),
                    ],
                  ),
                ),
    );
  }
}
