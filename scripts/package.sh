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

# Copy assets (icons)
if [ -d "assets" ]; then
    cp -r assets "$DIST_DIR/"
fi

# Copy any present cores
if [ -d "cores" ] && [ "$(ls -A cores 2>/dev/null)" ]; then
    cp -r cores/* "$DIST_DIR/cores/"
fi

# Create native macOS .app bundle if on Darwin
if [ "$(uname -s)" = "Darwin" ]; then
    echo "🍏 Creating macOS Application Bundle (PixelDrive.app)..."
    APP_DIR="dist/PixelDrive.app/Contents"
    mkdir -p "$APP_DIR/MacOS" "$APP_DIR/Resources" "$APP_DIR/Resources/cores" "$APP_DIR/Resources/saves"

    if [ -f "target/release/pixel-drive" ]; then
        cp target/release/pixel-drive "$APP_DIR/MacOS/"
    fi

    if [ -f "assets/icon.icns" ]; then
        cp assets/icon.icns "$APP_DIR/Resources/"
    fi

    cat << 'EOF' > "$APP_DIR/Info.plist"
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>CFBundleExecutable</key>
    <string>pixel-drive</string>
    <key>CFBundleIconFile</key>
    <string>icon</string>
    <key>CFBundleIdentifier</key>
    <string>com.pixeldrive.emulator</string>
    <key>CFBundleName</key>
    <string>PixelDrive</string>
    <key>CFBundleDisplayName</key>
    <string>PixelDrive</string>
    <key>CFBundlePackageType</key>
    <string>APPL</string>
    <key>CFBundleShortVersionString</key>
    <string>1.0.0</string>
    <key>CFBundleVersion</key>
    <string>1</string>
    <key>NSHighResolutionCapable</key>
    <true/>
</dict>
</plist>
EOF
fi

echo "🗜️ Creating release archive..."
cd dist
tar -czf PixelDrive-Release.tar.gz PixelDrive
echo "✅ Distribution package created at: dist/PixelDrive-Release.tar.gz"

