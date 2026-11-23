import 'package:flutter_test/flutter_test.dart';
import 'package:pinepods_mobile/services/podcast/episode_search.dart';

void main() {
  test('description search text drops markup and links', () {
    const html = '<p>Building the <b>100 Ornithopters</b> cube &amp; more.</p>'
        '<p>Our cubes: <a href="https://cubecobra.com/cube/overview/100-ornithopters">here</a> '
        'https://cubecobra.com/cube/list/vintage www.patreon.com/show cubecobra.com/cube/abc</p>'
        '<script>var x = "hidden";</script>&#8212; &#x41; a < b';
    expect(episodeSearchText(html), 'Building the 100 Ornithopters cube & more. Our cubes: here — A a < b');
  });

  test('terms, phrases and ranking', () {
    final s = EpisodeSearch.parse('  100  "Ornithopters Cube" 100 "open');
    expect(s.terms, ['100', 'ornithopters cube', 'open']);
    expect(EpisodeSearch.parse(' "" ').isEmpty, isTrue);

    final q = EpisodeSearch.parse('100 ornithopters');
    expect(q.titleRank('the 100 ornithopters cube'), 0);
    expect(q.titleRank('ornithopters again'), 1);
    expect(q.titleRank('episode 12'), 2);
    expect(q.matches('episode 12', 'we revisit the 100 ornithopters list'), isTrue);
    expect(q.matches('ornithopters', 'nothing here'), isFalse);
  });
}
