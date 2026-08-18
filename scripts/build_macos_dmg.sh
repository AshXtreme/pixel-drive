#!/usr/bin/env bash
set -e

echo "🔨 Building PixelDrive in Release mode..."
cargo build --release

APP_NAME="PixelDrive"
DMG_DIR="dist/dmg_staging"
APP_BUNDLE="$DMG_DIR/$APP_NAME.app"
CONTENTS="$APP_BUNDLE/Contents"
MACOS="$CONTENTS/MacOS"
RESOURCES="$CONTENTS/Resources"

echo "🧹 Cleaning previous staging & disk image files..."
rm -rf dist/dmg_staging "dist/$APP_NAME-1.0.0.dmg"
mkdir -p "$MACOS" "$RESOURCES/cores" "$RESOURCES/saves"

echo "📦 Assembling $APP_NAME.app bundle..."
if [ -f "target/release/pixel-drive" ]; then
    cp target/release/pixel-drive "$MACOS/pixel-drive"
    chmod +x "$MACOS/pixel-drive"
    # Also provide alias pixeldrive if referenced
    cp target/release/pixel-drive "$MACOS/pixeldrive"
    chmod +x "$MACOS/pixeldrive"
elif [ -f "target/release/pixeldrive" ]; then
    cp target/release/pixeldrive "$MACOS/pixeldrive"
    chmod +x "$MACOS/pixeldrive"
    cp target/release/pixeldrive "$MACOS/pixel-drive"
    chmod +x "$MACOS/pixel-drive"
fi

if [ -f "assets/macos/Info.plist" ]; then
    cp assets/macos/Info.plist "$CONTENTS/"
fi

if [ -f "assets/macos/AppIcon.icns" ]; then
    cp assets/macos/AppIcon.icns "$RESOURCES/"
fi

# Copy any present dynamic cores into bundle Resources
if [ -d "cores" ] && [ "$(ls -A cores 2>/dev/null)" ]; then
    cp -r cores/* "$RESOURCES/cores/"
fi

echo "🔗 Creating Applications symlink for drag-and-drop installer..."
ln -s /Applications "$DMG_DIR/Applications"

echo "💿 Creating .dmg installer with hdiutil..."
mkdir -p dist
hdiutil create -volname "$APP_NAME" \
               -srcfolder "$DMG_DIR" \
               -ov \
               -format UDZO \
               "dist/$APP_NAME-1.0.0.dmg"

echo "🧹 Cleaning up temporary staging files..."
rm -rf "$DMG_DIR"

echo "✅ Success! macOS DMG created at: dist/$APP_NAME-1.0.0.dmg"
