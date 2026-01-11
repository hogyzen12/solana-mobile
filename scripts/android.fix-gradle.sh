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
UTILS_KT="$APP_DIR/app/src/main/kotlin/dev/dioxus/main/DioxusUtils.kt"

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

RES_DIR="$APP_DIR/app/src/main/res"
if [ -d "$RES_DIR" ]; then
  find "$RES_DIR" -type f -path "*/mipmap-*/ic_launcher.webp" -exec rm -f {} +
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

if [ -f "$UTILS_KT" ] && ! rg -q "fun signMessage\\(" "$UTILS_KT"; then
  python3 - "$UTILS_KT" <<'PY'
import re
import sys

path = sys.argv[1]
with open(path, "r", encoding="utf-8") as f:
    content = f.read()

block = r'''
        @Keep
        @JvmStatic
        fun signMessage(activity: ComponentActivity, message: ByteArray): String {
            Log.d(TAG, "signMessage called for activity: $activity with message of size ${message.size}")

            val sender: ActivityResultSender = if (activity is MainActivity) {
                activity.mwaActivityResultSender
            } else {
                val errorMsg = "Activity passed to signMessage is not an instance of MainActivity. Cannot get mwaActivityResultSender."
                Log.e(TAG, errorMsg)
                return errorMsg
            }

            val connectionIdentity = ConnectionIdentity(
                identityName = APP_IDENTITY_NAME,
                identityUri = APP_IDENTITY_URI,
                iconUri = APP_ICON_URI
            )

            val walletAdapter = MobileWalletAdapter(connectionIdentity)

            CoroutineScope(Dispatchers.IO).launch {
                Log.d(TAG, "Attempting MWA message signing...")
                walletAdapter.blockchain = Solana.Mainnet

                val result = walletAdapter.transact(sender) { authResult ->
                    val address = authResult.accounts.firstOrNull()?.publicKey
                        ?: throw IllegalStateException("No authorized accounts for message signing")
                    signMessages(arrayOf(message), arrayOf(address))
                }

                when (result) {
                    is TransactionResult.Success -> {
                        val signedMessageBytes = result.payload.signedPayloads.firstOrNull()
                        signedMessageBytes?.let {
                            val signedMessageBase58 = Base58.encodeToString(it)
                            Log.i(TAG, "Signed message: $signedMessageBase58")
                            Ipc.sendSignedMessage(signedMessageBase58)
                        } ?: run {
                            Log.w(TAG, "Message signing successful, but no signed payload was returned.")
                        }
                    }
                    is TransactionResult.Failure -> {
                        Log.e(TAG, "MWA Message Signing Failed: ${result.message}", result.e)
                    }
                    is TransactionResult.NoWalletFound -> {
                        Log.w(TAG, "MWA Message Signing Failed: No compatible wallet found. Message: ${result.message}")
                    }
                }
            }

            val immediateReturnMessage = "MWA message signing process initiated. Check Logcat for '$TAG' for asynchronous result."
            Log.d(TAG, immediateReturnMessage)
            return immediateReturnMessage
        }
'''

updated = re.sub(r"\n\s*}\s*\n}\s*$", block + "\n    }\n}", content, flags=re.S)
if updated == content:
    sys.exit("Failed to inject signMessage block")

with open(path, "w", encoding="utf-8") as f:
    f.write(updated)
PY
  echo "Patched DioxusUtils.kt with signMessage support"
fi

if [ -f "$UTILS_KT" ]; then
  python3 - "$UTILS_KT" <<'PY'
import sys

path = sys.argv[1]
with open(path, "r", encoding="utf-8") as f:
    content = f.read()

updated = content.replace(
    "val result = walletAdapter.transact(sender) {\n                    signMessages(arrayOf(message), arrayOf<ByteArray?>(null))\n                }",
    "val result = walletAdapter.transact(sender) { authResult ->\n                    val address = authResult.accounts.firstOrNull()?.publicKey\n                        ?: throw IllegalStateException(\"No authorized accounts for message signing\")\n                    signMessages(arrayOf(message), arrayOf(address))\n                }",
)

if updated != content:
    with open(path, "w", encoding="utf-8") as f:
        f.write(updated)
PY
fi

echo "Patched BuildConfig imports for package: $PKG"
