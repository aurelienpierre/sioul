# The notes folder

Your notes folder is a folder of **your own Markdown files**: an Obsidian vault, the folder of Nextcloud Notes, a git repository, a folder of notes. Beside your notes it holds what Sioul keeps as plain files: your projects (`sioul-projects.toml`, below), budgets and bank accounts (`sioul-budgets.toml`, `sioul-bank.toml`: [accounting.md](accounting.md)), contracts (`sioul-contracts.toml`), papers (`sioul-papers.toml` and `papers/`: [papers.md](papers.md)) and scanned letters (`letters/`). The configuration names it with `case_store`, its first key, kept so that every configuration reads as before. Sioul links to the records and never owns them: you, or an AI agent helping you, keep writing in your files as before.

A project is any matter you follow: a tax return, a health-cover request, a bill to contest, or work for a client ([projects.md](projects.md)). Its detailed record lives in your notes.

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
- **Renamed when you say so**: Settings ▸ Your folder and sharing ▸ "Rename to sioul-projects.toml" writes the file under the new names in one write, every field, route and comment kept, and checked to be so before anything is written; the old file stays beside it, as `sioul-cases.toml.before-rename` (`rename_file`). It is never done by itself: an older Sioul reads only the first name, so it waits until every device is updated.
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
- A project's tasks and events carry its id in `REFID` (RFC 9253), so other CalDAV clients keep the grouping.
- Notes refer to messages by `mid:` URIs (RFC 2392) and to tasks, events, contacts, projects and drafts by `sioul:` addresses (`sioul:task/<UID>`), in a Markdown link or the front matter ([tasks.md](tasks.md)). Sioul reads the folder as a vault: wikilinks, links back, tags.
- `sioul projects` lists the projects and flags files the manifest names but that are not there (`sioul cases`, its first name, still works).
