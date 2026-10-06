#!/bin/sh
set -eu

ROOT=$(CDPATH= cd -- "$(dirname -- "$0")/../.." && pwd)
OUTPUT="$ROOT/building/android/output"
IMAGE=udslauncher-android

echo "==> Building native Rust UDS Android library (libuds_android.so)..."
docker run --rm \
  -v "$HOME/.cargo:/root/.cargo:ro" \
  -v "$HOME/.rustup:/root/.rustup:ro" \
  -v "$ROOT:/src" \
  -w /src \
  -e PATH=/opt/android-sdk/ndk/29.0.13113456/toolchains/llvm/prebuilt/linux-x86_64/bin:/root/.cargo/bin:/usr/local/sbin:/usr/local/bin:/usr/sbin:/usr/bin:/sbin:/bin \
  -e CC_aarch64_linux_android=/opt/android-sdk/ndk/29.0.13113456/toolchains/llvm/prebuilt/linux-x86_64/bin/aarch64-linux-android29-clang \
  -e CARGO_TARGET_AARCH64_LINUX_ANDROID_LINKER=/opt/android-sdk/ndk/29.0.13113456/toolchains/llvm/prebuilt/linux-x86_64/bin/aarch64-linux-android29-clang \
  -e AR_aarch64_linux_android=/opt/android-sdk/ndk/29.0.13113456/toolchains/llvm/prebuilt/linux-x86_64/bin/llvm-ar \
  "$IMAGE" cargo build --release --features insecure-tls --target aarch64-linux-android -p uds-android

mkdir -p "$ROOT/building/android/uds-app/jniLibs/arm64-v8a"
cp "$ROOT/target/aarch64-linux-android/release/libuds_android.so" "$ROOT/building/android/uds-app/jniLibs/arm64-v8a/"

mkdir -p "$OUTPUT"
echo "==> Building FreeRDP Android APK with UDS launcher..."
docker build -t "$IMAGE" "$ROOT/building/android"
docker run --rm --user "$(id -u):$(id -g)" -v "$OUTPUT:/output" "$IMAGE" \
    cp /opt/freerdp/client/Android/Studio/aFreeRDP/build/outputs/apk/debug/aFreeRDP-debug.apk /output/uds-launcher-android-debug.apk

cp "$OUTPUT/uds-launcher-android-debug.apk" "$HOME/Desktop/uds-launcher-android-debug.apk"
echo "APK generated at: $OUTPUT/uds-launcher-android-debug.apk and copied to Desktop"
