# Extensions: the interface, version 1

An extension is a program apart from Sioul that brings data from a service Sioul does not speak to itself, such as a school's timetable. It is one WebAssembly file, written in Rust, the same on every system; Sioul runs it in a sandbox of its own, gives it only what its manifest declares, and takes back one kind of data: an agenda. The extensions live in a repository of their own (`sioul-extensions`), listed in one manifest that the person who publishes Sioul signs by hand.

This page is the reference. The design and its reasons are in the settings rework's notes (wave G); the code is `sioul_core::extensions` (the manifest, its signature, the messages), `sioul_sync::extensions` (the engine, what an extension may ask, the store) and the window's `extensions.rs`.

The interface's version is **1**. A change that an extension built for version 1 would not understand is a new version; Sioul runs only the versions it knows.

## The file

- A WebAssembly **core module** (not a component), built for `wasm32-unknown-unknown`, 8 MiB at most.
- It imports nothing but the functions of the module `sioul` below; no WASI. A module that imports anything else, or a function its manifest does not grant, is not started: "This extension asks for more than its list says."
- Sioul runs it with the pure-Rust interpreter **wasmi** 2.0 (MIT or Apache-2.0, with `std` and `validate` only): no code is generated at run time, so it runs the same on Linux, inside the Flatpak, on Windows, on macOS and on Android (built for `aarch64-linux-android` on 18 November 2026). A program that embeds it grows by about 1.6 MB (a minimal embedding, x86_64, release, stripped: 2.0 MB against 0.36 MB without it).

## What it exports

| Export | Type | What it does |
|---|---|---|
| `memory` | its one linear memory | where every message crosses |
| `sioul_interface` | `() -> i32` | the interface version it speaks: `1` |
| `sioul_alloc` | `(len: i32) -> i32` | the address of `len` free bytes of its memory, where Sioul writes a call; `0` when it has none |
| `sioul_call` | `(ptr: i32, len: i32) -> i32` | runs the call written at `ptr`; gives its reply through `sioul.reply` before returning, and returns `0` |

## How messages cross

Every message is **one JSON text in UTF-8**, written in the module's memory: a call and its reply, an ask and its answer.

1. Sioul asks `sioul_alloc(len)`, writes the call there, and runs `sioul_call(ptr, len)`.
2. While it runs, the extension asks Sioul for things: it writes an ask in its own memory and calls the import for that kind of ask, `(ptr: i32, len: i32) -> i32`. Sioul reads it, does what it may, and returns the length of the answer, which it keeps until the next ask. The extension then calls `sioul.answer(ptr)` with that many free bytes at `ptr`, and Sioul copies the answer there.
3. Before returning, the extension calls `sioul.reply(ptr, len)` once with its reply.

An ask that Sioul refuses is answered, never trapped: `{"refused": "<why>"}`, where `why` is one word (`host`, `scheme`, `address`, `size`, `time`, `count`, `form`, `not-now`, `keyring`, `network`). Only an address outside the module's memory ends the call.

| Message | At most |
|---|---|
| a call | 64 KiB |
| an ask | 1 MiB |
| an answer | 5 MiB |
| a reply | 8 MiB |

## What it imports

From the module `sioul`. The first six are given to every extension; `web` only to one whose manifest names hosts; `secret` and `sign_in` only to one whose manifest says it signs in.

| Import | Ask | Answer |
|---|---|---|
| `answer(ptr: i32)` | (copies the last answer to `ptr`) | |
| `reply(ptr: i32, len: i32)` | (the call's reply) | |
| `now` | `{}` | `{"now": "2026-11-18T21:04:05Z", "unix": 1795035845}` |
| `log` | `{"line": "…"}` | `{"logged": true}` |
| `hide` | `{"values": ["…"]}` | `{"hidden": 1}` |
| `random` | `{"bytes": 32}` (1 to 64; 32 when absent) | `{"random": "<URL-safe base64, no padding>"}` |
| `web` | `{"method": "GET", "url": "https://…", "headers": [["Accept", "application/json"]], "body": "…"}` | `{"status": 200, "headers": [["content-type", "…"]], "body": "…"}` |
| `secret` | `{"get": true}`, `{"keep": "…"}` or `{"forget": true}` | `{"secret": "…"}` or `{"secret": null}`, `{"kept": true}`, `{"forgotten": true}` |
| `sign_in` | `{"open": "https://…", "watch": "scheme://prefix"}` | `{"returned": "scheme://prefix?…"}` or `{"cancelled": true}` |

### The time and the log

- `now` gives the time in UTC, to the second. Nothing else of the device is given: no zone, no name, no language beyond the call's `lang`.
- `log` writes one line in the extension's own log, kept on this device (`$XDG_STATE_HOME/sioul/extensions/<id>.log` on Linux), never shared, never sent. A line is cut at 300 characters; 50 lines a call at most, the next ones dropped; the log keeps its last 400 lines and 128 KiB at most. Every line that reaches it is cut there again, the window's own lines included, and those name nothing an extension wrote.
- `random` gives bytes from the system's own source of randomness, which `wasm32-unknown-unknown` lacks: a sign-in's `state`, a PKCE verifier. The `state` is what ties the address a sign-in returns to the sign-in that was asked; a guessable one (the time) lets a page that gets into the sign-in end it with a code of its own account. Never logged.
- `hide` names values that must never reach the log (a token, a code): from then on in this call, any line holding one has it replaced by `[hidden]`. Values shorter than four characters are ignored. A call hides 256 values and 128 KiB at most, a value longer than 2 KiB kept in pieces that side by side hide it whole; past that, the extension's lines of the call are dropped (Sioul says how many), never written with a value it could not hide.
- **What Sioul holds for a call is bounded in bytes**, outside the module's own memory: each line is cut to 4 KiB when it is kept (a line cut so loses 2 KiB more when it is written, so that a value it held across the cut cannot show in part), Sioul's own lines are 160 at most, and a request to a host the manifest does not name is said once a call, the host cut to 80 characters, with how many asks named it. The secret kept, the address a sign-in returned and each value of its query and of its fragment, and the value of every `Authorization` header are hidden without asking. What hiding cannot cover: a value shorter than four characters, and a value the extension never names (each string of the kept secret is named by Sioul itself, so a platform's code or a host kept there reads `[hidden]` in Sioul's own lines too). Sioul's own lines in that log (a call's start and end, each web request's method, host and status, never its path, query or body: a path may name a pupil or an account) go through the same hiding.

### A web request

- `https` only, on the default port, no user name in the address.
- The host must be **exactly** one the manifest names (letters compared without case); no pattern, no subdomain.
- The name is resolved, and every address it gives must be public: a device of the person's network is never reached, nor a public name that points to a private address. No proxy is used, whatever the environment names (`HTTPS_PROXY`, `ALL_PROXY`): through one, the check would see the proxy's address, never the host's.
- Sioul names itself `Sioul/<version>` unless the extension sets its own `User-Agent`; the repository's extensions name themselves and their version (`Sioul-extension-skolengo/0.1.0`), nothing of the device or the person.
- **No redirect is followed**: a `3xx` comes back as it is, with its `location` header, and the extension asks again, its new host checked as the first.
- `method` is `GET` or `POST`; `body`, a text of 256 KiB at most. At most 32 headers of 8 KiB each; `Host`, `Content-Length`, `Transfer-Encoding`, `Connection`, `Upgrade`, `TE`, `Trailer` and `Proxy-*` are Sioul's to set and are refused. No cookie is kept between requests.
- The answer comes with its **status**, whatever it is, its headers (names in lower case) and its body as text (bytes that are not UTF-8 replaced). The body is cut at 4 MiB **once unpacked**: a larger one is refused (`size`), so a small compressed answer cannot fill the memory.
- Each request has 30 seconds; a call makes 100 at most.

### Its own secret

One entry in the system keyring (the Secret Service, Windows' Credential Manager, Apple's Keychain, Android's KeyStore), named `extension <id>`: only the extension of that id reads it, since Sioul names the entry from the manifest, never from the extension. 2400 bytes at most, kept as bytes (its UTF-8 as it is), which every system's entry holds, Windows' included. A longer secret is refused whole, never cut, and a line of Sioul's in the log says so. It is never written in a file or a log. Removing the extension forgets it: Remove first marks the extension as leaving in this device's record, so that a call under way, in this process or the phone's other one, ends at its next ask or slice of computation without keeping anything (`keep` is refused once the mark is read, and the mark is read at least once a second); it closes the extension's sign-in page, waits for the call's lock (45 seconds at most), then forgets the entry, takes out the extension's file and its log, and the record goes. While the keyring refuses (a keyring locked), the record stays, marked and out of the page, and the entry is tried again at the window's first minute and every six hours after, never more often: a locked keyring may ask for its password.

### A sign-in

Asked only during a `sign-in` call, once per call.

- `open`: the first page, `https` on a host the manifest names.
- `watch`: the beginning of the address the service returns to after the sign-in. It must begin with the prefix the manifest declares (`sign_in`).
- Sioul shows the page in a browser of its own, the site's name always in sight above it, and loads nothing but `https` pages. The name is the host of the last page that came in the main frame, never of a navigation still under way: a browser shows the address of a navigation the window itself starts (a page's pop-up, which the window opens in its own place) before that page comes, while the old page is still drawn. Meanwhile the page is covered, nothing can be typed into it, and the name reads "…"; a page that failed to come stays covered; one stopped leaves the old page and its name (`SignInSite.qml`, tested in `tst_signinsite.qml`). The window itself needs Qt WebEngine, which the QML tests do not start: to try once by hand, a page served over https that calls `window.open("https://<a named host>:81/")` (a port that never answers) must leave "…" above a covered page, never that host above the page that asked. When a navigation reaches an address beginning with `watch`, it is stopped there, never loaded, and returned whole: `{"returned": "…"}`. When the person closes the page, `{"cancelled": true}`.
- The site's name is said in its ASCII form (`xn--…` for a label in other letters, which a look-alike of a platform's name could use) and cut on the left, so that its end, the registrable part, stays in sight. The address watched for counts only from the main frame, and only when what follows the declared beginning ends its scheme and host (nothing, `/`, `?` or `#`): `skoapp-prod://sign-in-callback.other.example/…` is not it. One sign-in page at a time: a second extension's sign-in waits until the first page is closed, and each page's end clears its own place alone.
- The page never shares a store with the Sites page, so that a sign-in never signs a site out and neither sees the other's cookies. On a computer it is a window of Sioul's with Qt WebEngine and a profile of its own, off the record (no storage name: `SignInWindow.qml`), kept in memory and gone when the window closes, never the sites' persistent `sioul-sites` (`SitesWeb.qml`). On Android it is a screen of Sioul's own (`SignIn.java`) with the system's web view, in a process of its own whose web view keeps its own folders, its cookies, storage and caches emptied before and after each sign-in, never the sites' ([android.md](android.md#an-extensions-sign-in)): JavaScript on, since sign-in pages need it; files, content and other schemes off. It is part of Sioul's task, seen in Recents, so that the person can leave it to read a code and come back to it; if its process or Sioul's ends meanwhile, or the page's renderer does, the sign-in ends and can be started again at once. No scheme is registered with the system.
- The person has 15 minutes; that time is not counted in the call's limits.

## The calls

Every call is `{"interface": 1, "call": "<name>", "lang": "en"}`, with `lang` the language Sioul speaks now (`en` or `fr`) for the names the extension gives.

### `sign-in`

The person pressed **Sign in…**. Also carries `"choice"`: `null`, or the id the person chose.

| Reply | Meaning |
|---|---|
| `{"signed_in": true}` | the sign-in is kept; Sioul runs `refresh` next |
| `{"choose": [{"id": "…", "name": "…"}]}` | the extension needs the person to choose first (a platform, a school): Sioul shows the names (200 at most) and calls `sign-in` again with `choice` |
| `{"cancelled": true}` | the person closed the page |
| `{"problem": {…}}` | below |

### `refresh`

Sioul wants the extension's data now. The reply:

```json
{
  "agendas": [
    {
      "id": "pupil-1",
      "name": "Timetable: Alex",
      "calendar": "BEGIN:VCALENDAR\r\n…END:VCALENDAR\r\n",
      "period": {"from": "2026-11-11T23:00:00Z", "until": "2026-12-16T23:00:00Z"},
      "unread": [{"from": "2026-11-20T00:00:00Z", "until": "2026-11-21T00:00:00Z"}]
    }
  ],
  "problem": null
}
```

## What it hands back: an agenda

The one kind of data in version 1.

- `id`: the agenda's identifier among the extension's, lower-case letters, digits and `-`, 48 characters at most, stable from one refresh to the next. `name`: what the agenda is called, in the call's language, 80 characters at most.
- `calendar`: an iCalendar text (RFC 5545) holding events (`VEVENT`) and the time zones they name, each event with a `UID` unique in the agenda, the same at every refresh, and a `DTSTART`; 20,000 events, 250,000 lines and 8 MiB at most, whole (every component it opens closed in order, the calendar's own end there: a text cut short is nonsense, though the parser would take it). A cancelled event says `STATUS:CANCELLED`. No category: Sioul adds the extension's name.
- `period`: the period this answer read, RFC 3339, from before until, a little more than a year at most (400 days). A marked event Sioul holds of this agenda in that period and no longer finds in the answer is removed, the `unread` days aside. Without a `period`, nothing is removed.
- `unread`: the periods the extension could not read this time (a day the server failed on), 31 days each, 400 at most. Nothing Sioul holds there is changed or removed.
- A reply names 32 agendas at most.
- **Each agenda is written again from what was parsed, through an allow-list** (`reply::clean`), never passed on as the extension wrote it: names are judged once parsed, as Sioul reads every calendar (a name folded on two lines, a blank before its colon, quotes or a backslash in it are read as the parser reads them). Each event at the calendar's top keeps its `UID`, `DTSTART`, `DTEND` or `DURATION`, `SUMMARY`, `LOCATION`, `DESCRIPTION`, `STATUS`, `TRANSP`, `CATEGORIES`, `DTSTAMP`, `CREATED`, `LAST-MODIFIED` and `SEQUENCE`, with no parameter but `TZID` and `VALUE`, its texts without control characters (a new line kept in a description) nor marks that turn a line around, and no component inside it. Dropped in this version: guests, organizer, alarms, every `X-` line (Sioul's own included: a reminder, the time kept around, costs, an area, ties to your things are yours, never an extension's), repeat rules and their overrides, links, attachments, conference lines, class, and every component but an event and the time zones the events name; each of those zones is written once, its standard and daylight times alone, 64 KiB at most in all. A UID twice in an agenda is nonsense. An agenda's name and the names of its events are shown as plain text wherever Sioul shows them, never as its own voice; the marks that turn a line around are taken out of a name too.
- **Every bound is applied before anything is built** (`reply::refresh_reply`): the agendas are counted on the reply, the bytes and lines of each calendar on its text, its events once the text is parsed as Sioul parses every calendar. A reply past a bound is nonsense, nothing of it taken in.

## Where an agenda goes: a calendar of the person's own

Since 19 November 2026, in place of a read-only agenda kept by Sioul alone (`sioul_core::extensions::door`, `crates/sioul-app/src/extensions.rs`).

- **The person chooses the calendar**, one per agenda (a pupil's timetable each), in Settings ▸ Extensions, once the extension has named its agendas: **a new calendar** on one of their accounts that keep calendars (made as Agenda ⚙'s New agenda makes one, named after the agenda, on its server at the next sync), **one of theirs** that they may write (not one they only read, nor a subscribed one, nor one for tasks alone, nor a calendar of busy blocks Sioul keeps), or **a new calendar on this device only**. Until a calendar is chosen nothing is written, and the agenda's line says so.
- **The choice is an ordinary shared setting**, `[extensions.calendars]` `"<extension>/<agenda>" = "<account>/<id>"`, so that every device that runs the extension writes into the same calendar; this device's record keeps where it last wrote (`state::Brought::calendar`). Settings that do not read change nothing: nothing is written then.
- **Its sharing is the server's**: the person shares that calendar with the other parent from Agenda ⚙ or from the server; they need no Sioul, and read and write it with any program.
- **One writer**: each event goes through the agenda's own door for events (`vdir::write_item`, `vdir::remove_item`, under `agenda::guard`), as a person's own change does, into the local copy of that calendar; the ordinary sync sends it. Never a meeting with others: the allow-list leaves no guest nor organizer, and the guard refuses to change an event someone made one since.
- **Each event Sioul writes is marked three ways** (`door::lessons`):
  - its UID, `<extension>.<agenda key>.<lesson key>@extensions.sioul`, the two keys hashes (SHA-256, each part preceded by its length) of the extension's id, the agenda's id and the event's own UID (`extensions::event_uid`): the same at every refresh and on every device, and holding nothing of those ids but the extension's own, whatever they hold (a separator, a dot, a dash first, a path). Its file is `<extension>-<lesson key>.ics`, safe in a folder and in a server's address;
  - the category of the extension's name, its id with a capital (`extensions::category`: "Skolengo"), the same in every language, readable on a device that does not run the extension;
  - a last line of its description: "From Skolengo, through Sioul: a change made here is replaced when Skolengo changes this event." (in the language of the device that wrote it).
  It is written as free time (`TRANSP:TRANSPARENT`): a pupil's lesson is not the person's time. Sioul also keeps on it a hash of the event as the extension gave it, its stamps and Sioul's own lines aside (`X-SIOUL-EXTENSION-HASH`), and the same hash in this device's state (`state/extensions/<id>.lessons.toml`) for a server that drops the line.
- **Only marked events are touched**: an event is the extension's by the form of its UID alone (`extensions::uid_mark`), never by its title, its time, its category or its description. Whatever else the calendar holds, written by the person or by whoever it is shared with, is never read as the extension's, never changed, never removed; a lesson whose file name is taken by an event that is not it is left out (said in the log).
- **At each refresh**, for the period the answer says it read, its unread days aside (`door::plan`):
  - a lesson not yet there is added; one deleted by hand comes back (the timetable is the school's);
  - a lesson whose hash changed is written again; one whose hash did not change is left as it is, whatever someone else wrote on it (the event's own hash read first, then this device's note);
  - a cancelled lesson stays, its title "Cancelled: History" and `STATUS:CANCELLED`, so that whoever reads the calendar sees the free hour;
  - a marked event of this agenda in the period read that the answer no longer holds is removed.
  A period not read changes nothing; an answer without any event, or without a `period`, removes nothing (absence proves nothing). **Nothing is written when nothing differs**, this device's notes included.
- **Bounds, before anything is written** (`door::lessons`): each event becomes its own object with the time zones it needs (`agenda::calendar_objects`, the writer of the subscribed calendars and of a calendar file opened), the whole weighed against what came (`agenda::kept_most`); an event's file 256 KiB at most once marked. Past a bound, nothing of the agenda is written. 100,000 notes of hashes at most a device.
- **A calendar that cannot receive them** (gone, read only now, for tasks alone): nothing is written, and the agenda's line says so, with its chooser to pick another (`door::Problem::CalendarGone`). Another calendar chosen: the agenda's lessons leave the one before (`door::take_out_of`), then a refresh writes them into the new one.
- **Two devices that both run the extension** write into the same calendar through its server; the hash on each event makes the second write nothing when its fetch holds the same lessons. A device whose fetch is older than the one another device last wrote from writes nothing (the freshest-writer rule, `availability::may_write`, with the time the fetch's last answer came: `Outcome::answered`).
- **Remove** asks whether to leave the extension's lessons in the person's calendars or to take them out: taken out, every event whose UID is the extension's, in every calendar the person may write, whatever its agenda (never the last reply's list), through the agenda's door, and the calendars chosen for it leave the shared settings, so that another device still running it writes nothing until a calendar is chosen there again. The calendars themselves are never deleted. **Disable** stops the refreshes and leaves everything.
- **The event's card** says where a marked event comes from, "From Skolengo", beside its calendar.
- **Its problems** reach the extension's log as fixed words and ids (how many lessons were written, taken out, left, refused; why none were), cut as every line of the log, never a name an agenda or an event gave.

## A problem

`{"kind": "<kind>", "detail": "…"}`, in a reply to either call. Sioul says it in its own words, in one sentence. The detail is not kept: it comes after the call, where nothing hides what it holds. An extension that wants a detail in its log writes it there with `log` during the call, where it is hidden and cut.

| Kind | What Sioul says |
|---|---|
| `sign-in` | the sign-in has ended or was refused: sign in again |
| `unreachable` | the service did not answer |
| `refused` | the service refused (an account blocked, a school that does not subscribe) |
| `changed` | the service's answers no longer read as expected: an update of the extension is needed |

A reply with agendas and a problem is taken in, then the problem said.

## Limits of a call, and how it ends

| Limit | Value |
|---|---|
| memory | 64 MiB, one memory, one table of 10,000 elements |
| computation | 4 billion steps of the interpreter, given in slices |
| time | 5 minutes for a call, the person's time on a sign-in page aside |
| web requests | 100 a call, 30 seconds each |

A call that passes a limit, traps (the extension's own error), replies nothing, or replies what cannot be read ends at once, the module thrown away, and Sioul says one calm sentence ("Skolengo timetable stopped: it took too long."). Nothing of it is taken in, the person's agendas are untouched, and the next refresh starts afresh. Calls never run on the window's thread.

## The manifest

`manifest.toml` at the repository's root, with its detached signature `manifest.toml.sig` beside it.

```toml
format = 1
# Grows with each manifest signed; Sioul refuses one lower than the last it accepted.
serial = 1

[[extension]]
id = "skolengo"
version = "0.1.0"
interface = 1
name = { en = "Skolengo timetable", fr = "Emploi du temps Skolengo" }
about = { en = "…", fr = "…" }
brings = ["agenda"]
hosts = ["api.example.org", "sso.example.org"]
# The beginning of the address a sign-in returns to; absent when it keeps none.
sign_in = "skoapp-prod://sign-in-callback"
file = "https://github.com/<owner>/sioul-extensions/releases/download/skolengo-0.1.0/skolengo.wasm"
sha256 = "<64 hexadecimal digits>"
```

- `id`: lower-case letters, digits and `-`, 32 characters at most. `name` and `about` in English and French. `brings`: `agenda` in version 1. `hosts`: exact names. `file`: an `https` address, or a name relative to the manifest's own address.
- Sioul downloads the file (`https` only, public addresses, 8 MiB), and **runs it only after its SHA-256 matched the signed manifest**. It keeps it in its private data (`$XDG_DATA_HOME/sioul/extensions/`), and checks the hash again before each run, against the offer it accepted from the signed manifest and kept in this device's record (`extensions.toml`). That record is not signed: each offer in it is checked as a manifest's are whenever it is read, one that fails left out; whatever can write it already acts as the person on this device.
- An update is taken only when its version is not older than the one in use (versions compared part by part, numbers as numbers); an update waiting for the person's yes is dropped when a newer manifest no longer offers it; nothing is downloaded for an extension disabled on this device, which takes its update once enabled again.
- **An update that asks for more** than the version in use (another host, a sign-in where there was none) is not taken until the person says yes on Settings ▸ Extensions; the version in use goes on meanwhile.
- **Less applies at once.** What an extension is given never exceeds what the manifest read last lists: a host taken out of its list, or its sign-in, is taken from the version in use at once, its file unchanged, without asking. **An extension the manifest no longer lists stops** on each device that reads that manifest: it is disabled, its line says it is no longer in the list, an update waiting for it is dropped, and it cannot be enabled again until a manifest lists it again; Remove forgets its sign-in.
- Where the manifest is read from: `[extensions] manifest` in the configuration, by default the repository's address on GitHub. An address on this device (a folder or a `file://` address) is accepted too, for trials before the repository is public: the signature is checked all the same. This setting is each device's own and **never travels through the sharing** (`CONFIG_LOCAL` in `sioul-sync`'s `share.rs`): a path on one computer means nothing on a phone, and no device can change where the others read their list.
- When: when Settings ▸ Extensions is opened, and once a day while an extension is enabled. Never otherwise.

## The signature: exactly what is checked

Sioul holds one public key, compiled in: the primary key packet of the publisher's OpenPGP key (Ed25519, version 4) and its fingerprint, **nothing else of the certificate** (no user id, no subkey, no self-signature). Every build trusts that key alone: no feature of a build changes it, so a program built after a test run trusts the same key. The tests pass a key made for them to the function they test (`read_manifest_with`); that key exists in test builds only, marked so that `tools/check-test-marks.sh` refuses a program holding it.

The manifest is accepted only when:

1. `manifest.toml.sig` holds exactly one OpenPGP packet, a signature (binary or ASCII-armored);
2. it is of the **binary document** type (`0x00`);
3. its issuer is the compiled key: an issuer fingerprint subpacket equal to the compiled fingerprint, and no issuer naming another key;
4. its hash algorithm, and any critical subpacket, are accepted by Sequoia's standard policy now (`StandardPolicy::signature`, collision resistance required);
5. it was made no later than now (a minute of difference allowed), and has not expired if it says when it does;
6. it verifies cryptographically, with the compiled key, over the manifest's **exact bytes**, as they were downloaded;
7. the compiled packet's own fingerprint is the compiled fingerprint (a check of Sioul's build);
8. the manifest's `serial` is not lower than the last one this device accepted (kept in `$XDG_STATE_HOME/sioul/extensions.toml`), nor than the lowest this release of Sioul accepts (`LOWEST_SERIAL`, compiled in): an older signed manifest cannot be served again, to a fresh device or one whose record was lost either;
9. under the serial this device accepted last, it is the same manifest (the SHA-256 of its bytes): two manifests signed with one number cannot be alternated.

**Not checked**, since no certificate is read: the key's expiry, its revocation, its capabilities (the "sign" flag), its user ids. A revoked or expired key keeps signing manifests that Sioul accepts until a new release of Sioul compiles another key in: a change of key is a new release, and its notes say so.

**The signature binds no context**: any signature of a binary document by that key, over bytes that read as a manifest, is a manifest's signature; nothing in it says "Sioul's extensions". So a manifest signed for a trial (a test host, a file beside it) stays valid on every device until a higher serial is known there: trial manifests are signed with serials below the ones published after them, so that the first manifest published makes them stale on each device that reads it, and the next release's `LOWEST_SERIAL` keeps them from fresh devices. Git commits and release archives do not read as a manifest.

**What the serial leaves open.** No expiry is asked of a signature: the publisher would have to sign again on a timer. So, between two releases of Sioul, a manifest signed after the release's `LOWEST_SERIAL` and since replaced can still be served to a device that never saw the newer one (a new phone, a record lost); and a device whose address serves nothing new (an account taken over, an address changed on that device) hears of no update and no withdrawal, without a word. Each release of Sioul raises `LOWEST_SERIAL` to the serial of the manifest published then; a manifest signed for a trial stays valid until a higher serial is known (S5), so trial manifests are signed with serials below the published ones.

The command that makes the signature, on the publisher's computer with the key's security key plugged in (the `!` makes GnuPG use the primary key itself, not a signing subkey):

```sh
gpg --no-textmode --local-user '<FINGERPRINT>!' --digest-algo SHA512 --detach-sign --output manifest.toml.sig manifest.toml
```

## Settings ▸ Extensions

What the manifest offers, each with what it brings, the sites it talks to and whether it keeps a sign-in, before anything is enabled; then **Enable**, **Sign in…**, its state (when it last ran, what it brought, its last problem in one sentence; "brought" only when this device wrote its lessons, not when another device's fetch was the fresher), each agenda it brought with the calendar its lessons go into, chosen there, and how it went ("In Kids · example.org: 140 lessons, read Thursday 19 November at 14:05"; or why nothing was written), **Disable** and **Remove** (which forgets its sign-in and asks whether to leave its lessons in your calendars). Enabling is this device's: each device that should refresh an extension enables it and signs in itself; every device shows the lessons, which come with their calendar.

An extension enabled here refreshes when Sioul starts (on a computer: a phone's window starts and ends many times a day, so there the three hours alone decide), then every three hours while the window runs, and in the phone's background step; never two calls of the same extension at once, within one process or across a phone's two (the window's and the background service's, which share the extension's sign-in): each call holds a lock file beside the extension's log, taken without waiting, and a process that finds it held leaves that run to the other, then sees from the record that it ran. The three hours count from the last run this device made, as its record says or, when the record could not be written (a full disk, a folder Sioul may not write), as the process remembers it: such a run is said on the page ("…could not note it on this device…"), and the next one still waits three hours, never a run each minute against the service.
