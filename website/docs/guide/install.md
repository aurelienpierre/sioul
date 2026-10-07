---
description: Download Sioul for Windows, macOS or Linux, or build it from its sources.
---

# Install

## Download

The packages of each version are on [the releases page](https://github.com/aurelienpierre/sioul/releases/latest), built by GitHub from that version's sources:

| System | Package | How |
|---|---|---|
| Windows 10 and 11 (64-bit) | `sioul-<version>-setup.exe` | Run it. Windows may say it does not know the publisher, as the package is not signed yet: **More info**, then **Run anyway**. |
| macOS 13 and later, Apple silicon and Intel | `Sioul-<version>-macos-universal.dmg` | Open it, and drag Sioul into Applications. It is not notarised yet: the first time, open it from Applications, then **System Settings ▸ Privacy & Security ▸ Open Anyway**. |
| Linux, any distribution (64-bit) | `Sioul-<version>-x86_64.AppImage` | Make it executable (`chmod +x Sioul-*.AppImage`), then run it. On Ubuntu 24.04 and later, its sites run without Chromium's sandbox: prefer the Flatpak there. |
| Linux, with Flatpak | `Sioul-<version>.flatpak` | `flatpak install --user Sioul-<version>.flatpak`: the KDE runtime and Qt WebEngine come from Flathub. |

Each comes with the command line, `sioul`. These first packages were built and tested by GitHub, but few people have run them yet: a word in [GitHub issues](https://github.com/aurelienpierre/sioul/issues) helps.

## Resources needed

Measured on 6 October 2026 on a Linux computer with 8 cores, with Sioul and every process it starts counted together (memory as PSS: each shared page counted once):

| | Memory | Processor |
|---|---|---|
| **Sioul alone**: an invented profile, no site open, over five minutes | about 70 MB, in a single process | close to nothing at rest (0.2 % of one core) |
| **Sioul in daily use**: a real mailbox, notes and tasks, and three sites open in real time, over three minutes | about 1.15 GB in all: Sioul's own process 0.4 GB, the pages of the three sites 0.7 GB | under 1 % of one core most of the time (half of the time under 0.4 %), a few seconds at up to 40 % of one core now and then |

- **What is not open costs nothing.** Each page of the window, each form and menu is made the first time you open it, and the engine that shows websites starts with the first site you open. On 4 October, before this, Sioul alone took 280 MB, and 1.9 GB in daily use (its own process 0.7 GB).
- **The sites are the heavy part.** Each one kept open costs what a tab of a web browser costs, 150 to 350 MB, and the processor its page uses: a chat that keeps itself up to date is the busiest.
- **Disk**: a few megabytes read and written in a few minutes.
- **With the window closed**, the reminder watcher (Settings ▸ Reminders) takes 10 MB and almost no processor.
- **On a phone** (Android, a 2019 phone): about 200 MB while Sioul is on the screen and 140 MB once it is put away, with almost no processor then; each page opens in 0.1 to 0.5 s the first time, and at once after.
- **What to plan for**: 4 GB of memory for Sioul and a few sites beside your other programs, 8 GB to keep many sites open; without sites, Sioul needs very little. Any processor of the last ten years.

The measure can be made again on any computer: `tools/measure-load.py` in the sources ([building.md](https://github.com/aurelienpierre/sioul/blob/main/docs/building.md#measuring-the-load)).

## Or build it from its sources

The rest of this page builds Sioul from its sources: for Linux distributions without packages yet, and to follow the newest changes.

## What it needs

- **Rust** 1.89 or newer, with Cargo.
- **A C++ compiler**: the window is partly written in C++.
- **D-Bus** development files, on Linux: for the system keyring, where passwords go, and for notifications.
- **Qt 6.9 or newer** (Sioul is built and used with Qt 6.11), with these modules: Qt Declarative (QML and Qt Quick), Qt WebEngine with Qt WebChannel and Qt PDF, Qt Multimedia, Qt Positioning, Qt Location and Qt Image Formats.

Optional, and installable later:

- **ClamAV**, to have attachments checked before they open. Without it, Sioul tells you a file will not be checked, and asks before opening it.
- **Tesseract and Poppler**, to read the paper letters you scan. Without them, the scans wait, unread. See [Papers and letters](papers.md#paper-letters).

## On Linux

### The packages

=== "Fedora"

    ```
    sudo dnf install git rust cargo gcc-c++ dbus-devel \
        qt6-qtbase-devel qt6-qtdeclarative-devel qt6-qtwebengine-devel \
        qt6-qtwebchannel-devel qt6-qtpdf-devel qt6-qtpositioning-devel \
        qt6-qtmultimedia qt6-qtlocation qt6-qtimageformats
    ```

    Optional: the antivirus, then its signatures, kept up to date:

    ```
    sudo dnf install clamav clamav-update
    sudo freshclam
    sudo systemctl enable --now clamav-freshclam
    ```

    Optional: reading paper letters, in French and English:

    ```
    sudo dnf install tesseract tesseract-langpack-fra poppler-utils
    ```

=== "Debian, Ubuntu, Arch"

    The D-Bus development files, a C++ compiler, and Qt 6.9 or newer with the modules above, and their QML modules. Distributions name them differently: Debian and Ubuntu as `qt6-…-dev` and `qml6-module-…`, Arch as `qt6-…`. Your release must ship Qt 6.9 or newer.

    Rust from [rustup.rs](https://rustup.rs) when your release's own is older than 1.89.

Sioul is developed on Fedora: other systems have not been tried. If a module is missing, the build stops, or the window does not open and Qt names the missing module in its messages (`QT_FORCE_STDERR_LOGGING=1 sioul-app` shows them in the terminal). A word in [GitHub issues](https://github.com/aurelienpierre/sioul/issues) then helps the next person.

### Building

```
git clone https://github.com/aurelienpierre/sioul.git
cd sioul
cargo build --release
cargo build --release -p sioul-app
```

The first `cargo build` makes `sioul`, the command line; the second makes `sioul-app`, the window. The first build downloads and compiles a few hundred libraries: it takes a while, and a few gigabytes of disk.

### Into your application menu

```
install -Dm755 target/release/sioul-app ~/.local/bin/sioul-app
install -Dm755 target/release/sioul ~/.local/bin/sioul
install -Dm644 data/com.aurelienpierre.Sioul.desktop ~/.local/share/applications/com.aurelienpierre.Sioul.desktop
install -Dm644 data/com.aurelienpierre.Sioul.metainfo.xml ~/.local/share/metainfo/com.aurelienpierre.Sioul.metainfo.xml
mkdir -p ~/.local/share/icons && cp -r data/icons/hicolor ~/.local/share/icons/
```

Sioul then shows in your application menu as "Sioul". The command line `sioul` is worth installing too: reminders use it when the window is closed ([Settings](settings.md#reminders)).

### Updating

In the `sioul` folder: `git pull`, then the two `cargo build` lines and the three `install` lines again. Your settings and data are kept apart from the program, in your own folders: they stay as they are.

## Trying it on invented mail first

From the `sioul` folder, the command line can show the Porch on a few invented messages, without any account:

```
cargo run -q -- --config examples/demo.toml porch
```

## On Windows

Not tried yet. The steps, for those who want to:

1. Rust, through [rustup](https://rustup.rs), with Microsoft's build tools (MSVC).
2. Qt 6.9 or newer from Qt's online installer, for MSVC 2022 64-bit, with the modules above, and Qt's `bin` folder in your `PATH`.
3. `cargo build --release -p sioul-app`.
4. `windeployqt --release --qmldir crates\sioul-app\qml target\release\sioul-app.exe` gathers Qt beside the program. `packaging\windows\sioul.iss` makes an installer with Inno Setup.

On Windows, attachments are checked by Microsoft Defender, through the Antimalware Scan Interface. Reminders come only while the window is open.

## On macOS

Not tried yet. The steps are in [packaging/macos/README.md](https://github.com/aurelienpierre/sioul/blob/main/packaging/macos/README.md): Qt 6.9 or newer from Qt's online installer, Rust from rustup, then `cargo build --release -p sioul-app -p sioul-cli` and a bundle made with `macdeployqt`; the build workflow on GitHub can make the same `.dmg` when started by hand. ClamAV from Homebrew (`brew install clamav`) checks attachments when it is there. The bundle is not signed yet: macOS asks you to confirm the first opening (right click, Open).

## On Android

For 64-bit phones with Android 9 or later. Sioul is not in a store: its APK, `Sioul-<version>-android-arm64.apk`, is on [the releases page](https://github.com/aurelienpierre/sioul/releases/latest).

1. On the phone, open the releases page and download the APK.
2. Open it. Android asks whether your browser (or your file manager) may install apps: allow it, then **Install**. Play Protect, where the phone has it, may say it does not know the app.
3. Each version is signed with the same key, Sioul's: the next one installs over it, your data kept.

What Sioul then asks for, and what changes in use: [On a phone](first-steps.md#on-a-phone). How the APK is built, by GitHub or by hand, and what differs on Android: the [notes on Android](../dev/android.md).

## Next

[First steps](first-steps.md): adding your mail, your calendars and contacts, Google, and the sites you check.
