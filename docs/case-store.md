# The case store

A case is a dossier: a tax return, a health-cover request, a bill to contest. Its detailed record lives in **your own Markdown files**: an Obsidian vault, a git repository, a folder of notes. Sioul links to the record and never owns it. You, or an AI agent helping you, keep writing in your files as before.

## The manifest
One file at the root of the case store, `sioul-cases.toml`, names the cases (example: [../examples/sioul-cases.toml](../examples/sioul-cases.toml)).

```toml
[[case]]
id = "taxes-2025"            # short and stable: tasks, events and notes refer to it
title = "Income tax 2025"    # shown in the interface
files = ["admin/taxes-2025.md"]   # the record, relative to the store's root
status = "open"              # open, waiting, closed

[[case.route]]               # which mail belongs here
from_domains = ["impots.example"]

[[case.route]]
from_domains = ["hospital.example"]
subject_contains = ["certificate", "attestation"]
```

## Routes
- **A message belongs to a case** if any of its routes matches.
- **A route matches** when every list it fills has at least one match:
  - `from_domains`: the sender's domain or any subdomain ("finances.example" covers "dgfip.finances.example", not "notfinances.example");
  - `from_addresses`: an exact address, case ignored;
  - `subject_contains` and `text_contains`: whole words or phrases, case and accents ignored;
  - `attachment_contains`: words in an attachment's file name ("devis" takes `Devis_2026.pdf`), case and accents ignored.
- **Several cases may match**: the first in the file is the message's home; the others are listed as "also concerns".
- **A conversation follows its case**: a message that answers or cites one of a case's (In-Reply-To, References), or one tied to it by hand, goes to the same case ("in a conversation of this case"), unless it was set aside, is a code or is hostile.
- **Mail is tied to its case when it arrives** (`links.toml`, `mid:` ↔ `sioul:case/`), so the project page shows it whatever the route needed (the text, an attachment) and later replies follow it. Changing a case's routes ties the mail already here that its text or attachment routes take.
- **Every routing keeps its reason** ("sender's domain: impots.example"), shown on request.
- **Routes are edited** on the project's page ("Mail that comes here by itself") or in the Porch's lane for the case.

## In the standards
- A case's tasks and events carry its id in `REFID` (RFC 9253), so other CalDAV clients keep the grouping.
- Notes refer to messages by `mid:` URIs (RFC 2392) and to tasks, events, contacts and drafts by `sioul:` addresses (`sioul:task/<UID>`), in a Markdown link or the front matter ([tasks.md](tasks.md)). Sioul reads the store as a vault: wikilinks, links back, tags.
- `sioul cases` lists the cases and flags files the manifest names but that are not there.
