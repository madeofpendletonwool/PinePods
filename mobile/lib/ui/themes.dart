// Copyright 2020 Ben Hills and the project contributors. All rights reserved.
// Use of this source code is governed by a BSD-style license that can be
// found in the LICENSE file.

import 'package:flutter/material.dart';
import 'package:flutter/services.dart';

final ThemeData _lightTheme = _buildLightTheme();
final ThemeData _darkTheme = _buildDarkTheme();

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
    bottomAppBarTheme: const BottomAppBarTheme().copyWith(
      color: const Color(0xffffffff),
    ),
    cardTheme: const CardTheme().copyWith(
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
    bottomAppBarTheme: const BottomAppBarTheme().copyWith(
      color: const Color(0xff222222),
    ),
    cardTheme: const CardTheme().copyWith(
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

class Themes {
  final ThemeData themeData;

  Themes({required this.themeData});

  factory Themes.lightTheme() {
    return Themes(themeData: _lightTheme);
  }

  factory Themes.darkTheme() {
    return Themes(themeData: _darkTheme);
  }
}
