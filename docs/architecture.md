# Architecture

## Layers
- **`sioul-core`, a Rust library**: reading and judging mail, cases, the Porch, admin windows, contacts and calendars, tasks (`tasks`) and their plan (`plan`), notes (`notes`), links between everything (`links`), time spent (`timelog`), budgets, health, the spam filter every device runs (`spam`: its tokenizer, the messages' features, the table that scores them), translations. Everything the interface shows is decided and worded here (`view`, `taskview`), so the command line, the Qt interface and AI agents see the same thing.
- **`sioul` (`crates/sioul-cli`)**: the command line, first because agents and scripts use it too.
- **The Qt 6 interface (QML) through CXX-Qt** (KDAB; bridge API stable since 0.7, 0.10 in 2026). The sites (once called portals) use Qt WebEngine, on computers. The interface holds no logic. It is a library (`crates/sioul-app/src/lib.rs`), which the desktop program and Android's both run.
- **`sioul-sync`, the background service's library**: finding servers, the keyring, IMAP sync and what you do to messages on the server, sending through SMTP, IDLE watchers, the notifications, CalDAV and CardDAV, Google, GitHub, Bitwarden, the sharing between your devices (`share`). The window runs its watchers; without it, `sioul watch` keeps the inboxes open and `sioul remind --watch` tells reminders; on Android, a service of its own keeps the phone in step ([android.md](android.md#in-the-background)).
- **`sioul-learn`, on computers only**: the spam filter's training, on your own mail; it writes the table every device reads. Still being built.
- **Android** (`android/`): Qt for Android builds the window into an app; Java classes run what Android starts while the window is away (alarms, a background service, call screening, other apps' notifications), and ask Rust through `android/main.cpp` ([android-basics.md](android-basics.md), [android.md](android.md)).
- **An MCP server** for agents ([ai.md](ai.md)).

New to Rust, Qt Quick, CXX-Qt, Fluent or Android? [Start here](start-here.md) maps the code, and the guides beside it explain each tool as Sioul uses it, up to [a feature followed end to end](end-to-end.md).

## Storage: plain files wherever possible
- **Mail**: Maildir, readable by notmuch and mutt.
- **Calendars and contacts**: vdir, one `.ics` or `.vcf` per item, as pimsync and khal use.
- **Tasks**: VTODO files in the calendars' vdir; their links inside them (RFC 9253). **Links between things that cannot hold them**: `links.toml`. **Time spent**: one TOML file per month. **Today's choices** (weather, "not now"): `today.toml` in the state folder. SQLite and tantivy wait until plain files are too slow.
- **History**: a file another device changes is kept as it was, on this device, to be put back ([database.md](database.md#earlier-versions)).
- **Between your devices**: what only Sioul keeps (settings, senders, time, health…) travels as sealed records, one file per device, through a folder your sync app carries ([database.md](database.md)).
- **Cases**: your own Markdown files ([case-store.md](case-store.md)).

## Protocols
- **Mail**: IMAP (RFC 9051, IDLE, CONDSTORE/QRESYNC), JMAP (RFC 8620/8621), SMTP submission.
- **Calendars and contacts**: CalDAV (RFC 4791) and CardDAV (RFC 6352), through a small client of Sioul's own (`sioul_sync::dav`). Tasks are VTODO with RFC 9253 relationships.
- **Discovery**: Mozilla ISPDB and providers' autoconfig, SRV records (RFC 6186), DAV well-known URLs (RFC 6764).
- **What is built of these**, what each feature needs from a server or another program, and how far each was tried: [compatibility.md](compatibility.md). Not built yet: CONDSTORE and QRESYNC, JMAP, SRV records.

## Crates, and their licences
| Use | Crate | Licence |
|---|---|---|
| Parsing mail | `mail-parser` | Apache-2.0 or MIT |
| Settings, state, the window's JSON | `serde`, `serde_json`, `toml` | MIT or Apache-2.0 |
| IMAP | `async-imap`, on `tokio` | MIT or Apache-2.0; MIT |
| TLS | `rustls` and `tokio-rustls` with `ring`; `rustls-native-certs` | Apache-2.0, ISC or MIT |
| Mozilla's certificate list, when a system has none | `webpki-roots` | CDLA-Permissive-2.0 (data) |
| HTTPS: finding servers, CalDAV and CardDAV, Google, GitHub, Bitwarden, the weather | `ureq` | MIT or Apache-2.0 |
| Notifications | `notify-rust`; `dbus`, for the one that changes in place on Linux | MIT or Apache-2.0 |
| Editing the configuration in place | `toml_edit` | MIT or Apache-2.0 |
| Password prompt | `rpassword` | Apache-2.0 |
| The command line's arguments | `clap` | MIT or Apache-2.0 |
| DKIM, SPF, DMARC, ARC, reverse DNS (with `hickory-resolver`) | `mail-auth` | Apache-2.0 or MIT |
| HTML mail made safe | `ammonia` (on `html5ever`) | MIT or Apache-2.0 |
| Antivirus for attachments | the system's ClamAV (Defender on Windows, through `windows`), optional | GPL-2.0 (a separate program, not shipped); MIT or Apache-2.0 |
| Writing: Markdown to HTML, the message built | `pulldown-cmark`; `mail-builder` | MIT; Apache-2.0 or MIT |
| Sending | `mail-send` | Apache-2.0 or MIT |
| CalDAV, CardDAV | Sioul's own small client (`sioul_sync::dav`), on `ureq` | — |
| XML: WebDAV's answers, banks' exports | `roxmltree` | MIT or Apache-2.0 |
| iCalendar, vCard | `calcard`, with `chrono` and `chrono-tz` for its time zones | Apache-2.0 or MIT; MIT or Apache-2.0 |
| Qt | `cxx-qt`, `cxx-qt-lib`, `cxx-qt-build`, on `cxx` | MIT or Apache-2.0 |
| PGP | `sequoia-openpgp` | LGPL-2.0-or-later |
| Keyring: the Secret Service (KWallet, GNOME Keyring), Windows' Credential Manager, macOS's Keychain, Android's KeyStore | `keyring` 3; on Android, `android-keyring` with `ndk-context` | MIT or Apache-2.0; MIT; MIT or Apache-2.0 |
| Cryptography: Bitwarden's key derivation and encryption, its one-time codes (RFC 6238); the sharing's seal; Google's sign-in | `argon2`, `hkdf`, `hmac`, `sha1`, `sha2`, `aes`, `cbc`, `aes-gcm`, `chacha20poly1305`, `rsa`, `ciborium`, `zeroize`, `subtle` | MIT or Apache-2.0; `ciborium`: Apache-2.0; `subtle`: BSD-3-Clause |
| Base64; the devices' identifiers | `base64`, `uuid` | MIT or Apache-2.0 |
| Compression: notes and papers in the sharing folder, the spam filter's corpus | `flate2` | MIT or Apache-2.0 |
| Unicode normalisation: names of notes, the spam filter's words | `icu_normalizer` | Unicode-3.0 |
| The spam filter: its tokenizer's patterns; its language model, trained on computers (`sioul-learn`) | `regex`; `fasttext` | MIT or Apache-2.0; MIT |
| Phone numbers | Sioul's own table (`sioul_core::phones`) | — |
| Each system's folders (Windows, macOS) | `directories` | MIT or Apache-2.0 |
| The free space on a disk | `rustix` (Unix), `windows` | Apache-2.0 (with LLVM exception) or MIT; MIT or Apache-2.0 |
| Translations | `fluent-bundle`, `unic-langid` | Apache-2.0 or MIT |
| Dates | `jiff` | Unlicense or MIT |
| The Windows program's icon and details | `winresource` | MIT |
| Python filters (Virtual Secretary, mode B) | a separate process | — |

Qt is LGPL-3.0. Every licence above is compatible with Sioul's, GPL-3.0-or-later.

## Platforms
- **Linux first**: a Flatpak and an AppImage, made for each version by `.github/workflows/release.yml`.
- **Windows**: an installer, Qt beside the program. Maildir puts `:` in file names, which Windows forbids: there Sioul writes `!` instead, as mbsync does, and reads both (`sioul_core::maildir::INFO`).
- **macOS**: one `.dmg` for Apple silicon and Intel, signed ad hoc, not notarised.
- **Android**, an experiment, packaged since version 0.0.2: an APK for 64-bit phones, Android 9 and later. Qt for Android, with the same window; sites open in the browser and PDFs in another app, since Qt WebEngine and Qt PDF do not exist there; Java for what Android starts without the window ([android-basics.md](android-basics.md), [android.md](android.md)).
- **Built on each push to `main`** that changes the code: `.github/workflows/build.yml` builds and tests on Linux, Windows and macOS, `.github/workflows/android.yml` builds the APK.

## Known hurdles
- **Gmail**: full IMAP through OAuth is a restricted scope. An unverified app is capped at 100 users for its whole life, and verification needs a paid security assessment (CASA). App passwords work meanwhile.
- **Proton**: IMAP needs Bridge, a paid feature. On the free plan Proton is a portal.
- **Spamhaus**: queries through public DNS resolvers are refused; the free Data Query Service key works.

## Code style
- One task per function; early exits only for "nothing to do".
- Comments give the reason and the reference: the RFC, the paper, the document.
- Every rule the interface applies has a test, on invented mail using reserved example domains (RFC 2606) and no personal data.
