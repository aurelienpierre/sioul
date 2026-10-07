---
description: Privacy and security in Sioul - what stays on your device, what leaves it and when, how every message and attachment is checked, where passwords go.
---

# Privacy and security

Sioul runs on your device. There is no Sioul server, no account with us, and nothing goes to the developer. Your mail, calendars and contacts travel only between your device and your own providers.

The formal text, for Google's sign-in among others: [Privacy policy](../privacy.md).

## What stays on your device {#what-stays-on-your-computer}

Everything Sioul keeps is in plain files, in your own folders, readable by other programs:

| What | Where, on Linux |
|---|---|
| Settings, the senders you let in, safe, neutral, restricted or blocked | `~/.config/sioul/` |
| Mail (one Maildir per address), calendars and contacts (one file per item), drafts, time spent, invoices, medicines, your watch's days | `~/.local/share/sioul/` |
| Where each fetch stopped, where the Porch was closed, the sites' news, the log of an AI agent's calls | `~/.local/state/sioul/` |
| Your own spam filter: what its training read of your mail, the language model it learned, its table; what you said is spam or not | `~/.local/share/sioul/spam/`, `~/.local/state/sioul/spam/` |
| Notes, projects, budgets, papers, scanned letters | your notes folder, wherever you chose it |

On Windows, Sioul's folders are in `%APPDATA%\Sioul`; on macOS, in `~/Library/Application Support/Sioul`.

**Passwords**, Google's access, the keys and tokens of the services you turn on, and passphrases go to your system's keyring (GNOME Keyring or KWallet on Linux, the Credential Manager on Windows, the Keychain on macOS). Never in a file.

## What leaves your device, and when {#what-leaves-your-computer-and-when}

| To | What | When |
|---|---|---|
| Your mail provider | your mail: fetched, filed as you ask, sent when you send | always: it is your mail |
| Your calendar and contacts server, or Google | your calendars, tasks and contacts, both ways | always |
| The sites you pin | what any browser sends them | when they are open |
| Your provider's own settings page, and Thunderbird's list of providers | your address's domain, to find the server | when you add an account |
| Your system's DNS resolver | lookups of senders' domains, to check them | when mail arrives |
| Open-Meteo | the coordinates of the place you chose, rounded to two decimals, or the name of a town you search | only if you choose a place for the weather; every half hour at most |
| OpenStreetMap's geocoder (Nominatim) | your contacts' postal addresses, once each, nothing else | only after you choose **Place them** |
| OpenStreetMap's map images, or those you chose | which part of the map is shown | while the map is open |
| Their domain's Web Key Directory, then keys.openpgp.org | the addresses you write to | only when you choose **Look for their keys** |
| Each site you pin | a request for its own icon | about once a week |
| Anthropic | each new message to an address you protect | only with **Let the AI read it first**, and your own key |
| The AI agent you connect, and its provider | what it reads | only if you connect one ([Using an AI agent](ai-agent.md)) |
| GitHub | your token; searches for your issues and pull requests | only if you turn GitHub on |
| Your Bitwarden server | your login, to open your vault | when you fill a login from it |
| ClamAV's update servers | a request for virus signatures | once a day, only when your system keeps none of its own |

Sioul has no analytics, no advertising and no crash reports.

## Your mail server

Fetching changes nothing on the server: no message is marked read by being fetched. Sioul writes there only when you act: opening a message marks it read, as any mail program does; archiving, deleting, junking and moving happen ten seconds after you asked, so that **Undo** can stop them. Removing an account removes its password, not its mail.

## Every message checked

When a message arrives, Sioul checks it itself, through your system's DNS:

- **SPF**, on the server that handed the message to your provider;
- **DKIM**, each signature against the key its domain publishes, checked at arrival, since keys change;
- **DMARC**, with the domain's own policy;
- **ARC**, for forwarded mail, and the sending server's **reverse DNS**.

The results are kept with the message, under a name only your copy of Sioul uses, so that no sender can write results that pass for Sioul's. A message is then **verified**, **not verified** (with the reason), or **forged**. Forged mail is set aside, with the reason; nothing is deleted. Your provider's spam verdicts count too, for strangers' mail only ([below](#your-own-spam-filter)).

**Borrowed names** are caught: a sender calling itself after a bank, a public service, or your own domain, from an address that is not theirs, is set aside. Look-alike letters (a lowercase l for a capital I, a 0 for an O, Cyrillic letters) do not hide the name.

**HTML mail** is shown with its paragraphs, lists, bold text and links only: no images, no styles, no scripts, nothing that loads from the network, so no tracking image learns that you opened a message. Every link shows its full address before you click it.

## Your own spam filter

Sioul's own spam filter learns on your computer, when you press **Train now**, from your own mail ([Settings](settings.md#your-own-spam-filter)):

- **What training reads**: from every folder of every address but the trash, drafts, sent mail and Gmail's All Mail, the headers and the start of each text, never attachments, without changing anything on the server. It keeps them on this computer (`~/.local/share/sioul/spam/corpus/`), so that junk stays known after your provider empties its junk folder.
- **What it learns**: a language model of your mail's words, kept beside it (`language.bin`). It holds the words of your mail in plain text: it never leaves this computer.
- **What travels**: its table, the result, to your other devices through your folder, sealed like everything there: numbers, the words as hashes, never in plain text; and what you said, below. A phone never trains: it reads that table.
- **What you said**: **Junk**, **Not junk**, **Spam**, **Not spam** and blocking a sender from a message add a line to this device's record (`~/.local/state/sioul/spam/labels/`), and each message the filter moves into a Junk folder another (`~/.local/state/sioul/spam/moved/`): when, where, which message by its number and its Message-ID, spam or not; never a word of it. Each device keeps its own; they travel sealed to your other devices, so that a message you said is not spam on one is never flagged again on another.
- **Outside material**, if you import some (`sioul spam import`): the subjects and texts of mail labelled elsewhere, kept on that computer only, apart from your mail (`~/.local/share/sioul/spam/external/`), yours alone, never shared; `sioul spam import --remove` takes it away.
- **Nothing to anyone else**: no server, no AI. Its numbers are totals only; the words that weighed in one message are for you, in your own terminal (`sioul spam why`), never offered to an AI agent.

To forget it all, delete `~/.local/share/sioul/spam/` on that computer: its table, shared, then leaves your other devices too.

## Attachments and the antivirus

An attachment is checked before it opens or is saved, by your system's own antivirus:

- **On Linux and macOS**: ClamAV, when it is installed ([Install](install.md#the-packages)).
- **On Windows**: Microsoft Defender, through the Antimalware Scan Interface.

When a threat is found, nothing opens, and the copy is deleted. A program, a script or an installer is never started from a mail, even checked: save it if you trust it. Without an antivirus, Sioul does not refuse: it says the file will not be checked, asks before opening it (**Open it unchecked**), and gives the command that installs one. The attachments of mail set aside are not opened at all.

## Sites

The sites you pin live in one browser profile of Sioul's own, apart from your usual browser. Their notifications are kept by Sioul. The microphone, the camera and sharing the screen are allowed per site, by you; your location is never given. Downloads go to your downloads folder.

A security key's PIN is asked in Sioul's own dialog. Logins come from your Bitwarden vault, opened by Sioul itself, read only: your master password and keys are never kept, and the logins stay in memory until Sioul closes. A login made for another domain says so, so that a look-alike site shows. See [Sites](sites.md).

## Between your devices

Each device keeps its own data. What must travel between your computers and your phone goes through a folder your own sync app carries (Nextcloud, Dropbox, Syncthing, Google Drive, OneDrive…), sealed on your device before it is written there (XChaCha20-Poly1305, with a key made from your passphrase by Argon2id). The folder's server sees which device wrote, when and how much, and the size of each sealed note or paper; never what. Nothing that happens in that folder, a file damaged or deleted, can take data away from your devices. How it works, what it protects and what it cannot hide: [Sharing between your devices](sharing.md).

## Encrypted mail

With OpenPGP, Sioul signs and encrypts your messages as you send them, decrypts and checks those you receive, and gives your public key to those you write to (Autocrypt). Your secret keys stay on the device they were made on. See [Mail](mail.md#signing-and-encrypting).

## A public address

An address you publish can be protected against harassment: its mail is read before you see it, and hostile messages are set aside without their words being shown. See [the Porch](porch.md#a-public-address-protected).

## This website

This website has no analytics, no advertising and no cookies, and loads nothing from other sites. It is hosted by GitHub Pages, which keeps its own server logs, under [GitHub's privacy statement](https://docs.github.com/en/site-policy/privacy-policies/github-general-privacy-statement).

## Reporting a problem

A security problem, or anything that worries you: [GitHub issues](https://github.com/aurelienpierre/sioul/issues).
