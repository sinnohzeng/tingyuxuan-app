#!/bin/bash
# Build tingyuxuan-jni for Android targets using cargo-ndk.
#
# Prerequisites:
#   cargo install cargo-ndk
#   rustup target add aarch64-linux-android armv7-linux-androideabi x86_64-linux-android
#   ANDROID_NDK_HOME must be set
#
# Output: target/<arch>/release/libtingyuxuan_jni.so

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"

echo "=== Building tingyuxuan-jni for Android ==="

# Build for each target architecture.
TARGETS=(
    "arm64-v8a"
    "armeabi-v7a"
    "x86_64"
)

for target in "${TARGETS[@]}"; do
    echo "--- Building for $target ---"
    cargo ndk -t "$target" build --release --manifest-path "$SCRIPT_DIR/Cargo.toml"
done

echo "=== Done. Copy .so files to android/app/src/main/jniLibs/ ==="
echo "  arm64-v8a:    target/aarch64-linux-android/release/libtingyuxuan_jni.so"
echo "  armeabi-v7a:  target/armv7-linux-androideabi/release/libtingyuxuan_jni.so"
echo "  x86_64:       target/x86_64-linux-android/release/libtingyuxuan_jni.so"
