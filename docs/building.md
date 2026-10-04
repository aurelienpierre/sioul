# Building and running

## What it needs
- **Rust** 1.89 or newer.
- **D-Bus** development files, for the system keyring and notifications.
- **For the window only: Qt 6.9 or newer** (built and used with 6.11), with its development files: Qt Declarative (QML, Qt Quick, Controls, Layouts, Dialogs, Effects), Qt WebEngine with Qt WebChannel and Qt PDF, Qt Multimedia, Qt Positioning, Qt Location and Qt Image Formats. Sites keep their permissions with Qt WebEngine's permission API, long menus scroll as items of the window (both Qt 6.8), and each site's profile is made from a prototype (`WebEngineProfilePrototype`, Qt 6.9). The command line builds without Qt.

| System | Packages |
|---|---|
| Fedora | `dbus-devel`, `qt6-qtbase-devel`, `qt6-qtdeclarative-devel`, `qt6-qtwebengine-devel`, `qt6-qtwebchannel-devel`, `qt6-qtpdf-devel`, `qt6-qtpositioning-devel`, `qt6-qtmultimedia`, `qt6-qtlocation`, `qt6-qtimageformats`; to check attachments, `clamav` and `clamav-update`; to read paper letters, `tesseract` |
| Debian, Ubuntu, Arch | the D-Bus development files, and Qt 6.9 or newer with the modules above and their QML modules; distributions name them differently (Debian: `qt6-…-dev`, `qml6-module-…`; Arch: `qt6-…`) |

## The command line
```
cargo build --release
./target/release/sioul --help
```
`cargo build` and `cargo test` build the core, sync and the command line; the window is left out unless asked for, so machines without Qt can work on the rest.

## Windows and macOS
- **Windows**: Rust (rustup, MSVC), Qt 6.9 or newer from Qt's online installer (MSVC 2022 64-bit, with the modules above), with Qt's `bin` folder in `PATH`; then the same `cargo build --release -p sioul-app`. `windeployqt --release --qmldir crates\sioul-app\qml target\release\sioul-app.exe` gathers Qt beside the program, and `packaging\windows\sioul.iss` makes an installer with Inno Setup. Attachments are checked by Microsoft Defender through AMSI.
- **macOS**: see [packaging/macos/README.md](../packaging/macos/README.md).
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
`tools/measure-load.py <sioul-app pid> [<watcher pid>|0] [seconds] [out]` samples Sioul every five seconds: its own process and every process under it (Qt WebEngine's zygotes, each site's renderers, its utility processes), CPU from `/proc/<pid>/stat`, memory as PSS (shared pages counted once), USS and RSS, threads, context switches and disk I/O; it writes a CSV and a JSON summary. On 4 October 2026 (8 cores, Linux, Qt 6.11), over five minutes:
- **an invented profile, no site open** (the demo profile, offscreen): 4 processes, about 280 MB, no CPU at rest;
- **daily use** (a real mailbox, notes and tasks; Proton Mail, Discord and OkCupid open in real time): 10 processes, about 1.9 GB in all (`sioul-app` 0.7 GB, five renderers 1.1 GB, zygotes and the audio service 0.05 GB); CPU median 5.5 % of one core, mean 9.4 %, four peaks above 20 % (up to 76 %) in five minutes; the busiest renderer (a chat) about 4.5 % at rest; no growth over the five minutes; about 4 MB read and written.
- The reminder watcher (`sioul remind --watch`): 10 MB, 0.15 % of one core.
Not counted: GPU memory. Still to look at: why `sioul-app` holds 0.7 GB with real data and three sites against 0.25 GB on the demo profile (the sites' browser profiles live in the window's process, and so does every page's QML).

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
5. **Choose your hours** in Settings ▸ Hours: working hours, hours for your admin, free time ([areas.md](areas.md)); `[[window]]` in `~/.config/sioul/config.toml` holds them ([examples/config.toml](../examples/config.toml)). Without any, the Porch is always open.

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
- **Images of each page**: `SIOUL_GRAB=<folder> sioul-app` shows each page in turn (a message opened on the Porch included), saves it as `<page>.png` and quits. Without a screen: add `QT_QPA_PLATFORM=offscreen QT_QUICK_BACKEND=software`. With `XDG_CONFIG_HOME`, `XDG_DATA_HOME` and `XDG_STATE_HOME` pointed at a test folder, it shows invented mail instead of yours.
- **A demo, off the network**: `SIOUL_DEMO=1` keeps Sioul from fetching or sending anything by itself (mail, calendars, GitHub, weather, site icons, antivirus signatures, addresses on the map); it shows what is kept on the computer. `tools/demo/make-demo.py --into <folder>` writes an invented profile for it, and `tools/demo/screenshots.sh` takes the website's pictures from it.
- **Acting on mail in the window**: `SIOUL_GRAB_STEPS=actions` with `SIOUL_GRAB` archives a message and undoes it, deletes another, writes and sends one, saving an image at each step. It writes to the server: only against a test server (below).
- **Linting the QML**: `tools/lint-qml.sh`, after `cargo build -p sioul-app`.

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
