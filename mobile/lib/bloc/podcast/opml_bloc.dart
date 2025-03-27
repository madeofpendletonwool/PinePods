// Copyright 2020 Ben Hills and the project contributors. All rights reserved.
// Use of this source code is governed by a BSD-style license that can be
// found in the LICENSE file.

import 'package:pinepods_mobile/bloc/bloc.dart';
import 'package:pinepods_mobile/services/podcast/opml_service.dart';
import 'package:pinepods_mobile/state/opml_state.dart';
import 'package:logging/logging.dart';
import 'package:rxdart/rxdart.dart';

/// OPML (Outline Processor Markup Language) is an XML format for outlines, which is used in Podcast
/// apps for transferring podcast subscriptions/follows from/to other podcast apps.
///
/// Anytime supports both import and export of OPML.
class OPMLBloc extends Bloc {
  final log = Logger('OPMLBloc');

  final PublishSubject<OPMLEvent> _opmlEvent = PublishSubject<OPMLEvent>();
  final PublishSubject<OPMLState> _opmlState = PublishSubject<OPMLState>();
  final OPMLService opmlService;

  OPMLBloc({required this.opmlService}) {
    _listenOpmlEvents();
  }

  void _listenOpmlEvents() {
    _opmlEvent.listen((event) {
      if (event is OPMLImportEvent) {
        if (event.file != null) {
          opmlService.loadOPMLFile(event.file!).listen((state) {
            _opmlState.add(state);
          });
        }
      } else if (event is OPMLExportEvent) {
        opmlService.saveOPMLFile().listen((state) {
          _opmlState.add(state);
        });
      } else if (event is OPMLCancelEvent) {
        opmlService.cancel();
      }
    });
  }

  void Function(OPMLEvent) get opmlEvent => _opmlEvent.add;

  Stream<OPMLState> get opmlState => _opmlState.stream;
}
