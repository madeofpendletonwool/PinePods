// lib/ui/pinepods/user_stats.dart

import 'package:flutter/material.dart';
import 'package:pinepods_mobile/bloc/settings/settings_bloc.dart';
import 'package:pinepods_mobile/entities/user_stats.dart';
import 'package:pinepods_mobile/services/pinepods/pinepods_service.dart';
import 'package:pinepods_mobile/ui/widgets/platform_progress_indicator.dart';
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
    final uri = Uri.parse(url);
    if (await canLaunchUrl(uri)) {
      await launchUrl(uri, mode: LaunchMode.externalApplication);
    }
  }

  Widget _buildStatCard(String label, String value, {IconData? icon}) {
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
                color: Theme.of(context).primaryColor,
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
              'Current Version: ${_pinepodsVersion ?? "Unknown"}',
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
                        crossAxisCount: 2,
                        shrinkWrap: true,
                        physics: const NeverScrollableScrollPhysics(),
                        childAspectRatio: 1.0,
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
                        ],
                      ),
                      
                      const SizedBox(height: 16),
                      
                      // Sync Status Card
                      _buildSyncStatusCard(),
                      
                      const SizedBox(height: 16),
                      
                      // Info Card
                      _buildInfoCard(),
                    ],
                  ),
                ),
    );
  }
}