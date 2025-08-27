// Copyright 2020 Ben Hills and the project contributors. All rights reserved.
// Use of this source code is governed by a BSD-style license that can be
// found in the LICENSE file.

import 'package:pinepods_mobile/bloc/podcast/podcast_bloc.dart';
import 'package:pinepods_mobile/bloc/settings/settings_bloc.dart';
import 'package:pinepods_mobile/entities/podcast.dart';
import 'package:pinepods_mobile/entities/pinepods_search.dart';
import 'package:pinepods_mobile/ui/podcast/podcast_details.dart';
import 'package:pinepods_mobile/ui/pinepods/podcast_details.dart';
import 'package:pinepods_mobile/ui/widgets/tile_image.dart';
import 'package:pinepods_mobile/services/pinepods/pinepods_service.dart';
import 'package:flutter/material.dart';
import 'package:provider/provider.dart';

class PodcastTile extends StatelessWidget {
  final Podcast podcast;

  const PodcastTile({
    super.key,
    required this.podcast,
  });

  @override
  Widget build(BuildContext context) {
    final podcastBloc = Provider.of<PodcastBloc>(context);

    return ListTile(
      onTap: () async {
        await _navigateToPodcastDetails(context, podcastBloc);
      },
      minVerticalPadding: 9,
      leading: ExcludeSemantics(
        child: Hero(
          key: Key('tilehero${podcast.imageUrl}:${podcast.link}'),
          tag: '${podcast.imageUrl}:${podcast.link}',
          child: TileImage(
            url: podcast.imageUrl!,
            size: 60,
          ),
        ),
      ),
      title: Text(
        podcast.title,
        maxLines: 1,
      ),

      /// A ListTile's density changes depending upon whether we have 2 or more lines of text. We
      /// manually add a newline character here to ensure the density is consistent whether the
      /// podcast subtitle spans 1 or more lines. Bit of a hack, but a simple solution.
      subtitle: Text(
        '${podcast.copyright ?? ''}\n',
        maxLines: 2,
      ),
      isThreeLine: false,
    );
  }

  Future<void> _navigateToPodcastDetails(BuildContext context, PodcastBloc podcastBloc) async {
    // Check if this is a PinePods setup and if the podcast is already subscribed
    final settingsBloc = Provider.of<SettingsBloc>(context, listen: false);
    final settings = settingsBloc.currentSettings;
    
    if (settings.pinepodsServer != null && 
        settings.pinepodsApiKey != null && 
        settings.pinepodsUserId != null) {
      
      // Check if podcast is already subscribed
      final pinepodsService = PinepodsService();
      pinepodsService.setCredentials(settings.pinepodsServer!, settings.pinepodsApiKey!);
      
      try {
        final isSubscribed = await pinepodsService.checkPodcastExists(
          podcast.title, 
          podcast.url!, 
          settings.pinepodsUserId!
        );
        
        if (isSubscribed) {
          // Use PinePods podcast details for subscribed podcasts
          final unifiedPodcast = UnifiedPinepodsPodcast(
            id: 0, // Will be fetched by PinePods component (API is now fixed)
            indexId: 0, // Default for subscribed podcasts
            title: podcast.title,
            url: podcast.url ?? '',
            originalUrl: podcast.url ?? '',
            link: podcast.link ?? '',
            description: podcast.description ?? '',
            author: podcast.copyright ?? '',
            ownerName: podcast.copyright ?? '',
            image: podcast.imageUrl ?? '',
            artwork: podcast.imageUrl ?? '',
            lastUpdateTime: 0,
            explicit: false,
            episodeCount: 0, // Will be loaded
          );
          
          if (context.mounted) {
            Navigator.push(
              context,
              MaterialPageRoute<void>(
                settings: const RouteSettings(name: 'pinepodspodcastdetails'),
                builder: (context) => PinepodsPodcastDetails(
                  podcast: unifiedPodcast,
                  isFollowing: true,
                ),
              ),
            );
          }
          return;
        }
      } catch (e) {
        // If check fails, fall through to standard podcast details
        print('Error checking subscription status: $e');
      }
    }
    
    // Use standard podcast details for non-subscribed or non-PinePods setups
    if (context.mounted) {
      Navigator.push(
        context,
        MaterialPageRoute<void>(
          settings: const RouteSettings(name: 'podcastdetails'),
          builder: (context) => PodcastDetails(podcast, podcastBloc),
        ),
      );
    }
  }
}
