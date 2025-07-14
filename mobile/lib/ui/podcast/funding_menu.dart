// Copyright 2020 Ben Hills and the project contributors. All rights reserved.
// Use of this source code is governed by a BSD-style license that can be
// found in the LICENSE file.

import 'dart:async';

import 'package:pinepods_mobile/bloc/settings/settings_bloc.dart';
import 'package:pinepods_mobile/entities/app_settings.dart';
import 'package:pinepods_mobile/entities/funding.dart';
import 'package:pinepods_mobile/l10n/L.dart';
import 'package:pinepods_mobile/ui/widgets/action_text.dart';
import 'package:flutter/cupertino.dart';
import 'package:flutter/material.dart';
import 'package:flutter_dialogs/flutter_dialogs.dart';
import 'package:provider/provider.dart';
import 'package:url_launcher/url_launcher.dart';

/// This class is responsible for rendering the funding menu on the podcast details page.
///
/// It returns either a Material or Cupertino style menu instance depending upon which
/// platform we are running on.
///
/// The target platform is based on the current [Theme]: [ThemeData.platform].
class FundingMenu extends StatelessWidget {
  final List<Funding>? funding;

  const FundingMenu(
    this.funding, {
    super.key,
  });

  @override
  Widget build(BuildContext context) {
    var theme = Theme.of(context);

    switch (theme.platform) {
      case TargetPlatform.android:
      case TargetPlatform.fuchsia:
      case TargetPlatform.linux:
      case TargetPlatform.windows:
        return _MaterialFundingMenu(funding);
      case TargetPlatform.iOS:
      case TargetPlatform.macOS:
        return _CupertinoFundingMenu(funding);
    }
  }
}

/// This is the material design version of the context menu. This will be rendered
/// for all platforms that are not iOS.
class _MaterialFundingMenu extends StatelessWidget {
  final List<Funding>? funding;

  const _MaterialFundingMenu(this.funding);

  @override
  Widget build(BuildContext context) {
    final settingsBloc = Provider.of<SettingsBloc>(context);

    return funding == null || funding!.isEmpty
        ? const SizedBox(
            width: 0.0,
            height: 0.0,
          )
        : StreamBuilder<AppSettings>(
            stream: settingsBloc.settings,
            initialData: AppSettings.sensibleDefaults(),
            builder: (context, snapshot) {
              return Semantics(
                label: L.of(context)!.podcast_funding_dialog_header,
                child: PopupMenuButton<String>(
                  onSelected: (url) {
                    FundingLink.fundingLink(
                      url,
                      snapshot.data!.externalLinkConsent,
                      context,
                    ).then((value) {
                      settingsBloc.setExternalLinkConsent(value);
                    });
                  },
                  icon: const Icon(
                    Icons.payment,
                  ),
                  itemBuilder: (BuildContext context) {
                    return List<PopupMenuEntry<String>>.generate(funding!.length, (index) {
                      return PopupMenuItem<String>(
                        value: funding![index].url,
                        enabled: true,
                        child: Text(funding![index].value),
                      );
                    });
                  },
                ),
              );
            });
  }
}

/// This is the Cupertino context menu and is rendered only when running on
/// an iOS device.
class _CupertinoFundingMenu extends StatelessWidget {
  final List<Funding>? funding;

  const _CupertinoFundingMenu(this.funding);

  @override
  Widget build(BuildContext context) {
    final settingsBloc = Provider.of<SettingsBloc>(context);

    return funding == null || funding!.isEmpty
        ? const SizedBox(
            width: 0.0,
            height: 0.0,
          )
        : StreamBuilder<AppSettings>(
            stream: settingsBloc.settings,
            initialData: AppSettings.sensibleDefaults(),
            builder: (context, snapshot) {
              return IconButton(
                tooltip: L.of(context)!.podcast_funding_dialog_header,
                icon: const Icon(Icons.payment),
                visualDensity: VisualDensity.compact,
                onPressed: () => showCupertinoModalPopup<void>(
                  context: context,
                  builder: (BuildContext context) {
                    return CupertinoActionSheet(
                      actions: <Widget>[
                        ...List<CupertinoActionSheetAction>.generate(funding!.length, (index) {
                          return CupertinoActionSheetAction(
                            onPressed: () {
                              FundingLink.fundingLink(
                                funding![index].url,
                                snapshot.data!.externalLinkConsent,
                                context,
                              ).then((value) {
                                settingsBloc.setExternalLinkConsent(value);
                                if (context.mounted) {
                                  Navigator.of(context).pop('Cancel');
                                }
                              });
                            },
                            child: Text(funding![index].value),
                          );
                        }),
                      ],
                      cancelButton: CupertinoActionSheetAction(
                        isDefaultAction: true,
                        onPressed: () {
                          Navigator.pop(context, 'Cancel');
                        },
                        child: Text(L.of(context)!.cancel_option_label),
                      ),
                    );
                  },
                ),
              );
            });
  }
}

class FundingLink {
  /// Check the consent status. If this is the first time we have been
  /// requested to open a funding link, present the user with and
  /// information dialog first to make clear that the link is provided
  /// by the podcast owner and not Pinepods.
  static Future<bool> fundingLink(String url, bool consent, BuildContext context) async {
    bool? result = false;

    if (consent) {
      result = true;
      final uri = Uri.parse(url);

      if (!await launchUrl(
        uri,
        mode: LaunchMode.externalApplication,
      )) {
        throw Exception('Could not launch $uri');
      }
    } else {
      result = await showPlatformDialog<bool>(
        context: context,
        useRootNavigator: false,
        builder: (_) => BasicDialogAlert(
          title: Semantics(
            header: true,
            child: Text(L.of(context)!.podcast_funding_dialog_header),
          ),
          content: Text(L.of(context)!.consent_message),
          actions: <Widget>[
            BasicDialogAction(
              title: ActionText(
                L.of(context)!.go_back_button_label,
              ),
              onPressed: () {
                Navigator.pop(context, false);
              },
            ),
            BasicDialogAction(
              title: ActionText(
                L.of(context)!.continue_button_label,
              ),
              iosIsDefaultAction: true,
              onPressed: () {
                Navigator.pop(context, true);
              },
            ),
          ],
        ),
      );

      if (result!) {
        var uri = Uri.parse(url);

        unawaited(
          canLaunchUrl(uri).then((value) => launchUrl(uri)),
        );
      }
    }

    return Future.value(result);
  }
}
