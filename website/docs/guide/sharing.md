---
description: Using Sioul on several computers and a phone. How it works (each device keeps its own data; a folder your sync app carries passes sealed changes between them), what it protects and what it cannot hide, what travels part by part, and putting back earlier versions.
---

# Sharing between your devices

## In short {#in-short}

Each of your devices keeps all of its data in its own files, and works without the others. What must travel between them goes through a folder that a sync app you already use carries (Nextcloud, Dropbox, Syncthing, Google Drive, OneDrive, or another), sealed on your device first: the folder and its server can read nothing, and change nothing without it showing. There is no server of ours, and nothing reaches the developer. Nothing that happens to that folder can take data away from your devices, and what a device cannot know, it says.

## How it works {#how-it-works}

- **Your devices hold your data; the folder only passes messages between them.** Each device keeps its own complete copy. The shared folder holds sealed changes, sent from one device to the others. A device that loses the folder, or finds it damaged, keeps everything it had.
- **One file per device, written by that device alone.** Each device writes its changes at the end of its own file in the folder, one sealed line per change, and never touches the files of the others. A sync app makes "conflicted copies" when two devices change the same file. Here that never happens, so it never has to choose for you.
- **Changes, not copies.** Each minute, Sioul compares your files with what they held at its last look, and sends only what changed, one entry at a time: a setting, a line of time, a dose, a sender, a note. Each device reads the new lines of the others and applies them.
- **The later change wins, entry by entry.** A setting changed on two devices keeps the later change. Time noted on each device stays on both. A note changed on two devices keeps both versions, one of them under a name that says so. Once all your devices run this version or a later one ([below](#devices-on-different-versions)), a money line, a time session or a pinned site changed on two devices stays one, and two lines alike made on one device stay two: each has an id of its own, given when it is made, or written into it once, when your devices take the newer form.
- **What the sync app has to do: very little.** It must carry new files, and files that grow, sooner or later and in any order. It does not need to delete, rename or lock anything. That is why any sync app works. A slow one, such as a phone's that looks every half hour, makes changes come late, never wrong.
- **What a device cannot know, it says.** Each device says in the folder how it is: when it started, when it closed properly, whether it is in use, and when it last shared. Each knows how far it has read the others. A device that closed properly sent everything it marked; one in use is known while its news keeps coming. If something may have happened on a device whose news has not come (a dose marked on the phone while in use, which its sync app has not carried yet), Sioul says it does not know rather than guessing, and names that device. The dose is reminded with "check first", never as "not taken" ([Health](health.md#on-several-computers)).

### When the sync app is late {#when-the-sync-app-is-late}

Sync apps carry files at their own pace, and Sioul cannot hurry most of them. A computer's Nextcloud client sends a change within seconds; a phone's app may wait for its next scan, half an hour or more, and can leave some files behind for longer: on 6 October 2026, Murena's eDrive left a computer's changes on the server for hours while it scanned every few minutes. Until a change arrives, the other device does not know it: a do-not-disturb switch, a dose marked taken, whether the computer is still open. A device that cannot know says so (doses: "check before taking it"); it never guesses.

WebDAV is the one protocol for which Sioul takes matters into its own hands. When the shared folder sits on a Nextcloud server Sioul already has an account for (Murena's included), it also reads the other devices' files from the server itself, as a backup, and keeps whichever copy is newer: a phone then follows within about a minute, whatever its sync app does. With Nextcloud, a sync app is even optional: Sioul can keep the folder in step itself, sending as well as fetching, so that a phone needs no other app ([setting it up](#setting-it-up)). With any other sync app (Syncthing, Dropbox, Google Drive…), changes arrive when that app brings them. That is why Nextcloud is the first choice: [where to get one](compatibility.md#where-to-get-a-nextcloud).

### Sent at once {#sent-at-once}

A few seconds after you save anything that travels (a dose answered, a setting, a medicine, a project, a budget), Sioul exchanges, without waiting for the minute. And where the folder is on your Nextcloud, Sioul then sends this device's own files there itself, beside your sync app: your other devices have them within seconds, even when this device's sync app is late, and a server that lost them can be given them again (**Send everything again**, [below](#setting-it-up)).

- **Only this device's files**, never another's; each goes over what the server holds as Sioul last saw it, so it never undoes a newer copy; this device's records are never sent shorter than they were.
- **Your sync app goes on as before.** When it sends the same file again, nothing changes: Sioul finds it there as it is here and sends nothing twice. Sioul gives each file its date and checksum, so that Nextcloud's client recognises the same file without downloading it.
- **Small files, on a slow connection too.** Each change sends a small file (tens of kilobytes at most), so a phone's hotspot or mobile data carries it: a computer writing each minute sends about half a megabyte an hour. A large file, now and then, is given the time its size asks; one that does not go through is said in the panel (which file, how long it tried), and tried again later, never sooner: a minute, then two, five, a quarter of an hour, up to an hour. On a metered connection, or one measured slow, large files go at most every ten minutes.
- **A text to send goes first.** A text written on a computer travels in a small file of its own, sent first and alone, so that it reaches your phone within its quarter of an hour even while a large file is still on its way.
- **One moment is left**: a sync app looking at the server in the second between a change here and Sioul's sending. Nextcloud's client then keeps your newer file beside, as a "conflicted copy" on that computer (never read; delete it when you like), and Sioul starts its file again with all it holds. Nothing is lost, and the other devices never take a dose as known wrongly: at most they say for a minute that they do not know.

### When something goes wrong in the folder {#when-something-goes-wrong-in-the-folder}

Nothing that happens in the folder can take your data away from you. Sioul treats every surprise as something it does not know:

- **A file cut, missing, damaged or out of order** (a sync app that put back an older copy, a disk's damage): what it holds is counted as not known, and said. It is never read as "taken out".
- **A settings file found empty or half written** on this device (a crash, a full disk) is never read as "everything deleted". It waits ten minutes, and nothing of it is taken out elsewhere meanwhile.
- **Many files gone at once**, or a folder suddenly empty or unreadable (a memory card not mounted, an access withdrawn): nothing of them is taken out on your other devices until you say so.
- **An older copy put back by hand** (a backup restored with its old dates) does not undo your other devices' changes.
- **A device restored from a backup**, or one that lost its memory, carries on from where the folder is and catches up: what it marked after the backup comes back.
- **Before another device's change replaces or removes a file here**, the file as it was is kept on this device, to put back if needed ([below](#putting-back-an-older-version)).

## What it protects, and what it cannot hide {#what-it-protects-and-what-it-cannot-hide}

- **Sealed on your device.** Every change is encrypted before it is written into the folder, with XChaCha20-Poly1305, a cipher that also detects any change made to what it sealed. Notes and papers are compressed, then sealed in pieces of 1 MiB.
- **One passphrase.** The key is made from a passphrase you choose, through Argon2id, which is deliberately slow and costly to try (64 MiB of memory and three passes for each guess). You type it once on each device, which keeps the key made from it in its keyring (on a phone, behind Android's KeyStore); the passphrase itself is kept nowhere and sent nowhere. A wrong one is said at once. It needs at least 12 characters: a few words you will not forget.
- **What the folder and its server can see**: which device wrote (an identifier drawn at random, not your name nor the device's), when, and how much. For notes and papers, how big each sealed file is: a well-known document, such as a public form, might be recognised by its size. They can also tell when a change reuses a content already sealed there (a file put back as it was).
- **What they never see**: what the changes are. Not the names of things (addresses, file names, settings), not their content.
- **Tampering shows.** Each line is bound to the device that wrote it, its place in that device's file, and its time. A line changed, moved into another device's file, or put in another order does not open. Each piece of a sealed file is bound to its file and its place, so pieces cannot be swapped, cut or added. Whatever does not open is set aside as damaged and said. It never erases anything.
- **Someone who gets the folder but not the passphrase** (a hacked cloud account, a curious provider) can read nothing and forge nothing. They can delete or damage files: Sioul notices, says so, and your devices lose nothing ([above](#when-something-goes-wrong-in-the-folder)).
- **Someone who has the passphrase**, or an unlocked device of yours, can read and send changes like any of your devices. Even then, a change from the folder is written only where it belongs: never through a link, never outside your notes folder, never into Sioul's own files, never in plain into the sharing folder.
- **Passwords never travel.** Accounts arrive on another device without their passwords: each device asks once, and keeps them in its own keyring, or takes them from Bitwarden.
- **Not hidden yet**: the sizes and times above, and a small note each device leaves in the folder, unsealed: when it last exchanged, and how far it has read the others.

!!! warning "A lost passphrase cannot be found again"
    Nobody can recover it for you: it is never sent anywhere. If it is lost, stop sharing on every device and start again with a new folder. Nothing on your devices is lost.

## What travels, and how {#what-travels-and-how}

| What | How it reaches your other devices |
|---|---|
| Mail, contacts, events and tasks, with what Sioul writes in your tasks (steps, waits, links, kinds) | by their own servers, as with any program |
| Your notes folder: notes, projects, budgets, papers, scanned letters, pictures and memos | by the folder's own sync; or, when no sync carries it (a phone's), through the sharing, sealed, once you switch them on |
| What Sioul keeps on this device alone: your settings and accounts (without passwords), who may reach you, the ties between things, time spent, drafts, invoices, medicines and doses, lists kept on this device only, where the Porch was closed | through the sharing, sealed |

**Never shared**: what each device chooses to share, where things are on each device (each keeps its own folders), how text reads on this screen, how pages are laid out, this device's browser notices, caches, and your own OpenPGP keys (copy them by hand). Passwords stay in each device's keyring.

## Setting it up {#setting-it-up}

No cloud yet? Nextcloud is the one Sioul is tested with: [where to get a Nextcloud](compatibility.md#where-to-get-a-nextcloud).

On the first device:

1. Open **Settings ▸ Your folder and sharing**, and find **Between your devices**.
2. **Folder**: choose a folder inside the one your sync carries, such as a new folder `Sioul` in your Nextcloud folder.
3. **Passphrase**, then **Once more**: a few words you will not forget, at least 12 characters.
4. **Share**.

On the other device, choose the same folder (as your sync shows it there). Sioul sees that another device shares through it, and asks for the passphrase chosen there. Then **Share**.

With a Nextcloud account in Sioul (Murena's included), a device needs no sync app at all: choose **Sioul keeps it in step with your Nextcloud itself**, the account, and the folder's place among your files there (`Documents/Sioul`), then the passphrase: typed twice when the folder is new there, once when another device shares through it already. Sioul keeps its own copy of the folder on that device and sends and fetches the files itself; devices that use a sync app on the same folder share with it as before ([why](#when-the-sync-app-is-late)).

The first time, a copy of what this device had is kept aside, in case. Then the other device's settings come, and what this one alone had goes out: two devices set up apart end with the first one's settings and both of their lists.

From then on, changes are exchanged a few seconds after anything that travels is saved here, each minute, and when you choose **Refresh everything** or **Exchange now**. The panel says through which folder you share, with how many other devices, and when they were last heard from. Under **Your other devices**, it lists each one: in use now, or closed at 22:14, and when it last shared, and the Sioul it runs ("Sioul 0.0.3 (eff8661abcde)."; an older one than this device's said, so that you can update it when you like); one you said is off, with **Count it again**; one silent for a week, with **Forget this device**. Under them, this device's own: "This device: Sioul 0.0.3 (eff8661abcde).", as `sioul --version` says it. **Stop sharing** asks first, saying what stops and what stays, then ends it on this device: each device keeps its own files, and the others go on sharing among themselves.

On a Nextcloud (Murena's included), the panel also says whether Sioul reads the folder on the server itself, beside your sync app, and when it last did: **Also fetch them from the server**, on once the folder is found there under the same seal ([why](#when-the-sync-app-is-late)). If Sioul does not find it, give its place there (such as `Documents/Sioul`) and choose **Look there**. Below it, **Also send this device's changes to cloud.example.org directly**, on with it: Sioul sends this device's own files there itself, right after each change ([why](#sent-at-once)), and says when it last did.

**Send everything again**, while you share: each of this device's files the server lacks, or holds otherwise, is sent again, whatever was sent before; then the other devices' news is fetched. For a sync app that was late, or a server that lost files. It says what it did, such as "Sent 14 files to cloud.example.org at 10:42.", or why nothing went.

Your notes folder travels by its own sync, not by Sioul, unless you switch it on below. If it does not seem to be inside a synced folder, the panel says so: your other device would not see your notes and projects. Moved into one (and chosen again in Settings), they travel too; or switch **Notes**, **Projects and money** and **Papers** on, and Sioul carries them, sealed.

## Devices on different versions of Sioul {#devices-on-different-versions}

Your devices may run different versions of Sioul for a while, such as a phone updated after the computer. They go on sharing; what they send each other changes form only once all of them can read the new one.

- **A device on an older Sioul holds the newer form back.** This version of Sioul shares money lines, time sessions, pinned sites and lists of words in a newer form: one line or one session changed on two devices stays one, and two sites pinned, or two words added, on two devices at the same moment both stay. Until every device of the sharing runs this version or a later one, all of them keep sharing in the form the older one reads, and the panel says so: "One of your devices runs an older Sioul…", the device's own line saying which. Meanwhile, a time session, your pinned sites or a list of words changed on two devices at the same moment keep only the later change, as before. Once that device is updated, your devices take the newer form by themselves, once: each writes into its own files an id for each money line, preset, split and time session that has none (the file as it was is kept among the earlier versions), and sends only what the older form had lost, such as a second line alike. A device retired without **Stop sharing** holds the newer form back until you choose **Forget this device** for it (offered once it has been silent for a week, or at once for a device known only by what it shared before), or for 180 days.
- **A device that needs a newer Sioul.** If your other devices share a part in a form newer than this device's Sioul reads, this device says so once in the status line ("This device needs a newer Sioul to share “Time”…") and, as long as it lasts, in **Settings ▸ Your folder and sharing**. It still reads what it can. What you change here in that part stays on this device, and what changes on the others waits here. Once you update Sioul on this device, both are applied, the later change winning: a change made here gives way to a later one made on the other device. If the device that shares the newer form stops sharing, you forget it, or it stays silent for 180 days, the part travels again between your other devices, and what waited here from that device is set aside. Should that part be Health, this device says meanwhile that it does not share its doses, after a restart too: your other devices then say they do not know a dose it may have answered, never that it was not taken.

## What travels from this device {#what-travels-from-this-device}

Under **What travels from this device**, each part has its switch, says what it carries, and when it last sent and received a change:

| Part | What it carries |
|---|---|
| Settings and accounts | your settings and accounts (never their passwords), the ties between things, where the Porch was closed, mail you said is no payment, do-not-disturb's switch |
| Senders | who may reach you (the lists: known, blocked, safe, neutral, restricted, with addresses, numbers and cards), who may reach you during do-not-disturb, what the shield read, others' public keys |
| Calls | the calls your phones screened, declined or let ring, a month of them, and those you marked Seen: so that a computer's Porch lists what your phone declined ([Calls](calls.md#on-your-computers)) |
| Messages from your phone | the messages your phone's notifications brought, for the apps you chose there (its texts first), a week of them, and those you marked Seen: so that a computer's Porch shows them; off until you turn it on, on the phone and on each computer that should show them ([What reaches you](notifications.md#messages-on-your-computers)) |
| Texts | your phone's texts, the whole history with its multimedia messages' media, read on the phone and kept as an archive that only grows; the texts written on a computer for the phone to send, and what became of each; sealed, and sealed again on each device; off until you turn it on, on the phone and on each computer that should read or write them ([Texts](texts.md)) |
| Spam filter | the table your own spam filter's training makes on a computer, so that every device judges mail alike, and what you said is spam or not on each device; never the mail it learned from, nor its words ([Settings](settings.md#your-own-spam-filter)) |
| Health | medicines, prescriptions and the doses taken |
| Time | time noted, the session running, the day's choices, where you stopped, working late or done for the day |
| Drafts and invoices | mail being written, invoices made |
| Projects and money | from your notes folder: projects and their mail routes, budgets, bank accounts and movements, contracts |
| Lists kept here | calendars and contacts kept on this device only |
| Notes | your notes folder: notes, their pictures, PDFs and memos, scanned letters |
| Papers | the papers wallet and its files |

Each device chooses for itself: switching a part off on the phone changes nothing on the desktop. A part switched off sends nothing and takes nothing out on your other devices; switched on again, it joins them as a new device would: what they hold comes first, and what only this one holds goes out. A device that never chose shares what Sioul shared before parts had switches: everything but notes and papers, and projects and money if you had ticked them; never the messages from your phone nor the texts, which wait for your choice on each device. Before **Notes** or **Papers** is switched on, the panel says how many files would travel, and how big they are in all.

## Notes and papers {#notes-and-papers}

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

## Putting back an older version {#putting-back-an-older-version}

Before a change from another device is written into a file here, or takes it out, Sioul keeps the file as it was, on this device only: the last 20 versions of each file, and all those of the last 30 days; of a file deleted since, those of the last 30 days; never more than a gigabyte in all, the oldest going first. They are never shared. A note or a paper whose version is still in the sharing folder is kept as a pointer to it, for two months, rather than copied.

In **Settings ▸ Your folder and sharing**, **Show earlier versions** lists them: each part, its files (the one changed last first; type part of a name to find older ones), and under a file its versions, with when and how big. **Put back** first says what it would change, then puts that version back:

- a note, a paper, a draft goes back whole; the file as it was just before is kept in the list too, so putting back can be undone the same way;
- a file of settings or lists (your settings, the doses taken, time noted) goes back entry by entry: each setting or mark the version held is put back as it was, those taken out since come back, and those added since stay. Putting back never takes out, on any device, what was added after that version: a dose marked this morning stays marked.

The next exchange sends what was put back to your other devices, as a change you made now.

## On a phone {#on-a-phone}

Sioul for Android shares the same way, through the folder your phone's sync app keeps on the phone: Murena's eDrive, Syncthing, FolderSync, Autosync, Nextcloud's own app, or any app that keeps a folder on the phone in step with your cloud (Nextcloud, Dropbox, Google Drive, OneDrive…). Sioul reads the folder; it asks only that new files, and files that grow, reach the other side some day.

- **No sync app at all**: with your Nextcloud account in Sioul (Murena's included), choose **Sioul keeps it in step with your Nextcloud itself** when you start sharing on the phone: Sioul then sends and fetches the files itself, and the phone needs neither eDrive nor another sync app for the sharing ([setting it up](#setting-it-up)).
- **Where**: some sync apps carry only a few folders. Murena's eDrive carries your cloud's **Documents** (with Pictures, Music…), not the rest of it: share through a folder inside Documents, such as `Documents/Sioul`. Sioul suggests one there when your synced folder has a Documents folder, and says so when the one chosen is outside it.
- **On the phone**: Settings ▸ Your folder and sharing, **Allow access to files** (Android's switch), then **Choose…** the folder, your passphrase, **Share**.
- **Accounts** come without their passwords: each asks for its own once, typed or [from Bitwarden](accounts.md#an-account-from-your-other-device).
- **Your notes, projects and papers**: the phone keeps a notes folder of its own, which no sync carries. Switch **Notes**, **Projects and money** and **Papers** on in the same panel, on the phone and on each device whose notes folder no sync carries: they then travel sealed with the rest, a note at a time. On a computer whose notes folder a sync app already carries, they stay off: that computer's notes reach the phone only from a folder no sync carries, with Notes switched on there too.
- **The sync app's pace**: a phone's sync app often brings the cloud's changes only every half hour. When the app offers a way to be asked (Murena's eDrive does), Sioul asks it to look now: after you mark something, when you come back to Sioul, and every five minutes while it is open, so what your other devices marked comes within a minute. Otherwise it comes at the app's pace, and the doses say what Sioul cannot know meanwhile. What the phone marks goes the other way at once where the folder is found on your Nextcloud: Sioul sends it there itself, then asks the sync app to look ([sent at once](#sent-at-once)).
- **With Sioul closed**: Sioul keeps your devices in step in the background, with one quiet notification. Every few minutes (every two while another device is in use, every fifteen while you sleep) it reads your other devices' news on your Nextcloud itself, where the folder is found there, and asks the sync app nothing, so that the phone sleeps between two looks; otherwise it asks the sync app to look. It also reads what the sync app brings as soon as it is written: do-not-disturb turned on at your computer follows within a few minutes ([Do not disturb on every device](notifications.md#do-not-disturb-on-every-device)). Turned off, changes come when you open Sioul.
- **Doses while Sioul is not on the screen**: each coming dose is given ahead to Android's alarm clock. At its time, Sioul first asks the sync app for news, then reminds you, or not if another device marked it taken, or with "check first" if it cannot know.
- **Files kept online only**: if your sync app keeps files on the server until you open them (OneDrive, Google Drive, iCloud, Nextcloud's "virtual files"), set the sharing folder to stay **always on this device**.
- **What is never needed**: deletions (eDrive never deletes on one side what was deleted on the other: the old files Sioul clears stay, and are not read again), and nothing it keeps aside, such as conflicted copies, is read.

## Some things, one device at a time {#some-things-one-device-at-a-time}

- **Medicines** are reminded by the device you are using only, so that a dose is not reminded twice. A dose marked taken goes to the others at once.
- **The sites' gathered notification** comes on the device you are using.
- **Invoices** are numbered on one device only, so that a number is never given twice. Another device says where they are made, and offers **Make invoices on this device**. See [Time and invoices](time.md#on-several-computers).

## Not there yet {#not-there-yet}

Sizes padded to steps, and the devices' notes sealed too, so that the folder tells even less; a database server instead of a folder, for those who would rather have one; drafts in your mail server's Drafts folder, for other mail programs to see; phones other than Android's. The format is plain and documented ([the design notes](../dev/database.md)), so that they can come.

## For technical readers {#for-technical-readers}

- **Records** are sealed with XChaCha20-Poly1305 (RustCrypto), a random 192-bit nonce each. The associated data binds each record to its device, its round, its place in the file and its clock: a record moved, reordered or copied into another device's file does not open.
- **The key**: Argon2id v1.3, 64 MiB, 3 passes, 1 lane, a 16-byte random salt, and a check value sealed under the key, all in the folder's `seal.toml`. A seal asking more than 1 GiB or 16 passes is refused, so that a tampered folder cannot make a device spend unbounded memory. Each device's keyring keeps the 256-bit key; the passphrase, at least 12 characters, is kept nowhere.
- **Notes and papers**: HKDF-SHA-256 derives a sealing key and a naming key from it. A file is gzip-compressed, then sealed in 1 MiB pieces, each bound to the file's name, its index and whether it is the last, so that pieces cannot be swapped, cut or added. Names are an HMAC-SHA-256 of the content's hash, so that equal files in two folders cannot be linked. A file must open whole and match its record before it replaces anything, and opening stops at 64 MiB, whatever the compressed size claims.
- **Order**: a hybrid logical clock, the later change winning entry by entry.
- **Writing** from the folder never goes through a link, outside the part's own folder, in plain into the sharing folder, or into Sioul's own state.
- **From a Nextcloud server**: WebDAV over HTTPS only, never redirected, with that account's password taken from the keyring for each request; a folder there is read only when its `seal.toml` is this folder's, byte for byte.
- **To a Nextcloud server**: this device's own files only, each a `PUT` with `If-Match` its ETag as last seen (`If-None-Match: *` where none was), `X-OC-Mtime` and `OC-Checksum: SHA1:…`; refused (`412`), the server's copy is looked at and taken as sent when it is the same (size and checksum, else its bytes). A records file is never sent shorter.
- **The build**: each device's version and commit, in its sealed entry and sealed in the first line of each batch of its records; the server never learns them.
- **Unsealed**: each device's small note (when it last exchanged, how far it read the others), sizes and times. Padding to fixed steps is not built.
- **Format**: [the design notes](../dev/database.md).
