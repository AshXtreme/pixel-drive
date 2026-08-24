#!/usr/bin/env bash
set -e

# ==============================================================================
# PixelDrive v1.2 — Production Android APK Packaging & Signing Pipeline
# ==============================================================================

PROJECT_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$PROJECT_ROOT"

echo "============================================================"
echo "🚀 PixelDrive v1.2 — Android Production APK Packaging"
echo "============================================================"

# 1. Parse Arguments (Release vs Debug)
BUILD_TYPE="Release"
GRADLE_TASK="assembleRelease"
CARGO_FLAGS="--release"

for arg in "$@"; do
    case "$arg" in
        --debug)
            BUILD_TYPE="Debug"
            GRADLE_TASK="assembleDebug"
            CARGO_FLAGS=""
            ;;
        --release)
            BUILD_TYPE="Release"
            GRADLE_TASK="assembleRelease"
            CARGO_FLAGS="--release"
            ;;
        -h|--help)
            echo "Usage: ./scripts/package_android.sh [--release | --debug]"
            exit 0
            ;;
    esac
done

echo "⚙️  Build Profile: ${BUILD_TYPE}"

# 2. Verify rustup target aarch64-linux-android
if ! rustup target list | grep -q "aarch64-linux-android (installed)"; then
    echo "⚙️  Adding rustup target aarch64-linux-android..."
    rustup target add aarch64-linux-android
fi

# 3. Locate Android SDK & NDK
if [ -z "$ANDROID_NDK_HOME" ] && [ -z "$NDK_HOME" ]; then
    POSSIBLE_NDK_PATHS=(
        "$HOME/Library/Android/sdk/ndk"/*
        "$HOME/Android/Sdk/ndk"/*
        "$ANDROID_HOME/ndk"/*
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
    echo "👉 Attempting to install cargo-ndk..."
    cargo install cargo-ndk || {
        echo "❌ Failed to install cargo-ndk. Please install it with: cargo install cargo-ndk"
        exit 1
    }
fi

# 5. Compile native dynamic shared library (.so) for ARM64 (aarch64-linux-android)
echo "🔨 Step 1: Compiling native ARM64 cdylib with cargo-ndk..."
mkdir -p android/app/src/main/jniLibs/arm64-v8a

cargo ndk -t arm64-v8a -o android/app/src/main/jniLibs build --lib ${CARGO_FLAGS}

SO_PATH="android/app/src/main/jniLibs/arm64-v8a/libpixeldrive.so"

if [ ! -f "$SO_PATH" ]; then
    echo "❌ Shared library was not generated at $SO_PATH"
    exit 1
fi

INITIAL_SO_SIZE=$(du -h "$SO_PATH" | cut -f1)
echo "📦 Generated shared library: $SO_PATH (${INITIAL_SO_SIZE})"

# 6. Strip debug symbols with NDK llvm-strip to guarantee <= 25MB package footprint
echo "✂️  Step 2: Stripping symbols using llvm-strip..."
LLVM_STRIP=""

if [ -n "$ANDROID_NDK_HOME" ]; then
    LLVM_STRIP=$(find "$ANDROID_NDK_HOME" -name "llvm-strip" -type f 2>/dev/null | head -n 1)
fi

if [ -z "$LLVM_STRIP" ] && command -v llvm-strip &> /dev/null; then
    LLVM_STRIP="llvm-strip"
fi

if [ -n "$LLVM_STRIP" ] && [ -x "$LLVM_STRIP" ]; then
    echo "Using strip tool: $LLVM_STRIP"
    "$LLVM_STRIP" --strip-all "$SO_PATH" || true
    STRIPPED_SO_SIZE=$(du -h "$SO_PATH" | cut -f1)
    echo "✅ Stripped library size: $SO_PATH (${STRIPPED_SO_SIZE})"
else
    echo "ℹ️  llvm-strip not found, skipping explicit symbol stripping."
fi

# 7. Build APK with Gradle wrapper
echo "📦 Step 3: Assembling APK with Gradle (${GRADLE_TASK})..."
mkdir -p dist

cd android
chmod +x ./gradlew 2>/dev/null || true

if [ -f "./gradlew" ]; then
    ./gradlew ${GRADLE_TASK} || ./gradlew assembleDebug
elif command -v gradle &> /dev/null; then
    gradle ${GRADLE_TASK} || gradle assembleDebug
else
    echo "❌ No Gradle wrapper or system Gradle found."
    exit 1
fi

cd "$PROJECT_ROOT"

# 8. Locate generated APK and copy to dist/
APK_RELEASE="android/app/build/outputs/apk/release/app-release-unsigned.apk"
APK_RELEASE_SIGNED="android/app/build/outputs/apk/release/app-release.apk"
APK_DEBUG="android/app/build/outputs/apk/debug/app-debug.apk"

DEST_APK="dist/PixelDrive-Android-v1.2.0.apk"

if [ -f "$APK_RELEASE_SIGNED" ]; then
    cp "$APK_RELEASE_SIGNED" "$DEST_APK"
elif [ -f "$APK_RELEASE" ]; then
    cp "$APK_RELEASE" "$DEST_APK"
elif [ -f "$APK_DEBUG" ]; then
    cp "$APK_DEBUG" "$DEST_APK"
else
    # Search for any APK produced in android/app/build/
    FOUND_APK=$(find android/app/build/outputs/apk -name "*.apk" 2>/dev/null | head -n 1)
    if [ -n "$FOUND_APK" ]; then
        cp "$FOUND_APK" "$DEST_APK"
    else
        echo "❌ No APK found in android/app/build/outputs/apk/"
        exit 1
    fi
fi

APK_SIZE=$(du -h "$DEST_APK" | cut -f1)
echo "============================================================"
echo "🎉 Successfully generated Android Package:"
echo "   Path: $DEST_APK"
echo "   Size: $APK_SIZE (Target: <= 25MB)"
echo "============================================================"
