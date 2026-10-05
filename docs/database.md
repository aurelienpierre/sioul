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
| `health-state.toml`, `quiet.toml`, `today.toml`, `money.toml`, `porch.toml`, `stopped.toml` (where you stopped) | one entry per setting, mark, dose, word; each account's Porch mark whole |
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
- **Prudence**: a settings file missing or unreadable (half written, broken by hand) is never read as emptied: nothing of it is taken out elsewhere, and what comes for it waits, kept, until it can be written. A settings file that held entries and is found gone or of no bytes at all (a crash, a full disk, a file written in place) waits ten minutes before its entries count as taken out: never everything, everywhere, at once (`share-emptied`); a list emptied on purpose, or a file gone from a folder of files (a draft sent), is taken out at once. A file over 16 MB stays here.
- **Stores that leave, arrive or move**: a store no longer shared here (projects no longer shared, a notes folder unset) takes nothing out elsewhere; a store new here (projects shared from now on, a store a new version adds) or moved (another notes folder) joins as a new computer joins: its files are rebuilt from all the records first, then only what they alone hold goes out, so an old copy here never beats the others' newer values and an empty new folder is filled.
- **Records** go to `<folder>/<computer>-<round>.jsonl`, one line each, appended. Each computer writes its own file only, so the sync never makes conflicted copies. Each reads the others' from where it stopped (round and byte), whole lines only, each line on its own: a line that is not text or does not open (a disk's damage) is one broken line, said, and the next ones are read. Past 1 MB of growth a computer starts a new round, opening on every entry it holds the last word on; an old round goes once every computer heard from in the last 180 days has read past it. Entries taken out are remembered 90 days.
- **A file put back older**: a sync app may put back an older copy of a computer's file (after a conflict, or keeping old content when only the version changed). Its writer notices its own file shorter than it left it and starts a new round with all it holds (`share-own-cut`); a reader finding a file shorter than what it read, or a record whose number does not follow the last one, says so (`share-other-cut`, `share-other-gap`) and counts what that computer wrote after as not known (doses: a doubt) until its next round. A line left half written is cut before the next one is appended.
- **A memory restored or lost**: a computer whose own records in the folder go further than its memory (restored from a backup, its state lost, sharing stopped and started again) goes on numbering after the folder's last line, never again from the start, and reads every file again from all the records, its own too, so what it marked after the backup comes back (`share-own-ahead`).
- **Notes to each other**: `<folder>/<computer>.toml` says when each computer last exchanged and how far it read the others; written when that changes or a quarter of an hour after the last time.

## What the sync app must do
Very little, so that any sync app carries the folder: Nextcloud and ownCloud, Murena's eDrive, Syncthing, Dropbox, Google Drive, OneDrive, iCloud Drive, or another. The sharing asks only that **new files and files that grow are copied some day**, in any order, maybe late, maybe never deleted, maybe with a conflicted copy beside them.
- **Nothing is rewritten in place at the same size**: the records only grow; the few files rewritten (the notes to each other, the claims) change size at each writing, one step past the last, over a turn of several hundred writings (`share::pad_for`). Some sync apps tell a change by its size alone: eDrive brings a file changed on the server only when its size differs from the phone's copy (its `FileDiffUtils.getActionForFileDiff`), others compare size and time.
- **Half-written files never travel**: files are written under a name starting with a dot (which sync apps leave out), then renamed.
- **Conflicted copies, temporary files, files of other names** are never read: only `<id>-<round>.jsonl`, `<id>.toml`, `leases/<part>/<id>.lease` and `seal.toml` count.
- **Deletions are not needed**: eDrive never deletes a file deleted elsewhere; an old round left behind is never read again.
- **Files kept on demand** (OneDrive's, iCloud's and Google Drive's online-only files, Nextcloud's virtual files): a file not downloaded yet reads as empty or not at all, which is a file not here yet; the doses say what is not known meanwhile. Keep the sharing folder "always on this device" in those apps.
- **Timing is the sync app's**: the desktop's apps bring a change within seconds; a phone's often look every half hour. On a phone, Sioul asks the sync app to look now when it offers a way (`CARRIERS` in `crates/sioul-app/src/share.rs`: eDrive's "force scan" today; Syncthing, FolderSync and others can be added the same way), after writing, when Sioul comes back on the screen, and every five minutes while it is shown. Without one, changes come at the sync app's own pace, and the doses say what is not known until then: nothing depends on it but how soon.
- **Tested against each kind of carrier**: a simulator copies the folder between a phone and a server as each kind does (every change both ways; size only; late and out of order; older copies put back with conflicted copies; files on demand); doses are never taken as known when they are not, and every dose comes in the end (`doses_are_never_known_wrongly_whatever_carries_them`).

## Knowing what the others wrote
For the doses (docs/health.md, "Knowing"), a computer must know whether it has read everything another wrote. Each claim says how far its computer wrote its records (round and number, `lease::Claim::wrote`), and whether it closed (`closed`: Sioul quit, or a phone put it away, after one last exchange); each computer keeps the round and number of the last record it read from each other (`Memory::read_n`, `share::heard`). A line that does not read is never passed over in silence: the problem is said (`share-other-line`) and when it was found is kept (`Memory::broken`). A file this computer can no longer trust (the doses' record broken or lost) can be rebuilt (`share::rebuild`): its entries are forgotten here, so nothing of it is taken out elsewhere, and read again from every computer's records, this one's too, at the next exchange. Files the window writes too are written under a lock beside them (`sioul_core::filelock`, `.<name>.lock`, which hidden names keep out of the sharing).

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
`cargo test -p sioul-sync share`: what the sync app must do (above: five kinds of carrier, eDrive's rule on each rewrite, a file put back shorter, a memory restored or lost, a gap, a damaged line, a half-written line, a big round, stores that leave, arrive or move, a settings file emptied), two computers joining (settings, accounts without their local paths, sender lists summed, a draft, a session), working apart (a session each in the same month, a setting each, a draft sent on one), the later word winning across two lists, a broken file left alone, the seal (wrong passphrase refused, nothing readable in the folder), rounds started again and old ones removed, a third computer joining from the latest round, a phone's notes folder getting projects, budgets and bank movements (`share_projects`) and both keeping what each added. In the window, two test setups sharing one folder: the first alone, the second finding the folder sealed and joining, the first hearing from the second.

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
