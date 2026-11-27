import 'package:flutter_test/flutter_test.dart';
import 'package:intl/date_symbol_data_local.dart';
import 'package:pinepods_mobile/entities/listening_report.dart';
import 'package:pinepods_mobile/ui/pinepods/rewind_player.dart';

void main() {
  setUpAll(() => initializeDateFormatting('en'));

  test('period labels and listener kinds', () {
    expect(rewindPeriodLabel('2026', 'en'), '2026');
    expect(rewindPeriodLabel('2026-11', 'en'), 'November 2026');
    expect(rewindListenerKind(23).$1, 'night_owl');
    expect(rewindListenerKind(7).$1, 'early_riser');
    expect(rewindChangePct(150, 100), 50);
    expect(rewindChangePct(50, 100), -50);
    expect(rewindChangePct(50, 0), isNull);
  });

  test('a Rewind parses, and tolerates missing fields', () {
    final r = Rewind.fromJson({
      'period': '2026-11',
      'report': {'seconds': 3600, 'top_podcasts': [{'podcast_id': 3, 'name': 'A', 'artwork': '', 'seconds': 3600, 'episodes': 2}]},
      'previous_seconds': 1800,
      'history_until': '2026-11-21',
      'new_podcasts': [{'podcast_id': 3, 'name': 'A', 'artwork': '', 'seconds': 3600, 'episodes': 2}],
      'new_podcast_count': 1,
    });
    expect((r.isYear, r.report.seconds, r.previousSeconds, r.historyUntil, r.firstPlay), (false, 3600, 1800, '2026-11-21', null));
    expect(r.newPodcasts.single.podcastId, 3);
    expect(Rewind.fromJson({'period': '2026'}).isYear, isTrue);
  });
}
