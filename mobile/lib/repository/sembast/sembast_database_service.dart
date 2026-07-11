// Copyright 2020 Ben Hills and the project contributors. All rights reserved.
// Use of this source code is governed by a BSD-style license that can be
// found in the LICENSE file.

import 'dart:async';

import 'package:path/path.dart';
import 'package:path_provider/path_provider.dart';
import 'package:sembast/sembast.dart';
import 'package:sembast/sembast_io.dart';

typedef DatabaseUpgrade = Future<void> Function(Database, int, int);

/// Provides a database instance to other services and handles the opening
/// of the Sembast DB.
class DatabaseService {
  Completer<Database>? _databaseCompleter;
  String databaseName;
  int? version = 1;
  DatabaseUpgrade? upgraderCallback;

  /// Injectable database factory. When null the default disk-backed
  /// [databaseFactoryIo] is used (with a path resolved via path_provider);
  /// tests pass an in-memory factory (`newDatabaseFactoryMemory()`) to avoid
  /// disk and platform-channel access.
  final DatabaseFactory? databaseFactory;

  DatabaseService(
    this.databaseName, {
    this.version,
    this.upgraderCallback,
    this.databaseFactory,
  });

  Future<Database> get database async {
    if (_databaseCompleter == null) {
      _databaseCompleter = Completer();
      await _openDatabase();
    }

    return _databaseCompleter!.future;
  }

  Future _openDatabase() async {
    Future<void> onVersionChanged(Database db, int oldVersion, int newVersion) async {
      if (upgraderCallback != null) {
        await upgraderCallback!(db, oldVersion, newVersion);
      }
    }

    final Database database;
    if (databaseFactory != null) {
      // In-memory factories key on the name rather than a filesystem path.
      database = await databaseFactory!.openDatabase(
        databaseName,
        version: version,
        onVersionChanged: onVersionChanged,
      );
    } else {
      final appDocumentDir = await getApplicationDocumentsDirectory();
      final dbPath = join(appDocumentDir.path, databaseName);
      database = await databaseFactoryIo.openDatabase(
        dbPath,
        version: version,
        onVersionChanged: onVersionChanged,
      );
    }

    _databaseCompleter!.complete(database);
  }
}
