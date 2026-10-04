# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright © 2026 Aurélien Pierre

# Sioul for Android, an experiment (docs/android.md). Qt for Android builds
# the app around Sioul's window, the Rust library crates/sioul-app, which
# Corrosion builds with Cargo for the phone's processor:
#
#   <Qt for Android>/bin/qt-cmake -S android -B build-android -G Ninja \
#       -DCMAKE_BUILD_TYPE=Release -DQT_HOST_PATH=<desktop Qt> \
#       -DANDROID_SDK_ROOT=<SDK> -DANDROID_NDK_ROOT=<NDK> \
#       -DANDROID_SUPPORT_FLEXIBLE_PAGE_SIZES=ON
#   cmake --build build-android --target apk
#
# .github/workflows/android.yml does it all, and signs the APK.

cmake_minimum_required(VERSION 3.22)

# Sioul's version, from Cargo's workspace: one place to change it.
set(SIOUL_ROOT "${CMAKE_CURRENT_LIST_DIR}/..")
file(STRINGS "${SIOUL_ROOT}/Cargo.toml" SIOUL_VERSION_LINE REGEX "^version = \"[0-9]+\\.[0-9]+\\.[0-9]+\"" LIMIT_COUNT 1)
string(REGEX MATCH "[0-9]+\\.[0-9]+\\.[0-9]+" SIOUL_VERSION "${SIOUL_VERSION_LINE}")

# C for the dependencies Cargo compiles (ring), C++ for Qt and CXX-Qt.
project(sioul VERSION ${SIOUL_VERSION} LANGUAGES C CXX)

set(CMAKE_CXX_STANDARD 17)
set(CMAKE_CXX_STANDARD_REQUIRED ON)

find_package(Qt6 6.8 REQUIRED COMPONENTS Core CorePrivate Gui Qml Quick Multimedia Positioning Location)

# The phone's processor, as Rust names it: 64-bit ARM for phones, x86-64 for
# the emulator.
if(ANDROID_ABI STREQUAL "arm64-v8a")
    set(Rust_CARGO_TARGET "aarch64-linux-android")
elseif(ANDROID_ABI STREQUAL "x86_64")
    set(Rust_CARGO_TARGET "x86_64-linux-android")
else()
    message(FATAL_ERROR "Sioul is built for arm64-v8a and x86_64, not for ${ANDROID_ABI}")
endif()

include(FetchContent)
FetchContent_Declare(Corrosion
    GIT_REPOSITORY https://github.com/corrosion-rs/corrosion.git
    GIT_TAG v0.6.1)
FetchContent_MakeAvailable(Corrosion)

corrosion_import_crate(
    MANIFEST_PATH "${SIOUL_ROOT}/Cargo.toml"
    CRATES sioul-app
    # A static library, linked into the program below; made only here, as
    # desktop builds make the program alone (crates/sioul-app/Cargo.toml).
    OVERRIDE_CRATE_TYPE sioul_app=staticlib
    LOCKED)

# Cargo links one dependency as a shared library of its own (android-keyring):
# with the NDK's compiler for this API level, which knows the target.
get_filename_component(SIOUL_NDK_BIN "${CMAKE_C_COMPILER}" DIRECTORY)
if(DEFINED ANDROID_PLATFORM_LEVEL)
    set(SIOUL_API_LEVEL ${ANDROID_PLATFORM_LEVEL})
else()
    set(SIOUL_API_LEVEL ${CMAKE_SYSTEM_VERSION})
endif()
corrosion_set_linker(sioul_app "${SIOUL_NDK_BIN}/${Rust_CARGO_TARGET}${SIOUL_API_LEVEL}-clang")

# CXX-Qt's part (its cxx_qt_import_crate), by hand: CXX-Qt's CMake names a
# crate's export folder after its library (sioul_app), its build script after
# its package (sioul-app).
set(CXX_QT_EXPORT_DIR "${CMAKE_CURRENT_BINARY_DIR}/cxxqt")
set(SIOUL_QMAKE "${QT6_INSTALL_PREFIX}/${QT6_INSTALL_BINS}/qmake" CACHE FILEPATH
    "Qt for Android's qmake: its headers and libraries, the desktop Qt's tools")
# Qt for Android reads no zstd-packed resources, which the desktop Qt's rcc
# makes unless told otherwise; Qt's own CMake tells it the same.
if(NOT QT_FEATURE_zstd)
    set(SIOUL_RCC_OPTIONS "--no-zstd")
endif()
corrosion_set_env_vars(sioul_app
    "CXX_QT_EXPORT_DIR=${CXX_QT_EXPORT_DIR}"
    "CXX_QT_EXPORT_CRATE_sioul-app=1"
    # The Qt modules CXX-Qt's crates ask for: cxx-qt-lib Gui and Qml, Sioul Quick.
    "CXX_QT_QT_MODULES=Qt6::Core,Qt6::Gui,Qt6::Qml,Qt6::Quick"
    $<$<BOOL:${SIOUL_RCC_OPTIONS}>:CXX_QT_AUTORCC_OPTIONS=${SIOUL_RCC_OPTIONS}>
    "QMAKE=${SIOUL_QMAKE}")
target_link_libraries(sioul_app-static INTERFACE Qt6::Core Qt6::Gui Qt6::Qml Qt6::Quick)

# The registration of the QML module, its resources and its types, which the
# build script leaves as object files beside the library: linked as they are,
# for nothing in them is asked for by name.
set(SIOUL_INITIALIZERS
    "${CXX_QT_EXPORT_DIR}/crates/sioul-app/initializers.o"
    "${CXX_QT_EXPORT_DIR}/qml_modules/com/aurelienpierre/sioul/plugin_init.o")
add_custom_target(sioul_app_initializers
    COMMAND ${CMAKE_COMMAND} -E true
    BYPRODUCTS ${SIOUL_INITIALIZERS})
add_dependencies(sioul_app_initializers cargo-build_sioul_app)
set_source_files_properties(${SIOUL_INITIALIZERS} PROPERTIES EXTERNAL_OBJECT TRUE GENERATED TRUE)

# Android's program: a library Qt's Java side loads, which calls main().
qt_add_executable(sioul main.cpp ${SIOUL_INITIALIZERS})
add_dependencies(sioul sioul_app_initializers)
# CorePrivate: Android's activities and their answers (the accounts' chooser, main.cpp).
target_link_libraries(sioul PRIVATE sioul_app Qt6::Core Qt6::CorePrivate Qt6::Gui Qt6::Qml Qt6::Quick log)
# The modules only the pages import, linked so that androiddeployqt takes
# their plugins along: Multimedia's backend (sounds, memos, recordings),
# Positioning's Android source, Location's map tiles.
target_link_libraries(sioul PRIVATE Qt6::Multimedia Qt6::Positioning Qt6::Location)
# Phones with 16 KB memory pages (Android 15 and later) load only libraries aligned for them.
target_link_options(sioul PRIVATE "-Wl,-z,max-page-size=16384")
# Each start, Android's loader fixes up the library's addresses before main().
# Rust's and the C++ glue's symbols stay inside it (--exclude-libs: from the
# static libraries, none of which is called from outside; Qt's Java side
# looks for main() only, main.cpp's): 28,000 symbols exported, 22,000 looked
# up by name at each start, before. The addresses relative to the library
# itself are written packed (RELR, read by Android 9 and later).
target_link_options(sioul PRIVATE "-Wl,--exclude-libs,ALL" "-Wl,--pack-dyn-relocs=android+relr" "-Wl,--use-android-relr-tags")

math(EXPR SIOUL_VERSION_CODE "${PROJECT_VERSION_MAJOR} * 10000 + ${PROJECT_VERSION_MINOR} * 100 + ${PROJECT_VERSION_PATCH}")
set_target_properties(sioul PROPERTIES
    # The manifest, the icon, the backup rules (package/).
    QT_ANDROID_PACKAGE_SOURCE_DIR "${CMAKE_CURRENT_SOURCE_DIR}/package"
    QT_ANDROID_PACKAGE_NAME "com.aurelienpierre.sioul"
    QT_ANDROID_APP_NAME "Sioul"
    QT_ANDROID_VERSION_NAME "${PROJECT_VERSION}"
    QT_ANDROID_VERSION_CODE ${SIOUL_VERSION_CODE}
    # Android 9 and later, as Qt 6.11.
    QT_ANDROID_MIN_SDK_VERSION 28
    QT_ANDROID_TARGET_SDK_VERSION 36
    # Where androiddeployqt reads the pages' imports, to take the QML modules they need.
    QT_QML_ROOT_PATH "${SIOUL_ROOT}/crates/sioul-app/qml")
