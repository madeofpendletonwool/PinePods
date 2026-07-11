// Spins up a SembastRepository backed by a fresh in-memory sembast database, so
// repository (and repository-dependent) tests run without touching disk or the
// path_provider platform channel.
//
// A new factory per call keeps each test fully isolated. `cleanup: false` skips
// the constructor's fire-and-forget startup cleanup, which would otherwise race
// with the test body.

import 'package:pinepods_mobile/repository/sembast/sembast_repository.dart';
import 'package:sembast/sembast_memory.dart';

SembastRepository newInMemoryRepository({bool cleanup = false}) {
  return SembastRepository(
    cleanup: cleanup,
    databaseName: 'test.db',
    databaseFactory: newDatabaseFactoryMemory(),
  );
}
