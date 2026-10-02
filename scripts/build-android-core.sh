#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")/.."
: "${ANDROID_NDK_HOME:?Set ANDROID_NDK_HOME to NDK 27.2.12479018}"
case "$(uname -s)" in
  Darwin) host=darwin-x86_64 ;;
  Linux) host=linux-x86_64 ;;
  *) echo 'This build script supports macOS and Linux hosts.' >&2; exit 1 ;;
esac
bin="$ANDROID_NDK_HOME/toolchains/llvm/prebuilt/$host/bin"
export CARGO_TARGET_AARCH64_LINUX_ANDROID_LINKER="$bin/aarch64-linux-android26-clang"
export CC_aarch64_linux_android="$CARGO_TARGET_AARCH64_LINUX_ANDROID_LINKER"
export AR_aarch64_linux_android="$bin/llvm-ar"
# Support Android devices using 16 KB pages.
export CARGO_TARGET_AARCH64_LINUX_ANDROID_RUSTFLAGS='-C link-arg=-Wl,-z,max-page-size=16384'
cargo build --locked --release --lib --target aarch64-linux-android -p civ3touch-core
mkdir -p android/app/build/generated/jniLibs/arm64-v8a
cp "${CARGO_TARGET_DIR:-target}/aarch64-linux-android/release/libciv3touch_core.so" android/app/build/generated/jniLibs/arm64-v8a/
