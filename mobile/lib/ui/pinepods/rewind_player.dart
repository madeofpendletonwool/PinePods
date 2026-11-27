// lib/ui/pinepods/rewind_player.dart

import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:intl/intl.dart';
import 'package:pinepods_mobile/entities/listening_report.dart';
import 'package:pinepods_mobile/l10n/L.dart';
import 'package:pinepods_mobile/l10n/app_localizations.dart';
import 'package:pinepods_mobile/services/pinepods/pinepods_service.dart';
import 'package:pinepods_mobile/ui/icons/phosphor_icons.dart';
import 'package:pinepods_mobile/ui/widgets/platform_progress_indicator.dart';
import 'package:pinepods_mobile/ui/widgets/podcast_image.dart';

/// How long each slide stays up.
const _slideDuration = Duration(seconds: 7);

/// `YYYY` or `YYYY-MM` as "2026" or "November 2026".
String rewindPeriodLabel(String period, String locale) {
  final parts = period.split('-');
  if (parts.length < 2) return period;
  final y = int.tryParse(parts[0]), m = int.tryParse(parts[1]);
  if (y == null || m == null) return period;
  return DateFormat.yMMMM(locale).format(DateTime(y, m));
}

/// The listener a peak hour makes you: (key, icon).
(String, IconData) rewindListenerKind(int hour) {
  if (hour >= 5 && hour <= 8) return ('early_riser', PhosphorIconsRegular.sunHorizon);
  if (hour >= 9 && hour <= 11) return ('morning', PhosphorIconsRegular.sunDim);
  if (hour >= 12 && hour <= 16) return ('afternoon', PhosphorIconsRegular.sun);
  if (hour >= 17 && hour <= 20) return ('evening', PhosphorIconsRegular.cloudMoon);
  return ('night_owl', PhosphorIconsRegular.moonStars);
}

/// Change from the period before, as a whole percentage.
int? rewindChangePct(int now, int before) => before > 0 && now > 0 ? ((now - before) / before * 100).round() : null;

class _Slide {
  final List<Color> colors;
  final List<Widget> children;

  const _Slide(this.colors, this.children);
}

/// A month's or a year's listening, played as full-screen slides. Tap the right side for the
/// next slide and the left for the previous; hold to pause.
class RewindPlayer extends StatefulWidget {
  /// `YYYY` or `YYYY-MM`.
  final String period;
  final PinepodsService service;
  final int userId;

  const RewindPlayer({super.key, required this.period, required this.service, required this.userId});

  static Future<void> open(BuildContext context, {required String period, required PinepodsService service, required int userId}) {
    return Navigator.of(context, rootNavigator: true).push(PageRouteBuilder(
      opaque: true,
      transitionDuration: const Duration(milliseconds: 250),
      pageBuilder: (_, _, _) => RewindPlayer(period: period, service: service, userId: userId),
      transitionsBuilder: (_, a, _, child) => FadeTransition(opacity: a, child: child),
    ));
  }

  @override
  State<RewindPlayer> createState() => _RewindPlayerState();
}

class _RewindPlayerState extends State<RewindPlayer> with SingleTickerProviderStateMixin {
  late final AnimationController _progress = AnimationController(vsync: this, duration: _slideDuration)
    ..addStatusListener((s) {
      if (s == AnimationStatus.completed) _next();
    });
  Rewind? _rewind;
  bool _loading = true;
  int _index = 0;
  int _count = 1;
  bool _paused = false;

  @override
  void initState() {
    super.initState();
    _load();
  }

  Future<void> _load() async {
    final r = await widget.service.getRewind(widget.userId, widget.period);
    if (!mounted) return;
    setState(() {
      _rewind = r;
      _loading = false;
    });
    _progress.forward(from: 0);
  }

  @override
  void dispose() {
    _progress.dispose();
    super.dispose();
  }

  void _go(int index) {
    if (index < 0 || index >= _count) {
      if (index < 0) _progress.forward(from: 0);
      return;
    }
    setState(() => _index = index);
    _progress.forward(from: 0);
    if (_paused) _progress.stop();
  }

  void _next() => _go(_index + 1);

  void _restart() {
    setState(() => _paused = false);
    _go(0);
  }

  void _togglePause() {
    setState(() => _paused = !_paused);
    _paused ? _progress.stop() : _progress.forward();
  }

  @override
  Widget build(BuildContext context) {
    final l = L.of(context)!;
    final locale = Localizations.localeOf(context).toString();
    final label = rewindPeriodLabel(widget.period, locale);
    final slides = _loading
        ? <_Slide>[const _Slide([Color(0xFF1D0F45), Color(0xFF7B2FF2)], [Center(child: PlatformProgressIndicator())])]
        : _rewind == null
            ? [_Slide(const [Color(0xFF1B2233), Color(0xFF44536E)], [_title(l.rewind_error), const SizedBox(height: 20), _actions(l, error: true)])]
            : _RewindSlides(context, _rewind!, label, locale, _actions(l)).build();
    _count = slides.length;
    final index = _index.clamp(0, slides.length - 1);
    final slide = slides[index];

    return AnnotatedRegion<SystemUiOverlayStyle>(
      value: SystemUiOverlayStyle.light,
      child: Scaffold(
        backgroundColor: Colors.black,
        body: AnimatedContainer(
          duration: const Duration(milliseconds: 600),
          decoration: BoxDecoration(
            gradient: LinearGradient(begin: Alignment.topLeft, end: Alignment.bottomRight, colors: slide.colors),
          ),
          child: SafeArea(
            child: GestureDetector(
              behavior: HitTestBehavior.opaque,
              onTapUp: (d) {
                final width = MediaQuery.of(context).size.width;
                d.localPosition.dx < width / 3 ? _go(index - 1) : _go(index + 1);
              },
              onLongPressStart: (_) => _progress.stop(),
              onLongPressEnd: (_) {
                if (!_paused) _progress.forward();
              },
              child: Column(
                children: [
                  Padding(
                    padding: const EdgeInsets.fromLTRB(12, 10, 12, 0),
                    child: Row(
                      children: [
                        for (var i = 0; i < slides.length; i++)
                          Expanded(
                            child: Padding(
                              padding: const EdgeInsets.symmetric(horizontal: 2),
                              child: ClipRRect(
                                borderRadius: BorderRadius.circular(2),
                                child: SizedBox(
                                  height: 3,
                                  child: i == index
                                      ? AnimatedBuilder(
                                          animation: _progress,
                                          builder: (_, _) => LinearProgressIndicator(
                                            value: _progress.value,
                                            backgroundColor: Colors.white30,
                                            valueColor: const AlwaysStoppedAnimation(Colors.white),
                                          ),
                                        )
                                      : Container(color: i < index ? Colors.white : Colors.white30),
                                ),
                              ),
                            ),
                          ),
                      ],
                    ),
                  ),
                  Padding(
                    padding: const EdgeInsets.fromLTRB(16, 4, 4, 0),
                    child: Row(
                      children: [
                        Expanded(
                          child: Text(l.rewind_title(label),
                              style: const TextStyle(color: Colors.white, fontWeight: FontWeight.w700, fontSize: 13)),
                        ),
                        IconButton(
                          onPressed: _togglePause,
                          tooltip: _paused ? l.play : l.pause,
                          icon: Icon(_paused ? PhosphorIconsRegular.play : PhosphorIconsRegular.pause, color: Colors.white),
                        ),
                        IconButton(
                          onPressed: () => Navigator.of(context).pop(),
                          tooltip: l.close_button_label,
                          icon: const Icon(PhosphorIconsRegular.x, color: Colors.white),
                        ),
                      ],
                    ),
                  ),
                  Expanded(
                    child: AnimatedSwitcher(
                      duration: const Duration(milliseconds: 300),
                      child: Padding(
                        key: ValueKey(index),
                        padding: const EdgeInsets.fromLTRB(28, 8, 28, 32),
                        child: Center(
                          child: SingleChildScrollView(
                            physics: const NeverScrollableScrollPhysics(),
                            child: DefaultTextStyle(
                              style: const TextStyle(color: Colors.white),
                              textAlign: TextAlign.center,
                              child: Column(mainAxisSize: MainAxisSize.min, children: _staggered(slide.children)),
                            ),
                          ),
                        ),
                      ),
                    ),
                  ),
                ],
              ),
            ),
          ),
        ),
      ),
    );
  }

  List<Widget> _staggered(List<Widget> children) {
    var i = 0;
    return [for (final c in children) c is SizedBox ? c : _Rise(delay: Duration(milliseconds: 120 + 160 * i++), child: c)];
  }

  Widget _title(String text) =>
      Text(text, textAlign: TextAlign.center, style: const TextStyle(color: Colors.white, fontSize: 34, fontWeight: FontWeight.w800, height: 1.1));

  Widget _actions(AppLocalizations l, {bool error = false}) {
    return Row(
      mainAxisAlignment: MainAxisAlignment.center,
      children: [
        if (!error) ...[
          TextButton.icon(
            onPressed: _restart,
            style: TextButton.styleFrom(foregroundColor: Colors.white, backgroundColor: Colors.white24, shape: const StadiumBorder()),
            icon: const Icon(PhosphorIconsRegular.arrowCounterClockwise, size: 18),
            label: Text(l.rewind_replay),
          ),
          const SizedBox(width: 10),
        ],
        FilledButton(
          onPressed: () => Navigator.of(context).pop(),
          style: FilledButton.styleFrom(foregroundColor: const Color(0xFF1A1030), backgroundColor: Colors.white),
          child: Text(error ? l.close_button_label : l.rewind_done),
        ),
      ],
    );
  }
}

/// Fades and lifts its child in after [delay].
class _Rise extends StatefulWidget {
  final Duration delay;
  final Widget child;

  const _Rise({required this.delay, required this.child});

  @override
  State<_Rise> createState() => _RiseState();
}

class _RiseState extends State<_Rise> with SingleTickerProviderStateMixin {
  late final AnimationController _c = AnimationController(vsync: this, duration: const Duration(milliseconds: 550));

  @override
  void initState() {
    super.initState();
    Future.delayed(widget.delay, () {
      if (mounted) _c.forward();
    });
  }

  @override
  void dispose() {
    _c.dispose();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    if (MediaQuery.of(context).disableAnimations) return widget.child;
    final curve = CurvedAnimation(parent: _c, curve: Curves.easeOutCubic);
    return FadeTransition(
      opacity: curve,
      child: SlideTransition(position: Tween(begin: const Offset(0, 0.25), end: Offset.zero).animate(curve), child: widget.child),
    );
  }
}

/// Builds the slides for a Rewind.
class _RewindSlides {
  final BuildContext context;
  final Rewind rewind;
  final String label;
  final String locale;
  final Widget actions;

  _RewindSlides(this.context, this.rewind, this.label, this.locale, this.actions);

  AppLocalizations get l => L.of(context)!;
  ListeningReport get r => rewind.report;

  String _group(int n) => NumberFormat.decimalPattern(locale).format(n);

  String _fmtSecs(int secs) {
    final h = secs ~/ 3600, m = (secs % 3600) ~/ 60;
    return h > 0 ? l.stats_hm('$h', '$m') : l.stats_m('$m');
  }

  String _fmtDay(String ymd) {
    final d = DateTime.tryParse(ymd);
    return d == null ? ymd : DateFormat.yMMMd(locale).format(d);
  }

  String _monthName(int m) => DateFormat.MMMM(locale).format(DateTime(2000, m));

  String get _previousLabel {
    final parts = rewind.period.split('-');
    if (parts.length < 2) return '${(int.tryParse(parts[0]) ?? 1) - 1}';
    final m = int.tryParse(parts[1]) ?? 1;
    return _monthName(m == 1 ? 12 : m - 1);
  }

  (String, String) _bigTime(int secs) {
    if (secs >= 3600) {
      final hours = secs / 3600;
      return (hours < 10 ? NumberFormat('0.0', locale).format(hours) : _group(hours.round()), l.rewind_hours);
    }
    return ('${secs ~/ 60}', l.rewind_minutes);
  }

  int? _peak(List<int> values) {
    int? best;
    for (var i = 0; i < values.length; i++) {
      if (values[i] > 0 && (best == null || values[i] > values[best])) best = i;
    }
    return best;
  }

  Widget _kicker(String text) => Padding(
        padding: const EdgeInsets.only(bottom: 10),
        child: Text(text, style: const TextStyle(fontSize: 15, fontWeight: FontWeight.w700, color: Colors.white70)),
      );

  Widget _title(String text, {bool long = false}) => Padding(
        padding: const EdgeInsets.only(bottom: 10),
        child: Text(text,
            textAlign: TextAlign.center,
            maxLines: 3,
            overflow: TextOverflow.ellipsis,
            style: TextStyle(color: Colors.white, fontSize: long ? 26 : 34, fontWeight: FontWeight.w800, height: 1.1)),
      );

  Widget _sub(String text) => Padding(
        padding: const EdgeInsets.only(bottom: 10),
        child: Text(text, textAlign: TextAlign.center, style: const TextStyle(fontSize: 16, height: 1.4, color: Colors.white)),
      );

  Widget _pill(String text) => Container(
        margin: const EdgeInsets.only(top: 6),
        padding: const EdgeInsets.symmetric(horizontal: 12, vertical: 6),
        decoration: BoxDecoration(color: Colors.white24, borderRadius: BorderRadius.circular(999)),
        child: Text(text, style: const TextStyle(fontWeight: FontWeight.w700, fontSize: 14, color: Colors.white)),
      );

  Widget _note() {
    final until = rewind.historyUntil;
    if (until == null) return const SizedBox.shrink();
    return Padding(
      padding: const EdgeInsets.only(top: 12),
      child: Text(l.rewind_times_from(_fmtDay(until)),
          textAlign: TextAlign.center, style: const TextStyle(fontSize: 12, color: Colors.white60)),
    );
  }

  Widget _big(String value, String unit) => Padding(
        padding: const EdgeInsets.only(bottom: 12),
        child: Column(children: [
          FittedBox(
            child: Text(value,
                style: const TextStyle(fontSize: 104, fontWeight: FontWeight.w900, height: 1, color: Colors.white, letterSpacing: -2)),
          ),
          const SizedBox(height: 4),
          Text(unit, style: const TextStyle(fontSize: 22, fontWeight: FontWeight.w700, color: Colors.white)),
        ]),
      );

  Widget _art(String url, double size, {double radius = 14}) => ClipRRect(
        borderRadius: BorderRadius.circular(radius),
        child: SizedBox(width: size, height: size, child: PodcastImage(url: url, width: size, height: size, fit: BoxFit.cover)),
      );

  Widget _episode(ReportEpisode e) => Column(children: [
        Container(
          decoration: BoxDecoration(boxShadow: const [BoxShadow(color: Colors.black38, blurRadius: 30, offset: Offset(0, 14))], borderRadius: BorderRadius.circular(16)),
          child: _art(e.artwork, 200, radius: 16),
        ),
        const SizedBox(height: 14),
        Text(e.title,
            textAlign: TextAlign.center,
            maxLines: 3,
            overflow: TextOverflow.ellipsis,
            style: const TextStyle(fontSize: 22, fontWeight: FontWeight.w800, height: 1.2, color: Colors.white)),
        const SizedBox(height: 4),
        Text(e.podcast, style: const TextStyle(fontSize: 15, color: Colors.white70)),
      ]);

  List<_Slide> build() {
    final out = <_Slide>[
      _Slide(const [Color(0xFF1D0F45), Color(0xFF7B2FF2)], [
        ClipRRect(borderRadius: BorderRadius.circular(18), child: Image.asset('assets/images/pinepods-logo.png', width: 72, height: 72)),
        const SizedBox(height: 16),
        _kicker(l.rewind_intro_kicker),
        _title(l.rewind_intro_title(label)),
        _sub(l.rewind_intro_sub),
      ]),
    ];
    if (r.seconds <= 0) {
      out.add(_Slide(const [Color(0xFF1B2233), Color(0xFF44536E)], [
        _title(l.rewind_empty_title),
        _sub(l.rewind_empty_sub(label)),
        const SizedBox(height: 16),
        actions,
      ]));
      return out;
    }

    // How much.
    final (value, unit) = _bigTime(r.seconds);
    final change = rewindChangePct(r.seconds, rewind.previousSeconds);
    final days = r.seconds ~/ 86400;
    out.add(_Slide(const [Color(0xFF0F2A5A), Color(0xFF1FA2A0)], [
      _kicker(l.rewind_time_kicker),
      _big(value, unit),
      if (days >= 1) _sub(l.rewind_time_days('$days')),
      Wrap(alignment: WrapAlignment.center, spacing: 8, runSpacing: 8, children: [
        _pill(l.rewind_n_podcasts(_group(r.podcasts))),
        _pill(l.rewind_n_episodes(_group(r.episodes))),
        if (r.finished > 0) _pill(l.rewind_n_finished(_group(r.finished))),
      ]),
      if (change != null) ...[
        const SizedBox(height: 14),
        _pill(change >= 0 ? l.rewind_more_than('${change.abs()}', _previousLabel) : l.rewind_less_than('${change.abs()}', _previousLabel)),
      ],
    ]));

    final first = rewind.firstPlay;
    if (first != null) {
      out.add(_Slide(const [Color(0xFF4A1A3A), Color(0xFFF08A4B)], [_kicker(l.rewind_first_kicker(label)), _episode(first)]));
    }

    // What.
    if (r.topPodcasts.isNotEmpty) {
      final top = r.topPodcasts.first;
      final share = (top.seconds / (r.seconds == 0 ? 1 : r.seconds) * 100).round();
      out.add(_Slide(const [Color(0xFF3C288C), Color(0xFF120B1F)], [
        _kicker(l.rewind_top_show_kicker),
        Container(
          decoration: BoxDecoration(boxShadow: const [BoxShadow(color: Colors.black45, blurRadius: 40, offset: Offset(0, 18))], borderRadius: BorderRadius.circular(18)),
          child: _art(top.artwork, 240, radius: 18),
        ),
        const SizedBox(height: 18),
        _title(top.name, long: top.name.length > 28),
        _sub(l.rewind_top_show_sub(_fmtSecs(top.seconds), '${top.episodes}')),
        if (share >= 5 && r.topPodcasts.length > 1) _pill(l.rewind_top_show_share('$share')),
      ]));
    }
    if (r.topPodcasts.length >= 3) {
      out.add(_Slide(const [Color(0xFF22113F), Color(0xFFC2367A)], [
        _kicker(l.rewind_top_shows_kicker),
        for (final (i, p) in r.topPodcasts.take(5).indexed)
          Container(
            margin: const EdgeInsets.only(bottom: 10),
            padding: const EdgeInsets.symmetric(horizontal: 10, vertical: 8),
            decoration: BoxDecoration(color: Colors.white12, borderRadius: BorderRadius.circular(14)),
            child: Row(children: [
              SizedBox(width: 26, child: Text('${i + 1}', style: const TextStyle(fontSize: 20, fontWeight: FontWeight.w900))),
              _art(p.artwork, 48, radius: 10),
              const SizedBox(width: 12),
              Expanded(child: Text(p.name, maxLines: 1, overflow: TextOverflow.ellipsis, textAlign: TextAlign.left, style: const TextStyle(fontWeight: FontWeight.w700))),
              Text(_fmtSecs(p.seconds), style: const TextStyle(fontSize: 13, color: Colors.white70)),
            ]),
          ),
      ]));
    }
    if (r.topEpisodes.isNotEmpty) {
      final e = r.topEpisodes.first;
      out.add(_Slide(const [Color(0xFF3A0D16), Color(0xFFE0533D)], [
        _kicker(l.rewind_top_episode_kicker),
        _episode(e),
        const SizedBox(height: 12),
        _sub(l.rewind_top_episode_sub(_fmtSecs(e.seconds))),
      ]));
    }
    if (rewind.newPodcastCount > 0) {
      out.add(_Slide(const [Color(0xFF0B3B2E), Color(0xFF4FBF6A)], [
        _kicker(l.rewind_new_kicker),
        _big('${rewind.newPodcastCount}', rewind.newPodcastCount == 1 ? l.rewind_new_show : l.rewind_new_shows),
        Wrap(spacing: 8, runSpacing: 8, alignment: WrapAlignment.center, children: [for (final p in rewind.newPodcasts) _art(p.artwork, 60, radius: 12)]),
        if (rewind.newPodcasts.isNotEmpty) ...[const SizedBox(height: 14), _sub(l.rewind_new_sub(rewind.newPodcasts.first.name))],
      ]));
    }
    if (r.topCategories.isNotEmpty) {
      final total = r.topCategories.fold<int>(0, (a, c) => a + c.seconds);
      out.add(_Slide(const [Color(0xFF2B145C), Color(0xFFE2A03F)], [
        _kicker(l.rewind_genres_kicker),
        _title(r.topCategories.first.name),
        const SizedBox(height: 8),
        for (final c in r.topCategories.take(5))
          Padding(
            padding: const EdgeInsets.only(bottom: 10),
            child: Row(children: [
              Expanded(flex: 2, child: Text(c.name, maxLines: 1, overflow: TextOverflow.ellipsis, textAlign: TextAlign.left, style: const TextStyle(fontWeight: FontWeight.w700))),
              const SizedBox(width: 10),
              Expanded(
                flex: 3,
                child: ClipRRect(
                  borderRadius: BorderRadius.circular(5),
                  child: LinearProgressIndicator(
                    minHeight: 10,
                    value: total == 0 ? 0 : c.seconds / total,
                    backgroundColor: Colors.white24,
                    valueColor: const AlwaysStoppedAnimation(Colors.white),
                  ),
                ),
              ),
              SizedBox(width: 48, child: Text('${total == 0 ? 0 : (c.seconds / total * 100).round()}%', textAlign: TextAlign.right)),
            ]),
          ),
      ]));
    }
    if (r.topPeople.isNotEmpty) {
      out.add(_Slide(const [Color(0xFF10294D), Color(0xFF8F4FD8)], [
        _kicker(l.rewind_people_kicker),
        const SizedBox(height: 8),
        Wrap(spacing: 14, runSpacing: 14, alignment: WrapAlignment.center, children: [
          for (final p in r.topPeople.take(3))
            SizedBox(
              width: 100,
              child: Column(children: [
                CircleAvatar(
                  radius: 44,
                  backgroundColor: Colors.white24,
                  foregroundImage: p.image == null ? null : NetworkImage(p.image!),
                  child: Text(p.name.isEmpty ? '' : p.name.characters.first.toUpperCase(),
                      style: const TextStyle(fontSize: 32, fontWeight: FontWeight.w800, color: Colors.white)),
                ),
                const SizedBox(height: 6),
                Text(p.name, textAlign: TextAlign.center, maxLines: 2, style: const TextStyle(fontWeight: FontWeight.w700, fontSize: 14)),
                Text(_fmtSecs(p.seconds), style: const TextStyle(fontSize: 12, color: Colors.white70)),
              ]),
            ),
        ]),
      ]));
    }

    // When.
    final hour = _peak(r.byHour);
    if (hour != null) {
      final (kind, icon) = rewindListenerKind(hour);
      final max = r.byHour.reduce((a, b) => a > b ? a : b);
      final dow = _peak(r.byDow);
      final kinds = {
        'early_riser': l.rewind_kind_early_riser,
        'morning': l.rewind_kind_morning,
        'afternoon': l.rewind_kind_afternoon,
        'evening': l.rewind_kind_evening,
        'night_owl': l.rewind_kind_night_owl,
      };
      out.add(_Slide(const [Color(0xFF070B24), Color(0xFF34307A)], [
        _kicker(l.rewind_when_kicker),
        Icon(icon, size: 64, color: Colors.white),
        const SizedBox(height: 10),
        _title(kinds[kind]!),
        _sub(l.rewind_when_sub(DateFormat.j(locale).format(DateTime(2000, 1, 1, hour)))),
        SizedBox(
          height: 90,
          child: Row(crossAxisAlignment: CrossAxisAlignment.end, children: [
            for (var h = 0; h < r.byHour.length; h++)
              Expanded(
                child: Container(
                  margin: const EdgeInsets.symmetric(horizontal: 1.5),
                  height: 90 * (max == 0 ? 0 : (r.byHour[h] / max).clamp(0.03, 1.0)),
                  decoration: BoxDecoration(
                    color: h == hour ? Colors.white : Colors.white30,
                    borderRadius: const BorderRadius.vertical(top: Radius.circular(3)),
                  ),
                ),
              ),
          ]),
        ),
        // 2 January 2000 was a Sunday.
        if (dow != null) ...[const SizedBox(height: 14), _sub(l.rewind_when_day(DateFormat.EEEE(locale).format(DateTime(2000, 1, 2 + dow))))],
        _note(),
      ]));
    }
    if (r.biggestDay != null || r.longestStreak > 1) {
      out.add(_Slide(const [Color(0xFF4B0F2F), Color(0xFFFF7A59)], [
        if (r.longestStreak > 1) ...[_kicker(l.rewind_streak_kicker), _big('${r.longestStreak}', l.rewind_days_in_a_row)],
        if (r.biggestDay != null) _sub(l.rewind_biggest_day(_fmtDay(r.biggestDay!.date), _fmtSecs(r.biggestDay!.seconds))),
        _sub(l.rewind_active_days('${r.activeDays}')),
        _note(),
      ]));
    }
    if (rewind.isYear && r.byMonth.where((m) => m.seconds > 0).length >= 2) {
      final best = r.byMonth.reduce((a, b) => b.seconds > a.seconds ? b : a);
      final max = best.seconds;
      out.add(_Slide(const [Color(0xFF0F2A5A), Color(0xFF1FA2A0)], [
        _kicker(l.rewind_months_kicker),
        _title(_monthName(best.monthNumber ?? 1)),
        const SizedBox(height: 8),
        SizedBox(
          height: 150,
          child: Row(crossAxisAlignment: CrossAxisAlignment.end, children: [
            for (final m in r.byMonth)
              Expanded(
                child: Column(mainAxisAlignment: MainAxisAlignment.end, children: [
                  Container(
                    margin: const EdgeInsets.symmetric(horizontal: 3),
                    height: 128 * (max == 0 ? 0 : (m.seconds / max).clamp(0.02, 1.0)),
                    decoration: BoxDecoration(
                      color: m.month == best.month ? Colors.white : Colors.white30,
                      borderRadius: const BorderRadius.vertical(top: Radius.circular(4)),
                    ),
                  ),
                  const SizedBox(height: 4),
                  Text(_monthName(m.monthNumber ?? 1).characters.first, style: const TextStyle(fontSize: 11, color: Colors.white70)),
                ]),
              ),
          ]),
        ),
        _note(),
      ]));
    }

    // How.
    final saved = r.timeSaved.total;
    final speed = r.averageSpeed != null && (r.averageSpeed! - 1).abs() >= 0.05 ? r.averageSpeed : null;
    final lag = r.releaseLag;
    final dayOne = lag != null && lag.episodes >= 3 && lag.withinDay > 0 ? lag : null;
    if (saved >= 60 || speed != null || dayOne != null) {
      Widget fact(IconData icon, String value, String text) => Container(
            margin: const EdgeInsets.only(bottom: 12),
            padding: const EdgeInsets.all(14),
            decoration: BoxDecoration(color: Colors.white12, borderRadius: BorderRadius.circular(14)),
            child: Row(children: [
              Icon(icon, size: 30, color: Colors.white),
              const SizedBox(width: 12),
              Expanded(
                child: Column(crossAxisAlignment: CrossAxisAlignment.start, children: [
                  Text(value, style: const TextStyle(fontSize: 24, fontWeight: FontWeight.w800)),
                  Text(text, textAlign: TextAlign.left, style: const TextStyle(fontSize: 13, color: Colors.white70)),
                ]),
              ),
            ]),
          );
      out.add(_Slide(const [Color(0xFF0B3B2E), Color(0xFF4FBF6A)], [
        _kicker(l.rewind_habits_kicker),
        if (speed != null) fact(PhosphorIconsRegular.gauge, '${NumberFormat('0.##', locale).format(speed)}×', l.rewind_habit_speed),
        if (saved >= 60) fact(PhosphorIconsRegular.lightning, _fmtSecs(saved), l.rewind_habit_saved),
        if (dayOne != null) fact(PhosphorIconsRegular.rocketLaunch, '${(dayOne.withinDay * 100).round()}%', l.rewind_habit_day_one),
      ]));
    }

    // The summary card.
    Widget cell(String label, String value) => Column(crossAxisAlignment: CrossAxisAlignment.start, children: [
          Text(label.toUpperCase(), style: const TextStyle(fontSize: 11, fontWeight: FontWeight.w700, letterSpacing: .6, color: Colors.white60)),
          const SizedBox(height: 2),
          Text(value, maxLines: 2, overflow: TextOverflow.ellipsis, style: const TextStyle(fontSize: 18, fontWeight: FontWeight.w700)),
        ]);
    final top = r.topPodcasts.isEmpty ? null : r.topPodcasts.first;
    out.add(_Slide(const [Color(0xFF461E78), Color(0xFF0D0A16)], [
      Container(
        width: double.infinity,
        padding: const EdgeInsets.all(18),
        decoration: BoxDecoration(
          color: Colors.white10,
          borderRadius: BorderRadius.circular(20),
          border: Border.all(color: Colors.white24),
        ),
        child: Column(crossAxisAlignment: CrossAxisAlignment.start, children: [
          Text(l.rewind_card_title(label), style: const TextStyle(fontWeight: FontWeight.w800, fontSize: 15)),
          const SizedBox(height: 14),
          if (top != null) ...[
            Row(children: [
              _art(top.artwork, 64, radius: 12),
              const SizedBox(width: 12),
              Expanded(child: cell(l.rewind_card_top_show, top.name)),
            ]),
            const SizedBox(height: 14),
          ],
          Row(children: [
            Expanded(child: cell(l.rewind_card_listened, '$value $unit')),
            Expanded(child: cell(l.rewind_card_episodes, _group(r.episodes))),
          ]),
          const SizedBox(height: 12),
          Row(children: [
            Expanded(child: cell(l.rewind_card_shows, _group(r.podcasts))),
            Expanded(
              child: r.topCategories.isNotEmpty
                  ? cell(l.rewind_card_genre, r.topCategories.first.name)
                  : cell(l.rewind_card_days, '${r.activeDays}'),
            ),
          ]),
          if (r.topPodcasts.length > 1) ...[
            const SizedBox(height: 12),
            for (final (i, p) in r.topPodcasts.skip(1).take(4).indexed)
              Padding(
                padding: const EdgeInsets.only(bottom: 3),
                child: Text('${i + 2}  ${p.name}', maxLines: 1, overflow: TextOverflow.ellipsis, style: const TextStyle(fontSize: 14)),
              ),
          ],
        ]),
      ),
      const SizedBox(height: 20),
      actions,
    ]));
    return out;
  }
}
