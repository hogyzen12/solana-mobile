
#!/usr/bin/env bash
set -euo pipefail

# ──────────────── Config ────────────────

OPENSSL_VERSION="3.0.16"
OPENSSL_TARBALL="openssl-${OPENSSL_VERSION}.tar.gz"
OPENSSL_URL="https://www.openssl.org/source/${OPENSSL_TARBALL}"
BUILD_ROOT="$HOME/openssl-${OPENSSL_VERSION}"
INSTALL_DIR_BASE="${BUILD_ROOT}/android-build"

# This script is for CI, where ANDROID_NDK_HOME is always provided.
if [ -z "${ANDROID_NDK_HOME-}" ]; then
  echo "Error: ANDROID_NDK_HOME is not set. This script is intended for CI use."
  exit 1
fi
NDK_HOME="${ANDROID_NDK_HOME}"

# Minimum Android API level
API=24

# Set up toolchain paths
HOST_TAG="linux-x86_64"
TOOLCHAIN="${NDK_HOME}/toolchains/llvm/prebuilt/${HOST_TAG}"
export PATH="${TOOLCHAIN}/bin:${PATH}"
export ANDROID_NDK_ROOT="${NDK_HOME}"

# ─────────── Download & unpack OpenSSL ───────────

mkdir -p "${BUILD_ROOT}"
cd "${BUILD_ROOT}"

if [ ! -f "${OPENSSL_TARBALL}" ]; then
  echo "Downloading OpenSSL ${OPENSSL_VERSION}..."
  wget "${OPENSSL_URL}"
fi

if [ -d "openssl-${OPENSSL_VERSION}" ]; then
  echo "Removing previous source directory..."
  rm -rf "openssl-${OPENSSL_VERSION}"
fi

echo "Extracting source..."
tar xzf "${OPENSSL_TARBALL}"
cd "openssl-${OPENSSL_VERSION}"

# ───────── Architectures to build ─────────

# Mapping from Rust target triple to OpenSSL configure target
declare -A arch_map
arch_map["aarch64-linux-android"]="android-arm64"
arch_map["x86_64-linux-android"]="android-x86_64"

# ───────── Build for each architecture ─────────

for rust_arch in "${!arch_map[@]}"; do
    openssl_arch=${arch_map[$rust_arch]}
    INSTALL_DIR_ARCH="${INSTALL_DIR_BASE}/${rust_arch}"
    
    echo "--------------------------------------------------"
    echo "Building OpenSSL for ${rust_arch} (${openssl_arch})"
    echo "--------------------------------------------------"
    
    if [ -f "Makefile" ]; then
        make clean
    fi

    echo "Configuring for ${openssl_arch} (API ${API})..."
    ./Configure "${openssl_arch}" \
      -D__ANDROID_API__=${API} \
      --prefix="${INSTALL_DIR_ARCH}" \
      --openssldir="${INSTALL_DIR_ARCH}" \
      --libdir=lib

    echo "Building (make -j)…"
    make -j"$(nproc)"

    echo "Installing to ${INSTALL_DIR_ARCH}…"
    make install_sw
done

echo "✅ OpenSSL built for all architectures."

