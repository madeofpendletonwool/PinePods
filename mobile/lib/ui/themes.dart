// Copyright 2020 Ben Hills and the project contributors. All rights reserved.
// Use of this source code is governed by a BSD-style license that can be
// found in the LICENSE file.

import 'package:flutter/material.dart';
import 'package:flutter/services.dart';

final ThemeData _lightTheme = _buildLightTheme();
final ThemeData _darkTheme = _buildDarkTheme();
final ThemeData _nordTheme = _buildNordTheme();
final ThemeData _draculaTheme = _buildDraculaTheme();
final ThemeData _nordicTheme = _buildNordicTheme();
final ThemeData _gruvboxDarkTheme = _buildGruvboxDarkTheme();
final ThemeData _catppuccinMochaTheme = _buildCatppuccinMochaTheme();
final ThemeData _abyssTheme = _buildAbyssTheme();
final ThemeData _cyberSynthwaveTheme = _buildCyberSynthwaveTheme();
final ThemeData _midnightOceanTheme = _buildMidnightOceanTheme();
final ThemeData _forestDepthsTheme = _buildForestDepthsTheme();
final ThemeData _sunsetHorizonTheme = _buildSunsetHorizonTheme();
final ThemeData _arcticFrostTheme = _buildArcticFrostTheme();
final ThemeData _neonTheme = _buildNeonTheme();
final ThemeData _kimbieTheme = _buildKimbieTheme();
final ThemeData _gruvboxLightTheme = _buildGruvboxLightTheme();
final ThemeData _greenMeanieTheme = _buildGreenMeanieTheme();
final ThemeData _wildberriesTheme = _buildWildberriesTheme();
final ThemeData _softLavenderTheme = _buildSoftLavenderTheme();
final ThemeData _mintyFreshTheme = _buildMintyFreshTheme();
final ThemeData _warmVanillaTheme = _buildWarmVanillaTheme();
final ThemeData _coastalBlueTheme = _buildCoastalBlueTheme();
final ThemeData _paperCreamTheme = _buildPaperCreamTheme();
final ThemeData _githubLightTheme = _buildGithubLightTheme();
final ThemeData _hotDogStandTheme = _buildHotDogStandTheme();

class ThemeDefinition {
  final String key;
  final String name;
  final String description;
  final ThemeData themeData;
  final bool isDark;

  const ThemeDefinition({
    required this.key,
    required this.name,
    required this.description,
    required this.themeData,
    required this.isDark,
  });
}

class ThemeRegistry {
  static final Map<String, ThemeDefinition> _themes = {
    'Light': ThemeDefinition(
      key: 'Light',
      name: 'Light',
      description: 'Clean and bright theme',
      themeData: _lightTheme,
      isDark: false,
    ),
    'Dark': ThemeDefinition(
      key: 'Dark',
      name: 'Dark',
      description: 'Classic dark theme',
      themeData: _darkTheme,
      isDark: true,
    ),
    'Nordic': ThemeDefinition(
      key: 'Nordic',
      name: 'Nordic',
      description: 'Cool Nordic inspired theme',
      themeData: _nordTheme,
      isDark: true,
    ),
    'Dracula': ThemeDefinition(
      key: 'Dracula',
      name: 'Dracula',
      description: 'Popular dark theme with purple accents',
      themeData: _draculaTheme,
      isDark: true,
    ),
    'Nordic Light': ThemeDefinition(
      key: 'Nordic Light',
      name: 'Nordic Light',
      description: 'Light Nordic inspired theme',
      themeData: _nordicTheme,
      isDark: false,
    ),
    'Gruvbox Dark': ThemeDefinition(
      key: 'Gruvbox Dark',
      name: 'Gruvbox Dark',
      description: 'Retro groove dark theme',
      themeData: _gruvboxDarkTheme,
      isDark: true,
    ),
    'Catppuccin Mocha Mauve': ThemeDefinition(
      key: 'Catppuccin Mocha Mauve',
      name: 'Catppuccin Mocha Mauve',
      description: 'Soothing pastel dark theme',
      themeData: _catppuccinMochaTheme,
      isDark: true,
    ),
    'Abyss': ThemeDefinition(
      key: 'Abyss',
      name: 'Abyss',
      description: 'Deep space darkness',
      themeData: _abyssTheme,
      isDark: true,
    ),
    'Cyber Synthwave': ThemeDefinition(
      key: 'Cyber Synthwave',
      name: 'Cyber Synthwave',
      description: 'Retro cyberpunk vibes',
      themeData: _cyberSynthwaveTheme,
      isDark: true,
    ),
    'Midnight Ocean': ThemeDefinition(
      key: 'Midnight Ocean',
      name: 'Midnight Ocean',
      description: 'Dark blue oceanic theme',
      themeData: _midnightOceanTheme,
      isDark: true,
    ),
    'Forest Depths': ThemeDefinition(
      key: 'Forest Depths',
      name: 'Forest Depths',
      description: 'Deep forest green theme',
      themeData: _forestDepthsTheme,
      isDark: true,
    ),
    'Sunset Horizon': ThemeDefinition(
      key: 'Sunset Horizon',
      name: 'Sunset Horizon',
      description: 'Warm sunset colors',
      themeData: _sunsetHorizonTheme,
      isDark: true,
    ),
    'Arctic Frost': ThemeDefinition(
      key: 'Arctic Frost',
      name: 'Arctic Frost',
      description: 'Cool arctic theme',
      themeData: _arcticFrostTheme,
      isDark: true,
    ),
    'Neon': ThemeDefinition(
      key: 'Neon',
      name: 'Neon',
      description: 'Bright neon colors',
      themeData: _neonTheme,
      isDark: true,
    ),
    'Kimbie': ThemeDefinition(
      key: 'Kimbie',
      name: 'Kimbie',
      description: 'Warm brown theme',
      themeData: _kimbieTheme,
      isDark: true,
    ),
    'Gruvbox Light': ThemeDefinition(
      key: 'Gruvbox Light',
      name: 'Gruvbox Light',
      description: 'Retro groove light theme',
      themeData: _gruvboxLightTheme,
      isDark: false,
    ),
    'Greenie Meanie': ThemeDefinition(
      key: 'Greenie Meanie',
      name: 'Greenie Meanie',
      description: 'Matrix green theme',
      themeData: _greenMeanieTheme,
      isDark: true,
    ),
    'Wildberries': ThemeDefinition(
      key: 'Wildberries',
      name: 'Wildberries',
      description: 'Purple berry theme',
      themeData: _wildberriesTheme,
      isDark: true,
    ),
    'Soft Lavender': ThemeDefinition(
      key: 'Soft Lavender',
      name: 'Soft Lavender',
      description: 'Gentle purple light theme',
      themeData: _softLavenderTheme,
      isDark: false,
    ),
    'Minty Fresh': ThemeDefinition(
      key: 'Minty Fresh',
      name: 'Minty Fresh',
      description: 'Cool mint green theme',
      themeData: _mintyFreshTheme,
      isDark: false,
    ),
    'Warm Vanilla': ThemeDefinition(
      key: 'Warm Vanilla',
      name: 'Warm Vanilla',
      description: 'Cozy vanilla theme',
      themeData: _warmVanillaTheme,
      isDark: false,
    ),
    'Coastal Blue': ThemeDefinition(
      key: 'Coastal Blue',
      name: 'Coastal Blue',
      description: 'Ocean blue theme',
      themeData: _coastalBlueTheme,
      isDark: false,
    ),
    'Paper Cream': ThemeDefinition(
      key: 'Paper Cream',
      name: 'Paper Cream',
      description: 'Vintage paper theme',
      themeData: _paperCreamTheme,
      isDark: false,
    ),
    'Github Light': ThemeDefinition(
      key: 'Github Light',
      name: 'Github Light',
      description: 'Clean GitHub-inspired theme',
      themeData: _githubLightTheme,
      isDark: false,
    ),
    'Hot Dog Stand - MY EYES': ThemeDefinition(
      key: 'Hot Dog Stand - MY EYES',
      name: 'Hot Dog Stand - MY EYES',
      description: 'Eye-searing hot dog stand theme',
      themeData: _hotDogStandTheme,
      isDark: true,
    ),
  };

  static Map<String, ThemeDefinition> get themes => _themes;
  static List<String> get themeKeys => _themes.keys.toList();
  static List<ThemeDefinition> get themeList => _themes.values.toList();
  
  static ThemeDefinition getTheme(String key) {
    return _themes[key] ?? _themes['Dark']!;
  }
  
  static ThemeData getThemeData(String key) {
    return getTheme(key).themeData;
  }
  
  static bool isValidTheme(String key) {
    return _themes.containsKey(key);
  }
}

ThemeData _buildLightTheme() {
  final base = ThemeData.light(useMaterial3: false);

  return base.copyWith(
    colorScheme: const ColorScheme.light(
      primary: Color(0xffff9800),
      secondary: Color(0xfffb8c00),
      surface: Color(0xffffffff),
      error: Color(0xffd32f2f),
      onSurface: Color(0xfffb8c00),
    ),
    bottomAppBarTheme: const BottomAppBarThemeData().copyWith(
      color: const Color(0xffffffff),
    ),
    cardTheme: const CardThemeData().copyWith(
      color: const Color(0xffffa900),
      shadowColor: const Color(0xfff57c00),
    ),
    brightness: Brightness.light,
    primaryColor: const Color(0xffff9800),
    primaryColorLight: const Color(0xffffe0b2),
    primaryColorDark: const Color(0xfff57c00),
    canvasColor: const Color(0xffffffff),
    scaffoldBackgroundColor: const Color(0xffffffff),
    cardColor: const Color(0xffffffff),
    dividerColor: const Color(0x1f000000),
    highlightColor: const Color(0x66bcbcbc),
    splashColor: const Color(0x66c8c8c8),
    unselectedWidgetColor: const Color(0x8a000000),
    disabledColor: const Color(0x61000000),
    secondaryHeaderColor: const Color(0xffffffff),
    dialogBackgroundColor: const Color(0xffffffff),
    indicatorColor: Colors.blueAccent,
    hintColor: const Color(0x8a000000),
    primaryTextTheme: Typography.material2021(platform: TargetPlatform.android).black,
    textTheme: Typography.material2021(
      platform: TargetPlatform.android,
    ).black,
    primaryIconTheme: IconThemeData(color: Colors.grey[800]),
    buttonTheme: base.buttonTheme.copyWith(
      buttonColor: Colors.orange,
    ),
    iconTheme: base.iconTheme.copyWith(
      color: Colors.orange,
    ),
    sliderTheme: const SliderThemeData().copyWith(
      valueIndicatorColor: Colors.orange,
      trackHeight: 2.0,
      thumbShape: const RoundSliderThumbShape(
        enabledThumbRadius: 6.0,
        disabledThumbRadius: 6.0,
      ),
    ),
    appBarTheme: base.appBarTheme.copyWith(
        backgroundColor: Colors.white,
        foregroundColor: Colors.black,
        systemOverlayStyle: SystemUiOverlayStyle.light.copyWith(
          systemNavigationBarIconBrightness: Brightness.dark,
          systemNavigationBarColor: Colors.white,
          statusBarIconBrightness: Brightness.dark,
        )),
    snackBarTheme: base.snackBarTheme.copyWith(
      actionTextColor: Colors.white,
    ),
    outlinedButtonTheme: OutlinedButtonThemeData(
      style: OutlinedButton.styleFrom(foregroundColor: Colors.grey[800]),
    ),
  );
}

ThemeData _buildDarkTheme() {
  final base = ThemeData.dark(useMaterial3: false);

  return base.copyWith(
    colorScheme: const ColorScheme.dark(
      primary: Color(0xffffffff),
      secondary: Color(0xfffb8c00),
      surface: Color(0xff222222),
      error: Color(0xffd32f2f),
      onSurface: Color(0xffffffff),
    ),
    bottomAppBarTheme: const BottomAppBarThemeData().copyWith(
      color: const Color(0xff222222),
    ),
    cardTheme: const CardThemeData().copyWith(
      color: const Color(0xff444444),
      shadowColor: const Color(0x77ffffff),
    ),
    brightness: Brightness.dark,
    primaryColor: const Color(0xffffffff),
    primaryColorLight: const Color(0xffffe0b2),
    primaryColorDark: const Color(0xfff57c00),
    canvasColor: const Color(0xff000000),
    scaffoldBackgroundColor: const Color(0xff000000),
    cardColor: const Color(0xff0F0F0F),
    dividerColor: const Color(0xff444444),
    highlightColor: const Color(0xff222222),
    splashColor: const Color(0x66c8c8c8),
    unselectedWidgetColor: Colors.white,
    disabledColor: const Color(0x77ffffff),
    secondaryHeaderColor: const Color(0xff222222),
    dialogBackgroundColor: const Color(0xff222222),
    indicatorColor: Colors.orange,
    hintColor: const Color(0x80ffffff),
    primaryTextTheme: Typography.material2021(platform: TargetPlatform.android).white,
    textTheme: Typography.material2021(platform: TargetPlatform.android).white,
    primaryIconTheme: const IconThemeData(color: Colors.white),
    iconTheme: base.iconTheme.copyWith(
      color: Colors.white,
    ),
    dividerTheme: base.dividerTheme.copyWith(
      color: const Color(0xff444444),
    ),
    sliderTheme: const SliderThemeData().copyWith(
      valueIndicatorColor: Colors.white,
      trackHeight: 2.0,
      thumbShape: const RoundSliderThumbShape(
        enabledThumbRadius: 6.0,
        disabledThumbRadius: 6.0,
      ),
    ),
    appBarTheme: base.appBarTheme.copyWith(
        backgroundColor: const Color(0xff222222),
        foregroundColor: Colors.white,
        shadowColor: const Color(0xff222222),
        elevation: 1.0,
        systemOverlayStyle: SystemUiOverlayStyle.light.copyWith(
          systemNavigationBarIconBrightness: Brightness.light,
          systemNavigationBarColor: const Color(0xff222222),
          statusBarIconBrightness: Brightness.light,
        )),
    snackBarTheme: base.snackBarTheme.copyWith(
      actionTextColor: Colors.orange,
    ),
    outlinedButtonTheme: OutlinedButtonThemeData(
      style: OutlinedButton.styleFrom(
        foregroundColor: const Color(0xffffffff),
        side: const BorderSide(
          color: Color(0xffffffff),
          style: BorderStyle.solid,
        ),
      ),
    ),
  );
}

ThemeData _buildNordTheme() {
  final base = ThemeData.dark(useMaterial3: false);
  
  return base.copyWith(
    colorScheme: const ColorScheme.dark(
      primary: Color(0xff3550af),
      secondary: Color(0xff5d80aa),
      surface: Color(0xff2e3440),
      error: Color(0xffbf616a),
      onSurface: Color(0xfff6f5f4),
      background: Color(0xff3C4252),
    ),
    bottomAppBarTheme: const BottomAppBarThemeData().copyWith(
      color: const Color(0xff2e3440),
    ),
    cardTheme: const CardThemeData().copyWith(
      color: const Color(0xff2b2f3a),
      shadowColor: const Color(0xff3e4555),
    ),
    brightness: Brightness.dark,
    primaryColor: const Color(0xff3550af),
    canvasColor: const Color(0xff3C4252),
    scaffoldBackgroundColor: const Color(0xff3C4252),
    cardColor: const Color(0xff2b2f3a),
    dividerColor: const Color(0xff6d747f),
    highlightColor: const Color(0xff5d80aa),
    splashColor: const Color(0xff5d80aa),
    unselectedWidgetColor: const Color(0xfff6f5f4),
    disabledColor: const Color(0x776d747f),
    secondaryHeaderColor: const Color(0xff2e3440),
    dialogBackgroundColor: const Color(0xff2e3440),
    indicatorColor: const Color(0xff3550af),
    hintColor: const Color(0x80f6f5f4),
    primaryTextTheme: Typography.material2021(platform: TargetPlatform.android).white,
    textTheme: Typography.material2021(platform: TargetPlatform.android).white,
    primaryIconTheme: const IconThemeData(color: Color(0xfff6f5f4)),
    iconTheme: base.iconTheme.copyWith(
      color: const Color(0xfff6f5f4),
    ),
    dividerTheme: base.dividerTheme.copyWith(
      color: const Color(0xff6d747f),
    ),
    sliderTheme: const SliderThemeData().copyWith(
      valueIndicatorColor: const Color(0xff3550af),
      trackHeight: 2.0,
      thumbShape: const RoundSliderThumbShape(
        enabledThumbRadius: 6.0,
        disabledThumbRadius: 6.0,
      ),
    ),
    appBarTheme: base.appBarTheme.copyWith(
      backgroundColor: const Color(0xff2e3440),
      foregroundColor: const Color(0xfff6f5f4),
      shadowColor: const Color(0xff2e3440),
      elevation: 1.0,
      systemOverlayStyle: SystemUiOverlayStyle.light.copyWith(
        systemNavigationBarIconBrightness: Brightness.light,
        systemNavigationBarColor: const Color(0xff2e3440),
        statusBarIconBrightness: Brightness.light,
      ),
    ),
    snackBarTheme: base.snackBarTheme.copyWith(
      actionTextColor: const Color(0xff3550af),
    ),
    outlinedButtonTheme: OutlinedButtonThemeData(
      style: OutlinedButton.styleFrom(
        foregroundColor: const Color(0xfff6f5f4),
        side: const BorderSide(
          color: Color(0xff3550af),
          style: BorderStyle.solid,
        ),
      ),
    ),
    elevatedButtonTheme: ElevatedButtonThemeData(
      style: ElevatedButton.styleFrom(
        backgroundColor: const Color(0xff3550af),
        foregroundColor: const Color(0xfff6f5f4),
      ),
    ),
  );
}

ThemeData _buildDraculaTheme() {
  final base = ThemeData.dark(useMaterial3: false);
  
  return base.copyWith(
    colorScheme: const ColorScheme.dark(
      primary: Color(0xffbd93f9),
      secondary: Color(0xff6590fd),
      surface: Color(0xff282A36),
      error: Color(0xffff5555),
      onSurface: Color(0xfff6f5f4),
      background: Color(0xff282A36),
    ),
    bottomAppBarTheme: const BottomAppBarThemeData().copyWith(
      color: const Color(0xff262626),
    ),
    cardTheme: const CardThemeData().copyWith(
      color: const Color(0xff191a21),
      shadowColor: const Color(0xff292e42),
    ),
    brightness: Brightness.dark,
    primaryColor: const Color(0xffbd93f9),
    canvasColor: const Color(0xff282A36),
    scaffoldBackgroundColor: const Color(0xff282A36),
    cardColor: const Color(0xff191a21),
    dividerColor: const Color(0xff727580),
    highlightColor: const Color(0xff4b5563),
    splashColor: const Color(0xff4b5563),
    unselectedWidgetColor: const Color(0xfff6f5f4),
    disabledColor: const Color(0x77727580),
    secondaryHeaderColor: const Color(0xff262626),
    dialogBackgroundColor: const Color(0xff262626),
    indicatorColor: const Color(0xffbd93f9),
    hintColor: const Color(0x80f6f5f4),
    primaryTextTheme: Typography.material2021(platform: TargetPlatform.android).white,
    textTheme: Typography.material2021(platform: TargetPlatform.android).white,
    primaryIconTheme: const IconThemeData(color: Color(0xfff6f5f4)),
    iconTheme: base.iconTheme.copyWith(
      color: const Color(0xfff6f5f4),
    ),
    dividerTheme: base.dividerTheme.copyWith(
      color: const Color(0xff727580),
    ),
    sliderTheme: const SliderThemeData().copyWith(
      valueIndicatorColor: const Color(0xffbd93f9),
      trackHeight: 2.0,
      thumbShape: const RoundSliderThumbShape(
        enabledThumbRadius: 6.0,
        disabledThumbRadius: 6.0,
      ),
    ),
    appBarTheme: base.appBarTheme.copyWith(
      backgroundColor: const Color(0xff262626),
      foregroundColor: const Color(0xfff6f5f4),
      shadowColor: const Color(0xff262626),
      elevation: 1.0,
      systemOverlayStyle: SystemUiOverlayStyle.light.copyWith(
        systemNavigationBarIconBrightness: Brightness.light,
        systemNavigationBarColor: const Color(0xff262626),
        statusBarIconBrightness: Brightness.light,
      ),
    ),
    snackBarTheme: base.snackBarTheme.copyWith(
      actionTextColor: const Color(0xffbd93f9),
    ),
    outlinedButtonTheme: OutlinedButtonThemeData(
      style: OutlinedButton.styleFrom(
        foregroundColor: const Color(0xfff6f5f4),
        side: const BorderSide(
          color: Color(0xffbd93f9),
          style: BorderStyle.solid,
        ),
      ),
    ),
    elevatedButtonTheme: ElevatedButtonThemeData(
      style: ElevatedButton.styleFrom(
        backgroundColor: const Color(0xffbd93f9),
        foregroundColor: const Color(0xff000000),
      ),
    ),
  );
}

ThemeData _buildNordicTheme() {
  final base = ThemeData.light(useMaterial3: false);
  
  return base.copyWith(
    colorScheme: const ColorScheme.light(
      primary: Color(0xff2a85cf),
      secondary: Color(0xff2984ce),
      surface: Color(0xffd8dee9),
      error: Color(0xffd32f2f),
      onSurface: Color(0xff656d76),
      background: Color(0xffeceff4),
    ),
    bottomAppBarTheme: const BottomAppBarThemeData().copyWith(
      color: const Color(0xffe5e9f0),
    ),
    cardTheme: const CardThemeData().copyWith(
      color: const Color(0xffd8dee9),
      shadowColor: const Color(0xffd8dee9),
    ),
    brightness: Brightness.light,
    primaryColor: const Color(0xff2a85cf),
    canvasColor: const Color(0xffeceff4),
    scaffoldBackgroundColor: const Color(0xffeceff4),
    cardColor: const Color(0xffd8dee9),
    dividerColor: const Color(0xff878d95),
    highlightColor: const Color(0xff2a85cf),
    splashColor: const Color(0xff2a85cf),
    unselectedWidgetColor: const Color(0xff656d76),
    disabledColor: const Color(0x77878d95),
    secondaryHeaderColor: const Color(0xffe5e9f0),
    dialogBackgroundColor: const Color(0xffe5e9f0),
    indicatorColor: const Color(0xff2984ce),
    hintColor: const Color(0x80656d76),
    primaryTextTheme: Typography.material2021(platform: TargetPlatform.android).black,
    textTheme: Typography.material2021(platform: TargetPlatform.android).black,
    primaryIconTheme: const IconThemeData(color: Color(0xff656d76)),
    iconTheme: base.iconTheme.copyWith(
      color: const Color(0xff656d76),
    ),
    dividerTheme: base.dividerTheme.copyWith(
      color: const Color(0xff878d95),
    ),
    sliderTheme: const SliderThemeData().copyWith(
      valueIndicatorColor: const Color(0xff2984ce),
      trackHeight: 2.0,
      thumbShape: const RoundSliderThumbShape(
        enabledThumbRadius: 6.0,
        disabledThumbRadius: 6.0,
      ),
    ),
    appBarTheme: base.appBarTheme.copyWith(
      backgroundColor: const Color(0xffe5e9f0),
      foregroundColor: const Color(0xff656d76),
      shadowColor: const Color(0xffe5e9f0),
      elevation: 1.0,
      systemOverlayStyle: SystemUiOverlayStyle.dark.copyWith(
        systemNavigationBarIconBrightness: Brightness.dark,
        systemNavigationBarColor: const Color(0xffe5e9f0),
        statusBarIconBrightness: Brightness.dark,
      ),
    ),
    snackBarTheme: base.snackBarTheme.copyWith(
      actionTextColor: const Color(0xff2a85cf),
    ),
    outlinedButtonTheme: OutlinedButtonThemeData(
      style: OutlinedButton.styleFrom(
        foregroundColor: const Color(0xff656d76),
        side: const BorderSide(
          color: Color(0xff2a85cf),
          style: BorderStyle.solid,
        ),
      ),
    ),
    elevatedButtonTheme: ElevatedButtonThemeData(
      style: ElevatedButton.styleFrom(
        backgroundColor: const Color(0xff2a85cf),
        foregroundColor: const Color(0xffffffff),
      ),
    ),
  );
}

ThemeData _buildGruvboxDarkTheme() {
  final base = ThemeData.dark(useMaterial3: false);
  
  return base.copyWith(
    colorScheme: const ColorScheme.dark(
      primary: Color(0xff424314),
      secondary: Color(0xff6f701b),
      surface: Color(0xff282828),
      error: Color(0xffcc241d),
      onSurface: Color(0xff868729),
      background: Color(0xff32302f),
    ),
    bottomAppBarTheme: const BottomAppBarThemeData().copyWith(
      color: const Color(0xff282828),
    ),
    cardTheme: const CardThemeData().copyWith(
      color: const Color(0xff302e2e),
      shadowColor: const Color(0xff303648),
    ),
    brightness: Brightness.dark,
    primaryColor: const Color(0xff424314),
    canvasColor: const Color(0xff32302f),
    scaffoldBackgroundColor: const Color(0xff32302f),
    cardColor: const Color(0xff302e2e),
    dividerColor: const Color(0xffebdbb2),
    highlightColor: const Color(0xff59544a),
    splashColor: const Color(0xff59544a),
    unselectedWidgetColor: const Color(0xff868729),
    disabledColor: const Color(0x77ebdbb2),
    secondaryHeaderColor: const Color(0xff282828),
    dialogBackgroundColor: const Color(0xff282828),
    indicatorColor: const Color(0xff424314),
    hintColor: const Color(0x80868729),
    primaryTextTheme: Typography.material2021(platform: TargetPlatform.android).white,
    textTheme: Typography.material2021(platform: TargetPlatform.android).white,
    primaryIconTheme: const IconThemeData(color: Color(0xff868729)),
    iconTheme: base.iconTheme.copyWith(
      color: const Color(0xff868729),
    ),
    dividerTheme: base.dividerTheme.copyWith(
      color: const Color(0xffebdbb2),
    ),
    sliderTheme: const SliderThemeData().copyWith(
      valueIndicatorColor: const Color(0xff424314),
      trackHeight: 2.0,
      thumbShape: const RoundSliderThumbShape(
        enabledThumbRadius: 6.0,
        disabledThumbRadius: 6.0,
      ),
    ),
    appBarTheme: base.appBarTheme.copyWith(
      backgroundColor: const Color(0xff282828),
      foregroundColor: const Color(0xff868729),
      shadowColor: const Color(0xff282828),
      elevation: 1.0,
      systemOverlayStyle: SystemUiOverlayStyle.light.copyWith(
        systemNavigationBarIconBrightness: Brightness.light,
        systemNavigationBarColor: const Color(0xff282828),
        statusBarIconBrightness: Brightness.light,
      ),
    ),
    snackBarTheme: base.snackBarTheme.copyWith(
      actionTextColor: const Color(0xff424314),
    ),
    outlinedButtonTheme: OutlinedButtonThemeData(
      style: OutlinedButton.styleFrom(
        foregroundColor: const Color(0xff868729),
        side: const BorderSide(
          color: Color(0xff424314),
          style: BorderStyle.solid,
        ),
      ),
    ),
    elevatedButtonTheme: ElevatedButtonThemeData(
      style: ElevatedButton.styleFrom(
        backgroundColor: const Color(0xff424314),
        foregroundColor: const Color(0xff868729),
      ),
    ),
  );
}

ThemeData _buildCatppuccinMochaTheme() {
  final base = ThemeData.dark(useMaterial3: false);
  
  return base.copyWith(
    colorScheme: const ColorScheme.dark(
      primary: Color(0xffcba6f7),
      secondary: Color(0xfff5c2e7),
      surface: Color(0xff313244),
      error: Color(0xfff38ba8),
      onSurface: Color(0xffcdd6f4),
      background: Color(0xff1e1e2e),
    ),
    bottomAppBarTheme: const BottomAppBarThemeData().copyWith(
      color: const Color(0xff11111b),
    ),
    cardTheme: const CardThemeData().copyWith(
      color: const Color(0xff313244),
      shadowColor: const Color(0xff313244),
    ),
    brightness: Brightness.dark,
    primaryColor: const Color(0xffcba6f7),
    canvasColor: const Color(0xff1e1e2e),
    scaffoldBackgroundColor: const Color(0xff1e1e2e),
    cardColor: const Color(0xff313244),
    dividerColor: const Color(0xffcba6f7),
    highlightColor: const Color(0xff6c7086),
    splashColor: const Color(0xff6c7086),
    unselectedWidgetColor: const Color(0xffcdd6f4),
    disabledColor: const Color(0x77bac2de),
    secondaryHeaderColor: const Color(0xff11111b),
    dialogBackgroundColor: const Color(0xff11111b),
    indicatorColor: const Color(0xffa6e3a1),
    hintColor: const Color(0x80cdd6f4),
    primaryTextTheme: Typography.material2021(platform: TargetPlatform.android).white,
    textTheme: Typography.material2021(platform: TargetPlatform.android).white,
    primaryIconTheme: const IconThemeData(color: Color(0xffcdd6f4)),
    iconTheme: base.iconTheme.copyWith(
      color: const Color(0xffcdd6f4),
    ),
    dividerTheme: base.dividerTheme.copyWith(
      color: const Color(0xffcba6f7),
    ),
    sliderTheme: const SliderThemeData().copyWith(
      valueIndicatorColor: const Color(0xffa6e3a1),
      trackHeight: 2.0,
      thumbShape: const RoundSliderThumbShape(
        enabledThumbRadius: 6.0,
        disabledThumbRadius: 6.0,
      ),
    ),
    appBarTheme: base.appBarTheme.copyWith(
      backgroundColor: const Color(0xff11111b),
      foregroundColor: const Color(0xffcdd6f4),
      shadowColor: const Color(0xff11111b),
      elevation: 1.0,
      systemOverlayStyle: SystemUiOverlayStyle.light.copyWith(
        systemNavigationBarIconBrightness: Brightness.light,
        systemNavigationBarColor: const Color(0xff11111b),
        statusBarIconBrightness: Brightness.light,
      ),
    ),
    snackBarTheme: base.snackBarTheme.copyWith(
      actionTextColor: const Color(0xffcba6f7),
    ),
    outlinedButtonTheme: OutlinedButtonThemeData(
      style: OutlinedButton.styleFrom(
        foregroundColor: const Color(0xffcdd6f4),
        side: const BorderSide(
          color: Color(0xffcba6f7),
          style: BorderStyle.solid,
        ),
      ),
    ),
    elevatedButtonTheme: ElevatedButtonThemeData(
      style: ElevatedButton.styleFrom(
        backgroundColor: const Color(0xffcba6f7),
        foregroundColor: const Color(0xff000000),
      ),
    ),
  );
}

class Themes {
  final ThemeData themeData;

  Themes({required this.themeData});

  factory Themes.lightTheme() {
    return Themes(themeData: _lightTheme);
  }

  factory Themes.darkTheme() {
    return Themes(themeData: _darkTheme);
  }

  factory Themes.fromKey(String key) {
    return Themes(themeData: ThemeRegistry.getThemeData(key));
  }
}

ThemeData _buildAbyssTheme() {
  final base = ThemeData.dark(useMaterial3: false);
  
  return base.copyWith(
    colorScheme: const ColorScheme.dark(
      primary: Color(0xff326fef),
      secondary: Color(0xffc8aa7d),
      surface: Color(0xff061940),
      error: Color(0xffbf616a),
      onSurface: Color(0xfff6f5f4),
      background: Color(0xff000C18),
    ),
    bottomAppBarTheme: const BottomAppBarThemeData().copyWith(
      color: const Color(0xff051336),
    ),
    cardTheme: const CardThemeData().copyWith(
      color: const Color(0xff061940),
      shadowColor: const Color(0xff303648),
    ),
    brightness: Brightness.dark,
    primaryColor: const Color(0xff326fef),
    canvasColor: const Color(0xff000C18),
    scaffoldBackgroundColor: const Color(0xff000C18),
    cardColor: const Color(0xff061940),
    dividerColor: const Color(0xff838385),
    highlightColor: const Color(0xff152037),
    splashColor: const Color(0xff152037),
    unselectedWidgetColor: const Color(0xfff6f5f4),
    disabledColor: const Color(0x77838385),
    secondaryHeaderColor: const Color(0xff051336),
    dialogBackgroundColor: const Color(0xff051336),
    indicatorColor: const Color(0xff326fef),
    hintColor: const Color(0x80f6f5f4),
    primaryTextTheme: Typography.material2021(platform: TargetPlatform.android).white,
    textTheme: Typography.material2021(platform: TargetPlatform.android).white,
    primaryIconTheme: const IconThemeData(color: Color(0xfff6f5f4)),
    iconTheme: base.iconTheme.copyWith(
      color: const Color(0xfff6f5f4),
    ),
    dividerTheme: base.dividerTheme.copyWith(
      color: const Color(0xff838385),
    ),
    sliderTheme: const SliderThemeData().copyWith(
      valueIndicatorColor: const Color(0xff326fef),
      trackHeight: 2.0,
      thumbShape: const RoundSliderThumbShape(
        enabledThumbRadius: 6.0,
        disabledThumbRadius: 6.0,
      ),
    ),
    appBarTheme: base.appBarTheme.copyWith(
      backgroundColor: const Color(0xff051336),
      foregroundColor: const Color(0xfff6f5f4),
      shadowColor: const Color(0xff051336),
      elevation: 1.0,
      systemOverlayStyle: SystemUiOverlayStyle.light.copyWith(
        systemNavigationBarIconBrightness: Brightness.light,
        systemNavigationBarColor: const Color(0xff051336),
        statusBarIconBrightness: Brightness.light,
      ),
    ),
    snackBarTheme: base.snackBarTheme.copyWith(
      actionTextColor: const Color(0xff326fef),
    ),
    outlinedButtonTheme: OutlinedButtonThemeData(
      style: OutlinedButton.styleFrom(
        foregroundColor: const Color(0xfff6f5f4),
        side: const BorderSide(
          color: Color(0xff326fef),
          style: BorderStyle.solid,
        ),
      ),
    ),
    elevatedButtonTheme: ElevatedButtonThemeData(
      style: ElevatedButton.styleFrom(
        backgroundColor: const Color(0xff326fef),
        foregroundColor: const Color(0xffffffff),
      ),
    ),
  );
}

ThemeData _buildCyberSynthwaveTheme() {
  final base = ThemeData.dark(useMaterial3: false);
  
  return base.copyWith(
    colorScheme: const ColorScheme.dark(
      primary: Color(0xfff92aad),
      secondary: Color(0xffff71ce),
      surface: Color(0xff2a1f3a),
      error: Color(0xffff2e63),
      onSurface: Color(0xffeee6ff),
      background: Color(0xff1a1721),
    ),
    bottomAppBarTheme: const BottomAppBarThemeData().copyWith(
      color: const Color(0xff2a1f3a),
    ),
    cardTheme: const CardThemeData().copyWith(
      color: const Color(0xff2a1f3a),
      shadowColor: const Color(0xff2a1f3a),
    ),
    brightness: Brightness.dark,
    primaryColor: const Color(0xfff92aad),
    canvasColor: const Color(0xff1a1721),
    scaffoldBackgroundColor: const Color(0xff1a1721),
    cardColor: const Color(0xff2a1f3a),
    dividerColor: const Color(0xffc3b7d9),
    highlightColor: const Color(0xffb31777),
    splashColor: const Color(0xffb31777),
    unselectedWidgetColor: const Color(0xffeee6ff),
    disabledColor: const Color(0x77c3b7d9),
    secondaryHeaderColor: const Color(0xff2a1f3a),
    dialogBackgroundColor: const Color(0xff2a1f3a),
    indicatorColor: const Color(0xfff92aad),
    hintColor: const Color(0x80eee6ff),
    primaryTextTheme: Typography.material2021(platform: TargetPlatform.android).white,
    textTheme: Typography.material2021(platform: TargetPlatform.android).white,
    primaryIconTheme: const IconThemeData(color: Color(0xffeee6ff)),
    iconTheme: base.iconTheme.copyWith(
      color: const Color(0xffeee6ff),
    ),
    dividerTheme: base.dividerTheme.copyWith(
      color: const Color(0xffc3b7d9),
    ),
    sliderTheme: const SliderThemeData().copyWith(
      valueIndicatorColor: const Color(0xfff92aad),
      trackHeight: 2.0,
      thumbShape: const RoundSliderThumbShape(
        enabledThumbRadius: 6.0,
        disabledThumbRadius: 6.0,
      ),
    ),
    appBarTheme: base.appBarTheme.copyWith(
      backgroundColor: const Color(0xff2a1f3a),
      foregroundColor: const Color(0xffeee6ff),
      shadowColor: const Color(0xff2a1f3a),
      elevation: 1.0,
      systemOverlayStyle: SystemUiOverlayStyle.light.copyWith(
        systemNavigationBarIconBrightness: Brightness.light,
        systemNavigationBarColor: const Color(0xff2a1f3a),
        statusBarIconBrightness: Brightness.light,
      ),
    ),
    snackBarTheme: base.snackBarTheme.copyWith(
      actionTextColor: const Color(0xfff92aad),
    ),
    outlinedButtonTheme: OutlinedButtonThemeData(
      style: OutlinedButton.styleFrom(
        foregroundColor: const Color(0xffeee6ff),
        side: const BorderSide(
          color: Color(0xfff92aad),
          style: BorderStyle.solid,
        ),
      ),
    ),
    elevatedButtonTheme: ElevatedButtonThemeData(
      style: ElevatedButton.styleFrom(
        backgroundColor: const Color(0xfff92aad),
        foregroundColor: const Color(0xff000000),
      ),
    ),
  );
}

ThemeData _buildMidnightOceanTheme() {
  final base = ThemeData.dark(useMaterial3: false);
  
  return base.copyWith(
    colorScheme: const ColorScheme.dark(
      primary: Color(0xff38bdf8),
      secondary: Color(0xff60a5fa),
      surface: Color(0xff1e293b),
      error: Color(0xffef4444),
      onSurface: Color(0xffe2e8f0),
      background: Color(0xff0f172a),
    ),
    bottomAppBarTheme: const BottomAppBarThemeData().copyWith(
      color: const Color(0xff1e293b),
    ),
    cardTheme: const CardThemeData().copyWith(
      color: const Color(0xff1e293b),
      shadowColor: const Color(0xff1e293b),
    ),
    brightness: Brightness.dark,
    primaryColor: const Color(0xff38bdf8),
    canvasColor: const Color(0xff0f172a),
    scaffoldBackgroundColor: const Color(0xff0f172a),
    cardColor: const Color(0xff1e293b),
    dividerColor: const Color(0xff1e293b),
    highlightColor: const Color(0xff0ea5e9),
    splashColor: const Color(0xff0ea5e9),
    unselectedWidgetColor: const Color(0xffe2e8f0),
    disabledColor: const Color(0x7794a3b8),
    secondaryHeaderColor: const Color(0xff1e293b),
    dialogBackgroundColor: const Color(0xff1e293b),
    indicatorColor: const Color(0xff0ea5e9),
    hintColor: const Color(0x80e2e8f0),
    primaryTextTheme: Typography.material2021(platform: TargetPlatform.android).white,
    textTheme: Typography.material2021(platform: TargetPlatform.android).white,
    primaryIconTheme: const IconThemeData(color: Color(0xffe2e8f0)),
    iconTheme: base.iconTheme.copyWith(
      color: const Color(0xffe2e8f0),
    ),
    dividerTheme: base.dividerTheme.copyWith(
      color: const Color(0xff1e293b),
    ),
    sliderTheme: const SliderThemeData().copyWith(
      valueIndicatorColor: const Color(0xff38bdf8),
      trackHeight: 2.0,
      thumbShape: const RoundSliderThumbShape(
        enabledThumbRadius: 6.0,
        disabledThumbRadius: 6.0,
      ),
    ),
    appBarTheme: base.appBarTheme.copyWith(
      backgroundColor: const Color(0xff1e293b),
      foregroundColor: const Color(0xffe2e8f0),
      shadowColor: const Color(0xff1e293b),
      elevation: 1.0,
      systemOverlayStyle: SystemUiOverlayStyle.light.copyWith(
        systemNavigationBarIconBrightness: Brightness.light,
        systemNavigationBarColor: const Color(0xff1e293b),
        statusBarIconBrightness: Brightness.light,
      ),
    ),
    snackBarTheme: base.snackBarTheme.copyWith(
      actionTextColor: const Color(0xff38bdf8),
    ),
    outlinedButtonTheme: OutlinedButtonThemeData(
      style: OutlinedButton.styleFrom(
        foregroundColor: const Color(0xffe2e8f0),
        side: const BorderSide(
          color: Color(0xff38bdf8),
          style: BorderStyle.solid,
        ),
      ),
    ),
    elevatedButtonTheme: ElevatedButtonThemeData(
      style: ElevatedButton.styleFrom(
        backgroundColor: const Color(0xff38bdf8),
        foregroundColor: const Color(0xff000000),
      ),
    ),
  );
}

ThemeData _buildForestDepthsTheme() {
  final base = ThemeData.dark(useMaterial3: false);
  
  return base.copyWith(
    colorScheme: const ColorScheme.dark(
      primary: Color(0xff7fb685),
      secondary: Color(0xffa1d0a5),
      surface: Color(0xff2d4a33),
      error: Color(0xffe67c73),
      onSurface: Color(0xffc9e4ca),
      background: Color(0xff1a2f1f),
    ),
    bottomAppBarTheme: const BottomAppBarThemeData().copyWith(
      color: const Color(0xff2d4a33),
    ),
    cardTheme: const CardThemeData().copyWith(
      color: const Color(0xff2d4a33),
      shadowColor: const Color(0xff2d4a33),
    ),
    brightness: Brightness.dark,
    primaryColor: const Color(0xff7fb685),
    canvasColor: const Color(0xff1a2f1f),
    scaffoldBackgroundColor: const Color(0xff1a2f1f),
    cardColor: const Color(0xff2d4a33),
    dividerColor: const Color(0xff2d4a33),
    highlightColor: const Color(0xff5c8b61),
    splashColor: const Color(0xff5c8b61),
    unselectedWidgetColor: const Color(0xffc9e4ca),
    disabledColor: const Color(0x778fbb91),
    secondaryHeaderColor: const Color(0xff2d4a33),
    dialogBackgroundColor: const Color(0xff2d4a33),
    indicatorColor: const Color(0xff5c8b61),
    hintColor: const Color(0x80c9e4ca),
    primaryTextTheme: Typography.material2021(platform: TargetPlatform.android).white,
    textTheme: Typography.material2021(platform: TargetPlatform.android).white,
    primaryIconTheme: const IconThemeData(color: Color(0xffc9e4ca)),
    iconTheme: base.iconTheme.copyWith(
      color: const Color(0xffc9e4ca),
    ),
    dividerTheme: base.dividerTheme.copyWith(
      color: const Color(0xff2d4a33),
    ),
    sliderTheme: const SliderThemeData().copyWith(
      valueIndicatorColor: const Color(0xff7fb685),
      trackHeight: 2.0,
      thumbShape: const RoundSliderThumbShape(
        enabledThumbRadius: 6.0,
        disabledThumbRadius: 6.0,
      ),
    ),
    appBarTheme: base.appBarTheme.copyWith(
      backgroundColor: const Color(0xff2d4a33),
      foregroundColor: const Color(0xffc9e4ca),
      shadowColor: const Color(0xff2d4a33),
      elevation: 1.0,
      systemOverlayStyle: SystemUiOverlayStyle.light.copyWith(
        systemNavigationBarIconBrightness: Brightness.light,
        systemNavigationBarColor: const Color(0xff2d4a33),
        statusBarIconBrightness: Brightness.light,
      ),
    ),
    snackBarTheme: base.snackBarTheme.copyWith(
      actionTextColor: const Color(0xff7fb685),
    ),
    outlinedButtonTheme: OutlinedButtonThemeData(
      style: OutlinedButton.styleFrom(
        foregroundColor: const Color(0xffc9e4ca),
        side: const BorderSide(
          color: Color(0xff7fb685),
          style: BorderStyle.solid,
        ),
      ),
    ),
    elevatedButtonTheme: ElevatedButtonThemeData(
      style: ElevatedButton.styleFrom(
        backgroundColor: const Color(0xff7fb685),
        foregroundColor: const Color(0xff000000),
      ),
    ),
  );
}

ThemeData _buildSunsetHorizonTheme() {
  final base = ThemeData.dark(useMaterial3: false);
  
  return base.copyWith(
    colorScheme: const ColorScheme.dark(
      primary: Color(0xffff9e64),
      secondary: Color(0xffffb088),
      surface: Color(0xff432e44),
      error: Color(0xffff6b6b),
      onSurface: Color(0xffffd9c0),
      background: Color(0xff2b1c2c),
    ),
    bottomAppBarTheme: const BottomAppBarThemeData().copyWith(
      color: const Color(0xff432e44),
    ),
    cardTheme: const CardThemeData().copyWith(
      color: const Color(0xff432e44),
      shadowColor: const Color(0xff432e44),
    ),
    brightness: Brightness.dark,
    primaryColor: const Color(0xffff9e64),
    canvasColor: const Color(0xff2b1c2c),
    scaffoldBackgroundColor: const Color(0xff2b1c2c),
    cardColor: const Color(0xff432e44),
    dividerColor: const Color(0xff432e44),
    highlightColor: const Color(0xffe8875c),
    splashColor: const Color(0xffe8875c),
    unselectedWidgetColor: const Color(0xffffd9c0),
    disabledColor: const Color(0x77d4a5a5),
    secondaryHeaderColor: const Color(0xff432e44),
    dialogBackgroundColor: const Color(0xff432e44),
    indicatorColor: const Color(0xffe8875c),
    hintColor: const Color(0x80ffd9c0),
    primaryTextTheme: Typography.material2021(platform: TargetPlatform.android).white,
    textTheme: Typography.material2021(platform: TargetPlatform.android).white,
    primaryIconTheme: const IconThemeData(color: Color(0xffffd9c0)),
    iconTheme: base.iconTheme.copyWith(
      color: const Color(0xffffd9c0),
    ),
    dividerTheme: base.dividerTheme.copyWith(
      color: const Color(0xff432e44),
    ),
    sliderTheme: const SliderThemeData().copyWith(
      valueIndicatorColor: const Color(0xffff9e64),
      trackHeight: 2.0,
      thumbShape: const RoundSliderThumbShape(
        enabledThumbRadius: 6.0,
        disabledThumbRadius: 6.0,
      ),
    ),
    appBarTheme: base.appBarTheme.copyWith(
      backgroundColor: const Color(0xff432e44),
      foregroundColor: const Color(0xffffd9c0),
      shadowColor: const Color(0xff432e44),
      elevation: 1.0,
      systemOverlayStyle: SystemUiOverlayStyle.light.copyWith(
        systemNavigationBarIconBrightness: Brightness.light,
        systemNavigationBarColor: const Color(0xff432e44),
        statusBarIconBrightness: Brightness.light,
      ),
    ),
    snackBarTheme: base.snackBarTheme.copyWith(
      actionTextColor: const Color(0xffff9e64),
    ),
    outlinedButtonTheme: OutlinedButtonThemeData(
      style: OutlinedButton.styleFrom(
        foregroundColor: const Color(0xffffd9c0),
        side: const BorderSide(
          color: Color(0xffff9e64),
          style: BorderStyle.solid,
        ),
      ),
    ),
    elevatedButtonTheme: ElevatedButtonThemeData(
      style: ElevatedButton.styleFrom(
        backgroundColor: const Color(0xffff9e64),
        foregroundColor: const Color(0xff000000),
      ),
    ),
  );
}

ThemeData _buildArcticFrostTheme() {
  final base = ThemeData.dark(useMaterial3: false);
  
  return base.copyWith(
    colorScheme: const ColorScheme.dark(
      primary: Color(0xff88c0d0),
      secondary: Color(0xff81a1c1),
      surface: Color(0xff2a2f36),
      error: Color(0xffbf616a),
      onSurface: Color(0xffeceff4),
      background: Color(0xff1a1d21),
    ),
    bottomAppBarTheme: const BottomAppBarThemeData().copyWith(
      color: const Color(0xff2a2f36),
    ),
    cardTheme: const CardThemeData().copyWith(
      color: const Color(0xff2a2f36),
      shadowColor: const Color(0xff2a2f36),
    ),
    brightness: Brightness.dark,
    primaryColor: const Color(0xff88c0d0),
    canvasColor: const Color(0xff1a1d21),
    scaffoldBackgroundColor: const Color(0xff1a1d21),
    cardColor: const Color(0xff2a2f36),
    dividerColor: const Color(0xff2a2f36),
    highlightColor: const Color(0xff5e81ac),
    splashColor: const Color(0xff5e81ac),
    unselectedWidgetColor: const Color(0xffeceff4),
    disabledColor: const Color(0x77aeb3bb),
    secondaryHeaderColor: const Color(0xff2a2f36),
    dialogBackgroundColor: const Color(0xff2a2f36),
    indicatorColor: const Color(0xff5e81ac),
    hintColor: const Color(0x80eceff4),
    primaryTextTheme: Typography.material2021(platform: TargetPlatform.android).white,
    textTheme: Typography.material2021(platform: TargetPlatform.android).white,
    primaryIconTheme: const IconThemeData(color: Color(0xffeceff4)),
    iconTheme: base.iconTheme.copyWith(
      color: const Color(0xffeceff4),
    ),
    dividerTheme: base.dividerTheme.copyWith(
      color: const Color(0xff2a2f36),
    ),
    sliderTheme: const SliderThemeData().copyWith(
      valueIndicatorColor: const Color(0xff88c0d0),
      trackHeight: 2.0,
      thumbShape: const RoundSliderThumbShape(
        enabledThumbRadius: 6.0,
        disabledThumbRadius: 6.0,
      ),
    ),
    appBarTheme: base.appBarTheme.copyWith(
      backgroundColor: const Color(0xff2a2f36),
      foregroundColor: const Color(0xffeceff4),
      shadowColor: const Color(0xff2a2f36),
      elevation: 1.0,
      systemOverlayStyle: SystemUiOverlayStyle.light.copyWith(
        systemNavigationBarIconBrightness: Brightness.light,
        systemNavigationBarColor: const Color(0xff2a2f36),
        statusBarIconBrightness: Brightness.light,
      ),
    ),
    snackBarTheme: base.snackBarTheme.copyWith(
      actionTextColor: const Color(0xff88c0d0),
    ),
    outlinedButtonTheme: OutlinedButtonThemeData(
      style: OutlinedButton.styleFrom(
        foregroundColor: const Color(0xffeceff4),
        side: const BorderSide(
          color: Color(0xff88c0d0),
          style: BorderStyle.solid,
        ),
      ),
    ),
    elevatedButtonTheme: ElevatedButtonThemeData(
      style: ElevatedButton.styleFrom(
        backgroundColor: const Color(0xff88c0d0),
        foregroundColor: const Color(0xff000000),
      ),
    ),
  );
}

ThemeData _buildNeonTheme() {
  final base = ThemeData.dark(useMaterial3: false);
  
  return base.copyWith(
    colorScheme: const ColorScheme.dark(
      primary: Color(0xfff75c1d),
      secondary: Color(0xff7000ff),
      surface: Color(0xff1a171e),
      error: Color(0xffff5555),
      onSurface: Color(0xff9F9DA1),
      background: Color(0xff120e16),
    ),
    bottomAppBarTheme: const BottomAppBarThemeData().copyWith(
      color: const Color(0xff120e16),
    ),
    cardTheme: const CardThemeData().copyWith(
      color: const Color(0xff1a171e),
      shadowColor: const Color(0xff303648),
    ),
    brightness: Brightness.dark,
    primaryColor: const Color(0xfff75c1d),
    canvasColor: const Color(0xff120e16),
    scaffoldBackgroundColor: const Color(0xff120e16),
    cardColor: const Color(0xff1a171e),
    dividerColor: const Color(0xff4a535e),
    highlightColor: const Color(0xff7000ff),
    splashColor: const Color(0xff7000ff),
    unselectedWidgetColor: const Color(0xff9F9DA1),
    disabledColor: const Color(0x774a535e),
    secondaryHeaderColor: const Color(0xff120e16),
    dialogBackgroundColor: const Color(0xff120e16),
    indicatorColor: const Color(0xfff75c1d),
    hintColor: const Color(0x809F9DA1),
    primaryTextTheme: Typography.material2021(platform: TargetPlatform.android).white,
    textTheme: Typography.material2021(platform: TargetPlatform.android).white,
    primaryIconTheme: const IconThemeData(color: Color(0xff9F9DA1)),
    iconTheme: base.iconTheme.copyWith(
      color: const Color(0xff9F9DA1),
    ),
    dividerTheme: base.dividerTheme.copyWith(
      color: const Color(0xff4a535e),
    ),
    sliderTheme: const SliderThemeData().copyWith(
      valueIndicatorColor: const Color(0xfff75c1d),
      trackHeight: 2.0,
      thumbShape: const RoundSliderThumbShape(
        enabledThumbRadius: 6.0,
        disabledThumbRadius: 6.0,
      ),
    ),
    appBarTheme: base.appBarTheme.copyWith(
      backgroundColor: const Color(0xff120e16),
      foregroundColor: const Color(0xff9F9DA1),
      shadowColor: const Color(0xff120e16),
      elevation: 1.0,
      systemOverlayStyle: SystemUiOverlayStyle.light.copyWith(
        systemNavigationBarIconBrightness: Brightness.light,
        systemNavigationBarColor: const Color(0xff120e16),
        statusBarIconBrightness: Brightness.light,
      ),
    ),
    snackBarTheme: base.snackBarTheme.copyWith(
      actionTextColor: const Color(0xfff75c1d),
    ),
    outlinedButtonTheme: OutlinedButtonThemeData(
      style: OutlinedButton.styleFrom(
        foregroundColor: const Color(0xff9F9DA1),
        side: const BorderSide(
          color: Color(0xfff75c1d),
          style: BorderStyle.solid,
        ),
      ),
    ),
    elevatedButtonTheme: ElevatedButtonThemeData(
      style: ElevatedButton.styleFrom(
        backgroundColor: const Color(0xfff75c1d),
        foregroundColor: const Color(0xff000000),
      ),
    ),
  );
}

ThemeData _buildKimbieTheme() {
  final base = ThemeData.dark(useMaterial3: false);
  
  return base.copyWith(
    colorScheme: const ColorScheme.dark(
      primary: Color(0xffca9858),
      secondary: Color(0xfff6f5f4),
      surface: Color(0xff362712),
      error: Color(0xffff5555),
      onSurface: Color(0xffB1AD86),
      background: Color(0xff221a0f),
    ),
    bottomAppBarTheme: const BottomAppBarThemeData().copyWith(
      color: const Color(0xff131510),
    ),
    cardTheme: const CardThemeData().copyWith(
      color: const Color(0xff362712),
      shadowColor: const Color(0xff65533c),
    ),
    brightness: Brightness.dark,
    primaryColor: const Color(0xffca9858),
    canvasColor: const Color(0xff221a0f),
    scaffoldBackgroundColor: const Color(0xff221a0f),
    cardColor: const Color(0xff362712),
    dividerColor: const Color(0xff4a535e),
    highlightColor: const Color(0xffd3af86),
    splashColor: const Color(0xffd3af86),
    unselectedWidgetColor: const Color(0xffB1AD86),
    disabledColor: const Color(0x774a535e),
    secondaryHeaderColor: const Color(0xff131510),
    dialogBackgroundColor: const Color(0xff131510),
    indicatorColor: const Color(0xffca9858),
    hintColor: const Color(0x80B1AD86),
    primaryTextTheme: Typography.material2021(platform: TargetPlatform.android).white,
    textTheme: Typography.material2021(platform: TargetPlatform.android).white,
    primaryIconTheme: const IconThemeData(color: Color(0xffB1AD86)),
    iconTheme: base.iconTheme.copyWith(
      color: const Color(0xffB1AD86),
    ),
    dividerTheme: base.dividerTheme.copyWith(
      color: const Color(0xff4a535e),
    ),
    sliderTheme: const SliderThemeData().copyWith(
      valueIndicatorColor: const Color(0xffca9858),
      trackHeight: 2.0,
      thumbShape: const RoundSliderThumbShape(
        enabledThumbRadius: 6.0,
        disabledThumbRadius: 6.0,
      ),
    ),
    appBarTheme: base.appBarTheme.copyWith(
      backgroundColor: const Color(0xff131510),
      foregroundColor: const Color(0xffB1AD86),
      shadowColor: const Color(0xff131510),
      elevation: 1.0,
      systemOverlayStyle: SystemUiOverlayStyle.light.copyWith(
        systemNavigationBarIconBrightness: Brightness.light,
        systemNavigationBarColor: const Color(0xff131510),
        statusBarIconBrightness: Brightness.light,
      ),
    ),
    snackBarTheme: base.snackBarTheme.copyWith(
      actionTextColor: const Color(0xffca9858),
    ),
    outlinedButtonTheme: OutlinedButtonThemeData(
      style: OutlinedButton.styleFrom(
        foregroundColor: const Color(0xffB1AD86),
        side: const BorderSide(
          color: Color(0xffca9858),
          style: BorderStyle.solid,
        ),
      ),
    ),
    elevatedButtonTheme: ElevatedButtonThemeData(
      style: ElevatedButton.styleFrom(
        backgroundColor: const Color(0xffca9858),
        foregroundColor: const Color(0xff000000),
      ),
    ),
  );
}

ThemeData _buildGruvboxLightTheme() {
  final base = ThemeData.light(useMaterial3: false);
  
  return base.copyWith(
    colorScheme: const ColorScheme.light(
      primary: Color(0xffd1ac0e),
      secondary: Color(0xffa68738),
      surface: Color(0xfffbf1c7),
      error: Color(0xffcc241d),
      onSurface: Color(0xff5f5750),
      background: Color(0xfff9f5d7),
    ),
    bottomAppBarTheme: const BottomAppBarThemeData().copyWith(
      color: const Color(0xfffbf1c7),
    ),
    cardTheme: const CardThemeData().copyWith(
      color: const Color(0xfffbf1c7),
      shadowColor: const Color(0xffaca289),
    ),
    brightness: Brightness.light,
    primaryColor: const Color(0xffd1ac0e),
    canvasColor: const Color(0xfff9f5d7),
    scaffoldBackgroundColor: const Color(0xfff9f5d7),
    cardColor: const Color(0xfffbf1c7),
    dividerColor: const Color(0xffe0dbb2),
    highlightColor: const Color(0xffcfd2a8),
    splashColor: const Color(0xffcfd2a8),
    unselectedWidgetColor: const Color(0xff5f5750),
    disabledColor: const Color(0x77aca289),
    secondaryHeaderColor: const Color(0xfffbf1c7),
    dialogBackgroundColor: const Color(0xfffbf1c7),
    indicatorColor: const Color(0xffd1ac0e),
    hintColor: const Color(0x805f5750),
    primaryTextTheme: Typography.material2021(platform: TargetPlatform.android).black,
    textTheme: Typography.material2021(platform: TargetPlatform.android).black,
    primaryIconTheme: const IconThemeData(color: Color(0xff5f5750)),
    iconTheme: base.iconTheme.copyWith(
      color: const Color(0xff5f5750),
    ),
    dividerTheme: base.dividerTheme.copyWith(
      color: const Color(0xffe0dbb2),
    ),
    sliderTheme: const SliderThemeData().copyWith(
      valueIndicatorColor: const Color(0xffd1ac0e),
      trackHeight: 2.0,
      thumbShape: const RoundSliderThumbShape(
        enabledThumbRadius: 6.0,
        disabledThumbRadius: 6.0,
      ),
    ),
    appBarTheme: base.appBarTheme.copyWith(
      backgroundColor: const Color(0xfffbf1c7),
      foregroundColor: const Color(0xff5f5750),
      shadowColor: const Color(0xfffbf1c7),
      elevation: 1.0,
      systemOverlayStyle: SystemUiOverlayStyle.dark.copyWith(
        systemNavigationBarIconBrightness: Brightness.dark,
        systemNavigationBarColor: const Color(0xfffbf1c7),
        statusBarIconBrightness: Brightness.dark,
      ),
    ),
    snackBarTheme: base.snackBarTheme.copyWith(
      actionTextColor: const Color(0xffd1ac0e),
    ),
    outlinedButtonTheme: OutlinedButtonThemeData(
      style: OutlinedButton.styleFrom(
        foregroundColor: const Color(0xff5f5750),
        side: const BorderSide(
          color: Color(0xffd1ac0e),
          style: BorderStyle.solid,
        ),
      ),
    ),
    elevatedButtonTheme: ElevatedButtonThemeData(
      style: ElevatedButton.styleFrom(
        backgroundColor: const Color(0xffd1ac0e),
        foregroundColor: const Color(0xff000000),
      ),
    ),
  );
}

ThemeData _buildGreenMeanieTheme() {
  final base = ThemeData.dark(useMaterial3: false);
  
  return base.copyWith(
    colorScheme: const ColorScheme.dark(
      primary: Color(0xff224e44),
      secondary: Color(0xff6590fd),
      surface: Color(0xff292A2E),
      error: Color(0xffff5555),
      onSurface: Color(0xff489D50),
      background: Color(0xff142e28),
    ),
    bottomAppBarTheme: const BottomAppBarThemeData().copyWith(
      color: const Color(0xff292A2E),
    ),
    cardTheme: const CardThemeData().copyWith(
      color: const Color(0xff292A2E),
      shadowColor: const Color(0xff489D50),
    ),
    brightness: Brightness.dark,
    primaryColor: const Color(0xff224e44),
    canvasColor: const Color(0xff142e28),
    scaffoldBackgroundColor: const Color(0xff142e28),
    cardColor: const Color(0xff292A2E),
    dividerColor: const Color(0xff446448),
    highlightColor: const Color(0xff4b5563),
    splashColor: const Color(0xff4b5563),
    unselectedWidgetColor: const Color(0xff489D50),
    disabledColor: const Color(0x77446448),
    secondaryHeaderColor: const Color(0xff292A2E),
    dialogBackgroundColor: const Color(0xff292A2E),
    indicatorColor: const Color(0xff224e44),
    hintColor: const Color(0x80489D50),
    primaryTextTheme: Typography.material2021(platform: TargetPlatform.android).white,
    textTheme: Typography.material2021(platform: TargetPlatform.android).white,
    primaryIconTheme: const IconThemeData(color: Color(0xff489D50)),
    iconTheme: base.iconTheme.copyWith(
      color: const Color(0xff489D50),
    ),
    dividerTheme: base.dividerTheme.copyWith(
      color: const Color(0xff446448),
    ),
    sliderTheme: const SliderThemeData().copyWith(
      valueIndicatorColor: const Color(0xff224e44),
      trackHeight: 2.0,
      thumbShape: const RoundSliderThumbShape(
        enabledThumbRadius: 6.0,
        disabledThumbRadius: 6.0,
      ),
    ),
    appBarTheme: base.appBarTheme.copyWith(
      backgroundColor: const Color(0xff292A2E),
      foregroundColor: const Color(0xff489D50),
      shadowColor: const Color(0xff292A2E),
      elevation: 1.0,
      systemOverlayStyle: SystemUiOverlayStyle.light.copyWith(
        systemNavigationBarIconBrightness: Brightness.light,
        systemNavigationBarColor: const Color(0xff292A2E),
        statusBarIconBrightness: Brightness.light,
      ),
    ),
    snackBarTheme: base.snackBarTheme.copyWith(
      actionTextColor: const Color(0xff224e44),
    ),
    outlinedButtonTheme: OutlinedButtonThemeData(
      style: OutlinedButton.styleFrom(
        foregroundColor: const Color(0xff489D50),
        side: const BorderSide(
          color: Color(0xff224e44),
          style: BorderStyle.solid,
        ),
      ),
    ),
    elevatedButtonTheme: ElevatedButtonThemeData(
      style: ElevatedButton.styleFrom(
        backgroundColor: const Color(0xff489D50),
        foregroundColor: const Color(0xff000000),
      ),
    ),
  );
}

ThemeData _buildWildberriesTheme() {
  final base = ThemeData.dark(useMaterial3: false);
  
  return base.copyWith(
    colorScheme: const ColorScheme.dark(
      primary: Color(0xff4b246b),
      secondary: Color(0xff5196B2),
      surface: Color(0xff19002E),
      error: Color(0xffff5555),
      onSurface: Color(0xffCF8B3E),
      background: Color(0xff240041),
    ),
    bottomAppBarTheme: const BottomAppBarThemeData().copyWith(
      color: const Color(0xff19002E),
    ),
    cardTheme: const CardThemeData().copyWith(
      color: const Color(0xff19002E),
      shadowColor: const Color(0xff3a264a),
    ),
    brightness: Brightness.dark,
    primaryColor: const Color(0xff4b246b),
    canvasColor: const Color(0xff240041),
    scaffoldBackgroundColor: const Color(0xff240041),
    cardColor: const Color(0xff19002E),
    dividerColor: const Color(0xffC79BFF),
    highlightColor: const Color(0xff44433A),
    splashColor: const Color(0xff44433A),
    unselectedWidgetColor: const Color(0xffCF8B3E),
    disabledColor: const Color(0x77C79BFF),
    secondaryHeaderColor: const Color(0xff19002E),
    dialogBackgroundColor: const Color(0xff19002E),
    indicatorColor: const Color(0xff4b246b),
    hintColor: const Color(0x80CF8B3E),
    primaryTextTheme: Typography.material2021(platform: TargetPlatform.android).white,
    textTheme: Typography.material2021(platform: TargetPlatform.android).white,
    primaryIconTheme: const IconThemeData(color: Color(0xffCF8B3E)),
    iconTheme: base.iconTheme.copyWith(
      color: const Color(0xffCF8B3E),
    ),
    dividerTheme: base.dividerTheme.copyWith(
      color: const Color(0xffC79BFF),
    ),
    sliderTheme: const SliderThemeData().copyWith(
      valueIndicatorColor: const Color(0xff4b246b),
      trackHeight: 2.0,
      thumbShape: const RoundSliderThumbShape(
        enabledThumbRadius: 6.0,
        disabledThumbRadius: 6.0,
      ),
    ),
    appBarTheme: base.appBarTheme.copyWith(
      backgroundColor: const Color(0xff19002E),
      foregroundColor: const Color(0xffCF8B3E),
      shadowColor: const Color(0xff19002E),
      elevation: 1.0,
      systemOverlayStyle: SystemUiOverlayStyle.light.copyWith(
        systemNavigationBarIconBrightness: Brightness.light,
        systemNavigationBarColor: const Color(0xff19002E),
        statusBarIconBrightness: Brightness.light,
      ),
    ),
    snackBarTheme: base.snackBarTheme.copyWith(
      actionTextColor: const Color(0xff4b246b),
    ),
    outlinedButtonTheme: OutlinedButtonThemeData(
      style: OutlinedButton.styleFrom(
        foregroundColor: const Color(0xffCF8B3E),
        side: const BorderSide(
          color: Color(0xff4b246b),
          style: BorderStyle.solid,
        ),
      ),
    ),
    elevatedButtonTheme: ElevatedButtonThemeData(
      style: ElevatedButton.styleFrom(
        backgroundColor: const Color(0xff4b246b),
        foregroundColor: const Color(0xffCF8B3E),
      ),
    ),
  );
}

ThemeData _buildSoftLavenderTheme() {
  final base = ThemeData.light(useMaterial3: false);
  
  return base.copyWith(
    colorScheme: const ColorScheme.light(
      primary: Color(0xff9b7cb6),
      secondary: Color(0xffc8a8d8),
      surface: Color(0xfff8f5ff),
      error: Color(0xffb91c1c),
      onSurface: Color(0xff3e2851),
      background: Color(0xfff5f2ff),
    ),
    bottomAppBarTheme: const BottomAppBarThemeData().copyWith(
      color: const Color(0xfff8f5ff),
    ),
    cardTheme: const CardThemeData().copyWith(
      color: const Color(0xfff8f5ff),
      shadowColor: const Color(0xffc8a8d8),
    ),
    brightness: Brightness.light,
    primaryColor: const Color(0xff9b7cb6),
    canvasColor: const Color(0xfff5f2ff),
    scaffoldBackgroundColor: const Color(0xfff5f2ff),
    cardColor: const Color(0xfff8f5ff),
    dividerColor: const Color(0xffc8a8d8),
    highlightColor: const Color(0xffc8a8d8),
    splashColor: const Color(0xffc8a8d8),
    unselectedWidgetColor: const Color(0xff3e2851),
    disabledColor: const Color(0x77c8a8d8),
    secondaryHeaderColor: const Color(0xfff8f5ff),
    dialogBackgroundColor: const Color(0xfff8f5ff),
    indicatorColor: const Color(0xff9b7cb6),
    hintColor: const Color(0x803e2851),
    primaryTextTheme: Typography.material2021(platform: TargetPlatform.android).black,
    textTheme: Typography.material2021(platform: TargetPlatform.android).black,
    primaryIconTheme: const IconThemeData(color: Color(0xff3e2851)),
    iconTheme: base.iconTheme.copyWith(
      color: const Color(0xff3e2851),
    ),
    dividerTheme: base.dividerTheme.copyWith(
      color: const Color(0xffc8a8d8),
    ),
    sliderTheme: const SliderThemeData().copyWith(
      valueIndicatorColor: const Color(0xff9b7cb6),
      trackHeight: 2.0,
      thumbShape: const RoundSliderThumbShape(
        enabledThumbRadius: 6.0,
        disabledThumbRadius: 6.0,
      ),
    ),
    appBarTheme: base.appBarTheme.copyWith(
      backgroundColor: const Color(0xfff8f5ff),
      foregroundColor: const Color(0xff3e2851),
      shadowColor: const Color(0xfff8f5ff),
      elevation: 1.0,
      systemOverlayStyle: SystemUiOverlayStyle.dark.copyWith(
        systemNavigationBarIconBrightness: Brightness.dark,
        systemNavigationBarColor: const Color(0xfff8f5ff),
        statusBarIconBrightness: Brightness.dark,
      ),
    ),
    snackBarTheme: base.snackBarTheme.copyWith(
      actionTextColor: const Color(0xff9b7cb6),
    ),
    outlinedButtonTheme: OutlinedButtonThemeData(
      style: OutlinedButton.styleFrom(
        foregroundColor: const Color(0xff3e2851),
        side: const BorderSide(
          color: Color(0xff9b7cb6),
          style: BorderStyle.solid,
        ),
      ),
    ),
    elevatedButtonTheme: ElevatedButtonThemeData(
      style: ElevatedButton.styleFrom(
        backgroundColor: const Color(0xff9b7cb6),
        foregroundColor: const Color(0xffffffff),
      ),
    ),
  );
}

ThemeData _buildMintyFreshTheme() {
  final base = ThemeData.light(useMaterial3: false);
  
  return base.copyWith(
    colorScheme: const ColorScheme.light(
      primary: Color(0xff0d9488),
      secondary: Color(0xff5eead4),
      surface: Color(0xfff0fdfa),
      error: Color(0xffdc2626),
      onSurface: Color(0xff134e4a),
      background: Color(0xffecfdf5),
    ),
    bottomAppBarTheme: const BottomAppBarThemeData().copyWith(
      color: const Color(0xfff0fdfa),
    ),
    cardTheme: const CardThemeData().copyWith(
      color: const Color(0xfff0fdfa),
      shadowColor: const Color(0xff5eead4),
    ),
    brightness: Brightness.light,
    primaryColor: const Color(0xff0d9488),
    canvasColor: const Color(0xffecfdf5),
    scaffoldBackgroundColor: const Color(0xffecfdf5),
    cardColor: const Color(0xfff0fdfa),
    dividerColor: const Color(0xff5eead4),
    highlightColor: const Color(0xff5eead4),
    splashColor: const Color(0xff5eead4),
    unselectedWidgetColor: const Color(0xff134e4a),
    disabledColor: const Color(0x775eead4),
    secondaryHeaderColor: const Color(0xfff0fdfa),
    dialogBackgroundColor: const Color(0xfff0fdfa),
    indicatorColor: const Color(0xff0d9488),
    hintColor: const Color(0x80134e4a),
    primaryTextTheme: Typography.material2021(platform: TargetPlatform.android).black,
    textTheme: Typography.material2021(platform: TargetPlatform.android).black,
    primaryIconTheme: const IconThemeData(color: Color(0xff134e4a)),
    iconTheme: base.iconTheme.copyWith(
      color: const Color(0xff134e4a),
    ),
    dividerTheme: base.dividerTheme.copyWith(
      color: const Color(0xff5eead4),
    ),
    sliderTheme: const SliderThemeData().copyWith(
      valueIndicatorColor: const Color(0xff0d9488),
      trackHeight: 2.0,
      thumbShape: const RoundSliderThumbShape(
        enabledThumbRadius: 6.0,
        disabledThumbRadius: 6.0,
      ),
    ),
    appBarTheme: base.appBarTheme.copyWith(
      backgroundColor: const Color(0xfff0fdfa),
      foregroundColor: const Color(0xff134e4a),
      shadowColor: const Color(0xfff0fdfa),
      elevation: 1.0,
      systemOverlayStyle: SystemUiOverlayStyle.dark.copyWith(
        systemNavigationBarIconBrightness: Brightness.dark,
        systemNavigationBarColor: const Color(0xfff0fdfa),
        statusBarIconBrightness: Brightness.dark,
      ),
    ),
    snackBarTheme: base.snackBarTheme.copyWith(
      actionTextColor: const Color(0xff0d9488),
    ),
    outlinedButtonTheme: OutlinedButtonThemeData(
      style: OutlinedButton.styleFrom(
        foregroundColor: const Color(0xff134e4a),
        side: const BorderSide(
          color: Color(0xff0d9488),
          style: BorderStyle.solid,
        ),
      ),
    ),
    elevatedButtonTheme: ElevatedButtonThemeData(
      style: ElevatedButton.styleFrom(
        backgroundColor: const Color(0xff0d9488),
        foregroundColor: const Color(0xffffffff),
      ),
    ),
  );
}

ThemeData _buildWarmVanillaTheme() {
  final base = ThemeData.light(useMaterial3: false);
  
  return base.copyWith(
    colorScheme: const ColorScheme.light(
      primary: Color(0xffd97706),
      secondary: Color(0xfffbbf24),
      surface: Color(0xfffffbeb),
      error: Color(0xffdc2626),
      onSurface: Color(0xff78350f),
      background: Color(0xfffef3c7),
    ),
    bottomAppBarTheme: const BottomAppBarThemeData().copyWith(
      color: const Color(0xfffffbeb),
    ),
    cardTheme: const CardThemeData().copyWith(
      color: const Color(0xfffffbeb),
      shadowColor: const Color(0xfffbbf24),
    ),
    brightness: Brightness.light,
    primaryColor: const Color(0xffd97706),
    canvasColor: const Color(0xfffef3c7),
    scaffoldBackgroundColor: const Color(0xfffef3c7),
    cardColor: const Color(0xfffffbeb),
    dividerColor: const Color(0xfffbbf24),
    highlightColor: const Color(0xfffbbf24),
    splashColor: const Color(0xfffbbf24),
    unselectedWidgetColor: const Color(0xff78350f),
    disabledColor: const Color(0x77fbbf24),
    secondaryHeaderColor: const Color(0xfffffbeb),
    dialogBackgroundColor: const Color(0xfffffbeb),
    indicatorColor: const Color(0xffd97706),
    hintColor: const Color(0x8078350f),
    primaryTextTheme: Typography.material2021(platform: TargetPlatform.android).black,
    textTheme: Typography.material2021(platform: TargetPlatform.android).black,
    primaryIconTheme: const IconThemeData(color: Color(0xff78350f)),
    iconTheme: base.iconTheme.copyWith(
      color: const Color(0xff78350f),
    ),
    dividerTheme: base.dividerTheme.copyWith(
      color: const Color(0xfffbbf24),
    ),
    sliderTheme: const SliderThemeData().copyWith(
      valueIndicatorColor: const Color(0xffd97706),
      trackHeight: 2.0,
      thumbShape: const RoundSliderThumbShape(
        enabledThumbRadius: 6.0,
        disabledThumbRadius: 6.0,
      ),
    ),
    appBarTheme: base.appBarTheme.copyWith(
      backgroundColor: const Color(0xfffffbeb),
      foregroundColor: const Color(0xff78350f),
      shadowColor: const Color(0xfffffbeb),
      elevation: 1.0,
      systemOverlayStyle: SystemUiOverlayStyle.dark.copyWith(
        systemNavigationBarIconBrightness: Brightness.dark,
        systemNavigationBarColor: const Color(0xfffffbeb),
        statusBarIconBrightness: Brightness.dark,
      ),
    ),
    snackBarTheme: base.snackBarTheme.copyWith(
      actionTextColor: const Color(0xffd97706),
    ),
    outlinedButtonTheme: OutlinedButtonThemeData(
      style: OutlinedButton.styleFrom(
        foregroundColor: const Color(0xff78350f),
        side: const BorderSide(
          color: Color(0xffd97706),
          style: BorderStyle.solid,
        ),
      ),
    ),
    elevatedButtonTheme: ElevatedButtonThemeData(
      style: ElevatedButton.styleFrom(
        backgroundColor: const Color(0xffd97706),
        foregroundColor: const Color(0xffffffff),
      ),
    ),
  );
}

ThemeData _buildCoastalBlueTheme() {
  final base = ThemeData.light(useMaterial3: false);
  
  return base.copyWith(
    colorScheme: const ColorScheme.light(
      primary: Color(0xff0369a1),
      secondary: Color(0xff7dd3fc),
      surface: Color(0xfff0f9ff),
      error: Color(0xffdc2626),
      onSurface: Color(0xff0c4a6e),
      background: Color(0xffe0f2fe),
    ),
    bottomAppBarTheme: const BottomAppBarThemeData().copyWith(
      color: const Color(0xfff0f9ff),
    ),
    cardTheme: const CardThemeData().copyWith(
      color: const Color(0xfff0f9ff),
      shadowColor: const Color(0xff7dd3fc),
    ),
    brightness: Brightness.light,
    primaryColor: const Color(0xff0369a1),
    canvasColor: const Color(0xffe0f2fe),
    scaffoldBackgroundColor: const Color(0xffe0f2fe),
    cardColor: const Color(0xfff0f9ff),
    dividerColor: const Color(0xff7dd3fc),
    highlightColor: const Color(0xff7dd3fc),
    splashColor: const Color(0xff7dd3fc),
    unselectedWidgetColor: const Color(0xff0c4a6e),
    disabledColor: const Color(0x777dd3fc),
    secondaryHeaderColor: const Color(0xfff0f9ff),
    dialogBackgroundColor: const Color(0xfff0f9ff),
    indicatorColor: const Color(0xff0369a1),
    hintColor: const Color(0x800c4a6e),
    primaryTextTheme: Typography.material2021(platform: TargetPlatform.android).black,
    textTheme: Typography.material2021(platform: TargetPlatform.android).black,
    primaryIconTheme: const IconThemeData(color: Color(0xff0c4a6e)),
    iconTheme: base.iconTheme.copyWith(
      color: const Color(0xff0c4a6e),
    ),
    dividerTheme: base.dividerTheme.copyWith(
      color: const Color(0xff7dd3fc),
    ),
    sliderTheme: const SliderThemeData().copyWith(
      valueIndicatorColor: const Color(0xff0369a1),
      trackHeight: 2.0,
      thumbShape: const RoundSliderThumbShape(
        enabledThumbRadius: 6.0,
        disabledThumbRadius: 6.0,
      ),
    ),
    appBarTheme: base.appBarTheme.copyWith(
      backgroundColor: const Color(0xfff0f9ff),
      foregroundColor: const Color(0xff0c4a6e),
      shadowColor: const Color(0xfff0f9ff),
      elevation: 1.0,
      systemOverlayStyle: SystemUiOverlayStyle.dark.copyWith(
        systemNavigationBarIconBrightness: Brightness.dark,
        systemNavigationBarColor: const Color(0xfff0f9ff),
        statusBarIconBrightness: Brightness.dark,
      ),
    ),
    snackBarTheme: base.snackBarTheme.copyWith(
      actionTextColor: const Color(0xff0369a1),
    ),
    outlinedButtonTheme: OutlinedButtonThemeData(
      style: OutlinedButton.styleFrom(
        foregroundColor: const Color(0xff0c4a6e),
        side: const BorderSide(
          color: Color(0xff0369a1),
          style: BorderStyle.solid,
        ),
      ),
    ),
    elevatedButtonTheme: ElevatedButtonThemeData(
      style: ElevatedButton.styleFrom(
        backgroundColor: const Color(0xff0369a1),
        foregroundColor: const Color(0xffffffff),
      ),
    ),
  );
}

ThemeData _buildPaperCreamTheme() {
  final base = ThemeData.light(useMaterial3: false);
  
  return base.copyWith(
    colorScheme: const ColorScheme.light(
      primary: Color(0xff8b5a3c),
      secondary: Color(0xffd4af8c),
      surface: Color(0xfff9f7f4),
      error: Color(0xffdc2626),
      onSurface: Color(0xff4a3728),
      background: Color(0xfff5f2ef),
    ),
    bottomAppBarTheme: const BottomAppBarThemeData().copyWith(
      color: const Color(0xfff9f7f4),
    ),
    cardTheme: const CardThemeData().copyWith(
      color: const Color(0xfff9f7f4),
      shadowColor: const Color(0xffd4af8c),
    ),
    brightness: Brightness.light,
    primaryColor: const Color(0xff8b5a3c),
    canvasColor: const Color(0xfff5f2ef),
    scaffoldBackgroundColor: const Color(0xfff5f2ef),
    cardColor: const Color(0xfff9f7f4),
    dividerColor: const Color(0xffd4af8c),
    highlightColor: const Color(0xffd4af8c),
    splashColor: const Color(0xffd4af8c),
    unselectedWidgetColor: const Color(0xff4a3728),
    disabledColor: const Color(0x77d4af8c),
    secondaryHeaderColor: const Color(0xfff9f7f4),
    dialogBackgroundColor: const Color(0xfff9f7f4),
    indicatorColor: const Color(0xff8b5a3c),
    hintColor: const Color(0x804a3728),
    primaryTextTheme: Typography.material2021(platform: TargetPlatform.android).black,
    textTheme: Typography.material2021(platform: TargetPlatform.android).black,
    primaryIconTheme: const IconThemeData(color: Color(0xff4a3728)),
    iconTheme: base.iconTheme.copyWith(
      color: const Color(0xff4a3728),
    ),
    dividerTheme: base.dividerTheme.copyWith(
      color: const Color(0xffd4af8c),
    ),
    sliderTheme: const SliderThemeData().copyWith(
      valueIndicatorColor: const Color(0xff8b5a3c),
      trackHeight: 2.0,
      thumbShape: const RoundSliderThumbShape(
        enabledThumbRadius: 6.0,
        disabledThumbRadius: 6.0,
      ),
    ),
    appBarTheme: base.appBarTheme.copyWith(
      backgroundColor: const Color(0xfff9f7f4),
      foregroundColor: const Color(0xff4a3728),
      shadowColor: const Color(0xfff9f7f4),
      elevation: 1.0,
      systemOverlayStyle: SystemUiOverlayStyle.dark.copyWith(
        systemNavigationBarIconBrightness: Brightness.dark,
        systemNavigationBarColor: const Color(0xfff9f7f4),
        statusBarIconBrightness: Brightness.dark,
      ),
    ),
    snackBarTheme: base.snackBarTheme.copyWith(
      actionTextColor: const Color(0xff8b5a3c),
    ),
    outlinedButtonTheme: OutlinedButtonThemeData(
      style: OutlinedButton.styleFrom(
        foregroundColor: const Color(0xff4a3728),
        side: const BorderSide(
          color: Color(0xff8b5a3c),
          style: BorderStyle.solid,
        ),
      ),
    ),
    elevatedButtonTheme: ElevatedButtonThemeData(
      style: ElevatedButton.styleFrom(
        backgroundColor: const Color(0xff8b5a3c),
        foregroundColor: const Color(0xffffffff),
      ),
    ),
  );
}

ThemeData _buildGithubLightTheme() {
  final base = ThemeData.light(useMaterial3: false);
  
  return base.copyWith(
    colorScheme: const ColorScheme.light(
      primary: Color(0xff0969da),
      secondary: Color(0xff54aeff),
      surface: Color(0xffffffff),
      error: Color(0xffcf222e),
      onSurface: Color(0xff1f2328),
      background: Color(0xfff6f8fa),
    ),
    bottomAppBarTheme: const BottomAppBarThemeData().copyWith(
      color: const Color(0xffffffff),
    ),
    cardTheme: const CardThemeData().copyWith(
      color: const Color(0xffffffff),
      shadowColor: const Color(0xffd0d7de),
    ),
    brightness: Brightness.light,
    primaryColor: const Color(0xff0969da),
    canvasColor: const Color(0xfff6f8fa),
    scaffoldBackgroundColor: const Color(0xfff6f8fa),
    cardColor: const Color(0xffffffff),
    dividerColor: const Color(0xffd0d7de),
    highlightColor: const Color(0xffd0d7de),
    splashColor: const Color(0xffd0d7de),
    unselectedWidgetColor: const Color(0xff1f2328),
    disabledColor: const Color(0x77d0d7de),
    secondaryHeaderColor: const Color(0xffffffff),
    dialogBackgroundColor: const Color(0xffffffff),
    indicatorColor: const Color(0xff0969da),
    hintColor: const Color(0x801f2328),
    primaryTextTheme: Typography.material2021(platform: TargetPlatform.android).black,
    textTheme: Typography.material2021(platform: TargetPlatform.android).black,
    primaryIconTheme: const IconThemeData(color: Color(0xff1f2328)),
    iconTheme: base.iconTheme.copyWith(
      color: const Color(0xff1f2328),
    ),
    dividerTheme: base.dividerTheme.copyWith(
      color: const Color(0xffd0d7de),
    ),
    sliderTheme: const SliderThemeData().copyWith(
      valueIndicatorColor: const Color(0xff0969da),
      trackHeight: 2.0,
      thumbShape: const RoundSliderThumbShape(
        enabledThumbRadius: 6.0,
        disabledThumbRadius: 6.0,
      ),
    ),
    appBarTheme: base.appBarTheme.copyWith(
      backgroundColor: const Color(0xffffffff),
      foregroundColor: const Color(0xff1f2328),
      shadowColor: const Color(0xffffffff),
      elevation: 1.0,
      systemOverlayStyle: SystemUiOverlayStyle.dark.copyWith(
        systemNavigationBarIconBrightness: Brightness.dark,
        systemNavigationBarColor: const Color(0xffffffff),
        statusBarIconBrightness: Brightness.dark,
      ),
    ),
    snackBarTheme: base.snackBarTheme.copyWith(
      actionTextColor: const Color(0xff0969da),
    ),
    outlinedButtonTheme: OutlinedButtonThemeData(
      style: OutlinedButton.styleFrom(
        foregroundColor: const Color(0xff1f2328),
        side: const BorderSide(
          color: Color(0xff0969da),
          style: BorderStyle.solid,
        ),
      ),
    ),
    elevatedButtonTheme: ElevatedButtonThemeData(
      style: ElevatedButton.styleFrom(
        backgroundColor: const Color(0xff0969da),
        foregroundColor: const Color(0xffffffff),
      ),
    ),
  );
}

ThemeData _buildHotDogStandTheme() {
  final base = ThemeData.light(useMaterial3: false);
  
  return base.copyWith(
    colorScheme: const ColorScheme.light(
      primary: Color(0xffff0000),
      secondary: Color(0xffffff00),
      surface: Color(0xffffff00),
      error: Color(0xffffff00),
      onSurface: Color(0xff000000),
      background: Color(0xffffff00),
    ),
    bottomAppBarTheme: const BottomAppBarThemeData().copyWith(
      color: const Color(0xffffff00),
    ),
    cardTheme: const CardThemeData().copyWith(
      color: const Color(0xffffff00),
      shadowColor: const Color(0xffffffff),
    ),
    brightness: Brightness.light,
    primaryColor: const Color(0xffff0000),
    canvasColor: const Color(0xffffff00),
    scaffoldBackgroundColor: const Color(0xffffff00),
    cardColor: const Color(0xffffff00),
    dividerColor: const Color(0xffff0000),
    highlightColor: const Color(0xffff0000),
    splashColor: const Color(0xffff0000),
    unselectedWidgetColor: const Color(0xff000000),
    disabledColor: const Color(0x77ff0000),
    secondaryHeaderColor: const Color(0xffffff00),
    dialogBackgroundColor: const Color(0xffffff00),
    indicatorColor: const Color(0xffff0000),
    hintColor: const Color(0x80000000),
    primaryTextTheme: Typography.material2021(platform: TargetPlatform.android).black,
    textTheme: Typography.material2021(platform: TargetPlatform.android).black,
    primaryIconTheme: const IconThemeData(color: Color(0xff000000)),
    iconTheme: base.iconTheme.copyWith(
      color: const Color(0xff000000),
    ),
    dividerTheme: base.dividerTheme.copyWith(
      color: const Color(0xffff0000),
    ),
    sliderTheme: const SliderThemeData().copyWith(
      valueIndicatorColor: const Color(0xffff0000),
      trackHeight: 2.0,
      thumbShape: const RoundSliderThumbShape(
        enabledThumbRadius: 6.0,
        disabledThumbRadius: 6.0,
      ),
    ),
    appBarTheme: base.appBarTheme.copyWith(
      backgroundColor: const Color(0xffffff00),
      foregroundColor: const Color(0xff000000),
      shadowColor: const Color(0xffffff00),
      elevation: 1.0,
      systemOverlayStyle: SystemUiOverlayStyle.dark.copyWith(
        systemNavigationBarIconBrightness: Brightness.dark,
        systemNavigationBarColor: const Color(0xffffff00),
        statusBarIconBrightness: Brightness.dark,
      ),
    ),
    snackBarTheme: base.snackBarTheme.copyWith(
      actionTextColor: const Color(0xffff0000),
    ),
    outlinedButtonTheme: OutlinedButtonThemeData(
      style: OutlinedButton.styleFrom(
        foregroundColor: const Color(0xff000000),
        side: const BorderSide(
          color: Color(0xffff0000),
          style: BorderStyle.solid,
        ),
      ),
    ),
    elevatedButtonTheme: ElevatedButtonThemeData(
      style: ElevatedButton.styleFrom(
        backgroundColor: const Color(0xffff0000),
        foregroundColor: const Color(0xffffff00),
      ),
    ),
  );
}
