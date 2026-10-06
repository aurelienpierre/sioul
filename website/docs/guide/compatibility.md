---
description: What Sioul works with, feature by feature - mail and calendar servers, Google, the apps you already use on the same accounts, Obsidian and Nextcloud Notes, sync apps, websites, Bitwarden, OpenPGP, watches, antivirus, AI agents and the systems it runs on; what was tested, what is expected, and the known limits.
---

# Works with

Sioul speaks open standards, so it works with most servers, and beside the programs you already use. This page says, feature by feature, what each needs, and how sure that is:

- **Tested**: tried with the server, program or device named.
- **Expected**: built to the standard they follow, not tried with them yet.
- **Limit**: what does not work, or works less, and why.
- **Not supported**: not built.

Nothing here goes beyond what is built. The standards behind each line, and where each test is recorded: [the design note](../dev/compatibility.md).

## In short

| Area | Works with | How sure |
|---|---|---|
| [Mail](#mail) | any IMAP and SMTP server that takes a password over an encrypted connection; Gmail and Google Workspace with an app password, or by signing in with Google with a key of your own | **Tested** with a test server, and a real mailbox in daily use; **expected** elsewhere. **Not supported**: Outlook.com, Hotmail, Microsoft 365 |
| [Calendars, tasks and contacts](#calendars-tasks-and-contacts) | any CalDAV and CardDAV server over HTTPS: Nextcloud (Murena's too), Radicale, Fastmail, Posteo, mailbox.org, iCloud, your host's | **Tested** with Radicale, and in daily use with Murena (Nextcloud) since 4 October 2026; **expected** with the others. **Limits**: iCloud's reminders; Baïkal must be set to Basic sign-in |
| [Google](#google) | calendars, contacts and Google Tasks, signed in on Google's page | **Expected**: tried only against a stand-in of Google Tasks |
| [Other apps on the same accounts](#other-apps-on-the-same-accounts) | Thunderbird and phone mail apps; Nextcloud Tasks, Tasks.org; DAVx⁵ with OpenTasks or jtx Board | **Expected**, from their code. **Limit**: a task tied to another by a wait, changed on a phone through DAVx⁵ |
| [Notes](#notes) | Obsidian, Nextcloud Notes, any Markdown editor | **Tested** on files written as each writes them |
| [Sharing between your devices](#sharing-between-your-devices) | Nextcloud first ([where to get one](#where-to-get-a-nextcloud)); any app that keeps a folder in step: Nextcloud, Murena's eDrive, Syncthing, Dropbox, Google Drive, OneDrive… | **Tested** with a simulator of how sync apps behave, and with eDrive on a phone; **expected** with the others |
| [Sites](#sites) | websites that work in Chrome or Chromium: secure mailboxes, chats, calls; security keys | **Expected**; seen working with Proton Mail. **Limits**: on a phone, sites open in your browser |
| [Logins and keys](#logins-and-keys) | Bitwarden (its cloud, your own server, Vaultwarden); OpenPGP with GnuPG and other mail programs | GnuPG **tested** both ways; Bitwarden **tested** with the owner's own vault, in daily use (6 October 2026) |
| [Your watch](#your-watch) | a Garmin watch's own files: from the watch, from Gadgetbridge, or from Garmin's export | **Tested** on files made by hand, not on a real watch |
| [Antivirus and scanned letters](#antivirus-and-scanned-letters) | ClamAV, Microsoft Defender, Tesseract and Poppler | **Tested** on Linux; Defender **expected** |
| [AI agents](#ai-agents) | Claude Code, Claude Desktop, other MCP clients that start a program | **Tested** on its own, not yet inside those clients |
| [GitHub](#github) | your issues and pull requests, read with a token | **Tested** against a stand-in of GitHub |
| [Systems](#systems) | Linux (AppImage, Flatpak), Windows 10 and 11, macOS 13 and later; Android, being tried | Linux in daily use; Windows and macOS built and tested by GitHub, not yet run by a person; Android tried on one phone |

## Mail

| Feature | What it needs | Tested with | Expected with | Limits |
|---|---|---|---|---|
| Finding your server | your address | Murena's, by hand | providers that publish their settings, and those in Thunderbird's list | a server found by guessing is said to be a guess, to check |
| Receiving and keeping mail | IMAP over an encrypted connection (port 993, or 143 with STARTTLS), your password | GreenMail, a test server; Murena, in daily use since 4 October 2026 | Fastmail, Posteo, mailbox.org, your host's, your own Dovecot; iCloud Mail and Yahoo with an app password | fetching changes nothing on the server; no unencrypted connection |
| Codes within seconds | IDLE, which most servers have | — | most servers | without it, Sioul looks every two minutes |
| Archive, delete, junk, move | MOVE, else UIDPLUS; folders marked by their purpose | GreenMail: archive, delete | most servers | without MOVE and UIDPLUS, the original stays marked as deleted until another program clears it; unmarked folders are found by their names, and a missing Trash, Junk or Archive is made |
| What other programs changed | — | — | any server | read, flagged, moved or deleted elsewhere: seen at each round, by asking every message's flags, which is slower on very big folders |
| Sending | SMTP on port 465 (encrypted) or 587 (STARTTLS), your password | GreenMail | your provider's sending server | a copy goes into Sent (Gmail files its own); no unencrypted connection |
| Gmail and Google Workspace | an app password, which needs Google's two-step verification; or "Sign in with Google" with a Google key of your own | its server reached by hand; its refusal of a usual password recognised, in tests; signing in with Google against stand-ins only | Gmail; Workspace domains, recognised by their mail servers | "All Mail" serves as the archive. Not tried against Google itself. Gmail's access is restricted by Google: not with Sioul's own key |
| Encrypted mail | OpenPGP, as PGP/MIME | GnuPG 2.4, both ways | Thunderbird, Proton, any program that reads PGP/MIME | see [Logins and keys](#logins-and-keys) |

**Not supported**:

- **Outlook.com, Hotmail and Live addresses**: since 16 September 2024, Microsoft takes only its own sign-in page from other mail programs, and Sioul has none for mail. **Microsoft 365** work and school addresses: the same.
- **Gmail with Sioul's own Google key**: Google restricts full mail access, and Sioul's key is not verified for it. Use an app password, or a Google key of your own ([Google](#google)).
- JMAP servers, POP3, and Exchange's own protocols.
- **Proton Mail through its Bridge**: not tried. Sioul trusts only the certificates your system trusts, so Bridge's own certificate would have to be added to your system first. Proton Mail works as a [site](#sites).

### The same mailbox in other programs

Thunderbird, your phone's mail app and the webmail see the same mailbox:

- Fetching marks nothing as read. Opening a message in Sioul marks it read on the server, as any mail program does.
- Archive, delete, junk and move reach the server ten seconds after you act, as real moves: other programs see them. **Junk** and **Not junk** also mark the message for the server's spam filter.
- What other programs do (read, flagged, moved, deleted) comes back to Sioul at each round.
- Sioul makes a Trash, Junk, Archive or Sent folder only when the server has none, and deletes only empty folders you made.
- Replying from Sioul does not mark the message as answered on the server.
- Drafts stay on your device: other programs do not see them.

## Calendars, tasks and contacts

| Feature | What it needs | Tested with | Expected with | Limits |
|---|---|---|---|---|
| Finding your account | your address; else the server's address, given once | Murena's, by hand; Radicale | servers that publish the standard's addresses; Fastmail, iCloud, Posteo, mailbox.org and cPanel hosts are known | a redirection to another domain is not followed: give the server's address |
| Events and contacts, both ways | CalDAV and CardDAV over HTTPS, your password | Radicale, a test server; Murena (Nextcloud), in daily use since 4 October 2026 | Nextcloud elsewhere, Fastmail, Posteo, mailbox.org, SOGo, your host's; iCloud with an app-specific password | no unencrypted server. **Baïkal**: set its "WebDAV authentication type" to Basic: its default, Digest, is not supported |
| Only what changed | sync tokens, else each item's tag | Radicale; a stand-in in the tests | most servers | without sync tokens, every item's tag is compared at each round, which is slower |
| Tasks | task lists | Murena (Nextcloud), in daily use since 4 October 2026 | servers whose calendars take tasks, Nextcloud's among them | **iCloud**: reminders upgraded since iOS 13 are not reachable by any CalDAV program. **Google**: through Google Tasks ([below](#google)) |
| Steps, waits, links, kinds, costs | a server that keeps what it is given, lines it does not know included | Murena (Nextcloud), in daily use since 4 October 2026 | servers that store tasks as they are sent, Nextcloud's among them | other apps may drop some of them when they save a task ([below](#other-apps-on-the-same-accounts)) |
| A time given to a step for one day | a line of Sioul's own in the task | — | — | other apps do not show it. It is not the task's start: servers that check (Nextcloud's) refuse a start with a time when the date asked has none. **Coming**: an event in a calendar, tied to the task, which every calendar app shows |
| New lists, calendars and address books; renaming them | the standard's requests to make and rename them | — | Nextcloud and most servers | not at Google |
| Contact categories | the card's own categories | cards written as Nextcloud writes them | Nextcloud Contacts (its groups); DAVx⁵ set to keep groups as categories | groups kept as cards of their own (Apple's way, and DAVx⁵'s other setting) are not read as categories: such a group shows as a card |
| Invitations | received by mail; your answer goes back by mail | Radicale, an invitation answered | the organiser's own mail program, whatever it is | inviting people from Sioul: not supported. The copy in your calendar does not record your answer, so other apps may show it as not answered |
| Changed in two places at once | each item's tag | a stand-in in the tests | any server | the server's version wins; yours is kept aside, and the status line says where |

## Google

| Feature | What it needs | Tested with | Expected with | Limits |
|---|---|---|---|---|
| Signing in | Google's page, in your browser | its parts, in tests | Google | not yet tried with Google itself. With a Google key of your own left "in testing", Google ends each sign-in after seven days ([the design note](../dev/google.md#your-own-google-key)) |
| Calendars | Google's CalDAV | — | Google | no calendar made, renamed or deleted from Sioul; no tasks in a Google calendar; Google adds your default reminders to every event, and shifts events written without a time zone |
| Contacts | Google's CardDAV | — | Google | Google keeps the older vCard: labels, birthdays without a year and newer fields are lost, said before a contact moves there; no new address book |
| Tasks | Google Tasks | a stand-in of Google Tasks | Google Tasks | Google keeps a title, notes, done or not, a day, and one level of steps; the rest is greyed in the task's form, with why |
| Gmail | an app password, or "Sign in with Google" with your own key | see [Mail](#mail) | | |

## Other apps on the same accounts

Your tasks, events and contacts are standard: every app on the same server shows them. What an app keeps of Sioul's own lines when it saves a task is up to that app. This was read in their source code on 6 October 2026, not tried with Sioul yet:

| App | Steps | Waits | Links, kinds, costs, ratings | How sure |
|---|---|---|---|---|
| Nextcloud Tasks, on the web | shown as subtasks | kept | kept | **expected** |
| Tasks.org, with its own CalDAV sync | shown as subtasks | kept | kept | **expected** |
| Tasks.org or OpenTasks, through DAVx⁵ | shown as subtasks | shown as steps, and written back as steps when the phone changes the task | kept | **limit** |
| jtx Board, through DAVx⁵ | shown as subtasks | dropped when the phone changes the task | kept | **limit** |
| Android's calendar, through DAVx⁵ (events) | — | — | kept | **expected** |
| Thunderbird, Apple's Calendar and Reminders, Evolution, KOrganizer | not checked | not checked | not checked | an app that rewrites a task from only what it understands drops the rest |
| Google Tasks | one level | not kept | not kept | **limit**: greyed in Sioul, with why |

!!! warning "Waits on a phone, through DAVx⁵"
    With OpenTasks, Tasks.org or jtx Board through DAVx⁵, changing on the phone a task tied to another by a wait (either of the two), even ticking it done, turns the wait into a step, or drops it. Until that changes, change those tasks in Sioul, in Nextcloud Tasks, or in Tasks.org with its own sync. Only reading tasks on the phone is safe: nothing is written back.

## Notes

Fully compatible with an Obsidian vault and with Nextcloud Notes, side by side: [Notes](notes.md#the-same-folder-as-obsidian-and-nextcloud-notes).

| Feature | What it needs | Tested with | Expected with | Limits |
|---|---|---|---|---|
| An Obsidian vault | the vault chosen as Sioul's notes folder | files written as Obsidian writes them (wikilinks, embeds, tags, front matter, aliases, its trash) | Obsidian, on the computer and on the phone, through your sync | Sioul never reads Obsidian's own settings (`.obsidian`) |
| Nextcloud Notes | the Notes folder of your Nextcloud, synced to the device | files written as Nextcloud Notes writes them (`.txt` or `.md`, categories as folders) | Nextcloud Notes on the web and in its phone apps | Sioul reads the synced folder, not Nextcloud's Notes app: the folder must be on the device |
| Any other editor | plain Markdown files | — | any | a note changed elsewhere while open in Sioul is kept beside it, "(conflict …)", never written over |

## Sharing between your devices

Sharing needs only a folder that a sync app keeps in step between your devices: new files, and files that grow, must reach the others some day. Nothing has to be renamed, deleted or locked. How it works: [Sharing between your devices](sharing.md).

| Feature | What it needs | Tested with | Expected with | Limits |
|---|---|---|---|---|
| The sharing folder, on a computer | a folder your sync app keeps in step | a simulator of how sync apps carry files: every change both ways (as Nextcloud's client, Dropbox, Google Drive and OneDrive do), late and out of order, older copies put back, files kept online only | Nextcloud's client (Murena's too), ownCloud, Syncthing, Dropbox, Google Drive, OneDrive, iCloud Drive, pCloud, Seafile | files kept online only: set the sharing folder to stay "always on this device" |
| The sharing folder, on a phone | Android's access to all files; a folder the sync app keeps on the phone | Murena's eDrive 1.9.2, on /e/OS: the folder came at its next full scan | Nextcloud's app, Syncthing, FolderSync, Autosync | eDrive carries only Documents, Pictures, Music…: share through `Documents/Sioul`. It never deletes what was deleted elsewhere: count on room twice over. Only eDrive is asked to look at once; other apps bring changes at their own pace, often each half hour |
| Notes and papers, carried by Sioul | a notes folder that no sync app carries | the simulator | — | refused where a sync app already carries the notes folder. Recognised: Nextcloud, Dropbox, Syncthing, and folders named ownCloud, Sync, OneDrive, pCloudDrive or Seafile. **Not recognised**: Google Drive, Insync, rclone, MEGA: leave Notes off where they carry your notes |
| Changed on two devices | — | the simulator | — | both versions are kept, one named "(conflict …)"; the sync apps' own conflicted copies are never read |

### Where to get a Nextcloud {#where-to-get-a-nextcloud}

Nextcloud is the sync backend Sioul is tested with, and the only one Sioul reaches itself, over WebDAV: as a backup when a phone's sync app is late, or with no sync app at all, Sioul keeping the folder in step itself ([why](sharing.md#when-the-sync-app-is-late)). Any Nextcloud account does; the same account can also hold your calendars, tasks and contacts. Sioul's own files are small: a few megabytes, more if you share your notes and papers.

| Where | Free | Paid | Notes |
|---|---|---|---|
| [Murena](https://murena.io/signup), France | 1 GB | 20 GB to 2 TB | the account Sioul is tested with; mail, calendars, contacts and files in one |
| [Zaclys](https://www.zaclys.com/), France | a free plan | 12 € a year | a small French host of free software |
| [CHATONS](https://entraide.chatons.org/), France | depends on the host | depends on the host | a collective of ethical hosts: search for "cloud" or "Nextcloud" |
| [Hetzner Storage Share](https://www.hetzner.com/storage/storage-share/), Germany | — | from 1 TB | Nextcloud kept up by Hetzner, in its data centre in Falkenstein |
| [Nextcloud's own list](https://nextcloud.com/sign-up/) | 2 to 5 GB | more space, a server of your own | hosts in several countries; the paid offers fund the free accounts |
| [Your own server](https://nextcloud.com/install/) | — | — | Nextcloud is free software; a small server or a Raspberry Pi is enough for one person |

Checked on 6 October 2026. Offers change: read each host's terms, and where your files are kept, before you choose.

## Sites

Sites run in Qt WebEngine, the engine of Chromium, in a profile of their own, apart from your usual browser.

| Feature | What it needs | Tested with | Expected with | Limits |
|---|---|---|---|---|
| Websites | — | Proton Mail's web app, in use | sites that work in Chrome or Chromium | signing in with Google inside a site may be refused: Google blocks sign-ins in embedded browsers |
| Staying logged in | the site's cookies and storage, kept; each page closed as a browser closes its tabs when Sioul quits | a test page that keeps its login as Discord does | most sites | Sioul killed (a crash, the power cut) loses what its pages held |
| Notifications | the site's own | — | any site that notifies | kept for your hours ([Sites](sites.md#notifications-at-your-pace)) |
| Calls | the microphone, the camera and the speaker, allowed per site; sharing a screen, a window or nothing, chosen in Sioul's dialog | sharing the screen: a test page, on a screen nobody sees | the calls of chats and video sites that work in Chrome | full screen is not available |
| Security keys (FIDO2, WebAuthn) | a USB key such as a YubiKey; on Linux, Qt WebEngine built with udev (Fedora's is) | — | GitHub, Google, Proton, Bitwarden | passkeys kept in a phone or in the system: not on Linux and macOS. On Windows, Windows' own dialog asks. A plain touch shows nothing in Sioul |
| Downloads and PDFs | — | — | — | downloads go to your downloads folder; a PDF opens in the site's view |
| Logins from Bitwarden | see [Logins and keys](#logins-and-keys) | | | |
| On a phone | — | — | — | sites open in your browser: no login kept by Sioul, no notifications gathered |

## Logins and keys

| Feature | What it needs | Tested with | Expected with | Limits |
|---|---|---|---|---|
| Logins from Bitwarden | your Bitwarden account: bitwarden.com, bitwarden.eu, your own server or Vaultwarden, over HTTPS | the owner's own vault on Bitwarden's cloud, in daily use (6 October 2026); its decryption checked on Bitwarden's own test values | your own server or Vaultwarden | read only: nothing is written to your vault. Duo as a second step: not supported. On a phone: no security key |
| Your accounts' passwords | your system's keyring | Linux, in daily use | Windows' Credential Manager, macOS' Keychain, Android's KeyStore | passwords never travel between your devices |
| Encrypted mail (OpenPGP) | your key, made in Sioul or imported from GnuPG | GnuPG 2.4, both ways: signed, encrypted, tampered | Thunderbird, Proton, any program that reads PGP/MIME; the Web Key Directory of their domain, keys.openpgp.org | Sioul keeps its own keys and never reads or writes GnuPG's. Keys kept on a smartcard or a security key: not supported. Your secret keys stay on their device |

## Your watch

| Feature | What it needs | Tested with | Expected with | Limits |
|---|---|---|---|---|
| A Garmin watch | its own files (FIT): from its `GARMIN` folder when your desktop shows it, from Gadgetbridge's exports, or from Garmin's export | files made by hand | Garmin watches; Gadgetbridge with a Garmin watch | never through a Garmin account. Recent watches on KDE need kio-fuse; without it, copy the watch's folders by hand ([Health](health.md#your-watch)) |
| Other watches | — | — | — | **not supported**: only Garmin's files are read; not Gadgetbridge's own database, Apple Health, Health Connect or Fitbit |

## Antivirus and scanned letters

| Feature | What it needs | Tested with | Expected with | Limits |
|---|---|---|---|---|
| Attachments checked, on Linux and macOS | ClamAV, from your system's packages or Homebrew | Linux without ClamAV: Sioul asks first, and gives the command that installs it | ClamAV, its daemon or its scanner | the Flatpak does not reach your system's ClamAV: there, Sioul asks first. No antivirus on a phone |
| Attachments checked, on Windows | Microsoft Defender, or another antivirus that answers Windows' scan interface (AMSI) | — | Windows' antivirus | built, never run on Windows yet |
| Scanned letters read | Tesseract and Poppler | Tesseract, by hand, on a French letter drawn as a picture | Linux, Windows and macOS with both installed | the Flatpak carries neither: whether it reads scans is not checked. Not on a phone |

## AI agents

| Feature | What it needs | Tested with | Expected with | Limits |
|---|---|---|---|---|
| An agent on this computer | an MCP client that starts `sioul mcp` | the server on its own, in its tests and by hand | Claude Code, Claude Desktop, other MCP clients that start a program; local models through such a client | not yet tried inside Claude Code or Claude Desktop |
| An agent on the Internet | — | — | — | **not supported**: ChatGPT reaches servers over the Internet only |
| The AI reading of a protected address | your Anthropic key | a stand-in of Anthropic's service | Anthropic | only for an address you protect, when you allow it ([the Porch](porch.md#a-public-address-protected)) |

How to connect one, and what it may see: [Using an AI agent](ai-agent.md).

## GitHub

| Feature | What it needs | Tested with | Expected with | Limits |
|---|---|---|---|---|
| Your issues and pull requests as tasks | a fine-grained token, read only | a stand-in of GitHub | github.com | GitHub refused the searches of fine-grained tokens until Sioul's fix of 6 October 2026, not tried with GitHub since. Nothing is written to GitHub. GitHub Enterprise Server, GitLab, Codeberg: not supported |

## Systems

| | Linux | Windows | macOS | Android |
|---|---|---|---|---|
| Package | AppImage, Flatpak, or from the sources | an installer, Windows 10 and 11 (64-bit) | a disk image, macOS 13 and later | an APK, installed by hand (Android 9 and later, 64-bit) |
| How sure | in daily use, on Fedora | built and tested by GitHub at each change, not yet run by a person | the same | tried on one phone (Android 12) |
| Passwords kept in | your keyring (GNOME Keyring, KWallet) | the Credential Manager | the Keychain | Android's KeyStore |
| Reminders with the window closed | yes | not yet | yes | doses and the wake-up alarm |
| Notifications | yes, with buttons; the time running | yes | yes | doses, the time running, the alarm |
| Attachments checked | ClamAV, when installed (not in the Flatpak) | Microsoft Defender | ClamAV, from Homebrew | no |
| Scanned letters read | Tesseract and Poppler, when installed | the same | the same, from Homebrew | no |
| Sites | inside Sioul | inside Sioul | inside Sioul | in your browser |
| Security keys in sites | USB keys, their PIN asked by Sioul | Windows' own dialog | USB keys not checked; no Touch ID | your browser's |

On Ubuntu 24.04 and later, the AppImage runs sites without Chromium's sandbox: prefer the Flatpak there ([Install](install.md#download)). **Not supported**: iPhone and iPad.

## Not checked yet

Worth trying, and welcome in [GitHub issues](https://github.com/aurelienpierre/sioul/issues) once tried:

- A full sync of tasks with steps, waits and links on Fastmail, iCloud, or Baïkal set to Basic (Murena's Nextcloud is in daily use); contact categories and lists made or renamed, on any server.
- Waits changed on a phone through DAVx⁵, to confirm what its code says; what Thunderbird and Apple's apps keep.
- An invitation accepted on a server that sends invitations itself (Nextcloud, Google, iCloud).
- Google itself, and GitHub itself since the fix of 6 October 2026.
- A security key on GitHub, Google and Proton in Sites; a call with the microphone and the camera; a PDF a site shows.
- A site that keeps its login inside its page, such as Discord, staying logged in after Sioul quits.
- Bitwarden on your own server or Vaultwarden.
- Windows and macOS, run by a person.
- Scanned letters in the Flatpak; Proton Mail Bridge; a real Garmin watch; Claude Code and Claude Desktop.
