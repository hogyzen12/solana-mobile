#!/bin/bash
set -euo pipefail

ROOT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" &> /dev/null && pwd)/.."
APP_NAME="$(awk -F'=' '/^name[[:space:]]*=/ { gsub(/["[:space:]]/, "", $2); print tolower($2); exit }' "$ROOT_DIR/Dioxus.toml")"
if [ -z "$APP_NAME" ]; then
  APP_NAME="mobile"
fi
APP_DIR="$ROOT_DIR/target/dx/${APP_NAME}/release/android/app"
GRADLE_FILE="$APP_DIR/app/build.gradle.kts"
MAIN_ACTIVITY="$APP_DIR/app/src/main/kotlin/dev/dioxus/main/MainActivity.kt"
LOGGER="$APP_DIR/app/src/main/kotlin/dev/dioxus/main/Logger.kt"

if [ ! -f "$GRADLE_FILE" ]; then
  echo "Missing $GRADLE_FILE. Run dx build first."
  exit 1
fi

PKG="$(awk -F'\"' '
  /applicationId/ { print $2; found=1; exit }
  /namespace/ { ns=$2 }
  END { if (!found && length(ns) > 0) print ns }
' "$GRADLE_FILE")"
if [ -z "$PKG" ]; then
  echo "Could not determine applicationId or namespace in $GRADLE_FILE"
  exit 1
fi

if [ -f "$MAIN_ACTIVITY" ]; then
  perl -0pi -e "s@^\\Q${PKG}\\E\\.BuildConfig;?\\n@@mg" "$MAIN_ACTIVITY"
  perl -0pi -e "s@^import\\s+[^\\n]*BuildConfig;?\\s*\\n@@mg" "$MAIN_ACTIVITY"
  perl -0pi -e "s@^import\\s+\\Q${PKG}\\E\\s*\\n@@mg" "$MAIN_ACTIVITY"
  if ! rg -q "^import .*BuildConfig" "$MAIN_ACTIVITY"; then
    perl -0pi -e "s@^package\\s+dev\\.dioxus\\.main\\s*\\n@package dev.dioxus.main\\n\\nimport ${PKG}.BuildConfig\\n@m" "$MAIN_ACTIVITY"
  fi
  perl -0pi -e "s@typealias\\s+BuildConfig\\s*=.*@typealias BuildConfig = ${PKG}.BuildConfig@" "$MAIN_ACTIVITY"
fi

if [ -f "$LOGGER" ]; then
  perl -0pi -e "s@^\\Q${PKG}\\E\\.BuildConfig;?\\n@@mg" "$LOGGER"
  perl -0pi -e "s@^import\\s+[^\\n]*BuildConfig;?\\s*\\n@@mg" "$LOGGER"
  perl -0pi -e "s@^import\\s+\\Q${PKG}\\E\\s*\\n@@mg" "$LOGGER"
  if ! rg -q "^import .*BuildConfig" "$LOGGER"; then
    perl -0pi -e "s@^package\\s+dev\\.dioxus\\.main\\s*\\n@package dev.dioxus.main\\n\\nimport ${PKG}.BuildConfig\\n@m" "$LOGGER"
  fi
fi

echo "Patched BuildConfig imports for package: $PKG"
