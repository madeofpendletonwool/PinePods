// Copyright 2020 Ben Hills and the project contributors. All rights reserved.
// Use of this source code is governed by a BSD-style license that can be
// found in the LICENSE file.

/// Anytime can support multiple search providers.
///
/// This class represents a provider.
class SearchProvider {
  final String key;
  final String name;

  SearchProvider({
    required this.key,
    required this.name,
  });
}
