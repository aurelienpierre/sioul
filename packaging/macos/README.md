# Sioul on macOS

1. **Qt 6.9 or newer** from Qt's online installer (with Qt WebEngine, Qt PDF, Qt Multimedia, Qt Positioning, Qt Location, Qt Image Formats), and Rust from rustup.
2. **Build**: `cargo build --release -p sioul-app -p sioul-cli`, with Qt's `bin` folder in `PATH` (`qmake6` is how CXX-Qt finds Qt).
3. **The bundle**: make `Sioul.app/Contents/MacOS/` hold `sioul-app` and `sioul`, put `packaging/macos/Info.plist` in `Sioul.app/Contents/` and `packaging/macos/sioul.icns` in `Sioul.app/Contents/Resources/` (the icon) (without its microphone and camera sentences, macOS ends the program the first time a memo is recorded or a site calls), then gather Qt into it:
   `macdeployqt Sioul.app -qmldir=crates/sioul-app/qml -dmg` makes `Sioul.dmg`. The workflow `.github/workflows/build.yml`, started by hand, does the same on GitHub.
4. **Antivirus**: ClamAV from Homebrew (`brew install clamav`) is used when present, nothing of it in the bundle; when the system keeps no signatures, Sioul keeps its own with Homebrew's `freshclam` in `~/Library/Application Support/sioul/clamav`. Without ClamAV, an attachment opens only after Sioul says it will not be checked and you confirm; the message gives `brew install clamav`.
5. **Passwords** go to the Keychain; **notifications** to the Notification Centre, without the "copy" button Linux desktops offer.
6. **Signing and notarising** the bundle needs an Apple developer account; until then, macOS asks to confirm the first opening (right click, Open).
