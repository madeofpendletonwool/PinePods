// lib/ui/pinepods/people/person_page.dart
//
// A person's page: who they are, follow, the shows they're on, and the
// episodes they're credited in, in the library and elsewhere.

import 'dart:async';
import 'dart:math';

import 'package:flutter/material.dart';
import 'package:pinepods_mobile/entities/people.dart';
import 'package:pinepods_mobile/entities/pinepods_episode.dart';
import 'package:pinepods_mobile/services/global_services.dart';
import 'package:pinepods_mobile/ui/pinepods/episode_details.dart';
import 'package:pinepods_mobile/ui/pinepods/people/people_widgets.dart';
import 'package:pinepods_mobile/ui/utils/player_utils.dart';
import 'package:pinepods_mobile/ui/widgets/pinepods_episode_card.dart';
import 'package:pinepods_mobile/ui/widgets/platform_progress_indicator.dart';
import 'package:pinepods_mobile/ui/widgets/podcast_image.dart';
import 'package:url_launcher/url_launcher.dart';
import 'package:pinepods_mobile/ui/icons/phosphor_icons.dart';

const int _pageSize = 50;

/// How often, and how many times, to re-check while the server looks up
/// episodes from shows outside the library.
const Duration _refreshPoll = Duration(seconds: 4);
const int _maxRefreshPolls = 8;

class PersonPage extends StatefulWidget {
  final int personId;

  /// Shown in the app bar until the profile loads.
  final String? initialName;

  const PersonPage({super.key, required this.personId, this.initialName});

  @override
  State<PersonPage> createState() => _PersonPageState();
}

enum _Tab { library, elsewhere }

class _PersonPageState extends State<PersonPage> {
  PersonProfile? _profile;
  List<PinepodsEpisode> _episodes = [];
  bool _hasMore = false;
  bool _loadingMore = false;
  String? _error;
  _Tab _tab = _Tab.library;
  bool _tabChosen = false;
  Timer? _poll;
  int _polls = 0;

  @override
  void initState() {
    super.initState();
    _load();
  }

  @override
  void dispose() {
    _poll?.cancel();
    super.dispose();
  }

  Future<void> _load() async {
    final service = signedInService(context);
    if (service == null) {
      setState(() => _error = 'Not connected to a PinePods server');
      return;
    }
    try {
      // Keep however many library episodes are already loaded.
      final limit = max(_pageSize, _episodes.length);
      final profile = await service.getPersonProfile(widget.personId, limit: limit);
      if (!mounted) return;
      setState(() {
        _profile = profile;
        _episodes = profile.episodes;
        _hasMore = profile.episodes.length >= limit;
        _error = null;
        // Default to whichever tab has something, until the user picks one.
        if (!_tabChosen) {
          _tab = profile.episodes.isEmpty && profile.externalEpisodes.isNotEmpty ? _Tab.elsewhere : _Tab.library;
        }
      });
      _poll?.cancel();
      if (profile.refreshing && _polls < _maxRefreshPolls) {
        _polls++;
        _poll = Timer(_refreshPoll, _load);
      }
    } catch (e) {
      if (!mounted) return;
      setState(() => _error = e.toString().replaceFirst('Exception: ', ''));
    }
  }

  Future<void> _loadMore() async {
    final service = signedInService(context);
    if (service == null || _loadingMore) return;
    setState(() => _loadingMore = true);
    try {
      final page = await service.getPersonProfile(widget.personId, limit: _pageSize, offset: _episodes.length);
      if (!mounted) return;
      setState(() {
        _episodes = [..._episodes, ...page.episodes];
        _hasMore = page.episodes.length >= _pageSize;
      });
    } catch (_) {
      // Leave the button so the user can retry.
    } finally {
      if (mounted) setState(() => _loadingMore = false);
    }
  }

  Future<void> _play(PinepodsEpisode episode) async {
    final audio = GlobalServices.pinepodsAudioService;
    if (audio == null) return;
    try {
      await playPinepodsEpisodeWithOptionalFullScreen(context, audio, episode, resume: episode.isStarted);
    } catch (e) {
      if (!mounted) return;
      ScaffoldMessenger.of(context).showSnackBar(SnackBar(content: Text('Failed to play episode: $e')));
    }
  }

  void _openEpisode(PinepodsEpisode episode) {
    if (episode.episodeId > 0) {
      Navigator.push(
        context,
        MaterialPageRoute<void>(builder: (context) => PinepodsEpisodeDetails(initialEpisode: episode)),
      );
      return;
    }
    // Outside the library: open its show, which lists the feed's episodes.
    final show = _profile?.podcasts.where((p) => !p.inLibrary && p.podcastName == episode.podcastName).firstOrNull;
    if (show != null) openPersonPodcast(context, show);
  }

  @override
  Widget build(BuildContext context) {
    final profile = _profile;
    return Scaffold(
      appBar: AppBar(title: Text(profile?.person.name ?? widget.initialName ?? '')),
      body: SafeArea(
        child: Column(
          children: [
            Expanded(
              child: profile == null
                  ? Center(
                      child: _error == null
                          ? const PlatformProgressIndicator()
                          : Padding(
                              padding: const EdgeInsets.all(24),
                              child: Column(
                                mainAxisSize: MainAxisSize.min,
                                children: [
                                  Text(_error!, textAlign: TextAlign.center),
                                  const SizedBox(height: 12),
                                  OutlinedButton(onPressed: _load, child: const Text('Retry')),
                                ],
                              ),
                            ),
                    )
                  : RefreshIndicator(
                      onRefresh: _load,
                      child: CustomScrollView(
                        slivers: [
                          SliverToBoxAdapter(child: _header(profile)),
                          if (profile.podcasts.isNotEmpty) SliverToBoxAdapter(child: _shows(profile)),
                          SliverToBoxAdapter(child: _tabs(profile)),
                          ..._episodeList(profile),
                        ],
                      ),
                    ),
            ),
          ],
        ),
      ),
    );
  }

  Widget _header(PersonProfile profile) {
    final theme = Theme.of(context);
    final person = profile.person;
    final link = person.href;
    return Padding(
      padding: const EdgeInsets.fromLTRB(16, 16, 16, 8),
      child: Row(
        children: [
          PersonAvatar(url: person.img, size: 88),
          const SizedBox(width: 16),
          Expanded(
            child: Column(
              crossAxisAlignment: CrossAxisAlignment.start,
              children: [
                Text(person.name, style: theme.textTheme.headlineSmall!.copyWith(fontWeight: FontWeight.bold)),
                if (link != null && Uri.tryParse(link)?.hasScheme == true)
                  InkWell(
                    onTap: () => launchUrl(Uri.parse(link), mode: LaunchMode.externalApplication),
                    child: Padding(
                      padding: const EdgeInsets.symmetric(vertical: 4),
                      child: Text(
                        Uri.parse(link).host,
                        style: theme.textTheme.bodyMedium!.copyWith(color: theme.colorScheme.primary),
                        maxLines: 1,
                        overflow: TextOverflow.ellipsis,
                      ),
                    ),
                  ),
                const SizedBox(height: 8),
                PersonFollowButton(
                  personId: person.id,
                  followed: person.followed,
                  onChanged: (followed) => setState(() {
                    _profile = PersonProfile(
                      person: profile.person.copyWith(followed: followed),
                      podcasts: profile.podcasts,
                      episodes: profile.episodes,
                      externalEpisodes: profile.externalEpisodes,
                      refreshing: profile.refreshing,
                    );
                  }),
                ),
              ],
            ),
          ),
        ],
      ),
    );
  }

  Widget _shows(PersonProfile profile) {
    final theme = Theme.of(context);
    final muted = theme.textTheme.bodySmall!.copyWith(color: theme.colorScheme.onSurfaceVariant);
    return Padding(
      padding: const EdgeInsets.only(top: 8),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          Padding(
            padding: const EdgeInsets.symmetric(horizontal: 16),
            child: Text('Shows', style: theme.textTheme.titleMedium!.copyWith(fontWeight: FontWeight.bold)),
          ),
          const SizedBox(height: 8),
          SizedBox(
            height: 180,
            child: ListView.separated(
              padding: const EdgeInsets.symmetric(horizontal: 16),
              scrollDirection: Axis.horizontal,
              itemCount: profile.podcasts.length,
              separatorBuilder: (_, _) => const SizedBox(width: 12),
              itemBuilder: (context, index) {
                final show = profile.podcasts[index];
                final roles = show.roles.map(roleLabel).join(', ');
                final count = '${show.episodeCount} episode${show.episodeCount == 1 ? '' : 's'}';
                final detail = show.podcastLevel || show.episodeCount == 0
                    ? roles
                    : (roles.isEmpty ? count : '$roles · $count');
                final fallback = Container(
                  color: theme.colorScheme.surfaceContainerHighest,
                  child: Icon(PhosphorIconsRegular.microphoneStage, color: theme.colorScheme.onSurfaceVariant),
                );
                return InkWell(
                  borderRadius: BorderRadius.circular(8),
                  onTap: () => openPersonPodcast(context, show),
                  child: SizedBox(
                    width: 112,
                    child: Column(
                      crossAxisAlignment: CrossAxisAlignment.start,
                      children: [
                        Stack(
                          children: [
                            ClipRRect(
                              borderRadius: BorderRadius.circular(8),
                              child: SizedBox(
                                width: 112,
                                height: 112,
                                child: show.artworkUrl == null
                                    ? fallback
                                    : PodcastImage(
                                        url: show.artworkUrl!,
                                        width: 112,
                                        height: 112,
                                        placeholder: fallback,
                                        errorPlaceholder: fallback,
                                      ),
                              ),
                            ),
                            if (!show.inLibrary)
                              Positioned(
                                left: 4,
                                top: 4,
                                child: Container(
                                  padding: const EdgeInsets.symmetric(horizontal: 6, vertical: 2),
                                  decoration: BoxDecoration(
                                    color: Colors.black.withValues(alpha: 0.65),
                                    borderRadius: BorderRadius.circular(4),
                                  ),
                                  child: const Text(
                                    'Not in library',
                                    style: TextStyle(color: Colors.white, fontSize: 10),
                                  ),
                                ),
                              ),
                          ],
                        ),
                        const SizedBox(height: 4),
                        Text(
                          show.podcastName,
                          style: theme.textTheme.bodySmall!.copyWith(fontWeight: FontWeight.w600),
                          maxLines: 2,
                          overflow: TextOverflow.ellipsis,
                        ),
                        if (detail.isNotEmpty)
                          Text(detail, style: muted, maxLines: 1, overflow: TextOverflow.ellipsis),
                      ],
                    ),
                  ),
                );
              },
            ),
          ),
        ],
      ),
    );
  }

  Widget _tabs(PersonProfile profile) {
    final libraryCount = _hasMore ? '${_episodes.length}+' : '${_episodes.length}';
    return Padding(
      padding: const EdgeInsets.fromLTRB(16, 16, 16, 8),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.stretch,
        children: [
          SegmentedButton<_Tab>(
            segments: [
              ButtonSegment(value: _Tab.library, label: Text('In your library ($libraryCount)')),
              ButtonSegment(value: _Tab.elsewhere, label: Text('Elsewhere (${profile.externalEpisodes.length})')),
            ],
            selected: {_tab},
            showSelectedIcon: false,
            onSelectionChanged: (s) => setState(() {
              _tab = s.first;
              _tabChosen = true;
            }),
          ),
          if (profile.refreshing && _tab == _Tab.elsewhere)
            const Padding(
              padding: EdgeInsets.only(top: 8),
              child: LinearProgressIndicator(),
            ),
        ],
      ),
    );
  }

  List<Widget> _episodeList(PersonProfile profile) {
    final episodes = _tab == _Tab.library ? _episodes : profile.externalEpisodes;
    if (episodes.isEmpty) {
      final message = _tab == _Tab.library
          ? 'No episodes with ${profile.person.name} in your library.'
          : profile.refreshing
              ? 'Looking for episodes on other shows…'
              : 'No episodes found on shows outside your library.';
      return [
        SliverToBoxAdapter(
          child: Padding(
            padding: const EdgeInsets.all(32),
            child: Text(message, textAlign: TextAlign.center),
          ),
        ),
      ];
    }
    return [
      SliverList.builder(
        itemCount: episodes.length,
        itemBuilder: (context, index) {
          final episode = episodes[index];
          return PinepodsEpisodeCard(
            episode: episode,
            onTap: () => _openEpisode(episode),
            onPlayPressed: () => _play(episode),
          );
        },
      ),
      if (_tab == _Tab.library && _hasMore)
        SliverToBoxAdapter(
          child: Padding(
            padding: const EdgeInsets.all(16),
            child: Center(
              child: _loadingMore
                  ? const PlatformProgressIndicator()
                  : OutlinedButton(onPressed: _loadMore, child: const Text('Load more')),
            ),
          ),
        ),
      const SliverToBoxAdapter(child: SizedBox(height: 16)),
    ];
  }
}
