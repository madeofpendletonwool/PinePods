import 'package:flutter/widgets.dart';
import 'package:pinepods_mobile/l10n/L.dart';

/// Label for a "Delete after" choice (0 means never).
String deleteAfterDaysLabel(BuildContext context, int d) =>
    d == 0 ? L.of(context)!.settings_auto_update_episodes_never : L.of(context)!.days_count(d);
