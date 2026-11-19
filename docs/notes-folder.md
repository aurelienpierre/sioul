# The notes folder

Your notes folder is a folder of **your own Markdown files**: an Obsidian vault, the folder of Nextcloud Notes, a git repository, a folder of notes. Beside your notes it holds what Sioul keeps as plain files: your projects (`sioul-projects.toml`, below), budgets and bank accounts (`sioul-budgets.toml`, `sioul-bank.toml`: [accounting.md](accounting.md)), contracts (`sioul-contracts.toml`), your clients' profiles for invoices (`sioul-clients.toml`: [projects.md](projects.md#clients)), papers (`sioul-papers.toml` and `papers/`: [papers.md](papers.md)) and scanned letters (`letters/`). The configuration names it with `case_store`, its first key, kept so that every configuration reads as before. Sioul links to the records and never owns them: you, or an AI agent helping you, keep writing in your files as before.

A project is any matter you follow: a tax return, a health-cover request, a bill to contest, or work for a client ([projects.md](projects.md)). Its detailed record lives in your notes.

## Where it is set, and when there is none
- **Set on the Notes page**, the first row of its ⚙ ("The notes folder", the configuration's `case_store`), above "New notes go into". It is this device's own (`CONFIG_LOCAL` in the sharing): each device says where its copy is. Settings ▸ Sharing keeps one line saying which folder it is, since your notes travel from it, with a button to that row; every link to it by its key (`settings:case_store`, the Porch's own door, the sharing's line) opens it there (`settings::page_of`, `SettingDoor.qml`).
- **Its state** (`Config::notes_folder`): ready (set, and it reads), unreadable (set, and `read_dir` fails: a drive not plugged in, a folder moved or renamed), or unset (none set; on Android, `~/Notes` counts as set once it reads). The window holds it as the property `notesFolder` (JSON: `{"state", "path"}`), found off the window's thread at the start, when the folder is chosen, and at each minute, so that a drive plugged in is seen within a minute and a drive that does not answer never holds the window.
- **Asked for in the Notes page's place** while it is not ready (`FolderNeeded.qml`), never an empty list: unset, one sentence on what the folder is (plain Markdown notes that stay yours, which other programs may open and sync, which Sioul links to and never owns); unreadable, the folder by its path, and why it may not read. Under it, "Choose a folder…" ("Choose again…"), which opens the system's folder chooser right there; on Android, Sioul's own (`FolderBrowser.qml`), which asks for "All files access" as the sharing's chooser does. Chosen, it is written as `case_store` and the page fills; refused, the reason is said under the button. Tested by `tst_folderneeded.qml`, and on the real page at 1400 and at 412 points by `tst_mailopen.qml`.

## The manifest
One file at the root of the notes folder, `sioul-projects.toml`, names the projects (example: [../examples/sioul-projects.toml](../examples/sioul-projects.toml)).

```toml
[[project]]
id = "taxes-2025"            # short and stable: tasks, events and notes refer to it
title = "Income tax 2025"    # shown in the interface
files = ["admin/taxes-2025.md"]   # the record, relative to the folder's root
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
