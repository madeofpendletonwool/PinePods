import 'package:flutter/material.dart';
import 'package:intl/intl.dart';
import 'package:pinepods_mobile/services/pinepods/pinepods_service.dart';
import 'package:pinepods_mobile/ui/icons/phosphor_icons.dart';
import 'package:pinepods_mobile/ui/widgets/lazy_network_image.dart';
import 'package:pinepods_mobile/ui/widgets/soft_ui.dart';

/// Moments shown before "more moments".
const _foldedMoments = 2;

String formatTranscriptTime(double seconds) {
  final s = seconds.floor();
  final h = s ~/ 3600, m = (s % 3600) ~/ 60, sec = s % 60;
  String two(int v) => v.toString().padLeft(2, '0');
  return h > 0 ? '$h:${two(m)}:${two(sec)}' : '${two(m)}:${two(sec)}';
}

/// [text] with every occurrence of [terms] (case-insensitive) highlighted.
TextSpan highlightTerms(String text, List<String> terms, TextStyle base, Color mark) {
  final lower = text.toLowerCase();
  final marked = List<bool>.filled(text.length, false);
  // toLowerCase can change length for a few characters; only highlight when it doesn't.
  if (lower.length == text.length) {
    for (final t in terms.where((t) => t.isNotEmpty)) {
      var from = 0;
      while (true) {
        final i = lower.indexOf(t, from);
        if (i < 0) break;
        for (var k = i; k < i + t.length; k++) {
          marked[k] = true;
        }
        from = i + t.length;
      }
    }
  }
  final spans = <TextSpan>[];
  var i = 0;
  while (i < text.length) {
    final on = marked[i];
    final start = i;
    while (i < text.length && marked[i] == on) {
      i++;
    }
    spans.add(TextSpan(
      text: text.substring(start, i),
      style: on ? base.copyWith(backgroundColor: mark, fontWeight: FontWeight.w600) : null,
    ));
  }
  return TextSpan(style: base, children: spans);
}

/// An episode whose transcript matched: where it was said, with a button to
/// play from each moment.
class TranscriptHitCard extends StatefulWidget {
  final TranscriptHit hit;
  final List<String> terms;
  final VoidCallback onOpen;
  final void Function(double seconds) onPlayFrom;

  const TranscriptHitCard({
    super.key,
    required this.hit,
    required this.terms,
    required this.onOpen,
    required this.onPlayFrom,
  });

  @override
  State<TranscriptHitCard> createState() => _TranscriptHitCardState();
}

class _TranscriptHitCardState extends State<TranscriptHitCard> {
  bool _expanded = false;

  String _date(String raw) {
    final d = DateTime.tryParse(raw);
    return d == null ? '' : DateFormat.yMMMd().format(d);
  }

  @override
  Widget build(BuildContext context) {
    final theme = Theme.of(context);
    final hit = widget.hit;
    final ep = hit.episode;
    final accent = theme.colorScheme.primary;
    final muted = theme.hintColor;
    final moments = _expanded ? hit.moments : hit.moments.take(_foldedMoments).toList();
    final hidden = hit.moments.length - moments.length;
    final art = ep.episodeArtwork.isNotEmpty ? ep.episodeArtwork : ep.artworkUrl;
    final meta = [
      ep.podcastName,
      _date(ep.episodePubDate),
      hit.mentions == 1 ? '1 mention' : '${hit.mentions} mentions',
    ].where((s) => s.isNotEmpty).join(' · ');
    final textStyle = theme.textTheme.bodyMedium!.copyWith(height: 1.4);

    return Padding(
      padding: const EdgeInsets.fromLTRB(16, 0, 16, 10),
      child: Material(
        color: softFill(theme),
        borderRadius: BorderRadius.circular(16),
        child: InkWell(
          borderRadius: BorderRadius.circular(16),
          onTap: widget.onOpen,
          child: Padding(
            padding: const EdgeInsets.all(12),
            child: Column(
              crossAxisAlignment: CrossAxisAlignment.start,
              children: [
                Row(
                  crossAxisAlignment: CrossAxisAlignment.start,
                  children: [
                    LazyNetworkImage(imageUrl: art, width: 52, height: 52, borderRadius: BorderRadius.circular(10)),
                    const SizedBox(width: 12),
                    Expanded(
                      child: Column(
                        crossAxisAlignment: CrossAxisAlignment.start,
                        children: [
                          Text(
                            ep.episodeTitle,
                            maxLines: 2,
                            overflow: TextOverflow.ellipsis,
                            style: theme.textTheme.titleSmall!.copyWith(fontWeight: FontWeight.w600),
                          ),
                          const SizedBox(height: 2),
                          Text(meta, maxLines: 1, overflow: TextOverflow.ellipsis, style: theme.textTheme.bodySmall!.copyWith(color: muted)),
                          const SizedBox(height: 2),
                          Text(
                            hit.source == 'generated' ? 'AI transcript' : "Podcast's transcript",
                            style: theme.textTheme.labelSmall!.copyWith(color: hit.source == 'generated' ? accent : muted),
                          ),
                        ],
                      ),
                    ),
                  ],
                ),
                const SizedBox(height: 10),
                for (final m in moments)
                  Padding(
                    padding: const EdgeInsets.only(bottom: 8),
                    child: Row(
                      crossAxisAlignment: CrossAxisAlignment.start,
                      children: [
                        if (m.start != null)
                          _MomentButton(
                            label: formatTranscriptTime(m.start!),
                            accent: accent,
                            onTap: () => widget.onPlayFrom(m.start!),
                          )
                        else
                          Padding(
                            padding: const EdgeInsets.only(top: 2, right: 4),
                            child: Icon(PhosphorIconsRegular.quotes, size: 16, color: muted),
                          ),
                        const SizedBox(width: 10),
                        Expanded(
                          child: Text.rich(highlightTerms(m.text, widget.terms, textStyle, accent.withOpacity(0.22))),
                        ),
                      ],
                    ),
                  ),
                if (hidden > 0 || (_expanded && hit.moments.length > _foldedMoments))
                  TextButton.icon(
                    style: TextButton.styleFrom(
                      padding: const EdgeInsets.symmetric(horizontal: 4),
                      minimumSize: const Size(0, 32),
                      foregroundColor: muted,
                    ),
                    onPressed: () => setState(() => _expanded = !_expanded),
                    icon: Icon(_expanded ? PhosphorIconsRegular.caretUp : PhosphorIconsRegular.caretDown, size: 16),
                    label: Text(_expanded ? 'Show fewer' : '$hidden more moments'),
                  ),
              ],
            ),
          ),
        ),
      ),
    );
  }
}

class _MomentButton extends StatelessWidget {
  final String label;
  final Color accent;
  final VoidCallback onTap;

  const _MomentButton({required this.label, required this.accent, required this.onTap});

  @override
  Widget build(BuildContext context) {
    return Semantics(
      button: true,
      label: 'Play from $label',
      child: Material(
        color: accent.withOpacity(0.12),
        shape: StadiumBorder(side: BorderSide(color: accent.withOpacity(0.35))),
        child: InkWell(
          customBorder: const StadiumBorder(),
          onTap: onTap,
          child: Padding(
            padding: const EdgeInsets.fromLTRB(8, 5, 10, 5),
            child: Row(
              mainAxisSize: MainAxisSize.min,
              children: [
                Icon(PhosphorIconsFill.play, size: 12, color: accent),
                const SizedBox(width: 4),
                Text(
                  label,
                  style: TextStyle(
                    color: accent,
                    fontSize: 12.5,
                    fontWeight: FontWeight.w600,
                    fontFeatures: const [FontFeature.tabularFigures()],
                  ),
                ),
              ],
            ),
          ),
        ),
      ),
    );
  }
}
