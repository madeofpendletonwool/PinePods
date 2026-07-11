import 'package:flutter_test/flutter_test.dart';
import 'package:pinepods_mobile/entities/person.dart';

void main() {
  group('construction', () {
    test('upgrades http image/link to https', () {
      final person = Person(name: 'Host', image: 'http://example.com/host.png', link: 'http://example.com');

      expect(person.image, 'https://example.com/host.png');
      expect(person.link, 'https://example.com');
    });

    test('role and group default to an empty string', () {
      final person = Person(name: 'Host');

      expect(person.role, '');
      expect(person.group, '');
    });
  });

  group('toMap/fromMap', () {
    test('round-trips every field', () {
      final original = Person(name: 'Host', role: 'host', group: 'cast', image: 'https://example.com/host.png', link: 'https://example.com');

      final restored = Person.fromMap(original.toMap());

      expect(restored, original);
      expect(restored.name, 'Host');
      expect(restored.role, 'host');
      expect(restored.group, 'cast');
    });

    test('missing fields default to an empty string rather than throwing', () {
      final restored = Person.fromMap(const {});

      expect(restored.name, '');
      expect(restored.role, '');
      expect(restored.group, '');
    });
  });

  group('equality', () {
    test('two persons with identical fields are equal', () {
      final a = Person(name: 'Host', role: 'host');
      final b = Person(name: 'Host', role: 'host');

      expect(a, equals(b));
      expect(a.hashCode, b.hashCode);
    });

    test('persons with a different name are not equal', () {
      final a = Person(name: 'Host A');
      final b = Person(name: 'Host B');

      expect(a, isNot(equals(b)));
    });
  });
}
