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

From the module `sioul`. The first five are given to every extension; `web` only to one whose manifest names hosts; `secret` and `sign_in` only to one whose manifest says it signs in.

| Import | Ask | Answer |
|---|---|---|
| `answer(ptr: i32)` | (copies the last answer to `ptr`) | |
| `reply(ptr: i32, len: i32)` | (the call's reply) | |
| `now` | `{}` | `{"now": "2026-11-18T21:04:05Z", "unix": 1795035845}` |
| `log` | `{"line": "…"}` | `{"logged": true}` |
| `hide` | `{"values": ["…"]}` | `{"hidden": 1}` |
| `web` | `{"method": "GET", "url": "https://…", "headers": [["Accept", "application/json"]], "body": "…"}` | `{"status": 200, "headers": [["content-type", "…"]], "body": "…"}` |
| `secret` | `{"get": true}`, `{"keep": "…"}` or `{"forget": true}` | `{"secret": "…"}` or `{"secret": null}`, `{"kept": true}`, `{"forgotten": true}` |
| `sign_in` | `{"open": "https://…", "watch": "scheme://prefix"}` | `{"returned": "scheme://prefix?…"}` or `{"cancelled": true}` |

### The time and the log

- `now` gives the time in UTC, to the second. Nothing else of the device is given: no zone, no name, no language beyond the call's `lang`.
- `log` writes one line in the extension's own log, kept on this device (`$XDG_STATE_HOME/sioul/extensions/<id>.log` on Linux), never shared, never sent. A line is cut at 300 characters; 50 lines a call at most, the next ones dropped; the log keeps its last 400 lines.
- `hide` names values that must never reach the log (a token, a code): from then on in this call, any line holding one has it replaced by `[hidden]`. Values shorter than four characters are ignored. The secret kept, the address a sign-in returned and each value of its query, and the value of every `Authorization` header are hidden without asking. Sioul's own lines in that log (a call's start and end, each web request's method, host, path and status, never its query or body) go through the same hiding.

### A web request

- `https` only, on the default port, no user name in the address.
- The host must be **exactly** one the manifest names (letters compared without case); no pattern, no subdomain.
- The name is resolved, and every address it gives must be public: a device of the person's network is never reached, nor a public name that points to a private address.
- **No redirect is followed**: a `3xx` comes back as it is, with its `location` header, and the extension asks again, its new host checked as the first.
- `method` is `GET` or `POST`; `body`, a text of 256 KiB at most. At most 32 headers of 8 KiB each; `Host`, `Content-Length`, `Transfer-Encoding`, `Connection`, `Upgrade`, `TE`, `Trailer` and `Proxy-*` are Sioul's to set and are refused. No cookie is kept between requests.
- The answer comes with its **status**, whatever it is, its headers (names in lower case) and its body as text (bytes that are not UTF-8 replaced). The body is cut at 4 MiB **once unpacked**: a larger one is refused (`size`), so a small compressed answer cannot fill the memory.
- Each request has 30 seconds; a call makes 100 at most.

### Its own secret

One entry in the system keyring (the Secret Service, Windows' Credential Manager, Apple's Keychain, Android's KeyStore), named `extension <id>`: only the extension of that id reads it, since Sioul names the entry from the manifest, never from the extension. 2400 bytes at most (what Windows keeps). It is never written in a file or a log. Removing the extension forgets it.

### A sign-in

Asked only during a `sign-in` call, once per call.

- `open`: the first page, `https` on a host the manifest names.
- `watch`: the beginning of the address the service returns to after the sign-in. It must begin with the prefix the manifest declares (`sign_in`).
- Sioul shows the page in a browser of its own, the site's name always in sight above it, and loads nothing but `https` pages. The name is the host of the last page that came in the main frame, never of a navigation still under way: a browser shows the address of a navigation the window itself starts (a page's pop-up, which the window opens in its own place) before that page comes, while the old page is still drawn. Meanwhile the page is covered, nothing can be typed into it, and the name reads "…"; a page that failed to come stays covered; one stopped leaves the old page and its name (`SignInSite.qml`, tested in `tst_signinsite.qml`). The window itself needs Qt WebEngine, which the QML tests do not start: to try once by hand, a page served over https that calls `window.open("https://<a named host>:81/")` (a port that never answers) must leave "…" above a covered page, never that host above the page that asked. When a navigation reaches an address beginning with `watch`, it is stopped there, never loaded, and returned whole: `{"returned": "…"}`. When the person closes the page, `{"cancelled": true}`.
- On a computer the page is a window of Sioul's with Qt WebEngine and a profile of its own, kept in memory and emptied when the window closes. On Android it is a screen of Sioul's own (`SignIn.java`) with the system's web view: JavaScript on, since sign-in pages need it; files, content and other schemes off; its cookies and storage cleared when it closes. It is part of Sioul's task, seen in Recents, so that the person can leave it to read a code and come back to it; if Sioul's process ends meanwhile, or the page's renderer does, the sign-in ends and can be started again at once. No scheme is registered with the system.
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
      "unread": [{"from": "2026-11-20T00:00:00Z", "until": "2026-11-21T00:00:00Z"}]
    }
  ],
  "problem": null
}
```

## What it hands back: an agenda

The one kind of data in version 1.

- `id`: the agenda's identifier among the extension's, lower-case letters, digits and `-`, 48 characters at most, stable from one refresh to the next. `name`: what the agenda is called, in the call's language, 80 characters at most.
- `calendar`: an iCalendar text (RFC 5545) holding events only (`VEVENT`), each with a `UID` unique in the agenda and a `DTSTART`; 20,000 events and 8 MiB at most. A cancelled event says `STATUS:CANCELLED`.
- `unread`: the periods the extension could not read this time (a day the server failed on). What the last copy held there **stays**: an event of the last copy that overlaps such a period is kept, and nothing else of the last copy.
- Sioul takes each agenda in as a **read-only agenda**, replaced whole at each refresh. It travels to the person's other devices through the sharing (its part "lists"), read only there too. When two devices both run the extension, the one whose fetch is the freshest writes.
- An agenda the reply does not name is left as it is; removing the extension asks whether to keep its agendas.

## A problem

`{"kind": "<kind>", "detail": "…"}`, in a reply to either call. Sioul says it in its own words, in one sentence, and writes the detail in the log only (hidden values replaced).

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
hosts = ["api.skolengo.com", "sso.monbureaunumerique.fr"]
# The beginning of the address a sign-in returns to; absent when it keeps none.
sign_in = "skoapp-prod://sign-in-callback"
file = "https://github.com/<owner>/sioul-extensions/releases/download/skolengo-0.1.0/skolengo.wasm"
sha256 = "<64 hexadecimal digits>"
```

- `id`: lower-case letters, digits and `-`, 32 characters at most. `name` and `about` in English and French. `brings`: `agenda` in version 1. `hosts`: exact names. `file`: an `https` address, or a name relative to the manifest's own address.
- Sioul downloads the file (`https` only, public addresses, 8 MiB), and **runs it only after its SHA-256 matched the signed manifest**. It keeps it in its private data (`$XDG_DATA_HOME/sioul/extensions/`), and checks the hash again before each run.
- **An update that asks for more** than the version in use (another host, a sign-in where there was none) is not taken until the person says yes on Settings ▸ Extensions; the version in use goes on meanwhile.
- Where the manifest is read from: `[extensions] manifest` in the configuration, by default the repository's address on GitHub. An address on this device (a folder or a `file://` address) is accepted too, for trials before the repository is public: the signature is checked all the same.
- When: when Settings ▸ Extensions is opened, and once a day while an extension is enabled. Never otherwise.

## The signature: exactly what is checked

Sioul holds one public key, compiled in: the primary key packet of the publisher's OpenPGP key (Ed25519, version 4) and its fingerprint, **nothing else of the certificate** (no user id, no subkey, no self-signature). A test build holds a key made for the tests instead; a release build holds the publisher's.

The manifest is accepted only when:

1. `manifest.toml.sig` holds exactly one OpenPGP packet, a signature (binary or ASCII-armored);
2. it is of the **binary document** type (`0x00`);
3. its issuer is the compiled key: an issuer fingerprint subpacket equal to the compiled fingerprint, and no issuer naming another key;
4. its hash algorithm, and any critical subpacket, are accepted by Sequoia's standard policy now (`StandardPolicy::signature`, collision resistance required);
5. it was made no later than now (a minute of difference allowed), and has not expired if it says when it does;
6. it verifies cryptographically, with the compiled key, over the manifest's **exact bytes**, as they were downloaded;
7. the compiled packet's own fingerprint is the compiled fingerprint (a check of Sioul's build);
8. the manifest's `serial` is not lower than the last one this device accepted (kept in `$XDG_STATE_HOME/sioul/extensions.toml`): an older signed manifest cannot be served again.

**Not checked**, since no certificate is read: the key's expiry, its revocation, its capabilities (the "sign" flag), its user ids. A change of key is a new release of Sioul. The serial number guards against an older manifest only on a device that saw a newer one.

The command that makes the signature, on the publisher's computer with the key's security key plugged in (the `!` makes GnuPG use the primary key itself, not a signing subkey):

```sh
gpg --no-textmode --local-user '<FINGERPRINT>!' --digest-algo SHA512 --detach-sign --output manifest.toml.sig manifest.toml
```

## Settings ▸ Extensions

What the manifest offers, each with what it brings, the sites it talks to and whether it keeps a sign-in, before anything is enabled; then **Enable**, **Sign in…**, its state (when it last ran, what it brought, its last problem in one sentence), **Disable** and **Remove** (which forgets its sign-in and asks whether to keep its agendas). Enabling is this device's: each device that should refresh an extension enables it and signs in itself; a device that does not still shows the agendas, which come through the sharing.

An extension enabled here refreshes when Sioul starts, then every three hours while the window runs, and in the phone's background step; never two calls of the same extension at once, within one process or across a phone's two (the window's and the background service's, which share the extension's sign-in): each call holds a lock file beside the extension's log, taken without waiting, and a process that finds it held leaves that run to the other, then sees from the record that it ran. The three hours count from the last run this device made, as its record says or, when the record could not be written (a full disk, a folder Sioul may not write), as the process remembers it: such a run is said on the page ("…could not note it on this device…"), and the next one still waits three hours, never a run each minute against the service.
