# Source the environment file only if not in a CI environment
if [ -z "${CI-}" ]; then
  SCRIPT_DIR=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" &> /dev/null && pwd)
  source "$SCRIPT_DIR/android.env"
fi
#dxedge build --platform android --release --verbose --target aarch64-linux-android
# Make sure to remove the target/dx folder when building with the script for each cli version.
dx build --platform android --release --verbose --target aarch64-linux-android || true

./scripts/android.fix-gradle.sh

APP_NAME="$(awk -F'=' '/^name[[:space:]]*=/ { gsub(/["[:space:]]/, "", $2); print tolower($2); exit }' Dioxus.toml)"
if [ -z "$APP_NAME" ]; then
  APP_NAME="mobile"
fi

cd "target/dx/${APP_NAME}/release/android/app"
./gradlew assembleDebug
