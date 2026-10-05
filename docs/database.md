# Your data on several computers

Built: sharing through a folder your sync carries (Nextcloud, Dropbox, Syncthing), set up in Settings ▸ Between your computers. A database server stays an option, not built (below). Code: `crates/sioul-sync/src/share.rs` (the log), `crates/sioul-app/src/share.rs` and `qml/SharePanel.qml` (the window).

## What travels how
- **By their servers**: mail (IMAP), contacts, events and tasks (CardDAV, CalDAV, Google), with what Sioul writes in each task (kinds, projects, links, steps, waits: RFC 9253).
- **By their own folder**: the case store: projects and their routes (`sioul-cases.toml`), budgets and bank accounts (`sioul-budgets.toml`), the bank's movements (`sioul-bank.toml`), notes, their pictures, PDFs and memos. Sioul does not carry them: a file carried twice, by the sync and by the log, would undo changes. The Settings page says when the case store seems outside every synced folder (Nextcloud's own list of folders, `~/Nextcloud`, `~/Dropbox`, `~/Sync`…).
- **By the log, when asked**: a notes folder no sync carries (a phone's: eDrive syncs only its own folders) sends its projects, budgets and bank movements through the log instead: Settings ▸ Your folder and sharing ▸ "Projects, budgets and the bank travel here too, sealed" (`share_projects`), ticked on one device: the setting travels to the others. The notes themselves stay.
- **By the log**: what only this computer keeps:

| File | Shared as |
|---|---|
| `config.toml` | one entry per setting; accounts one entry each (by id) |
| `known-`, `blocked-`, `safe-`, `neutral-senders.txt` | one entry per line |
| `links.toml` | one entry per tie |
| `time/<month>.toml`, `time/running.toml` | one entry per session (start, task, project); the session running now whole |
| `drafts/`, `invoices/`, `pgp/others/`, `shield/` | one entry per file |
| `health.toml` | one entry per setting; prescriptions and medicines one each (by id) |
| `health-state.toml`, `quiet.toml`, `today.toml`, `money.toml`, `porch.toml` | one entry per setting, mark, dose, word; each account's Porch mark whole |
| `watch/` (a day each), `watch-offers.json` | one entry per file |
| lists kept on this computer only (`calendars/local/`, `contacts/local/`, their state in `dav/local/`) | one entry per file |
| with `share_projects`: `sioul-cases.toml`, `sioul-budgets.toml`, `sioul-bank.toml` from the notes folder | projects, budgets, presets, reserves and bank accounts one each (by id); a choice for one movement by its account and movement; lines, covers, mail rules and splits one each, as themselves; the bank's accounts by id, its movements by account and the bank's own id |

- **Never**: where things are on each computer (the case store, a sender list's own path, each account's mail folder, the watch's folder, how far back mail goes), how text reads on this screen (`reading`), how pages are laid out (`tasks-view.toml`), this computer's browser notices, your own PGP keys (secret keys never leave the computer they were made on: copy them by hand), sync's state, caches, the keyring. Passwords and tokens stay in each computer's keyring: an account arriving from another computer asks for its password once.

## The log
- **Each computer has a name**: a UUID made once (`$XDG_STATE_HOME/sioul/share/here.toml`), made again when the configuration was copied to another machine (a hash of `/etc/machine-id` tells).
- **Changes are found, not announced**: Sioul writes its files as before. Each minute (and on "Refresh everything"), the shared files are compared with what they held at the last look, entry by entry; a file whose size and time did not change is not read again. Each change becomes a record: the entry, its new value or "taken out", a clock.
- **The clock** is hybrid logical: the time in milliseconds, made strictly increasing on each computer and never behind a record already read. A change is dated by its file's time, so a change made here before another computer's, but found after, still loses to it.
- **Merging**: for each entry, the record with the later clock wins (ties broken by the computers' names). A setting changed on each computer keeps the later; two sessions noted apart both stay; a sender let in on one computer and blocked later on another ends blocked, and let in nowhere.
- **Joining**: on the first exchange, a copy of every shared file goes to `share/before-sharing-<day>/`; the others' records come first; then what this computer alone had goes out. Two computers set up apart end with the first one's settings and the sum of both lists.
- **Prudence**: a settings file missing or unreadable (half written, broken by hand) is never read as emptied: nothing of it is taken out elsewhere, and what comes for it waits, kept, until it can be written. A file over 16 MB stays here.
- **Records** go to `<folder>/<computer>-<round>.jsonl`, one line each, appended. Each computer writes its own file only, so the sync never makes conflicted copies. Each reads the others' from where it stopped (round and byte), whole lines only. Past 1 MB a computer starts a new round, opening on every entry it holds the last word on; an old round goes once every computer heard from in the last 180 days has read past it. Entries taken out are remembered 90 days.
- **Notes to each other**: `<folder>/<computer>.toml` says when each computer last exchanged and how far it read the others; written when that changes or a quarter of an hour after the last time.

## Leases: one computer at a time
Some work must happen on one computer only: reminding a dose (twice, it could be taken twice), numbering invoices (twice, one number twice), the gathered notification of sites. `sioul_sync::lease` keeps them apart through the same folder: each computer writes its own claim on a part, sealed like the records (`<folder>/leases/<part>/<computer>.lease`, one writer per file), renewed each minute while Sioul runs, alive five minutes after. Every computer reads the claims the same way:
- **following you** (medicines, the gathered notification): the live claim of the computer used most recently keeps it;
- **staying put** (invoices): the claim taken on purpose last ("Make invoices on this computer"), else the oldest.
A computer acts only once it has kept a part for a minute and a half, time for the others' claims to come through the sync. Invoices also want the other live computers heard from within three minutes: a stopped sync could hide a claim, so they wait rather than risk a number twice. Sharing off, this computer keeps everything.

## Sealed
- Each record is encrypted on the computer with XChaCha20-Poly1305, bound to its computer, round, number and clock. The folder and its server see which computer wrote, when and how much, never what: not the entries' names (they hold addresses), not their values.
- The key is made from a passphrase with Argon2id (64 MiB, 3 passes, a salt in `<folder>/seal.toml`), typed once on each computer (twice on the first) and kept in its keyring. `seal.toml` also holds a value sealed with the key: a wrong passphrase is told at once. At least 12 characters.
- Lost, the passphrase cannot be found again: stop sharing everywhere, start again with a new folder; each computer keeps its own files.

## Notes
- The notes travel with their folder's own sync.
- **A link keeps its file's name** (`sioul:note/admin/lease.md`, relative to the notes folder), so it holds on every computer, wherever the folder is.
- **A linked note not on this computer yet** (its sync not done, or the file removed elsewhere) shows faded, with its name and "Not on this computer yet: it may still be syncing (Nextcloud, Dropbox)."
- A note renamed in Sioul updates its links (tasks' LINK lines, `links.toml`); the other computer gets the new file from the folder, the new links from CalDAV and the log.

## Tested
`cargo test -p sioul-sync share`: two computers joining (settings, accounts without their local paths, sender lists summed, a draft, a session), working apart (a session each in the same month, a setting each, a draft sent on one), the later word winning across two lists, a broken file left alone, the seal (wrong passphrase refused, nothing readable in the folder), rounds started again and old ones removed, a third computer joining from the latest round, a phone's notes folder getting projects, budgets and bank movements (`share_projects`) and both keeping what each added. In the window, two test setups sharing one folder: the first alone, the second finding the folder sealed and joining, the first hearing from the second.

## Not built
- **A database server** carrying the same records, for those who want a server rather than a folder:

```sql
CREATE TABLE sioul_changes (
  computer CHAR(36) NOT NULL,
  round INT UNSIGNED NOT NULL,
  seq BIGINT UNSIGNED NOT NULL,
  clock BIGINT UNSIGNED NOT NULL,
  sealed VARBINARY(65535) NOT NULL,
  PRIMARY KEY (computer, round, seq)
);
```

  It needs the server's port open to the Internet with TLS (`REQUIRE SSL`), a user limited to this table, and on shared hosting each computer's address let in by hand. It brings one place at once, and transactions, which only matter for invoice numbers.
- **One series of invoice numbers per computer** ("2026-A-014"): built instead is one computer numbering them at a time, the others waiting their turn ([accounting.md](accounting.md), "Invoices on one computer"). French rules accept several series only when the way the activity runs justifies them (BOI-TVA-DECLA-30-20-20-10), your accountant's call.
- **Drafts in the server's Drafts folder** (IMAP), for other mail clients to see: drafts travel by the log for now.
- **The Porch's marks in IMAP METADATA** (RFC 5464) where the server has it: by the log for now.
- **Phones**: the format is plain (JSON lines, XChaCha20-Poly1305, Argon2id); no phone app reads it yet.
