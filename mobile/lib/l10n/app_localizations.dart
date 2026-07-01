import 'dart:async';

import 'package:flutter/foundation.dart';
import 'package:flutter/widgets.dart';
import 'package:flutter_localizations/flutter_localizations.dart';
import 'package:intl/intl.dart' as intl;

import 'app_localizations_de.dart';
import 'app_localizations_en.dart';
import 'app_localizations_it.dart';

// ignore_for_file: type=lint

/// Callers can lookup localized strings with an instance of AppLocalizations
/// returned by `AppLocalizations.of(context)`.
///
/// Applications need to include `AppLocalizations.delegate()` in their app's
/// `localizationDelegates` list, and the locales they support in the app's
/// `supportedLocales` list. For example:
///
/// ```dart
/// import 'l10n/app_localizations.dart';
///
/// return MaterialApp(
///   localizationsDelegates: AppLocalizations.localizationsDelegates,
///   supportedLocales: AppLocalizations.supportedLocales,
///   home: MyApplicationHome(),
/// );
/// ```
///
/// ## Update pubspec.yaml
///
/// Please make sure to update your pubspec.yaml to include the following
/// packages:
///
/// ```yaml
/// dependencies:
///   # Internationalization support.
///   flutter_localizations:
///     sdk: flutter
///   intl: any # Use the pinned version from flutter_localizations
///
///   # Rest of dependencies
/// ```
///
/// ## iOS Applications
///
/// iOS applications define key application metadata, including supported
/// locales, in an Info.plist file that is built into the application bundle.
/// To configure the locales supported by your app, you’ll need to edit this
/// file.
///
/// First, open your project’s ios/Runner.xcworkspace Xcode workspace file.
/// Then, in the Project Navigator, open the Info.plist file under the Runner
/// project’s Runner folder.
///
/// Next, select the Information Property List item, select Add Item from the
/// Editor menu, then select Localizations from the pop-up menu.
///
/// Select and expand the newly-created Localizations item then, for each
/// locale your application supports, add a new item and select the locale
/// you wish to add from the pop-up menu in the Value field. This list should
/// be consistent with the languages listed in the AppLocalizations.supportedLocales
/// property.
abstract class AppLocalizations {
  AppLocalizations(String locale)
    : localeName = intl.Intl.canonicalizedLocale(locale.toString());

  final String localeName;

  static AppLocalizations? of(BuildContext context) {
    return Localizations.of<AppLocalizations>(context, AppLocalizations);
  }

  static const LocalizationsDelegate<AppLocalizations> delegate =
      _AppLocalizationsDelegate();

  /// A list of this localizations delegate along with the default localizations
  /// delegates.
  ///
  /// Returns a list of localizations delegates containing this delegate along with
  /// GlobalMaterialLocalizations.delegate, GlobalCupertinoLocalizations.delegate,
  /// and GlobalWidgetsLocalizations.delegate.
  ///
  /// Additional delegates can be added by appending to this list in
  /// MaterialApp. This list does not have to be used at all if a custom list
  /// of delegates is preferred or required.
  static const List<LocalizationsDelegate<dynamic>> localizationsDelegates =
      <LocalizationsDelegate<dynamic>>[
        delegate,
        GlobalMaterialLocalizations.delegate,
        GlobalCupertinoLocalizations.delegate,
        GlobalWidgetsLocalizations.delegate,
      ];

  /// A list of this localizations delegate's supported locales.
  static const List<Locale> supportedLocales = <Locale>[
    Locale('de'),
    Locale('en'),
    Locale('it'),
  ];

  /// Full title for the application
  ///
  /// In en, this message translates to:
  /// **'PinePods Podcast Player'**
  String get app_title;

  /// Title for the application
  ///
  /// In en, this message translates to:
  /// **'Pinepods'**
  String get app_title_short;

  /// Library tab label
  ///
  /// In en, this message translates to:
  /// **'Library'**
  String get library;

  /// Discover tab label
  ///
  /// In en, this message translates to:
  /// **'Discover'**
  String get discover;

  /// Downloads tab label
  ///
  /// In en, this message translates to:
  /// **'Downloads'**
  String get downloads;

  /// Subscribe button label
  ///
  /// In en, this message translates to:
  /// **'Follow'**
  String get subscribe_button_label;

  /// Unsubscribe button label
  ///
  /// In en, this message translates to:
  /// **'Unfollow'**
  String get unsubscribe_button_label;

  /// Cancel button label
  ///
  /// In en, this message translates to:
  /// **'Cancel'**
  String get cancel_button_label;

  /// OK button label
  ///
  /// In en, this message translates to:
  /// **'OK'**
  String get ok_button_label;

  /// Subscribe label
  ///
  /// In en, this message translates to:
  /// **'Follow'**
  String get subscribe_label;

  /// Unsubscribe label
  ///
  /// In en, this message translates to:
  /// **'Unfollow'**
  String get unsubscribe_label;

  /// Displayed when the user unsubscribes from a podcast.
  ///
  /// In en, this message translates to:
  /// **'Unfollowing will delete all downloaded episodes of this podcast.'**
  String get unsubscribe_message;

  /// Hint displayed on search bar when the user clicks the search icon.
  ///
  /// In en, this message translates to:
  /// **'Search for new podcasts'**
  String get search_for_podcasts_hint;

  /// Displayed on the library tab when the user has no subscriptions
  ///
  /// In en, this message translates to:
  /// **'Head to Settings to Connect a Pinepods Server if you haven\'t yet!'**
  String get no_subscriptions_message;

  /// Delete label
  ///
  /// In en, this message translates to:
  /// **'Delete'**
  String get delete_label;

  /// Delete label
  ///
  /// In en, this message translates to:
  /// **'Delete'**
  String get delete_button_label;

  /// Mark as played
  ///
  /// In en, this message translates to:
  /// **'Mark Played'**
  String get mark_played_label;

  /// Mark as unplayed
  ///
  /// In en, this message translates to:
  /// **'Mark Unplayed'**
  String get mark_unplayed_label;

  /// User is asked to confirm when they attempt to delete an episode
  ///
  /// In en, this message translates to:
  /// **'Are you sure you wish to delete this episode?'**
  String get delete_episode_confirmation;

  /// Delete label
  ///
  /// In en, this message translates to:
  /// **'Delete Episode'**
  String get delete_episode_title;

  /// Displayed on the library tab when the user has no subscriptions
  ///
  /// In en, this message translates to:
  /// **'You do not have any downloaded episodes'**
  String get no_downloads_message;

  /// Displayed on the library tab when the user has no subscriptions
  ///
  /// In en, this message translates to:
  /// **'No podcasts found'**
  String get no_search_results_message;

  /// Displayed on the podcast details page when the details could not be loaded
  ///
  /// In en, this message translates to:
  /// **'Could not load podcast episodes. Please check your connection.'**
  String get no_podcast_details_message;

  /// Semantic label for the play button
  ///
  /// In en, this message translates to:
  /// **'Play episode'**
  String get play_button_label;

  /// Semantic label for the pause button
  ///
  /// In en, this message translates to:
  /// **'Pause episode'**
  String get pause_button_label;

  /// Semantic label for the download episode button
  ///
  /// In en, this message translates to:
  /// **'Download episode'**
  String get download_episode_button_label;

  /// Semantic label for the delete episode
  ///
  /// In en, this message translates to:
  /// **'Delete downloaded episode'**
  String get delete_episode_button_label;

  /// Close button label
  ///
  /// In en, this message translates to:
  /// **'Close'**
  String get close_button_label;

  /// Search button label
  ///
  /// In en, this message translates to:
  /// **'Search'**
  String get search_button_label;

  /// Search button label
  ///
  /// In en, this message translates to:
  /// **'Clear search text'**
  String get clear_search_button_label;

  /// Search button label
  ///
  /// In en, this message translates to:
  /// **'Back'**
  String get search_back_button_label;

  /// Search button label
  ///
  /// In en, this message translates to:
  /// **'Minimise player window'**
  String get minimise_player_window_button_label;

  /// Rewind button tooltip
  ///
  /// In en, this message translates to:
  /// **'Rewind episode 10 seconds'**
  String get rewind_button_label;

  /// Fast forward tooltip
  ///
  /// In en, this message translates to:
  /// **'Fast-forward episode 30 seconds'**
  String get fast_forward_button_label;

  /// About menu item
  ///
  /// In en, this message translates to:
  /// **'About'**
  String get about_label;

  /// Mark all episodes played menu item
  ///
  /// In en, this message translates to:
  /// **'Mark all episodes as played'**
  String get mark_episodes_played_label;

  /// Mark all episodes not-played menu item
  ///
  /// In en, this message translates to:
  /// **'Mark all episodes as not played'**
  String get mark_episodes_not_played_label;

  /// User is asked to confirm when they wish to stop the active download.
  ///
  /// In en, this message translates to:
  /// **'Are you sure you wish to stop this download and delete the episode?'**
  String get stop_download_confirmation;

  /// Stop label
  ///
  /// In en, this message translates to:
  /// **'Stop'**
  String get stop_download_button_label;

  /// Stop download label
  ///
  /// In en, this message translates to:
  /// **'Stop Download'**
  String get stop_download_title;

  /// Mark deleted episodes as played setting
  ///
  /// In en, this message translates to:
  /// **'Mark deleted episodes as played'**
  String get settings_mark_deleted_played_label;

  /// Delete downloaded episodes once played setting
  ///
  /// In en, this message translates to:
  /// **'Delete downloaded episodes once played'**
  String get settings_delete_played_label;

  /// Download episodes to SD card setting
  ///
  /// In en, this message translates to:
  /// **'Download episodes to SD card'**
  String get settings_download_sd_card_label;

  /// Displayed when user switches from internal storage to SD card
  ///
  /// In en, this message translates to:
  /// **'New downloads will be saved to the SD card. Existing downloads will remain on internal storage.'**
  String get settings_download_switch_card;

  /// Displayed when user switches from internal SD card to internal storage
  ///
  /// In en, this message translates to:
  /// **'New downloads will be saved to internal storage. Existing downloads will remain on the SD card.'**
  String get settings_download_switch_internal;

  /// Dialog label for storage switch
  ///
  /// In en, this message translates to:
  /// **'Change storage location'**
  String get settings_download_switch_label;

  /// Cancel option label
  ///
  /// In en, this message translates to:
  /// **'Cancel'**
  String get cancel_option_label;

  /// Dark theme
  ///
  /// In en, this message translates to:
  /// **'Dark theme'**
  String get settings_theme_switch_label;

  /// Set playback speed icon label
  ///
  /// In en, this message translates to:
  /// **'Playback speed'**
  String get playback_speed_label;

  /// Set show notes icon label
  ///
  /// In en, this message translates to:
  /// **'Show notes'**
  String get show_notes_label;

  /// Set search provider label
  ///
  /// In en, this message translates to:
  /// **'Search provider'**
  String get search_provider_label;

  /// Settings label
  ///
  /// In en, this message translates to:
  /// **'Settings'**
  String get settings_label;

  /// Go-back button label
  ///
  /// In en, this message translates to:
  /// **'Go Back'**
  String get go_back_button_label;

  /// Continue button label
  ///
  /// In en, this message translates to:
  /// **'Continue'**
  String get continue_button_label;

  /// Display when first accessing external funding link
  ///
  /// In en, this message translates to:
  /// **'This funding link will take you to an external site where you will be able to directly support the show. Links are provided by the podcast authors and is not controlled by PinePods.'**
  String get consent_message;

  /// Tab label on now playing screen.
  ///
  /// In en, this message translates to:
  /// **'Episode'**
  String get episode_label;

  /// Tab label on now playing screen.
  ///
  /// In en, this message translates to:
  /// **'Chapters'**
  String get chapters_label;

  /// Tab label on now playing screen.
  ///
  /// In en, this message translates to:
  /// **'Description'**
  String get notes_label;

  /// Header on podcast funding consent dialog
  ///
  /// In en, this message translates to:
  /// **'Podcast Funding'**
  String get podcast_funding_dialog_header;

  /// Displayed when user switches to use full screen player automatically
  ///
  /// In en, this message translates to:
  /// **'Full screen player mode on episode start'**
  String get settings_auto_open_now_playing;

  /// Displayed when attempting to start streaming an episode with no data connection
  ///
  /// In en, this message translates to:
  /// **'Unable to play episode. Please check your connection and try again.'**
  String get error_no_connection;

  /// Displayed when attempting to start streaming an episode with no data connection
  ///
  /// In en, this message translates to:
  /// **'An unexpected error occurred during playback. Please check your connection and try again.'**
  String get error_playback_fail;

  /// Option label for adding manual RSS feed url
  ///
  /// In en, this message translates to:
  /// **'Add RSS Feed'**
  String get add_rss_feed_option;

  /// Option label importing OPML file
  ///
  /// In en, this message translates to:
  /// **'Import OPML'**
  String get settings_import_opml;

  /// Option label exporting OPML file
  ///
  /// In en, this message translates to:
  /// **'Export OPML'**
  String get settings_export_opml;

  /// Label for importing OPML dialog
  ///
  /// In en, this message translates to:
  /// **'Importing'**
  String get label_opml_importing;

  /// Option label for auto updating of episodes
  ///
  /// In en, this message translates to:
  /// **'Refresh episodes on details screen after'**
  String get settings_auto_update_episodes_heading;

  /// Option label for auto updating of episodes
  ///
  /// In en, this message translates to:
  /// **'Auto update episodes'**
  String get settings_auto_update_episodes;

  /// Option label for auto updating of episodes
  ///
  /// In en, this message translates to:
  /// **'Never'**
  String get settings_auto_update_episodes_never;

  /// Option label for auto updating of episodes
  ///
  /// In en, this message translates to:
  /// **'Always'**
  String get settings_auto_update_episodes_always;

  /// Option label for auto updating of episodes
  ///
  /// In en, this message translates to:
  /// **'10 minutes since last update'**
  String get settings_auto_update_episodes_10min;

  /// Option label for auto updating of episodes
  ///
  /// In en, this message translates to:
  /// **'30 minutes since last update'**
  String get settings_auto_update_episodes_30min;

  /// Option label for auto updating of episodes
  ///
  /// In en, this message translates to:
  /// **'1 hour since last update'**
  String get settings_auto_update_episodes_1hour;

  /// Option label for auto updating of episodes
  ///
  /// In en, this message translates to:
  /// **'3 hours since last update'**
  String get settings_auto_update_episodes_3hour;

  /// Option label for auto updating of episodes
  ///
  /// In en, this message translates to:
  /// **'6 hours since last update'**
  String get settings_auto_update_episodes_6hour;

  /// Option label for auto updating of episodes
  ///
  /// In en, this message translates to:
  /// **'12 hours since last update'**
  String get settings_auto_update_episodes_12hour;

  /// Option label for new episodes snackbar
  ///
  /// In en, this message translates to:
  /// **'New episodes are available'**
  String get new_episodes_label;

  /// Option action label for new episodes snackbar
  ///
  /// In en, this message translates to:
  /// **'VIEW NOW'**
  String get new_episodes_view_now_label;

  /// Settings divider label for personalisation
  ///
  /// In en, this message translates to:
  /// **'Personalisation'**
  String get settings_personalisation_divider_label;

  /// Settings divider label for episodes
  ///
  /// In en, this message translates to:
  /// **'EPISODES'**
  String get settings_episodes_divider_label;

  /// Settings divider label for playback
  ///
  /// In en, this message translates to:
  /// **'Playback'**
  String get settings_playback_divider_label;

  /// Settings divider label for data
  ///
  /// In en, this message translates to:
  /// **'DATA'**
  String get settings_data_divider_label;

  /// Label for trim silence toggle
  ///
  /// In en, this message translates to:
  /// **'Trim Silence'**
  String get audio_effect_trim_silence_label;

  /// Label for volume boost toggle
  ///
  /// In en, this message translates to:
  /// **'Volume Boost'**
  String get audio_effect_volume_boost_label;

  /// Label for playback settings widget
  ///
  /// In en, this message translates to:
  /// **'Playback Speed'**
  String get audio_settings_playback_speed_label;

  /// Displayed when there are no items left in the queue
  ///
  /// In en, this message translates to:
  /// **'Your queue is empty'**
  String get empty_queue_message;

  /// Clear queue button label
  ///
  /// In en, this message translates to:
  /// **'CLEAR QUEUE'**
  String get clear_queue_button_label;

  /// Now playing label on queue
  ///
  /// In en, this message translates to:
  /// **'Now Playing'**
  String get now_playing_queue_label;

  /// Up next label on queue
  ///
  /// In en, this message translates to:
  /// **'Up Next'**
  String get up_next_queue_label;

  /// More label
  ///
  /// In en, this message translates to:
  /// **'More'**
  String get more_label;

  /// Queue add label
  ///
  /// In en, this message translates to:
  /// **'Add'**
  String get queue_add_label;

  /// Queue remove label
  ///
  /// In en, this message translates to:
  /// **'Remove'**
  String get queue_remove_label;

  /// OPML Import button label
  ///
  /// In en, this message translates to:
  /// **'Import'**
  String get opml_import_button_label;

  /// OPML Export button label
  ///
  /// In en, this message translates to:
  /// **'Export'**
  String get opml_export_button_label;

  /// OPML Import/Export label
  ///
  /// In en, this message translates to:
  /// **'OPML Import/Export'**
  String get opml_import_export_label;

  /// Shown on dialog box when clearing queue
  ///
  /// In en, this message translates to:
  /// **'Are you sure you wish to clear the queue?'**
  String get queue_clear_label;

  /// Shown on dialog box when clearing queue
  ///
  /// In en, this message translates to:
  /// **'Clear'**
  String get queue_clear_button_label;

  /// Shown on dialog box when clearing queue
  ///
  /// In en, this message translates to:
  /// **'Clear Queue'**
  String get queue_clear_label_title;

  /// Layout menu label
  ///
  /// In en, this message translates to:
  /// **'Layout'**
  String get layout_label;

  /// Comma separated list of iTunes categories
  ///
  /// In en, this message translates to:
  /// **'<All>,Arts,Business,Comedy,Education,Fiction,Government,Health & Fitness,History,Kids & Family,Leisure,Music,News,Religion & Spirituality,Science,Society & Culture,Sports,TV & Film,Technology,True Crime'**
  String get discovery_categories_itunes;

  /// Comma separated list of Podcast Index categories
  ///
  /// In en, this message translates to:
  /// **'<All>,After-Shows,Alternative,Animals,Animation,Arts,Astronomy,Automotive,Aviation,Baseball,Basketball,Beauty,Books,Buddhism,Business,Careers,Chemistry,Christianity,Climate,Comedy,Commentary,Courses,Crafts,Cricket,Cryptocurrency,Culture,Daily,Design,Documentary,Drama,Earth,Education,Entertainment,Entrepreneurship,Family,Fantasy,Fashion,Fiction,Film,Fitness,Food,Football,Games,Garden,Golf,Government,Health,Hinduism,History,Hobbies,Hockey,Home,HowTo,Improv,Interviews,Investing,Islam,Journals,Judaism,Kids,Language,Learning,Leisure,Life,Management,Manga,Marketing,Mathematics,Medicine,Mental,Music,Natural,Nature,News,NonProfit,Nutrition,Parenting,Performing,Personal,Pets,Philosophy,Physics,Places,Politics,Relationships,Religion,Reviews,Role-Playing,Rugby,Running,Science,Self-Improvement,Sexuality,Soccer,Social,Society,Spirituality,Sports,Stand-Up,Stories,Swimming,TV,Tabletop,Technology,Tennis,Travel,True Crime,Video-Games,Visual,Volleyball,Weather,Wilderness,Wrestling'**
  String get discovery_categories_pindex;

  /// Transcript label
  ///
  /// In en, this message translates to:
  /// **'Transcript'**
  String get transcript_label;

  /// Displayed in transcript view when no transcript is available
  ///
  /// In en, this message translates to:
  /// **'A transcript is not available for this podcast'**
  String get no_transcript_available_label;

  /// Hint text for transcript search box
  ///
  /// In en, this message translates to:
  /// **'Search transcript'**
  String get search_transcript_label;

  /// Auto scroll switch label
  ///
  /// In en, this message translates to:
  /// **'Follow transcript'**
  String get auto_scroll_transcript_label;

  /// Link to why no transcript is available
  ///
  /// In en, this message translates to:
  /// **'Why not?'**
  String get transcript_why_not_label;

  /// Language specific link
  ///
  /// In en, this message translates to:
  /// **'https://www.pinepods.online/docs/Features/Transcript'**
  String get transcript_why_not_url;

  /// Describes podcast details page
  ///
  /// In en, this message translates to:
  /// **'Podcast details and episodes page'**
  String get semantics_podcast_details_header;

  /// Describes list layout button
  ///
  /// In en, this message translates to:
  /// **'List layout'**
  String get semantics_layout_option_list;

  /// Describes compact grid layout button
  ///
  /// In en, this message translates to:
  /// **'Compact grid layout'**
  String get semantics_layout_option_compact_grid;

  /// Describes grid layout button
  ///
  /// In en, this message translates to:
  /// **'Grid layout'**
  String get semantics_layout_option_grid;

  /// Describes the mini player
  ///
  /// In en, this message translates to:
  /// **'Mini player. Swipe right to play/pause button. Activate to open main player window'**
  String get semantics_mini_player_header;

  /// Describes the main player
  ///
  /// In en, this message translates to:
  /// **'Main player window'**
  String get semantics_main_player_header;

  /// Describes play/pause toggle button
  ///
  /// In en, this message translates to:
  /// **'Play/pause toggle'**
  String get semantics_play_pause_toggle;

  /// Describes speed adjustment control
  ///
  /// In en, this message translates to:
  /// **'Decrease playback speed'**
  String get semantics_decrease_playback_speed;

  /// Describes speed adjustment control
  ///
  /// In en, this message translates to:
  /// **'Increase playback speed'**
  String get semantics_increase_playback_speed;

  /// Describes podcast collapse/expand button
  ///
  /// In en, this message translates to:
  /// **'Expand podcast description'**
  String get semantics_expand_podcast_description;

  /// Describes podcast collapse/expand button
  ///
  /// In en, this message translates to:
  /// **'Collapse podcast description'**
  String get semantics_collapse_podcast_description;

  /// Describes add to queue button
  ///
  /// In en, this message translates to:
  /// **'Add episode to queue'**
  String get semantics_add_to_queue;

  /// Describes add to queue button
  ///
  /// In en, this message translates to:
  /// **'Remove episode from queue'**
  String get semantics_remove_from_queue;

  /// Describes mark played button
  ///
  /// In en, this message translates to:
  /// **'Mark Episode as played'**
  String get semantics_mark_episode_played;

  /// Describes mark unplayed button
  ///
  /// In en, this message translates to:
  /// **'Mark Episode as un-played'**
  String get semantics_mark_episode_unplayed;

  /// Describes episode tile options when collapsed
  ///
  /// In en, this message translates to:
  /// **'Episode list item. Showing image, summary and main controls.'**
  String get semantics_episode_tile_collapsed;

  /// Describes episode tile options when expanded
  ///
  /// In en, this message translates to:
  /// **'Episode list item. Showing description, main controls and additional controls.'**
  String get semantics_episode_tile_expanded;

  /// Describes episode tile options when collapsed
  ///
  /// In en, this message translates to:
  /// **'expand and show more details and additional options'**
  String get semantics_episode_tile_collapsed_hint;

  /// Describes episode tile options when expanded
  ///
  /// In en, this message translates to:
  /// **'collapse and show summary, download and play control'**
  String get semantics_episode_tile_expanded_hint;

  /// Describes off sleep label
  ///
  /// In en, this message translates to:
  /// **'Off'**
  String get sleep_off_label;

  /// Describes end of episode sleep label
  ///
  /// In en, this message translates to:
  /// **'End of episode'**
  String get sleep_episode_label;

  /// Describes the number of minutes to sleep
  ///
  /// In en, this message translates to:
  /// **'{minutes} minutes'**
  String sleep_minute_label(String minutes);

  /// Describes sleep timer label
  ///
  /// In en, this message translates to:
  /// **'Sleep Timer'**
  String get sleep_timer_label;

  /// Feedback option in main menu
  ///
  /// In en, this message translates to:
  /// **'Feedback'**
  String get feedback_menu_item_label;

  /// Podcast details overflow menu
  ///
  /// In en, this message translates to:
  /// **'Options menu'**
  String get podcast_options_overflow_menu_semantic_label;

  /// Spoken when search in progress.
  ///
  /// In en, this message translates to:
  /// **'Searching, please wait.'**
  String get semantic_announce_searching;

  /// Placed on options handle when screen reader enabled.
  ///
  /// In en, this message translates to:
  /// **'Open playing options slider'**
  String get semantic_playing_options_expand_label;

  /// Placed on options handle when screen reader enabled.
  ///
  /// In en, this message translates to:
  /// **'Close playing options slider'**
  String get semantic_playing_options_collapse_label;

  /// Placed around podcast image on main player
  ///
  /// In en, this message translates to:
  /// **'Podcast artwork'**
  String get semantic_podcast_artwork_label;

  /// Placed around chapter link
  ///
  /// In en, this message translates to:
  /// **'Chapter web link'**
  String get semantic_chapter_link_label;

  /// Placed around chapter
  ///
  /// In en, this message translates to:
  /// **'Current chapter'**
  String get semantic_current_chapter_label;

  /// Episodes not filtered
  ///
  /// In en, this message translates to:
  /// **'None'**
  String get episode_filter_none_label;

  /// Only show episodes that have been started
  ///
  /// In en, this message translates to:
  /// **'Started'**
  String get episode_filter_started_label;

  /// Only show episodes that have been played
  ///
  /// In en, this message translates to:
  /// **'Played'**
  String get episode_filter_played_label;

  /// Only show episodes that have not been played
  ///
  /// In en, this message translates to:
  /// **'Unplayed'**
  String get episode_filter_unplayed_label;

  /// No Episodes title
  ///
  /// In en, this message translates to:
  /// **'No Episodes Found'**
  String get episode_filter_no_episodes_title_label;

  /// No episodes found description
  ///
  /// In en, this message translates to:
  /// **'This podcast has no episodes matching your search criteria and filter'**
  String get episode_filter_no_episodes_title_description;

  /// Clear filters button
  ///
  /// In en, this message translates to:
  /// **'Clear Filters'**
  String get episode_filter_clear_filters_button_label;

  /// Episode filter semantic label
  ///
  /// In en, this message translates to:
  /// **'Filter episodes'**
  String get episode_filter_semantic_label;

  /// Episode sort semantic label
  ///
  /// In en, this message translates to:
  /// **'Sort episodes'**
  String get episode_sort_semantic_label;

  /// Episode default sort
  ///
  /// In en, this message translates to:
  /// **'Default'**
  String get episode_sort_none_label;

  /// Episode latest first sort
  ///
  /// In en, this message translates to:
  /// **'Latest first'**
  String get episode_sort_latest_first_label;

  /// Episode earliest first sort
  ///
  /// In en, this message translates to:
  /// **'Earliest first'**
  String get episode_sort_earliest_first_label;

  /// Episode alphabetical ascending
  ///
  /// In en, this message translates to:
  /// **'Alphabetical A-Z'**
  String get episode_sort_alphabetical_ascending_label;

  /// Episode alphabetical descending
  ///
  /// In en, this message translates to:
  /// **'Alphabetical Z-A'**
  String get episode_sort_alphabetical_descending_label;

  /// Open show website in browser
  ///
  /// In en, this message translates to:
  /// **'Open show website'**
  String get open_show_website_label;

  /// Menu item to refresh episodes
  ///
  /// In en, this message translates to:
  /// **'Refresh episodes'**
  String get refresh_feed_label;

  /// Replaces default scrim label for layout selector bottom sheet.
  ///
  /// In en, this message translates to:
  /// **'Dismiss layout selector'**
  String get scrim_layout_selector;

  /// Episode position slider control label
  ///
  /// In en, this message translates to:
  /// **'Episode position'**
  String get now_playing_episode_position;

  /// Episode time remaining slider control label
  ///
  /// In en, this message translates to:
  /// **'Time remaining'**
  String get now_playing_episode_time_remaining;

  /// Semantic label for the resume button
  ///
  /// In en, this message translates to:
  /// **'Resume episode'**
  String get resume_button_label;

  /// Semantic label for the play downloaded episode button
  ///
  /// In en, this message translates to:
  /// **'Play downloaded episode'**
  String get play_download_button_label;

  /// Semantic label for the play cancel download button
  ///
  /// In en, this message translates to:
  /// **'Cancel download'**
  String get cancel_download_button_label;

  /// Semantic label for the show info button.
  ///
  /// In en, this message translates to:
  /// **'Show episode information'**
  String get episode_details_button_label;

  /// Replaces default scrim label for custom.
  ///
  /// In en, this message translates to:
  /// **'Dismiss sleep timer selector'**
  String get scrim_sleep_timer_selector;

  /// Replaces default scrim label for custom.
  ///
  /// In en, this message translates to:
  /// **'Dismiss playback speed selector'**
  String get scrim_speed_selector;

  /// For current sleep setting
  ///
  /// In en, this message translates to:
  /// **'Current value'**
  String get semantic_current_value_label;

  /// Replaces default scrim label for episode details bottom sheet.
  ///
  /// In en, this message translates to:
  /// **'Dismiss episode details'**
  String get scrim_episode_details_selector;

  /// Replaces default scrim label for episode sort bottom sheet.
  ///
  /// In en, this message translates to:
  /// **'Dismiss episode sort'**
  String get scrim_episode_sort_selector;

  /// Replaces default scrim label for episode filter bottom sheet.
  ///
  /// In en, this message translates to:
  /// **'Dismiss episode filter'**
  String get scrim_episode_filter_selector;

  /// Hint text for episode search box
  ///
  /// In en, this message translates to:
  /// **'Search episodes'**
  String get search_episodes_label;
}

class _AppLocalizationsDelegate
    extends LocalizationsDelegate<AppLocalizations> {
  const _AppLocalizationsDelegate();

  @override
  Future<AppLocalizations> load(Locale locale) {
    return SynchronousFuture<AppLocalizations>(lookupAppLocalizations(locale));
  }

  @override
  bool isSupported(Locale locale) =>
      <String>['de', 'en', 'it'].contains(locale.languageCode);

  @override
  bool shouldReload(_AppLocalizationsDelegate old) => false;
}

AppLocalizations lookupAppLocalizations(Locale locale) {
  // Lookup logic when only language code is specified.
  switch (locale.languageCode) {
    case 'de':
      return AppLocalizationsDe();
    case 'en':
      return AppLocalizationsEn();
    case 'it':
      return AppLocalizationsIt();
  }

  throw FlutterError(
    'AppLocalizations.delegate failed to load unsupported locale "$locale". This is likely '
    'an issue with the localizations generation tool. Please file an issue '
    'on GitHub with a reproducible sample app and the gen-l10n configuration '
    'that was used.',
  );
}
