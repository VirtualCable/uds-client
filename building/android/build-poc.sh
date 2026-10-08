#!/bin/sh
set -eu

ROOT=$(CDPATH= cd -- "$(dirname -- "$0")/../.." && pwd)
OUTPUT="$ROOT/building/android/output"
IMAGE=udslauncher-android

CARGO_FEATURES_ARGS=""
if [ -n "${UDS_CARGO_FEATURES:-}" ]; then
    CARGO_FEATURES_ARGS="--features $UDS_CARGO_FEATURES"
fi

NDK=/opt/android-sdk/ndk/29.0.13113456
NDK_BIN="$NDK/toolchains/llvm/prebuilt/linux-x86_64/bin"

build_abi() {
    abi="$1"
    target="$2"
    linker_name="$3"
    target_upper=$(echo "$target" | tr '[:lower:]' '[:upper:]' | tr '-' '_')
    echo "==> Building libuds_android.so for $abi ($target)..."
    # Mount rustup RW so that rustup target add can install missing std targets
    # for armeabi-v7a and x86_64. NDK 29 only ships llvm-ar (not a per-target
    # <triple>-ar), so set TARGET_AR + AR_<target_upper> to the absolute path of
    # llvm-ar so cc-rs and aws-lc-sys both find it. CXX mirrors CC (clang++).
    docker run --rm \
        -v "$HOME/.cargo:/root/.cargo:ro" \
        -v "$HOME/.rustup:/root/.rustup" \
        -v "$ROOT:/src" \
        -w /src \
        -e PATH="/root/.cargo/bin:/usr/local/sbin:/usr/local/bin:/usr/sbin:/usr/bin:/sbin:/bin" \
        -e CC_"$target_upper"="$NDK_BIN/$linker_name" \
        -e CXX_"$target_upper"="$NDK_BIN/${linker_name%++}++" \
        -e AR_"$target_upper"="$NDK_BIN/llvm-ar" \
        -e TARGET_CC="$NDK_BIN/$linker_name" \
        -e TARGET_CXX="$NDK_BIN/${linker_name%++}++" \
        -e TARGET_AR="$NDK_BIN/llvm-ar" \
        "$IMAGE" sh -c "rustup target add $target 2>&1 | tail -1; cargo build --release --target $target -p uds-android $CARGO_FEATURES_ARGS"

    mkdir -p "$ROOT/building/android/uds-app/jniLibs/$abi"
    cp "$ROOT/target/$target/release/libuds_android.so" "$ROOT/building/android/uds-app/jniLibs/$abi/"
}

build_abi arm64-v8a   aarch64-linux-android    aarch64-linux-android29-clang
build_abi armeabi-v7a arm-linux-androideabi     armv7a-linux-androideabi29-clang
build_abi x86_64       x86_64-linux-android       x86_64-linux-android29-clang

mkdir -p "$OUTPUT"
echo "==> Building FreeRDP Android APK with UDS launcher..."
docker build -t "$IMAGE" "$ROOT/building/android"
docker run --rm --user "$(id -u):$(id -g)" -v "$OUTPUT:/output" "$IMAGE" \
    cp /opt/freerdp/client/Android/Studio/aFreeRDP/build/outputs/apk/debug/aFreeRDP-debug.apk /output/uds-launcher-android-debug.apk

cp "$OUTPUT/uds-launcher-android-debug.apk" "$HOME/Desktop/uds-launcher-android-debug.apk"
echo "APK generated at: $OUTPUT/uds-launcher-android-debug.apk and copied to Desktop"