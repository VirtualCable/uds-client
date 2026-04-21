#!/bin/bash
set -e

# Detect architecture
ARCH=$(uname -m)
if [ "$ARCH" == "x86_64" ]; then
    APPIMAGE_ARCH="x86_64"
    APT_ARCH="amd64"
elif [ "$ARCH" == "aarch64" ]; then
    APPIMAGE_ARCH="aarch64"
    APT_ARCH="arm64"
else
    echo "Unsupported architecture: $ARCH"
    exit 1
fi

echo "Building AppImage for $ARCH ($APT_ARCH)..."
export APPIMAGE_ARCH
export APT_ARCH

# Use /crate/target as working directory.
# Note: rustbuilder.py mounts [host-target/rustbuilder/AppImage] to [/crate/target]
BUILD_DIR="/crate/target"
mkdir -p "$BUILD_DIR"
cd "$BUILD_DIR"

echo "Working directory: $(pwd)"

# Use a temporary recipe to avoid modifying the original
RECIPE="$BUILD_DIR/AppImageBuilder.yml"
cp /crate/building/linux/builders/AppImage/AppImageBuilder.yml "$RECIPE"

# Replace placeholders in the recipe
export VERSION=${VERSION:-"5.0.0"}
export PNAME=${PNAME:-"udslauncher"}
sed -i "s/{{VERSION}}/${VERSION}/g" "$RECIPE"
sed -i "s/{{APT_ARCH}}/${APT_ARCH}/g" "$RECIPE"
sed -i "s/{{APPIMAGE_ARCH}}/${APPIMAGE_ARCH}/g" "$RECIPE"

# Prepare AppDir
mkdir -p AppDir/usr/bin
mkdir -p AppDir/usr/lib
mkdir -p AppDir/usr/share/icons/hicolor/scalable/apps

# Copy binary from the previously built target release
cp /crate/target/release/launcher AppDir/usr/bin/udslauncher

# Copy desktop file and icon
cp /crate/building/linux/builders/AppImage/udslauncher.desktop AppDir/
cp /crate/assets/img/uds.png AppDir/udslauncher.png
cp /crate/assets/img/uds.png AppDir/usr/share/icons/hicolor/scalable/apps/udslauncher.png

# Run appimage-builder
appimage-builder --recipe "$RECIPE" --skip-test

# Cleanup is NOT needed inside here anymore as it is in target/ and we want the results
