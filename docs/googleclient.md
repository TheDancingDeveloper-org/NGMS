# NGMS Player — Google Play Store Build Guide

## Overview

The NGMS Player is a Tauri 2 Android app wrapping the React frontend. It connects to the user's NGMS server via the bootstrap invite code flow, which handles server discovery and registration automatically.

- **App identifier:** `com.ngms.player`
- **Min SDK:** 24 (Android 7.0)
- **Tauri config:** `client/src-tauri/tauri.conf.json`

## Prerequisites (Windows)

### 1. Android Studio

- Download and install [Android Studio](https://developer.android.com/studio)
- During setup, install:
  - Android SDK (API 34+)
  - Android NDK (27+)
  - Android SDK Build-Tools
  - Android SDK Command-line Tools

### 2. Environment Variables

Add to Windows system environment variables:

```
ANDROID_HOME = C:\Users\<you>\AppData\Local\Android\Sdk
NDK_HOME = %ANDROID_HOME%\ndk\<version>
JAVA_HOME = C:\Program Files\Android\Android Studio\jbr
```

Add to `PATH`:
```
%ANDROID_HOME%\platform-tools
%ANDROID_HOME%\cmdline-tools\latest\bin
```

### 3. Rust Android Targets

```powershell
rustup target add aarch64-linux-android
rustup target add armv7-linux-androideabi
rustup target add x86_64-linux-android
rustup target add i686-linux-android
```

### 4. Node.js + npm

Ensure Node.js is installed on Windows (not just WSL2).

## Setup Steps

### 1. Clone / sync the repo on Windows

### 2. Install frontend deps

```powershell
cd client
npm install
```

### 3. Initialize Android project

```powershell
npx tauri android init
```

This generates `client/src-tauri/gen/android/` — a full Gradle project. This directory is gitignored by default.

### 4. Network Security Config

Android 9+ blocks cleartext HTTP by default. NGMS needs to probe local IPs during server discovery.

After `tauri android init`, create `client/src-tauri/gen/android/app/src/main/res/xml/network_security_config.xml`:

```xml
<?xml version="1.0" encoding="utf-8"?>
<network-security-config>
    <base-config cleartextTrafficPermitted="false">
        <trust-anchors>
            <certificates src="system" />
        </trust-anchors>
    </base-config>
    <!-- Allow cleartext for local/private networks (server discovery) -->
    <domain-config cleartextTrafficPermitted="true">
        <domain includeSubdomains="true">10.0.0.0</domain>
        <domain includeSubdomains="true">172.16.0.0</domain>
        <domain includeSubdomains="true">192.168.0.0</domain>
    </domain-config>
</network-security-config>
```

Then reference it in `AndroidManifest.xml`:
```xml
<application android:networkSecurityConfig="@xml/network_security_config" ...>
```

> **Note:** The `domain-config` approach above is simplified. You may need to use `usesCleartextTraffic="true"` on the application tag as a broader alternative, or refine the domain list based on your network setup. Test on a real device to confirm.

### 5. Dev Build (device/emulator)

```powershell
npx tauri android dev
```

Requires either:
- A physical device connected via USB with developer mode enabled
- An Android emulator running in Android Studio

### 6. Release Build (AAB for Play Store)

```powershell
npx tauri android build
```

Produces an `.aab` (Android App Bundle) at:
```
client/src-tauri/gen/android/app/build/outputs/bundle/universalRelease/app-universal-release.aab
```

## Signing

The AAB must be signed for Play Store upload.

### Option A: Let Tauri handle it

Add to `client/src-tauri/tauri.conf.json` under `bundle.android`:
```json
{
  "bundle": {
    "android": {
      "minSdkVersion": 24,
      "signing": {
        "keystore": "path/to/ngms-release.keystore",
        "keystorePassword": "env:NGMS_KEYSTORE_PASSWORD",
        "keyAlias": "ngms",
        "keyPassword": "env:NGMS_KEY_PASSWORD"
      }
    }
  }
}
```

### Option B: Generate keystore manually

```powershell
keytool -genkey -v -keystore ngms-release.keystore -alias ngms -keyalg RSA -keysize 2048 -validity 10000
```

**IMPORTANT:** Back up the keystore and passwords. Store in `pass` or equivalent. Losing the signing key means you cannot update the app on the Play Store.

## Google Play Console Setup

1. Create a [Google Play Developer account](https://play.google.com/console/) ($25 one-time fee)
2. Create a new app:
   - App name: **NGMS Player**
   - Default language: English
   - App or game: App
   - Free or paid: Free
3. Go to **Testing → Internal testing** → Create a new release
4. Upload the signed `.aab`
5. Add testers by email address
6. Testers receive a Play Store link to install

## Architecture Notes

### Connectivity Flow

The existing bootstrap invite flow works as-is for Android:

1. User enters invite code in the app
2. Client calls bootstrap (`streambootstrap.indexarr.net`) to redeem code
3. Bootstrap returns server IPs (local + public) and port
4. Client probes each IP to find a reachable server (needs cleartext exemption for LAN)
5. Server URL persisted in WebView localStorage
6. User registers with pre-filled invite code
7. On login, request a `device_name` to get a persistent device token for mobile auth

### What doesn't need to change

- Bootstrap service — already returns all needed info
- Server auth — device tokens already supported
- Registration flow — invite code pre-fill already works
- Server URL persistence — localStorage works in Tauri WebView

### What to verify on Android

- Cleartext HTTP probing works on LAN with the network security config
- WebView localStorage persists across app restarts
- Device token auth works (vs session cookies which may not persist in WebView)
- Deep links / URL schemes if needed later

## Troubleshooting

- **Gradle build fails:** Ensure `JAVA_HOME` points to Android Studio's bundled JDK, not a system Java
- **NDK not found:** Verify `NDK_HOME` matches the installed NDK version directory
- **ADB doesn't see device:** Enable USB debugging on phone, try different USB cable/port
- **Cleartext blocked:** Check logcat for `CLEARTEXT communication not permitted` — verify network security config is applied
