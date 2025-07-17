# 🚀 PinePods Mobile - Complete Store Deployment Guide

This guide covers deployment to **Google Play Store**, **iOS App Store**, **F-Droid**, and **IzzyOnDroid**.

## 📋 Overview

- **Google Play Store**: Official Android distribution
- **iOS App Store**: Official iOS distribution
- **F-Droid**: Open-source Android app repository
- **IzzyOnDroid**: F-Droid compatible repository with faster updates

---

## 🔐 Step 1: Create Signing Certificates & Keys

### **Android Keystore (Required for Google Play, F-Droid, IzzyOnDroid)**

```bash
# Create upload keystore
keytool -genkey -v -keystore upload-keystore.jks -keyalg RSA -keysize 2048 -validity 10000 -alias upload

# Follow prompts to set:
# - Keystore password (save as ANDROID_STORE_PASSWORD)
# - Key password (save as ANDROID_KEY_PASSWORD)
# - Alias: "upload" (save as ANDROID_KEY_ALIAS)
# - Your name/organization details

# Convert keystore to base64 for GitHub secrets
base64 upload-keystore.jks > keystore.base64.txt
# Copy contents for ANDROID_KEYSTORE_BASE64 secret
```

### **Google Play Console API Key**

1. Go to [Google Cloud Console](https://console.cloud.google.com/)
2. Create new project or select existing project
3. Enable **Google Play Developer API**
4. Go to IAM & Admin → Service Accounts
5. Create Service Account with name "github-actions"
6. Grant **Service Account User** role
7. Create key → JSON format → Download
8. Convert to base64: `base64 service-account.json > gplay-api.base64.txt`
9. Copy contents for `GOOGLE_PLAY_SERVICE_ACCOUNT_JSON` secret

### **iOS Distribution Certificate (Apple Developer Account Required - $99/year)**

1. **Get Apple Developer Account**: https://developer.apple.com/programs/
2. **Create Distribution Certificate**:
   - Open Xcode → Preferences → Accounts
   - Add your Apple ID → Select team
   - Manage Certificates → "+" → iOS Distribution
   - Export certificate as .p12 file with password
   - Convert: `base64 certificate.p12 > ios-cert.base64.txt`
   - Use for `IOS_CERTIFICATE_BASE64` and password for `IOS_CERTIFICATE_PASSWORD`

3. **Create App Store Provisioning Profile**:
   - Go to [Apple Developer Portal](https://developer.apple.com/)
   - Certificates, Identifiers & Profiles
   - Create App ID: `com.gooseberrydevelopment.pinepods`
   - Create App Store Provisioning Profile linked to your App ID
   - Download .mobileprovision file
   - Convert: `base64 profile.mobileprovision > ios-profile.base64.txt`
   - Use for `IOS_PROVISIONING_PROFILE_BASE64`

4. **App Store Connect API Key**:
   - Go to [App Store Connect](https://appstoreconnect.apple.com/)
   - Users and Access → Keys → "+"
   - Create key with "App Manager" role
   - Download .p8 file and note Key ID + Issuer ID
   - Convert: `base64 AuthKey_XXXXXXXXXX.p8 > app-store-api.base64.txt`
   - Key ID → `APP_STORE_CONNECT_API_KEY_ID`
   - Issuer ID → `APP_STORE_CONNECT_ISSUER_ID`
   - Base64 content → `APP_STORE_CONNECT_API_KEY_BASE64`

5. **Get Team ID**:
   - In Apple Developer Portal → Membership
   - Copy 10-character Team ID → `IOS_TEAM_ID`
   - Set any password for `KEYCHAIN_PASSWORD` (used temporarily in CI)

---

## 🔑 Step 2: Add GitHub Secrets

Go to your repo → **Settings** → **Secrets and variables** → **Actions** → **New repository secret**

### **Android Secrets:**
- `ANDROID_KEYSTORE_BASE64` - Base64 encoded upload-keystore.jks
- `ANDROID_STORE_PASSWORD` - Keystore password
- `ANDROID_KEY_PASSWORD` - Key password
- `ANDROID_KEY_ALIAS` - Key alias (usually "upload")
- `GOOGLE_PLAY_SERVICE_ACCOUNT_JSON` - Base64 encoded service account JSON

### **iOS Secrets:**
- `IOS_CERTIFICATE_BASE64` - Base64 encoded distribution certificate
- `IOS_CERTIFICATE_PASSWORD` - Certificate password
- `IOS_PROVISIONING_PROFILE_BASE64` - Base64 encoded provisioning profile
- `IOS_TEAM_ID` - Apple Developer team ID
- `KEYCHAIN_PASSWORD` - Any secure password for temporary keychain
- `APP_STORE_CONNECT_API_KEY_ID` - App Store Connect API key ID
- `APP_STORE_CONNECT_ISSUER_ID` - Issuer ID
- `APP_STORE_CONNECT_API_KEY_BASE64` - Base64 encoded API key file

---

## 📱 Step 3: Google Play Store

### **Setup:**
1. **Create Google Play Console Account** ($25 one-time fee): https://play.google.com/console
2. **Create App Listing**:
   - Create app → Package name: `com.gooseberrydevelopment.pinepods`
   - App name: "PinePods"
   - Select "App" (not game)

### **Required Assets:**
Create these images and place in `mobile/fastlane/metadata/android/en-US/images/`:

- **App Icon**: `icon/icon.png` (512x512px)
- **Feature Graphic**: `featureGraphic/feature.png` (1024x500px)
- **Phone Screenshots**: `phoneScreenshots/` (4-8 screenshots, 16:9 or 9:16 ratio)
- **Tablet Screenshots**: `tenInchScreenshots/` (4-8 screenshots, landscape recommended)

### **App Information:**
- **Privacy Policy URL**: Required (create one at https://privacypolicytemplate.net/)
- **Content Rating**: Complete questionnaire in Play Console
- **Target Audience**: 13+ (contains user-generated content)
- **Data Safety**: Declare what data your app collects

### **Deployment:**
```bash
# Test build locally
cd mobile
flutter build appbundle --release

# Deploy via GitHub Actions
git tag v0.7.9001
git push origin v0.7.9
# This triggers automatic build and upload to Play Store
```

---

## 🍎 Step 4: iOS App Store

### **Setup:**
1. **App Store Connect**: https://appstoreconnect.apple.com/
2. **Create App Record**:
   - Apps → "+" → New App
   - Bundle ID: `com.gooseberrydevelopment.pinepods`
   - App name: "PinePods"

### **Required Assets:**
Create these images and place in `mobile/fastlane/metadata/ios/en-US/images/`:

- **App Icon**: Various sizes (handled by flutter_launcher_icons)
- **iPhone Screenshots**: 6.7" display (1290x2796px) - 6-10 screenshots
- **iPad Screenshots**: 12.9" display (2048x2732px) - 6-10 screenshots

### **App Information:**
- **App Privacy**: Complete privacy questionnaire
- **Age Rating**: 12+ (realistic infrequent violence due to podcast content)
- **App Review Information**: Provide test account credentials
- **Export Compliance**: Select "No" unless app uses encryption

### **Deployment:**
```bash
# Test build locally (macOS only)
cd mobile
flutter build ios --release --no-codesign

# Deploy via GitHub Actions
git tag v0.7.9001
git push origin v0.7.9
# This triggers automatic build and upload to App Store Connect
```

---

## 🤖 Step 5: F-Droid

F-Droid builds apps from source automatically. No signing required from your end.

### **Requirements Met ✅:**
- ✅ Open source (GitHub repository)
- ✅ No proprietary dependencies
- ✅ Metadata files created in `mobile/metadata/`
- ✅ Build workflow for unsigned APK

### **Submission Process:**
1. **Fork F-Droid Data Repository**: https://gitlab.com/fdroid/fdroiddata
2. **Create App Metadata**:
   ```bash
   # Clone your fork
   git clone https://gitlab.com/yourusername/fdroiddata.git
   cd fdroiddata

   # Create app directory
   mkdir metadata/com.gooseberrydevelopment.pinepods.yml
   ```

3. **Create Metadata File** (`metadata/com.gooseberrydevelopment.pinepods.yml`):
   ```yaml
   Categories:
     - Multimedia
   License: GPL-3.0-or-later
   AuthorName: Collin Pendleton
   AuthorEmail: your-email@example.com
   SourceCode: https://github.com/madeofpendletonwool/PinePods
   IssueTracker: https://github.com/madeofpendletonwool/PinePods/issues

   AutoName: PinePods
   Description: |-
       A beautiful, self-hosted podcast app with powerful server synchronization.

       Features:
       * Self-hosted podcast server synchronization
       * Beautiful, intuitive mobile interface
       * Download episodes for offline listening
       * Chapter support with navigation
       * Playlist management
       * User statistics and listening history
       * Multi-device synchronization
       * Search and discovery
       * Background audio playback
       * Sleep timer and playback speed controls

       Note: This app requires a PinePods server to be set up.

   RepoType: git
   Repo: https://github.com/madeofpendletonwool/PinePods.git
   Binaries: https://github.com/madeofpendletonwool/PinePods/releases/download/v%v/PinePods-fdroid-%v.apk

   Builds:
     - versionName: 0.7.9
       versionCode: 20250714
       commit: v0.7.9
       subdir: mobile
       output: build/app/outputs/flutter-apk/app-release.apk
       build:
         - $$flutter$$/bin/flutter config --no-analytics
         - $$flutter$$/bin/flutter pub get
         - $$flutter$$/bin/flutter build apk --release

   AutoUpdateMode: Version v%v
   UpdateCheckMode: Tags
   CurrentVersion: 0.7.9
   CurrentVersionCode: 20250714
   ```

4. **Submit Merge Request**:
   ```bash
   git add metadata/com.gooseberrydevelopment.pinepods.yml
   git commit -m "Add PinePods podcast app"
   git push origin master
   # Create merge request in GitLab
   ```

5. **F-Droid Review Process**:
   - Review can take 2-8 weeks
   - Maintainers will test build and review code
   - Address any feedback in follow-up commits

---

## ⚡ Step 6: IzzyOnDroid

IzzyOnDroid accepts APKs directly and offers faster updates than F-Droid.

### **Requirements:**
- ✅ Signed APK (using your Android keystore)
- ✅ Open source repository
- ✅ No tracking libraries (F-Droid friendly)

### **Submission Process:**

1. **Build Signed APK**:
   ```bash
   cd mobile/android
   # Create key.properties file
   echo "storePassword=YOUR_STORE_PASSWORD" > key.properties
   echo "keyPassword=YOUR_KEY_PASSWORD" >> key.properties
   echo "keyAlias=upload" >> key.properties
   echo "storeFile=../upload-keystore.jks" >> key.properties

   # Copy your keystore
   cp /path/to/upload-keystore.jks ./

   # Build signed APK
   cd ..
   flutter build apk --release
   ```

2. **Create GitHub Release**:
   ```bash
   git tag v0.7.9
   git push origin v0.7.9
   # Upload the signed APK to GitHub releases
   ```

3. **Submit to IzzyOnDroid**:
   - **Email**: android@izzysoft.de
   - **Subject**: "New app submission: PinePods"
   - **Include**:
     - App name: PinePods
     - Package name: com.gooseberrydevelopment.pinepods
     - Source code: https://github.com/madeofpendletonwool/PinePods
     - APK download: Link to GitHub release
     - Brief description of your app
     - License: GPL-3.0-or-later

4. **IzzyOnDroid Review**:
   - Usually processed within 1-2 weeks
   - Much faster than F-Droid
   - Compatible with F-Droid client

---

## 📸 Step 7: Create Screenshots & Assets

### **Required Screenshots:**

**For Google Play & IzzyOnDroid:**
- Phone: 4-8 screenshots (minimum 1080px on shortest side)
- Tablet: 4-8 screenshots (minimum 1200px on shortest side)

**For iOS App Store:**
- iPhone: 6-10 screenshots (6.7" display: 1290x2796px)
- iPad: 6-10 screenshots (12.9" display: 2048x2732px)

**For F-Droid:**
- Phone: 2-6 screenshots (place in `fastlane/metadata/android/en-US/images/phoneScreenshots/`)

### **Screenshot Ideas:**
1. Home screen with episode list
2. Now playing screen with controls
3. Podcast discovery/search
4. Downloads/offline content
5. Settings/preferences
6. Player with chapters
7. Playlist management
8. User statistics

### **Tools for Screenshots:**
- **Android**: Use Android Studio Device Manager or physical device
- **iOS**: Use iOS Simulator or physical device
- **Design**: Use Figma/Canva for feature graphics

---

## 🚀 Step 8: Deploy Everything

### **1. Create Release**:
```bash
# Ensure all secrets are set in GitHub
# Ensure screenshots are added to fastlane/metadata directories
# Create and push tag
git tag v0.7.9
git push origin v0.7.9
```

### **2. Automated Deployments**:
- ✅ **Google Play**: Automatic upload via GitHub Actions
- ✅ **iOS App Store**: Automatic upload via GitHub Actions
- ✅ **F-Droid**: Builds automatically after merge request accepted
- ✅ **IzzyOnDroid**: Manual submission of signed APK

### **3. Monitor Builds**:
- Check GitHub Actions for build status
- Monitor store review processes
- Respond to any review feedback

---

## 📋 Checklist

### **Pre-Deployment:**
- [ ] Android keystore created and base64 encoded
- [ ] Google Play Console account created ($25)
- [ ] Apple Developer account created ($99/year)
- [ ] All GitHub secrets configured
- [ ] Screenshots created for all platforms
- [ ] Privacy policy created and published
- [ ] App descriptions finalized

### **Store Setup:**
- [ ] Google Play Console app listing created
- [ ] App Store Connect app record created
- [ ] F-Droid metadata file created
- [ ] IzzyOnDroid submission email prepared

### **Deploy:**
- [ ] Create GitHub release with tag
- [ ] Verify builds complete successfully
- [ ] Submit to F-Droid (create merge request)
- [ ] Submit to IzzyOnDroid (send email)
- [ ] Monitor store review processes

### **Post-Deployment:**
- [ ] Test apps on real devices
- [ ] Respond to user reviews
- [ ] Plan update process for future releases
- [ ] Monitor crash reports and analytics

---

## 🔄 Future Updates

For subsequent releases:

1. **Update version** in `pubspec.yaml` (e.g., 0.7.9002)
2. **Create new release tag**
3. **All platforms auto-update** except F-Droid (needs manual merge request for new versions)

---

## 📞 Support & Resources

- **Google Play Console**: https://support.google.com/googleplay/android-developer/
- **App Store Connect**: https://developer.apple.com/support/app-store-connect/
- **F-Droid Documentation**: https://f-droid.org/docs/
- **IzzyOnDroid**: https://apt.izzysoft.de/fdroid/
- **Flutter Deployment**: https://docs.flutter.dev/deployment

---

## 🎉 Success!

Once deployed, your app will be available on:
- **Google Play Store**: Official Android users
- **iOS App Store**: iPhone/iPad users
- **F-Droid**: Privacy-focused Android users
- **IzzyOnDroid**: F-Droid users who want faster updates

Your app will reach the maximum possible audience across all major distribution channels! 🚀
