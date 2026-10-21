// lib/ui/pinepods/people/followed_people.dart
//
// The People tab: find anyone (on this server or PodPeopleDB), everyone the
// user follows, and one combined list of their latest episodes across every
// show.

import 'dart:async';

import 'package:flutter/material.dart';
import 'package:pinepods_mobile/entities/people.dart';
import 'package:pinepods_mobile/entities/pinepods_episode.dart';
import 'package:pinepods_mobile/services/global_services.dart';
import 'package:pinepods_mobile/ui/pinepods/episode_details.dart';
import 'package:pinepods_mobile/ui/pinepods/people/people_widgets.dart';
import 'package:pinepods_mobile/ui/utils/player_utils.dart';
import 'package:pinepods_mobile/ui/widgets/pinepods_episode_card.dart';
import 'package:pinepods_mobile/ui/widgets/platform_progress_indicator.dart';

/// Episodes fetched per person, and the most shown in the combined list.
const int _perPerson = 20;
const int _feedLimit = 100;

/// The People tab. A single sliver, like the other bottom-bar pages.
class PinepodsPeople extends StatefulWidget {
  const PinepodsPeople({super.key});

  @override
  State<PinepodsPeople> createState() => _PinepodsPeopleState();
}

class _PinepodsPeopleState extends State<PinepodsPeople> {
  List<PersonInfo>? _people;
  List<PinepodsEpisode>? _feed;

  /// Shows outside the library, by name, so their episodes can open the show.
  final Map<String, PersonPodcast> _externalShows = {};
  String? _error;

  final _searchController = TextEditingController();
  Timer? _debounce;
  int _searchSeq = 0;
  List<PersonHit>? _hits;
  bool _searching = false;
  bool _opening = false;

  @override
  void initState() {
    super.initState();
    _load();
  }

  @override
  void dispose() {
    _debounce?.cancel();
    _searchController.dispose();
    super.dispose();
  }

  Future<void> _load() async {
    final service = signedInService(context);
    if (service == null) {
      setState(() => _error = 'Not connected to a PinePods server');
      return;
    }
    try {
      final people = await service.getFollowedPeople();
      if (!mounted) return;
      setState(() {
        _people = people;
        _error = null;
      });

      final profiles = await Future.wait(people.map((p) async {
        try {
          return await service.getPersonProfile(p.id, limit: _perPerson);
        } catch (_) {
          return null;
        }
      }));
      final seen = <String>{};
      final episodes = <PinepodsEpisode>[];
      for (final profile in profiles.whereType<PersonProfile>()) {
        for (final show in profile.podcasts.where((s) => !s.inLibrary)) {
          _externalShows[show.podcastName] = show;
        }
        for (final ep in [...profile.episodes, ...profile.externalEpisodes]) {
          // The same episode can credit several followed people.
          if (seen.add(ep.episodeUrl)) episodes.add(ep);
        }
      }
      episodes.sort((a, b) => b.episodePubDate.compareTo(a.episodePubDate));
      if (!mounted) return;
      setState(() => _feed = episodes.take(_feedLimit).toList());
    } catch (e) {
      if (!mounted) return;
      setState(() => _error = e.toString().replaceFirst('Exception: ', ''));
    }
  }

  void _onSearchChanged(String text) {
    _debounce?.cancel();
    final query = text.trim();
    if (query.length < 2) {
      _searchSeq++;
      setState(() {
        _hits = null;
        _searching = false;
      });
      return;
    }
    setState(() => _searching = true);
    _debounce = Timer(const Duration(milliseconds: 300), () async {
      final service = signedInService(context);
      if (service == null) return;
      final seq = ++_searchSeq;
      List<PersonHit> hits;
      try {
        hits = await service.searchPeople(query);
      } catch (_) {
        hits = [];
      }
      // Only the latest query's results are kept.
      if (!mounted || seq != _searchSeq) return;
      setState(() {
        _hits = hits;
        _searching = false;
      });
    });
  }

  void _clearSearch() {
    _searchController.clear();
    _onSearchChanged('');
    FocusScope.of(context).unfocus();
  }

  /// Open a person page, then refresh: they may have been followed or
  /// unfollowed there.
  Future<void> _open(int personId, String name) async {
    await openPerson(context, personId, name: name);
    if (mounted) _load();
  }

  /// Search hits from PodPeopleDB may not be on this server yet; the lookup
  /// brings them in and gives their id.
  Future<void> _openHit(PersonHit hit) async {
    if (_opening) return;
    var id = hit.personId;
    if (id == null && hit.podpeopleId != null) {
      final service = signedInService(context);
      setState(() => _opening = true);
      try {
        id = await service?.lookupPerson(podpeopleId: hit.podpeopleId);
      } catch (_) {
        id = null;
      }
      if (!mounted) return;
      setState(() => _opening = false);
    }
    if (id == null) {
      ScaffoldMessenger.of(context).showSnackBar(SnackBar(content: Text('Couldn\'t open ${hit.name}')));
      return;
    }
    await _open(id, hit.name);
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
    } else if (_externalShows[episode.podcastName] case final show?) {
      openPersonPodcast(context, show);
    }
  }

  @override
  Widget build(BuildContext context) {
    final searching = _hits != null || _searching;
    return SliverMainAxisGroup(
      slivers: [
        SliverToBoxAdapter(child: _searchBar()),
        if (searching) ..._searchResults() else ..._followed(),
        const SliverToBoxAdapter(child: SizedBox(height: 16)),
      ],
    );
  }

  Widget _searchBar() {
    return Padding(
      padding: const EdgeInsets.fromLTRB(16, 16, 16, 8),
      child: TextField(
        controller: _searchController,
        textInputAction: TextInputAction.search,
        onChanged: _onSearchChanged,
        decoration: InputDecoration(
          hintText: 'Find hosts and guests',
          prefixIcon: const Icon(Icons.person_search),
          suffixIcon: _searchController.text.isEmpty
              ? null
              : IconButton(icon: const Icon(Icons.clear), tooltip: 'Clear', onPressed: _clearSearch),
          border: OutlineInputBorder(borderRadius: BorderRadius.circular(12)),
          filled: true,
          fillColor: Theme.of(context).cardColor,
          contentPadding: const EdgeInsets.symmetric(horizontal: 16, vertical: 12),
        ),
      ),
    );
  }

  List<Widget> _searchResults() {
    final theme = Theme.of(context);
    final hits = _hits;
    if (hits == null || (_searching && hits.isEmpty)) {
      return const [
        SliverToBoxAdapter(
          child: Padding(padding: EdgeInsets.all(32), child: Center(child: PlatformProgressIndicator())),
        ),
      ];
    }
    if (hits.isEmpty) {
      return [
        SliverToBoxAdapter(
          child: Padding(
            padding: const EdgeInsets.all(32),
            child: Text(
              'Nobody found. People are added from podcast and episode pages.',
              textAlign: TextAlign.center,
              style: theme.textTheme.bodyMedium,
            ),
          ),
        ),
      ];
    }
    return [
      if (_opening) const SliverToBoxAdapter(child: LinearProgressIndicator()),
      SliverList.builder(
        itemCount: hits.length,
        itemBuilder: (context, index) {
          final hit = hits[index];
          return ListTile(
            leading: PersonAvatar(url: hit.img, size: 44),
            title: Text(hit.name),
            subtitle: hit.knownFor != null ? Text(hit.knownFor!, maxLines: 1, overflow: TextOverflow.ellipsis) : null,
            trailing: hit.followed ? Icon(Icons.check_circle, color: theme.colorScheme.primary) : null,
            onTap: () => _openHit(hit),
          );
        },
      ),
    ];
  }

  List<Widget> _followed() {
    final theme = Theme.of(context);
    final people = _people;
    if (people == null) {
      return [
        SliverToBoxAdapter(
          child: Padding(
            padding: const EdgeInsets.all(32),
            child: Center(
              child: _error == null
                  ? const PlatformProgressIndicator()
                  : Column(
                      children: [
                        Text(_error!, textAlign: TextAlign.center),
                        const SizedBox(height: 12),
                        OutlinedButton(onPressed: _load, child: const Text('Retry')),
                      ],
                    ),
            ),
          ),
        ),
      ];
    }
    if (people.isEmpty) {
      return [
        SliverToBoxAdapter(
          child: Padding(
            padding: const EdgeInsets.all(32),
            child: Column(
              children: [
                Icon(Icons.people_outline, size: 64, color: theme.colorScheme.onSurfaceVariant),
                const SizedBox(height: 16),
                Text('Not following anyone yet', style: theme.textTheme.titleMedium),
                const SizedBox(height: 8),
                const Text(
                  'Search above, or tap a host or guest on a podcast or episode, then Follow. '
                  'Their new episodes on any show will show up here.',
                  textAlign: TextAlign.center,
                ),
              ],
            ),
          ),
        ),
      ];
    }

    final feed = _feed;
    return [
      SliverToBoxAdapter(
        child: Padding(
          padding: const EdgeInsets.fromLTRB(16, 8, 16, 8),
          child: Text(
            'Following · ${people.length}',
            style: theme.textTheme.titleMedium!.copyWith(fontWeight: FontWeight.bold),
          ),
        ),
      ),
      SliverPadding(
        padding: const EdgeInsets.symmetric(horizontal: 12),
        sliver: SliverGrid.builder(
          gridDelegate: const SliverGridDelegateWithMaxCrossAxisExtent(
            maxCrossAxisExtent: 96,
            mainAxisExtent: 116,
            crossAxisSpacing: 4,
          ),
          itemCount: people.length,
          itemBuilder: (context, index) {
            final person = people[index];
            return InkWell(
              borderRadius: BorderRadius.circular(8),
              onTap: () => _open(person.id, person.name),
              child: Padding(
                padding: const EdgeInsets.all(4),
                child: Column(
                  children: [
                    PersonAvatar(url: person.img, size: 64),
                    const SizedBox(height: 6),
                    Text(
                      person.name,
                      style: theme.textTheme.bodySmall!.copyWith(fontWeight: FontWeight.w600),
                      textAlign: TextAlign.center,
                      maxLines: 2,
                      overflow: TextOverflow.ellipsis,
                    ),
                  ],
                ),
              ),
            );
          },
        ),
      ),
      SliverToBoxAdapter(
        child: Padding(
          padding: const EdgeInsets.fromLTRB(16, 16, 16, 8),
          child: Text('Latest episodes', style: theme.textTheme.titleMedium!.copyWith(fontWeight: FontWeight.bold)),
        ),
      ),
      if (feed == null)
        const SliverToBoxAdapter(
          child: Padding(padding: EdgeInsets.all(32), child: Center(child: PlatformProgressIndicator())),
        )
      else if (feed.isEmpty)
        const SliverToBoxAdapter(
          child: Padding(padding: EdgeInsets.all(32), child: Text('No episodes yet.', textAlign: TextAlign.center)),
        )
      else
        SliverList.builder(
          itemCount: feed.length,
          itemBuilder: (context, index) {
            final episode = feed[index];
            return PinepodsEpisodeCard(
              episode: episode,
              onTap: () => _openEpisode(episode),
              onPlayPressed: () => _play(episode),
            );
          },
        ),
    ];
  }
}
