#!/usr/bin/env bash
set -e

echo "🔨 Building PixelDrive in Release mode..."
cargo build --release

DIST_DIR="dist/PixelDrive"
rm -rf dist
mkdir -p "$DIST_DIR/cores" "$DIST_DIR/saves"

echo "📦 Copying binary and assets..."
if [ -f "target/release/pixel-drive" ]; then
    cp target/release/pixel-drive "$DIST_DIR/"
elif [ -f "target/release/pixeldrive" ]; then
    cp target/release/pixeldrive "$DIST_DIR/"
elif [ -f "target/release/pixel-drive.exe" ]; then
    cp target/release/pixel-drive.exe "$DIST_DIR/"
elif [ -f "target/release/pixeldrive.exe" ]; then
    cp target/release/pixeldrive.exe "$DIST_DIR/"
fi

cp README.md "$DIST_DIR/"
[ -f LICENSE ] && cp LICENSE "$DIST_DIR/"

# Copy any present cores
if [ -d "cores" ] && [ "$(ls -A cores 2>/dev/null)" ]; then
    cp -r cores/* "$DIST_DIR/cores/"
fi

echo "🗜️ Creating release archive..."
cd dist
tar -czf PixelDrive-Release.tar.gz PixelDrive
echo "✅ Distribution package created at: dist/PixelDrive-Release.tar.gz"
