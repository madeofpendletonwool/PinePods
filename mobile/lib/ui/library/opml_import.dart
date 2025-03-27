// Copyright 2020 Ben Hills and the project contributors. All rights reserved.
// Use of this source code is governed by a BSD-style license that can be
// found in the LICENSE file.

import 'package:pinepods_mobile/bloc/podcast/opml_bloc.dart';
import 'package:pinepods_mobile/l10n/L.dart';
import 'package:pinepods_mobile/state/opml_state.dart';
import 'package:flutter/material.dart';
import 'package:provider/provider.dart';

class OPMLImport extends StatefulWidget {
  final String file;

  const OPMLImport({
    super.key,
    required this.file,
  });

  @override
  State<OPMLImport> createState() => _OPMLImportState();
}

class _OPMLImportState extends State<OPMLImport> {
  @override
  Widget build(BuildContext context) {
    final bloc = Provider.of<OPMLBloc>(context, listen: false);
    final width = MediaQuery.of(context).size.width - 60.0;

    return IntrinsicHeight(
      child: SizedBox(
        width: width,
        child: StreamBuilder<OPMLState>(
            initialData: OPMLNoneState(),
            stream: bloc.opmlState,
            builder: (context, snapshot) {
              String? t = '';
              var d = snapshot.data;

              if (d is OPMLCompletedState) {
                WidgetsBinding.instance.addPostFrameCallback((_) {
                  Navigator.pop(context);
                });
              } else if (d is OPMLLoadingState) {
                double pct = ((d.current / d.total) * 100);
                t = '${pct.toInt()}% - ${d.podcast}';
              }

              return Row(
                mainAxisAlignment: MainAxisAlignment.start,
                crossAxisAlignment: CrossAxisAlignment.center,
                children: [
                  const Flexible(
                    child: CircularProgressIndicator.adaptive(),
                  ),
                  Flexible(
                    flex: 4,
                    child: Padding(
                      padding: const EdgeInsets.only(left: 16.0),
                      child: Column(
                        mainAxisAlignment: MainAxisAlignment.center,
                        crossAxisAlignment: CrossAxisAlignment.start,
                        children: <Widget>[
                          Text(
                            L.of(context)!.label_opml_importing,
                            maxLines: 1,
                          ),
                          const SizedBox(
                            width: 0.0,
                            height: 2.0,
                          ),
                          Text(
                            t,
                            maxLines: 1,
                            overflow: TextOverflow.ellipsis,
                            style: const TextStyle(
                              fontSize: 12.0,
                            ),
                          ),
                        ],
                      ),
                    ),
                  ),
                ],
              );
            }),
      ),
    );
  }

  @override
  void initState() {
    super.initState();

    final bloc = Provider.of<OPMLBloc>(context, listen: false);

    bloc.opmlEvent(
      OPMLImportEvent(file: widget.file),
    );
  }
}
