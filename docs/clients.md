# Clients and their files

Whoever works for others keeps things about them: who to bill, when they came, what was done, what they said, a consent signed. Sioul keeps a **client** as an object of its own, tied to one or several contact cards, and, when the person uses it, a **client's file**: a folder and a set of databases. The person who only sends invoices sees a name, its cards and its billing identity, and nothing else: no template, no folder, no question exists until the file is used. Whoever keeps records of care finds the protections a care practice needs as options, proposed together by a start by trade, never forced.

Code: `crates/sioul-core/src/clients.rs` (today's client profiles, the countries, `resolve`) and its submodules in `crates/sioul-core/src/clients/`: `record` (the client), `file` (its file's parts), `template` (the two templates and their evolutions, the starts by trade of `crates/sioul-core/data/client-starts.toml`), `ties` (ties and their index), `appointment` (how an appointment finds its client), `extension` (an extension's people as clients), `protect` (what protects), `keep` (keeping, giving, erasing, Same as…), `moving` (moving today's profiles), `door` (the store's door), `address`. What turns them on in the window: `crates/sioul-app/src/clientfiles.rs`; their carrying between devices: `crates/sioul-sync/src/share.rs` (`CLIENT_FILES_STORE`, the gates); their records as a database at the export: `crates/sioul-cli/src/clientsdb.rs`. Its interface for the window: the module docs.

## The store

Every part of a client's file is a file of its own in Sioul's sealed store ([store.md](store.md)), under the logical folder `data/clients/` (the data folder's `clients/`): a damaged file costs that one row, note, tie or attachment, never the file. Every file there is sealed on the disk with everything Sioul keeps: once out in clear (`sioul store export --clients`, below), they are Markdown, TOML and a SQLite database, readable without Sioul. The code reaches the store through `clients::door::Door`, shaped on the store's calls: a read gives a generation, a write may expect one (`Expect::Absent`, `Expect::Generation`), a listing is one folder, a purge leaves nothing. **A failure is never an empty value**: `Ok(None)` means never written or removed; a file that does not open is `Error::Damaged`, counted by every listing (`Read::damaged`), and the reader keeps what it has.

**Which door** (`door::opening`): a door put in place (`door::install`, tests), else the profile's own store as this program reaches it at each call (`sealed::files::profile_store`: the one the window mounted after the screen before it; the one the command line and the agents' server open at their first need), bound by `door::Sealed`, each call one of the store's (`read`, `write`, `remove`, `list`, `folders`, `purge`, `purge_under`), its errors mapped one to one and naming ids alone. Three states, which every hook below reads:

- **Open**: the store's door.
- **No store** (a phone until its own files are sealed, a profile a sync app carries, the demo without its key): **nothing of a client is kept on that device**, no plain file is ever written; every hook does nothing, and the part does not travel there (below).
- **Wait** (the profile is sealed and its store cannot be opened yet: the keyring locked, or late to answer): nothing is made, moved or decided absent; `Clients::load` says so rather than bill a client by its address as if none existed; what protects keeps every card from agents (`protect::kept_for_agents_in`).

A client's file is written once while another device holds the clients' files (its own files sealed and its part on: `share::Outcome::client_files_elsewhere`, `door::set_travels`), and twice until then (`How::only_here`, the store's twin), since it exists nowhere else: beside a phone whose files are not sealed, a computer keeps two copies. `door::Memory` serves the tests, beside `door::Sealed` on a test store (`door::sealed_store`).

```
data/clients/<id>/client.toml              the record
data/clients/<id>/description.md           timeless, Markdown
data/clients/<id>/notes/<note>.md          front matter: folder, row, ties, title, health, made
data/clients/<id>/rows.toml                each row's kind and time: rebuilt from the rows when lost
data/clients/<id>/db/<kind>/<row>.toml     one row
data/clients/<id>/files/<file>             an attachment's bytes
data/clients/<id>/files/<file>.meta.toml   its name, folder, row, type, size, health mark
data/clients/<id>/files/<file>.text        a PDF's text, once read
data/clients/<id>/ties/<tie>.toml          one tie
data/clients/declined/<mark>.toml          "Not a client" for an extension's person
data/clients/erased/<id>.toml              a client erased: its id and a day, no name
data/clients/moved.toml                    what the move made once (below)
```

Beside them, in this device's sealed state and never shared, `state/clients/purged.toml`: which erased clients this device purged already.

Every path is made of ids: **no client's name is ever in a file name**, a diagnostic line (`Error`'s words name ids alone) or a log line (counts alone).

## The client (lean)

`record::Client`: `id` (a UUID; its address `sioul:client/<id>`), `name` ("" for its first card's), `cards` (contact UIDs), `billing` (today's profile's fields: kind, legal name, addresses, identifiers, language, currency, terms, e-mail, purchase order), `from` and `mark` (an extension's set and person), `agreed`, `kept_from_agents`, `archived`, `declined`, `joined`, the folder template's version its file was made with, its own folders. `since`, `last` and `next` are computed from its ties (`ties::Index::facts`), never stored.

- A change is written **key by key** into the record as it stands (`record::change`), so what another device or a newer Sioul wrote stays; a write that finds the record changed meanwhile reads it again and redoes the change.
- **One card, one client.** A client made with a card has the id made from its first card (`record::card_id`, UUID v5): two devices make one client. A card a client holds already gives that client (`record::make`); where two hold one (made apart), the smallest id answers everywhere (`record::of_card`). A client without a card has a random id. Its cards are a table (UID → place), each travelling on its own.
- **Same as…** (`keep::same_as`) joins two clients that are one human; **the smaller id is always kept**, whichever way it was said, so that two joins said in opposite directions on two devices end as one client. Every part moves, every key of the record is merged (Kept from AI agents if either was, the earlier agreement, own folders, profile, mark, billing where the kept one has none); the other record only redirects (`joined`).
- **One billing identity, one truth.** A client moved from a profile names it (`profile`). While the profile is in `sioul-clients.toml`, the billing identity is the profile's: read by `Client::billing_of`, written by `record::change_billing`, as a 0.1.0 device reads and writes it. When every device knows clients, the client adopts it (`moving::adopt`) and the profile leaves the file. `record::Client::invoice_client` gives what a draft freezes at issue. Once the store is open, `Clients::load` lists the file's profiles, then each client whose profile is not there as a profile whose id is its address (and its old profile's id, once adopted), so that a draft naming `sioul:client/<id>` finds its billing identity wherever profiles are read (`resolve`: what a store client's billing lacks comes from its card, never what it says: the card's postal lines, postcode and city when the billing has none of them, the card's country only when the billing says none, so that a country set in the invoice editor, which decides VAT, is never replaced or blanked). **A client made from a card keeps no name of its own**: it bills, is listed and is found in the invoices' picker by its card's name as the card says it now (`clients::name_shown`), so that a card renamed is followed. **A billing identity changed from the window is written where it lives** (`clients::change_where_it_lives`): a profile of the file by its id, a client of the store by its address or by the id of the profile it adopted (`record::change_billing`), so that the profile form and a draft's client kind or country never answer "changed elsewhere" for a client.

## The file

A description; notes in folders; databases (one per kind of record, a row a file); attachments (`file`). A row keeps its fields' values **as written** (TOML values by field id), the kinds' template version it was written with (`schema`), its day or time, its health mark and the supplies it used. `Row::value` reads a field through its template: `Value::Empty` for a field added after the row (no default is ever written into older rows), `Value::AsWrittenBefore` for a value that no longer fits its field's changed type, `Value::Unknown` for a field this device does not know. A row saved untouched is not kept (`Error::Empty`).

## The templates

Two templates, one for every file, settings in `config.toml` carried like every setting: the folder tree and the kinds of record. Each folder, kind, field and choice is a **table keyed by its id** (`[clients.folder.<id>]`, `[clients.kind.<id>]`, `[clients.kind.<id>.field.<id>]`, `….choice.<id>`): the sharing carries each value on its own, where an element of an array of tables would travel whole and two devices changing two fields of one kind would lose one. Each template has a version (`folders_version`, `kinds_version`) raised at every change; a removal is the version it happened at, never a deletion.

| Evolution | What it does to files |
|---|---|
| a folder added, renamed | everywhere at once; its id stays |
| a folder removed | it stays only in files that hold something there: "No longer in your template", with Move its contents… |
| a folder this device does not know | "Not in this device's template yet" |
| a field added | older rows show it empty; nothing is written into them |
| a field renamed | its id, and so every value, stays |
| a field's type changed | a value that no longer fits is kept as written, "as written before", asked again only when that row is edited |
| a field removed, brought back | hidden, its values kept; back as they were |
| a kind removed | archived: its rows stay, shown; no new one is made |
| a choice added, renamed, archived | alike; a row holding an archived choice still reads it |
| split, merge | not offered: old rows would change meaning |

**An older device** reads a row and never writes it when the row holds a field, a field type or a choice its template lacks (`Row::read_only`, `Error::NewerTemplate`); never because of a version number, which travels as a setting and may go back when two devices change the template at once. Rows are written field by field (`file::change_row`), so nothing a newer device wrote is lost. Every evolution returns the part as it was (`template::Evolved`); `template::undo` puts it back and raises the version again.

**Starts by trade** are data (`data/client-starts.toml`): *Beauty care*, *Care practice*, *Consulting*, *Craft*, each a set of kinds and folders written in the window's language where their ids are free, then the person's to change. A start proposes options that protect (`proposes`: `keep-tied`, `stay-here`, `keeping`), each shown with its sentence; it never sets them.

## Ties

Every tie is two-way and kept where the thing that makes it keeps its own data, never on a server's copy (no `CONTACT`, no `X-SIOUL-LINK`, no category written for a client):

| Tie | Kept in |
|---|---|
| a card | the client's record (`cards`) |
| an invoice, its card, a line's row | the invoice: `client_ref`, `contact`, a line's `row` |
| a project | the project's `client` |
| a supply used by a row | the supply's movement, `for` |
| an appointment, a task, a time entry, a message tied by hand | the client's tie files |
| a note's ties | the note's front matter |
| an appointment, by itself | a `CONTACT` line or a guest's address naming one of a client's cards (an extension's door writes the `CONTACT` of the person its event is with) |

**A tie file** (`format = 1`) is named from its two ends, so two devices making one tie write one file; it keeps what an appointment said (`day`, `title`) for when the event is not here. It is only ever changed in place: **every key a later version adds is kept**. `[shown]` (`title`, `place`, `notes`) is reserved for an appointment's real words when its event on the server says something neutral: written by Sioul alone, never by `refresh_said`, read over the event's title (`Tie::words`), erased with the client.

**An event absent on this device is not gone**: a calendar may live on another device only. No sweep, merge or move removes or rewrites a tie because its event is not here; only `untie` (the person's word) and `row_removed` (a row taken out: its ties move to the client, its notes become the file's) touch tie files. A row taken out leaves an invoice's line and a supply's movement naming it: the index ties them to the client.

`ties::Index` reads every tie in memory, never writes it, and answers from both ends: `of_client`, `of_row`, `clients_of(address)`, `client_of_card`, `facts`. An invoice is tied by its client's address, its card, the profile's id it names (what 0.1.0 writes for a profile without a card), or exactly one client's billing name. The global tie index (`crate::ties`) reads the clients too (`ties::Sources::clients`), never a part of a file, so a card, an event, an invoice or a project lists "Client: …" through `World::related`, and `links::Kind::Client` names `sioul:client/…`. **A client's address is never written into a task, an event or a note** (`Loaded::tie` refuses it): its ties are the clients' own.

## How an appointment finds its client

`appointment::client_of_event`, in this order: a tie (by hand, or a row written on it); a card the event names (a `CONTACT` line's `ALTREP`, read into `agenda::EventRef::contacts`, `sioul:contact/<UID>` or `urn:uuid:<UID>`, or a guest's address on a client's card). An extension's appointment is the second case: the extension names the person on the event (`X-SIOUL-PERSON`, [extensions.md](extensions.md)) and its door writes that person's card as the event's `CONTACT`. Else, when its title holds exactly one client's name (every word, in order, case and accents aside), "With Marie Exemple?" is offered (`Found::Offer`): **a name alone never ties**.

## An extension's people as clients

An extension says it (docs/extensions.md, "What it hands back: contacts"): a set with `"clients": true`. While Clients is on and the person has not taken the set back (`[extensions.clients]`, `"<extension>/<set>" = false`, "They are not my clients"), each person of the set becomes a client after its cards are written (`extension::after_set`), its id made from the person's mark, `from` the set; a client already holding the person's card keeps its record, the mark noted on it. "Not a client" for one (`extension::decline`) keeps the record out of every list and a mark in `data/clients/declined/`, so that no refresh makes them again, even once the record is erased. The core infers nothing from whose an agenda is and names no extension.

For such a set, the door writes no category on the cards it makes and takes the extension's category off the cards it made, where it is still exactly its word (`extensions::cards::without_category`): a category on a server's card would say who is a client. The mark and the hash stay.

Whom an extension's appointment is with is the door's own `CONTACT` (above): no map of the clients' is kept.

## What protects

`[clients.protect]`, every option off but "Rules may act on clients' files": **nothing that reaches agents or rules today closes because someone is a client.**

- **A client follows what its cards and projects are kept from** (`protect::kept_by_consent`): one card kept, the client kept; a project its cards are in, or that names it, closed: the client kept; in none, as things outside projects are. The parts of its file the index names are closed unless the person opened the files to agents. **Every protection fails closed**: a key of `[clients.protect]` that does not read takes its protective value (`Protect::of`); a clients' list that does not open, or a store sealed here and not open yet, keeps every card from agents; on a device that holds no store, "Keep from AI agents what is tied to a client" keeps the client profiles' cards (every card when their file does not read).
- `keep_tied`, "Keep from AI agents what is tied to a client", and each client's mark `kept_from_agents`: `protect::kept_from_agents` gives the addresses agents must not get (the client, its cards, the messages from their cards' addresses (the mail index keeps senders: mail sent to a client is not among them yet), its appointments, tasks, time and invoices), which `sioul mcp` passes to `Consent::keep_persons`, counted, never named.
- `agents_files`, "AI agents may read and write clients' files": the file's contents are closed to agents until it is on, and always for a client marked Kept from AI agents (`protect::files_open_to_agents`); the client's record and its ties reach agents as contacts and client profiles did.
- An invoice for a client whose file holds a health mark carries `health = true` (`protect::health_invoice`): kept from agents while they may not read the files, its printed copy named by its number alone (`invoice::pdf_path`). **At issue** (`protect::health_at_issue`, in `invoice::life::issue`) the client is found as `resolve` finds it: by its address, by a profile's id or name (what 0.1.0 writes, and every draft until the drafts' gate), by the card a bare UID in `contact` names. **What cannot be read counts as health**: the store waiting for its keyring, a record, a note, a row or an attachment that does not open (`file::holds_health` fails closed), a configuration that does not read.
- `archive_by_itself`, `active_years`, `kept_years`: off until set. `keep::due_to_archive` lists whom Sioul archives by itself; `keep::reached_end` counts those whose keeping time is over, for a yearly sentence; **Sioul never erases by itself**.
- Health marks: on a field, a kind or a folder (the template), on a note, a row or an attachment (`file::set_health`). `health_profession` keeps the answer to the one question asked when a health mark is first used.
- "Clients' files stay on this device" is not here: its one place is the part's switch in Settings ▸ Sharing.
- There is no lock of Sioul's own: a client's file opens with the session, like everything Sioul keeps.

## Keeping, giving, erasing

- **Export for them…** (`keep::export`): `client.json` (name, billing identity where it lives now: its profile's until the gate), the description, the notes in Markdown and the attachments in their folders (named by the template's words), each kind's rows as JSON with their fields' names (GDPR art. 20).
- **Erase…** (`keep::erase`) writes a mark first, `data/clients/erased/<id>.toml` (its id and a day: no name), for the client and each client joined into it, notes them as moved (`moved.toml`), then purges everything ever written under their folders, parts removed earlier and their earlier versions included (`Door::purge_under`), and takes their profiles out of `sioul-clients.toml`, so that no device makes them again; it returns what stays: issued invoices (kept ten years), the cards and past events (offered apart). Refused within the keeping time set by a health professional (`Error::KeptUntil`). The ten seconds of Undo are the window's, before it calls.
- **On the other devices** the mark travels with the part (a device whose part is off keeps the client until the part is on there again, then the mark purges it, as the window's sentence says); each device purges what is left of that client there, its earlier versions with it, once, and again whenever a file of it comes back (an older copy, a backup put back): `keep::apply_erased`, run by every pass of the move, and the sharing's own earlier versions of those files (`share::forget_erased_client`). An erased client's id is never written again (`record::make` gives a card whose client was erased a new id); **met again** (its key lost from `moved.toml`, which travels as one file; a joined client's person met by an extension's refresh before its "Not a client" travelled), it stays erased and is noted as done (`Moved::erased`, `Taken::declined`), never an error that would stop the move or a refresh at every pass; a draft billing its card is never rewritten toward it (the drafts move only toward a client that is there) and keeps billing the card as it did. An erase declines the marks of every client joined into it too. Its sealed contents leave the sharing folder as any file taken out does.
- **Out in clear**: `sioul store export --to DIR --clients` writes every file of `data/clients/` at its path, the description and notes as Markdown, the records and rows as TOML, the attachments as they came, and, for each client with records, `records.sqlite` (a table per kind of record named as the template names it, the columns `id`, `when`, `health`, then a column per field named by the template, a choice by its name; `crates/sioul-cli/src/clientsdb.rs`) and `records/<kind>.csv` (the same table, RFC 4180), which any database tool or spreadsheet opens: a column `supplies` says what each row used ("Clay mask × 1", by the supplies' list), a supply field names its supply, an attachment field its name and its place in the export ("Before.jpg (files/<id>)"); a row that does not open is left out of both and counted with the export's sentences. **Without `--clients` nothing of a client is written, printed or listed** (`sioul store export`, `cat`, `ls`, `check`, `unseal`): neither `data/clients/`, nor this device's notes about them (`state/clients/`), nor the sharing's earlier versions of their files (any part `clients` below `state/share/`), nor a copy of any of these that a start set aside (`data/set-aside/<day>/…`, read by what follows the day).

## Moving what exists

`moving::plan` (pure, the same on every device) and `moving::apply`:
- a client profile becomes a client naming it, with its card, its id made from its card's UID when it names one, else from the profile's id; a client holding the card already (found by `profile`, then by card) gets the profile instead; its billing stays the profile's until the gate;
- a project's card, a draft's or an issued invoice's card: that card's human becomes a client; a name alone makes none;
- Sioul's links from an event to a client's card become the client's ties; taking them out of the events, where they may be written, goes through the window's unlink (`work::unlink`: a meeting with others is left as it is), only through the gate;
- a profiles' file that does not read stops the move with its error: never a move made of nothing; each thing moves once (`data/clients/moved.toml`), so a client erased since never comes back;
- issued invoices are read, never rewritten.

**When it runs** (`sioul-app/src/clientfiles.rs`): once the store is open, at the first exchange's turn after the window opens (with sharing or without), again when the gate opens, and whenever an exchange brought clients' files or client profiles. A file that does not read makes it wait for the next turn.

**Mixed versions: drafts and profiles wait for every device to hold clients.** A draft's `client_ref` becomes the client's address, `contact` beside it, and each profile is adopted and leaves the file, only once every device holds clients' files: its files sealed and the part on (`share::client_files_held`, below). A device that does not hold them (a phone whose files are not sealed yet, a device with the part off, an older Sioul) would find no client by that address. A 0.1.0 device reading a draft that says `sioul:client/<id>` would find no profile and freeze that text as the client's name at issue (`resolve` falls back to the reference as written; the test `an_older_device_reads_drafts_only_before_the_gate`). Until then a draft's old reference (a card's address, a profile's id or name) reads as the client of its card. An invoice's `contact`, `health` and a line's `row` are new keys: a 0.1.0 device reads around them, and drops them when it rewrites a draft. Dropping `contact` and `row` loses those ties; dropping `health` loses more: the draft is then no longer named by its number alone nor kept from agents until a device of this version marks it again at issue (`protect::health_invoice`). A device on 0.1.0 silent for longer than the sharing counts it (`SILENT_DAYS`) is not asked: back after the gate opened, it would print a client's address as a name on a draft it issues, or bill a client whose profile left the file; the same holds of the persons' and profiles' gates.

## Between devices

The part **"Clients' files"** (`share::CLIENT_FILES_PART`, its one switch in Settings ▸ Sharing, on like every part once sharing is set up): the clients' folder travels as one store of the sharing, `files/clients/` in the records, each file whole and sealed apart in the folder (`Shape::Files`, as the notes): an attachment travels as a sealed file, an attachment over 64 MiB stays on its device, and of two changes of one file made at once on two devices the later wins, the other kept beside it as the sharing's conflict copy ("client (conflict …).toml"), which the clients' code passes over (`door::Sealed::list` takes only the names it makes) and the export writes out, and a version replaced by another device's change is kept as an earlier version on the device it replaced (the sharing's `history`). A device reads and writes the store's files through their paths (`sealed::files`), so that what arrives is sealed where it lands; whoever carries the folder sees how many files, their sizes and when they change, never a name or a word.

Two gates, each kept open for good once open (the sharing's memory):

| Gate | Opens when | What it opens |
|---|---|---|
| `client_files_shared` | every other device says `client_files` ≥ 1 in its entry: its Sioul knows clients' files | the part travels, between the devices that hold it |
| `client_files_held` | this device carries the part now, and every other device says 2: its files are sealed and its part is on | drafts naming their client by its address, profiles adopted (the move through its gate) |

- **A device whose files are not sealed** (a phone today) says 1: it knows the records, never takes the part in (`client_files_here`: the store's folder is not sealed there), and nothing of a client is written there. It keeps the second gate shut: drafts and profiles stay as they are, and invoices go on billing from profiles there.
- **A device on a Sioul before this one** (0.1.0, or main before clients' files) says nothing: the part waits, so it never meets a record it would keep waiting at every exchange, and loses nothing; its own profiles and drafts are untouched. Once it is updated, the files travel to it.
- **The part off on a device**: its clients stay there and nothing of them is sent or taken in; it says 1, so the second gate waits for it too.
- **Sweeping**: a sealed content none of their records names goes only where every part whose files are sealed apart is read (`FILE_PARTS`, the clients' files once they travel), as for notes and papers.
