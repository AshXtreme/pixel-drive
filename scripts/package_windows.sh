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
echo "🔨 Packaging Windows x86_64 Release ($TAG)..."
echo "============================================================"
DIST_DIR="dist/PixelDrive-Windows"
rm -rf "$DIST_DIR" "dist/PixelDrive-Windows-$TAG.zip" "dist/PixelDrive-Windows-x86_64.zip"
mkdir -p "$DIST_DIR/cores" "$DIST_DIR/saves" "$DIST_DIR/assets"

# Detect and copy Windows executable
if [ -f "target/x86_64-pc-windows-msvc/release/pixel-drive.exe" ]; then
    cp target/x86_64-pc-windows-msvc/release/pixel-drive.exe "$DIST_DIR/PixelDrive.exe"
elif [ -f "target/x86_64-pc-windows-msvc/release/pixeldrive.exe" ]; then
    cp target/x86_64-pc-windows-msvc/release/pixeldrive.exe "$DIST_DIR/PixelDrive.exe"
elif [ -f "target/x86_64-pc-windows-gnu/release/pixel-drive.exe" ]; then
    cp target/x86_64-pc-windows-gnu/release/pixel-drive.exe "$DIST_DIR/PixelDrive.exe"
elif [ -f "target/x86_64-pc-windows-gnu/release/pixeldrive.exe" ]; then
    cp target/x86_64-pc-windows-gnu/release/pixeldrive.exe "$DIST_DIR/PixelDrive.exe"
elif [ -f "target/release/pixel-drive.exe" ]; then
    cp target/release/pixel-drive.exe "$DIST_DIR/PixelDrive.exe"
elif [ -f "target/release/pixeldrive.exe" ]; then
    cp target/release/pixeldrive.exe "$DIST_DIR/PixelDrive.exe"
fi

cp README.md "$DIST_DIR/"
[ -f LICENSE ] && cp LICENSE "$DIST_DIR/"
[ -f LEGAL.md ] && cp LEGAL.md "$DIST_DIR/"
[ -f assets/windows/icon.ico ] && cp assets/windows/icon.ico "$DIST_DIR/assets/"

# Copy dynamic cores if present
if [ -d "cores" ] && [ "$(ls -A cores 2>/dev/null)" ]; then
    cp -r cores/* "$DIST_DIR/cores/"
fi

cd dist
zip -r "PixelDrive-Windows-$TAG.zip" PixelDrive-Windows
cp "PixelDrive-Windows-$TAG.zip" PixelDrive-Windows-x86_64.zip
echo "============================================================"
echo "✅ Windows bundle created: dist/PixelDrive-Windows-$TAG.zip"
echo "============================================================"
