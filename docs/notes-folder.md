# The notes folder

Your notes folder is a folder of **your own Markdown files**: an Obsidian vault, the folder of Nextcloud Notes, a git repository, a folder of notes. The configuration names it with `case_store`, its first key, kept so that every configuration reads as before. Sioul links to the notes and never owns them: you, or an AI agent helping you, keep writing in your files as before.

Sioul's own lists are plain files too: the projects (`sioul-projects.toml`, below), budgets and bank accounts (`sioul-budgets.toml`, `sioul-bank.toml`: [accounting.md](accounting.md)), money movements (`sioul-movements.toml`), contracts (`sioul-contracts.toml`), the clients' profiles for invoices (`sioul-clients.toml`: [projects.md](projects.md#clients)), the papers' list (`sioul-papers.toml`: [papers.md](papers.md)) and the letters' list and texts. Every release before the move kept them at the root of the notes folder, with the papers' files in its `papers/` and the scanned letters in its `letters/`. Once a device has moved them (below, "What lives where"), they are in Sioul's data folder, the papers and letters in folders of their own, and the notes folder holds the person's notes alone.

A project is any matter you follow: a tax return, a health-cover request, a bill to contest, or work for a client ([projects.md](projects.md)). Its detailed record lives in your notes.

## Where it is set, and when there is none
- **Set on the Notes page**, the first row of its ⚙ ("The notes folder", the configuration's `case_store`), above "New notes go into". It is this device's own (`CONFIG_LOCAL` in the sharing): each device says where its copy is. Settings ▸ Sharing keeps one line saying which folder it is, since your notes travel from it, with a button to that row; every link to it by its key (`settings:case_store`, the Porch's own door, the sharing's line) opens it there (`settings::page_of`, `SettingDoor.qml`).
- **Its state** (`Config::notes_folder`): ready (set, and it reads), unreadable (set, and `read_dir` fails: a drive not plugged in, a folder moved or renamed), or unset (none set; on Android, `~/Notes` counts as set once it reads). The window holds it as the property `notesFolder` (JSON: `{"state", "path"}`), found off the window's thread at the start, when the folder is chosen, and at each minute, so that a drive plugged in is seen within a minute and a drive that does not answer never holds the window.
- **Asked for in the Notes page's place** while it is not ready (`FolderNeeded.qml`), never an empty list: unset, one sentence on what the folder is (plain Markdown notes that stay yours, which other programs may open and sync, which Sioul links to and never owns); unreadable, the folder by its path, and why it may not read. Under it, "Choose a folder…" ("Choose again…"), which opens the system's folder chooser right there; on Android, Sioul's own (`FolderBrowser.qml`), which asks for "All files access" as the sharing's chooser does. Chosen, it is written as `case_store` and the page fills; refused, the reason is said under the button. Tested by `tst_folderneeded.qml`, and on the real page at 1400 and at 412 points by `tst_mailopen.qml`.

## The manifest
One file among Sioul's lists, `sioul-projects.toml`, names the projects (example: [../examples/sioul-projects.toml](../examples/sioul-projects.toml)): in `records/` of the data folder once the device moved, at the root of the notes folder before. A project's `files` stay named from the notes folder's root, wherever the list is.

```toml
[[project]]
id = "taxes-2025"            # short and stable: tasks, events and notes refer to it
title = "Income tax 2025"    # shown in the interface
files = ["admin/taxes-2025.md"]   # the record, relative to the notes folder's root
status = "open"              # open, waiting, closed

[[project.route]]            # which mail belongs here
from_domains = ["impots.example"]

[[project.route]]
from_domains = ["hospital.example"]
subject_contains = ["certificate", "attestation"]
```

## Its first name
Before projects and cases became one thing, the file was `sioul-cases.toml` and each entry `[[case]]`. Nothing written then is lost (`sioul_core::projects`):
- **Read as before**: Sioul reads `sioul-projects.toml`, and `sioul-cases.toml` when only that one is there (`file_in`); `[[project]]` and `[[case]]` both read.
- **Written under the file's own names**: a change goes into the file read, under the table name it already uses, so that a device with an older Sioul still reads it. A new file is `sioul-projects.toml`, with `[[project]]`.
- **Renamed when you say so**: the Notes page's ⚙ ▸ "Rename to sioul-projects.toml" writes the file under the new names in one write, every field, route and comment kept, and checked to be so before anything is written; the old file stays beside it, as `sioul-cases.toml.before-rename` (`rename_file`). It is never done by itself: an older Sioul reads only the first name, so it waits until every device is updated.
- **Both files there**: Settings says so; Sioul reads `sioul-projects.toml` and leaves `sioul-cases.toml` as it is.
- **Shared**: the sharing's records keep the first names, `notes/sioul-cases.toml` and `case`, which every version reads; each device writes them into its own file under that file's names ([database.md](database.md)).
- **Addresses**: a project is `sioul:project/<id>`. `sioul:case/<id>`, as Sioul wrote it before, opens the same project, wherever it is written: a note, `links.toml`, a task, an agent's request. A note's front matter `case: <id>` still ties the note to the project.

## Routes
- **A message belongs to a project** if any of its routes matches.
- **A route matches** when every list it fills has at least one match:
  - `from_domains`: the sender's domain or any subdomain ("finances.example" covers "dgfip.finances.example", not "notfinances.example");
  - `from_addresses`: an exact address, case ignored;
  - `subject_contains` and `text_contains`: whole words or phrases, case and accents ignored;
  - `attachment_contains`: words in an attachment's file name ("devis" takes `Devis_2026.pdf`), case and accents ignored.
- **Several projects may match**: the first in the file is the message's home; the others are listed as "also concerns".
- **A conversation follows its project**: a message that answers or cites one of a project's (In-Reply-To, References), or one tied to it by hand, goes to the same project ("in a conversation of this project"), unless it was set aside, is a code or is hostile.
- **Mail is tied to its project when it arrives** (`links.toml`, `mid:` ↔ `sioul:project/`, `how = "project"`), so the project's page shows it whatever the route needed (the text, an attachment) and later replies follow it. Changing a project's routes ties the mail already here that its text or attachment routes take, as the Porch would take it (`porch::Gate::routed`): never what it sets aside. A tie you make by hand is `how = "link"`.
- **The Porch's checks hold everywhere a project takes mail** (`porch::admission`): its lanes, a project's page, the ties made when routes change, the agents' `list_projects` and what agents may read of each project (`consent::Consent::of`), the project a task made from a message is given. What it sets aside joins no project; what nothing authenticates joins by no route on its sender.
- **Every routing keeps its reason** ("sender's domain: impots.example"), shown on request.
- **Routes are edited** on the project's page ("Mail that comes here by itself") or in the Porch's lane for the project.

## In the standards
- A project's tasks and events carry its id in `X-SIOUL-REFID` (RFC 9253's `REFID` under Sioul's own name, which a phone's account manager can send back: [compatibility.md](compatibility.md), "A phone's ical4j"; `REFID` is read too), so other CalDAV clients keep the grouping.
- Notes refer to messages by `mid:` URIs (RFC 2392) and to tasks, events, contacts, projects and drafts by `sioul:` addresses (`sioul:task/<UID>`), in a Markdown link or the front matter ([tasks.md](tasks.md)). Sioul reads the folder as a vault: wikilinks, links back, tags.
- `sioul projects` lists the projects and flags files the manifest names but that are not there (`sioul cases`, its first name, still works).

## What lives where
Sioul's own lists leave the notes folder for a place of their own, once on each device, on the person's word, so that the notes folder holds the person's notes alone. Code: `sioul_core::storage` (the places, the move, the clean-up), `sioul_sync::share` (the gate, the moved stores, the guard), `crates/sioul-app/src/storage.rs` and `qml/StorageMove.qml` (the window). Decided on 18 November 2026: papers and letters in `Documents/Papers` and `Documents/Letters` by default; nothing taken out of the notes folder until every device has moved; the lists travel by Sioul's sharing alone afterwards; they stay plain text files, editable by hand (a database would be another work).

### The places

| Thing | Before the move (every release before) | After this device moved |
|---|---|---|
| Lists: projects (`sioul-projects.toml`, or `sioul-cases.toml` and its `.before-rename` copies), client profiles, budgets, the bank's lines, movements, contracts, the papers' list | the notes folder's root | `<data>/records/`, under the same names (`~/.local/share/sioul/records/` on Linux) |
| The letters' list and texts | `letters/letters.toml`, `letters/<id>.txt` in the notes folder | `<data>/records/letters/` |
| Papers' files | `papers/…` in the notes folder, `file = "papers/x.pdf"` | the papers' folder (`[papers] folder`, `Documents/Papers` by default), `file = "x.pdf"` |
| Letters' scans and the folder scans arrive in | `letters/<year>/…`, `letters/inbox/` in the notes folder | the letters' folder (`[letters] folder`, `Documents/Letters` by default), `file = "<year>/…"`, `inbox/` (unless `[letters] inbox` names another) |
| Lock files | beside each list, in the notes folder | beside each list, in `records/` |
| A project's notes | `files`, from the notes folder's root | unchanged |

- **One way to read them** (`Config::places()`): `Places::under(notes)` before the move, `Places::apart(records, notes, papers, letters)` after; lists are read from `records_root()`, the notes themselves still from `notes_root_path()`. Before the move a device with no notes folder has no lists; once moved, the lists need none.
- **Paths read both ways** (`Docs::path_of`): a device moved and one not yet read each other's `file` fields (`papers/x.pdf` and `x.pdf`), since the papers' list travels between devices at different steps; a path written by hand from the notes folder is still found; an absolute path stays as it is.
- **Where this device stands**: `storage.toml` in the state folder (`Storage`: `moved`, `from`, `cleaned`), read by `Config::load`, never from `config.toml`, so that nothing that travels can move it. A file that does not read counts as not moved: the old places are whole until the clean-up.
- **Defaults**: the documents folder as each system names it; on a phone, the folder the notes are in when they are on its shared storage, else Sioul's own storage, never a shared folder Sioul could not list without "All files access".
- **The papers' and letters' folders** are each device's own (`CONFIG_LOCAL`), set in Papers ▸ ⚙ and Letters ▸ ⚙ (`settings::for_view`, views `papers` and `letters`), read only once moved. Chosen again after the move, `storage::move_documents` copies every file to the new folder and checks each byte for byte, never over a different file already there (refused, said), switches the setting with every exchange held, then names the list's absolute paths into the old folder from the new one. The old folder stays as it was.

### The gate
- **Each device says what it knows** in its sealed entry (`devices::Entry::storage`): 1 (`STORAGE_KNOWS`) once its Sioul knows the new places, 2 (`STORAGE_MOVED`) once it moved.
- **Nothing moves anywhere until every other device that counts says 1** (`share::storage_stand`; `present`: not left, not forgotten, not said replaced, heard from within 180 days). An older Sioul would go on editing the copy left behind, so the window names it and asks to update it or forget it (Settings ▸ Devices). With no sharing set up, there is nothing to wait for.
- **The window** (`StorageMove.qml`) shows itself once, when the move is open and this device has not moved (`offer`, the view flag `storage-move-said`); Settings ▸ Sharing has a line saying where the lists are, and "Where Sioul keeps its lists" opens it at any time. It says what moves and where, with the two folders to choose; nothing moves before "Move now". With no sharing set up, it says that lists carried until then by a synced notes folder will no longer travel, and asks to set up the sharing on each device first.

### The move, and what makes each step safe to stop
1. **Plan** (`plan`): the lists at the notes folder's root (the projects' file under both names, its `.before-rename` copies); `letters.toml` and `*.txt` at the top of `letters/` to the records; everything else in `letters/` to the letters' folder, and in `papers/` to the papers' folder. Hidden files are skipped (locks, sync apps' files). A link anywhere is refused, and nothing is done. `check_folders`: each folder absolute and its own, none inside the notes folder nor holding it, none inside Sioul's own folders.
2. **Copy** (`prepare`): each file goes through a hidden name beside its place (`.<name>.sioul-move`), its bytes synced, compared with the source byte for byte, dated as the source; then the journal (`storage-move.toml`, in the state folder) names it as this move's, synced, **before** it is renamed into place. A file of the person's at a paper's or a letter's place is kept, and the copy takes a free name (`name-2.ext`) that its record names; at a list's place, the move is refused (`records-taken`). Stopped anywhere, the hidden copy is removed at the next run; a file the journal names and that holds its bytes is the move's own; one with the same bytes that the journal does not name is adopted, never taken back.
3. **Check** (`check`): every copy against its source, byte for byte, again.
4. **Dated copy** (`keep`): `<data>/moved/<day>/<path>`, a second name of the source where the disk allows (a hard link: nothing writes the old files any more), else a copy, checked byte for byte; taken out after 30 days (`sweep_kept`, after each exchange).
5. **Paths named** (`name_paths`): in the copies of the papers' and letters' lists only, `papers/x` becomes `x`, an absolute path inside the old `papers/` or `letters/` becomes relative, a renamed copy takes its new name; anything else is unchanged, checked to change nothing but `file` fields (`same_but_files`). The journal notes the new bytes' SHA-256 before the file is replaced.
6. **Switch** (`switch`), with every exchange held (`hold_exchanges`: this Sioul's own exchanges told to hurry and waited for, and the exchanges' lock) and every old list's own lock held: each source is looked at again (a list by its bytes, a paper or a scan by size and time); one changed since its copy is copied again, checked, kept, its paths named again; then `storage.toml` is written, synced: **the one step after which the device reads the new places.** Its entry then says 2.

- **A failure before the switch** (`move_now`) takes back what the move made (only files that still hold its bytes, the dated copies, the journal): the old places were never touched, and the window says why in one sentence (`storage-*`). **Killed instead**, nothing is taken back, and the next run finishes, tested at each of about fifty points (`each_point_stopped_then_run_again_finishes_once`).
- **The sharing never takes the moved stores for new ones**: each says where the memory knew it before (`Store::was`), honoured in the first exchange that reads them all after the move (`Memory::relocated`), so that a change made just before the move and not sent yet goes out, never replaced by an older record (`two_devices_at_different_steps_stay_in_step`).

### The clean-up
- **When**: once every device that counts says 2 (`clean`, from `app::storage::tidy` after each exchange). Nobody reads the old files any more, so a removal that a notes sync carries to the others harms nothing.
- **What**: only files whose bytes still equal the copy's (one changed since stays, said in the log), their lock files, then `papers/` and `letters/` once empty.
- **Who**: where a sync app carries the notes folder, the device every device agrees on (`StorageStand::cleaner`: the smallest sharing id among those that count), its sync carrying the removal; where none does, each device its own. Without sharing, or with no other device counting, only from a notes folder no sync app carries: another device reading it would not be known here.

### How they travel afterwards
- **Sioul's lists travel by the sealed sharing alone**, in the part "projects" (Projects and money), which once moved also carries the papers' list and the letters' list and texts (before: in "papers" and "notes"), so that they are on wherever sharing is on and never refused for a carried folder. Their records keep their names (`notes/sioul-cases.toml`, `notes/sioul-papers.toml`, `files/notes/letters/letters.toml`, `files/notes/letters/<id>.txt`). The part is on by default once moved (`shared_by_default`), but a recorded "off" stands, so the window offers to switch it on, ticked, saying why (`app::storage::move_now(…, carry)`).
- **Papers' files** travel in "papers" under `files/papers/…`; **the letters' scans** under a name of their own, `files/letters/…` (`LETTERS_FILES`), since their list and texts kept the old prefix in the records; it is written only once every device knows it (the gate), and `known_part` lets a device not moved pass those records over. "papers" is refused while a sync app carries the papers' or the letters' folder (`app::share::carried_for`): one carrier for a file.
- **The notes folder's switch** "a sync app carries my notes" then speaks of the notes alone; the old files that stay there until the clean-up are left out of the notes part (`NOT_NOTES_MOVED`).

### The guard
A list this device read before, now gone or of no bytes but blanks, is never read as "everything taken out": [database.md](database.md), "Sioul's own lists".

### Trying it
`sioul move-lists --notes … --records … --papers … --letters … --kept … --state …` runs the move on the folders given, without the window or the sharing: a trial on a copy of real files. Every folder is given; nothing of the device's own is read or written (the command loads `config.toml` for its language only). `--plan` lists `from → to` without copying; `--clean` then takes the old files out, as once every device moved. Afterwards every list in `records/` equals its source byte for byte, except `sioul-papers.toml` and `letters/letters.toml`, whose `file` fields are named from their new folders. Tests: `crates/sioul-core/src/storage_tests.rs` (the move on stand-in folders made from `tests/fixtures/old-layout/`), `crates/sioul-sync/src/share_lists_tests.rs` (the guard), `share_move_tests.rs` (devices at different steps, an older build, offline through the move, a removal carried).

### Limits
- **A writer that took the old path before the switch** and waited on that list's lock writes the old file after the switch: that change stays in the old file (never cleaned, since its bytes differ), not in the new one.
- **A list removed and written again with one entry** by a Sioul writer before the next exchange looks is taken as that one entry: the guard sees only gone or blank files.
- **A device whose lists travelled only by a notes folder's sync** (part "projects" off) diverges from the moved devices until it moves too; when it does, its part goes on and it joins, the records' values winning for an entry both changed. The window says so before the press.
- **Between the moves of two devices**, a scan made on one reaches the other only once both moved; its list entry comes at once.
- **Tasks' and events' `file://` links to a scan** (made by "A task for that date" and "Into the agenda") are not rewritten: they live on CalDAV servers. The matching that ties such a task to its letter still ties (`dated::links_scan` matches the old link by its end), but opening the link fails once the old files are cleaned.
- **A blank letters' list** written again from the records leaves a "(conflict …)" copy of the blank file beside it.
- **On Windows and Android**, the dated copy's hard links fall back to copies (untested there), and `sync_dir` does nothing on Windows.
