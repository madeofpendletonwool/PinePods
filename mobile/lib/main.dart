// Copyright 2020 Ben Hills and the project contributors. All rights reserved.
// Use of this source code is governed by a BSD-style license that can be
// found in the LICENSE file.

import 'dart:io';

import 'package:pinepods_mobile/services/settings/mobile_settings_service.dart';
import 'package:pinepods_mobile/services/logging/app_logger.dart';
import 'package:pinepods_mobile/ui/pinepods_podcast_app.dart';
import 'package:pinepods_mobile/ui/widgets/restart_widget.dart';
import 'package:device_info_plus/device_info_plus.dart';
import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:logging/logging.dart';

// ignore_for_file: avoid_print
void main() async {
  List<int> certificateAuthorityBytes = [];
  WidgetsFlutterBinding.ensureInitialized();
  SystemChrome.setSystemUIOverlayStyle(const SystemUiOverlayStyle(statusBarColor: Colors.transparent));

  // Initialize app logger
  final appLogger = AppLogger();
  await appLogger.initialize();

  Logger.root.level = Level.FINE;

  Logger.root.onRecord.listen((record) {
    print('${record.level.name}: - ${record.time}: ${record.loggerName}: ${record.message}');
    
    // Also log to our app logger
    LogLevel appLogLevel;
    switch (record.level.name) {
      case 'SEVERE':
        appLogLevel = LogLevel.critical;
        break;
      case 'WARNING':
        appLogLevel = LogLevel.warning;
        break;
      case 'INFO':
        appLogLevel = LogLevel.info;
        break;
      case 'FINE':
      case 'FINER':
      case 'FINEST':
        appLogLevel = LogLevel.debug;
        break;
      default:
        appLogLevel = LogLevel.info;
        break;
    }
    
    appLogger.log(appLogLevel, record.loggerName, record.message);
  });

  var mobileSettingsService = (await MobileSettingsService.instance())!;
  certificateAuthorityBytes = await setupCertificateAuthority();

  runApp(RestartWidget(
    child: PinepodsPodcastApp(
      mobileSettingsService: mobileSettingsService,
      certificateAuthorityBytes: certificateAuthorityBytes,
    ),
  ));
}

/// When certificate authorities certificates expire, older devices may not be able to handle
/// the re-issued certificate resulting in SSL errors being thrown. This routine is called to
/// manually install the newer certificates on older devices so they continue to work.
Future<List<int>> setupCertificateAuthority() async {
  List<int> ca = [];
  var loadedCerts = false;

  if (Platform.isAndroid) {
    DeviceInfoPlugin deviceInfo = DeviceInfoPlugin();
    AndroidDeviceInfo androidInfo = await deviceInfo.androidInfo;
    var major = androidInfo.version.release.split('.');

    if ((int.tryParse(major[0]) ?? 100.0) < 8.0) {
      ByteData data = await PlatformAssetBundle().load('assets/ca/lets-encrypt-r3.pem');
      ca.addAll(data.buffer.asUint8List());
      loadedCerts = true;
    }

    if ((int.tryParse(major[0]) ?? 100.0) < 10.0) {
      ByteData data = await PlatformAssetBundle().load('assets/ca/globalsign-gcc-r6-alphassl-ca-2023.pem');
      ca.addAll(data.buffer.asUint8List());
      loadedCerts = true;
    }

    if (loadedCerts) {
      SecurityContext.defaultContext.setTrustedCertificatesBytes(ca);
    }
  }

  return ca;
}
