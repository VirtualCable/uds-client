#!/bin/bash
set -e

# Default values
PNAME=${PNAME:-launcher}
VERSION=${VERSION:-5.0.0}
TARGET_DIR=${TARGET_DIR:-/crate/target/release}

echo "Building Nix-Portable for $PNAME version $VERSION"

# Use a temporary default.nix to avoid modifying the original template
TEMP_NIX=$(mktemp)
cp /crate/building/linux/builders/nix-portable/default.nix "$TEMP_NIX"

sed -i "s/%%PNAME%%/$PNAME/g" "$TEMP_NIX"
sed -i "s/%%VERSION%%/$VERSION/g" "$TEMP_NIX"

cd /crate

# Build with nix-portable
nix-portable nix build -f "$TEMP_NIX"
nix-portable nix bundle -f "$TEMP_NIX"

# Move the bundle to the target directory
# The bundle filename usually contains the pname and version
mkdir -p "$TARGET_DIR"
mv "$PNAME"-* "$TARGET_DIR/udslauncher-portable"

# Cleanup
rm -f "$TEMP_NIX" result
