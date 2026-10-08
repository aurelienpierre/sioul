# Building and running

## What it needs
- **Rust** 1.89 or newer.
- **D-Bus** development files, for the system keyring and notifications.
- **pcsc-lite**'s development files on Linux and BSDs, for security keys (OpenPGP cards: a YubiKey): `pcsc-sys` links its client, found by pkg-config. Windows and macOS have theirs built in (WinSCard, the PCSC framework); Android needs none. To use a key, the smart card service must run: `pcscd` with its CCID driver (Fedora `pcsc-lite` and `pcsc-lite-ccid`, then `sudo systemctl enable --now pcscd.socket`; Debian and Ubuntu `pcscd`; Arch `pcsclite` and `ccid`).
- **For the window only: Qt 6.9 or newer** (built and used with 6.11), with its development files: Qt Declarative (QML, Qt Quick, Controls, Layouts, Dialogs, Effects), Qt WebEngine with Qt WebChannel and Qt PDF, Qt Multimedia, Qt Positioning, Qt Location and Qt Image Formats. Sites keep their permissions with Qt WebEngine's permission API, long menus scroll as items of the window (both Qt 6.8), and each site's profile is made from a prototype (`WebEngineProfilePrototype`, Qt 6.9). The command line builds without Qt.

| System | Packages |
|---|---|
| Fedora | `dbus-devel`, `pcsc-lite-devel`, `qt6-qtbase-devel`, `qt6-qtdeclarative-devel`, `qt6-qtwebengine-devel`, `qt6-qtwebchannel-devel`, `qt6-qtpdf-devel`, `qt6-qtpositioning-devel`, `qt6-qtmultimedia`, `qt6-qtlocation`, `qt6-qtimageformats`; to check attachments, `clamav` and `clamav-update`; to read paper letters, `tesseract` |
| Debian, Ubuntu, Arch | the D-Bus and pcsc-lite development files (Debian, Ubuntu: `libpcsclite-dev`; Arch: `pcsclite`), and Qt 6.9 or newer with the modules above and their QML modules; distributions name them differently (Debian: `qt6-…-dev`, `qml6-module-…`; Arch: `qt6-…`) |

## The command line
```
cargo build --release
./target/release/sioul --help
```
`cargo build` and `cargo test` build the core, sync and the command line; the window is left out unless asked for, so machines without Qt can work on the rest.

## Which build
Each build knows its version and the commit it was made from (`sioul_core::build`: `VERSION`, `COMMIT`, `DESCRIBED`, "0.0.3 (eff8661abcde)"), and says them: `sioul --version`; Settings ▸ Your folder and sharing ("This device: Sioul 0.0.3 (eff8661abcde)."); this device's entry among your devices, which the others show under its name; the first line of each batch of its records ([database.md](database.md), "Devices", "The log"). `crates/sioul-core/build.rs` finds the commit:
1. `git rev-parse --short=12 HEAD`, with "-dirty" after it when tracked files hold changes not committed (`git status --porcelain --untracked-files=no`: files git does not track never count);
2. else the `.git` folder read directly (HEAD, the branch it names, the packed references), for a build where `git` cannot run, or refuses a checkout another user owns (a container's); never "-dirty" then;
3. else `SIOUL_COMMIT` from the environment (letters, digits, `-` and `.` only), for a build from a copy without the repository's history;
4. else "unknown".

The workflows (`.github/workflows/*.yml`, `actions/checkout`, shallow) and the phone's build (`~/Android/build.sh`, `android/CMakeLists.txt` through Corrosion) build from the checkout itself, and find it the first way. The Flatpak copies the checkout as its source (`type: dir`), its `.git` among it unless skipped: the first or second way; `SIOUL_COMMIT` in the manifest's `build-options` would do where neither does (not needed so far, not checked inside flatpak-builder).

**When it is looked at again**: when the commit changes (HEAD, its branch, the packed references), when `SIOUL_COMMIT` changes, when a file of `sioul-core` changes. Not when another crate's file changes: watching the whole tree would build every crate again at each edit, and `.git/index` changes at each `git status`. So "-dirty" says the tree held changes when `sioul-core` was last built; a change made since, in another crate alone, does not add it. `touch crates/sioul-core/build.rs` before a build makes sure.

## Windows and macOS
- **Windows**: Rust (rustup, MSVC), Qt 6.9 or newer from Qt's online installer (MSVC 2022 64-bit, with the modules above), with Qt's `bin` folder in `PATH`; then the same `cargo build --release -p sioul-app`. `windeployqt --release --qmldir crates\sioul-app\qml target\release\sioul-app.exe` gathers Qt beside the program, and `packaging\windows\sioul.iss` makes an installer with Inno Setup. Attachments are checked by Microsoft Defender through AMSI.
- **macOS**: see [packaging/macos/README.md](../packaging/macos/README.md).
- **Android**, an experiment: see [android.md](android.md).
- **Checking both from Linux** is done by the workflow `.github/workflows/build.yml`: on each push to `main` that touches code it builds and tests Sioul on Linux, Windows and macOS with Qt 6.11; started by hand (Actions, "Build on three systems", "Run workflow"), it also makes a Windows folder with Qt beside the program and a macOS `.dmg`.

## The window
```
cargo build --release -p sioul-app
./target/release/sioul-app
```
To find it in the application menu:
```
install -Dm755 target/release/sioul-app ~/.local/bin/sioul-app
install -Dm755 target/release/sioul ~/.local/bin/sioul
install -Dm644 data/com.aurelienpierre.Sioul.desktop ~/.local/share/applications/com.aurelienpierre.Sioul.desktop
install -Dm644 data/com.aurelienpierre.Sioul.metainfo.xml ~/.local/share/metainfo/com.aurelienpierre.Sioul.metainfo.xml
mkdir -p ~/.local/share/icons && cp -r data/icons/hicolor ~/.local/share/icons/
```

## Measuring the load
`tools/measure-load.py <sioul-app pid> [<watcher pid>|0] [seconds] [out]` samples Sioul every five seconds: its own process and every process under it (Qt WebEngine's zygotes, each site's renderers, its utility processes), CPU from `/proc/<pid>/stat`, memory as PSS (shared pages counted once), USS and RSS, threads, context switches and disk I/O; it writes a CSV and a JSON summary. On 6 October 2026 (8 cores, Linux, Qt 6.11, the release build of 5d80878):
- **an invented profile, no site open** (the demo profile, offscreen, five minutes): one process, about 70 MB, CPU 0.2 % of one core on average (peak 3 %): Qt WebEngine starts with the first site opened, and each page of the window, each form and menu, with its first use. On 4 October it was four processes and about 280 MB.
- **daily use** (a real mailbox, notes and tasks; three sites open in real time; three minutes, the window then closed): 8 processes, about 1.15 GB in all (`sioul-app` 0.39 GB, from 0.38 to 0.45; three renderers 0.7 GB; zygotes and the audio service 0.05 GB); CPU median 0.4 % of one core, mean 4 %, two samples above 20 % (up to 40 %); `sioul-app` read 3 MB and wrote 4 MB, closing included. On 4 October: about 1.9 GB (`sioul-app` 0.7 GB, five renderers 1.1 GB), CPU median 5.5 %, mean 9.4 %, peaks to 76 %.
- The reminder watcher (`sioul remind --watch`): 10 MB, 0.16 % of one core.
- A phone (Gigaset GS290, Android 12): `adb shell dumpsys meminfo` gives about 200 MB on the screen and 140 MB once put away (graphics given back, [android.md](android.md)); the window's log times each page's first opening (`adb logcat | grep sioul-perf`: 0.1 to 0.5 s).
Not counted: GPU memory. Still to look at: what of `sioul-app`'s 0.4 GB in daily use is the sites' browser profiles (they live in the window's process), what the real mail and notes, and what the pages opened.

## The icon
The drawings are in `data/icons/`: `sioul.svg`, `sioul-small.svg` (drawn for 32 pixels and under), `sioul-symbolic.svg` (one colour, the desktop's), and the quill alone for pages (`sioul-mark.svg`, `sioul-mark-on-dark.svg`). `tools/make-icons.py` (needs `rsvg-convert`) makes from them the icon theme folders a Linux desktop reads (`data/icons/hicolor/`, under the application's id, `com.aurelienpierre.Sioul`, as its desktop file, `data/com.aurelienpierre.Sioul.desktop`, and its AppStream file, `data/com.aurelienpierre.Sioul.metainfo.xml`, which software centres read), the Windows `.ico` and the macOS `.icns` in `packaging/`, and the website's favicon and logo; the window carries the PNGs (`crates/sioul-app/app.qrc`, `cpp/appicon.cpp`). Run it again after changing a drawing.

## First steps
1. **Add your mail accounts**, in the window (Accounts, the person at the bottom of the side bar) or from the terminal:
   ```
   sioul account add you@example.org
   ```
   Sioul finds the server from the address (your provider's own settings, then Thunderbird's list of providers), says what it found, asks for the password, tests it, keeps it in your system keyring, and fetches the last 14 days. Gmail wants an app password (myaccount.google.com/apppasswords).
2. **Web-only mailboxes**, chats and other sites open in Sites (Ctrl+4), each logged in once in its own lasting profile ([sites.md](sites.md)); from the terminal, `sioul account portal proton https://mail.proton.me` adds one.
3. **Keep the window open**, or run `sioul watch` in a terminal: each account's inbox stays open (IMAP IDLE), and the codes and links you ask sites for become quiet notifications within seconds.
4. **Install an antivirus** to have attachments checked before they open (without one, Sioul asks first): `sudo dnf install clamav clamav-update`, then `sudo freshclam` for its signatures and `sudo systemctl enable --now clamav-freshclam` to keep them current.
5. **Choose your hours** in Settings ▸ Hours: working hours, hours for your admin; every other time is leisure, meals and sleep come from the Health page ([areas.md](areas.md)); `[[window]]` in `~/.config/sioul/config.toml` holds them ([examples/config.toml](../examples/config.toml)). Without any, the Porch is always open.

## Where things are
| What | Where |
|---|---|
| Configuration | `~/.config/sioul/config.toml` |
| Senders you let in | `~/.config/sioul/known-senders.txt` |
| Passwords | the system keyring, service `sioul` |
| Mail, one Maildir per account, a Maildir++ subfolder per folder | `~/.local/share/sioul/mail/<account>`, `…/<account>/.Sent` |
| Drafts, with a copy of the message each answers | `~/.local/share/sioul/drafts/<draft>.toml`, `<draft>.eml` |
| Where sync stopped, per folder | `~/.local/state/sioul/sync/<account>.toml` |
| An account's folders, as last listed | `~/.local/state/sioul/sync/<account>.folders.toml` |
| Where the Porch was closed | `~/.local/state/sioul/porch.toml` |

Fetching never changes the server: folders are opened with EXAMINE and messages fetched with BODY.PEEK. Sioul writes to the server only when you act: opening a message in the window marks it read, as any client does; archiving, deleting, junking and moving happen ten seconds after you asked, so "Undo" can stop them; sending goes through your provider's SMTP server, and a copy is filed in Sent. What you do elsewhere (read on the phone, deleted in the webmail) comes back at each sync. Removing an account removes its password, not its mail.

Contacts and calendars live in `~/.local/share/sioul/contacts/<account>/<address book>/` and `~/.local/share/sioul/calendars/<account>/<calendar>/`, one file per contact or event, readable by khal, khard, pimsync and vdirsyncer. From the terminal: `sioul dav add you@example.org` (the server is found from the address, or `--url`), `sioul dav sync`, `sioul contacts [name]`, `sioul contact --name … --email …`, `sioul agenda [--days 15]`, `sioul event --title … --start 2026-10-05T09:00 [--end …] [--all-day] [--repeat weekly]`. OpenPGP: `sioul pgp list|make <address>|import <file>|export <fingerprint>|lookup <address>`, and `sioul mail send … --sign --encrypt`.

From the terminal, the same without the ten seconds:
```
sioul mail folders <account>
sioul mail list <account> [folder]
sioul mail act <file> read|unread|flag|unflag|archive|trash|junk|not-junk|move --to <folder>
sioul mail send --account <account> --to <address> --subject <subject> --body <markdown> [--attach <file>]
sioul mail send --account <account> --reply <file> --body <markdown>     # also --reply-all, --forward
```

## Working on the window
- **Qt's messages**: Fedora's Qt sends them to the system journal; `QT_FORCE_STDERR_LOGGING=1` brings them back to the terminal.
- **Images of each page**: with `SIOUL_GRAB=<folder>`, the window shows each page in turn (a message opened on the Porch included), saves it as `<page>.png` and quits; `SIOUL_GRAB_STEPS` names another step list. For a check, the runner sets them (below).
- **A demo, off the network**: `SIOUL_DEMO=1` keeps Sioul from fetching or sending anything by itself (mail, calendars, GitHub, weather, site icons, antivirus signatures, addresses on the map); it shows what is kept on the computer. `tools/demo/make-demo.py --into <folder>` writes an invented profile for it, and `tools/demo/screenshots.sh` takes the website's pictures from it.
- **Acting on mail in the window**: `SIOUL_GRAB_STEPS=actions` with `SIOUL_GRAB` archives a message and undoes it, deletes another, writes and sends one, saving an image at each step. It writes to the server: only from the test build, against a test server (below).
- **Linting the QML**: `tools/lint-qml.sh`, after `cargo build -p sioul-app`.
- **The window's buttons**: on a computer, Sioul draws its own title bar, the window's buttons on the side and in the order the desktop uses, read once at start, off the window's thread (`src/desktop.rs`, `system_layout`). `SIOUL_BUTTON_LAYOUT`, in GNOME's format (`close,minimize,maximize:` puts them on the left), comes first, to try a layout the desktop does not have. On KDE Plasma, kwinrc's `[org.kde.kdecoration2]` `ButtonsOnLeft` and `ButtonsOnRight`, the strongest file first (`$XDG_CONFIG_HOME`, a Flatpak's `$HOST_XDG_CONFIG_HOME`, `~/.config`, each one's `kdedefaults/`, then `$XDG_CONFIG_DIRS`), else Plasma's defaults, `MS` and `HIAX` (I minimises, A maximises, X closes; the other letters are ignored). On Xfce, `xfconf-query -c xfwm4 -p /general/button_layout`. On GNOME, Unity, Budgie, Pantheon, Cinnamon and MATE, `gsettings get <schema> button-layout`, Cinnamon's and MATE's own schemas before GNOME's, and GNOME's own default, close alone on the right, when nothing answers. Elsewhere (sway, i3…), minimise, maximise and close on the right; on a Mac, close, minimise and maximise on the left; on Windows, on the right. In a Flatpak, `gsettings` may read the sandbox's settings rather than the desktop's (untried); the settings portal would read the desktop's. A right click on the bar does nothing: Qt 6.11 has no public call for the window manager's menu (X11's `_GTK_SHOW_WINDOW_MENU` would work on KWin; Wayland needs Qt's private Wayland client).

## Running the window for a check
`tools/demo/run.sh [--release] OUT en|fr STEPS [phone]` is the one way to start the window for a check, for a person as for an agent: never `sioul-app` on your own desktop session, nor on your own profile. Started there, a window under test reaches your desktop (the system tray, notifications, D-Bus names, do-not-disturb) and your mail and files. The runner:
- **makes a demo profile**, anew, in `OUT/profile` (`tools/demo/make-demo.py`), on a weekday afternoon whatever the day (`tools/demo/moment.py`; `RUN_HOUR=19` for 19:xx instead), in English or French;
- **starts the window in a sandbox** (bubblewrap; it refuses to run without): the real home out of reach, the profile as the home of a user "demo", the repository read-only; no network (a network namespace of its own, and `SIOUL_DEMO=1`); no display (offscreen, software rendering); a D-Bus session and a runtime folder of its own, the session's own runtime folder hidden; the fonts of `tools/demo/fonts.conf`; 400 seconds at most. The debug build, or the release one with `--release`;
- **goes through the step list** STEPS, its pictures in `OUT/shots/` and the window's messages in `OUT/shots/log`; `phone` gives the window a phone's size and layout;
- **says what went wrong**: it prints the log's lines from QML and its errors, and exits with 1 when the window crashed or was stopped, or a script error is in the log.

**The step lists** are `grabber.steps` in `main.qml`, each a series of steps a moment apart. Which a build allows is `grab_steps` in `crates/sioul-app/src/backend.rs`: "pages" (each page, the default) always; "demo", "phone" and "drag", pictures alone; on a demo profile only (`SIOUL_DEMO=1`, which the runner sets), the lists that act in the window: "taskform", "tiles", "review", "rail", "pauses", "blocks", "unsubscribe", "attention", "line", "share-panel", "share-send", "spam", "mail-search", "mail-filters", "security-key", "calls", "words", "movetask", "compose", "attachments" (a message's attachments unfolded in the Reader, a phone's words with `phone`, nothing opened), "projects-file" (with `SIOUL_DEMO_OLD_PROJECTS=1`: the projects' file under its first name, renamed from Settings, then a project's page and the form of a new one), and the four of the sites, which `tools/check-sites.py` runs in a sandbox of its own; every other list archives, sends or syncs, from the test build only, against the test servers below, on test folders, never your own. A list a build does not allow runs "pages". "compose" answers a message with a subject of more than sixty characters, two addresses to answer and a copy, which `make-demo.py --compose` adds (`SIOUL_DEMO_COMPOSE=1 tools/demo/run.sh OUT fr compose phone`), then writes a new message and deletes it; with `phone`, the writing window takes the phone's size too, and each of its pictures names what passes its right edge. `tools/demo/screenshots.sh` takes the website's pictures with "demo", in a sandbox of its own, on the runner's moment and fonts.

**Before installing a release build**, a maintainer starts each batch's release build once on a real desktop session, with a throwaway profile: the runner shows nothing of what the desktop itself draws (the system tray, the title bar's buttons where the desktop puts them, its notifications and its do-not-disturb).

## The checks before a commit
- **All at once**: `tools/final-pass.sh [FOLDER]`, from the repository alone, each step's log in FOLDER (a new folder under `~/.cache/sioul-final-pass/` by default): the JVM checks; each crate's tests in release, one crate at a time, each on a D-Bus session of its own; the two programs in release, and no test hook in them; the messages (`tools/check-messages.py`); QML lint and the QML tests; rustdoc with warnings as errors, as the website's API reference. It stops before a build when less than 1.5 GB is free where `target/` is (`MIN_FREE_GB` sets another floor), and exits with 1 when a step failed. It never starts the window: the runner does (above).
- **The QML tests**: `tools/qml-test.sh [tst_name.qml…]` runs `crates/sioul-app/tests/qml/tst_*.qml` with Qt 6's runner (Fedora's `qmltestrunner-qt6`: a bare `qmltestrunner` there is Qt 5's), offscreen, each component on a stand-in for Sioul that records the calls it makes: the clock and the line at now in the day views, the timelines' drags by mouse and by touch, the costs' tiles and "How was it?", the Bitwarden login chooser, do-not-disturb's switch and its parts in What reaches you and This phone, a medicine's takes as rows with their time picker, a line's commands apart to copy, and the phone's messages on a computer's Porch (Seen and its undo) with the phone's tab for them. Each test imports the window's QML folder by its relative path (`import "../../qml"`): no build is needed.
- **The phone's Java on a computer**: `android/jvm-checks/run.sh` ([android.md](android.md#building-it)).
- **A notification end to end**: `tools/e2e/notifications/run.sh buttons|quit|elsewhere`, the real window in the runner's sandbox against a stand-in notification server ([A feature, end to end](end-to-end.md#1-the-core-the-rule-and-the-file)).

## Testing against a local mail server
Writing to a server and sending are tested against [GreenMail](https://greenmail-mail-test.github.io/greenmail/), never against real mailboxes. It accepts any login and makes the accounts as mail comes:
```
podman run -d --name sioul-greenmail -e GREENMAIL_OPTS='-Dgreenmail.setup.test.all -Dgreenmail.hostname=0.0.0.0 -Dgreenmail.auth.disabled' \
  -p 127.0.0.1:3993:3993 -p 127.0.0.1:3465:3465 -p 127.0.0.1:3025:3025 -p 127.0.0.1:3143:3143 docker.io/greenmail/standalone:2.1.8
```
Its certificate is its own, so the test build accepts any certificate, and only when `SIOUL_TEST_INSECURE_TLS` is set; `SIOUL_TEST_PASSWORD` stands in for the keyring. **Never build Sioul for use with this feature.**
```
cargo build -p sioul-cli -p sioul-app --features sioul-cli/insecure-test-tls,sioul-app/insecure-test-tls
export XDG_CONFIG_HOME=/some/test/folder/config XDG_DATA_HOME=/some/test/folder/data XDG_STATE_HOME=/some/test/folder/state
export SIOUL_TEST_INSECURE_TLS=1 SIOUL_TEST_PASSWORD=any
```
A test account in `$XDG_CONFIG_HOME/sioul/config.toml`: `host = "localhost"`, `port = 3993`, `security = "tls"`, `smtp_host = "localhost"`, `smtp_port = 3465`, `smtp_security = "tls"`. Mail goes in through port 3025 without TLS (Python's `smtplib` will do), and can be looked at on port 3143.

Contacts and calendars are tested against [Radicale](https://radicale.org), in a Python virtual environment, without authentication:
```
python3 -m venv /some/test/folder/radicale && /some/test/folder/radicale/bin/pip install radicale
/some/test/folder/radicale/bin/radicale --server-hosts 127.0.0.1:5232 --auth-type none --storage-filesystem-folder /some/test/folder/radicale-data
```
Radicale makes no collection by itself: make a calendar with `MKCALENDAR` and an address book with an extended `MKCOL` (curl will do), then `sioul dav add you@example.org --url http://127.0.0.1:5232/`. The test build allows plain HTTP to this computer only, and only with `SIOUL_TEST_INSECURE_TLS`. `SIOUL_GRAB_STEPS=pim` answers an invitation, writes a weekly event and changes a contact in the window.

Google Tasks and GitHub are tested against stand-ins of their APIs, in the test build only: `tools/google-tasks-stand-in.py 39871` with `SIOUL_TEST_GOOGLE_TASKS=http://127.0.0.1:39871 SIOUL_TEST_GOOGLE_TOKEN=test-token` (`cargo test -p sioul-sync --features insecure-test-tls -- --ignored stand_in`), and `tools/github-stand-in.py 39872` with `SIOUL_TEST_GITHUB=http://127.0.0.1:39872 SIOUL_TEST_GITHUB_TOKEN=test-token` and `[github] enabled = true` (`SIOUL_GRAB_STEPS=github`). Google's own servers, its sign-in and GitHub itself are not reached by any test.

OpenPGP is tested against GnuPG in a throwaway `GNUPGHOME`: `sioul pgp make you@example.org` makes Alice's key (the test build gives every key `SIOUL_TEST_PASSWORD` as passphrase and leaves the keyring alone), `gpg --quick-generate-key` makes Bob's; each imports the other's public key (`sioul pgp import`, `sioul pgp export`, `gpg --import`); `sioul mail send --sign --encrypt` goes to Bob, whose GnuPG must decrypt it and find a good signature; messages built with GnuPG come back to Alice, and `SIOUL_GRAB_STEPS=pgp` shows what the reader says of them.
