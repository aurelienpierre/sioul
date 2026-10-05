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

## Starting
On a 2019 phone (Gigaset GS290, Helio P23, Android 12), Sioul's window draws its first frame about 1.8 s after a tap: 0.55 s before Sioul's own code runs (Android starts the app, Qt's Java side loads Qt's libraries and Sioul's), then 1.25 s. It was 3.5 s. `adb logcat -s sioul` says where the time goes ("Sioul: the window loaded after … ms"); on a computer, `SIOUL_TIMING=1`.
- **Made when needed**: the pages but the one shown, the Reader on the Porch, the menus, dialogs, settings panels and pop-ups, and the windows (writing, focus, routines). Those made by their file (`Loader.setSource`, `Qt.createComponent`) are not even read at the start.
- **Read once, beside**: while the QML engine reads the window's files, a thread reads the system's fonts (200 on a phone) and the image formats (`cpp/warmup.cpp`).
- **Looked up less**: Qt's file selectors off (`QT_NO_BUILTIN_SELECTORS`), the icon theme in four folders instead of eighteen, Sioul Symbols for the symbols Roboto lacks (no search through the phone's fonts), the Basic style for Qt's own tips (Material not loaded).
- **Linked lighter**: Sioul's library exports 19 symbols, not 28,000, and its relocations are packed (`--exclude-libs`, RELR: `android/CMakeLists.txt`).
- **What remains is Qt's**: relocating Qt's own libraries (0.3 s), registering the types of Qt Quick and its controls (0.3 s), and the media and position plugins Qt's Java side loads at the start, FFmpeg with them, since their `JNI_OnLoad` must run from Java.
- **Measured with** `simpleperf` (the manifest makes the app profileable from a computer plugged in) and Qt's QML profiler (`qmlprofiler`, with `QQmlDebuggingEnabler::startTcpDebugServer` in a build of one's own, never shipped).

## What differs on Android
- **Sites**: Qt WebEngine has no Android version. The Sites page lists your sites and opens each in the browser (`qml/android/SitesPage.qml`): no logins kept in Sioul, no notifications gathered.
- **PDFs**: Qt PDF has no Android version either; a PDF of the notes opens with "Open with…" (`qml/android/PdfView.qml`). Both pages keep the names of the computer's, so the module's `qmldir` finds them (`build.rs` lists one or the other).
- **Passwords** go to Android's KeyStore, through [android-keyring](https://github.com/Andrepuel/android-keyring): a key the KeyStore keeps, and never lets out, encrypts each password (AES-GCM) into the app's private storage. Its author calls it experimental.
- **Sender checks** ask the network's DNS servers, which Android gives through Java (hickory reads them with the Context lent at start).
- **Certificates**: Android's own (its Conscrypt module, else the system image's), for IMAP and SMTP alike. On computers SMTP checks them with the system's verifier (`rustls-platform-verifier`), which on Android needs Java code Sioul does not ship.
- **The window on a phone**: below 720 pixels wide, the places are pulled over the pages from the left (☰) and each page shows one pane at a time, its list or what it opened; Android's Back goes back from what is open, then to the Porch. Upright, every page fits the screen's width: titles go under the arrows, buttons keep their icons alone, rows that do not fit wrap or go one under the other (the board's columns, an account's address, a movement's budget), margins are narrower (`Theme.gap`). Checked by the window's tests at a phone's size (`SIOUL_GRAB_PHONE`, 412 × 891), which name every item past the right edge.
- **Accounts**: "From this phone's accounts…" opens Android's chooser of the phone's accounts (Murena's, Nextcloud's, Google's…); the address chosen fills the form, Sioul finds the servers, and asks the password once, as on a computer. Mail, calendars and contacts then sync through Sioul's own code, not the phone's.
- **Sharing with your other devices**: the sharing folder is the one your sync app carries (Murena's eDrive, Nextcloud's, Syncthing, FolderSync…), read by its path, with Android's "All files access" (asked from the sharing panel). Sioul talks to no sync service itself; folders already shared by your other devices are offered, and folders are chosen in Sioul's own browser (`qml/FolderBrowser.qml`, the folders read through that access): Android's picker refuses the phone's storage on /e/OS ("for privacy") and gives `content://` addresses, read back as paths only for the phone's own storage (`Theme.localPath`). Murena's eDrive carries only fixed folders (Documents, Pictures, Music…, not the cloud's root): the folder goes inside Documents. Tried with eDrive 1.9.2: the folder came down at its next full scan. The notes folder stays on the phone (`~/Notes`, in the app's storage): its projects, budgets and bank movements come through the sharing once "Projects, budgets and the bank travel here too, sealed" is ticked, on any device: the setting travels to the others (docs/database.md).
- **Passwords of accounts from another device** never travel: each account asks for its own (**Password…** on its card), typed or taken from Bitwarden by Sioul's own client (`qml/AccountPassword.qml`, `qml/VaultUnlock.qml`). No security key there: Sioul asks one through Qt WebEngine, which Android lacks.

## Not done yet
- **Notifications**: none on Android (codes, reminders, doses); they need Android's own, through Java.
- **Touch**: the pages are laid out for a phone, but drawn for a mouse: small targets, menus on a right click.
- **In the background**: Android suspends an app it does not show; mail is fetched while Sioul is open.
- **Files from Android's pickers** come as `content://` addresses, which the core does not read yet; downloads stay in the app's own folder.
- **Other programs** Sioul calls on a computer (Tesseract for paper letters, ClamAV for attachments) do not exist on a phone.
- **Permissions asked at the time** (the microphone for memos, the position for maps), as Android wants them.
- **Signing**: with no key in the repository's secrets, each build is signed with a key of its own, so a phone takes the next one for another app (uninstall first). A store needs a key kept for good, and Google Play an Android App Bundle; F-Droid builds everything from source, Qt included.
