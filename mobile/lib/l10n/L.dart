// Copyright 2020 Ben Hills and the project contributors. All rights reserved.
// Use of this source code is governed by a BSD-style license that can be
// found in the LICENSE file.

import 'package:flutter/widgets.dart';
import 'package:intl/intl.dart';

import 'messages_all.dart';

class L {
  L(this.localeName, this.overrides);

  static Future<L> load(
    Locale locale,
    Map<String, Map<String, String>> overrides,
  ) {
    final name = locale.countryCode?.isEmpty ?? true
        ? locale.languageCode
        : locale.toString();
    final localeName = Intl.canonicalizedLocale(name);

    return initializeMessages(localeName).then((_) {
      return L(localeName, overrides);
    });
  }

  static L? of(BuildContext context) {
    return Localizations.of<L>(context, L);
  }

  final String localeName;
  Map<String, Map<String, String>> overrides;

  /// Message definitions start here
  String? message(String name) {
    if (overrides == null ||
        overrides.isEmpty ||
        !overrides.containsKey(name)) {
      return null;
    } else {
      return overrides[name]![localeName] ??
          'Missing translation for $name and locale $localeName';
    }
  }

  /// General
  String get app_title {
    return message('app_title') ??
        Intl.message(
          'Pinepods Podcast Client',
          name: 'app_title',
          desc: 'Full title for the application',
          locale: localeName,
        );
  }

  String get app_title_short {
    return message('app_title_short') ??
        Intl.message(
          'Pinepods',
          name: 'app_title_short',
          desc: 'Title for the application',
          locale: localeName,
        );
  }

  String get library {
    return message('library') ??
        Intl.message(
          'Library',
          name: 'library',
          desc: 'Library tab label',
          locale: localeName,
        );
  }

  String get discover {
    return message('discover') ??
        Intl.message(
          'Discover',
          name: 'discover',
          desc: 'Discover tab label',
          locale: localeName,
        );
  }

  String get downloads {
    return message('downloads') ??
        Intl.message(
          'Downloads',
          name: 'downloads',
          desc: 'Downloads tab label',
          locale: localeName,
        );
  }

  /// Podcasts
  String get subscribe_button_label {
    return message('subscribe_button_label') ??
        Intl.message(
          'Follow',
          name: 'subscribe_button_label',
          desc: 'Subscribe button label',
          locale: localeName,
        );
  }

  String get unsubscribe_button_label {
    return message('unsubscribe_button_label') ??
        Intl.message(
          'Unfollow',
          name: 'unsubscribe_button_label',
          desc: 'Unsubscribe button label',
          locale: localeName,
        );
  }

  String get cancel_button_label {
    return message('cancel_button_label') ??
        Intl.message(
          'Cancel',
          name: 'cancel_button_label',
          desc: 'Cancel button label',
          locale: localeName,
        );
  }

  String get ok_button_label {
    return message('ok_button_label') ??
        Intl.message(
          'OK',
          name: 'ok_button_label',
          desc: 'OK button label',
          locale: localeName,
        );
  }

  String get subscribe_label {
    return message('subscribe_label') ??
        Intl.message(
          'Follow',
          name: 'subscribe_label',
          desc: 'Subscribe label',
          locale: localeName,
        );
  }

  String get unsubscribe_label {
    return message('unsubscribe_label') ??
        Intl.message(
          'Unfollow',
          name: 'unsubscribe_label',
          desc: 'Unsubscribe label',
          locale: localeName,
        );
  }

  String get unsubscribe_message {
    return message('unsubscribe_message') ??
        Intl.message(
          'Unfollowing will delete all downloaded episodes of this podcast.',
          name: 'unsubscribe_message',
          desc: 'Displayed when the user unfollows a podcast.',
          locale: localeName,
        );
  }

  String get search_for_podcasts_hint {
    return message('search_for_podcasts_hint') ??
        Intl.message(
          'Search for podcasts',
          name: 'search_for_podcasts_hint',
          desc:
              'Hint displayed on search bar when the user clicks the search icon.',
          locale: localeName,
        );
  }

  String get no_subscriptions_message {
    return message('no_subscriptions_message') ??
        Intl.message(
          'Head to Settings to Connect a Pinepods Server if you haven\'t yet!',
          name: 'no_subscriptions_message',
          desc:
              'Displayed on the library tab when the user has no subscriptions',
          locale: localeName,
        );
  }

  String get delete_label {
    return message('delete_label') ??
        Intl.message(
          'Delete',
          name: 'delete_label',
          desc: 'Delete label',
          locale: localeName,
        );
  }

  String get delete_button_label {
    return message('delete_button_label') ??
        Intl.message(
          'Delete',
          name: 'delete_button_label',
          desc: 'Delete label',
          locale: localeName,
        );
  }

  String get mark_played_label {
    return message('mark_played_label') ??
        Intl.message(
          'Mark Played',
          name: 'mark_played_label',
          desc: 'Mark as played',
          locale: localeName,
        );
  }

  String get mark_unplayed_label {
    return message('mark_unplayed_label') ??
        Intl.message(
          'Mark Unplayed',
          name: 'mark_unplayed_label',
          desc: 'Mark as unplayed',
          locale: localeName,
        );
  }

  String get delete_episode_confirmation {
    return message('delete_episode_confirmation') ??
        Intl.message(
          'Are you sure you wish to delete this episode?',
          name: 'delete_episode_confirmation',
          desc:
              'User is asked to confirm when they attempt to delete an episode',
          locale: localeName,
        );
  }

  String get delete_episode_title {
    return message('delete_episode_title') ??
        Intl.message(
          'Delete Episode',
          name: 'delete_episode_title',
          desc: 'Delete label',
          locale: localeName,
        );
  }

  String get no_downloads_message {
    return message('no_downloads_message') ??
        Intl.message(
          'You do not have any downloaded episodes',
          name: 'no_downloads_message',
          desc:
              'Displayed on the library tab when the user has no subscriptions',
          locale: localeName,
        );
  }

  String get no_search_results_message {
    return message('no_search_results_message') ??
        Intl.message(
          'No podcasts found',
          name: 'no_search_results_message',
          desc:
              'Displayed on the library tab when the user has no subscriptions',
          locale: localeName,
        );
  }

  String get no_podcast_details_message {
    return message('no_podcast_details_message') ??
        Intl.message(
          'Could not load podcast episodes. Please check your connection.',
          name: 'no_podcast_details_message',
          desc:
              'Displayed on the podcast details page when the details could not be loaded',
          locale: localeName,
        );
  }

  String get play_button_label {
    return message('play_button_label') ??
        Intl.message(
          'Play episode',
          name: 'play_button_label',
          desc: 'Semantic label for the play button',
          locale: localeName,
        );
  }

  String get pause_button_label {
    return message('pause_button_label') ??
        Intl.message(
          'Pause episode',
          name: 'pause_button_label',
          desc: 'Semantic label for the pause button',
          locale: localeName,
        );
  }

  String get download_episode_button_label {
    return message('download_episode_button_label') ??
        Intl.message(
          'Download episode',
          name: 'download_episode_button_label',
          desc: 'Semantic label for the download episode button',
          locale: localeName,
        );
  }

  String get delete_episode_button_label {
    return message('delete_episode_button_label') ??
        Intl.message(
          'Delete downloaded episode',
          name: 'delete_episode_button_label',
          desc: 'Semantic label for the delete episode',
          locale: localeName,
        );
  }

  String get close_button_label {
    return message('close_button_label') ??
        Intl.message(
          'Close',
          name: 'close_button_label',
          desc: 'Close button label',
          locale: localeName,
        );
  }

  String get search_button_label {
    return message('search_button_label') ??
        Intl.message(
          'Search',
          name: 'search_button_label',
          desc: 'Search button label',
          locale: localeName,
        );
  }

  String get clear_search_button_label {
    return message('clear_search_button_label') ??
        Intl.message(
          'Clear search text',
          name: 'clear_search_button_label',
          desc: 'Search button label',
          locale: localeName,
        );
  }

  String get search_back_button_label {
    return message('search_back_button_label') ??
        Intl.message(
          'Back',
          name: 'search_back_button_label',
          desc: 'Search button label',
          locale: localeName,
        );
  }

  String get minimise_player_window_button_label {
    return message('minimise_player_window_button_label') ??
        Intl.message(
          'Minimise player window',
          name: 'minimise_player_window_button_label',
          desc: 'Search button label',
          locale: localeName,
        );
  }

  String get rewind_button_label {
    return message('rewind_button_label') ??
        Intl.message(
          'Rewind episode 10 seconds',
          name: 'rewind_button_label',
          desc: 'Rewind button tooltip',
          locale: localeName,
        );
  }

  String get fast_forward_button_label {
    return message('fast_forward_button_label') ??
        Intl.message(
          'Fast-forward episode 30 seconds',
          name: 'fast_forward_button_label',
          desc: 'Fast forward tooltip',
          locale: localeName,
        );
  }

  String get about_label {
    return message('about_label') ??
        Intl.message(
          'About',
          name: 'about_label',
          desc: 'About menu item',
          locale: localeName,
        );
  }

  String get mark_episodes_played_label {
    return message('mark_episodes_played_label') ??
        Intl.message(
          'Mark all episodes as played',
          name: 'mark_episodes_played_label',
          desc: 'Mark all episodes played menu item',
          locale: localeName,
        );
  }

  String get mark_episodes_not_played_label {
    return message('mark_episodes_not_played_label') ??
        Intl.message(
          'Mark all episodes as not played',
          name: 'mark_episodes_not_played_label',
          desc: 'Mark all episodes not played menu item',
          locale: localeName,
        );
  }

  String get stop_download_confirmation {
    return message('stop_download_confirmation') ??
        Intl.message(
          'Are you sure you wish to stop this download and delete the episode?',
          name: 'stop_download_confirmation',
          desc:
              'User is asked to confirm when they wish to stop the active download.',
          locale: localeName,
        );
  }

  String get stop_download_button_label {
    return message('stop_download_button_label') ??
        Intl.message(
          'Stop',
          name: 'stop_download_button_label',
          desc: 'Stop label',
          locale: localeName,
        );
  }

  String get stop_download_title {
    return message('stop_download_title') ??
        Intl.message(
          'Stop Download',
          name: 'stop_download_title',
          desc: 'Stop download label',
          locale: localeName,
        );
  }

  String get settings_mark_deleted_played_label {
    return message('settings_mark_deleted_played_label') ??
        Intl.message(
          'Mark deleted episodes as played',
          name: 'settings_mark_deleted_played_label',
          desc: 'Mark deleted episodes as played setting',
          locale: localeName,
        );
  }

  String get settings_delete_played_label {
    return message('settings_delete_played_label') ??
        Intl.message(
          'Delete downloaded episodes once played',
          name: 'settings_delete_played_label',
          desc: 'Delete downloaded episodes once played setting',
          locale: localeName,
        );
  }

  String get settings_download_sd_card_label {
    return message('settings_download_sd_card_label') ??
        Intl.message(
          'Download episodes to SD card',
          name: 'settings_download_sd_card_label',
          desc: 'Download episodes to SD card setting',
          locale: localeName,
        );
  }

  String get settings_download_switch_card {
    return message('settings_download_switch_card') ??
        Intl.message(
          'New downloads will be saved to the SD card. Existing downloads will remain on internal storage.',
          name: 'settings_download_switch_card',
          desc: 'Displayed when user switches from internal storage to SD card',
          locale: localeName,
        );
  }

  String get settings_download_switch_internal {
    return message('settings_download_switch_internal') ??
        Intl.message(
          'New downloads will be saved to internal storage. Existing downloads will remain on the SD card.',
          name: 'settings_download_switch_internal',
          desc:
              'Displayed when user switches from internal SD card to internal storage',
          locale: localeName,
        );
  }

  String get settings_download_switch_label {
    return message('settings_download_switch_label') ??
        Intl.message(
          'Change storage location',
          name: 'settings_download_switch_label',
          desc: 'Dialog label for storage switch',
          locale: localeName,
        );
  }

  String get cancel_option_label {
    return message('cancel_option_label') ??
        Intl.message(
          'Cancel',
          name: 'cancel_option_label',
          desc: 'Cancel option label',
          locale: localeName,
        );
  }

  String get settings_theme_switch_label {
    return message('settings_theme_switch_label') ??
        Intl.message(
          'Dark theme',
          name: 'settings_theme_switch_label',
          desc: 'Dark theme',
          locale: localeName,
        );
  }

  String get playback_speed_label {
    return message('playback_speed_label') ??
        Intl.message(
          'Playback speed',
          name: 'playback_speed_label',
          desc: 'Set playback speed icon label',
          locale: localeName,
        );
  }

  String get show_notes_label {
    return message('show_notes_label') ??
        Intl.message(
          'Show notes',
          name: 'show_notes_label',
          desc: 'Set show notes icon label',
          locale: localeName,
        );
  }

  String get search_provider_label {
    return message('search_provider_label') ??
        Intl.message(
          'Search provider',
          name: 'search_provider_label',
          desc: 'Set search provider label',
          locale: localeName,
        );
  }

  String get settings_label {
    return message('settings_label') ??
        Intl.message(
          'Settings',
          name: 'settings_label',
          desc: 'Settings label',
          locale: localeName,
        );
  }

  String get go_back_button_label {
    return message('go_back_button_label') ??
        Intl.message(
          'Go Back',
          name: 'go_back_button_label',
          desc: 'Go-back button label',
          locale: localeName,
        );
  }

  String get continue_button_label {
    return message('continue_button_label') ??
        Intl.message(
          'Continue',
          name: 'continue_button_label',
          desc: 'Continue button label',
          locale: localeName,
        );
  }

  String get consent_message {
    return message('consent_message') ??
        Intl.message(
          'This funding link will take you to an external site where you will be able to directly support the show. Links are provided by the podcast authors and is not controlled by PinePods.',
          name: 'consent_message',
          desc: 'Display when first accessing external funding link',
          locale: localeName,
        );
  }

  String get episode_label {
    return message('episode_label') ??
        Intl.message(
          'Episode',
          name: 'episode_label',
          desc: 'Tab label on now playing screen.',
          locale: localeName,
        );
  }

  String get chapters_label {
    return message('chapters_label') ??
        Intl.message(
          'Chapters',
          name: 'chapters_label',
          desc: 'Tab label on now playing screen.',
          locale: localeName,
        );
  }

  String get notes_label {
    return message('notes_label') ??
        Intl.message(
          'Notes',
          name: 'notes_label',
          desc: 'Tab label on now playing screen.',
          locale: localeName,
        );
  }

  String get podcast_funding_dialog_header {
    return message('podcast_funding_dialog_header') ??
        Intl.message(
          'Podcast Funding',
          name: 'podcast_funding_dialog_header',
          desc: 'Header on podcast funding consent dialog',
          locale: localeName,
        );
  }

  String get settings_auto_open_now_playing {
    return message('settings_auto_open_now_playing') ??
        Intl.message(
          'Full screen player mode on episode start',
          name: 'settings_auto_open_now_playing',
          desc:
              'Displayed when user switches to use full screen player automatically',
          locale: localeName,
        );
  }

  String get error_no_connection {
    return message('error_no_connection') ??
        Intl.message(
          'Unable to play episode. Please check your connection and try again.',
          name: 'error_no_connection',
          desc:
              'Displayed when attempting to start streaming an episode with no data connection',
          locale: localeName,
        );
  }

  String get error_playback_fail {
    return message('error_playback_fail') ??
        Intl.message(
          'An unexpected error occurred during playback. Please check your connection and try again.',
          name: 'error_playback_fail',
          desc:
              'Displayed when attempting to start streaming an episode with no data connection',
          locale: localeName,
        );
  }

  String get add_rss_feed_option {
    return message('add_rss_feed_option') ??
        Intl.message(
          'Add RSS Feed',
          name: 'add_rss_feed_option',
          desc: 'Option label for adding manual RSS feed url',
          locale: localeName,
        );
  }

  String get settings_import_opml {
    return message('settings_import_opml') ??
        Intl.message(
          'Import OPML',
          name: 'settings_import_opml',
          desc: 'Option label importing OPML file',
          locale: localeName,
        );
  }

  String get settings_export_opml {
    return message('settings_export_opml') ??
        Intl.message(
          'Export OPML',
          name: 'settings_export_opml',
          desc: 'Option label exporting OPML file',
          locale: localeName,
        );
  }

  String get label_opml_importing {
    return message('label_opml_importing') ??
        Intl.message(
          'Importing',
          name: 'label_opml_importing',
          desc: 'Label for importing OPML dialog',
          locale: localeName,
        );
  }

  String get settings_auto_update_episodes {
    return message('settings_auto_update_episodes') ??
        Intl.message(
          'Auto update episodes',
          name: 'settings_auto_update_episodes',
          desc: 'Option label for auto updating of episodes',
          locale: localeName,
        );
  }

  String get settings_auto_update_episodes_never {
    return message('settings_auto_update_episodes_never') ??
        Intl.message(
          'Never',
          name: 'settings_auto_update_episodes_never',
          desc: 'Option label for auto updating of episodes',
          locale: localeName,
        );
  }

  String get settings_auto_update_episodes_heading {
    return message('settings_auto_update_episodes_heading') ??
        Intl.message(
          'Refresh episodes on details screen after',
          name: 'settings_auto_update_episodes_heading',
          desc: 'Option label for auto updating of episodes',
          locale: localeName,
        );
  }

  String get settings_auto_update_episodes_always {
    return message('settings_auto_update_episodes_always') ??
        Intl.message(
          'Always',
          name: 'settings_auto_update_episodes_always',
          desc: 'Option label for auto updating of episodes',
          locale: localeName,
        );
  }

  String get settings_auto_update_episodes_10min {
    return message('settings_auto_update_episodes_10min') ??
        Intl.message(
          '10 minutes since last update',
          name: 'settings_auto_update_episodes_10min',
          desc: 'Option label for auto updating of episodes',
          locale: localeName,
        );
  }

  String get settings_auto_update_episodes_30min {
    return message('settings_auto_update_episodes_30min') ??
        Intl.message(
          '30 minutes since last update',
          name: 'settings_auto_update_episodes_30min',
          desc: 'Option label for auto updating of episodes',
          locale: localeName,
        );
  }

  String get settings_auto_update_episodes_1hour {
    return message('settings_auto_update_episodes_1hour') ??
        Intl.message(
          '1 hour since last update',
          name: 'settings_auto_update_episodes_1hour',
          desc: 'Option label for auto updating of episodes',
          locale: localeName,
        );
  }

  String get settings_auto_update_episodes_3hour {
    return message('settings_auto_update_episodes_3hour') ??
        Intl.message(
          '3 hours since last update',
          name: 'settings_auto_update_episodes_3hour',
          desc: 'Option label for auto updating of episodes',
          locale: localeName,
        );
  }

  String get settings_auto_update_episodes_6hour {
    return message('settings_auto_update_episodes_6hour') ??
        Intl.message(
          '6 hours since last update',
          name: 'settings_auto_update_episodes_6hour',
          desc: 'Option label for auto updating of episodes',
          locale: localeName,
        );
  }

  String get settings_auto_update_episodes_12hour {
    return message('settings_auto_update_episodes_12hour') ??
        Intl.message(
          '12 hours since last update',
          name: 'settings_auto_update_episodes_12hour',
          desc: 'Option label for auto updating of episodes',
          locale: localeName,
        );
  }

  String get new_episodes_label {
    return message('new_episodes_label') ??
        Intl.message(
          'New episodes are available',
          name: 'new_episodes_label',
          desc: 'Option label for auto updating of episodes',
          locale: localeName,
        );
  }

  String get new_episodes_view_now_label {
    return message('new_episodes_view_now_label') ??
        Intl.message(
          'VIEW NOW',
          name: 'new_episodes_view_now_label',
          desc: 'Option label for auto updating of episodes',
          locale: localeName,
        );
  }

  String get settings_personalisation_divider_label {
    return message('settings_personalisation_divider_label') ??
        Intl.message(
          'PERSONALISATION',
          name: 'settings_personalisation_divider_label',
          desc: 'Settings divider label for personalisation',
          locale: localeName,
        );
  }

  String get settings_episodes_divider_label {
    return message('settings_episodes_divider_label') ??
        Intl.message(
          'EPISODES',
          name: 'settings_episodes_divider_label',
          desc: 'Settings divider label for episodes',
          locale: localeName,
        );
  }

  String get settings_playback_divider_label {
    return message('settings_playback_divider_label') ??
        Intl.message(
          'PLAYBACK',
          name: 'settings_playback_divider_label',
          desc: 'Settings divider label for playback',
          locale: localeName,
        );
  }

  String get settings_data_divider_label {
    return message('settings_data_divider_label') ??
        Intl.message(
          'DATA',
          name: 'settings_data_divider_label',
          desc: 'Settings divider label for data',
          locale: localeName,
        );
  }

  String get audio_effect_trim_silence_label {
    return message('audio_effect_trim_silence_label') ??
        Intl.message(
          'Trim Silence',
          name: 'audio_effect_trim_silence_label',
          desc: 'Label for trim silence toggle',
          locale: localeName,
        );
  }

  String get audio_effect_volume_boost_label {
    return message('audio_effect_volume_boost_label') ??
        Intl.message(
          'Volume Boost',
          name: 'audio_effect_volume_boost_label',
          desc: 'Label for volume boost toggle',
          locale: localeName,
        );
  }

  String get audio_settings_playback_speed_label {
    return message('audio_settings_playback_speed_label') ??
        Intl.message(
          'Playback Speed',
          name: 'audio_settings_playback_speed_label',
          desc: 'Label for playback settings widget',
          locale: localeName,
        );
  }

  String get empty_queue_message {
    return message('empty_queue_message') ??
        Intl.message(
          'Your queue is empty',
          name: 'empty_queue_message',
          desc: 'Displayed when there are no items left in the queue',
          locale: localeName,
        );
  }

  String get clear_queue_button_label {
    return message('clear_queue_button_label') ??
        Intl.message(
          'CLEAR QUEUE',
          name: 'clear_queue_button_label',
          desc: 'Clear queue button label',
          locale: localeName,
        );
  }

  String get now_playing_queue_label {
    return message('now_playing_queue_label') ??
        Intl.message(
          'Now Playing',
          name: 'now_playing_queue_label',
          desc: 'Now playing label on queue',
          locale: localeName,
        );
  }

  String get up_next_queue_label {
    return message('up_next_queue_label') ??
        Intl.message(
          'Up Next',
          name: 'up_next_queue_label',
          desc: 'Up next label on queue',
          locale: localeName,
        );
  }

  String get more_label {
    return message('more_label') ??
        Intl.message(
          'More',
          name: 'more_label',
          desc: 'More label',
          locale: localeName,
        );
  }

  String get queue_add_label {
    return message('queue_add_label') ??
        Intl.message(
          'Add',
          name: 'queue_add_label',
          desc: 'Queue add label',
          locale: localeName,
        );
  }

  String get queue_remove_label {
    return message('queue_remove_label') ??
        Intl.message(
          'Remove',
          name: 'queue_remove_label',
          desc: 'Queue remove label',
          locale: localeName,
        );
  }

  String get opml_import_button_label {
    return message('opml_import_button_label') ??
        Intl.message(
          'Import',
          name: 'opml_import_button_label',
          desc: 'OPML Import button label',
          locale: localeName,
        );
  }

  String get opml_export_button_label {
    return message('opml_export_button_label') ??
        Intl.message(
          'Export',
          name: 'opml_export_button_label',
          desc: 'OPML Export button label',
          locale: localeName,
        );
  }

  String get opml_import_export_label {
    return message('opml_import_export_label') ??
        Intl.message(
          'OPML Import/Export',
          name: 'opml_import_export_label',
          desc: 'OPML Import/Export label',
          locale: localeName,
        );
  }

  String get queue_clear_label {
    return message('queue_clear_label') ??
        Intl.message(
          'Are you sure you wish to clear the queue?',
          name: 'queue_clear_label',
          desc: 'Shown on dialog box when clearing queue',
          locale: localeName,
        );
  }

  String get queue_clear_button_label {
    return message('queue_clear_button_label') ??
        Intl.message(
          'Clear',
          name: 'queue_clear_button_label',
          desc: 'Shown on dialog box when clearing queue',
          locale: localeName,
        );
  }

  String get queue_clear_label_title {
    return message('queue_clear_label_title') ??
        Intl.message(
          'Clear Queue',
          name: 'queue_clear_label_title',
          desc: 'Shown on dialog box when clearing queue',
          locale: localeName,
        );
  }

  String get layout_label {
    return message('layout_label') ??
        Intl.message(
          'Layout',
          name: 'layout_label',
          desc: 'Layout menu label',
          locale: localeName,
        );
  }

  String get discovery_categories_itunes {
    return message('discovery_categories_itunes') ??
        Intl.message(
          '<All>,Arts,Business,Comedy,Education,Fiction,Government,Health & Fitness,History,Kids & Family,Leisure,Music,News,Religion & Spirituality,Science,Society & Culture,Sports,TV & Film,Technology,True Crime',
          name: 'discovery_categories_itunes',
          desc: 'Comma separated list of iTunes categories',
          locale: localeName,
        );
  }

  String get discovery_categories_pindex {
    return message('discovery_categories_pindex') ??
        Intl.message(
          '<All>,After-Shows,Alternative,Animals,Animation,Arts,Astronomy,Automotive,Aviation,Baseball,Basketball,Beauty,Books,Buddhism,Business,Careers,Chemistry,Christianity,Climate,Comedy,Commentary,Courses,Crafts,Cricket,Cryptocurrency,Culture,Daily,Design,Documentary,Drama,Earth,Education,Entertainment,Entrepreneurship,Family,Fantasy,Fashion,Fiction,Film,Fitness,Food,Football,Games,Garden,Golf,Government,Health,Hinduism,History,Hobbies,Hockey,Home,HowTo,Improv,Interviews,Investing,Islam,Journals,Judaism,Kids,Language,Learning,Leisure,Life,Management,Manga,Marketing,Mathematics,Medicine,Mental,Music,Natural,Nature,News,NonProfit,Nutrition,Parenting,Performing,Personal,Pets,Philosophy,Physics,Places,Politics,Relationships,Religion,Reviews,Role-Playing,Rugby,Running,Science,Self-Improvement,Sexuality,Soccer,Social,Society,Spirituality,Sports,Stand-Up,Stories,Swimming,TV,Tabletop,Technology,Tennis,Travel,True Crime,Video-Games,Visual,Volleyball,Weather,Wilderness,Wrestling',
          name: 'discovery_categories_pindex',
          desc: 'Comma separated list of Podcast Index categories',
          locale: localeName,
        );
  }

  String get transcript_label {
    return message('transcript_label') ??
        Intl.message(
          'Transcript',
          name: 'transcript_label',
          desc: 'Transcript label',
          locale: localeName,
        );
  }

  String get no_transcript_available_label {
    return message('no_transcript_available_label') ??
        Intl.message(
          'A transcript is not available for this podcast',
          name: 'no_transcript_available_label',
          desc: 'Displayed in transcript view when no transcript is available',
          locale: localeName,
        );
  }

  String get search_transcript_label {
    return message('search_transcript_label') ??
        Intl.message(
          'Search transcript',
          name: 'search_transcript_label',
          desc: 'Hint text for transcript search box',
          locale: localeName,
        );
  }

  String get auto_scroll_transcript_label {
    return message('auto_scroll_transcript_label') ??
        Intl.message(
          'Follow transcript',
          name: 'auto_scroll_transcript_label',
          desc: 'Auto scroll switch label',
          locale: localeName,
        );
  }

  String get transcript_why_not_label {
    return message('transcript_why_not_label') ??
        Intl.message(
          'Why not?',
          name: 'transcript_why_not_label',
          desc: 'Link to why no transcript is available',
          locale: localeName,
        );
  }

  String get transcript_why_not_url {
    return message('transcript_why_not_url') ??
        Intl.message(
          'https://www.pinepods.online/docs/Features/Transcript',
          name: 'transcript_why_not_url',
          desc: 'Language specific link',
          locale: localeName,
        );
  }

  String get semantics_podcast_details_header {
    return message('semantics_podcast_details_header') ??
        Intl.message(
          'Podcast details and episodes page',
          name: 'semantics_podcast_details_header',
          desc: 'Describes podcast details page',
          locale: localeName,
        );
  }

  String get semantics_layout_option_list {
    return message('semantics_layout_option_list') ??
        Intl.message(
          'List layout',
          name: 'semantics_layout_option_list',
          desc: 'Describes list layout button',
          locale: localeName,
        );
  }

  String get semantics_layout_option_compact_grid {
    return message('semantics_layout_option_compact_grid') ??
        Intl.message(
          'Compact grid layout',
          name: 'semantics_layout_option_compact_grid',
          desc: 'Describes compact grid layout button',
          locale: localeName,
        );
  }

  String get semantics_layout_option_grid {
    return message('semantics_layout_option_grid') ??
        Intl.message(
          'Grid layout',
          name: 'semantics_layout_option_grid',
          desc: 'Describes grid layout button',
          locale: localeName,
        );
  }

  String get semantics_mini_player_header {
    return message('semantics_mini_player_header') ??
        Intl.message(
          'Mini player. Swipe right to play/pause button. Activate to open main player window',
          name: 'semantics_mini_player_header',
          desc: 'Describes the mini player',
          locale: localeName,
        );
  }

  String get semantics_main_player_header {
    return message('semantics_main_player_header') ??
        Intl.message(
          'Main player window',
          name: 'semantics_main_player_header',
          desc: 'Describes the main player',
          locale: localeName,
        );
  }

  String get semantics_play_pause_toggle {
    return message('semantics_play_pause_toggle') ??
        Intl.message(
          'Play/pause toggle',
          name: 'semantics_play_pause_toggle',
          desc: 'Describes play/pause toggle button',
          locale: localeName,
        );
  }

  String get semantics_decrease_playback_speed {
    return message('semantics_decrease_playback_speed') ??
        Intl.message(
          'Decrease playback speed',
          name: 'semantics_decrease_playback_speed',
          desc: 'Describes speed adjustment control',
          locale: localeName,
        );
  }

  String get semantics_increase_playback_speed {
    return message('semantics_increase_playback_speed') ??
        Intl.message(
          'Increase playback speed',
          name: 'semantics_increase_playback_speed',
          desc: 'Describes speed adjustment control',
          locale: localeName,
        );
  }

  String get semantics_expand_podcast_description {
    return message('semantics_expand_podcast_description') ??
        Intl.message(
          'Expand podcast description',
          name: 'semantics_expand_podcast_description',
          desc: 'Describes podcast collapse/expand button',
          locale: localeName,
        );
  }

  String get semantics_collapse_podcast_description {
    return message('semantics_collapse_podcast_description') ??
        Intl.message(
          'Collapse podcast description',
          name: 'semantics_collapse_podcast_description',
          desc: 'Describes podcast collapse/expand button',
          locale: localeName,
        );
  }

  String get semantics_add_to_queue {
    return message('semantics_add_to_queue') ??
        Intl.message(
          'Add episode to queue',
          name: 'semantics_add_to_queue',
          desc: 'Describes add to queue button',
          locale: localeName,
        );
  }

  String get semantics_remove_from_queue {
    return message('semantics_remove_from_queue') ??
        Intl.message(
          'Remove episode from queue',
          name: 'semantics_remove_from_queue',
          desc: 'Describes add to queue button',
          locale: localeName,
        );
  }

  String get semantics_mark_episode_played {
    return message('semantics_mark_episode_played') ??
        Intl.message(
          'Mark Episode as played',
          name: 'semantics_mark_episode_played',
          desc: 'Describes mark played button',
          locale: localeName,
        );
  }

  String get semantics_mark_episode_unplayed {
    return message('semantics_mark_episode_unplayed') ??
        Intl.message(
          'Mark Episode as un-played',
          name: 'semantics_mark_episode_unplayed',
          desc: 'Describes mark unplayed button',
          locale: localeName,
        );
  }

  String get semantics_episode_tile_collapsed {
    return message('semantics_episode_tile_collapsed') ??
        Intl.message(
          'Episode list item. Showing image, summary and main controls.',
          name: 'semantics_episode_tile_collapsed',
          desc: 'Describes episode tile options when collapsed',
          locale: localeName,
        );
  }

  String get semantics_episode_tile_expanded {
    return message('semantics_episode_tile_expanded') ??
        Intl.message(
          'Episode list item. Showing description, main controls and additional controls.',
          name: 'semantics_episode_tile_expanded',
          desc: 'Describes episode tile options when expanded',
          locale: localeName,
        );
  }

  String get semantics_episode_tile_collapsed_hint {
    return message('semantics_episode_tile_collapsed_hint') ??
        Intl.message(
          'expand and show more details and additional options',
          name: 'semantics_episode_tile_collapsed_hint',
          desc: 'Describes episode tile options when collapsed',
          locale: localeName,
        );
  }

  String get semantics_episode_tile_expanded_hint {
    return message('semantics_episode_tile_expanded_hint') ??
        Intl.message(
          'collapse and show summary, download and play control',
          name: 'semantics_episode_tile_expanded_hint',
          desc: 'Describes episode tile options when expanded',
          locale: localeName,
        );
  }

  String get sleep_off_label {
    return message('sleep_off_label') ??
        Intl.message(
          'Off',
          name: 'sleep_off_label',
          desc: 'Describes off sleep label',
          locale: localeName,
        );
  }

  String get sleep_episode_label {
    return message('sleep_episode_label') ??
        Intl.message(
          'End of episode',
          name: 'sleep_episode_label',
          desc: 'Describes end of episode sleep label',
          locale: localeName,
        );
  }

  String sleep_minute_label(String minutes) {
    return message('sleep_minute_label') ??
        Intl.message(
          '$minutes minutes',
          args: [minutes],
          name: 'sleep_minute_label',
          desc: 'Describes the number of minutes to sleep',
          locale: localeName,
        );
  }

  String get sleep_timer_label {
    return message('sleep_timer_label') ??
        Intl.message(
          'Sleep Timer',
          name: 'sleep_timer_label',
          desc: 'Describes sleep timer label',
          locale: localeName,
        );
  }

  String get feedback_menu_item_label {
    return message('feedback_menu_item_label') ??
        Intl.message(
          'Feedback',
          name: 'feedback_menu_item_label',
          desc: 'Feedback option in main menu',
          locale: localeName,
        );
  }

  String get podcast_options_overflow_menu_semantic_label {
    return message('podcast_options_overflow_menu_semantic_label') ??
        Intl.message(
          'Options menu',
          name: 'podcast_options_overflow_menu_semantic_label',
          desc: 'Podcast details overflow menu',
          locale: localeName,
        );
  }

  String get semantic_announce_searching {
    return message('semantic_announce_searching') ??
        Intl.message(
          'Searching, please wait.',
          name: 'semantic_announce_searching',
          desc: 'Spoken when search in progress.',
          locale: localeName,
        );
  }

  String get semantic_playing_options_expand_label {
    return message('semantic_playing_options_expand_label') ??
        Intl.message(
          'Open playing options slider',
          name: 'semantic_playing_options_expand_label',
          desc: 'Placed on options handle when screen reader enabled.',
          locale: localeName,
        );
  }

  String get semantic_playing_options_collapse_label {
    return message('semantic_playing_options_collapse_label') ??
        Intl.message(
          'Close playing options slider',
          name: 'semantic_playing_options_collapse_label',
          desc: 'Placed on options handle when screen reader enabled.',
          locale: localeName,
        );
  }

  String get semantic_podcast_artwork_label {
    return message('semantic_podcast_artwork_label') ??
        Intl.message(
          'Podcast artwork',
          name: 'semantic_podcast_artwork_label',
          desc: 'Placed around podcast image on main player',
          locale: localeName,
        );
  }

  String get semantic_chapter_link_label {
    return message('semantic_chapter_link_label') ??
        Intl.message(
          'Chapter web link',
          name: 'semantic_chapter_link_label',
          desc: 'Placed around chapter link',
          locale: localeName,
        );
  }

  String get semantic_current_chapter_label {
    return message('semantic_current_chapter_label') ??
        Intl.message(
          'Current chapter',
          name: 'semantic_current_chapter_label',
          desc: 'Placed around chapter',
          locale: localeName,
        );
  }

  String get episode_filter_none_label {
    return message('episode_filter_none_label') ??
        Intl.message(
          'None',
          name: 'episode_filter_none_label',
          desc: 'Episodes not filtered',
          locale: localeName,
        );
  }

  String get episode_filter_started_label {
    return message('episode_filter_started_label') ??
        Intl.message(
          'Started',
          name: 'episode_filter_started_label',
          desc: 'Only show episodes that have been started',
          locale: localeName,
        );
  }

  String get episode_filter_played_label {
    return message('episode_filter_played_label') ??
        Intl.message(
          'Played',
          name: 'episode_filter_played_label',
          desc: 'Only show episodes that have been played',
          locale: localeName,
        );
  }

  String get episode_filter_unplayed_label {
    return message('episode_filter_unplayed_label') ??
        Intl.message(
          'Unplayed',
          name: 'episode_filter_unplayed_label',
          desc: 'Only show episodes that have not been played',
          locale: localeName,
        );
  }

  String get episode_filter_no_episodes_title_label {
    return message('episode_filter_no_episodes_title_label') ??
        Intl.message(
          'No Episodes Found',
          name: 'episode_filter_no_episodes_title_label',
          desc: 'No Episodes title',
          locale: localeName,
        );
  }

  String get episode_filter_no_episodes_title_description {
    return message('episode_filter_no_episodes_title_description') ??
        Intl.message(
          'No Episodes Found',
          name: 'episode_filter_no_episodes_title_description',
          desc:
              'This podcast has no episodes matching your search criteria and filter',
          locale: localeName,
        );
  }

  String get episode_filter_clear_filters_button_label {
    return message('episode_filter_clear_filters_button_label') ??
        Intl.message(
          'Clear Filters',
          name: 'episode_filter_clear_filters_button_label',
          desc: 'Clear filters button',
          locale: localeName,
        );
  }

  String get episode_filter_semantic_label {
    return message('episode_filter_semantic_label') ??
        Intl.message(
          'Episode filter',
          name: 'episode_filter_semantic_label',
          desc: 'Episode filter semantic label',
          locale: localeName,
        );
  }

  String get episode_sort_semantic_label {
    return message('episode_sort_semantic_label') ??
        Intl.message(
          'Episode sort',
          name: 'episode_sort_semantic_label',
          desc: 'Episode sort semantic label',
          locale: localeName,
        );
  }

  String get episode_sort_none_label {
    return message('episode_sort_none_label') ??
        Intl.message(
          'Default',
          name: 'episode_sort_none_label',
          desc: 'Episode default sort',
          locale: localeName,
        );
  }

  String get episode_sort_latest_first_label {
    return message('episode_sort_latest_first_label') ??
        Intl.message(
          'Latest first',
          name: 'episode_sort_latest_first_label',
          desc: 'Episode latest first sort',
          locale: localeName,
        );
  }

  String get episode_sort_earliest_first_label {
    return message('episode_sort_earliest_first_label') ??
        Intl.message(
          'Earliest first',
          name: 'episode_sort_earliest_first_label',
          desc: 'Episode earliest first sort',
          locale: localeName,
        );
  }

  String get episode_sort_alphabetical_ascending_label {
    return message('episode_sort_alphabetical_ascending_label') ??
        Intl.message(
          'Alphabetical A-Z',
          name: 'episode_sort_alphabetical_ascending_label',
          desc: 'Episode alphabetical ascending',
          locale: localeName,
        );
  }

  String get episode_sort_alphabetical_descending_label {
    return message('episode_sort_alphabetical_descending_label') ??
        Intl.message(
          'Alphabetical Z-A',
          name: 'episode_sort_alphabetical_descending_label',
          desc: 'Episode alphabetical descending',
          locale: localeName,
        );
  }

  String get open_show_website_label {
    return message('open_show_website_label') ??
        Intl.message(
          'Open show website',
          name: 'open_show_website_label',
          desc: 'Open show website in browser',
          locale: localeName,
        );
  }

  String get refresh_feed_label {
    return message('refresh_feed_label') ??
        Intl.message(
          'Refresh episodes',
          name: 'refresh_feed_label',
          desc: 'Menu item to refresh episodes',
          locale: localeName,
        );
  }

  String get scrim_layout_selector {
    return message('scrim_layout_selector') ??
        Intl.message(
          'Dismiss layout selector',
          name: 'scrim_layout_selector',
          desc:
              'Replaces default scrim label for layout selector bottom sheet.',
          locale: localeName,
        );
  }

  String get now_playing_episode_position {
    return message('now_playing_episode_position') ??
        Intl.message(
          'Episode position',
          name: 'now_playing_episode_position',
          desc: 'Episode position slider control label',
          locale: localeName,
        );
  }

  String get now_playing_episode_time_remaining {
    return message('now_playing_episode_time_remaining') ??
        Intl.message(
          'Time remaining',
          name: 'now_playing_episode_time_remaining',
          desc: 'Episode time remaining slider control label',
          locale: localeName,
        );
  }

  String get resume_button_label {
    return message('resume_button_label') ??
        Intl.message(
          'Resume episode',
          name: 'resume_button_label',
          desc: 'Semantic label for the resume button',
          locale: localeName,
        );
  }

  String get play_download_button_label {
    return message('play_download_button_label') ??
        Intl.message(
          'Play downloaded episode',
          name: 'play_download_button_label',
          desc: 'Semantic label for the play downloaded episode button',
          locale: localeName,
        );
  }

  String get cancel_download_button_label {
    return message('cancel_download_button_label') ??
        Intl.message(
          'Cancel download',
          name: 'cancel_download_button_label',
          desc: 'Semantic label for the play cancel download button',
          locale: localeName,
        );
  }

  String get episode_details_button_label {
    return message('episode_details_button_label') ??
        Intl.message(
          'Show episode information',
          name: 'episode_details_button_label',
          desc: 'Semantic label for the show info button.',
          locale: localeName,
        );
  }

  String get scrim_sleep_timer_selector {
    return message('scrim_sleep_timer_selector') ??
        Intl.message(
          'Dismiss sleep timer selector',
          name: 'scrim_sleep_timer_selector',
          desc: 'Replaces default scrim label for custom.',
          locale: localeName,
        );
  }

  String get scrim_speed_selector {
    return message('scrim_speed_selector') ??
        Intl.message(
          'Dismiss playback speed selector',
          name: 'scrim_speed_selector',
          desc: 'Replaces default scrim label for custom.',
          locale: localeName,
        );
  }

  String get semantic_current_value_label {
    return message('semantic_current_value_label') ??
        Intl.message(
          'Current value',
          name: 'semantic_current_value_label',
          desc: 'For current sleep setting',
          locale: localeName,
        );
  }

  String get scrim_episode_details_selector {
    return message('scrim_episode_details_selector') ??
        Intl.message(
          'Dismiss episode details',
          name: 'scrim_episode_details_selector',
          desc:
              'Replaces default scrim label for episode details bottom sheet.',
          locale: localeName,
        );
  }

  String get scrim_episode_sort_selector {
    return message('scrim_episode_sort_selector') ??
        Intl.message(
          'Dismiss episode sort',
          name: 'scrim_episode_sort_selector',
          desc: 'Replaces default scrim label for episode sort bottom sheet.',
          locale: localeName,
        );
  }

  String get scrim_episode_filter_selector {
    return message('scrim_episode_filter_selector') ??
        Intl.message(
          'Dismiss episode filter',
          name: 'scrim_episode_filter_selector',
          desc: 'Replaces default scrim label for episode filter bottom sheet.',
          locale: localeName,
        );
  }

  String get search_episodes_label {
    return message('search_episodes_label') ??
        Intl.message(
          'Search episodes',
          name: 'search_episodes_label',
          desc: 'Hint text for episode search box',
          locale: localeName,
        );
  }
}

class PinepodsLocalisationsDelegate extends LocalizationsDelegate<L> {
  const PinepodsLocalisationsDelegate();

  @override
  bool isSupported(Locale locale) =>
      ['en', 'de', 'it'].contains(locale.languageCode);

  @override
  Future<L> load(Locale locale) => L.load(locale, const {});

  @override
  bool shouldReload(PinepodsLocalisationsDelegate old) => false;
}

/// This class can be used by third-parties who wish to override or replace
/// some of the strings built into Pinepods. This class takes a map
/// of message labels which takes a map of localised string replacements. For
/// example, to update the app title you may passes messages containing:
/// app_title: {
///   'en': 'my new app title',
///   'de': 'Mein app-titel'
/// }
class EmbeddedLocalisationsDelegate extends LocalizationsDelegate<L> {
  final Map<String, Map<String, String>> messages;

  EmbeddedLocalisationsDelegate({@required this.messages = const {}});

  @override
  bool isSupported(Locale locale) =>
      ['en', 'de', 'it'].contains(locale.languageCode);

  @override
  Future<L> load(Locale locale) => L.load(locale, messages);

  @override
  bool shouldReload(EmbeddedLocalisationsDelegate old) => false;
}
