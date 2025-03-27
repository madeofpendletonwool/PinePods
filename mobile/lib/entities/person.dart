// Copyright 2020 Ben Hills and the project contributors. All rights reserved.
// Use of this source code is governed by a BSD-style license that can be
// found in the LICENSE file.

import 'package:pinepods_mobile/core/extensions.dart';

/// This class represents a person of interest to the podcast.
///
/// It is primarily intended to identify people like hosts, co-hosts and guests.
class Person {
  final String name;
  final String role;
  final String group;
  final String? image;
  final String? link;

  Person({
    required this.name,
    this.role = '',
    this.group = '',
    String? image = '',
    String? link = '',
  })  : image = image?.forceHttps,
        link = link?.forceHttps;

  Map<String, dynamic> toMap() {
    return <String, dynamic>{
      'name': name,
      'role': role,
      'group': group,
      'image': image,
      'link': link,
    };
  }

  static Person fromMap(Map<String, dynamic> chapter) {
    return Person(
      name: chapter['name'] as String? ?? '',
      role: chapter['role'] as String? ?? '',
      group: chapter['group'] as String? ?? '',
      image: chapter['image'] as String? ?? '',
      link: chapter['link'] as String? ?? '',
    );
  }

  @override
  bool operator ==(Object other) =>
      identical(this, other) ||
      other is Person &&
          runtimeType == other.runtimeType &&
          name == other.name &&
          role == other.role &&
          group == other.group &&
          image == other.image &&
          link == other.link;

  @override
  int get hashCode => name.hashCode ^ role.hashCode ^ group.hashCode ^ image.hashCode ^ link.hashCode;

  @override
  String toString() {
    return 'Person{name: $name, role: $role, group: $group, image: $image, link: $link}';
  }
}
