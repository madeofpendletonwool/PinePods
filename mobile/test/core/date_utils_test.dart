// Regression tests for the pubdate parsing/formatting helpers. The backend
// serializes pubdates as naive UTC strings, and these helpers must
// reinterpret them as UTC (not device-local) before converting - getting
// this wrong silently shifts every displayed date by the device's UTC
// offset, so it's worth pinning down precisely.

import 'package:flutter_test/flutter_test.dart';
import 'package:pinepods_mobile/core/date_utils.dart';

void main() {
  group('parseEpisodePubDateLocal', () {
    test('returns null for an empty string', () {
      expect(parseEpisodePubDateLocal(''), isNull);
    });

    test('returns null for an unparsable string', () {
      expect(parseEpisodePubDateLocal('not a date'), isNull);
    });

    test('reinterprets a naive (no offset) string as UTC before converting to local', () {
      final result = parseEpisodePubDateLocal('2024-01-15T14:30:00');

      final expected = DateTime.utc(2024, 1, 15, 14, 30, 0).toLocal();
      expect(result, expected);
    });

    test('converts a string that already carries an explicit UTC designator', () {
      final result = parseEpisodePubDateLocal('2024-01-15T14:30:00Z');

      final expected = DateTime.utc(2024, 1, 15, 14, 30, 0).toLocal();
      expect(result, expected);
    });

    test('converts a string with an explicit non-UTC offset', () {
      final result = parseEpisodePubDateLocal('2024-01-15T14:30:00+05:00');

      // 14:30 at +05:00 is 09:30 UTC.
      final expected = DateTime.utc(2024, 1, 15, 9, 30, 0).toLocal();
      expect(result, expected);
    });
  });

  group('calendarDaysAgo', () {
    test('is 0 for the same calendar day regardless of time of day', () {
      final morning = DateTime(2024, 7, 10, 1, 0);
      final night = DateTime(2024, 7, 10, 23, 59);

      expect(calendarDaysAgo(morning, now: night), 0);
    });

    test('is 1 for yesterday even if less than 24 wall-clock hours apart', () {
      final lateYesterday = DateTime(2024, 7, 9, 23, 0);
      final earlyToday = DateTime(2024, 7, 10, 1, 0);

      expect(calendarDaysAgo(lateYesterday, now: earlyToday), 1);
    });

    test('counts whole calendar days for dates further in the past', () {
      // Deliberately mid-year (not March/April or October/November) so this
      // doesn't straddle a DST transition in the host's local timezone,
      // which would otherwise make the raw Duration between the two local
      // midnights 23 or 25 hours short/long of a whole number of days.
      final past = DateTime(2024, 7, 1);
      final now = DateTime(2024, 7, 15);

      expect(calendarDaysAgo(past, now: now), 14);
    });

    test('is negative for a date in the future', () {
      final future = DateTime(2024, 7, 20);
      final now = DateTime(2024, 7, 15);

      expect(calendarDaysAgo(future, now: now), -5);
    });
  });

  group('relativePubDateLabel', () {
    test('returns the original string unchanged when unparsable', () {
      expect(relativePubDateLabel('garbage'), 'garbage');
    });

    test('labels today as Today', () {
      final now = DateTime.now();
      final pubdate =
          '${now.year.toString().padLeft(4, '0')}-${now.month.toString().padLeft(2, '0')}-${now.day.toString().padLeft(2, '0')}T00:00:00';

      // Using a naive string means it's reinterpreted as UTC then converted
      // to local, so depending on the host's UTC offset this lands on
      // either the same local calendar day (positive offsets) or the
      // previous one (negative offsets, e.g. the Americas) - both are
      // correct outcomes of the reinterpretation this helper exists to do.
      expect(relativePubDateLabel(pubdate), anyOf('Today', 'Yesterday'));
    });

    test('labels a date within the last week as "N days ago"', () {
      // toUtc().toIso8601String() carries an explicit 'Z', so this round-trips
      // through parseEpisodePubDateLocal losslessly - no reinterpretation
      // ambiguity, unlike the naive-string case above.
      final threeDaysAgo = DateTime.now().subtract(const Duration(days: 3));
      final pubdate = threeDaysAgo.toUtc().toIso8601String();

      expect(relativePubDateLabel(pubdate), '3 days ago');
    });

    test('labels a date a few weeks ago as "N weeks ago"', () {
      final twoWeeksAgo = DateTime.now().subtract(const Duration(days: 15));
      final pubdate = twoWeeksAgo.toUtc().toIso8601String();

      expect(relativePubDateLabel(pubdate), '2 weeks ago');
    });

    test('labels exactly one week ago as singular "1 week ago"', () {
      final oneWeekAgo = DateTime.now().subtract(const Duration(days: 8));
      final pubdate = oneWeekAgo.toUtc().toIso8601String();

      expect(relativePubDateLabel(pubdate), '1 week ago');
    });

    test('labels a date over a month ago as "N months ago"', () {
      final twoMonthsAgo = DateTime.now().subtract(const Duration(days: 65));
      final pubdate = twoMonthsAgo.toUtc().toIso8601String();

      expect(relativePubDateLabel(pubdate), '2 months ago');
    });
  });
}
