#!/usr/bin/env bash
# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright © 2026 Aurélien Pierre
#
# Sioul for Android from sources alone, as F-Droid builds it
# (packaging/fdroid/README.md): Qt from its own sources, first for this
# computer (its tools build for the phone) then for Android, and Sioul's APK
# from that Qt, unsigned. No library comes built: F-Droid takes none, where
# .github/workflows/android.yml takes Qt's.
#
#     packaging/fdroid/build.sh STAGE...
#
# The stages, in order (`all` is the three):
#   sources  Qt's modules that Sioul needs, fetched into QT_SOURCE
#            (qt5.git at Sioul's Qt tag; its init-repository);
#   qt       the desktop Qt, then Qt for Android, installed under WORK;
#   apk      Sioul's APK from that Qt: WORK/sioul-unsigned.apk.
#
# It reads:
#   QT_SOURCE           qt5.git, checked out at the tag of QT_VERSION below
#   ANDROID_SDK_ROOT    the SDK, with the platform android-36 and its build tools
#   ANDROID_NDK_ROOT    the NDK Qt 6.11 is built with, r27c (27.2.12479018)
#   CORROSION_SOURCE    Corrosion's sources at the tag android/CMakeLists.txt
#                       names (optional: CMake fetches them without it)
#   SIOUL_FDROID_WORK   where it builds (default: build/fdroid/ here)
#   JOBS                how many at once (default: every processor)
# and needs CMake 3.22 or later, Ninja, a C and C++ compiler for this
# computer, Java 17 or later, git, unzip, and Rust with its phone target
# (`rustup target add aarch64-linux-android`).
#
# What differs from the APK on the releases page, and why:
# - Qt Quick Controls' Basic style alone, the one Sioul's pages import (the
#   other five weighed 11 MiB in the APK of 0.0.10);
# - no QML debugger or profiler, which a release never uses;
# - no FFmpeg: Qt Multimedia plays through Android's own media player (its
#   "native Android backend", deprecated in Qt 6.11 but built). Memos are not
#   recorded on Android yet (docs/android.md, "Not done yet"), and Android
#   plays their Ogg Opus itself;
# - no OpenSSL in Qt: the APK of the releases page has Qt's OpenSSL plugin but
#   not OpenSSL, so neither has TLS in Qt; Sioul's own connections are Rust's.
set -euo pipefail

QT_VERSION=6.11.3
# Qt's modules for the phone; the desktop Qt needs only those whose tools
# build the others (shaders, QML).
QT_MODULES_ANDROID=qtbase,qtshadertools,qtdeclarative,qtsvg,qtimageformats,qtserialport,qtpositioning,qtlocation,qtmultimedia
QT_MODULES_HOST=qtbase,qtshadertools,qtdeclarative
ABI=arm64-v8a
# Sioul's oldest Android, as android/CMakeLists.txt says (Android 9).
API=28

root=$(cd "$(dirname "$0")/../.." && pwd)
work=${SIOUL_FDROID_WORK:-$root/build/fdroid}
jobs=${JOBS:-$(nproc)}
host_prefix=$work/qt-host
android_prefix=$work/qt-android

need() {
    for variable in "$@"; do
        if [ -z "${!variable:-}" ]; then
            echo "$variable is not set (packaging/fdroid/build.sh says what it needs)" >&2
            exit 2
        fi
    done
}

# Qt's modules, fetched from where qt5.git came from (code.qt.io for F-Droid).
sources() {
    need QT_SOURCE
    (cd "$QT_SOURCE" && ./init-repository -f --module-subset="$QT_MODULES_ANDROID")
    found=$(sed -n 's/^set(QT_REPO_MODULE_VERSION "\(.*\)")$/\1/p' "$QT_SOURCE/qtbase/.cmake.conf")
    if [ "$found" != "$QT_VERSION" ]; then
        echo "QT_SOURCE holds Qt $found; Sioul is built with Qt $QT_VERSION" >&2
        exit 1
    fi
}

# Each Qt in a folder of its own, configured, built and installed; the build
# folder removed after, to keep the disk for the next.
build_qt() {
    local name=$1
    shift
    rm -rf "$work/build-$name"
    mkdir -p "$work/build-$name"
    (
        cd "$work/build-$name"
        "$QT_SOURCE/configure" "$@"
        cmake --build . --parallel "$jobs"
        cmake --install .
    )
    rm -rf "$work/build-$name"
}

qt() {
    need QT_SOURCE ANDROID_SDK_ROOT ANDROID_NDK_ROOT
    # This computer's Qt: its tools only, with the libraries Qt carries in its
    # sources rather than the system's.
    build_qt host -prefix "$host_prefix" -release -opensource -confirm-license \
        -submodules "$QT_MODULES_HOST" -nomake tests -nomake examples \
        -no-opengl -no-dbus -no-openssl -no-glib -no-icu -no-cups -no-fontconfig \
        -no-feature-testlib -no-feature-qml-debug \
        -qt-zlib -qt-pcre -qt-harfbuzz -qt-freetype -qt-libpng -qt-libjpeg \
        -- -DCMAKE_INSTALL_MESSAGE=NEVER
    # Qt for the phone. Flexible page sizes: Android 15 runs on phones with
    # 16 KB memory pages, where a library aligned for 4 KB does not load.
    build_qt android -platform android-clang -prefix "$android_prefix" -release \
        -opensource -confirm-license \
        -android-sdk "$ANDROID_SDK_ROOT" -android-ndk "$ANDROID_NDK_ROOT" \
        -android-abis "$ABI" -android-ndk-platform "android-$API" \
        -qt-host-path "$host_prefix" \
        -submodules "$QT_MODULES_ANDROID" -nomake tests -nomake examples \
        -no-openssl -no-feature-ffmpeg -no-feature-qml-debug \
        -no-feature-quickcontrols2-fusion -no-feature-quickcontrols2-imagine \
        -no-feature-quickcontrols2-material -no-feature-quickcontrols2-universal \
        -no-feature-quickcontrols2-fluentwinui3 -no-feature-quickcontrols2-stylekit \
        -no-feature-quickcontrols2-macos -no-feature-quickcontrols2-ios \
        -no-feature-quickcontrols2-windows \
        -- -DANDROID_SUPPORT_FLEXIBLE_PAGE_SIZES=ON -DCMAKE_INSTALL_MESSAGE=NEVER
}

apk() {
    need ANDROID_SDK_ROOT ANDROID_NDK_ROOT
    local build=$work/sioul
    local args=(-S "$root/android" -B "$build" -G Ninja
        -DCMAKE_BUILD_TYPE=Release -DQT_HOST_PATH="$host_prefix"
        -DANDROID_SDK_ROOT="$ANDROID_SDK_ROOT" -DANDROID_NDK_ROOT="$ANDROID_NDK_ROOT"
        -DANDROID_SUPPORT_FLEXIBLE_PAGE_SIZES=ON)
    if [ -n "${CORROSION_SOURCE:-}" ]; then
        args+=(-DFETCHCONTENT_SOURCE_DIR_CORROSION="$CORROSION_SOURCE")
    fi
    "$android_prefix/bin/qt-cmake" "${args[@]}"
    cmake --build "$build" --parallel "$jobs" --target apk
    local unsigned
    unsigned=$(find "$build" -path '*/outputs/apk/release/*-unsigned.apk' | head -n 1)
    if [ -z "$unsigned" ]; then
        find "$build" -name '*.apk' >&2
        echo "No unsigned release APK" >&2
        exit 1
    fi
    cp "$unsigned" "$work/sioul-unsigned.apk"
    # No test hook in what ships (tools/check-test-marks.sh).
    rm -rf "$work/apk-libs"
    unzip -q "$work/sioul-unsigned.apk" 'lib/*' -d "$work/apk-libs"
    sh "$root/tools/check-test-marks.sh" "$work"/apk-libs/lib/*/libsioul_*.so
    rm -rf "$work/apk-libs"
    ls -l "$work/sioul-unsigned.apk"
}

if [ "$#" -eq 0 ]; then
    echo "usage: $0 sources|qt|apk|all..." >&2
    exit 2
fi
mkdir -p "$work"
for stage in "$@"; do
    case "$stage" in
        sources) sources ;;
        qt) qt ;;
        apk) apk ;;
        all) sources; qt; apk ;;
        *) echo "unknown stage: $stage (sources, qt, apk or all)" >&2; exit 2 ;;
    esac
done
