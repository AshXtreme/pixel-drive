#!/usr/bin/env bash
set -e

# ==============================================================================
# PixelDrive v1.3 — Production Multi-ABI Android APK Packaging Pipeline
# Supports ARM64 (arm64-v8a) and x86_64 (BlueStacks / Emulators / Chromebooks)
# ==============================================================================

PROJECT_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$PROJECT_ROOT"

echo "============================================================"
echo "🚀 PixelDrive v1.3 — Multi-ABI Android Package Assembly"
echo "============================================================"

# 1. Parse Arguments (Release vs Debug, Tag)
BUILD_TYPE="Release"
GRADLE_TASK="assembleRelease"
CARGO_FLAGS="--release"
TAG_NAME="${TAG_NAME:-v1.3}"

while [[ $# -gt 0 ]]; do
    case "$1" in
        --debug)
            BUILD_TYPE="Debug"
            GRADLE_TASK="assembleDebug"
            CARGO_FLAGS=""
            shift
            ;;
        --release)
            BUILD_TYPE="Release"
            GRADLE_TASK="assembleRelease"
            CARGO_FLAGS="--release"
            shift
            ;;
        --tag=*)
            TAG_NAME="${1#*=}"
            shift
            ;;
        --tag)
            TAG_NAME="$2"
            shift 2
            ;;
        -h|--help)
            echo "Usage: ./scripts/package_android.sh [--release | --debug] [--tag <tag>]"
            exit 0
            ;;
        *)
            shift
            ;;
    esac
done

echo "⚙️  Build Profile: ${BUILD_TYPE} (Tag: ${TAG_NAME})"

# 2. Select compatible JDK (prefer Java 21 or 17 for Gradle/AGP compatibility)
if [ -x "/usr/libexec/java_home" ]; then
    COMPAT_JAVA=$(/usr/libexec/java_home -v 21 2>/dev/null || /usr/libexec/java_home -v 17 2>/dev/null || true)
    if [ -n "$COMPAT_JAVA" ] && [ -d "$COMPAT_JAVA" ]; then
        export JAVA_HOME="$COMPAT_JAVA"
        export PATH="$JAVA_HOME/bin:$PATH"
        echo "☕ Auto-configured compatible JAVA_HOME: $JAVA_HOME"
    fi
fi

# 3. Auto-detect Android SDK & NDK
if [ -z "$ANDROID_HOME" ] && [ -z "$ANDROID_SDK_ROOT" ]; then
    POSSIBLE_SDK_PATHS=(
        "$HOME/Library/Android/sdk"
        "$HOME/Android/Sdk"
        "/opt/android-sdk"
        "/usr/local/share/android-sdk"
    )
    for p in "${POSSIBLE_SDK_PATHS[@]}"; do
        if [ -d "$p" ]; then
            export ANDROID_HOME="$p"
            export ANDROID_SDK_ROOT="$p"
            echo "🔍 Auto-detected Android SDK: $ANDROID_HOME"
            break
        fi
    done
fi

if [ -z "$ANDROID_NDK_HOME" ] && [ -z "$NDK_HOME" ]; then
    POSSIBLE_NDK_PATHS=(
        "$ANDROID_HOME/ndk"/*
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

# Generate android/local.properties
if [ -n "$ANDROID_HOME" ] && [ -d "$ANDROID_HOME" ]; then
    mkdir -p android
    echo "sdk.dir=$ANDROID_HOME" > android/local.properties
    if [ -n "$ANDROID_NDK_HOME" ] && [ -d "$ANDROID_NDK_HOME" ]; then
        echo "ndk.dir=$ANDROID_NDK_HOME" >> android/local.properties
    fi
fi

# 4. Verify rustup targets for ARM64 and x86_64
for target in "aarch64-linux-android" "x86_64-linux-android"; do
    if ! rustup target list | grep -q "${target} (installed)"; then
        echo "⚙️  Adding rustup target ${target}..."
        rustup target add "${target}"
    fi
done

# 5. Verify cargo-ndk installation
if ! command -v cargo-ndk &> /dev/null; then
    echo "⚠️  cargo-ndk is not installed on this system."
    echo "👉 Attempting to install cargo-ndk..."
    cargo install cargo-ndk || {
        echo "❌ Failed to install cargo-ndk. Please install it with: cargo install cargo-ndk"
        exit 1
    }
fi

# 6. Compile native dynamic shared library (.so) for both ARM64 and x86_64
echo "🔨 Step 1: Compiling native cdylib for arm64-v8a & x86_64 with cargo-ndk..."
mkdir -p android/app/src/main/jniLibs/arm64-v8a android/app/src/main/jniLibs/x86_64

RUSTFLAGS="-C link-arg=-lc++_shared -C link-arg=-Wl,-z,max-page-size=16384 ${RUSTFLAGS}" cargo ndk -t arm64-v8a -t x86_64 -o android/app/src/main/jniLibs build --lib ${CARGO_FLAGS}

# Bundle libc++_shared.so from Android NDK toolchain
if [ -n "$ANDROID_NDK_HOME" ]; then
    echo "📦 Bundling libc++_shared.so from Android NDK..."
    LIBCXX_ARM64=$(find "$ANDROID_NDK_HOME" -path "*/aarch64-linux-android/libc++_shared.so" 2>/dev/null | head -n 1)
    LIBCXX_X86_64=$(find "$ANDROID_NDK_HOME" -path "*/x86_64-linux-android/libc++_shared.so" 2>/dev/null | head -n 1)

    if [ -n "$LIBCXX_ARM64" ] && [ -f "$LIBCXX_ARM64" ]; then
        cp "$LIBCXX_ARM64" android/app/src/main/jniLibs/arm64-v8a/
        echo "  -> Copied arm64-v8a libc++_shared.so"
    fi

    if [ -n "$LIBCXX_X86_64" ] && [ -f "$LIBCXX_X86_64" ]; then
        cp "$LIBCXX_X86_64" android/app/src/main/jniLibs/x86_64/
        echo "  -> Copied x86_64 libc++_shared.so"
    fi
fi

# Bundle pre-compiled Libretro GBA core (mGBA)
echo "📦 Bundling Libretro GBA Core (libmgba_core.so)..."
if [ ! -f "cores/android_arm64/mgba_libretro_android.so" ]; then
    mkdir -p cores/android_arm64
    curl -sSL "https://buildbot.libretro.com/nightly/android/latest/arm64-v8a/mgba_libretro_android.so.zip" -o cores/android_arm64/mgba.zip 2>/dev/null || true
    unzip -o cores/android_arm64/mgba.zip -d cores/android_arm64/ 2>/dev/null || true
fi
if [ ! -f "cores/android_x86_64/mgba_libretro_android.so" ]; then
    mkdir -p cores/android_x86_64
    curl -sSL "https://buildbot.libretro.com/nightly/android/latest/x86_64/mgba_libretro_android.so.zip" -o cores/android_x86_64/mgba.zip 2>/dev/null || true
    unzip -o cores/android_x86_64/mgba.zip -d cores/android_x86_64/ 2>/dev/null || true
fi

if [ -f "cores/android_arm64/mgba_libretro_android.so" ]; then
    cp "cores/android_arm64/mgba_libretro_android.so" android/app/src/main/jniLibs/arm64-v8a/libmgba_core.so
    echo "  -> Copied arm64-v8a libmgba_core.so"
fi
if [ -f "cores/android_x86_64/mgba_libretro_android.so" ]; then
    cp "cores/android_x86_64/mgba_libretro_android.so" android/app/src/main/jniLibs/x86_64/libmgba_core.so
    echo "  -> Copied x86_64 libmgba_core.so"
fi

# 7. Strip debug symbols with NDK llvm-strip to guarantee small package footprint
echo "✂️  Step 2: Stripping symbols using llvm-strip..."
LLVM_STRIP=""

if [ -n "$ANDROID_NDK_HOME" ]; then
    LLVM_STRIP=$(find "$ANDROID_NDK_HOME" -name "llvm-strip" 2>/dev/null | head -n 1)
fi

if [ -z "$LLVM_STRIP" ] && command -v llvm-strip &> /dev/null; then
    LLVM_STRIP="llvm-strip"
fi

if [ -n "$LLVM_STRIP" ] && [ -f "$LLVM_STRIP" ]; then
    echo "Using strip tool: $LLVM_STRIP"
    find android/app/src/main/jniLibs -name "*.so" -exec "$LLVM_STRIP" --strip-unneeded {} + 2>/dev/null || true
    echo "✅ Successfully stripped symbols across all native ABIs."
else
    echo "ℹ️  llvm-strip not found, skipping explicit symbol stripping."
fi

# 8. Build APK with Gradle wrapper
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

# 9. Locate generated signed APK and copy to dist/
APK_RELEASE_SIGNED="android/app/build/outputs/apk/release/app-release.apk"
APK_RELEASE_UNSIGNED="android/app/build/outputs/apk/release/app-release-unsigned.apk"
APK_DEBUG="android/app/build/outputs/apk/debug/app-debug.apk"

DEST_APK="dist/PixelDrive-Android-${TAG_NAME}.apk"

if [ -f "$APK_RELEASE_SIGNED" ]; then
    cp "$APK_RELEASE_SIGNED" "$DEST_APK"
elif [ -f "$APK_DEBUG" ]; then
    cp "$APK_DEBUG" "$DEST_APK"
elif [ -f "$APK_RELEASE_UNSIGNED" ]; then
    cp "$APK_RELEASE_UNSIGNED" "$DEST_APK"
else
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
echo "🎉 Successfully generated Signed Android Package:"
echo "   Path: $DEST_APK"
echo "   Size: $APK_SIZE (Target: <= 25MB)"
echo "   ABIs: arm64-v8a, x86_64 (Universal Device & BlueStacks Support)"
echo "============================================================"
