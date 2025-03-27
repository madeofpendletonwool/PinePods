// Copyright 2020 Ben Hills and the project contributors. All rights reserved.
// Use of this source code is governed by a BSD-style license that can be
// found in the LICENSE file.

import 'package:pinepods_mobile/l10n/L.dart';
import 'package:pinepods_mobile/ui/library/opml_import.dart';
import 'package:file_picker/file_picker.dart';
import 'package:flutter/cupertino.dart';
import 'package:flutter/foundation.dart';
import 'package:flutter/material.dart';

class OPMLSelect extends StatefulWidget {
  const OPMLSelect({super.key});

  @override
  State<OPMLSelect> createState() => _OPMLSelectState();
}

class _OPMLSelectState extends State<OPMLSelect> {
  @override
  Widget build(BuildContext context) {
    switch (defaultTargetPlatform) {
      case TargetPlatform.android:
        return _buildAndroid(context);
      case TargetPlatform.iOS:
        return _buildIos(context);
      default:
        assert(false, 'Unexpected platform $defaultTargetPlatform');
        return _buildAndroid(context);
    }
  }

  Widget _buildAndroid(BuildContext context) {
    return Scaffold(
      appBar: AppBar(
        elevation: 0.0,
        title: Text(
          L.of(context)!.opml_import_export_label,
        ),
      ),
      body: _buildBody(context),
    );
  }

  Widget _buildIos(BuildContext context) {
    return CupertinoPageScaffold(
      navigationBar: const CupertinoNavigationBar(),
      child: _buildBody(context),
    );
  }

  Widget _buildBody(BuildContext context) {
    return Row(
      mainAxisAlignment: MainAxisAlignment.center,
      children: [
        Column(
          mainAxisAlignment: MainAxisAlignment.center,
          crossAxisAlignment: CrossAxisAlignment.center,
          children: [
            ElevatedButton(
              onPressed: () async {
                final navigator = Navigator.of(context);
                var result = (await FilePicker.platform.pickFiles())!;

                if (result.count > 0) {
                  var file = result.files.first;

                  await navigator.push(
                    MaterialPageRoute<void>(
                      settings: const RouteSettings(name: 'opmlimport'),
                      builder: (context) => OPMLImport(file: file.path!),
                      fullscreenDialog: true,
                    ),
                  );

                  navigator.pop();
                }
              },
              child: Text(L.of(context)!.opml_import_button_label),
            ),
            ElevatedButton(
              onPressed: () {},
              child: Text(L.of(context)!.opml_export_button_label),
            ),
          ],
        ),
      ],
    );
  }
}
