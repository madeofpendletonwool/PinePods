// Copyright 2020 Ben Hills and the project contributors. All rights reserved.
// Use of this source code is governed by a BSD-style license that can be
// found in the LICENSE file.

import 'package:pinepods_mobile/bloc/podcast/audio_bloc.dart';
import 'package:pinepods_mobile/bloc/settings/settings_bloc.dart';
import 'package:pinepods_mobile/entities/app_settings.dart';
import 'package:pinepods_mobile/entities/sleep.dart';
import 'package:pinepods_mobile/l10n/L.dart';
import 'package:pinepods_mobile/ui/widgets/slider_handle.dart';
import 'package:flutter/material.dart';
import 'package:provider/provider.dart';

/// This widget allows the user to change the playback speed and toggle audio effects.
///
/// The two audio effects, trim silence and volume boost, are currently Android only.
class SleepSelectorWidget extends StatefulWidget {
  const SleepSelectorWidget({
    super.key,
  });

  @override
  State<SleepSelectorWidget> createState() => _SleepSelectorWidgetState();
}

class _SleepSelectorWidgetState extends State<SleepSelectorWidget> {
  @override
  Widget build(BuildContext context) {
    final audioBloc = Provider.of<AudioBloc>(context, listen: false);
    final settingsBloc = Provider.of<SettingsBloc>(context);
    var theme = Theme.of(context);

    return StreamBuilder<AppSettings>(
        stream: settingsBloc.settings,
        initialData: AppSettings.sensibleDefaults(),
        builder: (context, snapshot) {
          return Column(
            mainAxisAlignment: MainAxisAlignment.center,
            crossAxisAlignment: CrossAxisAlignment.center,
            mainAxisSize: MainAxisSize.min,
            children: [
              SizedBox(
                height: 48.0,
                width: 48.0,
                child: Center(
                  child: StreamBuilder<Sleep>(
                    stream: audioBloc.sleepStream,
                    initialData: Sleep(type: SleepType.none),
                    builder: (context, sleepSnapshot) {
                      var sl = '';

                      if (sleepSnapshot.hasData) {
                        var s = sleepSnapshot.data!;

                        switch(s.type) {
                          case SleepType.none:
                            sl = '';
                          case SleepType.time:
                            sl = '${L.of(context)!.now_playing_episode_time_remaining} ${SleepSlider.formatDuration(s.timeRemaining)}';
                          case SleepType.episode:
                            sl = '${L.of(context)!.semantic_current_value_label} ${L.of(context)!.sleep_episode_label}';
                        }
                      }

                      return IconButton(
                        icon: sleepSnapshot.data?.type != SleepType.none ? Icon(
                          Icons.bedtime,
                          semanticLabel: '${L.of(context)!.sleep_timer_label}. $sl',
                          size: 20.0,
                        ) : Icon(
                          Icons.bedtime_outlined,
                          semanticLabel: L.of(context)!.sleep_timer_label,
                          size: 20.0,
                        ),
                        onPressed: () {
                          showModalBottomSheet<void>(
                              isScrollControlled: true,
                              context: context,
                              backgroundColor: theme.secondaryHeaderColor,
                              barrierLabel: L.of(context)!.scrim_sleep_timer_selector,
                              shape: const RoundedRectangleBorder(
                                borderRadius: BorderRadius.only(
                                  topLeft: Radius.circular(16.0),
                                  topRight: Radius.circular(16.0),
                                ),
                              ),
                              builder: (context) {
                                return const SleepSlider();
                              });
                        },
                      );
                    }
                  ),
                ),
              ),
            ],
          );
        });
  }
}

class SleepSlider extends StatefulWidget {
  const SleepSlider({super.key});

  static String formatDuration(Duration duration) {
    String twoDigits(int n) {
      if (n >= 10) return '$n';
      return '0$n';
    }

    var twoDigitMinutes = twoDigits(duration.inMinutes.remainder(60).toInt());
    var twoDigitSeconds = twoDigits(duration.inSeconds.remainder(60).toInt());

    return '${twoDigits(duration.inHours)}:$twoDigitMinutes:$twoDigitSeconds';
  }

  @override
  State<SleepSlider> createState() => _SleepSliderState();
}

class _SleepSliderState extends State<SleepSlider> {
  @override
  Widget build(BuildContext context) {
    final audioBloc = Provider.of<AudioBloc>(context, listen: false);

    return StreamBuilder<Sleep>(
        stream: audioBloc.sleepStream,
        initialData: Sleep(type: SleepType.none),
        builder: (context, snapshot) {
          var s = snapshot.data;

          return Column(
              mainAxisSize: MainAxisSize.min,
              mainAxisAlignment: MainAxisAlignment.start,
              crossAxisAlignment: CrossAxisAlignment.center,
              children: <Widget>[
                const SliderHandle(),
                Padding(
                  padding: const EdgeInsets.only(top: 8.0, bottom: 8.0),
                  child: Semantics(
                    header: true,
                    child: Text(
                      L.of(context)!.sleep_timer_label,
                      style: Theme.of(context).textTheme.titleLarge,
                    ),
                  ),
                ),
                if (s != null && s.type == SleepType.none)
                  Text(
                    '(${L.of(context)!.sleep_off_label})',
                    semanticsLabel: '${L.of(context)!.semantic_current_value_label} ${L.of(context)!.sleep_off_label}',
                    style: Theme.of(context).textTheme.bodyLarge,
                  ),
                if (s != null && s.type == SleepType.time)
                  Text(
                    '(${SleepSlider.formatDuration(s.timeRemaining)})',
                    semanticsLabel:
                        '${L.of(context)!.semantic_current_value_label} ${SleepSlider.formatDuration(s.timeRemaining)}',
                    style: Theme.of(context).textTheme.bodyLarge,
                  ),
                if (s != null && s.type == SleepType.episode)
                  Text(
                    '(${L.of(context)!.sleep_episode_label})',
                    semanticsLabel:
                        '${L.of(context)!.semantic_current_value_label} ${L.of(context)!.sleep_episode_label}',
                    style: Theme.of(context).textTheme.bodyLarge,
                  ),
                Padding(
                  padding: const EdgeInsets.all(16.0),
                  child: ListView(
                    shrinkWrap: true,
                    children: [
                      SleepSelectorEntry(
                        sleep: Sleep(type: SleepType.none),
                        current: s,
                      ),
                      const Divider(),
                      SleepSelectorEntry(
                        sleep: Sleep(
                          type: SleepType.time,
                          duration: const Duration(minutes: 5),
                        ),
                        current: s,
                      ),
                      const Divider(),
                      SleepSelectorEntry(
                        sleep: Sleep(
                          type: SleepType.time,
                          duration: const Duration(minutes: 10),
                        ),
                        current: s,
                      ),
                      const Divider(),
                      SleepSelectorEntry(
                        sleep: Sleep(
                          type: SleepType.time,
                          duration: const Duration(minutes: 15),
                        ),
                        current: s,
                      ),
                      const Divider(),
                      SleepSelectorEntry(
                        sleep: Sleep(
                          type: SleepType.time,
                          duration: const Duration(minutes: 30),
                        ),
                        current: s,
                      ),
                      const Divider(),
                      SleepSelectorEntry(
                        sleep: Sleep(
                          type: SleepType.time,
                          duration: const Duration(minutes: 45),
                        ),
                        current: s,
                      ),
                      const Divider(),
                      SleepSelectorEntry(
                        sleep: Sleep(
                          type: SleepType.time,
                          duration: const Duration(minutes: 60),
                        ),
                        current: s,
                      ),
                      const Divider(),
                      SleepSelectorEntry(
                        sleep: Sleep(
                          type: SleepType.episode,
                        ),
                        current: s,
                      ),
                    ],
                  ),
                )
              ]);
        });
  }
}

class SleepSelectorEntry extends StatelessWidget {
  const SleepSelectorEntry({
    super.key,
    required this.sleep,
    required this.current,
  });

  final Sleep sleep;
  final Sleep? current;

  @override
  Widget build(BuildContext context) {
    final audioBloc = Provider.of<AudioBloc>(context, listen: false);

    return GestureDetector(
      behavior: HitTestBehavior.translucent,
      onTap: () {
        audioBloc.sleep(Sleep(
          type: sleep.type,
          duration: sleep.duration,
        ));

        Navigator.pop(context);
      },
      child: Padding(
        padding: const EdgeInsets.only(
          top: 4.0,
          bottom: 4.0,
        ),
        child: Row(
          mainAxisAlignment: MainAxisAlignment.spaceBetween,
          mainAxisSize: MainAxisSize.max,
          children: [
            if (sleep.type == SleepType.none)
              Text(
                L.of(context)!.sleep_off_label,
                style: Theme.of(context).textTheme.bodyLarge,
              ),
            if (sleep.type == SleepType.time)
              Text(
                L.of(context)!.sleep_minute_label(sleep.duration.inMinutes.toString()),
                style: Theme.of(context).textTheme.bodyLarge,
              ),
            if (sleep.type == SleepType.episode)
              Text(
                L.of(context)!.sleep_episode_label,
                style: Theme.of(context).textTheme.bodyLarge,
              ),
            if (sleep == current)
              const Icon(
                Icons.check,
                size: 18.0,
              ),
          ],
        ),
      ),
    );
  }
}
