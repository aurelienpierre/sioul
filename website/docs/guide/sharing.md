---
description: Using Sioul on several computers and a phone. How it works (each device keeps its own data; a folder your sync app carries passes sealed changes between them), what it protects and what it cannot hide, what travels part by part, and putting back earlier versions.
---

# Sharing between your devices

Each device keeps all of its data in its own files, and works without the others. What must travel between them goes through a folder that a sync app you already use carries: Nextcloud, Dropbox, Syncthing, Google Drive, OneDrive, or another. Everything is sealed on your device before it is written there, so the folder and its server hold nothing they can read. There is no server of ours, and nothing reaches the developer.

## How it works

- **Your devices hold your data; the folder only passes messages between them.** Each device keeps its own complete copy. The shared folder holds sealed changes, sent from one device to the others. A device that loses the folder, or finds it damaged, keeps everything it had.
- **One file per device, written by that device alone.** Each device writes its changes at the end of its own file in the folder, one sealed line per change, and never touches the files of the others. A sync app makes "conflicted copies" when two devices change the same file. Here that never happens, so it never has to choose for you.
- **Changes, not copies.** Each minute, Sioul compares your files with what they held at its last look, and sends only what changed, one entry at a time: a setting, a line of time, a dose, a sender, a note. Each device reads the new lines of the others and applies them.
- **The later change wins, entry by entry.** A setting changed on two devices keeps the later change. Time noted on each device stays on both. A note changed on two devices keeps both versions, one of them under a name that says so.
- **What the sync app has to do: very little.** It must carry new files, and files that grow, sooner or later and in any order. It does not need to delete, rename or lock anything. That is why any sync app works. A slow one, such as a phone's that looks every half hour, makes changes come late, never wrong.
- **What a device cannot know, it says.** Each device knows how far it has read the others, and when it last heard from each one. If something may have happened on a device not heard from yet (a dose marked on the desktop that the phone's sync app has not brought), Sioul says it does not know rather than guessing. The dose is reminded with "check first", never as "not taken" ([Health](health.md)).

### When something goes wrong in the folder

Nothing that happens in the folder can take your data away from you. Sioul treats every surprise as something it does not know:

- **A file cut, missing, damaged or out of order** (a sync app that put back an older copy, a disk's damage): what it holds is counted as not known, and said. It is never read as "taken out".
- **A settings file found empty or half written** on this device (a crash, a full disk) is never read as "everything deleted". It waits ten minutes, and nothing of it is taken out elsewhere meanwhile.
- **Many files gone at once**, or a folder suddenly empty or unreadable (a memory card not mounted, an access withdrawn): nothing of them is taken out on your other devices until you say so.
- **An older copy put back by hand** (a backup restored with its old dates) does not undo your other devices' changes.
- **A device restored from a backup**, or one that lost its memory, carries on from where the folder is and catches up: what it marked after the backup comes back.
- **Before another device's change replaces or removes a file here**, the file as it was is kept on this device, to put back if needed ([below](#putting-back-an-older-version)).

## What it protects, and what it cannot hide

- **Sealed on your device.** Every change is encrypted before it is written into the folder, with XChaCha20-Poly1305, a cipher that also detects any change made to what it sealed. Notes and papers are compressed, then sealed in pieces of 1 MiB.
- **One passphrase.** The key is made from a passphrase you choose, through Argon2id, which is deliberately slow and costly to try (64 MiB of memory and three passes for each guess). You type it once on each device, which keeps it in its keyring (on a phone, behind Android's KeyStore). It is never sent anywhere. A wrong one is said at once. It needs at least 12 characters: a few words you will not forget.
- **What the folder and its server can see**: which device wrote (an identifier drawn at random, not your name nor the computer's), when, and how much. For notes and papers, how big each sealed file is: a well-known document, such as a public form, might be recognised by its size. They can also tell when a change reuses a content already sealed there (a file put back as it was).
- **What they never see**: what the changes are. Not the names of things (addresses, file names, settings), not their content.
- **Tampering shows.** Each line is bound to the device that wrote it, its place in that device's file, and its time. A line changed, moved into another device's file, or put in another order does not open. Each piece of a sealed file is bound to its file and its place, so pieces cannot be swapped, cut or added. Whatever does not open is set aside as damaged and said. It never erases anything.
- **Someone who gets the folder but not the passphrase** (a hacked cloud account, a curious provider) can read nothing and forge nothing. They can delete or damage files: Sioul notices, says so, and your devices lose nothing ([above](#when-something-goes-wrong-in-the-folder)).
- **Someone who has the passphrase**, or an unlocked device of yours, can read and send changes like any of your devices. Even then, a change from the folder is written only where it belongs: never through a link, never outside your notes folder, never into Sioul's own files, never in plain into the sharing folder.
- **Passwords never travel.** Accounts arrive on another device without their passwords: each device asks once, and keeps them in its own keyring, or takes them from Bitwarden.
- **Not hidden yet**: the sizes and times above, and a small note each device leaves in the folder, unsealed: when it last exchanged, and how far it has read the others.

!!! warning "A lost passphrase cannot be found again"
    Nobody can recover it for you: it is never sent anywhere. If it is lost, stop sharing on every device and start again with a new folder. Nothing on your devices is lost.

## What travels, and how

| What | How it reaches your other devices |
|---|---|
| Mail, contacts, events and tasks, with what Sioul writes in your tasks (steps, waits, links, kinds) | by their own servers, as with any program |
| Your notes folder: notes, projects, budgets, papers, scanned letters, pictures and memos | by the folder's own sync; or, when no sync carries it (a phone's), through the sharing, sealed, once you switch them on |
| What Sioul keeps on this device alone: your settings and accounts (without passwords), who may write to you, the ties between things, time spent, drafts, invoices, medicines and doses, your watch's days, lists kept on this device only, where the Porch was closed | through the sharing, sealed |

**Never shared**: what each device chooses to share, where things are on each device (each keeps its own folders), how text reads on this screen, how pages are laid out, this computer's browser notices, caches, and your own OpenPGP keys (copy them by hand). Passwords stay in each device's keyring.

## Setting it up

On the first computer:

1. Open **Settings ▸ Your folder and sharing**, and find **Between your computers**.
2. **Folder**: choose a folder inside the one your sync carries, such as a new folder `Sioul` in your Nextcloud folder.
3. **Passphrase**, then **Once more**: a few words you will not forget, at least 12 characters.
4. **Share**.

On the other device, choose the same folder (as your sync shows it there). Sioul sees that another device shares through it, and asks for the passphrase chosen there. Then **Share**.

The first time, a copy of what this device had is kept aside, in case. Then the other device's settings come, and what this one alone had goes out: two devices set up apart end with the first one's settings and both of their lists.

From then on, changes are exchanged each minute, and when you choose **Refresh everything** or **Exchange now**. The panel says through which folder you share, with how many other devices, and when they were last heard from. **Stop sharing** ends it; each device keeps its own files.

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

## Notes and papers

Switched on, notes and papers travel through the same sealed folder, one file at a time:

- **Only what changed**: each file is compressed, sealed apart and sent once; a note changed sends that note alone, not the folder. Files over 64 MB stay on their device, and the panel says which. On a phone whose sync app never deletes what was deleted elsewhere (Murena's eDrive), every sealed file it brought stays in the sharing folder there: count on room for your notes and papers twice over.
- **Left out**: what your sync app or your editor makes beside your files (hidden files, `~` and `.tmp` files, conflicted copies, Office's locks), names that are not text, and what the other parts carry.
- **Changed on two devices at once**: both versions are kept. The later one keeps the name; the other is kept beside it as "lease (conflict 2026-10-05 16.05).md", on every device, and the panel says so for a day. Keep the one you want, or merge them by hand. The notes page does the same when you save a note that changed on another device while it was open.
- **An older copy put back by hand** (a backup restored with its old dates) does not undo your other devices' changes: the newer version comes back, the older copy kept beside it, and the panel says so. To send an older version, use **Put back** below.
- **Taken out**: a note deleted on one device goes on the others ten minutes later (it may be on its way back), kept there among the earlier versions in case.
- **Many gone at once**: if many files leave a folder at once, or a folder is suddenly empty, gone or unreadable, nothing of them is taken out on your other devices and nothing is written into it; the panel says so, with **Take them out everywhere** for when you did it on purpose. Switching Notes off and on fills the folder again from your other devices.
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
- **Your notes, projects and papers**: the phone keeps a notes folder of its own, which no sync carries. Switch **Notes**, **Projects and money** and **Papers** on in the same panel, on the phone and on each device whose notes folder no sync carries: they then travel sealed with the rest, a note at a time. On a computer whose notes folder a sync app already carries, they stay off: that computer's notes reach the phone only from a folder no sync carries, with Notes switched on there too.
- **The sync app's pace**: a phone's sync app often brings the cloud's changes only every half hour. When the app offers a way to be asked (Murena's eDrive does), Sioul asks it to look now: after you mark something, when you come back to Sioul, and every five minutes while it is open, so what your other devices marked comes within a minute. Otherwise it comes at the app's pace, and the doses say what Sioul cannot know meanwhile.
- **Doses while Sioul is not on the screen**: each coming dose is given ahead to Android's alarm clock. At its time, Sioul first asks the sync app for news, then reminds you, or not if another device marked it taken, or with "check first" if it cannot know.
- **Files kept online only**: if your sync app keeps files on the server until you open them (OneDrive, Google Drive, iCloud, Nextcloud's "virtual files"), set the sharing folder to stay **always on this device**.
- **What is never needed**: deletions (eDrive never deletes on one side what was deleted on the other: the old files Sioul clears stay, and are not read again), and nothing it keeps aside, such as conflicted copies, is read.

## Some things, one device at a time

- **Medicines** are reminded by the device you are using only, so that a dose is not reminded twice. A dose marked taken goes to the others at once.
- **The sites' gathered notification** comes on the computer you are at.
- **Invoices** are numbered on one computer only, so that a number is never given twice. Another computer says where they are made, and offers **Make invoices on this computer**. See [Time and invoices](time.md#on-several-computers).

## Not there yet

Sizes padded to steps, and the devices' notes sealed too, so that the folder tells even less; a database server instead of a folder, for those who would rather have one; drafts in your mail server's Drafts folder, for other mail programs to see; phones other than Android's. The format is plain and documented ([the design notes](../dev/database.md)), so that they can come.
