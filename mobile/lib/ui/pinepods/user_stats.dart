// lib/ui/pinepods/user_stats.dart

import 'dart:math' as math;

import 'package:extended_image/extended_image.dart';
import 'package:flutter/material.dart';
import 'package:intl/intl.dart';
import 'package:pinepods_mobile/bloc/settings/settings_bloc.dart';
import 'package:pinepods_mobile/core/environment.dart';
import 'package:pinepods_mobile/entities/listening_report.dart';
import 'package:pinepods_mobile/entities/user_stats.dart';
import 'package:pinepods_mobile/l10n/L.dart';
import 'package:pinepods_mobile/services/logging/app_logger.dart';
import 'package:pinepods_mobile/services/pinepods/pinepods_service.dart';
import 'package:pinepods_mobile/ui/icons/phosphor_icons.dart';
import 'package:pinepods_mobile/ui/pinepods/people/person_page.dart';
import 'package:pinepods_mobile/ui/pinepods/podcast_nav.dart';
import 'package:pinepods_mobile/ui/widgets/platform_progress_indicator.dart';
import 'package:pinepods_mobile/ui/widgets/podcast_image.dart';
import 'package:pinepods_mobile/ui/widgets/soft_ui.dart';
import 'package:provider/provider.dart';
import 'package:url_launcher/url_launcher.dart';

/// Days the all-time activity heatmap covers (matches the server).
const _heatmapDays = 182;

/// The period the page shows: all time, a year, or one month of a year.
class _Period {
  final int? year;
  final int? month;

  const _Period.all()
      : year = null,
        month = null;
  const _Period.year(int this.year) : month = null;
  const _Period.month(int this.year, int this.month);

  bool get isAll => year == null;

  /// First and last day, or null for all time.
  (DateTime, DateTime)? get range {
    if (year == null) return null;
    if (month == null) return (DateTime(year!, 1, 1), DateTime(year!, 12, 31));
    return (DateTime(year!, month!, 1), DateTime(year!, month! + 1, 0));
  }

  @override
  bool operator ==(Object other) => other is _Period && other.year == year && other.month == month;

  @override
  int get hashCode => Object.hash(year, month);
}

String _ymd(DateTime d) => DateFormat('yyyy-MM-dd').format(d); // i18n-ignore

DateTime _dateOnly(DateTime d) => DateTime(d.year, d.month, d.day);

class PinepodsUserStats extends StatefulWidget {
  const PinepodsUserStats({super.key});

  @override
  State<PinepodsUserStats> createState() => _PinepodsUserStatsState();
}

class _PinepodsUserStatsState extends State<PinepodsUserStats> {
  final PinepodsService _pinepodsService = PinepodsService();
  UserStats? _userStats;
  ExtendedStats? _extended;
  String? _pinepodsVersion;
  bool _isLoading = true;
  String? _errorMessage;

  _Period _period = const _Period.all();

  /// The report for [_reportPeriod]; kept on screen while another period loads.
  ListeningReport? _report;
  _Period? _reportPeriod;

  /// Months with listening (`YYYY-MM`), from the first report.
  List<String> _months = const [];

  int? get _userId => Provider.of<SettingsBloc>(context, listen: false).currentSettings.pinepodsUserId;

  @override
  void initState() {
    super.initState();
    final settings = Provider.of<SettingsBloc>(context, listen: false).currentSettings;
    if (settings.pinepodsServer != null && settings.pinepodsApiKey != null) {
      _pinepodsService.setCredentials(settings.pinepodsServer!, settings.pinepodsApiKey!);
    }
    _load();
  }

  Future<void> _load() async {
    final userId = _userId;
    if (userId == null) {
      setState(() {
        _errorMessage = L.of(context)!.not_logged_in;
        _isLoading = false;
      });
      return;
    }

    try {
      final results = await Future.wait([
        _pinepodsService.getUserStats(userId),
        _pinepodsService.getPinepodsVersion(),
        _pinepodsService.getExtendedStats(userId),
        _pinepodsService.getListeningReport(userId),
      ]);
      if (!mounted) return;
      final report = results[3] as ListeningReport?;
      setState(() {
        _userStats = results[0] as UserStats;
        _pinepodsVersion = results[1] as String;
        _extended = results[2] as ExtendedStats?;
        _report = report;
        _reportPeriod = const _Period.all();
        _months = report?.months ?? const [];
        _isLoading = false;
      });
    } catch (e) {
      if (!mounted) return;
      setState(() {
        _errorMessage = L.of(context)!.failed_to_load_stats(e.toString());
        _isLoading = false;
      });
    }
  }

  Future<void> _selectPeriod(_Period period) async {
    if (period == _period) return;
    setState(() => _period = period);
    final userId = _userId;
    if (userId == null) return;
    final range = period.range;
    final report = await _pinepodsService.getListeningReport(
      userId,
      start: range == null ? null : _ymd(range.$1),
      end: range == null ? null : _ymd(range.$2),
    );
    if (!mounted || _period != period || report == null) return;
    setState(() {
      _report = report;
      _reportPeriod = period;
      if (report.months.isNotEmpty) _months = report.months;
    });
  }

  ListeningReport? get _shownReport => _reportPeriod == _period ? _report : null;

  // ---- formatting ----

  String get _locale => Localizations.localeOf(context).toString();

  /// "3h 12m", "12m", or "0m".
  String _fmtSecs(int secs) {
    final h = secs ~/ 3600, m = (secs % 3600) ~/ 60;
    return h > 0 ? L.of(context)!.stats_hm('$h', '$m') : L.of(context)!.stats_m('$m');
  }

  /// Large totals as whole hours ("1,234h"), smaller ones as "3h 12m".
  String _fmtTotal(int secs) => secs >= 100 * 3600 ? L.of(context)!.stats_h(_group(secs ~/ 3600)) : _fmtSecs(secs);

  String _group(int n) => NumberFormat.decimalPattern(_locale).format(n);

  String _hourLabel(int h) => DateFormat.j(_locale).format(DateTime(2000, 1, 1, h));

  String _monthShort(int m) => DateFormat.MMM(_locale).format(DateTime(2000, m));

  String _fmtDay(DateTime d) => DateFormat.yMMMd(_locale).format(d);

  /// Sunday first. 2 January 2000 was a Sunday.
  List<String> get _dowLabels => [for (var i = 0; i < 7; i++) DateFormat.E(_locale).format(DateTime(2000, 1, 2 + i))];

  List<String> get _hourLabels => [for (var h = 0; h < 24; h++) h % 6 == 0 ? _hourLabel(h) : ''];

  String _periodTitle(_Period p) {
    if (p.isAll) return L.of(context)!.stats_period_all;
    if (p.month == null) return '${p.year}';
    return DateFormat.yMMMM(_locale).format(DateTime(p.year!, p.month!));
  }

  int? _peak(List<int> values) {
    int? best;
    for (var i = 0; i < values.length; i++) {
      if (values[i] > 0 && (best == null || values[i] > values[best])) best = i;
    }
    return best;
  }

  // ---- building blocks ----

  Widget _panel({required String title, String? note, required Widget child}) {
    final theme = Theme.of(context);
    return Container(
      width: double.infinity,
      margin: const EdgeInsets.only(bottom: 12),
      padding: const EdgeInsets.fromLTRB(16, 14, 16, 16),
      decoration: BoxDecoration(color: softFill(theme), borderRadius: BorderRadius.circular(18)),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          Text(title, style: theme.textTheme.titleMedium?.copyWith(fontWeight: FontWeight.w700)),
          if (note != null) ...[
            const SizedBox(height: 2),
            Text(note, style: theme.textTheme.bodySmall?.copyWith(color: theme.hintColor)),
          ],
          const SizedBox(height: 12),
          child,
        ],
      ),
    );
  }

  Widget _tile(IconData icon, String value, String label, {String? sub}) {
    final theme = Theme.of(context);
    return Container(
      padding: const EdgeInsets.all(14),
      decoration: BoxDecoration(color: softFill(theme), borderRadius: BorderRadius.circular(16)),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.start,
        mainAxisSize: MainAxisSize.min,
        children: [
          Icon(icon, size: 20, color: theme.colorScheme.primary),
          const SizedBox(height: 8),
          Text(value,
              style: theme.textTheme.titleLarge?.copyWith(fontWeight: FontWeight.w800),
              maxLines: 1,
              overflow: TextOverflow.ellipsis),
          Text(label, style: theme.textTheme.bodySmall?.copyWith(color: theme.hintColor), maxLines: 2),
          if (sub != null) ...[
            const SizedBox(height: 2),
            Text(sub,
                style: TextStyle(fontSize: 11, color: theme.hintColor), maxLines: 2, overflow: TextOverflow.ellipsis),
          ],
        ],
      ),
    );
  }

  Widget _tiles(List<Widget> tiles) {
    return LayoutBuilder(builder: (context, constraints) {
      final cols = constraints.maxWidth > 900 ? 4 : (constraints.maxWidth > 560 ? 3 : 2);
      const gap = 10.0;
      final w = (constraints.maxWidth - gap * (cols - 1)) / cols;
      return Padding(
        padding: const EdgeInsets.only(bottom: 12),
        child: Wrap(
          spacing: gap,
          runSpacing: gap,
          children: [for (final t in tiles) SizedBox(width: w, child: t)],
        ),
      );
    });
  }

  Widget? _savedTile(TimeSaved? saved) {
    if (saved == null || saved.total < 60) return null;
    return _tile(
      PhosphorIconsRegular.lightning,
      _fmtTotal(saved.total),
      L.of(context)!.stats_time_saved,
      sub: L.of(context)!.stats_time_saved_detail(_fmtSecs(math.max(saved.speed, 0)), _fmtSecs(saved.skipped)),
    );
  }

  Color _level(int secs, int max) {
    final primary = Theme.of(context).colorScheme.primary;
    if (secs <= 0) return softNeutral(Theme.of(context)).withValues(alpha: 0.08);
    final r = secs / math.max(max, 1);
    final opacity = r > 0.75 ? 1.0 : (r > 0.45 ? 0.75 : (r > 0.2 ? 0.5 : 0.28));
    return primary.withValues(alpha: opacity);
  }

  Widget _bars(List<int> values, List<String> labels, {int? highlight, double height = 110}) {
    final theme = Theme.of(context);
    final max = values.fold<int>(0, math.max);
    return SizedBox(
      height: height,
      child: Row(
        crossAxisAlignment: CrossAxisAlignment.stretch,
        children: [
          for (var i = 0; i < values.length; i++)
            Expanded(
              child: Tooltip(
                message: '${labels.elementAtOrNull(i) ?? ''} ${_fmtSecs(values[i])}'.trim(),
                child: Padding(
                  padding: EdgeInsets.symmetric(horizontal: values.length > 12 ? 1 : 3),
                  child: Column(
                    children: [
                      Expanded(
                        child: Align(
                          alignment: Alignment.bottomCenter,
                          child: FractionallySizedBox(
                            heightFactor: values[i] <= 0 ? 0.02 : math.max(values[i] / math.max(max, 1), 0.04),
                            child: Container(
                              decoration: BoxDecoration(
                                color: values[i] <= 0
                                    ? softNeutral(theme).withValues(alpha: 0.1)
                                    : theme.colorScheme.primary.withValues(alpha: i == highlight ? 1 : 0.45),
                                borderRadius: BorderRadius.circular(4),
                              ),
                            ),
                          ),
                        ),
                      ),
                      const SizedBox(height: 4),
                      SizedBox(
                        height: 14,
                        child: OverflowBox(
                          maxWidth: 60,
                          child: Text(labels.elementAtOrNull(i) ?? '',
                              style: TextStyle(fontSize: 10, color: theme.hintColor), maxLines: 1, softWrap: false),
                        ),
                      ),
                    ],
                  ),
                ),
              ),
            ),
        ],
      ),
    );
  }

  /// GitHub-style grid of [first]..[last]: one column per week, Sunday at the top.
  Widget _heatmap(List<DayStat> days, DateTime first, DateTime last) {
    final theme = Theme.of(context);
    final byDate = <DateTime, int>{for (final d in days) if (d.day != null) _dateOnly(d.day!): d.seconds};
    final max = byDate.values.fold<int>(0, math.max);
    final start = first.subtract(Duration(days: first.weekday % 7));
    final weeks = last.difference(start).inDays ~/ 7 + 1;

    return LayoutBuilder(builder: (context, constraints) {
      const gap = 3.0;
      final cell = math.min(16.0, (constraints.maxWidth - gap * (weeks - 1)) / weeks);
      final grid = Row(
        mainAxisSize: MainAxisSize.min,
        children: [
          for (var w = 0; w < weeks; w++)
            Padding(
              padding: EdgeInsets.only(right: w == weeks - 1 ? 0 : gap),
              child: Column(
                children: [
                  for (var d = 0; d < 7; d++)
                    Builder(builder: (_) {
                      final date = DateTime(start.year, start.month, start.day + w * 7 + d);
                      final out = date.isBefore(first) || date.isAfter(last);
                      final secs = byDate[date] ?? 0;
                      final box = Container(
                        width: cell,
                        height: cell,
                        margin: EdgeInsets.only(bottom: d == 6 ? 0 : gap),
                        decoration: BoxDecoration(
                          color: out ? Colors.transparent : _level(secs, max),
                          borderRadius: BorderRadius.circular(cell / 4),
                        ),
                      );
                      return out ? box : Tooltip(message: '${_fmtDay(date)}: ${_fmtSecs(secs)}', child: box);
                    }),
                ],
              ),
            ),
        ],
      );
      return Column(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          // A year is too wide for a phone at a readable size: scroll it, newest in view.
          SingleChildScrollView(scrollDirection: Axis.horizontal, reverse: true, child: grid),
          const SizedBox(height: 8),
          Row(
            mainAxisAlignment: MainAxisAlignment.end,
            children: [
              Text(L.of(context)!.stats_less, style: TextStyle(fontSize: 11, color: theme.hintColor)),
              const SizedBox(width: 4),
              for (final s in [0, 1, 3, 6, 10])
                Container(
                  width: 10,
                  height: 10,
                  margin: const EdgeInsets.symmetric(horizontal: 1.5),
                  decoration: BoxDecoration(color: _level(s, 10), borderRadius: BorderRadius.circular(2.5)),
                ),
              const SizedBox(width: 4),
              Text(L.of(context)!.stats_more, style: TextStyle(fontSize: 11, color: theme.hintColor)),
            ],
          ),
        ],
      );
    });
  }

  /// One month as a calendar, Sunday first, each day shaded by how much was played.
  Widget _monthCalendar(List<DayStat> days, DateTime first, DateTime last) {
    final theme = Theme.of(context);
    final byDate = <DateTime, int>{for (final d in days) if (d.day != null) _dateOnly(d.day!): d.seconds};
    final max = byDate.values.fold<int>(0, math.max);
    final lead = first.weekday % 7;
    final count = DateTime(first.year, first.month + 1, 0).day;
    final cells = <Widget>[
      for (final d in _dowLabels)
        Center(child: Text(d, style: TextStyle(fontSize: 11, color: theme.hintColor))),
      for (var i = 0; i < lead; i++) const SizedBox.shrink(),
      for (var i = 0; i < count; i++)
        Builder(builder: (_) {
          final date = DateTime(first.year, first.month, i + 1);
          final secs = byDate[date] ?? 0;
          final future = date.isAfter(last);
          final level = _level(secs, max);
          final strong = secs > 0 && secs / math.max(max, 1) > 0.45;
          return Tooltip(
            message: _fmtSecs(secs),
            child: Container(
              alignment: Alignment.center,
              decoration: BoxDecoration(
                color: future ? Colors.transparent : level,
                borderRadius: BorderRadius.circular(8),
              ),
              child: Text(
                '${i + 1}',
                style: TextStyle(
                  fontSize: 12,
                  fontWeight: FontWeight.w600,
                  color: future
                      ? theme.hintColor.withValues(alpha: 0.4)
                      : (strong ? theme.colorScheme.onPrimary : theme.colorScheme.onSurface),
                ),
              ),
            ),
          );
        }),
    ];
    return GridView.count(
      crossAxisCount: 7,
      shrinkWrap: true,
      physics: const NeverScrollableScrollPhysics(),
      mainAxisSpacing: 4,
      crossAxisSpacing: 4,
      children: cells,
    );
  }

  /// A ranked list with a meter under each row.
  Widget _ranked(List<_Ranked> rows) {
    final theme = Theme.of(context);
    final max = rows.fold<int>(0, (m, r) => math.max(m, r.seconds));
    return Column(
      children: [
        for (var i = 0; i < rows.length; i++)
          InkWell(
            onTap: rows[i].onTap,
            borderRadius: BorderRadius.circular(10),
            child: Padding(
              padding: const EdgeInsets.symmetric(vertical: 6),
              child: Row(
                children: [
                  SizedBox(
                    width: 22,
                    child: Text('${i + 1}',
                        style: theme.textTheme.bodySmall?.copyWith(color: theme.hintColor, fontWeight: FontWeight.w700)),
                  ),
                  if (rows[i].person)
                    _personAvatar(rows[i])
                  else if (rows[i].image != null && rows[i].image!.isNotEmpty)
                    ClipRRect(
                      borderRadius: BorderRadius.circular(8),
                      child: PodcastImage(url: rows[i].image!, width: 40, height: 40, fit: BoxFit.cover),
                    ),
                  const SizedBox(width: 10),
                  Expanded(
                    child: Column(
                      crossAxisAlignment: CrossAxisAlignment.start,
                      children: [
                        Row(
                          children: [
                            Expanded(
                              child: Text(rows[i].name,
                                  maxLines: 1,
                                  overflow: TextOverflow.ellipsis,
                                  style: theme.textTheme.bodyMedium?.copyWith(fontWeight: FontWeight.w600)),
                            ),
                            const SizedBox(width: 8),
                            Text(_fmtSecs(rows[i].seconds),
                                style: theme.textTheme.bodySmall?.copyWith(color: theme.hintColor)),
                          ],
                        ),
                        if (rows[i].sub != null)
                          Text(rows[i].sub!,
                              maxLines: 1,
                              overflow: TextOverflow.ellipsis,
                              style: theme.textTheme.bodySmall?.copyWith(color: theme.hintColor)),
                        const SizedBox(height: 4),
                        _meter(rows[i].seconds / math.max(max, 1)),
                      ],
                    ),
                  ),
                ],
              ),
            ),
          ),
      ],
    );
  }

  Widget _personAvatar(_Ranked row) {
    final theme = Theme.of(context);
    return CircleAvatar(
      radius: 20,
      backgroundColor: theme.colorScheme.primary.withValues(alpha: 0.15),
      foregroundImage: row.image == null ? null : ExtendedNetworkImageProvider(row.image!, cache: true),
      child: Text(
        row.name.isEmpty ? '' : row.name.characters.first.toUpperCase(),
        style: TextStyle(color: theme.colorScheme.primary, fontWeight: FontWeight.w700),
      ),
    );
  }

  Widget _meter(double fraction) {
    final theme = Theme.of(context);
    return ClipRRect(
      borderRadius: BorderRadius.circular(3),
      child: LinearProgressIndicator(
        value: fraction.clamp(0.02, 1.0),
        minHeight: 5,
        backgroundColor: softNeutral(theme).withValues(alpha: 0.1),
        valueColor: AlwaysStoppedAnimation(theme.colorScheme.primary),
      ),
    );
  }

  /// Shares of the total, as a list of meters.
  Widget _shares(List<NamedSeconds> rows) {
    final theme = Theme.of(context);
    final total = math.max(rows.fold<int>(0, (s, r) => s + r.seconds), 1);
    return Column(
      children: [
        for (final r in rows)
          Padding(
            padding: const EdgeInsets.symmetric(vertical: 6),
            child: Column(
              crossAxisAlignment: CrossAxisAlignment.start,
              children: [
                Row(
                  children: [
                    Expanded(
                      child: Text(r.name,
                          maxLines: 1,
                          overflow: TextOverflow.ellipsis,
                          style: theme.textTheme.bodyMedium?.copyWith(fontWeight: FontWeight.w600)),
                    ),
                    Text('${(r.seconds / total * 100).round()}% · ${_fmtSecs(r.seconds)}',
                        style: theme.textTheme.bodySmall?.copyWith(color: theme.hintColor)),
                  ],
                ),
                const SizedBox(height: 4),
                _meter(r.seconds / total),
              ],
            ),
          ),
      ],
    );
  }

  Widget _fact(IconData icon, String label, String value, String? sub) {
    final theme = Theme.of(context);
    return Padding(
      padding: const EdgeInsets.symmetric(vertical: 7),
      child: Row(
        children: [
          Container(
            width: 36,
            height: 36,
            decoration: BoxDecoration(
              color: theme.colorScheme.primary.withValues(alpha: 0.12),
              borderRadius: BorderRadius.circular(10),
            ),
            child: Icon(icon, size: 18, color: theme.colorScheme.primary),
          ),
          const SizedBox(width: 12),
          Expanded(
            child: Column(
              crossAxisAlignment: CrossAxisAlignment.start,
              children: [
                Text(label, style: theme.textTheme.bodySmall?.copyWith(color: theme.hintColor)),
                if (sub != null)
                  Text(sub, style: theme.textTheme.bodySmall?.copyWith(color: theme.hintColor, fontSize: 11)),
              ],
            ),
          ),
          Text(value, style: theme.textTheme.titleSmall?.copyWith(fontWeight: FontWeight.w700)),
        ],
      ),
    );
  }

  String _episodesLabel(int n) => L.of(context)!.episodes_count(n);

  void _openPodcast(int? id, String name, String artwork) {
    if (id == null) return;
    navigateToPodcastById(context, id, fallbackTitle: name, fallbackArtwork: artwork);
  }

  List<_Ranked> _podcastRows(List<ReportPodcast> pods, {bool withEpisodes = true}) => [
        for (final p in pods)
          _Ranked(
            name: p.name,
            sub: withEpisodes && p.episodes > 0 ? _episodesLabel(p.episodes) : null,
            image: p.artwork,
            seconds: p.seconds,
            onTap: p.podcastId == null ? null : () => _openPodcast(p.podcastId, p.name, p.artwork),
          ),
      ];

  // ---- sections ----

  Widget _hero() {
    final theme = Theme.of(context);
    final stats = _userStats!;
    final username = Provider.of<SettingsBloc>(context, listen: false).currentSettings.pinepodsUsername ?? '';
    final created = DateTime.tryParse(stats.userCreated);
    final badge = switch (_extended?.listeningBadge) {
      'night_owl' => (L.of(context)!.stats_badge_night_owl, PhosphorIconsRegular.moon),
      'evening_listener' => (L.of(context)!.stats_badge_evening_listener, PhosphorIconsRegular.sunHorizon),
      'afternoon_listener' => (L.of(context)!.stats_badge_afternoon_listener, PhosphorIconsRegular.sun),
      'morning_listener' => (L.of(context)!.stats_badge_morning_listener, PhosphorIconsRegular.sunDim),
      _ => null,
    };
    final primary = theme.colorScheme.primary;

    return Container(
      margin: const EdgeInsets.only(bottom: 16),
      padding: const EdgeInsets.all(18),
      decoration: BoxDecoration(
        borderRadius: BorderRadius.circular(22),
        gradient: LinearGradient(
          begin: Alignment.topLeft,
          end: Alignment.bottomRight,
          colors: [primary.withValues(alpha: 0.22), primary.withValues(alpha: 0.06)],
        ),
      ),
      child: Row(
        children: [
          Expanded(
            child: Column(
              crossAxisAlignment: CrossAxisAlignment.start,
              children: [
                if (username.isNotEmpty)
                  Text(username,
                      style: theme.textTheme.titleLarge?.copyWith(fontWeight: FontWeight.w800),
                      maxLines: 1,
                      overflow: TextOverflow.ellipsis),
                if (created != null)
                  Text(L.of(context)!.stats_listening_since(_fmtDay(created)),
                      style: theme.textTheme.bodySmall?.copyWith(color: theme.hintColor)),
                if (badge != null) ...[
                  const SizedBox(height: 8),
                  Container(
                    padding: const EdgeInsets.symmetric(horizontal: 10, vertical: 4),
                    decoration: BoxDecoration(color: primary.withValues(alpha: 0.16), borderRadius: BorderRadius.circular(20)),
                    child: Row(
                      mainAxisSize: MainAxisSize.min,
                      children: [
                        Icon(badge.$2, size: 14, color: primary),
                        const SizedBox(width: 5),
                        Text(badge.$1, style: TextStyle(fontSize: 12, fontWeight: FontWeight.w600, color: primary)),
                      ],
                    ),
                  ),
                ],
              ],
            ),
          ),
          const SizedBox(width: 12),
          Column(
            crossAxisAlignment: CrossAxisAlignment.end,
            children: [
              Text(_fmtTotal(stats.timeListened * 60),
                  style: theme.textTheme.headlineSmall?.copyWith(fontWeight: FontWeight.w800, color: primary)),
              Text(L.of(context)!.stat_time_listened, style: theme.textTheme.bodySmall?.copyWith(color: theme.hintColor)),
            ],
          ),
        ],
      ),
    );
  }

  Widget _periodPicker() {
    final parsed = <(int, int)>[];
    for (final m in _months) {
      final parts = m.split('-');
      final y = int.tryParse(parts.first), mo = int.tryParse(parts.elementAtOrNull(1) ?? '');
      if (y != null && mo != null) parsed.add((y, mo));
    }
    final years = parsed.map((p) => p.$1).toSet().toList()..sort((a, b) => b.compareTo(a));
    final year = _period.year;
    final months = year == null ? <int>[] : (parsed.where((p) => p.$1 == year).map((p) => p.$2).toSet().toList()..sort());

    Widget row(List<Widget> chips) => SizedBox(
          height: 40,
          child: ListView.separated(
            scrollDirection: Axis.horizontal,
            itemCount: chips.length,
            separatorBuilder: (_, _) => const SizedBox(width: 8),
            itemBuilder: (_, i) => chips[i],
          ),
        );

    return Padding(
      padding: const EdgeInsets.only(bottom: 12),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          row([
            FilterPill(
              label: L.of(context)!.stats_period_all,
              selected: _period.isAll,
              onTap: () => _selectPeriod(const _Period.all()),
            ),
            for (final y in years)
              FilterPill(label: '$y', selected: year == y, onTap: () => _selectPeriod(_Period.year(y))),
          ]),
          if (year != null) ...[
            const SizedBox(height: 6),
            row([
              FilterPill(
                label: L.of(context)!.stats_period_whole_year,
                selected: _period.month == null,
                onTap: () => _selectPeriod(_Period.year(year)),
              ),
              for (final m in months)
                FilterPill(
                  label: _monthShort(m),
                  selected: _period.month == m,
                  onTap: () => _selectPeriod(_Period.month(year, m)),
                ),
            ]),
          ],
        ],
      ),
    );
  }

  /// All time: the calendar figures from the extended stats (which reach back before the
  /// listening record), the rest from the report when the server has one.
  List<Widget> _allTime(ExtendedStats ext, ListeningReport? report) {
    final l = L.of(context)!;
    final today = _dateOnly(DateTime.now());
    final hasActivity = ext.byHour.any((v) => v > 0);
    final monthValues = ext.byMonth.map((m) => m.seconds).toList();
    final monthLabels = ext.byMonth.map((m) => m.monthNumber == null ? '' : _monthShort(m.monthNumber!)).toList();
    final podcasts = report != null && report.topPodcasts.isNotEmpty
        ? _podcastRows(report.topPodcasts)
        : _podcastRows(ext.topPodcasts, withEpisodes: false);
    final cats = report != null && report.topCategories.isNotEmpty
        ? report.topCategories.take(6).toList()
        : ext.favoriteCategories;
    final saved = _savedTile(report?.timeSaved ?? ext.timeSaved);

    return [
      _tiles([
        _tile(PhosphorIconsRegular.calendarCheck, _fmtTotal(ext.yearSeconds), l.stats_this_year('${today.year}')),
        if (ext.episodesCompleted != null)
          _tile(PhosphorIconsRegular.checkCircle, _group(ext.episodesCompleted!), l.stats_episodes_finished,
              sub: l.stats_completion_of('${ext.completionRate}')),
        _tile(PhosphorIconsRegular.flame, '${ext.currentStreak}', l.stats_current_streak,
            sub: ext.longestStreak > 0 ? l.stats_longest_streak('${ext.longestStreak}') : null),
        _tile(PhosphorIconsRegular.calendarDots, _group(ext.activeDays), l.stats_active_days),
        if (ext.episodesListened != null)
          _tile(PhosphorIconsRegular.headphones, _group(ext.episodesListened!), l.stats_episodes_played),
        _tile(PhosphorIconsRegular.hourglassMedium, _fmtTotal(ext.backlogSeconds), l.stats_backlog,
            sub: l.stats_backlog_episodes(_group(ext.backlogEpisodes))),
        ?saved,
      ]),
      if (hasActivity) ...[
        _panel(
          title: l.stats_activity,
          note: l.stats_activity_note,
          child: _heatmap(ext.daily, today.subtract(const Duration(days: _heatmapDays - 1)), today),
        ),
        _panel(
          title: l.stats_by_hour,
          note: ext.peakHour == null ? null : l.stats_peak_hour(_hourLabel(ext.peakHour!)),
          child: _bars(ext.byHour, _hourLabels, highlight: ext.peakHour),
        ),
        _panel(title: l.stats_by_dow, child: _bars(ext.byDow, _dowLabels, highlight: _peak(ext.byDow))),
        if (ext.byMonth.isNotEmpty)
          _panel(title: l.stats_by_month, child: _bars(monthValues, monthLabels, highlight: _peak(monthValues))),
      ],
      if (podcasts.isNotEmpty) _panel(title: l.stats_top_podcasts, child: _ranked(podcasts)),
      if (cats.isNotEmpty) _panel(title: l.stats_categories, child: _shares(cats)),
      if (ext.longestEpisode != null)
        _panel(
          title: l.stats_longest_episode,
          child: Row(
            children: [
              Icon(PhosphorIconsRegular.trophy, color: Theme.of(context).colorScheme.primary),
              const SizedBox(width: 12),
              Expanded(
                child: Column(
                  crossAxisAlignment: CrossAxisAlignment.start,
                  children: [
                    Text(ext.longestEpisode!.title,
                        style: Theme.of(context).textTheme.bodyMedium?.copyWith(fontWeight: FontWeight.w600)),
                    Text('${ext.longestEpisode!.podcastName} · ${_fmtSecs(ext.longestEpisode!.duration)}',
                        style: Theme.of(context).textTheme.bodySmall?.copyWith(color: Theme.of(context).hintColor)),
                  ],
                ),
              ),
            ],
          ),
        ),
    ];
  }

  /// A year or a month, all from the report.
  List<Widget> _periodView(ListeningReport r, _Period period) {
    final l = L.of(context)!;
    final theme = Theme.of(context);
    final range = period.range!;
    final first = range.$1;
    final today = _dateOnly(DateTime.now());
    final last = range.$2.isAfter(today) ? today : range.$2;
    if (r.seconds <= 0) {
      return [
        Padding(
          padding: const EdgeInsets.symmetric(vertical: 32),
          child: Center(child: Text(l.stats_period_empty, style: TextStyle(color: theme.hintColor))),
        ),
      ];
    }
    final since = r.recordedSince == null ? null : DateTime.tryParse(r.recordedSince!);
    final monthValues = r.byMonth.map((m) => m.seconds).toList();
    final monthLabels = r.byMonth.map((m) => m.monthNumber == null ? '' : _monthShort(m.monthNumber!)).toList();
    final hourPeak = _peak(r.byHour);
    final saved = _savedTile(r.timeSaved);

    return [
      if (since != null && since.isAfter(first))
        Padding(
          padding: const EdgeInsets.only(bottom: 12),
          child: Row(
            crossAxisAlignment: CrossAxisAlignment.start,
            children: [
              Icon(PhosphorIconsRegular.info, size: 16, color: theme.hintColor),
              const SizedBox(width: 6),
              Expanded(
                child: Text(l.stats_recorded_since(_fmtDay(since)),
                    style: theme.textTheme.bodySmall?.copyWith(color: theme.hintColor)),
              ),
            ],
          ),
        ),
      _tiles([
        _tile(PhosphorIconsRegular.headphones, _fmtTotal(r.seconds), l.stats_listened),
        _tile(PhosphorIconsRegular.calendarDots, _group(r.activeDays), l.stats_active_days,
            sub: r.longestStreak > 1 ? l.stats_longest_streak('${r.longestStreak}') : null),
        _tile(PhosphorIconsRegular.playCircle, _group(r.episodes), l.stats_episodes_played,
            sub: l.stats_podcasts_count(_group(r.podcasts))),
        _tile(PhosphorIconsRegular.checkCircle, _group(r.finished), l.stats_episodes_finished),
        ?saved,
      ]),
      if (!last.isBefore(first))
        _panel(
          title: l.stats_activity,
          child: period.month != null ? _monthCalendar(r.daily, first, last) : _heatmap(r.daily, first, last),
        ),
      _panel(
        title: l.stats_by_hour,
        note: hourPeak == null ? null : l.stats_peak_hour(_hourLabel(hourPeak)),
        child: _bars(r.byHour, _hourLabels, highlight: hourPeak),
      ),
      _panel(title: l.stats_by_dow, child: _bars(r.byDow, _dowLabels, highlight: _peak(r.byDow))),
      if (r.byMonth.isNotEmpty && period.month == null)
        _panel(
          title: l.stats_by_month_of('${first.year}'),
          child: _bars(monthValues, monthLabels, highlight: _peak(monthValues)),
        ),
      if (r.topPodcasts.isNotEmpty) _panel(title: l.stats_top_podcasts, child: _ranked(_podcastRows(r.topPodcasts))),
      if (r.topCategories.isNotEmpty) _panel(title: l.stats_categories, child: _shares(r.topCategories.take(6).toList())),
    ];
  }

  String _clientName(String name) => switch (name) {
        'web' => L.of(context)!.stats_client_web,
        'desktop' => L.of(context)!.stats_client_desktop,
        'android' => 'Android', // i18n-ignore
        'ios' => 'iOS', // i18n-ignore
        'sync' => L.of(context)!.stats_client_sync,
        _ => L.of(context)!.stats_client_other,
      };

  /// Sections only the report has, for any period: highlights, episodes, people, apps, ads.
  List<Widget> _reportSections(ListeningReport r) {
    if (r.seconds <= 0) return const [];
    final l = L.of(context)!;
    final facts = <Widget>[
      if (r.biggestDay != null)
        _fact(PhosphorIconsRegular.trophy, l.stats_biggest_day, _fmtSecs(r.biggestDay!.seconds),
            r.biggestDay!.day == null ? null : _fmtDay(r.biggestDay!.day!)),
      if (r.busiestWeek != null)
        _fact(PhosphorIconsRegular.calendarStar, l.stats_busiest_week, _fmtSecs(r.busiestWeek!.seconds),
            r.busiestWeek!.day == null ? null : l.stats_week_of(_fmtDay(r.busiestWeek!.day!))),
      if (r.sessions.count > 0) ...[
        _fact(
          PhosphorIconsRegular.timer,
          l.stats_longest_session,
          _fmtSecs(r.sessions.longestSeconds),
          r.sessions.longestStartedAt == null
              ? null
              : _fmtDay(DateTime.fromMillisecondsSinceEpoch(r.sessions.longestStartedAt! * 1000)),
        ),
        _fact(PhosphorIconsRegular.repeat, l.stats_sessions, _group(r.sessions.count),
            l.stats_average_session(_fmtSecs(r.sessions.averageSeconds))),
      ],
      if (r.averageSpeed != null)
        _fact(PhosphorIconsRegular.gauge, l.stats_average_speed, '${r.averageSpeed!.toStringAsFixed(2)}×', null),
      if (r.downloadedSeconds > 0)
        _fact(PhosphorIconsRegular.downloadSimple, l.stats_played_offline, _fmtSecs(r.downloadedSeconds),
            l.stats_of_listening('${(r.downloadedSeconds / math.max(r.seconds, 1) * 100).round()}')),
      if (r.releaseLag != null)
        _fact(
          PhosphorIconsRegular.rocketLaunch,
          l.stats_release_lag,
          r.releaseLag!.medianDays < 1 ? l.stats_same_day : l.stats_days_after('${r.releaseLag!.medianDays.round()}'),
          l.stats_within_a_day('${(r.releaseLag!.withinDay * 100).round()}'),
        ),
    ];
    final episodes = [
      for (final e in r.topEpisodes) _Ranked(name: e.title, sub: e.podcast, image: e.artwork, seconds: e.seconds),
    ];
    final people = [
      for (final p in r.topPeople)
        _Ranked(
          name: p.name,
          sub: p.episodes > 0 ? _episodesLabel(p.episodes) : null,
          image: p.image,
          person: true,
          seconds: p.seconds,
          onTap: () => Navigator.of(context).push(
            MaterialPageRoute(builder: (_) => PersonPage(personId: p.personId, initialName: p.name)),
          ),
        ),
    ];
    final clients = [for (final c in r.byClient) NamedSeconds(_clientName(c.name), c.seconds)];
    final showClients = clients.length > 1 || (clients.isNotEmpty && clients.first.name != l.stats_client_other);

    return [
      if (facts.isNotEmpty) _panel(title: l.stats_highlights, child: Column(children: facts)),
      if (episodes.isNotEmpty) _panel(title: l.stats_top_episodes, child: _ranked(episodes)),
      if (people.isNotEmpty) _panel(title: l.stats_top_people, note: l.stats_top_people_note, child: _ranked(people)),
      if (showClients) _panel(title: l.stats_by_app, child: _shares(clients)),
      if (r.adHeavy.isNotEmpty)
        _panel(
          title: l.stats_most_ads,
          note: l.stats_most_ads_note,
          child: _ranked(_podcastRows(r.adHeavy, withEpisodes: false)),
        ),
    ];
  }

  Widget _library() {
    final theme = Theme.of(context);
    final l = L.of(context)!;
    final stats = _userStats!;
    final size = _extended?.totalDownloadedFormatted;
    final syncUrls = stats.syncUrls(_pinepodsService.server);

    Widget row(IconData icon, String label, String value, {String? sub}) => Padding(
          padding: const EdgeInsets.symmetric(vertical: 8),
          child: Row(
            children: [
              Icon(icon, size: 20, color: theme.hintColor),
              const SizedBox(width: 12),
              Expanded(
                child: Column(
                  crossAxisAlignment: CrossAxisAlignment.start,
                  children: [
                    Text(label),
                    if (sub != null && sub.isNotEmpty)
                      Text(sub,
                          style: TextStyle(fontSize: 11, color: theme.hintColor),
                          maxLines: 2,
                          overflow: TextOverflow.ellipsis),
                  ],
                ),
              ),
              const SizedBox(width: 8),
              Flexible(
                child: Text(value,
                    textAlign: TextAlign.end, style: const TextStyle(fontWeight: FontWeight.w700), maxLines: 2),
              ),
            ],
          ),
        );

    return _panel(
      title: l.stats_library,
      child: Column(
        children: [
          row(PhosphorIconsRegular.microphoneStage, l.stat_podcasts_added, _group(stats.podcastsAdded)),
          row(PhosphorIconsRegular.playCircle, l.stat_podcasts_played, _group(stats.podcastsPlayed)),
          row(PhosphorIconsRegular.bookmarkSimple, l.stat_episodes_saved, _group(stats.episodesSaved)),
          row(
            PhosphorIconsRegular.downloadSimple,
            l.stat_episodes_downloaded,
            size == null || size.isEmpty || size == '0 B'
                ? _group(stats.episodesDownloaded)
                : '${_group(stats.episodesDownloaded)} · $size',
          ),
          row(
            stats.isSyncing ? PhosphorIconsRegular.arrowsClockwise : PhosphorIconsRegular.prohibit,
            l.sync_status,
            stats.syncStatusDescription,
            sub: syncUrls.join(' · '),
          ),
        ],
      ),
    );
  }

  Future<void> _launchUrl(String url) async {
    final logger = AppLogger();
    try {
      final launched = await launchUrl(Uri.parse(url), mode: LaunchMode.externalApplication);
      if (!launched) throw Exception('No app available to handle this URL');
    } catch (e) {
      logger.error('UserStats', 'Failed to launch URL: $url', e.toString());
      if (mounted) {
        ScaffoldMessenger.of(context).showSnackBar(
          SnackBar(content: Text(L.of(context)!.could_not_open_link(url)), backgroundColor: Colors.red),
        );
      }
    }
  }

  Widget _about() {
    final theme = Theme.of(context);
    final muted = theme.hintColor;

    Widget link(IconData icon, String label, VoidCallback onTap) => ListTile(
          leading: Icon(icon, color: theme.primaryColor),
          title: Text(label),
          trailing: Icon(PhosphorIconsRegular.caretRight, size: 18, color: muted),
          onTap: onTap,
        );

    return Column(
      children: [
        const SizedBox(height: 16),
        Image.asset('assets/images/pinepods-logo.png', width: 64, height: 64),
        const SizedBox(height: 12),
        Text(
          L.of(context)!.app_server_versions(Environment.projectVersion, _pinepodsVersion ?? L.of(context)!.unknown),
          style: theme.textTheme.titleSmall?.copyWith(fontWeight: FontWeight.w600),
          textAlign: TextAlign.center,
        ),
        const SizedBox(height: 10),
        Text(
          L.of(context)!.about_thanks,
          style: theme.textTheme.bodyMedium?.copyWith(color: muted, height: 1.4),
          textAlign: TextAlign.center,
        ),
        const SizedBox(height: 16),
        Material(
          color: softFill(theme),
          borderRadius: BorderRadius.circular(16),
          clipBehavior: Clip.antiAlias,
          child: Column(
            children: [
              link(PhosphorIconsRegular.fileText, L.of(context)!.pinepods_documentation,
                  () => _launchUrl('https://pinepods.online')),
              link(PhosphorIconsRegular.code, L.of(context)!.pinepods_on_github,
                  () => _launchUrl('https://github.com/madeofpendletonwool/pinepods')),
              link(PhosphorIconsRegular.coffee, L.of(context)!.buy_me_a_coffee,
                  () => _launchUrl('https://www.buymeacoffee.com/collinscoffee')),
              link(PhosphorIconsRegular.article, L.of(context)!.open_source_licenses,
                  () => showLicensePage(context: context)),
            ],
          ),
        ),
        const SizedBox(height: 16),
        Text(
          'Copyright © 2025 Gooseberry Development', // i18n-ignore
          style: TextStyle(fontSize: 12, color: muted, fontWeight: FontWeight.w500),
          textAlign: TextAlign.center,
        ),
        const SizedBox(height: 6),
        Text(
          'The PinePods Mobile App is an open-source podcast player adapted from the Anytime Podcast Player (© 2020 Ben Hills). Portions of this application retain the original BSD 3-Clause license.', // i18n-ignore
          style: TextStyle(fontSize: 12, color: muted, height: 1.3),
          textAlign: TextAlign.center,
        ),
        TextButton(
          onPressed: () => _launchUrl('https://github.com/amugofjava/anytime_podcast_player'),
          child: Text(L.of(context)!.view_original_project, style: const TextStyle(fontSize: 12)),
        ),
      ],
    );
  }

  Widget _body() {
    final theme = Theme.of(context);
    final report = _shownReport;
    final children = <Widget>[
      _hero(),
      if (_months.isNotEmpty) _periodPicker(),
      if (_period.isAll) ...[
        if (_extended != null) ..._allTime(_extended!, report),
      ] else ...[
        Padding(
          padding: const EdgeInsets.only(bottom: 10),
          child: Text(_periodTitle(_period), style: theme.textTheme.titleLarge?.copyWith(fontWeight: FontWeight.w800)),
        ),
        if (report != null)
          ..._periodView(report, _period)
        else
          const Padding(padding: EdgeInsets.all(32), child: Center(child: PlatformProgressIndicator())),
      ],
      if (report != null) ..._reportSections(report),
      if (_period.isAll) _library(),
      _about(),
    ];
    return RefreshIndicator(
      onRefresh: () async {
        _period = const _Period.all();
        await _load();
      },
      child: ListView(padding: const EdgeInsets.fromLTRB(16, 8, 16, 32), children: children),
    );
  }

  @override
  Widget build(BuildContext context) {
    return Scaffold(
      appBar: AppBar(title: Text(L.of(context)!.user_statistics), centerTitle: true),
      body: _isLoading
          ? const Center(child: PlatformProgressIndicator())
          : _errorMessage != null
              ? Center(
                  child: SoftEmptyState(
                    icon: PhosphorIconsRegular.warningCircle,
                    title: L.of(context)!.couldnt_load_stats,
                    body: _errorMessage!,
                    action: FilledButton.tonal(
                      onPressed: () {
                        setState(() {
                          _isLoading = true;
                          _errorMessage = null;
                        });
                        _load();
                      },
                      child: Text(L.of(context)!.retry),
                    ),
                  ),
                )
              : _body(),
    );
  }
}

/// One row of a ranked list.
class _Ranked {
  final String name;
  final String? sub;
  final String? image;

  /// A person: round picture, or their initial.
  final bool person;
  final int seconds;
  final VoidCallback? onTap;

  const _Ranked({required this.name, this.sub, this.image, this.person = false, required this.seconds, this.onTap});
}
