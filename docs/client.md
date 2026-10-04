# The full client: mail, contacts, calendars

Sioul becomes a complete client while keeping its rule: **only what is used most is in view; the rest is one right click, or one fold, away.** This page reviews what mail, address-book and calendar clients usually offer (Thunderbird, Apple Mail, Outlook, Gmail, Fastmail, K-9/Thunderbird for Android, Evolution, KOrganizer), sorts each feature into three tiers, and says how Sioul shows it.

- **In view**: on screen without asking, at most six actions at a time, in two groups (answering; filing away).
- **One step away**: the right-click menu (also the "…" button and the Menu key, for the keyboard), or a folded section that opens in place.
- **Settings**: beside what they change: a ⚙ at the end of each page's first row, each setting with a sentence on what it does, saved at once into the configuration file, its comments kept (`settings.rs`, `config::set_value`); "Aa" wherever long text is (font, size, line spacing).
- **Accounts**, in four tabs: *your accounts*, one card per address with each of its services (mail; calendars, tasks and contacts; Google), each with a switch (off, it keeps its settings and is neither synced nor shown; its calendars wait in their folder, `vdir::set_account_off`), and for mail what it is for, whether it is protected against harassment, how far back, how often, its rank, your name and signature, all in view; "What this server offers" asks the server (`sioul_sync::scout`: the provider's mail settings, calendars and contacts at the well-known addresses, a Nextcloud's version and its apps, Files, Notes, Talk, Deck, said as in Sioul or not yet), and adds the mail of an address whose calendars are here with their password; *add an account*; *senders* (safe, neutral, blocked: one list each, the only place they are edited); *encryption*. Sites are not accounts: they are `[[site]]`, made and changed on the Sites page.
- **Settings**, in tabs: display, hours, reminders and notifications, your folder and sharing, invoices.
- **One setting, one place**, by what owns or uses it: an object's own settings where the object is made (an address's area, history, pace and shield on its card in Accounts; a project's routes on its page; a bank account's budgets and rules on its card); a page's display and behaviour behind its ⚙ (Mail: conversations and pace; Tasks: their hours, types, lists, the categories of work and of your own, GitHub; the Porch: the projects it shows, letters, its sorting); what belongs to no single page in Settings (language, colours, your folder, the hours, reminders, invoices, sharing); who may write to you is in Accounts ▸ Senders. A test fails when a setting shows in two places (`settings::tests::each_page_has_its_own`).

## Calm first
A full client must not bring back the inbox that screams. These rules come before any feature; when a feature conflicts with one, the rule wins ([design.md](design.md), [research.md](research.md)).

1. **Nothing counts at you.** No unread numbers, no badges, no red. A folder with something new shows a small dot; how many is said in words when you open it.
2. **New mail does not interrupt.** It waits in the Porch for an admin window; only the codes and links you ask sites for notify, and what you send yourself comes in any hour ([porch.md](porch.md)). The client's folders are for when you choose to look.
3. **Undo, never "are you sure?".** Archiving, deleting, junking and moving happen at once, with a quiet "Undo" for ten seconds; the server is told only after that. No confirmation dialogs, nothing lost to a slip.
4. **Sending has a breath.** "Send" waits ten seconds with an "Undo" before the message leaves; drafts save as you type, so closing the window never loses anything.
5. **One thing at a time.** The list shows who, what and when, without previews; the reader shows one message; quotes, attachments and details stay folded.
6. **Plain words, no alarm.** "Move to the trash", "Junk", "Not now"; destructive actions are labelled plainly, never red; errors say what happened and what to do, in the status line, never in a pop-up.
7. **A short past by default.** A folder shows its last two weeks; "Earlier" opens the rest on request, so no list is endless. All the mail is kept all the same: after a first sync of two weeks, older mail comes a round at a time (300 messages, the newest first), each round's size asked first (`RFC822.SIZE`) so the disk keeps a reserve of 5 GB or a twentieth of it, whichever is more; when older mail waits for room, the status line says so once. Kept for searching, and for what will learn on this computer what is spam and what matters.
8. **The same place for the same thing.** Writing, the message's actions, the folders: always in the same spot, all reachable from the keyboard.
9. **Calendars without pressure.** What comes, from today; nothing "overdue" in red; one silent reminder the working day before, and the event's own alarms ([reminders.md](reminders.md)).
10. **Contacts are names first.** Details fold until opened.

## Mail

| Feature | Tier | How |
|---|---|---|
| Folders: inbox, sent, drafts, archive, junk, trash | in view | the left column, by account; other folders folded under "More folders" |
| The Porch (new mail, sorted and summarised) | in view | its own place, as now |
| Read a message | in view | headers apart, attachments folded, quotes folded ([porch.md](porch.md)) |
| Write | in view | one button, always in the same place |
| Reply; reply to all (shown only when there are other recipients); forward | in view | above the message |
| Archive, move to the trash, junk | in view | above the message, done at once with "Undo" for ten seconds (rule 3) |
| Mark unread or read, flag | one step | right click; opening a message marks it read; a small dot, not bold, marks the unread |
| Move or copy to a folder | one step | right click, then the folder list |
| Block the sender, let the sender in | one step | right click, and in the reader |
| Show the source, save as .eml | one step | right click |
| Empty the trash, the junk | one step | right click on the folder |
| Search | in view | one field at the top of a folder (body search comes with tantivy) |
| Threads | one step | a message's earlier messages are folded in it; a thread view comes later |
| Cc, Bcc, sender account | one step | folded in the writing window ("Cc, Bcc"); the account follows the message answered |
| Attachments when writing | in view | "Attach", and dropping files on the window |
| Drafts | in view | saved as you type; listed in Drafts |
| Signature | settings | per account, in Markdown |
| Send later, read receipts, templates, rules | later | |

### Writing in Markdown
- **You write Markdown**; the message goes out as HTML (CommonMark with tables and strikethrough, `pulldown-cmark`) and as plain text (the Markdown itself, readable as is), in `multipart/alternative`.
- **Replies and forwards**: your text is converted first, then the quoted message is appended below it, already HTML (made safe, [porch.md](porch.md)) or text converted, in a `blockquote`, under "On …, X wrote:"; a forward carries the original headers and its attachments.
- **Threading**: `In-Reply-To` and `References`, so every client threads the answer.
- **Sending is deliberate**: a button or Ctrl+Enter, never automatic; then a copy goes to Sent (Gmail keeps its own).

### Writing to the server
Sioul was read-only. The client writes, and only when you act: flags with `UID STORE`, moves with `UID MOVE` (else copy, flag and `UID EXPUNGE`, RFC 4315), sending through SMTP submission (RFC 8314, TLS from the first byte or STARTTLS), and the sent copy with `APPEND`. Folders are found by their special use (RFC 6154: `\\Sent`, `\\Drafts`, `\\Junk`, `\\Trash`, `\\Archive`), else by name. Each folder is a Maildir++ subfolder; flags and deletions made elsewhere are brought back at each sync.

### Writing, as built
- **Line breaks are kept**: a new line in the text is a new line in the mail, as in a chat; a blank line starts a paragraph.
- **The signature** (Accounts, "Name and signature…") is put below the text when the draft starts, after the usual "-- " line, so you see and can change what goes out. Your name there is the one recipients see.
- **What you answer stays out of the way**: the window says "Below your text: Jane's message of …, quoted"; the quote is added when the message leaves. A draft keeps its own copy of the message it answers, so archiving or deleting that message meanwhile changes nothing.
- **Drafts live on this computer**, saved as you type, listed under "Drafts" while there are some. They are not copied to the server's Drafts folder.
- **Answering your own message** (from Sent) goes to whom it went to, as other clients do.

### Selection, moves and conversations
- **Several messages**: Ctrl+click adds or takes out, Shift+click takes everything from the last one clicked, Ctrl+A all, Escape none; a bar then marks them read, archives, deletes or moves them, under one "Undo".
- **Dragging** a message (or the selection it is in) onto any folder of any account. Into another account, the message is appended there with its flags (`mailbox::move_across`), and taken off the first only once the second server has it: if the copy fails, the original stays; if removing it fails, both remain.
- **Folders kept here or on the server only**: right click on a folder, "Keep a copy here". A folder left on the server only shows muted, its copy here removed and where its sync stopped forgotten, so keeping it again brings it back whole (`skip_folders` in the account). The inbox is always kept. "New folder…" (under "More folders") makes one on the server (`CREATE`); "Delete this empty folder" takes one of yours off the server (`DELETE`), only when the server says it holds no message (`STATUS MESSAGES`), never a folder with a purpose (inbox, Sent, Drafts, trash, junk, archive).
- **Conversations** (Mail ⚙, "By conversation"): messages tied by Message-ID, In-Reply-To and References (never by subject alone), the newest of the folder first with the size of the conversation; unfolded, the others, oldest first, your answers from Sent among them (`threads.rs`).
- **Real time**: the checkbox on the Porch and in Mail fetches every folder of every address each minute, and opens the Porch, until unticked.

### Where it stands
| | |
|---|---|
| Done | folders by special use or name, fetched in the background; the list of the last two weeks with "Earlier", search by sender, recipient and subject; the reader with reply, reply to all, forward, archive, delete (for good in the trash), junk and not junk; read, unread, flag, move and the source one right click away; ten seconds to undo, also for sending and deleting a draft; the writing window with Cc and Bcc folded, attachments by button or drop, the Markdown preview, the sending account when there are several |
| Next | emptying the trash and the junk; copying to a folder; saving as .eml; a message's thread; drafts on the server |

## Contacts (CardDAV, RFC 6352)

| Feature | Tier | How |
|---|---|---|
| The list, with search | in view | names only, the search field on top |
| A contact: name, e-mail addresses, phone numbers | in view | |
| Write to, call | in view | next to each address and number |
| Postal address, organisation and role, birthday, notes, web sites, other fields | one step | folded under "More" |
| Add, edit | in view | "New contact"; editing in place |
| Delete, duplicate, merge two contacts | one step | right click |
| Address books (several per account) | one step | named on each card; a contact moved to another in one choice, asked first when it would lose something there |
| Groups, photos, the map, finding duplicates | later | |

From a message, a sender becomes a contact in one right click, and a contact's page lists the mail exchanged.

## Calendars (CalDAV, RFC 4791)

| Feature | Tier | How |
|---|---|---|
| What comes: today, then the next two weeks | in view | the default view, a calm list |
| Week and month | one step | a switch above the list |
| An event: title, when, where | in view | |
| Notes, reminders, recurrence, guests, the calendar it belongs to | one step | folded under "More" |
| New event | in view | "New event"; all-day in one tick |
| Edit, delete, duplicate, move to another calendar | one step | right click |
| Invitations by mail (`.ics`, iMIP): add, accept, decline | in view | in the message that brings them |
| Several calendars, their colours | one step | a filter, folded |
| Tasks (VTODO) with their links (RFC 9253) | in view | their own place, "Tasks" ([tasks.md](tasks.md)) |

Recurrences are expanded for display (RFC 5545 §3.3.10); times are kept with their time zone and shown in yours.

## Storage and sync
- **Contacts and calendars**: one `.vcf` or `.ics` file per item, one folder per address book or calendar (vdir, as pimsync and khal use), with the server's ETags kept apart in the state folder; changes go back with `If-Match`, so nobody's edit is overwritten silently. When both sides changed one item, the server's version is kept and yours is set aside in `~/.local/state/sioul/dav/conflicts`, said in the status line.
- **Discovery**: the address you give, else `.well-known/carddav` and `/caldav` (RFC 6764) on your domain, then a few providers' known places (Murena, Fastmail, iCloud, Posteo, mailbox.org) and port 2080 of your mail server for cPanel hosts; then the principal and its home sets. Redirections are followed within the same domain only (`caldav.example.org` from `example.org`), since each request carries your password: a domain whose calendars live with another provider needs that server's address given.
- **Google**: signed in on Google's page with your own key (OAuth, no password), calendars and contacts over its CalDAV and CardDAV, tasks over Google Tasks; what Google does not keep shows greyed ([google.md](google.md)).
- **Moving** a task to another list or a contact to another address book, across accounts too: written there, then taken out here; when the place would not keep something it uses (Google), Sioul says what before moving.
- **Sync**: what changed here goes first (PUT with `If-Match`, new items with `If-None-Match: *`, DELETE), then what changed there, with the sync token when the server keeps one (RFC 6578), else by comparing ETags; changed items come back in batches (multiget). Every fifteen minutes in the background, and at once after a change in the window.
- **Editing keeps what Sioul does not show**: cards and events are changed line by line, so a photo, an alarm, a guest list or another application's fields come back exactly as they were. An event's time and repeat rule are rewritten only when you changed them: an event in another time zone keeps its zone, a rule the form cannot show ("every other Monday and Wednesday") stays whole.
- **Libraries**: `calcard` (Apache-2.0 or MIT, Stalwart) reads vCard and iCalendar and expands repeating events (RRULE, EXDATE, changed occurrences, time zones); the protocol is a small client of Sioul's own on `ureq` and `roxmltree`, since the handful of requests needed did not justify an asynchronous HTTP stack.

### Where it stands
| | |
|---|---|
| Done | accounts found from the address or a server address; address books and calendars listed and synced both ways; contacts: the list with search, the card (addresses with "Write", numbers with "Call", the rest folded), editing in place, new contacts, deleting with "Undo", completion of addresses when writing, a sender made a contact in one click; agenda: today then two weeks, the week, the month, an event's details with notes and guests folded, new events (whole days, repeating ones), editing, deleting this time or every time with "Undo"; invitations in mail: accept, maybe, decline (the answer goes to the organizer, iMIP), add a published event, take a cancelled one out |
| Next | moving an event to another calendar; changing one occurrence only; groups of contacts and photos; a calendar's colour chosen here |

## PGP (RFC 9580, PGP/MIME RFC 3156)
- **Sequoia** (`sequoia-openpgp`, LGPL-2.0-or-later), with its pure-Rust cryptography, so it builds the same on Windows.
- **Your keys** live in Sioul's store; their passphrase in the system keyring, so nothing is asked at each message. Keys come in by import (a file, or GnuPG's export), or are made by Sioul.
- **Others' keys**: from their messages (Autocrypt headers, attached keys), from their contact card, from the Web Key Directory of their domain and keys.openpgp.org.
- **Reading**: an encrypted message is decrypted on opening; a signature is checked and said next to the sender's shield (signed by whom, valid or not).
- **Writing**: two small toggles, sign and encrypt; encrypt is offered only when every recipient's key is known, and says whose key is missing otherwise.
- **In view**: the two toggles and the reader's line. **One step**: key details, fingerprints, import and export, in Accounts under "Encryption".

### As built
- **Sending**: PGP/MIME (RFC 3156). Signed alone: `multipart/signed`, the text parts in quoted-printable so trailing spaces (the "-- " line) and long lines arrive as signed. Encrypted: `multipart/encrypted`, signed inside, to every recipient's key and yours (to read it again in Sent). The subject stays readable, as in most clients today.
- **Reading**: PGP/MIME encrypted or signed, and inline PGP; a decrypted message lives only in memory, its attachments decrypted again when opened. The line under the sender says "Encrypted · Signed by …", "Signed with a key Sioul does not know", or "The signature does not match the text", in warm colours, never red. Answering an encrypted message keeps a decrypted copy with the draft and switches "Encrypt" on when every key is known.
- **Keys**: "Make a key for …" in Accounts makes a Curve25519 key valid three years, with a random passphrase the keyring keeps; "Import a key…" takes GnuPG's export (`gpg --export-secret-keys --armor you@example.org`), asks its passphrase once and keeps it in the keyring; "Save the public key" writes it to your downloads. Others' keys come from their messages (Autocrypt, only when the message is not forged), from a file, or from "Look for their keys" in the writing window (their domain's Web Key Directory, then keys.openpgp.org: only on request, since it tells a server to whom you write). Your outgoing mail carries your key in an Autocrypt header.
- **Tested** both ways with GnuPG 2.4: Sioul's signed and encrypted message decrypts in GnuPG with "Good signature"; GnuPG's encrypted, signed and tampered messages read as such in Sioul.
- **Next**: subject protection (protected headers), revoking and extending a key, trusting keys explicitly.

## Antivirus
- **A program is never opened from mail**: an attachment that runs (`.exe`, `.js`, `.lnk`, `.desktop`, `.terminal`, `.iso`…) can only be saved, and opening it stays your own act, outside Sioul. Files opened or saved from mail carry the system's mark that they came from the Internet (Windows' Mark of the Web, macOS' quarantine), so Office opens them in Protected View and macOS checks them.
- **An attachment is checked before it opens or is saved, when an antivirus is there** ([porch.md](porch.md)). Without one, nothing is refused: Sioul says the file will not be checked, asks before opening or saving it ("Open it unchecked"), and gives the command that installs one on this system.
- **Each system's own antivirus, nothing shipped**:
  - **Windows**: the Antimalware Scan Interface (AMSI), which hands the file to the antivirus Windows already runs (Defender by default).
  - **Linux and macOS**: ClamAV when the system has it (its packages; Homebrew on a Mac), as macOS always had it. Nothing of ClamAV ships with Sioul: ClamAV is GPL-2.0-only, and a separate program beside Sioul would be allowed, but the packages stay simpler and every system keeps its antivirus current itself. A Flatpak does not reach the system's ClamAV: there, Sioul asks.
- One interface in Sioul (`scan(file) → clean | threat | unavailable`), one implementation per system; `install_hint()` gives the command (`sudo dnf install clamav clamav-update && sudo freshclam` on Fedora, apt, pacman, zypper elsewhere, `brew install clamav` on a Mac).

### As built
- `sioul-sync/src/antivirus.rs`: ClamAV's daemon, then its scanner with the system's signatures, then with Sioul's own in `~/.local/share/sioul/clamav`, which Sioul refreshes with the system's `freshclam` once a day when the system keeps none. On Windows, AMSI. When none answers: the question, with the command.
- **Tested** on Linux without ClamAV (the question comes, with Fedora's command). The AMSI code is written to Microsoft's documented sequence and is checked by the Windows build of the workflow below; it has not run on Windows yet.

## Windows, macOS, then Android
- **Folders**: the system's own (`directories`): `%APPDATA%\\Sioul` on Windows, `~/Library/Application Support/Sioul` on macOS, XDG on Linux.
- **Maildir**: `:` is forbidden in Windows file names, so the info separator is `!` there, as mbsync does.
- **Passwords**: the keyring crate's Windows Credential Manager and macOS Keychain backends.
- **Notifications**: Windows toasts and macOS notifications (`notify-rust`).
- **Icons**: the Breeze icons Sioul uses (LGPL-3.0) ship inside it, since only Linux desktops have them.
- **DNS** for the sender checks: the system's resolver on each (hickory reads Windows' own settings).
- **Building**: Qt 6 from Qt's installer on Windows (MSVC) and macOS; a CI build on the three systems for each change.
- **Packages**: Flatpak on Linux, an installer on Windows (MSIX or Inno Setup, `windeployqt`), a disk image on macOS.
- **Android** comes after, with Qt for Android ([architecture.md](architecture.md)).

### As built
- **Folders**: the XDG variables when set, on every system (tests set them); else XDG on Linux, `%APPDATA%` and `%LOCALAPPDATA%` on Windows, `~/Library/Application Support` on macOS (`directories`). Downloads and the cache follow the same rule.
- **Maildir**: `!` before the flags on Windows; `:` and `!` are both read everywhere, so a Maildir copied across reads the same.
- **Keyring**: the Secret Service on Linux, the Credential Manager on Windows, the Keychain on macOS (the keyring crate's backend per system).
- **Notifications**: the "copy" button only where the desktop offers buttons (Linux); elsewhere the code is in the text.
- **Icons**: the Breeze icons Sioul uses ship inside the window, light and dark (`tools/bundle-icons.py`, 1.4 MB, Breeze's LGPL-3.0 licence alongside); the desktop's own theme still comes first on Linux.
- **OpenPGP**: Sequoia with its pure-Rust cryptography, the same on the three systems.
- **Building**: `.github/workflows/build.yml` tests the core, sync and command line, and builds the window, on Linux, Windows and macOS. It runs only when started by hand (Actions, "Build on three systems"), since on a private repository minutes are counted, macOS ones ten times.
- **Packages**: first files in `packaging/`: a Flatpak manifest (its open points written at the top), an Inno Setup script for Windows (after `windeployqt`), and the steps for a macOS bundle. None has been built yet.

## Order
1. **Mail, writing to the server**: folders, flags, archive, delete, junk, move; writing in Markdown, replies and forwards; sending; drafts.
2. **Contacts and calendars**: discovery, sync both ways, the list and the agenda, editing, invitations from mail.
3. **PGP**: reading (decrypt, verify) and writing (sign, encrypt), keys from Autocrypt and WKD.
4. **Antivirus and Windows**: AMSI, the system's ClamAV elsewhere, the system folders, the Maildir separator, the icons, the CI on three systems.
