// Copyright 2020 Ben Hills and the project contributors. All rights reserved.
// Use of this source code is governed by a BSD-style license that can be
// found in the LICENSE file.

import 'package:pinepods_mobile/bloc/podcast/podcast_bloc.dart';
import 'package:pinepods_mobile/bloc/settings/settings_bloc.dart';
import 'package:pinepods_mobile/core/utils.dart';
import 'package:pinepods_mobile/entities/app_settings.dart';
import 'package:pinepods_mobile/l10n/L.dart';
import 'package:pinepods_mobile/ui/settings/episode_refresh.dart';
import 'package:pinepods_mobile/ui/settings/search_provider.dart';
import 'package:pinepods_mobile/ui/settings/settings_section_label.dart';
import 'package:pinepods_mobile/ui/settings/bottom_bar_order.dart';
import 'package:pinepods_mobile/ui/widgets/action_text.dart';
import 'package:pinepods_mobile/ui/settings/pinepods_login.dart';
import 'package:pinepods_mobile/ui/debug/debug_logs_page.dart';
import 'package:pinepods_mobile/ui/themes.dart';
import 'package:file_picker/file_picker.dart';
import 'package:flutter/cupertino.dart';
import 'package:flutter/foundation.dart';
import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:flutter_dialogs/flutter_dialogs.dart';
import 'package:provider/provider.dart';

/// This is the settings page and allows the user to select various
/// options for the app.
///
/// This is a self contained page and so, unlike the other forms, talks directly
/// to a settings service rather than a BLoC. Whilst this deviates slightly from
/// the overall architecture, adding a BLoC to simply be consistent with the rest
/// of the application would add unnecessary complexity.
///
/// This page is built with both Android & iOS in mind. However, the
/// rest of the application is not prepared for iOS design; this
/// is in preparation for the iOS version.
class Settings extends StatefulWidget {
  const Settings({
    super.key,
  });

  @override
  State<Settings> createState() => _SettingsState();
}

class _SettingsState extends State<Settings> {
  bool sdcard = false;

  Widget _buildList(BuildContext context) {
    var settingsBloc = Provider.of<SettingsBloc>(context);
    var podcastBloc = Provider.of<PodcastBloc>(context);

    return StreamBuilder<AppSettings>(
        stream: settingsBloc.settings,
        initialData: settingsBloc.currentSettings,
        builder: (context, snapshot) {
          return ListView(
            children: [
              SettingsDividerLabel(label: L.of(context)!.settings_personalisation_divider_label),
              const Divider(),
              MergeSemantics(
                child: Padding(
                  padding: const EdgeInsets.symmetric(horizontal: 16.0, vertical: 8.0),
                  child: Column(
                    crossAxisAlignment: CrossAxisAlignment.start,
                    children: [
                      Text(
                        L.of(context)!.settings_theme_switch_label,
                        style: Theme.of(context).textTheme.titleMedium,
                      ),
                      const SizedBox(height: 8),
                      Text(
                        ThemeRegistry.getTheme(snapshot.data!.theme).description,
                        style: Theme.of(context).textTheme.bodyMedium?.copyWith(
                          color: Theme.of(context).colorScheme.onSurface.withOpacity(0.7),
                        ),
                      ),
                      const SizedBox(height: 12),
                      Row(
                        children: [
                          const Icon(Icons.palette, size: 20),
                          const SizedBox(width: 12),
                          Expanded(
                            child: DropdownButton<String>(
                              value: snapshot.data!.theme,
                              isExpanded: true,
                              underline: Container(),
                              items: ThemeRegistry.themeList.map((theme) {
                                return DropdownMenuItem<String>(
                                  value: theme.key,
                                  child: Row(
                                    children: [
                                      Container(
                                        width: 16,
                                        height: 16,
                                        decoration: BoxDecoration(
                                          color: theme.isDark ? Colors.grey[800] : Colors.grey[200],
                                          border: Border.all(
                                            color: theme.themeData.colorScheme.primary,
                                            width: 2,
                                          ),
                                          borderRadius: BorderRadius.circular(8),
                                        ),
                                      ),
                                      const SizedBox(width: 8),
                                      Expanded(
                                        child: Text(
                                          theme.name,
                                          overflow: TextOverflow.ellipsis,
                                        ),
                                      ),
                                    ],
                                  ),
                                );
                              }).toList(),
                              onChanged: (String? newTheme) {
                                if (newTheme != null) {
                                  settingsBloc.setTheme(newTheme);
                                }
                              },
                            ),
                          ),
                        ],
                      ),
                    ],
                  ),
                ),
              ),
              sdcard
                  ? MergeSemantics(
                      child: ListTile(
                        title: Text(L.of(context)!.settings_download_sd_card_label),
                        trailing: Switch.adaptive(
                          value: snapshot.data!.storeDownloadsSDCard,
                          onChanged: (value) => sdcard
                              ? setState(() {
                                  if (value) {
                                    _showStorageDialog(enableExternalStorage: true);
                                  } else {
                                    _showStorageDialog(enableExternalStorage: false);
                                  }

                                  settingsBloc.storeDownloadonSDCard(value);
                                })
                              : null,
                        ),
                      ),
                    )
                  : const SizedBox(
                      height: 0,
                      width: 0,
                    ),
              SettingsDividerLabel(label: 'Navigation'),
              const Divider(),
              ListTile(
                title: const Text('Reorganize Bottom Bar'),
                subtitle: const Text('Customize the order of bottom navigation items'),
                leading: const Icon(Icons.reorder),
                onTap: () {
                  Navigator.push(
                    context,
                    MaterialPageRoute(
                      builder: (context) => const BottomBarOrderWidget(),
                    ),
                  );
                },
              ),
              SettingsDividerLabel(label: L.of(context)!.settings_playback_divider_label),
              const Divider(),
              MergeSemantics(
                child: ListTile(
                  title: Text(L.of(context)!.settings_auto_open_now_playing),
                  trailing: Switch.adaptive(
                    value: snapshot.data!.autoOpenNowPlaying,
                    onChanged: (value) => setState(() => settingsBloc.setAutoOpenNowPlaying(value)),
                  ),
                ),
              ),
              const SearchProviderWidget(),
              SettingsDividerLabel(label: 'Debug'),
              const Divider(),
              ListTile(
                title: const Text('App Logs'),
                subtitle: const Text('View debug logs and device information'),
                leading: const Icon(Icons.bug_report),
                onTap: () {
                  Navigator.push(
                    context,
                    MaterialPageRoute(
                      builder: (context) => const DebugLogsPage(),
                    ),
                  );
                },
              ),
              const PinepodsLoginWidget(),
              const _WebAppInfoWidget(),
            ],
          );
        });
  }

  Widget _buildAndroid(BuildContext context) {
    return AnnotatedRegion<SystemUiOverlayStyle>(
      value: Theme.of(context).appBarTheme.systemOverlayStyle!,
      child: Scaffold(
        appBar: AppBar(
          elevation: 0.0,
          title: Text(
            L.of(context)!.settings_label,
          ),
        ),
        body: _buildList(context),
      ),
    );
  }

  Widget _buildIos(BuildContext context) {
    return CupertinoPageScaffold(
      navigationBar: CupertinoNavigationBar(
        padding: const EdgeInsetsDirectional.all(0.0),
        leading: CupertinoButton(
            child: const Icon(Icons.arrow_back_ios),
            onPressed: () {
              Navigator.pop(context);
            }),
        middle: Text(
          L.of(context)!.settings_label,
          style: TextStyle(color: Theme.of(context).colorScheme.primary),
        ),
        backgroundColor: Theme.of(context).colorScheme.surface,
      ),
      child: Material(child: _buildList(context)),
    );
  }

  void _showStorageDialog({required bool enableExternalStorage}) {
    showPlatformDialog<void>(
      context: context,
      useRootNavigator: false,
      builder: (_) => BasicDialogAlert(
        title: Text(L.of(context)!.settings_download_switch_label),
        content: Text(
          enableExternalStorage
              ? L.of(context)!.settings_download_switch_card
              : L.of(context)!.settings_download_switch_internal,
        ),
        actions: <Widget>[
          BasicDialogAction(
            title: Text(
              L.of(context)!.ok_button_label,
            ),
            onPressed: () {
              Navigator.pop(context);
            },
          ),
        ],
      ),
    );
  }

  @override
  Widget build(context) {
    switch (defaultTargetPlatform) {
      case TargetPlatform.android:
        return _buildAndroid(context);
      case TargetPlatform.iOS:
        return _buildIos(context);
      default:
        assert(false, 'Unexpected platform $defaultTargetPlatform');
        return _buildAndroid(context);
    }
  }

  @override
  void initState() {
    super.initState();

    hasExternalStorage().then((value) {
      setState(() {
        sdcard = value;
      });
    });
  }
}

class _WebAppInfoWidget extends StatelessWidget {
  const _WebAppInfoWidget();

  @override
  Widget build(BuildContext context) {
    return Consumer<SettingsBloc>(
      builder: (context, settingsBloc, child) {
        final settings = settingsBloc.currentSettings;
        final serverUrl = settings.pinepodsServer;

        // Only show if user is connected to a server
        if (serverUrl == null || serverUrl.isEmpty) {
          return const SizedBox.shrink();
        }

        return Padding(
          padding: const EdgeInsets.all(16.0),
          child: Card(
            child: Padding(
              padding: const EdgeInsets.all(16.0),
              child: Column(
                crossAxisAlignment: CrossAxisAlignment.start,
                children: [
                  Row(
                    children: [
                      Icon(
                        Icons.web,
                        color: Theme.of(context).primaryColor,
                        size: 20,
                      ),
                      const SizedBox(width: 8),
                      Text(
                        'Web App Settings',
                        style: TextStyle(
                          fontWeight: FontWeight.bold,
                          color: Theme.of(context).primaryColor,
                        ),
                      ),
                    ],
                  ),
                  const SizedBox(height: 8),
                  Text(
                    'Many more server side and user settings available from the PinePods web app. Please head to $serverUrl to adjust much more',
                    style: Theme.of(context).textTheme.bodySmall,
                  ),
                ],
              ),
            ),
          ),
        );
      },
    );
  }
}
