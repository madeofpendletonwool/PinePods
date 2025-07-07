// Copyright 2020 Ben Hills and the project contributors. All rights reserved.
// Use of this source code is governed by a BSD-style license that can be
// found in the LICENSE file.

import 'package:pinepods_mobile/bloc/podcast/podcast_bloc.dart';
import 'package:pinepods_mobile/bloc/settings/settings_bloc.dart';
import 'package:pinepods_mobile/entities/app_settings.dart';
import 'package:pinepods_mobile/entities/podcast.dart';
import 'package:pinepods_mobile/l10n/L.dart';
import 'package:pinepods_mobile/ui/widgets/platform_progress_indicator.dart';
import 'package:pinepods_mobile/ui/widgets/pinepods_podcast_grid_tile.dart';
import 'package:pinepods_mobile/ui/widgets/pinepods_podcast_tile.dart';
import 'package:pinepods_mobile/services/pinepods/pinepods_service.dart';
import 'package:flutter/material.dart';
import 'package:provider/provider.dart';

/// This class displays the list of podcasts the user is subscribed to on the PinePods server.
class PinepodsPodcasts extends StatefulWidget {
  const PinepodsPodcasts({
    super.key,
  });

  @override
  State<PinepodsPodcasts> createState() => _PinepodsPodcastsState();
}

class _PinepodsPodcastsState extends State<PinepodsPodcasts> {
  List<Podcast>? _podcasts;
  bool _isLoading = true;
  String? _errorMessage;
  final PinepodsService _pinepodsService = PinepodsService();

  @override
  void initState() {
    super.initState();
    _loadPodcasts();
  }

  Future<void> _loadPodcasts() async {
    final settingsBloc = Provider.of<SettingsBloc>(context, listen: false);
    final settings = settingsBloc.currentSettings;

    if (settings.pinepodsServer == null || 
        settings.pinepodsApiKey == null || 
        settings.pinepodsUserId == null) {
      setState(() {
        _errorMessage = 'Not connected to PinePods server. Please connect in Settings.';
        _isLoading = false;
      });
      return;
    }

    setState(() {
      _isLoading = true;
      _errorMessage = null;
    });

    try {
      // Initialize the service with the stored credentials
      _pinepodsService.setCredentials(
        settings.pinepodsServer!,
        settings.pinepodsApiKey!,
      );
      
      final podcasts = await _pinepodsService.getUserPodcasts(settings.pinepodsUserId!);
      
      setState(() {
        _podcasts = podcasts;
        _isLoading = false;
      });
    } catch (e) {
      setState(() {
        _errorMessage = e.toString();
        _isLoading = false;
      });
    }
  }

  @override
  Widget build(BuildContext context) {
    final settingsBloc = Provider.of<SettingsBloc>(context);

    if (_isLoading) {
      return const SliverFillRemaining(
        hasScrollBody: false,
        child: Column(
          mainAxisAlignment: MainAxisAlignment.center,
          crossAxisAlignment: CrossAxisAlignment.center,
          children: <Widget>[
            PlatformProgressIndicator(),
          ],
        ),
      );
    }

    if (_errorMessage != null) {
      return SliverFillRemaining(
        hasScrollBody: false,
        child: Padding(
          padding: const EdgeInsets.all(32.0),
          child: Column(
            mainAxisAlignment: MainAxisAlignment.center,
            crossAxisAlignment: CrossAxisAlignment.center,
            children: <Widget>[
              Icon(
                Icons.error_outline,
                size: 75,
                color: Theme.of(context).colorScheme.error,
              ),
              const SizedBox(height: 16),
              Text(
                'Error loading podcasts',
                style: Theme.of(context).textTheme.titleLarge,
                textAlign: TextAlign.center,
              ),
              const SizedBox(height: 8),
              Text(
                _errorMessage!,
                style: Theme.of(context).textTheme.bodyMedium,
                textAlign: TextAlign.center,
              ),
              const SizedBox(height: 16),
              ElevatedButton(
                onPressed: _loadPodcasts,
                child: const Text('Retry'),
              ),
            ],
          ),
        ),
      );
    }

    if (_podcasts == null || _podcasts!.isEmpty) {
      return SliverFillRemaining(
        hasScrollBody: false,
        child: Padding(
          padding: const EdgeInsets.all(32.0),
          child: Column(
            mainAxisAlignment: MainAxisAlignment.center,
            crossAxisAlignment: CrossAxisAlignment.center,
            children: <Widget>[
              Icon(
                Icons.podcasts,
                size: 75,
                color: Theme.of(context).primaryColor,
              ),
              const SizedBox(height: 16),
              Text(
                'No podcasts found',
                style: Theme.of(context).textTheme.titleLarge,
                textAlign: TextAlign.center,
              ),
              const SizedBox(height: 8),
              Text(
                'You haven\'t subscribed to any podcasts yet. Search for podcasts to get started!',
                style: Theme.of(context).textTheme.bodyMedium,
                textAlign: TextAlign.center,
              ),
            ],
          ),
        ),
      );
    }

    return StreamBuilder<AppSettings>(
      stream: settingsBloc.settings,
      builder: (context, settingsSnapshot) {
        if (settingsSnapshot.hasData) {
          var mode = settingsSnapshot.data!.layout;
          var size = mode == 1 ? 100.0 : 160.0;

          if (mode == 0) {
            // List view
            return SliverList(
              delegate: SliverChildBuilderDelegate(
                (BuildContext context, int index) {
                  return PinepodsPodcastTile(podcast: _podcasts![index]);
                },
                childCount: _podcasts!.length,
                addAutomaticKeepAlives: false,
              ),
            );
          }

          // Grid view
          return SliverGrid(
            gridDelegate: SliverGridDelegateWithMaxCrossAxisExtent(
              maxCrossAxisExtent: size,
              mainAxisSpacing: 10.0,
              crossAxisSpacing: 10.0,
            ),
            delegate: SliverChildBuilderDelegate(
              (BuildContext context, int index) {
                return PinepodsPodcastGridTile(podcast: _podcasts![index]);
              },
              childCount: _podcasts!.length,
            ),
          );
        } else {
          return const SliverFillRemaining(
            hasScrollBody: false,
            child: SizedBox(
              height: 0,
              width: 0,
            ),
          );
        }
      },
    );
  }
}