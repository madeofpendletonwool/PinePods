import 'package:flutter_test/flutter_test.dart';
import 'package:pinepods_mobile/entities/listening_report.dart';

void main() {
  test('a full report parses', () {
    final r = ListeningReport.fromJson({
      'timezone': 'America/Chicago',
      'recorded_since': '2026-03-01',
      'months': ['2026-10', '2026-11'],
      'seconds': 7200,
      'active_days': 3,
      'episodes': 4,
      'podcasts': 2,
      'finished': 1,
      'top_podcasts': [
        {'podcast_id': 7, 'name': 'Show', 'artwork': 'a.jpg', 'seconds': 3600, 'episodes': 2}
      ],
      'top_episodes': [
        {'episode_id': 9, 'is_youtube': false, 'title': 'Ep', 'podcast': 'Show', 'artwork': 'a.jpg', 'seconds': 1800}
      ],
      'top_people': [
        {'person_id': 3, 'name': 'Host', 'image': null, 'seconds': 900, 'episodes': 1}
      ],
      'by_hour': List.filled(24, 0),
      'by_dow': [0, 1, 2, 3, 4, 5, 6],
      'daily': [
        {'date': '2026-11-02', 'seconds': 600}
      ],
      'biggest_day': {'date': '2026-11-02', 'seconds': 600},
      'sessions': {'count': 2, 'average_seconds': 300, 'longest_seconds': 400, 'longest_started_at': 1700000000},
      'average_speed': 1.25,
      'time_saved': {'speed': 120, 'silence': 30, 'ads': 60},
      'by_client': [
        {'name': 'android', 'seconds': 7200}
      ],
      'release_lag': {'median_days': 0.5, 'within_day': 0.8, 'within_week': 1.0, 'episodes': 4},
    });
    expect(r.months, ['2026-10', '2026-11']);
    expect(r.topPodcasts.single.podcastId, 7);
    expect(r.topPeople.single.image, isNull);
    expect(r.byDow[6], 6);
    expect(r.daily.single.day, DateTime(2026, 11, 2));
    expect(r.sessions.longestStartedAt, 1700000000);
    expect(r.timeSaved.total, 210);
    expect(r.timeSaved.skipped, 90);
    expect(r.releaseLag!.withinDay, 0.8);
  });

  test('an older server\'s extended stats parse with defaults', () {
    final e = ExtendedStats.fromJson({
      'top_podcasts': [
        {'podcastid': 1, 'podcastname': 'Show', 'artworkurl': 'a.jpg', 'total_seconds': 60}
      ],
      'favorite_categories': [],
      'completion_rate': 40,
      'longest_episode': null,
      'listening_badge': 'night_owl',
      'total_downloaded_bytes': 0,
      'total_downloaded_formatted': '0 B',
      'current_streak': 2,
      'listening_by_dow': [0, 0, 0, 0, 0, 0, 0],
    });
    expect(e.topPodcasts.single.name, 'Show');
    expect(e.byHour, isEmpty);
    expect(e.episodesCompleted, isNull);
    expect(e.timeSaved, isNull);
  });
}
