// Copyright 2020 Ben Hills and the project contributors. All rights reserved.
// Use of this source code is governed by a BSD-style license that can be
// found in the LICENSE file.
// ignore_for_file: must_be_immutable

import 'dart:async';

import 'package:pinepods_mobile/entities/person.dart';
import 'package:extended_image/extended_image.dart';
import 'package:flutter/material.dart';
import 'package:pinepods_mobile/ui/pinepods/people/people_widgets.dart' show openPerson;
import 'package:url_launcher/url_launcher.dart';

/// This Widget handles rendering of a person avatar.
///
/// The data comes from the <person> tag in the Podcasting 2.0 namespace.
///
/// https://github.com/Podcastindex-org/podcast-namespace/blob/main/docs/1.0.md#person
class PersonAvatar extends StatelessWidget {
  final Person person;
  String initials = '';
  String role = '';

  PersonAvatar({
    super.key,
    required this.person,
  }) {
    if (person.name.isNotEmpty) {
      var parts = person.name.split(' ');

      for (var i in parts) {
        if (i.isNotEmpty) {
          initials += i.substring(0, 1).toUpperCase();
        }
      }
    }

    if (person.role.isNotEmpty) {
      role = person.role.substring(0, 1).toUpperCase() + person.role.substring(1);
    }
  }

  /// Open their PinePods person page when the server knows them, otherwise their own link.
  VoidCallback? _onTap(BuildContext context) {
    final id = person.personId ?? 0;
    if (id > 0) {
      return () => unawaited(openPerson(context, id, name: person.name));
    }
    final link = person.link;
    if (link == null || link.isEmpty) return null;
    return () {
      final uri = Uri.parse(link);
      unawaited(canLaunchUrl(uri).then((value) => launchUrl(uri)));
    };
  }

  @override
  Widget build(BuildContext context) {
    return GestureDetector(
      behavior: HitTestBehavior.translucent,
      onTap: _onTap(context),
      child: SizedBox(
        width: 96,
        child: Padding(
          padding: const EdgeInsets.only(left: 16.0),
          child: Column(
            mainAxisSize: MainAxisSize.min,
            children: [
              CircleAvatar(
                radius: 32,
                // No image request for an empty URL: it can only fail.
                foregroundImage: (person.image ?? '').isEmpty
                    ? null
                    : ExtendedImage.network(person.image!, cache: true).image,
                child: Text(initials),
              ),
              Text(
                person.name,
                maxLines: 3,
                textAlign: TextAlign.center,
              ),
              Text(role),
            ],
          ),
        ),
      ),
    );
  }
}
