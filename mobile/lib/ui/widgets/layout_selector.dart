// Copyright 2020 Ben Hills and the project contributors. All rights reserved.
// Use of this source code is governed by a BSD-style license that can be
// found in the LICENSE file.

import 'package:pinepods_mobile/bloc/settings/settings_bloc.dart';
import 'package:pinepods_mobile/entities/app_settings.dart';
import 'package:pinepods_mobile/l10n/L.dart';
import 'package:pinepods_mobile/ui/widgets/slider_handle.dart';
import 'package:flutter/material.dart';
import 'package:provider/provider.dart';

/// Allows the user to select the layout for the library and discovery panes.
/// Can select from a list or different sized grids.
class LayoutSelectorWidget extends StatefulWidget {
  const LayoutSelectorWidget({
    super.key,
  });

  @override
  State<LayoutSelectorWidget> createState() => _LayoutSelectorWidgetState();
}

class _LayoutSelectorWidgetState extends State<LayoutSelectorWidget> {
  var speed = 1.0;

  @override
  void initState() {
    var settingsBloc = Provider.of<SettingsBloc>(context, listen: false);

    speed = settingsBloc.currentSettings.playbackSpeed;

    super.initState();
  }

  @override
  Widget build(BuildContext context) {
    final settingsBloc = Provider.of<SettingsBloc>(context, listen: false);

    return StreamBuilder<AppSettings>(
        stream: settingsBloc.settings,
        initialData: AppSettings.sensibleDefaults(),
        builder: (context, snapshot) {
          final mode = snapshot.data!.layout;

          return Column(
            mainAxisSize: MainAxisSize.min,
            mainAxisAlignment: MainAxisAlignment.start,
            crossAxisAlignment: CrossAxisAlignment.center,
            children: <Widget>[
              const SliderHandle(),
              Padding(
                padding: const EdgeInsets.fromLTRB(8.0, 24.0, 8.0, 24.0),
                child: Row(
                    mainAxisAlignment: MainAxisAlignment.center,
                    crossAxisAlignment: CrossAxisAlignment.center,
                    children: [
                      Expanded(
                          child: Row(
                        children: [
                          const Padding(
                            padding: EdgeInsets.only(left: 8.0, right: 8.0),
                            child: Icon(
                              Icons.grid_view,
                              size: 18,
                            ),
                          ),
                          ExcludeSemantics(
                            child: Text(
                              L.of(context)!.layout_label,
                              style: Theme.of(context).textTheme.titleMedium,
                            ),
                          ),
                        ],
                      )),
                      Padding(
                        padding: const EdgeInsets.only(
                          left: 8.0,
                          right: 8.0,
                        ),
                        child: OutlinedButton(
                          onPressed: () {
                            settingsBloc.layoutMode(0);
                          },
                          style: OutlinedButton.styleFrom(
                            backgroundColor: mode == 0 ? Theme.of(context).primaryColor : null,
                          ),
                          child: Semantics(
                            selected: mode == 0,
                            label: L.of(context)!.semantics_layout_option_list,
                            child: Icon(
                              Icons.list,
                              color: mode == 0 ? Theme.of(context).canvasColor : Theme.of(context).primaryColor,
                            ),
                          ),
                        ),
                      ),
                      Padding(
                        padding: const EdgeInsets.only(
                          right: 8.0,
                        ),
                        child: OutlinedButton(
                          onPressed: () {
                            settingsBloc.layoutMode(1);
                          },
                          style: OutlinedButton.styleFrom(
                            backgroundColor: mode == 1 ? Theme.of(context).primaryColor : null,
                          ),
                          child: Semantics(
                            selected: mode == 1,
                            label: L.of(context)!.semantics_layout_option_compact_grid,
                            child: Icon(
                              Icons.grid_on,
                              color: mode == 1 ? Theme.of(context).canvasColor : Theme.of(context).primaryColor,
                            ),
                          ),
                        ),
                      ),
                      Padding(
                        padding: const EdgeInsets.only(
                          right: 8.0,
                        ),
                        child: OutlinedButton(
                          onPressed: () {
                            settingsBloc.layoutMode(2);
                          },
                          style: OutlinedButton.styleFrom(
                            backgroundColor: mode == 2 ? Theme.of(context).primaryColor : null,
                          ),
                          child: Semantics(
                            selected: mode == 2,
                            label: L.of(context)!.semantics_layout_option_grid,
                            child: Icon(
                              Icons.grid_view,
                              color: mode == 2 ? Theme.of(context).canvasColor : Theme.of(context).primaryColor,
                            ),
                          ),
                        ),
                      ),
                    ]),
              ),
              const SizedBox(
                height: 8.0,
              ),
            ],
          );
        });
  }
}
