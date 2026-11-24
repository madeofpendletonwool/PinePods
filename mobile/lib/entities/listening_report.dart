// Listening statistics from /api/data/get_extended_stats and /api/data/listening_report.
// Every field is optional on the wire: older servers send fewer of them.

int _int(dynamic v) => v is num ? v.toInt() : 0;
double? _double(dynamic v) => v is num ? v.toDouble() : null;
String _str(dynamic v) => v is String ? v : '';
List<int> _ints(dynamic v) => v is List ? v.map(_int).toList() : const [];
List<T> _list<T>(dynamic v, T Function(Map<String, dynamic>) f) =>
    v is List ? v.whereType<Map<String, dynamic>>().map(f).toList() : const [];

class DayStat {
  /// `YYYY-MM-DD` in the user's timezone.
  final String date;
  final int seconds;

  const DayStat(this.date, this.seconds);

  factory DayStat.fromJson(Map<String, dynamic> j) => DayStat(_str(j['date']), _int(j['seconds']));

  DateTime? get day => DateTime.tryParse(date.length >= 10 ? date.substring(0, 10) : date);
}

class MonthStat {
  /// `YYYY-MM`
  final String month;
  final int seconds;

  const MonthStat(this.month, this.seconds);

  factory MonthStat.fromJson(Map<String, dynamic> j) => MonthStat(_str(j['month']), _int(j['seconds']));

  int? get monthNumber => int.tryParse(month.split('-').elementAtOrNull(1) ?? '');
}

class NamedSeconds {
  final String name;
  final int seconds;

  const NamedSeconds(this.name, this.seconds);

  factory NamedSeconds.fromJson(Map<String, dynamic> j) => NamedSeconds(_str(j['name']), _int(j['seconds']));
}

/// Seconds saved by playback speed (negative when slower than 1x) and by skipping.
class TimeSaved {
  final int speed;
  final int silence;
  final int ads;

  const TimeSaved({this.speed = 0, this.silence = 0, this.ads = 0});

  factory TimeSaved.fromJson(Map<String, dynamic> j) =>
      TimeSaved(speed: _int(j['speed']), silence: _int(j['silence']), ads: _int(j['ads']));

  int get skipped => (silence > 0 ? silence : 0) + (ads > 0 ? ads : 0);
  int get total => (speed > 0 ? speed : 0) + skipped;
}

class ReportPodcast {
  final int? podcastId;
  final String name;
  final String artwork;
  final int seconds;
  final int episodes;

  const ReportPodcast({this.podcastId, required this.name, required this.artwork, required this.seconds, this.episodes = 0});

  factory ReportPodcast.fromJson(Map<String, dynamic> j) => ReportPodcast(
        podcastId: j['podcast_id'] is num ? _int(j['podcast_id']) : null,
        name: _str(j['name']),
        artwork: _str(j['artwork']),
        seconds: _int(j['seconds']),
        episodes: _int(j['episodes']),
      );
}

class ReportEpisode {
  final int? episodeId;
  final bool isYoutube;
  final String title;
  final String podcast;
  final String artwork;
  final int seconds;

  const ReportEpisode({this.episodeId, this.isYoutube = false, required this.title, required this.podcast, required this.artwork, required this.seconds});

  factory ReportEpisode.fromJson(Map<String, dynamic> j) => ReportEpisode(
        episodeId: j['episode_id'] is num ? _int(j['episode_id']) : null,
        isYoutube: j['is_youtube'] == true,
        title: _str(j['title']),
        podcast: _str(j['podcast']),
        artwork: _str(j['artwork']),
        seconds: _int(j['seconds']),
      );
}

class ReportPerson {
  final int personId;
  final String name;
  final String? image;
  final int seconds;
  final int episodes;

  const ReportPerson({required this.personId, required this.name, this.image, required this.seconds, this.episodes = 0});

  factory ReportPerson.fromJson(Map<String, dynamic> j) => ReportPerson(
        personId: _int(j['person_id']),
        name: _str(j['name']),
        image: j['image'] is String && (j['image'] as String).isNotEmpty ? j['image'] as String : null,
        seconds: _int(j['seconds']),
        episodes: _int(j['episodes']),
      );
}

class SessionStats {
  final int count;
  final int averageSeconds;
  final int longestSeconds;
  final int? longestStartedAt;

  const SessionStats({this.count = 0, this.averageSeconds = 0, this.longestSeconds = 0, this.longestStartedAt});

  factory SessionStats.fromJson(Map<String, dynamic> j) => SessionStats(
        count: _int(j['count']),
        averageSeconds: _int(j['average_seconds']),
        longestSeconds: _int(j['longest_seconds']),
        longestStartedAt: j['longest_started_at'] is num ? _int(j['longest_started_at']) : null,
      );
}

class ReleaseLag {
  final double medianDays;
  final double withinDay;
  final double withinWeek;
  final int episodes;

  const ReleaseLag({this.medianDays = 0, this.withinDay = 0, this.withinWeek = 0, this.episodes = 0});

  factory ReleaseLag.fromJson(Map<String, dynamic> j) => ReleaseLag(
        medianDays: _double(j['median_days']) ?? 0,
        withinDay: _double(j['within_day']) ?? 0,
        withinWeek: _double(j['within_week']) ?? 0,
        episodes: _int(j['episodes']),
      );
}

/// A period's (or all time's) listening.
class ListeningReport {
  final String timezone;
  final String? recordedSince;

  /// Months with listening, `YYYY-MM`.
  final List<String> months;
  final int seconds;
  final int activeDays;
  final int episodes;
  final int podcasts;
  final int finished;
  final List<ReportPodcast> topPodcasts;
  final List<ReportEpisode> topEpisodes;
  final List<NamedSeconds> topCategories;
  final List<ReportPerson> topPeople;
  final List<int> byHour;
  final List<int> byDow;
  final List<DayStat> daily;
  final List<MonthStat> byMonth;
  final DayStat? biggestDay;
  final DayStat? busiestWeek;
  final int longestStreak;
  final SessionStats sessions;
  final double? averageSpeed;
  final TimeSaved timeSaved;
  final List<ReportPodcast> adHeavy;
  final List<NamedSeconds> byClient;
  final int downloadedSeconds;
  final ReleaseLag? releaseLag;

  const ListeningReport({
    this.timezone = '',
    this.recordedSince,
    this.months = const [],
    this.seconds = 0,
    this.activeDays = 0,
    this.episodes = 0,
    this.podcasts = 0,
    this.finished = 0,
    this.topPodcasts = const [],
    this.topEpisodes = const [],
    this.topCategories = const [],
    this.topPeople = const [],
    this.byHour = const [],
    this.byDow = const [],
    this.daily = const [],
    this.byMonth = const [],
    this.biggestDay,
    this.busiestWeek,
    this.longestStreak = 0,
    this.sessions = const SessionStats(),
    this.averageSpeed,
    this.timeSaved = const TimeSaved(),
    this.adHeavy = const [],
    this.byClient = const [],
    this.downloadedSeconds = 0,
    this.releaseLag,
  });

  factory ListeningReport.fromJson(Map<String, dynamic> j) => ListeningReport(
        timezone: _str(j['timezone']),
        recordedSince: j['recorded_since'] as String?,
        months: j['months'] is List ? (j['months'] as List).whereType<String>().toList() : const [],
        seconds: _int(j['seconds']),
        activeDays: _int(j['active_days']),
        episodes: _int(j['episodes']),
        podcasts: _int(j['podcasts']),
        finished: _int(j['finished']),
        topPodcasts: _list(j['top_podcasts'], ReportPodcast.fromJson),
        topEpisodes: _list(j['top_episodes'], ReportEpisode.fromJson),
        topCategories: _list(j['top_categories'], NamedSeconds.fromJson),
        topPeople: _list(j['top_people'], ReportPerson.fromJson),
        byHour: _ints(j['by_hour']),
        byDow: _ints(j['by_dow']),
        daily: _list(j['daily'], DayStat.fromJson),
        byMonth: _list(j['by_month'], MonthStat.fromJson),
        biggestDay: j['biggest_day'] is Map<String, dynamic> ? DayStat.fromJson(j['biggest_day']) : null,
        busiestWeek: j['busiest_week'] is Map<String, dynamic> ? DayStat.fromJson(j['busiest_week']) : null,
        longestStreak: _int(j['longest_streak']),
        sessions: j['sessions'] is Map<String, dynamic> ? SessionStats.fromJson(j['sessions']) : const SessionStats(),
        averageSpeed: _double(j['average_speed']),
        timeSaved: j['time_saved'] is Map<String, dynamic> ? TimeSaved.fromJson(j['time_saved']) : const TimeSaved(),
        adHeavy: _list(j['ad_heavy'], ReportPodcast.fromJson),
        byClient: _list(j['by_client'], NamedSeconds.fromJson),
        downloadedSeconds: _int(j['downloaded_seconds']),
        releaseLag: j['release_lag'] is Map<String, dynamic> ? ReleaseLag.fromJson(j['release_lag']) : null,
      );
}

class LongestEpisode {
  final String title;
  final int duration;
  final String podcastName;

  const LongestEpisode(this.title, this.duration, this.podcastName);
}

/// All-time figures, including listening from before the listening record began.
class ExtendedStats {
  final List<ReportPodcast> topPodcasts;
  final List<NamedSeconds> favoriteCategories;
  final int completionRate;
  final LongestEpisode? longestEpisode;
  final String listeningBadge;
  final String totalDownloadedFormatted;
  final int currentStreak;
  final List<int> byDow;
  final List<int> byHour;
  final List<MonthStat> byMonth;
  final List<DayStat> daily;
  final int activeDays;
  final int longestStreak;
  final int yearSeconds;
  final int? peakHour;
  final int backlogEpisodes;
  final int backlogSeconds;
  final int? episodesCompleted;
  final int? episodesListened;
  final String? timezone;
  final TimeSaved? timeSaved;

  const ExtendedStats({
    this.topPodcasts = const [],
    this.favoriteCategories = const [],
    this.completionRate = 0,
    this.longestEpisode,
    this.listeningBadge = '',
    this.totalDownloadedFormatted = '',
    this.currentStreak = 0,
    this.byDow = const [],
    this.byHour = const [],
    this.byMonth = const [],
    this.daily = const [],
    this.activeDays = 0,
    this.longestStreak = 0,
    this.yearSeconds = 0,
    this.peakHour,
    this.backlogEpisodes = 0,
    this.backlogSeconds = 0,
    this.episodesCompleted,
    this.episodesListened,
    this.timezone,
    this.timeSaved,
  });

  factory ExtendedStats.fromJson(Map<String, dynamic> j) {
    final longest = j['longest_episode'];
    return ExtendedStats(
      topPodcasts: _list(
          j['top_podcasts'],
          (p) => ReportPodcast(
                podcastId: p['podcastid'] is num ? _int(p['podcastid']) : null,
                name: _str(p['podcastname']),
                artwork: _str(p['artworkurl']),
                seconds: _int(p['total_seconds']),
              )),
      favoriteCategories: _list(j['favorite_categories'], (c) => NamedSeconds(_str(c['name']), _int(c['total_seconds']))),
      completionRate: _int(j['completion_rate']),
      longestEpisode: longest is Map<String, dynamic>
          ? LongestEpisode(_str(longest['episodetitle']), _int(longest['episodeduration']), _str(longest['podcastname']))
          : null,
      listeningBadge: _str(j['listening_badge']),
      totalDownloadedFormatted: _str(j['total_downloaded_formatted']),
      currentStreak: _int(j['current_streak']),
      byDow: _ints(j['listening_by_dow']),
      byHour: _ints(j['listening_by_hour']),
      byMonth: _list(j['listening_by_month'], MonthStat.fromJson),
      daily: _list(j['daily_listening'], DayStat.fromJson),
      activeDays: _int(j['active_days']),
      longestStreak: _int(j['longest_streak']),
      yearSeconds: _int(j['year_seconds']),
      peakHour: j['peak_hour'] is num ? _int(j['peak_hour']) : null,
      backlogEpisodes: _int(j['backlog_episodes']),
      backlogSeconds: _int(j['backlog_seconds']),
      episodesCompleted: j['episodes_completed'] is num ? _int(j['episodes_completed']) : null,
      episodesListened: j['episodes_listened'] is num ? _int(j['episodes_listened']) : null,
      timezone: j['timezone'] as String?,
      timeSaved: j['time_saved'] is Map<String, dynamic> ? TimeSaved.fromJson(j['time_saved']) : null,
    );
  }
}
