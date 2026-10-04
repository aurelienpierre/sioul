---
description: Using Sioul on several computers - what travels by your servers, by your notes folder, and by Sioul's own sealed log through a Nextcloud, Dropbox or Syncthing folder, end-to-end encrypted.
---

# Sharing between your computers

Sioul on a desktop and on a laptop: most of what you see already travels by itself. What Sioul alone keeps can travel too, sealed with a passphrase, through a folder your sync service already carries: Nextcloud, Dropbox, Syncthing. There is no server of ours in between.

## What travels, and how

| What | How it reaches your other computers |
|---|---|
| Mail, contacts, events and tasks, with what Sioul writes in your tasks (steps, waits, links, kinds) | by their own servers, as with any program |
| Your notes folder: notes, projects, budgets, papers, scanned letters, pictures and memos | by the folder's own sync |
| What Sioul keeps on this computer alone: your settings and accounts (without passwords), who may write to you, the ties between things, time spent, drafts, invoices, medicines and doses, your watch's days, lists kept on this computer only, where the Porch was closed | by Sioul's sealed log, through a folder you choose |

**Never shared**: where things are on each computer (each one keeps its own folders), how text reads on this screen, how pages are laid out, this computer's browser notices, caches, and your own OpenPGP keys (copy them by hand). Passwords stay in each computer's keyring: an account arriving from another computer asks for its password once.

## Setting it up

On the first computer:

1. Open **Settings ▸ Your folder and sharing**, and find **Between your computers**.
2. **Folder**: choose a folder inside the one your sync carries, such as a new folder `Sioul` in your Nextcloud folder.
3. **Passphrase**, then **Once more**: a few words you will not forget, at least 12 characters.
4. **Share**.

On the other computer, choose the same folder (as your sync shows it there). Sioul sees that another computer shares through it, and asks for the passphrase chosen there. Then **Share**.

The first time, a copy of what this computer had is kept aside, in case. Then the other computer's settings come, and what this one alone had goes out: two computers set up apart end with the first one's settings and both of their lists.

From then on, changes are exchanged each minute, and when you choose **Refresh everything** or **Exchange now**. The panel says through which folder you share, with how many other computers, and when they were last heard from. **Stop sharing** ends it; each computer keeps its own files.

Your notes folder travels by its own sync, not by Sioul. If it does not seem to be inside a synced folder, the panel says so: your other computer would not see your notes and projects. Moved into one (and chosen again in Settings), they travel too.

## On a phone

Sioul for Android shares the same way, through the folder your phone's sync app keeps on the phone: Murena's eDrive, Syncthing, FolderSync, Nextcloud's own app. Sioul talks to none of them: it reads the folder.

- **Where**: some sync apps carry only a few folders. Murena's eDrive carries your cloud's **Documents** (with Pictures, Music…), not the rest of it: share through a folder inside Documents, such as `Documents/Sioul`. Sioul suggests one there when your synced folder has a Documents folder, and says so when the one chosen is outside it.
- **On the phone**: Settings ▸ Your folder and sharing, **Allow access to files** (Android's switch), then **Choose…** the folder, your passphrase, **Share**.
- **Accounts** come without their passwords: each asks for its own once, typed or [from Bitwarden](accounts.md#an-account-from-your-other-device).
- **eDrive's pace**: it brings the cloud's changes about every half hour, sooner when you sync your Murena account by hand; the phone's own changes go up at once. It never deletes on one side what was deleted on the other: the old rounds Sioul clears stay on the phone, and are not read again.

## Sealed

Each change is encrypted on your computer before it is written in the folder (XChaCha20-Poly1305), with a key made from your passphrase (Argon2id). The folder, and the server that carries it, see which computer wrote, when, and how much; never what: not the names of the things, not their values.

The passphrase is typed once on each computer, and kept in its keyring. A wrong one is said at once.

!!! warning "A lost passphrase cannot be found again"
    Nobody can recover it for you: it is never sent anywhere. If it is lost, stop sharing on every computer and start again with a new folder. Nothing on your computers is lost.

## When two computers change the same thing

The later change wins, one setting, one line, one entry at a time:

- a setting changed on each computer keeps the later one;
- time noted on each computer, both stay;
- a sender let in on one computer and blocked later on the other ends blocked everywhere.

A settings file that is half written, or broken by hand, is never read as emptied: nothing of it is taken out elsewhere.

## Some things, one computer at a time

- **Medicines** are reminded by the computer you are at only, so that a dose is not reminded twice. A dose marked taken goes to the others at once.
- **The sites' gathered notification** comes on the computer you are at.
- **Invoices** are numbered on one computer only, so that a number is never given twice. Another computer says where they are made, and offers **Make invoices on this computer**. See [Time and invoices](time.md#on-several-computers).

## Not there yet

A database server instead of a folder, for those who would rather have one; drafts in your mail server's Drafts folder, for other mail programs to see; phones other than Android's. The format is plain and documented, so that they can come.
