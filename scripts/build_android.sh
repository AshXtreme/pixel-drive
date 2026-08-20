#!/usr/bin/env bash
set -e

# ==============================================================================
# PixelDrive Android Build & Packaging Script
# ==============================================================================

PROJECT_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$PROJECT_ROOT"

echo "============================================================"
echo "🕹️  PixelDrive v1.2 — Android Build & Verification Harness"
echo "============================================================"

# 1. Parse Arguments (Release vs Debug)
BUILD_MODE="release"
CARGO_FLAGS="--release"

for arg in "$@"; do
    case "$arg" in
        --debug)
            BUILD_MODE="debug"
            CARGO_FLAGS=""
            ;;
        --release)
            BUILD_MODE="release"
            CARGO_FLAGS="--release"
            ;;
        -h|--help)
            echo "Usage: ./scripts/build_android.sh [--release | --debug]"
            exit 0
            ;;
    esac
done

echo "Build Target Profile: ${BUILD_MODE}"

# 2. Verify rustup target aarch64-linux-android
if ! rustup target list | grep -q "aarch64-linux-android (installed)"; then
    echo "⚙️  Adding rustup target aarch64-linux-android..."
    rustup target add aarch64-linux-android
fi

# 3. Detect Android NDK Home if not explicitly set
if [ -z "$ANDROID_NDK_HOME" ] && [ -z "$NDK_HOME" ]; then
    # Search common macOS and Linux SDK/NDK install locations
    POSSIBLE_NDK_PATHS=(
        "$HOME/Library/Android/sdk/ndk"/*
        "$HOME/Android/Sdk/ndk"/*
        "/opt/android-sdk/ndk"/*
        "/usr/local/share/android-sdk/ndk"/*
    )
    for p in "${POSSIBLE_NDK_PATHS[@]}"; do
        if [ -d "$p" ]; then
            export ANDROID_NDK_HOME="$p"
            export NDK_HOME="$p"
            echo "🔍 Auto-detected Android NDK: $ANDROID_NDK_HOME"
            break
        fi
    done
fi

# 4. Verify cargo-ndk installation
if ! command -v cargo-ndk &> /dev/null; then
    echo "⚠️  cargo-ndk is not installed on this system."
    echo "👉 To install, run: cargo install cargo-ndk"
    echo ""
    echo "If you have the Android NDK installed, you can build manually via:"
    echo "   cargo ndk -t arm64-v8a -o android/app/src/main/jniLibs build --lib ${CARGO_FLAGS}"
    exit 1
fi

# 5. Execute cargo-ndk build for arm64-v8a (aarch64-linux-android)
echo "🔨 Compiling PixelDrive cdylib for aarch64-linux-android (arm64-v8a)..."
mkdir -p android/app/src/main/jniLibs/arm64-v8a

cargo ndk -t arm64-v8a -o android/app/src/main/jniLibs build --lib ${CARGO_FLAGS}

# 6. Verify generated shared library (.so)
SO_PATH="android/app/src/main/jniLibs/arm64-v8a/libpixeldrive.so"

if [ -f "$SO_PATH" ]; then
    SO_SIZE=$(du -h "$SO_PATH" | cut -f1)
    echo "✅ Successfully built: $SO_PATH (${SO_SIZE})"
    echo ""
    echo "📦 Next Steps for APK Packaging:"
    echo "   1. Open the './android' folder in Android Studio"
    echo "   2. Or build APK from command line with: cd android && ./gradlew assembleDebug"
    echo "============================================================"
else
    echo "❌ Build completed, but $SO_PATH was not found."
    exit 1
fi
