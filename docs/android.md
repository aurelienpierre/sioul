# Android

Sioul on Android is an experiment: the same window, built for 64-bit ARM phones running Android 9 or later, installed by hand (an APK). It is not in a store, and parts of Sioul do not work there yet (below).

## Building it
**On GitHub**: `.github/workflows/android.yml` runs on each push to `main` that touches the code or `android/`, and by hand (Actions, "Android", "Run workflow"). The APK is the run's artifact, `sioul-android-arm64`.

**By hand**, with:
- Qt for Android 6.11 (`android_arm64_v8a`) with Qt Multimedia, Qt Positioning, Qt Location, Qt Image Formats and Qt SerialPort, and the desktop Qt of the same version, whose tools build for Android;
- the Android SDK with the NDK Qt 6.11 is built with (r27c, `27.2.12479018`), a platform (`android-36`) and its build tools;
- Java 17 or newer, for Gradle;
- Rust with the phone's target: `rustup target add aarch64-linux-android`.

```
<Qt for Android>/bin/qt-cmake -S android -B build-android -G Ninja \
    -DCMAKE_BUILD_TYPE=Release -DQT_HOST_PATH=<desktop Qt> \
    -DANDROID_SDK_ROOT=<SDK> -DANDROID_NDK_ROOT=<NDK> \
    -DANDROID_SUPPORT_FLEXIBLE_PAGE_SIZES=ON
cmake --build build-android --target apk
```
The APK comes out unsigned, under `build-android/…/outputs/apk/release/`; `apksigner` (the SDK's build tools) signs it, as the workflow does.

## How it is made
- **The window is a library** (`crates/sioul-app/src/lib.rs`, `run()`): the desktop program (`main.rs`) calls it, and so does Android's (`android/main.cpp`, through `sioul_app_run`).
- **Qt for Android builds the app** around it with CMake (`android/CMakeLists.txt`): Corrosion builds the crate with Cargo as a static library for the phone (`OVERRIDE_CRATE_TYPE`, so that desktop builds make the program alone); it is linked with `main.cpp` into the library Qt's Java side loads, `libsioul_arm64-v8a.so`; `androiddeployqt` and Gradle make the APK, with the QML modules the pages import.
- **CXX-Qt's part is done by hand.** CXX-Qt's own CMake function (`cxx_qt_import_crate`) names a crate's export folder after its library (`sioul_app`), and its build script after its package (`sioul-app`): the CMake file sets the build script's variables itself and links the two object files it leaves (the QML module's registration).
- **What a desktop gives a program and Android does not**, `main.cpp` sets before the window starts: the XDG folders, in the app's private storage (configuration, data and state among its files, the cache where Android may empty it); `LANG`, from the phone's language; `SSL_CERT_DIR`, the system's certificates; stdout and stderr sent to logcat (`adb logcat -s sioul`); and the JavaVM and the application's Context, lent to Rust (`sioul_sync::android::init`).
- **The manifest** (`android/package/`) is Qt 6.11's own with Sioul's name, its icon (the quill on its green, in Android's vector drawings, from `tools/make-icons.py`), the network permissions, and no cloud backup: mail, notes, health readings and passwords stay on the phone. Moving to a new phone (cable or Wi-Fi between the two) takes everything but the passwords.
- **16 KB memory pages**: Android 15 runs on phones whose memory pages are 16 KB, where a library aligned for 4 KB does not load. The library is linked for them, and the workflow checks each library of the APK (its summary lists them).

## What differs on Android
- **Sites**: Qt WebEngine has no Android version. The Sites page lists your sites and opens each in the browser (`qml/android/SitesPage.qml`): no logins kept in Sioul, no notifications gathered.
- **PDFs**: Qt PDF has no Android version either; a PDF of the notes opens with "Open with…" (`qml/android/PdfView.qml`). Both pages keep the names of the computer's, so the module's `qmldir` finds them (`build.rs` lists one or the other).
- **Passwords** go to Android's KeyStore, through [android-keyring](https://github.com/Andrepuel/android-keyring): a key the KeyStore keeps, and never lets out, encrypts each password (AES-GCM) into the app's private storage. Its author calls it experimental.
- **Sender checks** ask the network's DNS servers, which Android gives through Java (hickory reads them with the Context lent at start).
- **Certificates**: Android's own (its Conscrypt module, else the system image's), for IMAP and SMTP alike. On computers SMTP checks them with the system's verifier (`rustls-platform-verifier`), which on Android needs Java code Sioul does not ship.

## Not done yet
- **Notifications**: none on Android (codes, reminders, doses); they need Android's own, through Java.
- **A window for a phone**: the pages are laid out for a computer's screen, a mouse and a keyboard.
- **In the background**: Android suspends an app it does not show; mail is fetched while Sioul is open.
- **Files from Android's pickers** come as `content://` addresses, which the core does not read yet; downloads stay in the app's own folder.
- **Other programs** Sioul calls on a computer (Tesseract for paper letters, ClamAV for attachments) do not exist on a phone.
- **Permissions asked at the time** (the microphone for memos, the position for maps), as Android wants them.
- **Signing**: with no key in the repository's secrets, each build is signed with a key of its own, so a phone takes the next one for another app (uninstall first). A store needs a key kept for good, and Google Play an Android App Bundle; F-Droid builds everything from source, Qt included.
