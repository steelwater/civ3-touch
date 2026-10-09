#!/usr/bin/env bash
# Shell-only test DEX, never packaged in the APK. Use a disposable API-36 emulator.
set -euo pipefail
serial=${1:?Pass the isolated emulator serial}
output=$(mktemp -d)
trap 'rm -rf "$output"' EXIT
"$JAVA_HOME/bin/javac" -classpath "$ANDROID_HOME/platforms/android-36/android.jar" -d "$output" \
  android/app/src/main/java/org/civ3touch/spike/SaveFiles.java \
  android/app/src/main/java/org/civ3touch/spike/GameSaves.java tests/saves/AndroidSaveSlotsTest.java
"$ANDROID_HOME/build-tools/35.0.0/d8" --lib "$ANDROID_HOME/platforms/android-36/android.jar" \
  --output "$output" "$output"/org/civ3touch/spike/*.class
adb -s "$serial" push "$output/classes.dex" /data/local/tmp/civ3touch-save-tests.dex
adb -s "$serial" shell 'CLASSPATH=/data/local/tmp/civ3touch-save-tests.dex app_process /system/bin org.civ3touch.spike.AndroidSaveSlotsTest /data/local/tmp'
