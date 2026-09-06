#!/usr/bin/env bash
set -e

TAG="v1.3"
while [[ $# -gt 0 ]]; do
  case "$1" in
    --tag)
      TAG="$2"
      shift 2
      ;;
    *)
      if [[ "$1" =~ ^v[0-9] ]]; then
        TAG="$1"
      fi
      shift
      ;;
  esac
done

if [ -n "$TAG_NAME" ]; then
  TAG="$TAG_NAME"
fi

echo "============================================================"
echo "🍏 PixelDrive $TAG — macOS Application & DMG Package Assembly"
echo "============================================================"

echo "🔨 Building PixelDrive in Release mode..."
cargo build --release

APP_NAME="PixelDrive"
DMG_DIR="dist/dmg_staging"
APP_BUNDLE="$DMG_DIR/$APP_NAME.app"
CONTENTS="$APP_BUNDLE/Contents"
MACOS="$CONTENTS/MacOS"
RESOURCES="$CONTENTS/Resources"

echo "🧹 Cleaning previous staging & disk image files..."
rm -rf "$DMG_DIR" "dist/$APP_NAME-$TAG.dmg" "dist/$APP_NAME-macOS-$TAG.dmg"
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
               "dist/$APP_NAME-$TAG.dmg"

# Also create PixelDrive-macOS-vX.X.dmg alias copy
cp "dist/$APP_NAME-$TAG.dmg" "dist/$APP_NAME-macOS-$TAG.dmg"

echo "🧹 Cleaning up temporary staging files..."
rm -rf "$DMG_DIR"

echo "============================================================"
echo "✅ Success! macOS DMG created at:"
echo "   - dist/$APP_NAME-$TAG.dmg"
echo "   - dist/$APP_NAME-macOS-$TAG.dmg"
echo "============================================================"
