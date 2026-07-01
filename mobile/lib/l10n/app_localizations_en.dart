// ignore: unused_import
import 'package:intl/intl.dart' as intl;
import 'app_localizations.dart';

// ignore_for_file: type=lint

/// The translations for English (`en`).
class AppLocalizationsEn extends AppLocalizations {
  AppLocalizationsEn([String locale = 'en']) : super(locale);

  @override
  String get app_title => 'PinePods Podcast Player';

  @override
  String get app_title_short => 'Pinepods';

  @override
  String get library => 'Library';

  @override
  String get discover => 'Discover';

  @override
  String get downloads => 'Downloads';

  @override
  String get subscribe_button_label => 'Follow';

  @override
  String get unsubscribe_button_label => 'Unfollow';

  @override
  String get cancel_button_label => 'Cancel';

  @override
  String get ok_button_label => 'OK';

  @override
  String get subscribe_label => 'Follow';

  @override
  String get unsubscribe_label => 'Unfollow';

  @override
  String get unsubscribe_message =>
      'Unfollowing will delete all downloaded episodes of this podcast.';

  @override
  String get search_for_podcasts_hint => 'Search for new podcasts';

  @override
  String get no_subscriptions_message =>
      'Head to Settings to Connect a Pinepods Server if you haven\'t yet!';

  @override
  String get delete_label => 'Delete';

  @override
  String get delete_button_label => 'Delete';

  @override
  String get mark_played_label => 'Mark Played';

  @override
  String get mark_unplayed_label => 'Mark Unplayed';

  @override
  String get delete_episode_confirmation =>
      'Are you sure you wish to delete this episode?';

  @override
  String get delete_episode_title => 'Delete Episode';

  @override
  String get no_downloads_message => 'You do not have any downloaded episodes';

  @override
  String get no_search_results_message => 'No podcasts found';

  @override
  String get no_podcast_details_message =>
      'Could not load podcast episodes. Please check your connection.';

  @override
  String get play_button_label => 'Play episode';

  @override
  String get pause_button_label => 'Pause episode';

  @override
  String get download_episode_button_label => 'Download episode';

  @override
  String get delete_episode_button_label => 'Delete downloaded episode';

  @override
  String get close_button_label => 'Close';

  @override
  String get search_button_label => 'Search';

  @override
  String get clear_search_button_label => 'Clear search text';

  @override
  String get search_back_button_label => 'Back';

  @override
  String get minimise_player_window_button_label => 'Minimise player window';

  @override
  String get rewind_button_label => 'Rewind episode 10 seconds';

  @override
  String get fast_forward_button_label => 'Fast-forward episode 30 seconds';

  @override
  String get about_label => 'About';

  @override
  String get mark_episodes_played_label => 'Mark all episodes as played';

  @override
  String get mark_episodes_not_played_label =>
      'Mark all episodes as not played';

  @override
  String get stop_download_confirmation =>
      'Are you sure you wish to stop this download and delete the episode?';

  @override
  String get stop_download_button_label => 'Stop';

  @override
  String get stop_download_title => 'Stop Download';

  @override
  String get settings_mark_deleted_played_label =>
      'Mark deleted episodes as played';

  @override
  String get settings_delete_played_label =>
      'Delete downloaded episodes once played';

  @override
  String get settings_download_sd_card_label => 'Download episodes to SD card';

  @override
  String get settings_download_switch_card =>
      'New downloads will be saved to the SD card. Existing downloads will remain on internal storage.';

  @override
  String get settings_download_switch_internal =>
      'New downloads will be saved to internal storage. Existing downloads will remain on the SD card.';

  @override
  String get settings_download_switch_label => 'Change storage location';

  @override
  String get cancel_option_label => 'Cancel';

  @override
  String get settings_theme_switch_label => 'Dark theme';

  @override
  String get playback_speed_label => 'Playback speed';

  @override
  String get show_notes_label => 'Show notes';

  @override
  String get search_provider_label => 'Search provider';

  @override
  String get settings_label => 'Settings';

  @override
  String get go_back_button_label => 'Go Back';

  @override
  String get continue_button_label => 'Continue';

  @override
  String get consent_message =>
      'This funding link will take you to an external site where you will be able to directly support the show. Links are provided by the podcast authors and is not controlled by PinePods.';

  @override
  String get episode_label => 'Episode';

  @override
  String get chapters_label => 'Chapters';

  @override
  String get notes_label => 'Description';

  @override
  String get podcast_funding_dialog_header => 'Podcast Funding';

  @override
  String get settings_auto_open_now_playing =>
      'Full screen player mode on episode start';

  @override
  String get error_no_connection =>
      'Unable to play episode. Please check your connection and try again.';

  @override
  String get error_playback_fail =>
      'An unexpected error occurred during playback. Please check your connection and try again.';

  @override
  String get add_rss_feed_option => 'Add RSS Feed';

  @override
  String get settings_import_opml => 'Import OPML';

  @override
  String get settings_export_opml => 'Export OPML';

  @override
  String get label_opml_importing => 'Importing';

  @override
  String get settings_auto_update_episodes_heading =>
      'Refresh episodes on details screen after';

  @override
  String get settings_auto_update_episodes => 'Auto update episodes';

  @override
  String get settings_auto_update_episodes_never => 'Never';

  @override
  String get settings_auto_update_episodes_always => 'Always';

  @override
  String get settings_auto_update_episodes_10min =>
      '10 minutes since last update';

  @override
  String get settings_auto_update_episodes_30min =>
      '30 minutes since last update';

  @override
  String get settings_auto_update_episodes_1hour => '1 hour since last update';

  @override
  String get settings_auto_update_episodes_3hour => '3 hours since last update';

  @override
  String get settings_auto_update_episodes_6hour => '6 hours since last update';

  @override
  String get settings_auto_update_episodes_12hour =>
      '12 hours since last update';

  @override
  String get new_episodes_label => 'New episodes are available';

  @override
  String get new_episodes_view_now_label => 'VIEW NOW';

  @override
  String get settings_personalisation_divider_label => 'Personalisation';

  @override
  String get settings_episodes_divider_label => 'EPISODES';

  @override
  String get settings_playback_divider_label => 'Playback';

  @override
  String get settings_data_divider_label => 'DATA';

  @override
  String get audio_effect_trim_silence_label => 'Trim Silence';

  @override
  String get audio_effect_volume_boost_label => 'Volume Boost';

  @override
  String get audio_settings_playback_speed_label => 'Playback Speed';

  @override
  String get empty_queue_message => 'Your queue is empty';

  @override
  String get clear_queue_button_label => 'CLEAR QUEUE';

  @override
  String get now_playing_queue_label => 'Now Playing';

  @override
  String get up_next_queue_label => 'Up Next';

  @override
  String get more_label => 'More';

  @override
  String get queue_add_label => 'Add';

  @override
  String get queue_remove_label => 'Remove';

  @override
  String get opml_import_button_label => 'Import';

  @override
  String get opml_export_button_label => 'Export';

  @override
  String get opml_import_export_label => 'OPML Import/Export';

  @override
  String get queue_clear_label => 'Are you sure you wish to clear the queue?';

  @override
  String get queue_clear_button_label => 'Clear';

  @override
  String get queue_clear_label_title => 'Clear Queue';

  @override
  String get layout_label => 'Layout';

  @override
  String get discovery_categories_itunes =>
      '<All>,Arts,Business,Comedy,Education,Fiction,Government,Health & Fitness,History,Kids & Family,Leisure,Music,News,Religion & Spirituality,Science,Society & Culture,Sports,TV & Film,Technology,True Crime';

  @override
  String get discovery_categories_pindex =>
      '<All>,After-Shows,Alternative,Animals,Animation,Arts,Astronomy,Automotive,Aviation,Baseball,Basketball,Beauty,Books,Buddhism,Business,Careers,Chemistry,Christianity,Climate,Comedy,Commentary,Courses,Crafts,Cricket,Cryptocurrency,Culture,Daily,Design,Documentary,Drama,Earth,Education,Entertainment,Entrepreneurship,Family,Fantasy,Fashion,Fiction,Film,Fitness,Food,Football,Games,Garden,Golf,Government,Health,Hinduism,History,Hobbies,Hockey,Home,HowTo,Improv,Interviews,Investing,Islam,Journals,Judaism,Kids,Language,Learning,Leisure,Life,Management,Manga,Marketing,Mathematics,Medicine,Mental,Music,Natural,Nature,News,NonProfit,Nutrition,Parenting,Performing,Personal,Pets,Philosophy,Physics,Places,Politics,Relationships,Religion,Reviews,Role-Playing,Rugby,Running,Science,Self-Improvement,Sexuality,Soccer,Social,Society,Spirituality,Sports,Stand-Up,Stories,Swimming,TV,Tabletop,Technology,Tennis,Travel,True Crime,Video-Games,Visual,Volleyball,Weather,Wilderness,Wrestling';

  @override
  String get transcript_label => 'Transcript';

  @override
  String get no_transcript_available_label =>
      'A transcript is not available for this podcast';

  @override
  String get search_transcript_label => 'Search transcript';

  @override
  String get auto_scroll_transcript_label => 'Follow transcript';

  @override
  String get transcript_why_not_label => 'Why not?';

  @override
  String get transcript_why_not_url =>
      'https://www.pinepods.online/docs/Features/Transcript';

  @override
  String get semantics_podcast_details_header =>
      'Podcast details and episodes page';

  @override
  String get semantics_layout_option_list => 'List layout';

  @override
  String get semantics_layout_option_compact_grid => 'Compact grid layout';

  @override
  String get semantics_layout_option_grid => 'Grid layout';

  @override
  String get semantics_mini_player_header =>
      'Mini player. Swipe right to play/pause button. Activate to open main player window';

  @override
  String get semantics_main_player_header => 'Main player window';

  @override
  String get semantics_play_pause_toggle => 'Play/pause toggle';

  @override
  String get semantics_decrease_playback_speed => 'Decrease playback speed';

  @override
  String get semantics_increase_playback_speed => 'Increase playback speed';

  @override
  String get semantics_expand_podcast_description =>
      'Expand podcast description';

  @override
  String get semantics_collapse_podcast_description =>
      'Collapse podcast description';

  @override
  String get semantics_add_to_queue => 'Add episode to queue';

  @override
  String get semantics_remove_from_queue => 'Remove episode from queue';

  @override
  String get semantics_mark_episode_played => 'Mark Episode as played';

  @override
  String get semantics_mark_episode_unplayed => 'Mark Episode as un-played';

  @override
  String get semantics_episode_tile_collapsed =>
      'Episode list item. Showing image, summary and main controls.';

  @override
  String get semantics_episode_tile_expanded =>
      'Episode list item. Showing description, main controls and additional controls.';

  @override
  String get semantics_episode_tile_collapsed_hint =>
      'expand and show more details and additional options';

  @override
  String get semantics_episode_tile_expanded_hint =>
      'collapse and show summary, download and play control';

  @override
  String get sleep_off_label => 'Off';

  @override
  String get sleep_episode_label => 'End of episode';

  @override
  String sleep_minute_label(String minutes) {
    return '$minutes minutes';
  }

  @override
  String get sleep_timer_label => 'Sleep Timer';

  @override
  String get feedback_menu_item_label => 'Feedback';

  @override
  String get podcast_options_overflow_menu_semantic_label => 'Options menu';

  @override
  String get semantic_announce_searching => 'Searching, please wait.';

  @override
  String get semantic_playing_options_expand_label =>
      'Open playing options slider';

  @override
  String get semantic_playing_options_collapse_label =>
      'Close playing options slider';

  @override
  String get semantic_podcast_artwork_label => 'Podcast artwork';

  @override
  String get semantic_chapter_link_label => 'Chapter web link';

  @override
  String get semantic_current_chapter_label => 'Current chapter';

  @override
  String get episode_filter_none_label => 'None';

  @override
  String get episode_filter_started_label => 'Started';

  @override
  String get episode_filter_played_label => 'Played';

  @override
  String get episode_filter_unplayed_label => 'Unplayed';

  @override
  String get episode_filter_no_episodes_title_label => 'No Episodes Found';

  @override
  String get episode_filter_no_episodes_title_description =>
      'This podcast has no episodes matching your search criteria and filter';

  @override
  String get episode_filter_clear_filters_button_label => 'Clear Filters';

  @override
  String get episode_filter_semantic_label => 'Filter episodes';

  @override
  String get episode_sort_semantic_label => 'Sort episodes';

  @override
  String get episode_sort_none_label => 'Default';

  @override
  String get episode_sort_latest_first_label => 'Latest first';

  @override
  String get episode_sort_earliest_first_label => 'Earliest first';

  @override
  String get episode_sort_alphabetical_ascending_label => 'Alphabetical A-Z';

  @override
  String get episode_sort_alphabetical_descending_label => 'Alphabetical Z-A';

  @override
  String get open_show_website_label => 'Open show website';

  @override
  String get refresh_feed_label => 'Refresh episodes';

  @override
  String get scrim_layout_selector => 'Dismiss layout selector';

  @override
  String get now_playing_episode_position => 'Episode position';

  @override
  String get now_playing_episode_time_remaining => 'Time remaining';

  @override
  String get resume_button_label => 'Resume episode';

  @override
  String get play_download_button_label => 'Play downloaded episode';

  @override
  String get cancel_download_button_label => 'Cancel download';

  @override
  String get episode_details_button_label => 'Show episode information';

  @override
  String get scrim_sleep_timer_selector => 'Dismiss sleep timer selector';

  @override
  String get scrim_speed_selector => 'Dismiss playback speed selector';

  @override
  String get semantic_current_value_label => 'Current value';

  @override
  String get scrim_episode_details_selector => 'Dismiss episode details';

  @override
  String get scrim_episode_sort_selector => 'Dismiss episode sort';

  @override
  String get scrim_episode_filter_selector => 'Dismiss episode filter';

  @override
  String get search_episodes_label => 'Search episodes';
}
