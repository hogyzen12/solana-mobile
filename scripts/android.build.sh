# Source the environment file from the same directory as the script
SCRIPT_DIR=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" &> /dev/null && pwd)
source "$SCRIPT_DIR/android.env"
dx build --platform android --release --verbose
