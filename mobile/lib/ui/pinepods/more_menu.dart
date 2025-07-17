// lib/ui/pinepods/more_menu.dart
import 'package:flutter/material.dart';
import 'package:pinepods_mobile/ui/library/downloads.dart';
import 'package:pinepods_mobile/ui/settings/settings.dart';
import 'package:pinepods_mobile/ui/pinepods/saved.dart';
import 'package:pinepods_mobile/ui/pinepods/history.dart';

class PinepodsMoreMenu extends StatelessWidget {
  // Constructor with optional key parameter
  const PinepodsMoreMenu({Key? key}) : super(key: key);

  @override
  Widget build(BuildContext context) {
    return SliverList(
      delegate: SliverChildListDelegate([
        Padding(
          padding: const EdgeInsets.all(16.0),
          child: Column(
            crossAxisAlignment: CrossAxisAlignment.start,
            children: [
              const Text(
                'More Options',
                style: TextStyle(
                  fontSize: 24,
                  fontWeight: FontWeight.bold,
                ),
              ),
              const SizedBox(height: 16),
              _buildMenuItem(
                context,
                'Downloads',
                Icons.download_outlined,
                    () => Navigator.push(
                  context,
                  MaterialPageRoute<void>(
                    fullscreenDialog: false,
                    builder: (context) => Scaffold(
                      appBar: AppBar(title: const Text('Downloads')),
                      body: const CustomScrollView(
                        slivers: [Downloads()],
                      ),
                    ),
                  ),
                ),
              ),
              _buildMenuItem(
                context,
                'Saved Episodes',
                Icons.bookmark_outline,
                    () => Navigator.push(
                  context,
                  MaterialPageRoute<void>(
                    fullscreenDialog: false,
                    builder: (context) => Scaffold(
                      appBar: AppBar(title: const Text('Saved Episodes')),
                      body: const CustomScrollView(
                        slivers: [PinepodsSaved()],
                      ),
                    ),
                  ),
                ),
              ),
              _buildMenuItem(
                context,
                'History',
                Icons.history,
                    () => Navigator.push(
                  context,
                  MaterialPageRoute<void>(
                    fullscreenDialog: false,
                    builder: (context) => Scaffold(
                      appBar: AppBar(title: const Text('History')),
                      body: const CustomScrollView(
                        slivers: [PinepodsHistory()],
                      ),
                    ),
                  ),
                ),
              ),
              _buildMenuItem(
                context,
                'Settings',
                Icons.settings_outlined,
                    () => Navigator.push(
                  context,
                  MaterialPageRoute<void>(
                    fullscreenDialog: true,
                    settings: const RouteSettings(name: 'settings'),
                    builder: (context) => const Settings(),
                  ),
                ),
              ),
            ],
          ),
        ),
      ]),
    );
  }

  Widget _buildMenuItem(
      BuildContext context,
      String title,
      IconData icon,
      VoidCallback onTap,
      ) {
    return Card(
      margin: const EdgeInsets.only(bottom: 12.0),
      child: ListTile(
        leading: Icon(icon),
        title: Text(title),
        trailing: const Icon(Icons.arrow_forward_ios, size: 16),
        onTap: onTap,
      ),
    );
  }
}