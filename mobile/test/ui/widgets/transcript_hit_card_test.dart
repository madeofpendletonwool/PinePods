import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:pinepods_mobile/services/pinepods/pinepods_service.dart';
import 'package:pinepods_mobile/ui/widgets/transcript_hit_card.dart';

void main() {
  test('reads a transcript search page', () {
    final page = TranscriptSearchPage.fromJson({
      'results': [
        {
          'episode': {'episodeid': 7, 'episodetitle': 'Ep', 'podcastname': 'Show', 'episodepubdate': ''},
          'source': 'generated',
          'mentions': 3,
          'moments': [
            {'start': 90, 'text': 'about Kubernetes'},
            {'start': null, 'text': 'untimed'},
          ],
        }
      ],
      'total': 1,
      'terms': ['kubernetes'],
      'searchable_episodes': 12,
    });
    expect(page.total, 1);
    expect(page.searchableEpisodes, 12);
    final hit = page.results.single;
    expect(hit.episode.episodeId, 7);
    expect(hit.source, 'generated');
    expect(hit.moments.first.start, 90.0);
    expect(hit.moments.last.start, isNull);
  });

  test('formats moment times', () {
    expect(formatTranscriptTime(5.9), '00:05');
    expect(formatTranscriptTime(1421), '23:41');
    expect(formatTranscriptTime(3725), '1:02:05');
  });

  test('highlights every term, ignoring case', () {
    const base = TextStyle();
    final span = highlightTerms('Kubernetes and kubernetes', ['kubernetes'], base, Colors.yellow);
    final parts = span.children!.cast<TextSpan>();
    expect(parts.map((p) => p.text), ['Kubernetes', ' and ', 'kubernetes']);
    expect(parts[0].style?.backgroundColor, Colors.yellow);
    expect(parts[1].style, isNull);
  });
}
