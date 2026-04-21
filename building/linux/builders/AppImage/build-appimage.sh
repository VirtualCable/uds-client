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

# Ensure we are in the crate root
cd /crate
echo "Current directory: $(pwd)"
ls -la # Debug: show contents of /crate

# Use a temporary recipe to avoid modifying the original
RECIPE="/tmp/AppImageBuilder.yml"
cp building/linux/builders/AppImage/AppImageBuilder.yml "$RECIPE"

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

# Copy binary
cp target/release/launcher AppDir/usr/bin/udslauncher

# Copy desktop file and icon
cp /crate/building/linux/builders/AppImage/udslauncher.desktop AppDir/
cp /crate/assets/img/uds.png AppDir/udslauncher.png
cp /crate/assets/img/uds.png AppDir/usr/share/icons/hicolor/scalable/apps/udslauncher.png

# Run appimage-builder
appimage-builder --recipe "$RECIPE" --skip-test

# Cleanup temporary build files
echo "Cleaning up..."
rm -rf AppDir appimage-build
