# Source the environment file only if not in a CI environment
if [ -z "${CI-}" ]; then
  SCRIPT_DIR=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" &> /dev/null && pwd)
  source "$SCRIPT_DIR/android.env"
fi
dx build --platform android --release --verbose
