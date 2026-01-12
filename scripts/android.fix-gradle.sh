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
LEDGER_KT="$APP_DIR/app/src/main/kotlin/dev/dioxus/main/LedgerUsb.kt"
MANIFEST_FILE="$APP_DIR/app/src/main/AndroidManifest.xml"

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

mkdir -p "$(dirname -- "$LEDGER_KT")"
cat > "$LEDGER_KT" <<'KOT'
package dev.dioxus.main

import android.app.PendingIntent
import android.content.Context
import android.content.Intent
import android.hardware.usb.UsbConstants
import android.hardware.usb.UsbDevice
import android.hardware.usb.UsbDeviceConnection
import android.hardware.usb.UsbEndpoint
import android.hardware.usb.UsbInterface
import android.hardware.usb.UsbManager
import android.util.Base64
import android.util.Log
import androidx.activity.ComponentActivity
import java.io.ByteArrayOutputStream
import kotlin.math.min

object LedgerUsb {
    private const val TAG = "LedgerUsb"
    private const val LEDGER_VENDOR_ID = 0x2c97
    private const val USB_PERMISSION_ACTION = "dev.dioxus.main.LEDGER_USB_PERMISSION"
    private const val CHANNEL = 0x0101
    private const val TAG_APDU = 0x05
    private const val PACKET_SIZE = 64
    private const val TIMEOUT_MS = 5000
    private const val P1_CONFIRM = 0x01
    private const val P2_EXTEND = 0x01
    private const val P2_MORE = 0x02
    private const val MAX_CHUNK_SIZE = 255

    private data class ApduResult(val payload: ByteArray?, val error: String?)

    @JvmStatic
    fun listLedgerDevices(activity: ComponentActivity): Array<String> {
        val manager = activity.getSystemService(Context.USB_SERVICE) as UsbManager
        val out = ArrayList<String>()
        for (device in manager.deviceList.values) {
            if (device.vendorId == LEDGER_VENDOR_ID) {
                Log.i(TAG, "Ledger device detected: ${device.vendorId}:${device.productId} name=${device.deviceName}")
                out.add("${device.vendorId}|${device.productId}|${device.deviceName}")
            }
        }
        if (out.isEmpty()) {
            Log.i(TAG, "No Ledger devices detected")
        }
        return out.toTypedArray()
    }

    @JvmStatic
    fun getLedgerPubkey(activity: ComponentActivity): String {
        var connection: UsbDeviceConnection? = null
        var iface: UsbInterface? = null
        try {
            val manager = activity.getSystemService(Context.USB_SERVICE) as UsbManager
            val device = manager.deviceList.values.firstOrNull { it.vendorId == LEDGER_VENDOR_ID }
                ?: return "ERROR:NoLedgerDevice".also { Log.w(TAG, it) }

            if (!manager.hasPermission(device)) {
                val intent = Intent(USB_PERMISSION_ACTION)
                val flags = PendingIntent.FLAG_IMMUTABLE
                val pendingIntent = PendingIntent.getBroadcast(activity, 0, intent, flags)
                manager.requestPermission(device, pendingIntent)
                return "ERROR:USB_PERMISSION_REQUIRED".also { Log.w(TAG, it) }
            }

            connection = manager.openDevice(device) ?: return "ERROR:USB_OPEN_FAILED"
            iface = findHidInterface(device) ?: return "ERROR:NoHidInterface"
            if (!connection.claimInterface(iface, true)) {
                return "ERROR:CLAIM_INTERFACE_FAILED"
            }

            val endpoints = findEndpoints(iface)
                ?: return "ERROR:NoEndpoints".also { Log.w(TAG, it) }
            val (inEp, outEp) = endpoints

            Log.i(TAG, "Using interface=${iface.id} endpoints in=${inEp.address} out=${outEp.address}")

            val hardened = 0x80000000.toInt()
            val path = intArrayOf(44 or hardened, 501 or hardened, 0 or hardened, 0 or hardened)
            val insCandidates = listOf(0x05, 0x02)

            var lastError: String? = null
            for (ins in insCandidates) {
                val apdu = buildApdu(ins, 0x00, 0x00, serializeDerivationPath(path))
                val result = sendApdu(connection, inEp, outEp, apdu, "pubkey")
                if (result.error != null) {
                    lastError = result.error
                    continue
                }
                val payload = result.payload ?: continue
                if (payload.size < 32) {
                    Log.w(TAG, "Pubkey too short (${payload.size} bytes) for INS $ins")
                    continue
                }
                val pubkey = payload.copyOfRange(0, 32)
                return Base64.encodeToString(pubkey, Base64.NO_WRAP)
            }

            return (lastError ?: "ERROR:SW_6a83").also { Log.w(TAG, it) }
        } catch (e: Exception) {
            Log.e(TAG, "Ledger pubkey error", e)
            return "ERROR:${e.message}"
        } finally {
            try {
                if (connection != null && iface != null) {
                    connection.releaseInterface(iface)
                }
            } catch (e: Exception) {
                Log.w(TAG, "Failed to release interface", e)
            }
            connection?.close()
        }
    }

    @JvmStatic
    fun signMessage(activity: ComponentActivity, messageBase64: String): String {
        var connection: UsbDeviceConnection? = null
        var iface: UsbInterface? = null
        try {
            val manager = activity.getSystemService(Context.USB_SERVICE) as UsbManager
            val device = manager.deviceList.values.firstOrNull { it.vendorId == LEDGER_VENDOR_ID }
                ?: return "ERROR:NoLedgerDevice".also { Log.w(TAG, it) }

            if (!manager.hasPermission(device)) {
                val intent = Intent(USB_PERMISSION_ACTION)
                val flags = PendingIntent.FLAG_IMMUTABLE
                val pendingIntent = PendingIntent.getBroadcast(activity, 0, intent, flags)
                manager.requestPermission(device, pendingIntent)
                return "ERROR:USB_PERMISSION_REQUIRED".also { Log.w(TAG, it) }
            }

            val message = Base64.decode(messageBase64, Base64.DEFAULT)
            if (message.isEmpty()) {
                return "ERROR:EmptyMessage".also { Log.w(TAG, it) }
            }

            connection = manager.openDevice(device) ?: return "ERROR:USB_OPEN_FAILED"
            iface = findHidInterface(device) ?: return "ERROR:NoHidInterface"
            if (!connection.claimInterface(iface, true)) {
                return "ERROR:CLAIM_INTERFACE_FAILED"
            }

            val endpoints = findEndpoints(iface)
                ?: return "ERROR:NoEndpoints".also { Log.w(TAG, it) }
            val (inEp, outEp) = endpoints

            val hardened = 0x80000000.toInt()
            val path = intArrayOf(44 or hardened, 501 or hardened, 0 or hardened, 0 or hardened)

            val insCandidates = if (message[0].toInt() == 0xff) {
                listOf(0x07)
            } else {
                listOf(0x06, 0x03)
            }

            var lastError: String? = null
            for (ins in insCandidates) {
                val result = signWithPath(connection, inEp, outEp, ins, path, message)
                val signature = result.payload
                if (result.error != null) {
                    lastError = result.error
                }
                if (signature != null) {
                    return Base64.encodeToString(signature, Base64.NO_WRAP)
                }
            }

            return (lastError ?: "ERROR:SignFailed").also { Log.w(TAG, it) }
        } catch (e: Exception) {
            Log.e(TAG, "Ledger sign error", e)
            return "ERROR:${e.message}"
        } finally {
            try {
                if (connection != null && iface != null) {
                    connection.releaseInterface(iface)
                }
            } catch (e: Exception) {
                Log.w(TAG, "Failed to release interface", e)
            }
            connection?.close()
        }
    }

    private fun findHidInterface(device: UsbDevice): UsbInterface? {
        for (i in 0 until device.interfaceCount) {
            val iface = device.getInterface(i)
            Log.i(TAG, "Interface $i class=${iface.interfaceClass} subclass=${iface.interfaceSubclass} protocol=${iface.interfaceProtocol} endpoints=${iface.endpointCount}")
            if (iface.interfaceClass == UsbConstants.USB_CLASS_HID) {
                return iface
            }
        }
        return null
    }

    private fun findEndpoints(iface: UsbInterface): Pair<UsbEndpoint, UsbEndpoint>? {
        var inEp: UsbEndpoint? = null
        var outEp: UsbEndpoint? = null
        for (i in 0 until iface.endpointCount) {
            val ep = iface.getEndpoint(i)
            Log.i(TAG, "Endpoint $i dir=${ep.direction} type=${ep.type} addr=${ep.address} max=${ep.maxPacketSize}")
            if (ep.direction == UsbConstants.USB_DIR_IN) {
                inEp = ep
            } else if (ep.direction == UsbConstants.USB_DIR_OUT) {
                outEp = ep
            }
        }
        if (inEp == null || outEp == null) {
            return null
        }
        return Pair(inEp, outEp)
    }

    private fun serializeDerivationPath(path: IntArray): ByteArray {
        val data = ByteArray(1 + path.size * 4)
        data[0] = path.size.toByte()
        var offset = 1
        for (value in path) {
            data[offset + 0] = ((value ushr 24) and 0xff).toByte()
            data[offset + 1] = ((value ushr 16) and 0xff).toByte()
            data[offset + 2] = ((value ushr 8) and 0xff).toByte()
            data[offset + 3] = (value and 0xff).toByte()
            offset += 4
        }
        return data
    }

    private fun serializeDerivationPathMultiple(path: IntArray): ByteArray {
        val single = serializeDerivationPath(path)
        val data = ByteArray(1 + single.size)
        data[0] = 0x01
        System.arraycopy(single, 0, data, 1, single.size)
        return data
    }

    private fun buildApdu(ins: Int, p1: Int, p2: Int, payload: ByteArray): ByteArray {
        val apdu = ByteArray(5 + payload.size)
        apdu[0] = 0xE0.toByte()
        apdu[1] = ins.toByte()
        apdu[2] = p1.toByte()
        apdu[3] = p2.toByte()
        apdu[4] = payload.size.toByte()
        System.arraycopy(payload, 0, apdu, 5, payload.size)
        return apdu
    }

    private fun sendApdu(
        connection: UsbDeviceConnection,
        inEp: UsbEndpoint,
        outEp: UsbEndpoint,
        apdu: ByteArray,
        label: String
    ): ApduResult {
        val response = exchangeApdu(connection, inEp, outEp, apdu)
        if (response.size < 2) {
            Log.w(TAG, "Short response for $label")
            return ApduResult(null, "ERROR:ShortResponse")
        }
        val sw1 = response[response.size - 2].toInt() and 0xff
        val sw2 = response[response.size - 1].toInt() and 0xff
        if (sw1 != 0x90 || sw2 != 0x00) {
            val error = "ERROR:SW_${sw1.toString(16)}${sw2.toString(16)}"
            Log.w(TAG, "Ledger SW ${sw1.toString(16)}${sw2.toString(16)} for $label")
            return ApduResult(null, error)
        }
        return ApduResult(response.copyOfRange(0, response.size - 2), null)
    }

    private fun signWithPath(
        connection: UsbDeviceConnection,
        inEp: UsbEndpoint,
        outEp: UsbEndpoint,
        ins: Int,
        path: IntArray,
        message: ByteArray
    ): ApduResult {
        val usesDeprecated = ins == 0x03
        val pathPayload = if (usesDeprecated) {
            serializeDerivationPath(path)
        } else {
            serializeDerivationPathMultiple(path)
        }

        val maxSize = MAX_CHUNK_SIZE - pathPayload.size
        val firstChunkLen = min(message.size, maxSize)
        val remaining = message.copyOfRange(firstChunkLen, message.size)

        val firstPayload = if (usesDeprecated) {
            val lenBytes = byteArrayOf(
                ((message.size ushr 8) and 0xff).toByte(),
                (message.size and 0xff).toByte()
            )
            val data = ByteArray(pathPayload.size + lenBytes.size + firstChunkLen)
            System.arraycopy(pathPayload, 0, data, 0, pathPayload.size)
            System.arraycopy(lenBytes, 0, data, pathPayload.size, lenBytes.size)
            System.arraycopy(message, 0, data, pathPayload.size + lenBytes.size, firstChunkLen)
            data
        } else {
            val data = ByteArray(pathPayload.size + firstChunkLen)
            System.arraycopy(pathPayload, 0, data, 0, pathPayload.size)
            System.arraycopy(message, 0, data, pathPayload.size, firstChunkLen)
            data
        }

        var p2 = if (remaining.isEmpty()) 0 else P2_MORE
        val firstApdu = buildApdu(ins, P1_CONFIRM, p2, firstPayload)
        var response = sendApdu(connection, inEp, outEp, firstApdu, "sign chunk 0")
        if (response.payload == null) {
            return response
        }

        if (remaining.isEmpty()) {
            return response
        }

        var offset = 0
        val chunks = ArrayList<Pair<Int, ByteArray>>()
        while (offset < remaining.size) {
            val len = min(MAX_CHUNK_SIZE, remaining.size - offset)
            val chunk = remaining.copyOfRange(offset, offset + len)
            val payload = if (usesDeprecated) {
                val lenBytes = byteArrayOf(
                    ((chunk.size ushr 8) and 0xff).toByte(),
                    (chunk.size and 0xff).toByte()
                )
                val data = ByteArray(lenBytes.size + chunk.size)
                System.arraycopy(lenBytes, 0, data, 0, lenBytes.size)
                System.arraycopy(chunk, 0, data, lenBytes.size, chunk.size)
                data
            } else {
                chunk
            }
            chunks.add(P2_EXTEND or P2_MORE to payload)
            offset += len
        }

        if (chunks.isNotEmpty()) {
            val last = chunks.last()
            chunks[chunks.size - 1] = (last.first and P2_MORE.inv()) to last.second
        }

        for ((flags, payload) in chunks) {
            val apdu = buildApdu(ins, P1_CONFIRM, flags, payload)
            response = sendApdu(connection, inEp, outEp, apdu, "sign chunk")
            if (response.payload == null) {
                return response
            }
        }

        return response
    }

    private fun exchangeApdu(
        connection: UsbDeviceConnection,
        inEp: UsbEndpoint,
        outEp: UsbEndpoint,
        apdu: ByteArray
    ): ByteArray {
        val packets = wrapApdu(apdu)
        for (packet in packets) {
            val wrote = connection.bulkTransfer(outEp, packet, packet.size, TIMEOUT_MS)
            if (wrote <= 0) {
                Log.e(TAG, "USB write failed")
                throw IllegalStateException("USB write failed")
            }
        }

        val response = ByteArrayOutputStream()
        var responseLen = -1
        var seq = 0

        while (responseLen < 0 || response.size() < responseLen) {
            val buffer = ByteArray(PACKET_SIZE)
            val read = connection.bulkTransfer(inEp, buffer, buffer.size, TIMEOUT_MS)
            if (read <= 0) {
                Log.e(TAG, "USB read failed")
                throw IllegalStateException("USB read failed")
            }

            val channel = ((buffer[0].toInt() and 0xff) shl 8) or (buffer[1].toInt() and 0xff)
            val tag = buffer[2].toInt() and 0xff
            val seqNum = ((buffer[3].toInt() and 0xff) shl 8) or (buffer[4].toInt() and 0xff)
            if (channel != CHANNEL || tag != TAG_APDU || seqNum != seq) {
                continue
            }

            var offset = 5
            if (seq == 0) {
                responseLen = ((buffer[5].toInt() and 0xff) shl 8) or (buffer[6].toInt() and 0xff)
                offset = 7
            }

            val remaining = responseLen - response.size()
            val chunkLen = min(PACKET_SIZE - offset, remaining)
            if (chunkLen > 0) {
                response.write(buffer, offset, chunkLen)
            }
            seq += 1
        }

        return response.toByteArray()
    }

    private fun wrapApdu(apdu: ByteArray): List<ByteArray> {
        val packets = ArrayList<ByteArray>()
        var seq = 0
        var offset = 0
        val totalLen = apdu.size

        while (offset < totalLen) {
            val packet = ByteArray(PACKET_SIZE)
            packet[0] = ((CHANNEL ushr 8) and 0xff).toByte()
            packet[1] = (CHANNEL and 0xff).toByte()
            packet[2] = TAG_APDU.toByte()
            packet[3] = ((seq ushr 8) and 0xff).toByte()
            packet[4] = (seq and 0xff).toByte()

            var headerLen = 5
            if (seq == 0) {
                packet[5] = ((totalLen ushr 8) and 0xff).toByte()
                packet[6] = (totalLen and 0xff).toByte()
                headerLen = 7
            }

            val chunkLen = min(PACKET_SIZE - headerLen, totalLen - offset)
            System.arraycopy(apdu, offset, packet, headerLen, chunkLen)
            packets.add(packet)
            offset += chunkLen
            seq += 1
        }
        return packets
    }
}
KOT

if [ -f "$GRADLE_FILE" ]; then
  python3 - "$GRADLE_FILE" <<'PY'
import sys

path = sys.argv[1]
with open(path, "r", encoding="utf-8") as f:
    content = f.read()

old_dep = 'implementation("com.hoho.android:usb-serial-for-android:3.9.0")'
legacy_dep = 'implementation("com.github.mik3y:usb-serial-for-android:3.5.1")'
new_dep = 'implementation("com.github.mik3y:usb-serial-for-android:3.9.0")'
if old_dep in content:
    content = content.replace(old_dep, new_dep)
    content = content.replace(legacy_dep, new_dep)
    with open(path, "w", encoding="utf-8") as f:
        f.write(content)
    sys.exit(0)
if legacy_dep in content:
    content = content.replace(legacy_dep, new_dep)
    with open(path, "w", encoding="utf-8") as f:
        f.write(content)
    sys.exit(0)
if new_dep in content:
    sys.exit(0)

lines = content.splitlines()
out = []
in_deps = False
added = False
for line in lines:
    out.append(line)
    stripped = line.strip()
    if stripped.startswith("dependencies"):
        in_deps = True
    elif in_deps and stripped.startswith("}"):
        if not added:
            out.insert(len(out) - 1, f"    {new_dep}")
            added = True
        in_deps = False

if added:
    with open(path, "w", encoding="utf-8") as f:
        f.write("\n".join(out) + "\n")
PY
fi

if [ -f "$APP_DIR/settings.gradle.kts" ]; then
  python3 - "$APP_DIR/settings.gradle.kts" <<'PY'
import sys

path = sys.argv[1]
with open(path, "r", encoding="utf-8") as f:
    content = f.read()

repo_line = 'maven { url = uri("https://jitpack.io") }'
if repo_line in content:
    sys.exit(0)

lines = content.splitlines()
out = []
in_repos = False
inserted = False
for line in lines:
    out.append(line)
    stripped = line.strip()
    if stripped.startswith("repositories"):
        in_repos = True
    elif in_repos and stripped.startswith("}"):
        if not inserted:
            out.insert(len(out) - 1, f"    {repo_line}")
            inserted = True
        in_repos = False

if inserted:
    with open(path, "w", encoding="utf-8") as f:
        f.write("\n".join(out) + "\n")
PY
fi

if [ -f "$APP_DIR/settings.gradle" ]; then
  python3 - "$APP_DIR/settings.gradle" <<'PY'
import sys

path = sys.argv[1]
with open(path, "r", encoding="utf-8") as f:
    content = f.read()

repo_block = '\n'.join([
    "pluginManagement {",
    "    repositories {",
    "        google()",
    "        mavenCentral()",
    "        maven { url 'https://jitpack.io' }",
    "        gradlePluginPortal()",
    "    }",
    "}",
    "dependencyResolutionManagement {",
    "    repositoriesMode.set(RepositoriesMode.PREFER_PROJECT)",
    "    repositories {",
    "        google()",
    "        mavenCentral()",
    "        maven { url 'https://jitpack.io' }",
    "    }",
    "}",
    "",
])

if "jitpack.io" in content:
    if "RepositoriesMode.FAIL_ON_PROJECT_REPOS" in content:
        content = content.replace("RepositoriesMode.FAIL_ON_PROJECT_REPOS", "RepositoriesMode.PREFER_PROJECT")
        with open(path, "w", encoding="utf-8") as f:
            f.write(content)
    sys.exit(0)

content = repo_block + content
with open(path, "w", encoding="utf-8") as f:
    f.write(content)
PY
fi

if [ -f "$APP_DIR/build.gradle.kts" ]; then
  python3 - "$APP_DIR/build.gradle.kts" <<'PY'
import sys

path = sys.argv[1]
with open(path, "r", encoding="utf-8") as f:
    content = f.read()

lines = content.splitlines()
out = []
in_allprojects = False
in_repos = False
inserted = False
for line in lines:
    out.append(line)
    stripped = line.strip()
    if stripped.startswith("allprojects"):
        in_allprojects = True
    elif in_allprojects and stripped.startswith("repositories"):
        in_repos = True
    elif in_allprojects and in_repos and stripped.startswith("}"):
        if not inserted:
            out.insert(len(out) - 1, "        maven(url = \"https://jitpack.io\")")
            inserted = True
        in_repos = False
    elif in_allprojects and stripped.startswith("}"):
        in_allprojects = False

if inserted:
    content = "\n".join(out) + "\n"
    with open(path, "w", encoding="utf-8") as f:
        f.write(content)
PY
fi

if [ -f "$MANIFEST_FILE" ]; then
  python3 - "$MANIFEST_FILE" <<'PY'
import sys

path = sys.argv[1]
with open(path, "r", encoding="utf-8") as f:
    content = f.read()

feature = '<uses-feature android:name="android.hardware.usb.host" />'
if feature in content:
    sys.exit(0)

insert_point = content.find("<application")
if insert_point == -1:
    sys.exit(0)

updated = content[:insert_point] + feature + "\n" + content[insert_point:]
with open(path, "w", encoding="utf-8") as f:
    f.write(updated)
PY
fi

echo "Patched BuildConfig imports for package: $PKG"
