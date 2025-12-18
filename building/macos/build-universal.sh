#!/bin/zsh
set -e

# Clean previous outputs
if [[ -d "package" ]]; then
    echo "Removing previous package directory..."
    rm -rf package
fi

if [[ -d "build-root" ]]; then
    echo "Removing previous build-root directory..."
    rm -rf build-root
fi

for pkg in UDSActor-*.pkg(N); do
    echo "Removing previous package file $pkg..."
    rm -f "$pkg"
done

SCRIPT_PATH="${(%):-%N}"
SCRIPT_DIR="$(cd "$(dirname "$SCRIPT_PATH")" && pwd)"
WORKSPACE_ROOT="$(cd "$SCRIPT_DIR/../.." && pwd)"
OUTPUT_DIR="$SCRIPT_DIR/package"
BUILD_ROOT="$SCRIPT_DIR/build-root"

VERSION_FILE="$WORKSPACE_ROOT/../VERSION"
VERSION="DEVEL"
[[ -f "$VERSION_FILE" ]] && VERSION="$(cat "$VERSION_FILE")"

echo "Building universal macOS binaries..."
echo "Workspace root: $WORKSPACE_ROOT"
echo "Version: $VERSION"

echo "Building for x86_64-apple-darwin..."
cargo build --release --target x86_64-apple-darwin

echo "Building for aarch64-apple-darwin..."
cargo build --release --target aarch64-apple-darwin

mkdir -p "$OUTPUT_DIR"

echo "Creating universal binaries..."
for BINARY in udsactor-client udsactor-service udsactor-unmanaged-config gui-helper; do
    lipo -create \
    "$WORKSPACE_ROOT/target/x86_64-apple-darwin/release/$BINARY" \
    "$WORKSPACE_ROOT/target/aarch64-apple-darwin/release/$BINARY" \
    -output "$OUTPUT_DIR/$BINARY"
    
    if [[ -n "$UDSACTOR_PROCESS_BINARY" ]]; then
        echo "Processing $BINARY with external hook..."
        "$UDSACTOR_PROCESS_BINARY" "$OUTPUT_DIR/$BINARY"
    else
        echo "No binary processing hook defined for $BINARY"
    fi
    
    echo "Created universal binary for $BINARY at $OUTPUT_DIR/$BINARY"
done

mv "$OUTPUT_DIR/udsactor-unmanaged-config" "$OUTPUT_DIR/udsactor-config"

echo "Preparing build-root structure..."
rm -rf "$BUILD_ROOT"
mkdir -p "$BUILD_ROOT/usr/local/bin"
mkdir -p "$BUILD_ROOT/usr/local/share/doc/udsactor"
mkdir -p "$BUILD_ROOT/Library/LaunchAgents"
mkdir -p "$BUILD_ROOT/Library/LaunchDaemons"
mkdir -p "$BUILD_ROOT/scripts"

echo "Copying binaries..."
cp "$OUTPUT_DIR/"* "$BUILD_ROOT/usr/local/bin/"

echo "Copying plist files..."
cp "$SCRIPT_DIR/plist/org.openuds.udsactor-client.plist" "$BUILD_ROOT/Library/LaunchAgents/"
cp "$SCRIPT_DIR/plist/org.openuds.udsactor-service.plist" "$BUILD_ROOT/Library/LaunchDaemons/"

echo "Copying uninstall script..."
cp "$SCRIPT_DIR/scripts/udsactor-uninstall.sh" "$BUILD_ROOT/usr/local/bin/udsactor-uninstall"
chmod +x "$BUILD_ROOT/usr/local/bin/udsactor-uninstall"

echo "Copying postinstall script..."
cp "$SCRIPT_DIR/scripts/postinstall.sh" "$BUILD_ROOT/scripts/postinstall"
chmod +x "$BUILD_ROOT/scripts/postinstall"

echo "Copying README.txt..."
cp "$SCRIPT_DIR/README.txt" "$BUILD_ROOT/usr/local/share/doc/udsactor/README.txt"
echo "Copying license.txt..."
cp "$SCRIPT_DIR/license.txt" "$BUILD_ROOT/usr/local/share/doc/udsactor/license.txt"

echo "Building .pkg..."
pkgname="UDSActor-${VERSION}.pkg"

productbuild \
  --root "$BUILD_ROOT/usr/local" /usr/local \
  --root "$BUILD_ROOT/Library" /Library \
  --scripts "$BUILD_ROOT/scripts" \
  "$pkgname"

if [[ -n "$UDSACTOR_PROCESS_PKG" ]]; then
    echo "Processing package with external hook..."
    "$UDSACTOR_PROCESS_PKG" "$pkgname"
else
    echo "No package processing hook defined."
fi

# Clean up
rm -rf "$BUILD_ROOT"

# Keep the output for reference/debugging

echo "Done. Package created: $pkgname"
