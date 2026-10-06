Sioul's packages, built by GitHub from this version's sources. Sioul is young: keep a copy of your mail and your notes elsewhere, as with any new program.

| System | Package | How |
|---|---|---|
| Windows 10 and 11 (64-bit) | `sioul-<version>-setup.exe` | Run it; Windows may say it does not know the publisher (the package is not signed yet): "More info", then "Run anyway". |
| macOS 13 and later, Apple silicon and Intel | `Sioul-<version>-macos-universal.dmg` | Open it and drag Sioul into Applications. It is not notarised yet: the first time, open it from Applications, then System Settings ▸ Privacy & Security ▸ "Open Anyway". |
| Linux, any distribution (x86-64) | `Sioul-<version>-x86_64.AppImage` | `chmod +x Sioul-*.AppImage`, then run it. On Ubuntu 24.04 and later, the Sites page runs without Chromium's sandbox there: prefer the Flatpak. |
| Linux, with Flatpak | `Sioul-<version>.flatpak` | `flatpak install --user Sioul-<version>.flatpak` (the KDE runtime and Qt WebEngine come from Flathub). |
| Android 9 and later, 64-bit phones | `Sioul-<version>-android-arm64.apk` | Download it on the phone and open it; when Android asks, allow your browser or file manager to install apps. Signed with Sioul's key: the next version installs over it, your data kept. |

The command line, `sioul`, comes with each computer's package (`sioul mcp` connects an AI agent: see the guide).

Guide, in English and French: https://aurelienpierre.github.io/sioul/ — what Sioul does, how to start, and the privacy policy. Questions and reports: https://github.com/aurelienpierre/sioul/issues
