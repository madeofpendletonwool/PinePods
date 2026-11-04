import 'package:flutter_test/flutter_test.dart';
import 'package:pinepods_mobile/core/html_text.dart';

void main() {
  group('htmlToPlainText', () {
    test('strips a wrapping paragraph', () {
      expect(htmlToPlainText('<p>Song Exploder is a podcast.</p>'), 'Song Exploder is a podcast.');
    });

    test('keeps paragraph and line breaks', () {
      expect(htmlToPlainText('<p>One</p><p>Two<br/>Three</p>'), 'One\n\nTwo\nThree');
    });

    test('strips inline tags and keeps their text', () {
      expect(htmlToPlainText('Visit <a href="https://x.test">our <b>site</b></a>'), 'Visit our site');
    });

    test('decodes entities', () {
      expect(htmlToPlainText('Rock &amp; Roll&nbsp;Hour'), 'Rock & Roll Hour');
    });

    test('leaves plain text alone', () {
      expect(htmlToPlainText('A < B and AT&T\n\nSecond line'), 'A < B and AT&T\n\nSecond line');
    });

    test('handles uppercase tags and empty markup', () {
      expect(htmlToPlainText('<P>One</P><BR>Two'), 'One\n\nTwo');
      expect(htmlToPlainText('<p></p>'), '');
    });
  });
}
