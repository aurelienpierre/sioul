# Architecture

## Layers
- **`sioul-core`, a Rust library**: reading and judging mail, cases, the Porch, admin windows, contacts and calendars, tasks (`tasks`) and their plan (`plan`), notes (`notes`), links between everything (`links`), time spent (`timelog`), budgets, translations. Everything the interface shows is decided and worded here (`view`, `taskview`), so the command line, the Qt interface and AI agents see the same thing.
- **`sioul` (`crates/sioul-cli`)**: the command line, first because agents and scripts use it too.
- **The Qt 6 interface (QML) through CXX-Qt** (KDAB; bridge API stable since 0.7, 0.10 in 2026). The portals use QtWebEngine. The interface holds no logic.
- **`sioul-sync`, the background service's library**: finding servers, the keyring, IMAP sync and what you do to messages on the server, sending through SMTP, IDLE watchers, the notification for short-lived codes. The window runs its watchers today; a separate user service comes next.
- **An MCP server** for agents ([ai.md](ai.md)).

## Storage: plain files wherever possible
- **Mail**: Maildir, readable by notmuch and mutt.
- **Calendars and contacts**: vdir, one `.ics` or `.vcf` per item, as pimsync and khal use.
- **Tasks**: VTODO files in the calendars' vdir; their links inside them (RFC 9253). **Links between things that cannot hold them**: `links.toml`. **Time spent**: one TOML file per month. **Today's choices** (weather, "not now"): `today.toml` in the state folder. SQLite and tantivy wait until plain files are too slow.
- **History**: git for contacts and notes.
- **Cases**: your own Markdown files ([case-store.md](case-store.md)).

## Protocols
- **Mail**: IMAP (RFC 9051, IDLE, CONDSTORE/QRESYNC), JMAP (RFC 8620/8621), SMTP submission.
- **Calendars and contacts**: CalDAV (RFC 4791) and CardDAV (RFC 6352) through `libdav`. Tasks are VTODO with RFC 9253 relationships.
- **Discovery**: Mozilla ISPDB and providers' autoconfig, SRV records (RFC 6186), DAV well-known URLs (RFC 6764).
- **What is built of these**, what each feature needs from a server or another program, and how far each was tried: [compatibility.md](compatibility.md). Not built yet: CONDSTORE and QRESYNC, JMAP, SRV records.

## Crates, and their licences
| Use | Crate | Licence |
|---|---|---|
| Parsing mail | `mail-parser` | Apache-2.0 or MIT |
| IMAP | `async-imap`, on `tokio` | MIT or Apache-2.0; MIT |
| TLS | `rustls` and `tokio-rustls` with `ring`; `rustls-native-certs` | Apache-2.0, ISC or MIT |
| Mozilla's certificate list, when a system has none | `webpki-roots` | CDLA-Permissive-2.0 (data) |
| Finding servers (HTTPS) | `ureq` | MIT or Apache-2.0 |
| Notifications | `notify-rust` | MIT or Apache-2.0 |
| Editing the configuration in place | `toml_edit` | MIT or Apache-2.0 |
| Password prompt | `rpassword` | Apache-2.0 |
| DKIM, SPF, DMARC, ARC, reverse DNS (with `hickory-resolver`) | `mail-auth` | Apache-2.0 or MIT |
| HTML mail made safe | `ammonia` (on `html5ever`) | MIT or Apache-2.0 |
| Antivirus for attachments | the system's ClamAV (Defender on Windows), optional | GPL-2.0 (a separate program, not shipped) |
| Sending | `mail-send` | Apache-2.0 or MIT |
| CalDAV, CardDAV | `libdav` | ISC |
| iCalendar, vCard | `calcard` or `icalendar` (to compare) | Apache-2.0 or MIT |
| Qt | `cxx-qt`, `cxx-qt-lib` | MIT or Apache-2.0 |
| Search | `tantivy` | MIT |
| Database | `rusqlite` | MIT |
| PGP | `sequoia-openpgp` | LGPL-2.0-or-later |
| Keyring (Secret Service: KWallet, GNOME Keyring) | `keyring` 3 | MIT or Apache-2.0 |
| Phone numbers | `phonenumber` | Apache-2.0 |
| SVM | `linfa-svm` | MIT or Apache-2.0 |
| Translations | `fluent-bundle` | Apache-2.0 or MIT |
| Dates | `jiff` | Unlicense or MIT |
| Python filters (Virtual Secretary, mode B) | a separate process | — |

Qt is LGPL-3.0. Every licence above is compatible with Sioul's, GPL-3.0-or-later.

## Platforms
- **Linux first**, as a Flatpak.
- **Windows**: Qt is fine. Maildir puts `:` in file names, which Windows forbids, so another separator is needed, as mbsync allows.
- **Android**: Qt for Android, with QtWebView for the portals (the system WebView; QtWebEngine does not exist there). CXX-Qt on Android is to be tested early.

## Known hurdles
- **Gmail**: full IMAP through OAuth is a restricted scope. An unverified app is capped at 100 users for its whole life, and verification needs a paid security assessment (CASA). App passwords work meanwhile.
- **Proton**: IMAP needs Bridge, a paid feature. On the free plan Proton is a portal.
- **Spamhaus**: queries through public DNS resolvers are refused; the free Data Query Service key works.

## Code style
- One task per function; early exits only for "nothing to do".
- Comments give the reason and the reference: the RFC, the paper, the document.
- Every rule the interface applies has a test, on invented mail using reserved example domains (RFC 2606) and no personal data.
