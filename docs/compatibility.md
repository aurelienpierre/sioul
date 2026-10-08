# Compatibility: what Sioul needs from servers and programs

What Sioul relies on in each protocol, what a server or another program must do for each feature, and how far each was tried. Read from the code at 098a9d5 (6 October 2026), for developers and server admins. The same for people using Sioul, in plain words: the guide's [Works with](https://aurelienpierre.github.io/sioul/guide/compatibility.html).

Each line says how sure it is:
- **Tested**: run against the server, program or device named; where that is recorded is in the last section.
- **Expected**: built to the standard named, not run against that one.
- **Limit**: what breaks or degrades, and why.
- **Not supported**: not built.

## Mail: IMAP
Code: `crates/sioul-sync/src/imap.rs` (the session), `fetch.rs` (fetching, IDLE), `mailbox.rs` (folders, what you do to messages).

| What | Standard | Used for | Without it |
|---|---|---|---|
| IMAP | RFC 9051 (IMAP4rev2), RFC 3501 (IMAP4rev1) | everything below | — |
| TLS from the first byte (993), or STARTTLS (143) | RFC 8314; RFC 9051 §6.2.1 | every connection; certificates checked against the system's store (rustls, Mozilla's list when the system has none) | no plain connection: refused. A certificate the system does not trust (Proton Mail Bridge's own, a self-signed one) is refused too |
| LOGIN, with the password | RFC 9051 §6.2.3 | signing in | AUTHENTICATE XOAUTH2 for Google accounts signed in with Google (a key of the person's own: [google.md](google.md), "Mail"); OAUTHBEARER (RFC 7628) is not used. **Not supported**: Microsoft's Outlook.com and Hotmail (OAuth only since 16 September 2024) and Microsoft 365 (no Basic authentication for IMAP in Exchange Online): no Microsoft sign-in yet |
| EXAMINE; UID FETCH with BODY.PEEK[] | RFC 9051 §6.3.3, §6.4.5 | fetching without setting any flag | — |
| UID SEARCH (SINCE, UID n:*); UIDVALIDITY, UIDNEXT | RFC 9051 | what is new, by UID; a new UIDVALIDITY fetches the folder again | without UIDNEXT, a folder empty in the first window is searched again by date |
| RFC822.SIZE | RFC 9051 | older mail sized before it is fetched, against the disk's reserve ([client.md](client.md), rule 7) | — |
| IDLE | RFC 2177 | the inbox kept open, renewed every five minutes: a code within seconds | the inbox is looked at every two minutes (`POLL`) |
| MOVE | RFC 6851 | archive, trash, junk, move | UID COPY, then `\Deleted` and UID EXPUNGE |
| UIDPLUS (UID EXPUNGE) | RFC 4315 | removing only the message moved or deleted | without MOVE and UIDPLUS, the copy is made and the original stays marked `\Deleted`: a plain EXPUNGE could remove what another client marked. Most clients hide it |
| SPECIAL-USE | RFC 6154 | Sent, Drafts, Junk, Trash, Archive, All | folders found by name (`folders::preferred`); a Trash, Junk, Archive or Sent missing is created, under `INBOX.` where the server keeps its folders there |
| `\Flagged` and `\Important` views | RFC 6154, RFC 8457 | left out (Gmail's Starred and Important); Gmail's All Mail is the archive when no folder says `\Archive` | — |
| Keywords `$Junk`, `$NotJunk` | IANA's IMAP keywords (RFC 5788) | told to the server's spam filter on Junk, Not junk and Not spam; kept at each sync in the file name (Dovecot's letters), for the Porch and the spam filter | a server that refuses keywords still gets the move; "Not spam" is then kept in this device's label log only |
| CREATE, DELETE, STATUS (MESSAGES) | RFC 9051 | "New folder…"; deleting a folder only when it is empty and has no purpose | — |
| APPEND | RFC 9051 | the copy in Sent (`\Seen`); a message moved to another account, with its `\Seen`, `\Flagged` and `\Answered` | at Gmail (`imap.gmail.com`) no copy: Gmail files its own |
| Modified UTF-7 folder names | RFC 3501 §5.1.3 | names typed with accents | — |

Microsoft's rule for Outlook.com, "Basic Authentication no longer available to access any Outlook account" from 16 September 2024: [Modern Authentication Methods now needed to continue syncing Outlook Email in non-Microsoft email apps](https://support.microsoft.com/en-us/support/known-issues/modern-authentication-methods-now-needed-to-continue-syncing-outlook-email-in-non-microsoft-email-ap).

**Not used**: CONDSTORE and QRESYNC (RFC 7162): flags and deletions made elsewhere are found by asking the FLAGS of every message kept here, at each full round (`mailbox::reconcile`), which is slower on big folders; they are planned ([porch.md](porch.md), "Next"), though [architecture.md](architecture.md) lists them with the protocols. METADATA (RFC 5464), considered for the Porch's marks ([database.md](database.md), "Not built"). JMAP (RFC 8620, RFC 8621): an account kind in the configuration (`AccountKind::Jmap`), no client. POP3, Exchange (EWS, ActiveSync): none.

**What another client on the same mailbox sees** (Thunderbird, a phone's mail app, the webmail):
- Fetching sets no flag. Opening a message in the window sets `\Seen`, as any client does; read, unread and flag are `UID STORE` with `.SILENT`.
- Archive, delete, junk and move reach the server ten seconds after the act (the "Undo" window), as moves.
- Replying sets no `\Answered`, forwarding no `$Forwarded`: nothing stores them.
- Drafts stay on the device; the server's Drafts folder is fetched like any folder, never written ([database.md](database.md), "Not built").
- What Sioul alone knows (the Porch's lanes, senders' lists, links) stays on the device and travels by the sharing log, never in IMAP.

## Mail: SMTP
Code: `crates/sioul-sync/src/send.rs`, on `mail-send` 0.6.
- **Submission** (RFC 6409) on 465 with TLS from the first byte (RFC 8314), or on 587 with STARTTLS (RFC 3207). A server that offers no STARTTLS on 587 is refused; there is no plain option.
- **AUTH** (RFC 4954) with the password: PLAIN (RFC 4616), LOGIN, CRAM-MD5 (RFC 2195) or DIGEST-MD5, as the server offers them; no XOAUTH2 or OAUTHBEARER.
- **EHLO** with the address literal `[127.0.0.1]` (RFC 5321 §4.1.4), not the computer's name, which providers copy into the `Received` header everyone reads.
- **The message**: MIME, `multipart/alternative` (the Markdown as text, and HTML); `In-Reply-To` and `References` (RFC 5322 §3.6.4); PGP/MIME when signed or encrypted (below); an `Autocrypt` header when you have a key.
- **The copy in Sent**: IMAP APPEND, read, except at Gmail. A copy that could not be filed is said; the message is not sent again.

## Finding servers
- **Mail** (`discover.rs`): Thunderbird's autoconfiguration format, HTTPS only: `https://autoconfig.<domain>/mail/config-v1.1.xml`, then `https://<domain>/.well-known/autoconfig/mail/config-v1.1.xml`, then Mozilla's ISPDB (`autoconfig.thunderbird.net`, told the domain only), then a guess (`imap.<domain>`, `mail.<domain>`, port 993), said as a guess. **Not used**: SRV records (RFC 6186), which [architecture.md](architecture.md) names; Microsoft's Autodiscover.
- **Calendars and contacts** (`dav.rs`, `candidates`): the address given; RFC 6764's `/.well-known/caldav` and `/.well-known/carddav` on the domain and on `caldav.` and `carddav.` hosts; the known places of murena.io and e.email, fastmail.com, icloud.com and me.com, posteo.de, mailbox.org; port 2080 of the mail server (cPanel hosts). No SRV or TXT lookups. Redirections are followed by hand, five at most, within the same domain (`sites::domain_of`), Google's excepted: each request carries the password.
- **What a server offers** (`scout.rs`): a Nextcloud's `status.php` (product, version) without a password; its OCS capabilities with the calendars' password, to name its apps (calendars and contacts are taken; Files, Notes, Talk, Deck, Mail, Bookmarks, Tables, Forms and Photos are said "not yet").

## Calendars and contacts: CalDAV and CardDAV
Code: `crates/sioul-sync/src/dav.rs`, a small client on `ureq` and `roxmltree`; the files on disk in `crates/sioul-core/src/vdir.rs`.

| Request or property | Standard | Needed for | Without it |
|---|---|---|---|
| HTTPS, Basic authentication | RFC 9110, RFC 7617 | every request | plain HTTP refused (test builds aside). **Not supported**: Digest (RFC 7616), Negotiate, client certificates. Baïkal's default is Digest: set its "WebDAV authentication type" to Basic |
| A bearer token | RFC 6750 | Google only | — |
| `current-user-principal` | RFC 5397 | finding the account | the address asked is taken as the principal |
| `calendar-home-set`, `addressbook-home-set` | RFC 4791 §6.2.1, RFC 6352 §7.1.1 | finding the collections | not found: the server's address must be given |
| PROPFIND Depth 1: `resourcetype`, `displayname`, `getetag` | RFC 4918 | listing collections and items | — |
| `current-user-privilege-set` | RFC 3744 §5.4 | collections that can only be read (none of `write`, `write-content`, `all`, `bind`) | taken as writable |
| `supported-calendar-component-set` | RFC 4791 §5.2.3 | which calendars hold tasks (VTODO) | a calendar that says nothing is offered for events and tasks |
| `calendar-color` | Apple's namespace, `http://apple.com/ns/ical/` | a calendar's colour | none |
| `getctag` | CalendarServer's namespace | a collection unchanged since the last round is not compared | its items' tags are compared each round |
| REPORT `sync-collection`, `sync-token` | RFC 6578 | only what changed | every item's `getetag` (PROPFIND Depth 1), compared with what is known: slower |
| REPORT `calendar-multiget`, `addressbook-multiget` | RFC 4791 §7.9, RFC 6352 §8.7 | changed items in batches of 50 | one GET each (also when a multiget answers nothing) |
| PUT with `If-Match`; a new item with `If-None-Match: *`; 412 | RFC 9110 §13.1 | nobody's change overwritten. On 412 the server's version is kept and yours set aside in `$XDG_STATE_HOME/sioul/dav/conflicts`, said | Google takes `If-Match` only: new items go without `If-None-Match` |
| An ETag in PUT's answer | RFC 9110 | knowing what the server stored | the item is read back at the pull |
| DELETE with `If-Match` | RFC 9110, RFC 4918 | deletions | — |
| MKCALENDAR with `supported-calendar-component-set` | RFC 4791 §5.3.1 | new task lists and calendars | not at Google (greyed, with why) |
| Extended MKCOL | RFC 5689 | new address books | not at Google |
| PROPPATCH `displayname`; DELETE of an empty collection | RFC 4918 §9.2 | renaming; deleting a list emptied | — |
| POST to the address book | RFC 5995 | Google's new contacts, when its PUT is refused (403, 405, 409) | — |

**Not used**: `calendar-query` and time ranges (everything is synced, and repeating events are expanded here, `calcard`); CalDAV scheduling (RFC 6638): invitations go by mail (below); free-busy; CalDAV sharing; WebDAV-Push.

**Sync rounds**: what changed here first, then what changed there; every fifteen minutes and at once after a change in the window; one sync of an account at a time, across processes; a round cut short resumes where it stopped (`dav::tests`).

**A server must keep what it is given.** Sioul writes tasks and cards line by line (`crates/sioul-core/src/lines.rs`) and expects them back as written (`capabilities.rs`: "A CalDAV or CardDAV server keeps everything Sioul writes"; Google is the exception, [google.md](google.md)). A server that validates may refuse what the standard forbids:
- **sabre/dav** (Nextcloud's and Baïkal's server) checks RFC 5545 §3.8.2.3 in a VTODO: `DUE` must have `DTSTART`'s value type and come after it. A task whose start has a time while its date asked is a date, or comes after it, is refused (415). So a time given to a step is never its `DTSTART`: it is an event, a "time block" in a calendar of the task list's account (« Planned tasks » unless Tasks ⚙ names another), tied to the task both ways (`X-SIOUL-TASK` and an RFC 9253 `LINK`), so that every calendar application shows it ([tasks.md](tasks.md), "Pinned to a time"). `X-SIOUL-AT`, which Sioul wrote for such a time before time blocks (`tasks::AT`), is read for compatibility and never written: one for today or later becomes a block once, and the line goes from the task.
- **iCloud** keeps reminders upgraded since iOS 13 out of CalDAV: its task lists are not reached ([DAVx⁵'s notes on iCloud](https://www.davx5.com/tested-with/icloud)); its calendars and contacts are, with an app-specific password.
- **Google** keeps less than it is given: [google.md](google.md), and below.

**Invitations**, by mail: iMIP (RFC 6047) carrying iTIP (RFC 5546).
- Read in a message: REQUEST, CANCEL, PUBLISH (`agenda::invitation`).
- Answered: a REPLY by mail to the organizer, from the account the invitation came to, with PARTSTAT ACCEPTED, TENTATIVE or DECLINED (`agenda::reply`).
- Kept: the invitation's event, its METHOD taken out (`agenda::stored`), into the first calendar. **Limit**: its ATTENDEE lines stay as the organizer sent them, so the copy does not hold your answer, and other calendar applications may show it unanswered. Whether a server that schedules itself (RFC 6638) also writes to the organizer when this copy arrives: not checked.
- **Not supported**: inviting people from Sioul (no METHOD:REQUEST is ever written).

## What Sioul writes in tasks, events and cards
Changed line by line: what Sioul does not edit is kept byte for byte (tests: `tasks::a_change_touches_only_its_lines` keeps another application's `X-OTHER-APP` and an alarm; the contacts' tests keep a photo and `X-OTHER-APP`). A form saved unchanged writes nothing.

| Line | Standard | In | For |
|---|---|---|---|
| VTODO with SUMMARY, DESCRIPTION, DTSTART, DUE, STATUS, COMPLETED, PERCENT-COMPLETE, PRIORITY, CATEGORIES, RRULE | RFC 5545 | tasks | the usual fields; `you`, `proxy`, `joy`, `someday` are categories |
| `RELATED-TO` (RELTYPE=PARENT, or none) | RFC 5545 §3.8.4.5 | the step | a step of a bigger task; `CHILD` in the bigger task is read too |
| `RELATED-TO;RELTYPE=DEPENDS-ON` | RFC 9253 | the task that waits | waiting for another task |
| `RELATED-TO;RELTYPE=FINISHTOSTART;GAP=P14D` | RFC 9253 §4 | the task that comes first | a wait with a gap; `NEXT` is read too |
| `LINK;LINKREL=describedby`, `via`, `related`; `VALUE=URI` or `UID` | RFC 9253 | tasks, events | notes (`sioul:note/…`), the message (`mid:`, RFC 2392), drafts, events |
| `CONCEPT` (tag URIs, RFC 4151) | RFC 9253 §7.3 | tasks | the kind; "needs an open office"; other applications' concepts kept |
| `REFID` | RFC 9253 | tasks, events | the project |
| `ESTIMATED-DURATION` | draft-ietf-calext-ical-tasks | tasks | how long it takes |
| `CONTACT;ALTREP="sioul:contact/<UID>"` | RFC 5545 §3.8.4.2 | tasks | the people and offices involved |
| `X-SIOUL-BEFORE`, `X-SIOUL-AFTER`, `X-SIOUL-COST`, `X-SIOUL-GAIN` | non-standard properties, RFC 5545 §3.8.8.2 | tasks, events | time kept around it; costs and gain, 0 to 10 |
| `X-SIOUL-FELT-COST`, `X-SIOUL-FELT-GAIN` (`X-SIOUL-ON`), `X-SIOUL-ESTIMATE-FIRST`, `X-SIOUL-ENERGY`, `X-SIOUL-OFFICE-HOURS`, `X-SIOUL-AREA`, `X-SIOUL-BILLABLE` | the same | tasks | how it was; the first estimate; light or heavy; an office's hours; what it is for; billed or not |
| `X-SIOUL-AT` | the same | tasks | a time given for one day, before time blocks: read, never written; one for today or later becomes a block once (above) |
| `X-SIOUL-TASK` | the same | events | the task a time block is for, by its UID (above) |
| `X-SIOUL-GITHUB`, `X-SIOUL-GITHUB-REASON` | the same | the local GitHub list only | never sent to a server |
| vCard `CATEGORIES` | RFC 6350 §6.7.1, RFC 2426 §3.6.1 | cards | categories, which Nextcloud Contacts shows as groups; several lines read, written back as one where the first was |
| New cards in vCard 3.0 | RFC 2426 | cards | vCard 4.0 (RFC 6350) is read and kept; a 4.0 line merged into a 3.0 card is written as 3.0 |

**Not read**: contact groups kept as cards of their own (`KIND:group` with `MEMBER` in vCard 4.0, Apple's `X-ADDRESSBOOKSERVER-KIND`): they are not categories here, and such a card shows as a contact.

## Other programs editing the same tasks and events
Read in their source code on 6 October 2026, not tried with Sioul. What each does with Sioul's lines when it saves a task or an event:

| Program | Steps (`RELATED-TO`, PARENT) | Waits (`DEPENDS-ON`, `FINISHTOSTART`, `GAP`) | Other lines (`LINK`, `CONCEPT`, `REFID`, `ESTIMATED-DURATION`, `CONTACT`, `X-SIOUL-*`) | Read in |
|---|---|---|---|---|
| DAVx⁵ with OpenTasks or Tasks.org (the dmfs task provider) | kept | **turned into PARENT** on the way in, and written back so when the phone changes the task: a wait becomes a step; `GAP` lost | kept, as unknown properties, up to 25,000 characters each | davx5-ose, `synctools/…/mapping/tasks/builder/RelationsBuilder.kt`, `handler/RelationsHandler.kt`, `builder/UnknownPropertiesBuilder.kt` |
| DAVx⁵ with jtx Board | kept | stored, then **left out** when the phone's change is uploaded (only PARENT, CHILD, SIBLING are written) | kept | davx5-ose, `synctools/…/mapping/jtx/builder/RelatedToBuilder.kt`, `handler/RelatedToHandler.kt` |
| DAVx⁵, events (Android's calendar) | — | — | kept, as unknown properties, up to 25,000 characters each | davx5-ose, `synctools/…/mapping/calendar/builder/UnknownPropertiesBuilder.kt` |
| Tasks.org, its own CalDAV sync | kept (the parent: the first PARENT or untyped line) | kept: only PARENT lines change when the parent does | kept (its `unknownProperties`; its test `VTodoPreservationTest`) | tasks/tasks, `kmp/src/commonMain/kotlin/org/tasks/caldav/iCalendar.kt` |
| Nextcloud Tasks (web) | kept (the parent: the first PARENT or untyped line) | kept: its setter changes only the parent line, "so we don't overwrite RELTYPE=CHILD/SIBLING entries" | kept: it keeps the parsed calendar data (ical.js) and changes the properties edited | nextcloud/tasks, `src/models/task.js` |

**Not checked**: Thunderbird, Apple's Calendar and Reminders, Evolution, KOrganizer, GNOME's applications. Any program that rewrites a task from only what it understands would drop Sioul's lines: costs and ratings may then be lost ([capacity.md](capacity.md), "Ratings may not survive other applications").

## Google
Code and details: [google.md](google.md).
- **Signing in**: OAuth 2.0 for native applications (RFC 8252): the system browser, the answer on `http://127.0.0.1:<random port>`, five minutes at most; PKCE with S256 (RFC 7636), `state`, the ID token's address checked against the one typed. Scopes: `openid email`, `…/auth/calendar`, `…/auth/carddav`, `…/auth/tasks`. Sioul's own client is read at build time (`SIOUL_GOOGLE_CLIENT_ID`, `SIOUL_GOOGLE_CLIENT_SECRET`), else the person's own.
- **Calendars**: Google's CalDAV (`https://apidata.googleusercontent.com/caldav/v2/<address>/user`), events only; no MKCALENDAR or PROPPATCH; new items without `If-None-Match`, known by their `Location`. Google adds the calendar's default reminders to every event and shifts floating times; both come back as Google keeps them.
- **Contacts**: Google's CardDAV (`https://www.googleapis.com/.well-known/carddav`), written in vCard 3.0 (`contacts::as_vcard3`); a new contact by PUT, else POST (RFC 5995); no new address book.
- **Tasks**: the Google Tasks API v1 (`https://tasks.googleapis.com/tasks/v1`), each list kept here as a folder of VTODO files; `updatedMin` with deleted and hidden tasks; Google keeps a title, notes, done or not, a day, one level of steps; the rest is greyed in the form, kept in the file here, not sent.
- **Gmail and Google Workspace** (Workspace recognised by its MX): IMAP and SMTP with an app password, or XOAUTH2 after signing in with Google with the person's own key. Gmail's scope is restricted, so Sioul's own key cannot ask for it ([google.md](google.md), "Mail"). Tested against stand-ins only.
- **Tested**: the sign-in's pieces (unit tests); Google Tasks against `tools/google-tasks-stand-in.py`. **Not against Google itself**: the token exchange, its CalDAV and CardDAV answers, Google Tasks' own.

## GitHub
Code and details: [github.md](github.md).
- The REST API at `https://api.github.com` only (GitHub Enterprise Server: not supported), `X-GitHub-Api-Version: 2022-11-28`, answers asked with their ETag (`If-None-Match`; a 304 costs nothing on the limits).
- `GET /search/issues`, each search with `is:issue` or `is:pr`: a fine-grained token's search that could return both is refused with 422 ("Query must include 'is:issue' or 'is:pull-request'"). Sioul asked both at once until 098a9d5 (6 October 2026), and the whole sync stopped there.
- A fine-grained personal access token, read only (Issues, Pull requests, Metadata), in the system keyring. Nothing is written to GitHub.
- **Tested** against `tools/github-stand-in.py`, which refuses a mixed search as GitHub does; not against GitHub itself since the fix.

## Notes
Code: `crates/sioul-core/src/notes.rs`; the guide: [Notes](https://aurelienpierre.github.io/sioul/guide/notes.html#the-same-folder-as-obsidian-and-nextcloud-notes).
- **Obsidian**: `[[wikilinks]]` with `#heading` and `|text`, found as Obsidian finds them (the same folder first, then the shortest path, then aliases); `![[embeds]]`; Markdown links; `#tags`; front matter (title, tags, aliases); `- [ ]`; links back; deleting into the vault's `.trash`. `.obsidian`, `.trash`, `.git` and hidden folders are never read.
- **Nextcloud Notes**: its categories are folders; its `.txt` notes (its default) and `.md` notes read and written as Markdown, each keeping its extension (`notes::kind_of_file`, `notes::text_stem`); new notes are `.md`. Sioul reads the folder Nextcloud's client syncs, not Nextcloud's Notes API.
- **Tested** on files shaped as each writes them (`cargo test -p sioul-core notes`), not side by side with the applications.

## Sharing between devices
Details: [database.md](database.md), "What the sync app must do".
- **What the sync app must do**: carry new files, and files that grow, some day, in any order. No rename, no deletion, no lock. Files are written under a dot name, then renamed; the few rewritten change size at each writing (eDrive tells a change by its size alone). Conflicted copies, temporary files and other names are never read.
- **Folders known to be carried** (Notes and Papers are refused there, two carriers would undo each other's changes): `~/Nextcloud`, `~/Dropbox`, `~/ownCloud`, `~/Sync`, `~/Syncthing`, `~/OneDrive`, `~/pCloudDrive`, `~/Seafile`; Nextcloud's own list (`nextcloud.cfg`), Syncthing's (`config.xml`), Dropbox's (`info.json`). **Not recognised**: Google Drive, Insync, rclone, MEGA.
- **On a phone**: the folder read by its path with "All files access"; Murena's eDrive asked to look now (`CARRIERS`: its `FORCE_SCAN`); other sync apps at their own pace.
- **Tested** with a simulator of five ways of carrying (every change both ways, as Nextcloud's desktop client, Dropbox, Drive and OneDrive do; a size-only rule, as eDrive's; late and out of order; older copies put back beside conflicted copies; files on demand): `cargo test -p sioul-sync share`. On a phone: eDrive 1.9.2 on /e/OS 4.0 brought the sharing folder down at its next full scan ([android.md](android.md)).

## Sites
Code: `crates/sioul-app/qml/SitesPage.qml`, `SitePopup.qml`, `cpp/webengine.cpp`; details: [sites.md](sites.md), [research/platform.md](research/platform.md) §2.
- **The engine**: Qt WebEngine, Chromium 140 with security patches to 154 in Qt 6.11 (the Flatpak's KDE runtime carries 6.10). One persistent profile (`sioul-sites`): cookies forced persistent, a disk cache, permissions stored. The user agent leaves out Qt's name.
- **Permissions**: notifications granted to every site and caught by Sioul; the clipboard; the microphone, the camera and the screen as each site's switches say; geolocation, local fonts and pointer lock refused.
- **Calls** (WebRTC): Chromium's. The microphone and the camera as each site's switches say; the devices chosen by name, and changed during a call, through a script in every page and frame that gives each call relay tracks (Chromium's `MediaStreamTrackGenerator`), the speaker by `setSinkId`; checked by `tools/check-sites.py devices` on Chromium's fake devices ([sites.md](sites.md), "The devices of calls"). Sharing the screen needs the view's `screenCaptureEnabled` setting: Qt WebEngine keeps it off ("Disabled by default") and then refuses every capture once the screen is chosen (Qt 6.11, `src/core/media_capture_devices_dispatcher.cpp`; the page gets `AbortError: Invalid state`). On in every site's view and pop-up since 6 October 2026; checked by `tools/check-sites.py share`, a test page on a screen nobody sees (Weston, headless, with Xwayland). **Limits**: full screen is not turned on (`fullScreenRequested` is not handled, and Qt keeps it off by default).
- **Closing**: each site's page is closed as a browser closes its tabs (beforeunload, pagehide, unload) before Qt WebEngine shuts down, however Sioul quits (its window, a logout, SIGTERM), so a site that keeps its login in the open page (Discord) keeps it; checked by `tools/check-sites.py quit` ([sites.md](sites.md), "Closing").
- **Security keys** (WebAuthn, FIDO2): Qt's `webAuthUxRequested` (Qt 6.7 and later) drawn by Sioul: the account, the PIN, a touch, failures in words. On Linux, Chromium reaches `/dev/hidraw*` only when Qt WebEngine is built with udev (Fedora's is) and the seat's user has access (systemd 244 and later); the Flatpak needs `--device=all` (given). `PublicKeyCredential.getClientCapabilities()` is taken out of every page (QTBUG-149575, Qt 6.10 and 6.11). Windows draws its own dialog. **Limits**: a plain touch shows nothing in Qt 6.11; passkeys in a phone or in the system: none on Linux and macOS; Touch ID and iCloud Keychain: none on macOS; USB keys on macOS: unverified.
- **Downloads**: accepted, into the downloads folder. **PDFs**: Qt WebEngine's viewer, on by default.
- **Sign-in with Google** inside a site may be refused: Google blocks embedded browsers ([research/platform.md](research/platform.md) §1.4).
- **Android**: no Qt WebEngine; sites open in the system's browser.
- **Seen in use**: Proton Mail's web app (it asked for notifications at each visit until permissions were stored: [sites.md](sites.md)). No call and no security key is recorded as tried.

## Bitwarden
Code: `crates/sioul-sync/src/bitwarden.rs`; details: [sites.md](sites.md), "Bitwarden".
- Servers: bitwarden.com, bitwarden.eu, a self-hosted Bitwarden, Vaultwarden, over HTTPS. Sioul declares itself Bitwarden's desktop client at the server's own version (read from its config once a session; 2026.9.0 when it says none), since Bitwarden refuses clients too far behind.
- Keys: PBKDF2-SHA256 or Argon2id as the account says; HKDF; AES-256-CBC with HMAC-SHA256; RSA for organisations; COSE (XChaCha20-Poly1305, AES-256-GCM; XAES-256-GCM not yet). Unlocking with a security key's passkey through WebAuthn PRF.
- Second steps: a FIDO2 key, YubiKey OTP, an authenticator app, e-mail, a recovery code. **Not supported**: Duo. Read only: nothing is written to the vault. On Android, no security key (no Qt WebEngine).
- **Tested**: Bitwarden's key-derivation test vectors, RFC 6238's codes, a changed byte refused, a COSE message; the live cloud with a made-up account, refused for its password and not for its version (`the_cloud_takes_the_version`, ignored). With the owner's own vault on Bitwarden's cloud: in daily use (6 October 2026).

## OpenPGP
Code: `crates/sioul-core/src/pgp.rs` (Sequoia, pure-Rust cryptography), `crates/sioul-sync/src/keys.rs`; details: [client.md](client.md), "PGP".
- RFC 9580 messages; PGP/MIME (RFC 3156) written and read; inline PGP read. New keys: Curve25519, three years.
- **GnuPG**: its export is imported (`gpg --export-secret-keys --armor`), public keys exported. Sioul keeps its own keys (`~/.local/share/sioul/pgp/`) and never reads or writes GnuPG's keyring or agent. Keys whose secret part lives on a security key (an OpenPGP card) are used by Sioul itself, through PC/SC, not through GnuPG ([client.md](client.md), "Your keys on a security key"); GnuPG and Sioul share the key when `scdaemon.conf` says `pcsc-shared`, and "Let GnuPG release it" frees it otherwise. Not yet tried with a real key.
- Others' keys: Autocrypt headers (Level 1) of messages not forged; the Web Key Directory (draft-koch-openpgp-webkey-service, the advanced address then the direct one); keys.openpgp.org (VKS, by e-mail), on request only.
- **Tested** both ways with GnuPG 2.4: Sioul's signed and encrypted message decrypts in GnuPG with a good signature; GnuPG's encrypted, signed and tampered messages read as such in Sioul ([building.md](building.md), "Testing against a local mail server").

## Antivirus and scanned letters
Code: `crates/sioul-sync/src/antivirus.rs`, `ocr.rs`.
- **ClamAV** (Linux, macOS): `clamdscan --fdpass` when the daemon runs, else `clamscan` with the system's signatures, else with Sioul's own, refreshed daily with the system's `freshclam`. Homebrew's on a Mac. The Flatpak does not reach the system's ClamAV.
- **AMSI** (Windows): `AmsiScanBuffer`, answered by Microsoft Defender or any antivirus registered with AMSI.
- **Scans**: Poppler's `pdftotext`, else `pdftoppm` at 300 dpi read by Tesseract (French and English when installed); images by Tesseract. The Flatpak carries neither.
- **Tested**: Linux without ClamAV (the question comes, with Fedora's command); Tesseract on a French letter drawn as an image (`ocr::a_letter_as_an_image`, ignored, run by hand). The AMSI code is compiled by the Windows build, never run.

## AI agents
Code: `crates/sioul-cli/src/mcp.rs`, `mcp/`; details: [mcp.md](mcp.md).
- MCP 2025-06-18, and 2025-03-26 and 2024-11-05 for older clients; JSON-RPC 2.0 over standard input and output, one message a line. **Not supported**: Streamable HTTP and MCP's OAuth, so no client that reaches servers over the Internet (ChatGPT).
- **Tested**: `cargo test -p sioul-cli mcp`, and by hand on standard input. Not inside Claude Code or Claude Desktop yet.
- Anthropic's Messages API (`anthropic-version: 2023-06-01`), for an address protected against harassment, with the person's key: tested against a stand-in (`shield_ai` tests).

## Other web services
Open-Meteo (weather), OpenStreetMap's Nominatim and map tiles, keys.openpgp.org and the Web Key Directory, Mozilla's ISPDB, ClamAV's mirrors, each site's own icon: what each is told, and when: the guide's [Privacy and security](https://aurelienpierre.github.io/sioul/guide/privacy-security.html#what-leaves-your-computer-and-when).

## Files other programs read
- **Mail**: Maildir, a Maildir++ subfolder per folder (notmuch, mutt); on Windows `!` before the flags, as mbsync writes it; `:` and `!` read everywhere.
- **Calendars and contacts**: vdir, one `.ics` or `.vcf` per item, `displayname` and `color` beside (khal, khard, vdirsyncer, pimsync); the sync's state kept apart.
- **Notes, and the records of projects**: Markdown with front matter; settings, time spent, the projects' file and budgets: TOML.
- **Expected**: none of these programs was run on Sioul's folders.

## Systems
| | Linux | Windows | macOS | Android |
|---|---|---|---|---|
| Packages | AppImage (x86-64), Flatpak (KDE runtime 6.10), sources (Qt 6.9 or newer) | installer (Inno Setup), Windows 10 and 11, 64-bit | one disk image, Apple silicon and Intel, macOS 13 and later; signed ad hoc, not notarised | an APK, 64-bit ARM, Android 9 and later; by hand, in no store |
| Tested | developed and used on Fedora (Qt 6.11.2); the CI's tests on Ubuntu 24.04 | built, and the core, sync and command line tested, on GitHub's Windows Server 2022 at each change; not run by a person | the same on GitHub's macOS 15; not run by a person | a Gigaset GS290, /e/OS 4.0, Android 12 ([android.md](android.md)) |
| Keyring | Secret Service (GNOME Keyring, KWallet) | Credential Manager | Keychain | KeyStore (`android-keyring`) |
| Reminders with the window closed | `sioul remind --watch`, started with the session (XDG autostart) | not yet | a launch agent | doses and the wake alarm, by Android's alarms |
| Notifications | the desktop's, with buttons; the time running | Windows' | macOS' | doses, the time running, the wake alarm |
| Antivirus | ClamAV (not in the Flatpak) | AMSI | ClamAV (Homebrew) | none |
| Scans read | Tesseract and Poppler (not in the Flatpak) | Tesseract and Poppler | Homebrew's | not read |
| Sites | inside (on Ubuntu 24.04 and later, the AppImage runs them without Chromium's sandbox) | inside | inside | the system's browser |

**Not supported**: iPhone and iPad (the sharing's format is documented for one: [database.md](database.md), "Not built"), 32-bit systems, Linux on ARM as a package.

## Tested, and where it is recorded
| What | Against | Recorded in |
|---|---|---|
| IMAP writes and SMTP sending | GreenMail 2.1.8, on the computer | [building.md](building.md), "Testing against a local mail server" |
| Reaching IMAP servers, up to their greeting | Gmail's, Murena's (`mail.ecloud.global`), Microsoft's; no login | `imap::reaches_real_servers_up_to_the_greeting` (ignored, run by hand) |
| Finding an account | Murena's mail and calendars, without a password | `scout::murena_scouted` (ignored, run by hand) |
| Daily use, mail, calendars, tasks (steps, waits, links) and contacts | Murena (Nextcloud): `mail.ecloud.global` and its CalDAV and CardDAV, on Linux and on an /e/OS phone, since 4 October 2026; Murena's DAVx⁵ fork on the phone reading tasks only | the owner, 6 October 2026 |
| CalDAV and CardDAV both ways, an invitation answered | Radicale, on the computer; a stand-in in the tests (sync tokens, multigets, cut connections, 412) | [building.md](building.md); `dav::tests` |
| Google Tasks | `tools/google-tasks-stand-in.py` | [google.md](google.md), "What was tested" |
| GitHub | `tools/github-stand-in.py` | [github.md](github.md), "What was tested" |
| OpenPGP | GnuPG 2.4, both ways | [client.md](client.md), "As built" |
| Bitwarden | its key-derivation vectors; the live cloud with a made-up account; the owner's own vault, in daily use | [sites.md](sites.md), "Bitwarden"; the owner, 6 October 2026 |
| Sharing | a simulator of five sync behaviours; eDrive 1.9.2 on a phone | [database.md](database.md), "Tested"; [android.md](android.md) |
| Notes | files as Obsidian and Nextcloud Notes write them | `notes` tests |
| Antivirus, scans | Linux without ClamAV; Tesseract on a drawn letter | [client.md](client.md), "Antivirus"; `ocr::a_letter_as_an_image` |
| MCP | the server on its own | [mcp.md](mcp.md), "Tested" |
| Android | one phone | [android.md](android.md) |

## Not checked yet, worth testing
- A full sync with Fastmail, iCloud, Baïkal set to Basic: tasks with steps, waits and links. On any server, Murena's included: contact categories (new on 6 October) and lists made and renamed.
- Waits edited on a phone through DAVx⁵ (OpenTasks, Tasks.org, jtx Board), to confirm what their code says; Thunderbird's and Apple's handling of Sioul's lines.
- An invitation accepted on a server that schedules itself (Nextcloud, Google, iCloud): one answer reaching the organizer, the event shown answered or not elsewhere.
- Google itself: the sign-in, CalDAV, CardDAV, Google Tasks.
- GitHub itself with a fine-grained token, since 098a9d5.
- A YubiKey on GitHub, Google and Proton in Sites; a call with the microphone and the camera, and changing them during a real call (Jitsi, Google Meet, Teams: only fake devices and a stand-in page were used); sharing a screen in a real call, on X11 and on Wayland; a PDF a site shows.
- Discord itself staying logged in after Sioul quits (a test page doing as it does is checked: `tools/check-sites.py quit`).
- Bitwarden on a self-hosted server or Vaultwarden.
- Windows and macOS run by a person: the keyring, notifications, AMSI, ClamAV from Homebrew, security keys.
- The Flatpak: whether scanned letters can be read there at all.
- Claude Code and Claude Desktop with `sioul mcp`.
- Proton Mail Bridge, once its certificate is trusted by the system.
- Sync apps on a computer other than through the simulator: Nextcloud's client, Syncthing, Dropbox, Google Drive, OneDrive.
