// Shared helpers for formatting episode publish dates.
//
// The PinePods backend serializes episode pubdates as *naive UTC* strings such
// as "2024-01-15T14:30:00" (no `Z`/offset). Dart's `DateTime.parse()` treats a
// suffix-less string as device-local, which would silently shift every pubdate
// by the device's UTC offset. These helpers reinterpret such strings as UTC and
// convert them to the device-local timezone before any comparison, and compare
// calendar dates (not rolling 24h windows) so "Today" snaps to local midnight.

/// Parse an episode pubdate string into a device-local [DateTime].
///
/// Strings that already carry timezone information (`Z` or an explicit offset)
/// parse to a UTC-normalized value and are converted to local as-is. Strings
/// without a designator are reinterpreted as UTC (matching the backend) before
/// converting. Returns `null` if the string is empty or cannot be parsed.
DateTime? parseEpisodePubDateLocal(String pubdate) {
  if (pubdate.isEmpty) return null;
  try {
    final parsed = DateTime.parse(pubdate);
    final utc = parsed.isUtc
        ? parsed
        : DateTime.utc(
            parsed.year,
            parsed.month,
            parsed.day,
            parsed.hour,
            parsed.minute,
            parsed.second,
            parsed.millisecond,
            parsed.microsecond,
          );
    return utc.toLocal();
  } catch (_) {
    return null;
  }
}

/// Whole calendar days between [local] and [now] (defaults to `DateTime.now()`).
///
/// Compares date-only values, so 0 means "same calendar day" (today), 1 means
/// "yesterday", regardless of the wall-clock time within each day.
int calendarDaysAgo(DateTime local, {DateTime? now}) {
  final n = now ?? DateTime.now();
  final d0 = DateTime(local.year, local.month, local.day);
  final n0 = DateTime(n.year, n.month, n.day);
  return n0.difference(d0).inDays;
}

/// Long relative label for an episode pubdate string: "Today", "Yesterday",
/// "N days ago", "N weeks ago", or "N months ago".
///
/// Returns the original [pubdate] string unchanged if it cannot be parsed.
String relativePubDateLabel(String pubdate) {
  final local = parseEpisodePubDateLocal(pubdate);
  if (local == null) return pubdate;

  final days = calendarDaysAgo(local);
  if (days <= 0) {
    return 'Today';
  } else if (days == 1) {
    return 'Yesterday';
  } else if (days < 7) {
    return '$days days ago';
  } else if (days < 30) {
    final weeks = (days / 7).floor();
    return weeks == 1 ? '1 week ago' : '$weeks weeks ago';
  } else {
    final months = (days / 30).floor();
    return months == 1 ? '1 month ago' : '$months months ago';
  }
}
