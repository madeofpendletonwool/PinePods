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
import 'package:pinepods_mobile/ui/icons/phosphor_icons.dart';
import 'package:pinepods_mobile/ui/widgets/soft_ui.dart';

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

  Widget _buildStatCard(String label, String value, {IconData? icon, Color? iconColor, List<String> details = const []}) {
    final theme = Theme.of(context);
    return Container(
      padding: const EdgeInsets.all(14),
      decoration: BoxDecoration(
        color: softFill(theme),
        borderRadius: BorderRadius.circular(16),
      ),
      child: Column(
        mainAxisAlignment: MainAxisAlignment.center,
        children: [
          if (icon != null) ...[
            Icon(icon, size: 24, color: iconColor ?? theme.primaryColor),
            const SizedBox(height: 8),
          ],
          Text(
            value,
            style: theme.textTheme.titleLarge?.copyWith(fontWeight: FontWeight.bold),
            textAlign: TextAlign.center,
            maxLines: 1,
            overflow: TextOverflow.ellipsis,
          ),
          const SizedBox(height: 2),
          Text(
            label,
            style: theme.textTheme.bodySmall?.copyWith(color: theme.hintColor),
            textAlign: TextAlign.center,
          ),
          for (final detail in details) ...[
            const SizedBox(height: 2),
            Text(
              detail,
              style: TextStyle(fontSize: 11, color: theme.hintColor),
              textAlign: TextAlign.center,
              maxLines: 1,
              overflow: TextOverflow.ellipsis,
            ),
          ],
        ],
      ),
    );
  }

  /// Build sync status card that fits in the grid with consistent styling
  Widget _buildSyncStatCard() {
    if (_userStats == null) return const SizedBox.shrink();

    final stats = _userStats!;
    final isNotSyncing = !stats.isSyncing;

    return _buildStatCard(
      'Sync Status',
      stats.syncStatusDescription,
      icon: isNotSyncing ? PhosphorIconsRegular.prohibit : PhosphorIconsRegular.arrowsClockwise,
      iconColor: isNotSyncing ? Colors.grey : null,
      details: stats.syncUrls(_pinepodsService.server),
    );
  }

  Widget _buildInfoCard() {
    final theme = Theme.of(context);
    final muted = theme.hintColor;

    Widget link(IconData icon, String label, VoidCallback onTap) => ListTile(
          leading: Icon(icon, color: theme.primaryColor),
          title: Text(label),
          trailing: Icon(PhosphorIconsRegular.caretRight, size: 18, color: muted),
          onTap: onTap,
        );

    return Column(
      children: [
        const SizedBox(height: 8),
        Image.asset('assets/images/pinepods-logo.png', width: 72, height: 72),
        const SizedBox(height: 12),
        Text(
          'App v${Environment.projectVersion}  ·  Server ${_pinepodsVersion ?? "unknown"}',
          style: theme.textTheme.titleSmall?.copyWith(fontWeight: FontWeight.w600),
          textAlign: TextAlign.center,
        ),
        const SizedBox(height: 10),
        Text(
          'Thanks for using PinePods! This app was born from a love for podcasts, of homelabs, and a desire to have a secure and central location to manage personal data.',
          style: theme.textTheme.bodyMedium?.copyWith(color: muted, height: 1.4),
          textAlign: TextAlign.center,
        ),
        const SizedBox(height: 16),
        Material(
          color: softFill(theme),
          borderRadius: BorderRadius.circular(16),
          clipBehavior: Clip.antiAlias,
          child: Column(
            children: [
              link(PhosphorIconsRegular.fileText, 'PinePods documentation', () => _launchUrl('https://pinepods.online')),
              link(PhosphorIconsRegular.code, 'PinePods on GitHub',
                  () => _launchUrl('https://github.com/madeofpendletonwool/pinepods')),
              link(PhosphorIconsRegular.coffee, 'Buy me a coffee',
                  () => _launchUrl('https://www.buymeacoffee.com/collinscoffee')),
              link(PhosphorIconsRegular.article, 'Open source licenses', () => showLicensePage(context: context)),
            ],
          ),
        ),
        const SizedBox(height: 16),
        Text(
          'Copyright © 2025 Gooseberry Development',
          style: TextStyle(fontSize: 12, color: muted, fontWeight: FontWeight.w500),
          textAlign: TextAlign.center,
        ),
        const SizedBox(height: 6),
        Text(
          'The PinePods Mobile App is an open-source podcast player adapted from the Anytime Podcast Player (© 2020 Ben Hills). Portions of this application retain the original BSD 3-Clause license.',
          style: TextStyle(fontSize: 12, color: muted, height: 1.3),
          textAlign: TextAlign.center,
        ),
        TextButton(
          onPressed: () => _launchUrl('https://github.com/amugofjava/anytime_podcast_player'),
          child: const Text('View original project on GitHub', style: TextStyle(fontSize: 12)),
        ),
      ],
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
                  child: SoftEmptyState(
                    icon: PhosphorIconsRegular.warningCircle,
                    title: 'Couldn\'t load stats',
                    body: _errorMessage!,
                    action: FilledButton.tonal(
                      onPressed: () {
                        setState(() {
                          _isLoading = true;
                          _errorMessage = null;
                        });
                        _loadUserStats();
                      },
                      child: const Text('Retry'),
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
                            icon: PhosphorIconsRegular.calendarBlank,
                          ),
                          _buildStatCard(
                            'Podcasts Played',
                            _userStats?.podcastsPlayed.toString() ?? '',
                            icon: PhosphorIconsFill.playCircle,
                          ),
                          _buildStatCard(
                            'Time Listened',
                            _userStats?.formattedTimeListened ?? '',
                            icon: PhosphorIconsRegular.clock,
                          ),
                          _buildStatCard(
                            'Podcasts Added',
                            _userStats?.podcastsAdded.toString() ?? '',
                            icon: PhosphorIconsRegular.plusSquare,
                          ),
                          _buildStatCard(
                            'Episodes Saved',
                            _userStats?.episodesSaved.toString() ?? '',
                            icon: PhosphorIconsFill.bookmarkSimple,
                          ),
                          _buildStatCard(
                            'Episodes Downloaded',
                            _userStats?.episodesDownloaded.toString() ?? '',
                            icon: PhosphorIconsRegular.downloadSimple,
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
