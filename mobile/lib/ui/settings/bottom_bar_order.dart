// Copyright 2020 Ben Hills and the project contributors. All rights reserved.
// Use of this source code is governed by a BSD-style license that can be
// found in the LICENSE file.

import 'package:flutter/material.dart';
import 'package:provider/provider.dart';
import 'package:pinepods_mobile/bloc/settings/settings_bloc.dart';
import 'package:pinepods_mobile/entities/app_settings.dart';
import 'package:pinepods_mobile/l10n/L.dart';
import 'package:pinepods_mobile/ui/icons/phosphor_icons.dart';
import 'package:pinepods_mobile/ui/widgets/pinepods_bottom_nav.dart';

/// A widget that allows users to reorder the bottom navigation bar items
class BottomBarOrderWidget extends StatefulWidget {
  const BottomBarOrderWidget({super.key});

  @override
  State<BottomBarOrderWidget> createState() => _BottomBarOrderWidgetState();
}

class _BottomBarOrderWidgetState extends State<BottomBarOrderWidget> {
  late List<String> _currentOrder;

  @override
  void initState() {
    super.initState();
    final settingsBloc = Provider.of<SettingsBloc>(context, listen: false);
    _currentOrder = List.from(settingsBloc.currentSettings.bottomBarOrder);
  }

  @override
  Widget build(BuildContext context) {
    return Scaffold(
      appBar: AppBar(
        title: const Text('Reorganize Bottom Bar'),
        actions: [
          TextButton(
            onPressed: () async {
              final settingsBloc = Provider.of<SettingsBloc>(context, listen: false);
              settingsBloc.setBottomBarOrder(_currentOrder);
              
              // Show a brief confirmation
              ScaffoldMessenger.of(context).showSnackBar(
                const SnackBar(
                  content: Text('Bottom bar order saved!'),
                  duration: Duration(seconds: 1),
                ),
              );
              
              // Small delay to let the user see the changes take effect
              await Future.delayed(const Duration(milliseconds: 500));
              
              if (mounted) {
                Navigator.pop(context);
              }
            },
            child: Text(
              'Save',
              style: TextStyle(color: Theme.of(context).colorScheme.primary),
            ),
          ),
        ],
      ),
      body: Column(
        children: [
          Padding(
            padding: const EdgeInsets.all(16.0),
            child: Text(
              'Drag to reorder. The first $bottomBarPinnedCount appear in the bottom bar; the rest are under More (everything shows when the screen is wide enough).',
              style: Theme.of(context).textTheme.bodyMedium?.copyWith(
                color: Theme.of(context).textTheme.bodySmall?.color,
              ),
            ),
          ),
          Expanded(
            child: ReorderableListView(
              onReorder: (oldIndex, newIndex) {
                setState(() {
                  if (newIndex > oldIndex) {
                    newIndex -= 1;
                  }
                  final item = _currentOrder.removeAt(oldIndex);
                  _currentOrder.insert(newIndex, item);
                });
              },
              children: [
                for (var i = 0; i < _currentOrder.length; i++)
                  ListTile(
                    key: Key(_currentOrder[i]),
                    leading: Icon(bottomNavIcon(_currentOrder[i])),
                    title: Text(_currentOrder[i]),
                    subtitle: Text(
                      i < bottomBarPinnedCount ? 'In bottom bar' : 'Under More',
                    ),
                    trailing: const Icon(PhosphorIconsRegular.dotsSix),
                  ),
              ],
            ),
          ),
          Padding(
            padding: const EdgeInsets.all(16.0),
            child: Row(
              children: [
                Expanded(
                  child: ElevatedButton(
                    onPressed: () {
                      setState(() {
                        _currentOrder = List.from(defaultBottomBarOrder);
                      });
                    },
                    child: const Text('Reset to Default'),
                  ),
                ),
              ],
            ),
          ),
        ],
      ),
    );
  }
}
