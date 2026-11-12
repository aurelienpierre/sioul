# Sioul for F-Droid

F-Droid builds every app it carries from its sources, on its own servers, and takes no library already built: Qt included, which the APK of the releases page takes from Qt's own packages. What is here lets F-Droid build Sioul that way.

- **`build.sh`** builds Qt 6.11.3 from its sources, first for the computer (its tools build for the phone) then for Android, and Sioul's APK from that Qt, unsigned. Its head says what it reads and needs.
- **`com.aurelienpierre.sioul.yml`** is the recipe F-Droid keeps in its repository of recipes, [fdroiddata](https://gitlab.com/fdroid/fdroiddata), as `metadata/com.aurelienpierre.sioul.yml`.
- **`srclibs/corrosion.yml`** names Corrosion's repository, a source library for fdroiddata's `srclibs/`: F-Droid fetches every source itself rather than let CMake fetch it while building. Qt's (`Qt5`) and rustup's are fdroiddata's own.
- **`../../fastlane/metadata/android/`** holds what F-Droid shows: the name, the summary, the description in English and French, and the icon.
- **`.github/workflows/fdroid.yml`** tries it all (below).

## What F-Droid runs
The recipe is GCompris's way of building Qt (`metadata/net.gcompris.full.yml` in fdroiddata, Qt 6.10, the same NDK), with the stages in `build.sh`:
1. **sudo**: Debian's compilers, CMake and Ninja.
2. **prebuild**: Android's platform 36 and its build tools; Rust 1.99.1 with the phone's target, through rustup; Qt's modules fetched into its source library (`build.sh sources`, Qt's own `init-repository`).
3. **build**: `build.sh qt apk`. F-Droid then signs `build/fdroid/sioul-unsigned.apk` with its key.

What differs from the APK of the releases page, and why, is at the head of `build.sh`: Quick Controls' Basic style alone, no QML debugger, no FFmpeg (Android's own media player instead), no OpenSSL in Qt (neither APK has TLS in Qt). Built so on 12 November 2026, the APK of 0.0.10 weighed 125 MiB and held 189 files, against 155 MiB and 233 for the releases page's, and every library in it, Qt's too, was aligned for 16 KB memory pages.

## Each version
- **Change `version` and `version-code` in `Cargo.toml` together**: major × 10,000 + minor × 100 + patch (0.0.11 → 11). F-Droid reads both there from each new tag `v…` (`UpdateCheckMode: Tags`), and adds the version to the recipe by itself (`AutoUpdateMode: Version`), the build otherwise unchanged. `android/CMakeLists.txt` stops when the two differ, so the Android workflow says it at the first push.
- **What is new** can go in `fastlane/metadata/android/en-US/changelogs/<version code>.txt` (and `fr-FR/`), 500 characters at most; F-Droid shows it with the version.
- **Qt's version** is in three places: `QT_VERSION` in `build.sh`, the recipe's `Qt5@v…`, and `QT_VERSION` in `.github/workflows/android.yml`. Rust's is in the recipe and in `fdroid.yml`. A change to them is a change to the recipe, sent to fdroiddata.

## Trying it: `.github/workflows/fdroid.yml`
It runs when this folder or the workflow change, and by hand (Actions, "F-Droid", "Run workflow"), in F-Droid's own build image (`registry.gitlab.com/fdroid/fdroidserver:buildserver-trixie`):
- **From sources**: `build.sh` as the recipe runs it. Qt (fetched, then built for the computer and for Android: 62 minutes on GitHub's runner on 12 November 2026; Sioul then took 14) is kept in the cache while `build.sh`'s Qt part does not change. The APK is the run's artifact, also signed with a key made for the run, to try on a phone (uninstall Sioul first: a phone takes it for another app).
- **F-Droid's tools**, after it: the recipe at the run's commit, fetched and built by fdroidserver as fdroiddata's CI builds a merge request's (`fdroid fetchsrclibs`, then `fdroid build --on-server --test`), F-Droid's scanner included.

On 12 November 2026, at commit 5a3b088, "F-Droid's tools" built `com.aurelienpierre.sioul_10.apk` (124 MiB) in 1 hour 42 minutes on GitHub's runner of four cores: F-Droid's scanner, with its signatures of that day, found nothing, and `fdroid build` said "Successfully built". By hand, on a computer, the same day: `fdroid lint` and `fdroid rewritemeta` on the recipe in a copy of fdroiddata both passed (the recipe as `rewritemeta` writes it).

## Sending it to F-Droid
Once a version tag holds this folder (the recipe names 0.0.11; change it if the first is another):
1. Fork [fdroiddata](https://gitlab.com/fdroid/fdroiddata) on GitLab, public, its branch unprotected; add `metadata/com.aurelienpierre.sioul.yml` and `srclibs/corrosion.yml` from here, and nothing else (the words and pictures come from `fastlane/` here).
2. Open a merge request titled "New app: Sioul". Its template asks what Sioul does and why it was written, with some screenshots, and for a list of checks ([the inclusion policy](https://f-droid.org/docs/Inclusion_Policy), the metadata, tagged releases with automatic updates). Its CI runs `fdroid lint`, `fdroid rewritemeta` and `fdroid build`; F-Droid's people review the recipe and `build.sh`.
3. Answer their questions there. Once merged, a build on F-Droid's servers publishes it.

**Screenshots** are not here yet: the template asks for them, and F-Droid shows them from `fastlane/metadata/android/<language>/images/phoneScreenshots/` (1.png, 2.png…). A phone's own, or the window at a phone's size on a computer (`SIOUL_GRAB_PHONE`, [Qt Quick and QML](../../docs/qt-quick.md#a-computer-a-tablet-and-a-phone)).

## Its key, and the choice it makes
F-Droid signs what it builds with a key of its own. Android installs a new version over the old only when both have the same signature, so the copy from F-Droid and the copy from the releases page are two apps to a phone: moving from one to the other means uninstalling, and the phone's data goes with it. The F-Droid page should say so.

F-Droid can instead publish Sioul's own APK, with Sioul's signature, when its build of the same version is identical to it, byte for byte, apart from the signature (reproducible builds: `AllowedAPKSigningKeys` and `Binaries` in the recipe). That needs the releases page's APK built by `build.sh` too, with the same toolchain at the same paths as F-Droid's server; it is not done. It has to be chosen before F-Droid first publishes Sioul: a change of key afterwards makes F-Droid's users uninstall once.

## Google's verification of developers
From 30 September 2026 in Brazil, Indonesia, Singapore and Thailand, and in 2027 everywhere, phones with Google's services install an app outside Google Play without a warning only when its package name and signing key are registered with Google (the [Android Developer Console](https://developer.android.com/developer-verification/guides)). For the releases page's APK, that is Sioul's key; for F-Droid's, F-Droid's key, which Google's [guide for open-source repositories](https://developer.android.com/developer-verification/guides/open-source-app-registration) registers from F-Droid's APK once published (an `adi-registration.properties` in the APK's assets, then a version). Without it, people can still install Sioul once they have turned on Android's "advanced flow", with its wait. F-Droid opposes the scheme ([its open letter](https://f-droid.org/2026/02/24/open-letter-opposing-developer-verification.html)).
