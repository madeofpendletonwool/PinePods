// People credited on podcasts and episodes, as served by the PinePods server's
// /api/data/people/* endpoints. These are distinct from [Person] (person.dart),
// which is the Podcasting 2.0 tag attached to a playing episode.

import 'package:pinepods_mobile/entities/pinepods_episode.dart';

const String roleHost = 'host';
const String roleCoHost = 'co-host';
const String roleGuest = 'guest';

String? _nonEmpty(dynamic value) {
  final s = value as String?;
  return (s == null || s.isEmpty) ? null : s;
}

List<String> _strings(dynamic value) =>
    (value as List<dynamic>? ?? const []).map((e) => e.toString()).toList();

/// Display label for a Podcasting 2.0 role ("co-host" -> "Co-host").
String roleLabel(String role) {
  if (role.isEmpty) return '';
  return role[0].toUpperCase() + role.substring(1);
}

/// A person as shown on a podcast or episode page.
class CreditedPerson {
  /// PinePods person id; 0 when the server is too old to provide one (then the
  /// person can't be opened or followed).
  final int id;
  final int? podpeopleId;
  final String name;
  final String? img;
  final String? href;

  /// Most significant role ("host", "co-host", "guest", ...).
  final String role;
  final List<String> roles;
  final String? group;

  /// "podcast", "episodes" or "episode".
  final String scope;
  final int episodeCount;
  final List<String> sources;

  /// False when every credit is an unreviewed community submission.
  final bool verified;
  final bool followed;

  const CreditedPerson({
    required this.id,
    this.podpeopleId,
    required this.name,
    this.img,
    this.href,
    this.role = '',
    this.roles = const [],
    this.group,
    this.scope = 'podcast',
    this.episodeCount = 0,
    this.sources = const [],
    this.verified = true,
    this.followed = false,
  });

  factory CreditedPerson.fromJson(Map<String, dynamic> json) {
    return CreditedPerson(
      id: json['id'] as int? ?? 0,
      podpeopleId: json['podpeople_id'] as int?,
      name: json['name'] as String? ?? '',
      img: _nonEmpty(json['img']),
      href: _nonEmpty(json['href']),
      role: json['role'] as String? ?? '',
      roles: _strings(json['roles']),
      group: _nonEmpty(json['group']),
      scope: json['scope'] as String? ?? 'podcast',
      episodeCount: json['episode_count'] as int? ?? 0,
      sources: _strings(json['sources']),
      verified: json['verified'] as bool? ?? true,
      followed: json['followed'] as bool? ?? false,
    );
  }

  /// Whether the person page can be opened (the server knows this person).
  bool get canOpen => id > 0;
}

/// Everyone credited on a podcast: regulars and episode-only guests.
class PodcastCredits {
  final List<CreditedPerson> people;
  final List<CreditedPerson> guests;

  const PodcastCredits({this.people = const [], this.guests = const []});

  factory PodcastCredits.fromJson(Map<String, dynamic> json) {
    List<CreditedPerson> list(String key) => (json[key] as List<dynamic>? ?? const [])
        .map((e) => CreditedPerson.fromJson(e as Map<String, dynamic>))
        .where((p) => p.name.isNotEmpty)
        .toList();
    return PodcastCredits(people: list('people'), guests: list('guests'));
  }
}

/// A person's identity, as on the person page and the followed list.
class PersonInfo {
  final int id;
  final int? podpeopleId;
  final String name;
  final String? img;
  final String? href;
  final bool followed;

  const PersonInfo({
    required this.id,
    this.podpeopleId,
    required this.name,
    this.img,
    this.href,
    this.followed = false,
  });

  factory PersonInfo.fromJson(Map<String, dynamic> json) {
    return PersonInfo(
      id: json['id'] as int? ?? 0,
      podpeopleId: json['podpeople_id'] as int?,
      name: json['name'] as String? ?? '',
      img: _nonEmpty(json['img']),
      href: _nonEmpty(json['href']),
      followed: json['followed'] as bool? ?? false,
    );
  }

  PersonInfo copyWith({bool? followed}) => PersonInfo(
        id: id,
        podpeopleId: podpeopleId,
        name: name,
        img: img,
        href: href,
        followed: followed ?? this.followed,
      );
}

/// A podcast a person is credited on.
class PersonPodcast {
  /// The user's podcast id, or null for a show outside their library.
  final int? podcastId;
  final String podcastName;
  final String? artworkUrl;
  final String feedUrl;
  final int? podcastIndexId;
  final List<String> roles;
  final bool podcastLevel;
  final int episodeCount;

  const PersonPodcast({
    this.podcastId,
    required this.podcastName,
    this.artworkUrl,
    required this.feedUrl,
    this.podcastIndexId,
    this.roles = const [],
    this.podcastLevel = false,
    this.episodeCount = 0,
  });

  bool get inLibrary => podcastId != null && podcastId! > 0;

  factory PersonPodcast.fromJson(Map<String, dynamic> json) {
    return PersonPodcast(
      podcastId: json['podcastid'] as int?,
      podcastName: json['podcastname'] as String? ?? '',
      artworkUrl: _nonEmpty(json['artworkurl']),
      feedUrl: json['feedurl'] as String? ?? '',
      podcastIndexId: json['podcastindexid'] as int?,
      roles: _strings(json['roles']),
      podcastLevel: json['podcast_level'] as bool? ?? false,
      episodeCount: json['episode_count'] as int? ?? 0,
    );
  }
}

/// A person's page: who they are, their shows, and their episodes.
class PersonProfile {
  final PersonInfo person;
  final List<PersonPodcast> podcasts;

  /// Library episodes the person is credited on, newest first.
  final List<PinepodsEpisode> episodes;

  /// Recent episodes from shows outside the library (episode id 0: played by URL).
  final List<PinepodsEpisode> externalEpisodes;

  /// True while the server refreshes [externalEpisodes] in the background.
  final bool refreshing;

  const PersonProfile({
    required this.person,
    this.podcasts = const [],
    this.episodes = const [],
    this.externalEpisodes = const [],
    this.refreshing = false,
  });

  factory PersonProfile.fromJson(Map<String, dynamic> json) {
    List<PinepodsEpisode> episodes(String key) => (json[key] as List<dynamic>? ?? const [])
        .map((e) => PinepodsEpisode.fromJson(e as Map<String, dynamic>))
        .toList();
    return PersonProfile(
      person: PersonInfo.fromJson(json['person'] as Map<String, dynamic>),
      podcasts: (json['podcasts'] as List<dynamic>? ?? const [])
          .map((e) => PersonPodcast.fromJson(e as Map<String, dynamic>))
          .toList(),
      episodes: episodes('episodes'),
      externalEpisodes: episodes('external_episodes'),
      refreshing: json['refreshing'] as bool? ?? false,
    );
  }
}

/// A people search result, from this server or PodPeopleDB.
class PersonHit {
  final int? personId;
  final int? podpeopleId;
  final String name;
  final String? img;

  /// A show they're known for, to tell same-named people apart.
  final String? knownFor;
  final bool followed;

  const PersonHit({
    this.personId,
    this.podpeopleId,
    required this.name,
    this.img,
    this.knownFor,
    this.followed = false,
  });

  factory PersonHit.fromJson(Map<String, dynamic> json) {
    return PersonHit(
      personId: json['person_id'] as int?,
      podpeopleId: json['podpeople_id'] as int?,
      name: json['name'] as String? ?? '',
      img: _nonEmpty(json['img']),
      knownFor: _nonEmpty(json['known_for']),
      followed: json['followed'] as bool? ?? false,
    );
  }
}

/// One person to add in a contribution: an existing person (by either id) or a new name.
class ContributePerson {
  final int? personId;
  final int? podpeopleId;
  final String name;
  final String role;

  const ContributePerson({
    this.personId,
    this.podpeopleId,
    required this.name,
    required this.role,
  });

  Map<String, dynamic> toJson() => {
        if (personId != null) 'person_id': personId,
        if (podpeopleId != null) 'podpeople_id': podpeopleId,
        // The name is sent for new people only; existing people are matched by id.
        if (personId == null && podpeopleId == null) 'name': name,
        'role': role,
      };
}

/// Split typed or pasted text into names. Commas, semicolons and line breaks
/// always separate; " and " and " & " only when [all] (paste or submit), so a
/// name being typed isn't cut. Returns the complete names and the unfinished
/// remainder (empty when [all]).
(List<String>, String) splitNames(String text, {required bool all}) {
  var normalized = text.replaceAll(RegExp(r'[\n\r;]'), ',');
  if (all) {
    normalized = normalized.replaceAll(RegExp(r' (and|&) ', caseSensitive: false), ',');
  }
  final raw = normalized.split(',');
  // Only the start of the remainder is trimmed: it's still being typed.
  final remainder = all ? '' : raw.removeLast().trimLeft();
  final names = raw.map((s) => s.trim()).where((s) => s.isNotEmpty).toList();
  return (names, remainder);
}
