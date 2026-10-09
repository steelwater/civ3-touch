#!/usr/bin/env bash
set -euo pipefail
output=$(mktemp -d)
trap 'rm -rf "$output"' EXIT
"${JAVA_HOME:+$JAVA_HOME/bin/}javac" -d "$output" \
  android/app/src/main/java/org/civ3touch/spike/MapCamera.java tests/touch/MapCameraTest.java
"${JAVA_HOME:+$JAVA_HOME/bin/}java" -cp "$output" org.civ3touch.spike.MapCameraTest
