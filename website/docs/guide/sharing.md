---
description: Using Sioul on several computers and a phone - what travels by your servers, by your notes folder, and by Sioul's own sealed log through a Nextcloud, Dropbox or Syncthing folder, end-to-end encrypted, part by part, with earlier versions to put back.
---

# Sharing between your computers

Sioul on a desktop and on a laptop: most of what you see already travels by itself. What Sioul alone keeps can travel too, sealed with a passphrase, through a folder your sync service already carries: Nextcloud, Dropbox, Syncthing. There is no server of ours in between.

## What travels, and how

| What | How it reaches your other computers |
|---|---|
| Mail, contacts, events and tasks, with what Sioul writes in your tasks (steps, waits, links, kinds) | by their own servers, as with any program |
| Your notes folder: notes, projects, budgets, papers, scanned letters, pictures and memos | by the folder's own sync; or, when no sync carries it (a phone's), by Sioul's sealed log, once you switch them on |
| What Sioul keeps on this computer alone: your settings and accounts (without passwords), who may write to you, the ties between things, time spent, drafts, invoices, medicines and doses, your watch's days, lists kept on this computer only, where the Porch was closed | by Sioul's sealed log, through a folder you choose |

**Never shared**: what each device chooses to share, where things are on each computer (each one keeps its own folders), how text reads on this screen, how pages are laid out, this computer's browser notices, caches, and your own OpenPGP keys (copy them by hand). Passwords stay in each computer's keyring: an account arriving from another computer asks for its password once.

## Setting it up

On the first computer:

1. Open **Settings ▸ Your folder and sharing**, and find **Between your computers**.
2. **Folder**: choose a folder inside the one your sync carries, such as a new folder `Sioul` in your Nextcloud folder.
3. **Passphrase**, then **Once more**: a few words you will not forget, at least 12 characters.
4. **Share**.

On the other computer, choose the same folder (as your sync shows it there). Sioul sees that another computer shares through it, and asks for the passphrase chosen there. Then **Share**.

The first time, a copy of what this computer had is kept aside, in case. Then the other computer's settings come, and what this one alone had goes out: two computers set up apart end with the first one's settings and both of their lists.

From then on, changes are exchanged each minute, and when you choose **Refresh everything** or **Exchange now**. The panel says through which folder you share, with how many other computers, and when they were last heard from. **Stop sharing** ends it; each computer keeps its own files.

Your notes folder travels by its own sync, not by Sioul, unless you switch it on below. If it does not seem to be inside a synced folder, the panel says so: your other computer would not see your notes and projects. Moved into one (and chosen again in Settings), they travel too; or switch **Notes**, **Projects and money** and **Papers** on, and Sioul carries them, sealed.

## What travels from this device

Under **What travels from this device**, each part has its switch, says what it carries, and when it last sent and received a change:

| Part | What it carries |
|---|---|
| Settings and accounts | your settings and accounts (never their passwords), the ties between things, where the Porch was closed, mail you said is no payment |
| Senders | who may write to you (known, blocked, safe, neutral), what the shield read, others' public keys |
| Health | medicines, prescriptions and the doses taken |
| Time | time noted, the session running, the day's choices, where you stopped, working late or done for the day |
| Drafts and invoices | mail being written, invoices made |
| Projects and money | from your notes folder: projects and their mail routes, budgets, bank accounts and movements, contracts |
| Watch | your watch's days |
| Lists kept here | calendars and contacts kept on this device only |
| Notes | your notes folder: notes, their pictures, PDFs and memos, scanned letters |
| Papers | the papers wallet and its files |

Each device chooses for itself: switching a part off on the phone changes nothing on the desktop. A part switched off sends nothing and takes nothing out on your other devices; switched on again, it joins them as a new device would: what they hold comes first, and what only this one holds goes out. A device that never chose shares what Sioul shared before parts had switches: everything but notes and papers, and projects and money if you had ticked them. Before **Notes** or **Papers** is switched on, the panel says how many files would travel, and how big they are in all.

## Notes and papers in the vault

Switched on, notes and papers travel through the same sealed folder, one file at a time:

- **Only what changed**: each file is compressed, sealed apart and sent once; a note changed sends that note alone, not the folder. Files over 64 MB stay on their device, and the panel says which. On a phone whose sync app never deletes what was deleted elsewhere (Murena's eDrive), every sealed file it brought stays in the sharing folder there: count on room for your notes and papers twice over.
- **Nothing readable**: the folder's server sees sealed files of some size, never their names nor their content. Their size can still tell a well-known document (a public form) from others.
- **Left out**: what your sync app or your editor makes beside your files (hidden files, `~` and `.tmp` files, conflicted copies, Office's locks), names that are not text, and what the other parts carry.
- **Changed on two devices at once**: both versions are kept. The later one keeps the name; the other is kept beside it as "lease (conflict 2026-10-05 16.05).md", on every device, and the panel says so for a day. Keep the one you want, or merge them by hand. The notes page does the same when you save a note that changed on another device while it was open.
- **An older copy put back by hand** (a backup restored with its old dates) does not undo your other devices' changes: the newer version comes back, the older copy kept beside it, and the panel says so. To send an older version, use **Put back** below.
- **Taken out**: a note deleted on one device goes on the others ten minutes later (it may be on its way back), kept there among the earlier versions in case.
- **Many gone at once**: if many files leave a folder at once, or a folder is suddenly empty, gone or unreadable (a memory card not mounted, a program that emptied it, an access withdrawn), nothing of them is taken out on your other devices and nothing is written into it; the panel says so, with **Take them out everywhere** for when you did it on purpose. Switching Notes off and on fills the folder again from your other devices.
- **Names a phone takes for one**: two notes whose names differ only by case or by how an accent is written ("Lease.md" and "lease.md") are one file on a phone: the second waits, and the panel says so; rename one.
- **Room**: nothing is written without room for it and some left over; a note that does not fit waits, said, and comes when there is room.
- **Not with a synced notes folder**: if a sync app already carries your notes folder on this device (Nextcloud, Dropbox, Syncthing, or on a phone the same folder as the sharing one), Sioul refuses to carry it too, and says why: two carriers would undo each other's changes. Move the notes to a folder no sync carries to let Sioul carry them, or leave them to that sync. Sioul does not recognise every sync app (Google Drive, Insync, rclone, MEGA…): leave Notes off where one carries your notes.
- **On Android**, notes and papers wait while Sioul lacks Android's access to all your files (without it, Sioul would see only the files it made).

## Putting back an older version

Before a change from another device is written into a file here, or takes it out, Sioul keeps the file as it was, on this device only: the last 20 versions of each file, and all those of the last 30 days; of a file deleted since, those of the last 30 days; never more than a gigabyte in all, the oldest going first. They are never shared. A note or a paper whose version is still in the sharing folder is kept as a pointer to it, for two months, rather than copied.

In **Settings ▸ Your folder and sharing**, **Show earlier versions** lists them: each part, its files (the one changed last first; type part of a name to find older ones), and under a file its versions, with when and how big. **Put back** first says what it would change, then puts that version back:

- a note, a paper, a draft goes back whole; the file as it was just before is kept in the list too, so putting back can be undone the same way;
- a file of settings or lists (your settings, the doses taken, time noted) goes back entry by entry: each setting or mark the version held is put back as it was, those taken out since come back, and those added since stay. Putting back never takes out, on any device, what was added after that version: a dose marked this morning stays marked.

The next exchange sends what was put back to your other devices, as a change you made now.

## On a phone

Sioul for Android shares the same way, through the folder your phone's sync app keeps on the phone: Murena's eDrive, Syncthing, FolderSync, Autosync, Nextcloud's own app, or any app that keeps a folder on the phone in step with your cloud (Nextcloud, Dropbox, Google Drive, OneDrive…). Sioul reads the folder; it asks only that new files, and files that grow, reach the other side some day.

- **Where**: some sync apps carry only a few folders. Murena's eDrive carries your cloud's **Documents** (with Pictures, Music…), not the rest of it: share through a folder inside Documents, such as `Documents/Sioul`. Sioul suggests one there when your synced folder has a Documents folder, and says so when the one chosen is outside it.
- **On the phone**: Settings ▸ Your folder and sharing, **Allow access to files** (Android's switch), then **Choose…** the folder, your passphrase, **Share**.
- **Accounts** come without their passwords: each asks for its own once, typed or [from Bitwarden](accounts.md#an-account-from-your-other-device).
- **Your notes, projects and papers**: the phone keeps a notes folder of its own, which no sync carries. Switch **Notes**, **Projects and money** and **Papers** on in the same panel, on the phone and on each device whose notes folder no sync carries: they then travel sealed with the rest, a note at a time. On a computer whose notes folder Nextcloud already carries, they stay off: that computer's notes reach the phone only once they are in a folder no sync carries, with Notes switched on there too.
- **The sync app's pace**: a phone's sync app often brings the cloud's changes only every half hour. When the app offers a way to be asked (Murena's eDrive does), Sioul asks it to look now: after you mark something, when you come back to Sioul, and every five minutes while it is open, so what your other devices marked comes within a minute. Otherwise it comes at the app's pace, and the doses say what Sioul cannot know meanwhile.
- **Files kept online only**: if your sync app keeps files on the server until you open them (OneDrive, Google Drive, iCloud, Nextcloud's "virtual files"), set the sharing folder to stay **always on this device**.
- **What is never needed**: deletions (eDrive never deletes on one side what was deleted on the other: the old files Sioul clears stay, and are not read again), and nothing it keeps aside, such as conflicted copies, is read.

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

A note or a paper changed on two devices is not merged line by line: both versions are kept, the earlier one beside the other under a name that says so (see above).

## Some things, one computer at a time

- **Medicines** are reminded by the computer you are at only, so that a dose is not reminded twice. A dose marked taken goes to the others at once.
- **The sites' gathered notification** comes on the computer you are at.
- **Invoices** are numbered on one computer only, so that a number is never given twice. Another computer says where they are made, and offers **Make invoices on this computer**. See [Time and invoices](time.md#on-several-computers).

## Not there yet

A database server instead of a folder, for those who would rather have one; drafts in your mail server's Drafts folder, for other mail programs to see; phones other than Android's. The format is plain and documented, so that they can come.
