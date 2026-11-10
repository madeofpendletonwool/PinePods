// Copyright 2020 Ben Hills and the project contributors. All rights reserved.
// Use of this source code is governed by a BSD-style license that can be
// found in the LICENSE file.
import 'package:flutter/widgets.dart';
import 'package:pinepods_mobile/l10n/app_localizations.dart';

/// Access to the gen-l10n generated [AppLocalizations].
///
/// Widgets use `L.of(context)!.some_key`. Code without a BuildContext
/// (services, CarPlay, entities) uses [L.current], which the app keeps in step
/// with the active locale. Strings are generated from the ARB files in this
/// directory; keys missing from a locale fall back to English.
class L {
  static AppLocalizations? of(BuildContext context) => AppLocalizations.of(context);

  /// Strings for the active locale, for code that has no BuildContext.
  static AppLocalizations current = lookupAppLocalizations(const Locale('en'));
}
