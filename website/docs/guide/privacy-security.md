---
description: Privacy and security in Sioul - what is protected by default, without setting anything up; how every message and attachment is checked; how what travels between your devices is sealed; security keys and Bitwarden; what leaves your device and when; what Sioul cannot protect; and how it compares with other apps.
---

# Privacy and security

## In short {#in-short}

Admin mail is where scams aim, and you should not have to be on guard every time you open it. So Sioul protects you by default, without asking you to set anything up. It runs on your devices, keeps your data in plain files in your own folders, and has no server of its own: nothing reaches the developer. Every message is checked before you see it, nothing loads from the network while you read, and an attachment goes to your antivirus before it opens. What travels between your devices is sealed with a key only you hold, passwords stay in your system's keyring, and a security key can guard your encrypted mail, the sites you keep and your Bitwarden vault.

The formal text, for Google's sign-in among others: [Privacy policy](../privacy.md).

## What is protected by default {#what-is-protected}

### Your data stays yours {#what-stays-on-your-computer}

Sioul runs on your device. There is no Sioul server and no account with us; Sioul sends no statistics, no crash reports and no update checks, and never contacts the developer. Your mail, calendars and contacts travel only between your device and your own providers.

Everything Sioul keeps is in plain files, in your own folders, readable by other programs, and yours to copy, back up or delete:

| What | Where, on Linux |
|---|---|
| Settings, the senders you let in, safe, neutral, restricted or blocked | `~/.config/sioul/` |
| Mail (one Maildir per address), calendars and contacts (one file per item), drafts, time spent, invoices, medicines, your phone's texts and their media (all of them, sealed) and those written for it to send | `~/.local/share/sioul/` |
| Where each fetch stopped, where the Porch was closed, the doses you marked and each dose's record, the sites' news, the log of an AI agent's calls, what the AI said of a protected address's mail, the calls your phones screened (a month of them), the messages your phone's notifications brought for the apps you send to your computers (a week of them) | `~/.local/state/sioul/` |
| Your own spam filter: what its training read of your mail, the language model it learned, its table; what you said is spam or not | `~/.local/share/sioul/spam/`, `~/.local/state/sioul/spam/` |
| Notes, projects, budgets, papers, scanned letters | your notes folder, wherever you chose it |

On Windows, Sioul's folders are in `%APPDATA%\Sioul`; on macOS, in `~/Library/Application Support/Sioul`. On Linux and macOS, Sioul makes its own folders yours alone each time it starts: other accounts on the same computer cannot open them. Your notes folder keeps the permissions you gave it.

**Passwords**, Google's access, the keys and tokens of the services you turn on, and passphrases go to your system's keyring: GNOME Keyring or KWallet on Linux, the Credential Manager on Windows, the Keychain on macOS; on a phone, encrypted with a key that Android's KeyStore keeps. Never in a plain file.

Sioul does not encrypt the files on your device itself: your system does that best, for every program at once. If its disk encryption is not on yet, turn it on: BitLocker or device encryption on Windows, FileVault on macOS, the encryption your Linux installer offers. Android phones encrypt their storage already.

### Every connection encrypted {#every-connection-encrypted}

Your mail, calendars, contacts, your Bitwarden vault and the lookups of encryption keys always travel encrypted, and each server's certificate is checked against your system's. There is no setting to send them in clear. Google's sign-in happens on Google's own page, in your usual browser, with whatever Google asks there (your password, your security key, a passkey): Sioul never sees your Google password, and receives Google's answer on your own device only.

### Every message checked {#every-message-checked}

When a message arrives, Sioul checks it itself, through your system's DNS:

- **SPF**, on the server that handed the message to your provider;
- **DKIM**, each signature against the key its domain publishes, checked at arrival, since keys change;
- **DMARC**, with the domain's own policy;
- **ARC**, for forwarded mail, and the sending server's **reverse DNS**.

The results are kept with the message, under a name only your copy of Sioul uses, so that no sender can write results that pass for Sioul's. A message is then **verified**, **not verified** (with the reason), or **forged**. A signature vouches for the sender when it comes from the sender's own organisation: `mail.example.org` signs for `shop.example.org`, while two names under a suffix shared by many owners, such as `github.io`, stay strangers to each other (the Public Suffix List tells which). Forged mail is set aside, with the reason; nothing is deleted. Your provider's spam verdicts count too, for strangers' mail only ([below](#your-own-spam-filter)).

**A copied address is not enough.** A message that nothing authenticates (it fails SPF and DKIM, and nothing vouches for it) is treated as a stranger's, whatever address it shows. Someone who writes from your bank's address, or a friend's, does not get their place on the Porch, their hours, or their protection from the spam filter.

**Borrowed names** are caught: a sender calling itself after a bank, a public service, or your own domain, from an address that is not theirs, is set aside. A brand whose name is also an everyday word or a place (Apple, Amazon, Orange, La Poste…) counts only when it stands alone or beside the words of a service, such as "Apple Support", "Amazon.com" or "Service client Orange", so that "Orange County Library" or "Café de la Poste" borrow nothing; any other brand counts wherever it is in the name. Look-alike letters (a lowercase l for a capital I, a 0 for an O, Cyrillic letters) do not hide the name. Look-alike *domains*, such as a bank's name with one letter changed, are not caught yet.

**The codes you ask for** come at once, whatever the time. If their sender is not verified, Sioul says to use them only if you just asked that site for one, since fake codes are a phishing trick; a forged one is set aside.

### Reading safely {#reading-safely}

**HTML mail** is shown with its paragraphs, lists, bold text and links only: no images, no styles, no scripts, no forms. Nothing loads from the network, ever, and no setting makes it: no tracking image learns that you opened a message. Sioul never sends a read receipt either.

**Links** show their full address before they open: under the pointer, or after a first tap on a touch screen. Only web and mail links open. A mail link starts a message to its address alone, without the subject or text it would fill in for you.

### Attachments and the antivirus {#attachments-and-the-antivirus}

An attachment is checked before it opens or is saved, by your system's own antivirus:

- **On Linux and macOS**: ClamAV, when it is installed ([Install](install.md#the-packages)).
- **On Windows**: Microsoft Defender, through the Antimalware Scan Interface.

When a threat is found, nothing opens, and the copy is deleted. A program, a script, a shortcut or an installer is never started from a mail, even checked: save it if you trust it. On Windows and macOS, a file you open or save from a mail carries your system's mark that it came from the Internet, so that Office opens it in Protected View and macOS checks it before it runs. The attachments of mail set aside are not opened at all.

Without an antivirus, Sioul does not refuse: it says the file will not be checked, asks before opening it (**Open it unchecked**), and says how to get one. A file larger than the antivirus scans (ClamAV stops at its limits on a file's size and on what an archive holds) is never called clean: Sioul says it was not scanned because it is too big, and asks the same way. A phone has no antivirus for Sioul to ask: attachments are not checked there, and Sioul says so rather than asking each time. The app you choose reads that one file and nothing else of Sioul's, and Android's installers never open.

### Your own spam filter {#your-own-spam-filter}

Sioul's own spam filter learns on your computer, when you press **Train now** and again by itself once a week when that computer is plugged in and idle, from your own mail ([Mail, Your own spam filter](mail.md#your-own-spam-filter)):

- **What training reads**: every folder of every address but the trash, drafts and sent mail (from Gmail's All Mail, only the archived mail that no other folder holds, never your own), without changing anything on the server. Of each message it keeps the headers, the names and types of its parts and the start of its text, never the attachments themselves; to check its sender as Sioul checks the mail it stores, it reads each message of up to 2 MB whole, in memory only, keeps only the results, and asks your system's DNS about the sender's domain. What it keeps stays on this computer (`~/.local/share/sioul/spam/corpus/`), so that junk stays known after your provider empties its junk folder.
- **What it learns**: a language model of your mail's words, kept beside it (`language.bin`). It holds the words of your mail in plain text: it never leaves this computer.
- **What travels**: its table, the result, to your other devices through your folder, sealed like everything there: numbers, the words as hashes, never in plain text; and what you said, below. A phone never trains: it reads that table.
- **What you said**: **Junk**, **Not junk**, **Spam**, **Not spam** and blocking a sender from a message add a line to this device's record (`~/.local/state/sioul/spam/labels/`), each message the filter moves into a Junk folder another (`~/.local/state/sioul/spam/moved/`), and each it flags where it is another (`~/.local/state/sioul/spam/flagged/`): when, where, which message by its number and its Message-ID, spam or not; never a word of it. Each device keeps its own; they travel sealed to your other devices, so that a message you said is not spam on one is never flagged again on another.
- **Outside material**, if you import some (`sioul spam import`): the subjects and texts of mail labelled elsewhere, kept on that computer only, apart from your mail (`~/.local/share/sioul/spam/external/`), yours alone, never shared; `sioul spam import --remove` takes it away.
- **What it asks your computer**, to train by itself: whether it is on mains power, saving power, idle or locked, and whether your connection is metered, from the system's own services; asked on this computer, kept nowhere, sent nowhere.
- **Nothing to anyone else**: no server, no AI of Sioul's. Its numbers are totals only; the words that weighed in one message are for you, in your own terminal (`sioul spam why`), never offered to an AI agent. An AI agent you connect yourself can run the filter's tools, which list its worst mistakes by sender and subject, with codes and account numbers masked, unless you keep those tools from it ([Using an AI agent](ai-agent.md#the-spam-filters-tools)).

To forget it all, delete `~/.local/share/sioul/spam/` on that computer: its table, shared, then leaves your other devices too.

### Sites, Bitwarden and security keys {#sites}

A security key cannot be fooled by a fake page: it answers only the site it was made for. That is why Sioul lets you use one wherever you sign in.

The sites you pin live in one browser profile of Sioul's own, apart from your usual browser. Their notifications are kept by Sioul. The microphone, the camera and sharing the screen are allowed per site, by you; your location is never given, nor the list of your computer's fonts, which sites use to tell computers apart. Downloads go to your downloads folder.

A site that asks for a security key gets it (FIDO2 and WebAuthn: a YubiKey, a Nitrokey), and so do the passkeys kept on it; the key's PIN is asked in Sioul's own dialog. Passkeys kept in a phone or in the system do not work in Sioul's sites on Linux and macOS.

Logins come from your Bitwarden vault, opened by Sioul itself, read only: nothing is ever written to it, your master password and keys are never kept, and the logins stay in memory until Sioul closes. From the Sites page, the vault can open with your security key, as its second step, or alone when Bitwarden knows the key as a passkey. A login made for another domain says so, so that a look-alike site shows. See [Sites](sites.md).

On a phone, sites open in your browser, with its own protections, and Sioul keeps no login there.

### Encrypted mail {#encrypted-mail}

With OpenPGP, Sioul signs and encrypts your messages as you send them, decrypts and checks those you receive, and gives your public key to those you write to (Autocrypt). It keeps a sender's key only from a message it verified, so that a forged message cannot slip in a key for someone you write to. It looks for someone's key on the network only when you ask (**Look for their keys**), since that tells a server to whom you write.

Your secret keys stay on the device they were made on. A key kept on a [security key](accounts.md#security-key) never leaves it: the key signs and opens itself, after its PIN, which Sioul keeps in memory fifteen minutes without use and never writes or logs anywhere. Sioul only uses the key: it never asks for its administration PIN and never changes what is on it. See [Mail](mail.md#signing-and-encrypting).

The subject of an encrypted message stays readable, as in most mail programs today.

### Between your devices {#between-your-devices}

Each device keeps its own data. What must travel between your computers and your phone goes through a folder your own sync app carries (Nextcloud, Dropbox, Syncthing, Google Drive, OneDrive…), sealed on your device before it is written there, with a key made from your passphrase. There is no server of ours in between. The folder's server sees which device wrote, when and how much, and the size of each sealed note or paper; never what. Nothing that happens in that folder, a file damaged or deleted, can take data away from your devices. A notification's words travel only if you turn on **Messages from your phone**, for the apps you choose, a week at most ([On your computers](notifications.md#messages-on-your-computers)). How it works, what it protects and what it cannot hide: [Sharing between your devices](sharing.md).

### AI, only if you ask {#ai-only-if-you-ask}

Sioul connects no AI by itself.

An AI agent you connect, such as Claude Code, can read what you open to it on this device and prepare tasks, events, notes and drafts: each project is closed to agents until you open it, and things in no project can be closed too ([Choosing what an agent sees](ai-agent.md#choosing-what-an-agent-sees)). It cannot send, delete, move money or read a password, and one-time codes, card numbers and bank account numbers are hidden from it; the company behind it receives what it reads. It can also run your own spam filter's tools, which read your mail servers without changing anything there; you can keep these tools from agents. See [Using an AI agent](ai-agent.md).

For an address you protect against harassment, **Let the AI read it first** is off unless you turn it on. Then Sioul sends Anthropic, with your own key, each message in that address's inbox, from anyone, those already there included: its subject and the first 4,000 characters of its text, **unmasked**. Unlike what an agent is given, nothing is hidden there: a code or an account number in such a message reaches Anthropic too. See [the Porch](porch.md#a-public-address-protected).

### A public address {#a-public-address}

An address you publish can be protected against harassment: its mail is read on your device before you see it, and hostile messages are set aside without their words being shown. See [the Porch](porch.md#a-public-address-protected).

### Your mail server {#your-mail-server}

Fetching changes nothing on the server: no message is marked read by being fetched. Sioul writes there only when you act: opening a message marks it read, as any mail program does; archiving, deleting, junking and moving happen ten seconds after you asked, so that **Undo** can stop them. Mail leaves only when you send it, or when you ask a mailing list to let you go. Removing an account removes its password, not its mail.

## What leaves your device, and when {#what-leaves-your-computer-and-when}

| To | What | When |
|---|---|---|
| Your mail provider | your mail: fetched, filed as you ask, sent when you send | always: it is your mail |
| Your calendar and contacts server, or Google | your calendars, tasks and contacts, both ways | always |
| The sites you pin | what any browser sends them | when they are open |
| Your provider's own settings page, and Thunderbird's list of providers | your address's domain, to find the server | when you add an account |
| Your system's DNS resolver | lookups of senders' domains, to check them | when mail arrives, and when your spam filter's training downloads your mail |
| A mailing list's own unsubscribe address | a one-click request, with nothing of yours but what the list wrote in it; or a short message to that address, from yours; or its page, in your browser | only when you choose **Unsubscribe** |
| Open-Meteo | the coordinates of the place you chose, rounded to two decimals, or the name of a town you search | only if you choose a place for the weather; every half hour at most |
| OpenStreetMap's geocoder (Nominatim) | your contacts' postal addresses, once each, nothing else | only after you choose **Place them** |
| OpenStreetMap's map images, or those you chose | which part of the map is shown | while the map is open |
| Their domain's Web Key Directory, then keys.openpgp.org | the addresses you write to | only when you choose **Look for their keys** |
| The address written on your security key, your domain's key directory, then keys.openpgp.org | your security key's fingerprint, and your addresses' domain | only when you choose **Look for it** or **Look for a newer version** |
| Each site you pin | a request for its own icon | about once a week |
| Anthropic | the subject and the first 4,000 characters of the text of each message in an address you protect, unmasked | only with **Let the AI read it first**, and your own key |
| The AI agent you connect, and its provider | what it reads | only if you connect one ([Using an AI agent](ai-agent.md)) |
| GitHub | your token; searches for your issues and pull requests | only if you turn GitHub on |
| Your Bitwarden server, and its own security-key page when your key is your second step | your login, to open your vault | when you fill a login from it |
| Your Nextcloud (Murena's included) | the sharing folder's sealed files | only if Sioul keeps that folder in step itself, or fetches it from there as a backup |
| ClamAV's update servers | a request for virus signatures | once a day, only when your system keeps none of its own |

Sioul has no analytics, no advertising, no crash reports and no update checks.

## What Sioul cannot protect {#what-sioul-cannot-protect}

Saying where protection stops is part of it:

- **Your mailbox stays at your provider.** Sioul is a mail program: your mail, calendars and contacts stay on your providers' servers, and what those servers can read depends on them. Only the mail you encrypt with OpenPGP is unreadable to them, its subject aside. Choose a provider you trust.
- **Your device is protected by your system.** Sioul's files are plain files, so that you and other programs can read them; your system's disk encryption and your session's password keep others out.
- **The folder between your devices tells a little.** Its server sees which device wrote, when and how much ([details](sharing.md#what-it-protects-and-what-it-cannot-hide)).
- **A phone has fewer protections.** On Android there is no antivirus to check attachments and no security key, and sites open in your browser.
- **Look-alike domains are not caught yet**, only borrowed names, and the list of brands is fixed: mostly French services and large platforms.
- **An AI agent you connect reads what you open to it** on that device (the projects you open, and things in no project unless you close them), never passwords, codes, account numbers, medicines and doses, and the company behind it receives what it reads.
- **The AI reading of a protected address sends each message's start as it is**: its subject and first 4,000 characters, codes and account numbers included.
- **Some protections are not yet tried on real hardware**: the antivirus on Windows, security keys with a real key, in the sites and for your OpenPGP key ([For technical readers](#for-technical-readers)).
- **The Windows and macOS packages are not signed yet**: your system warns the first time ([Install](install.md)).

## This website {#this-website}

This website has no analytics, no advertising and no cookies, and loads nothing from other sites. It is hosted by GitHub Pages, which keeps its own server logs, under [GitHub's privacy statement](https://docs.github.com/en/site-policy/privacy-policies/github-general-privacy-statement).

## Reporting a problem {#reporting-a-problem}

A security problem, or anything that worries you: [GitHub issues](https://github.com/aurelienpierre/sioul/issues).

## Compared with other apps {#compared-with-other-apps}

As of October 2026, from each app's own documentation.

✓ documented · partly, with a note · ✗ not found in the app's own documentation (for Sioul: not built) · ? not confirmed · — not applicable.

Each column is a whole set of tools: Thunderbird with Firefox and its password manager; Google, Gmail and Workspace with Chrome and Google Password Manager; Microsoft, Outlook and Microsoft 365 with Edge and Defender for Office 365; Proton, its Mail, Calendar, Drive and Pass with Bridge; Nextcloud, Nextcloud Hub with Mail and Calendar. The mail programs alone are compared on [the Mail page](mail.md#compared-with-other-apps).

=== "Everyday"

    | | Sioul | Thunderbird | Google | Microsoft | Proton | Nextcloud |
    |---|---|---|---|---|---|---|
    | No telemetry by default | ✓ | ✗¹ | ✗² | ✗³ | ✗⁴ | partly⁵ |
    | Each sender's authentication checked and shown | ✓⁶ | ✗⁷ | partly⁸ | partly⁹ | partly¹⁰ | ✗ |
    | Borrowed names and look-alike domains caught | partly¹¹ | ✗¹² | partly¹³ | partly¹⁴ | partly¹⁵ | partly¹⁶ |
    | Remote content blocked by default | ✓¹⁷ | ✓ | partly¹⁸ | partly¹⁹ | partly²⁰ | ✓ |
    | Attachments checked for malware | ✓²¹ | partly²² | ✓²³ | ✓²⁴ | partly²⁵ | partly²⁶ |
    | What syncs between your devices is end-to-end encrypted | ✓²⁷ | partly²⁸ | partly²⁹ | partly³⁰ | ✓³¹ | partly³² |
    | Where your data lives | your devices³³ | your computer³⁴ | Google | Microsoft³⁵ | Proton³⁶ | your server³⁷ |
    | Open source | ✓ | ✓ | ✗³⁸ | ✗ | partly³⁹ | ✓ |

    1. Thunderbird and Firefox send technical and interaction data until you turn it off.
    2. Google collects activity, device information and crash reports, as its privacy policy says.
    3. Microsoft 365 sends optional diagnostic data until an administrator changes it, and required service data always.
    4. Proton Mail, Drive, Calendar and Bridge share usage diagnostics until you turn them off.
    5. A Nextcloud server checks for updates by default, sending its version; the usage survey is a separate app.
    6. Sioul checks SPF, DKIM, DMARC, ARC and reverse DNS itself, as each message arrives, and says on every message whether it is verified, not verified or forged.
    7. Thunderbird: only through an add-on.
    8. Gmail: a question mark on mail that is not authenticated; "Mailed by" and "Signed by" in a message's details, on a computer and on Android.
    9. Outlook: a "?" when it cannot verify the sender, and "via" when the real sender differs.
    10. Proton Mail: a warning on mail that fails its domain's checks; nothing is said when it passes.
    11. Sioul: the names of about fifty brands and public services, and your own domains, even written with look-alike letters; look-alike domains are not caught yet.
    12. Thunderbird flags deceptive links, not borrowed names.
    13. In Workspace, set by the administrator: domains that look like the company's, and its people's names; in Gmail, a look-alike of a known sender's address is one reason for spam.
    14. In Defender for Office 365, for the people and domains an administrator lists.
    15. Proton Mail: PhishGuard flags potentially spoofed addresses; link confirmation warns of look-alike letters in links.
    16. Nextcloud Mail: a sender whose address differs from the one in your address book, a Reply-To that differs, a link that leads elsewhere than its text.
    17. Never loaded: no setting loads it.
    18. Gmail: shown at once, through Google's servers, which hide your device and location; a setting makes it ask first.
    19. Classic Outlook blocks pictures by default; Outlook.com loads them through Microsoft's proxy.
    20. Proton Mail: trackers removed and images loaded through Proton's proxy, shown at once.
    21. By your system's antivirus (ClamAV, Microsoft Defender) before an attachment opens; programs never open. Not on phones.
    22. Thunderbird lets your system's antivirus quarantine new messages; it has no scanner of its own.
    23. Executable files blocked and attachments scanned; a sandbox in Business Standard and above.
    24. On by default for every mailbox; detonation in Defender for Office 365.
    25. Proton Mail: on its servers, for mail that is not end-to-end encrypted; Proton Drive checks files against a list of known malicious ones.
    26. Files uploaded to Nextcloud, with its antivirus app; not mail attachments.
    27. What Sioul carries between your devices, with no server of Sioul's; your mail, calendars and contacts travel through your own providers, as with any mail program.
    28. Firefox Sync is end to end; Thunderbird has no sync of its own.
    29. Not Gmail or Drive; client-side encryption in Enterprise Plus, Education Standard and Plus, Frontline Plus; passwords once on-device encryption is set up.
    30. Microsoft holds the keys, except with Double Key Encryption (Microsoft 365 E5); Edge encrypts passwords before syncing them.
    31. Mail, calendar, Drive and Pass; subjects and addresses are not end to end.
    32. Chosen empty folders, from the desktop and mobile apps; not calendars, contacts or mail.
    33. In plain files; your mail also at your provider; the sharing folder sealed, on the cloud you choose.
    34. Thunderbird's profile; your mail at your provider; Firefox Sync's data at Mozilla, encrypted.
    35. The EU Data Boundary for organisations signed up in the EU or EFTA.
    36. Proton AG, in Switzerland.
    37. Or your provider's; your mail at your mail provider.
    38. Chromium, which Chrome is built on, is open source.
    39. Its apps, independently audited; not its servers.

=== "Technical"

    | | Sioul | Thunderbird | Google | Microsoft | Proton | Nextcloud |
    |---|---|---|---|---|---|---|
    | Your mail key on a security key or smart card | partly¹ | partly² | partly³ | ?⁴ | ✗⁵ | ✗ |
    | OpenPGP built in | ✓⁶ | ✓⁷ | ✗ | ✗ | ✓ | partly⁸ |
    | Security keys and passkeys to sign in | partly⁹ | ✓¹⁰ | ✓¹¹ | ✓ | ✓¹² | ✓¹³ |
    | Password manager | ✓¹⁴ | ✓ | ✓ | ✓ | ✓¹⁵ | partly¹⁶ |
    | S/MIME | ✗ | ✓ | partly¹⁷ | ✓ | ✗ | ✓ |
    | Stored mail unreadable to the provider | partly¹⁸ | partly¹⁸ | partly¹⁹ | partly²⁰ | ✓²¹ | partly¹⁸ |

    1. Sioul: an OpenPGP card (YubiKey, Nitrokey) signs and opens, its PIN held in memory only, never written or logged; on computers; so far tried with a software card, not yet with a real key.
    2. Thunderbird: through GnuPG, which you install and set up yourself.
    3. Google: a PIV smart card holds the S/MIME key, with client-side encryption, on Windows, in the top Workspace editions with Assured Controls.
    4. Microsoft: a smart card for Outlook's S/MIME is not confirmed on the pages read.
    5. Proton: its key documentation mentions no smart card or security key.
    6. Sioul: with Autocrypt (keys taken from verified mail only) and the Web Key Directory.
    7. Thunderbird: keys found by WKD and keys.openpgp.org; Autocrypt in part.
    8. Nextcloud: through the Mailvelope browser extension.
    9. Sioul: security keys, and the passkeys kept on them, in the sites you keep and to open your Bitwarden vault from the Sites page; on computers; built on Chromium's WebAuthn and not yet tried with a real key; passkeys kept in a phone or in the system do not work on Linux and macOS.
    10. Firefox: USB keys with their PIN, and passkeys; the Mozilla account itself takes authenticator codes.
    11. Google: passkeys, security keys, and Advanced Protection.
    12. Proton: security keys as the account's second step; passkeys saved in Proton Pass.
    13. Nextcloud: WebAuthn login, and FIDO2 keys as a second step.
    14. Sioul: reads your Bitwarden vault, without ever writing to it, and fills logins and their codes.
    15. Proton Pass, with passkeys.
    16. Nextcloud: a community app in its app store.
    17. Google: in Workspace; the editions were not confirmed on the page read.
    18. Only the mail you encrypt (OpenPGP, or S/MIME where offered); the rest stays as your provider keeps it.
    19. Google: client-side encryption in some Workspace editions; not Gmail otherwise.
    20. Microsoft: S/MIME, and Double Key Encryption in Microsoft 365 E5.
    21. Proton: zero-access encryption of every stored message.

??? info "Sources"
    Mozilla's help articles were read in the Internet Archive's copies, since support.mozilla.org refused automated reading that day.

    - Thunderbird, OpenPGP and smart cards: <https://support.mozilla.org/en-US/kb/openpgp-thunderbird-howto-and-faq>, read 8 October 2026.
    - Thunderbird, remote content: <https://support.mozilla.org/en-US/kb/remote-content-in-messages>, read 8 October 2026.
    - Thunderbird, scam detection: <https://support.mozilla.org/en-US/kb/thunderbirds-scam-detection>, read 8 October 2026.
    - Thunderbird, security panel (antivirus, passwords): <https://support.mozilla.org/en-US/kb/security-panel-settings-in-thunderbird>, read 8 October 2026.
    - Firefox, Sync: <https://support.mozilla.org/en-US/kb/sync>, read 8 October 2026.
    - Firefox, password manager: <https://support.mozilla.org/en-US/kb/password-manager-remember-delete-edit-logins>, read 8 October 2026.
    - Mozilla account, two-step authentication: <https://support.mozilla.org/en-US/kb/secure-mozilla-account-two-step-authentication>, read 8 October 2026.
    - Thunderbird, privacy notice (telemetry): <https://www.mozilla.org/en-US/privacy/thunderbird/>, read 8 October 2026.
    - Firefox, privacy notice (telemetry): <https://www.mozilla.org/en-US/privacy/firefox/>, read 8 October 2026.
    - Firefox 114, release notes (FIDO2): <https://www.firefox.com/en-US/firefox/114.0/releasenotes/>, read 8 October 2026.
    - Firefox 122, release notes (passkeys): <https://www.firefox.com/en-US/firefox/122.0/releasenotes/>, read 8 October 2026.
    - Mozilla, licensing: <https://www.mozilla.org/en-US/foundation/licensing/>, read 8 October 2026.
    - Firefox Sync's server (licence): <https://github.com/mozilla-services/syncstorage-rs>, read 8 October 2026.
    - Thunderbird, downloads (platforms): <https://www.thunderbird.net/en-US/download/>, read 8 October 2026.
    - Gmail, TLS and S/MIME: <https://support.google.com/mail/answer/6330403>, read 8 October 2026.
    - Google Workspace, client-side encryption: <https://knowledge.workspace.google.com/admin/security/about-client-side-encryption>, read 8 October 2026.
    - Google Workspace, hardware key encryption: <https://knowledge.workspace.google.com/admin/security/gmail-only-set-up-and-manage-hardware-key-encryption>, read 8 October 2026.
    - Google Password Manager, on-device encryption: <https://support.google.com/accounts/answer/11350823>, read 8 October 2026.
    - Google Workspace, data regions: <https://knowledge.workspace.google.com/admin/compliance/choose-a-geographic-location-for-your-data>, read 8 October 2026.
    - Gmail, authentication shown: <https://support.google.com/mail/answer/180707> and <https://support.google.com/mail/answer/1311182>, read 8 October 2026.
    - Gmail, spam and the reasons shown: <https://support.google.com/mail/answer/1366858>, read 8 October 2026.
    - Google Workspace, impersonation and malware protection: <https://knowledge.workspace.google.com/admin/gmail/advanced/advanced-phishing-and-malware-protection>, read 8 October 2026.
    - Gmail, blocked files: <https://support.google.com/mail/answer/6590>, read 8 October 2026.
    - Google Workspace, attachment sandbox: <https://knowledge.workspace.google.com/admin/gmail/advanced/set-up-rules-to-detect-harmful-attachments>, read 8 October 2026.
    - Gmail, images: <https://support.google.com/mail/answer/145919>, read 8 October 2026.
    - Google, passkeys: <https://support.google.com/accounts/answer/13548313>, read 8 October 2026.
    - Google, Advanced Protection: <https://landing.google.com/advancedprotection/>, read 8 October 2026.
    - Google, privacy policy: <https://policies.google.com/privacy>, read 8 October 2026.
    - Chrome, crash reports: <https://support.google.com/chrome/answer/96817>, read 8 October 2026.
    - Chromium: <https://www.chromium.org/chromium-projects/>, read 8 October 2026.
    - Microsoft, anti-phishing policies (impersonation): <https://learn.microsoft.com/en-us/defender-office-365/anti-phishing-policies-about>, read 8 October 2026.
    - Microsoft, spoofing protection: <https://learn.microsoft.com/en-us/defender-office-365/anti-phishing-protection-spoofing-about>, read 8 October 2026.
    - Outlook, phishing and suspicious behaviour ("?", "via"): <https://support.microsoft.com/en-us/outlook/mail/phishing-and-suspicious-behavior-in-outlook>, read 8 October 2026.
    - Microsoft, anti-malware protection: <https://learn.microsoft.com/en-us/defender-office-365/anti-malware-protection-about>, read 8 October 2026.
    - Outlook, picture downloads: <https://support.microsoft.com/en-us/office/block-or-unblock-automatic-picture-downloads-in-email-messages-15e08854-6808-49b1-9a0a-50b81f2d617a>, read 8 October 2026.
    - Outlook.com, external image protection: <https://support.microsoft.com/en-us/office/external-image-protection-in-outlook-com-43c0c17e-8fd1-41c6-93fe-ffe54638e82b>, read 8 October 2026.
    - Exchange Online, S/MIME: <https://learn.microsoft.com/en-us/exchange/security-and-compliance/smime-exo/smime-exo>, read 8 October 2026.
    - Microsoft, Double Key Encryption: <https://learn.microsoft.com/en-us/purview/double-key-encryption>, read 8 October 2026.
    - Microsoft, EU Data Boundary: <https://learn.microsoft.com/en-us/privacy/eudb/eu-data-boundary-learn>, read 8 October 2026.
    - Microsoft, passkeys and FIDO2: <https://learn.microsoft.com/en-us/entra/identity/authentication/how-to-enable-passkey-fido2> and <https://www.microsoft.com/en-us/security/blog/2025/05/01/pushing-passkeys-forward-microsofts-latest-updates-for-simpler-safer-sign-ins/>, read 8 October 2026.
    - Edge, password manager security: <https://learn.microsoft.com/en-us/deployedge/microsoft-edge-security-password-manager-security>, read 8 October 2026.
    - Microsoft 365 Apps, diagnostic data: <https://learn.microsoft.com/en-us/microsoft-365-apps/privacy/overview-privacy-controls>, read 8 October 2026.
    - Outlook, digital IDs: <https://support.microsoft.com/en-us/office/get-a-digital-id-0eaa0ab9-b8a2-4a7e-828b-9bded6370b7b>, read 8 October 2026.
    - Proton Mail, encryption: <https://proton.me/support/proton-mail-encryption-explained>, read 8 October 2026.
    - Proton Mail, authentication warning: <https://proton.me/support/email-has-failed-its-domains-authentication-requirements-warning>, read 8 October 2026.
    - Proton Mail, security features (PhishGuard): <https://proton.me/mail/security>, read 8 October 2026.
    - Proton Mail, homograph links: <https://proton.me/support/homograph-attacks>, read 8 October 2026.
    - Proton Mail, reporting phishing: <https://proton.me/support/report-phishing>, read 8 October 2026.
    - Proton Mail, link confirmation: <https://proton.me/support/link-confirmation>, read 8 October 2026.
    - Proton Mail, privacy policy (virus scanning): <https://proton.me/mail/privacy-policy>, read 8 October 2026.
    - Proton Drive, malware protection: <https://proton.me/support/proton-drive-malware-protection>, read 8 October 2026.
    - Proton Mail, tracker protection: <https://proton.me/support/email-tracker-protection>, read 8 October 2026.
    - Proton Mail, images: <https://proton.me/support/protonmail-images>, read 8 October 2026.
    - Proton Mail, PGP: <https://proton.me/support/how-to-use-pgp>, read 8 October 2026.
    - Proton, encryption keys: <https://proton.me/support/account/security-privacy/encryption-keys> and <https://proton.me/support/openpgp-keys-security>, read 8 October 2026.
    - Proton, security keys: <https://proton.me/support/2fa-security-key>, read 8 October 2026.
    - Proton Pass, passkeys: <https://proton.me/support/pass-use-passkeys>, read 8 October 2026.
    - Proton, usage statistics: <https://proton.me/support/share-usage-statistics>, read 8 October 2026.
    - Proton, privacy policy: <https://proton.me/legal/privacy>, read 8 October 2026.
    - Proton, open source: <https://proton.me/community/open-source>, read 8 October 2026.
    - Proton Mail, desktop app: <https://proton.me/support/mail-desktop-app>, read 8 October 2026.
    - Nextcloud, server-side encryption: <https://docs.nextcloud.com/server/latest/admin_manual/configuration_files/encryption_configuration.html>, read 8 October 2026.
    - Nextcloud, end-to-end encryption: <https://docs.nextcloud.com/server/latest/user_manual/en/files/using_e2ee.html>, read 8 October 2026.
    - Nextcloud Mail, phishing detection: <https://docs.nextcloud.com/server/latest/user_manual/en/groupware/mail.html>, read 8 October 2026.
    - Nextcloud, antivirus: <https://docs.nextcloud.com/server/latest/admin_manual/configuration_server/antivirus_configuration.html>, read 8 October 2026.
    - Nextcloud Mail, blocked images: <https://github.com/nextcloud/mail/blob/main/src/components/BlockedContentWarning.vue>, read 8 October 2026.
    - Nextcloud Mail, Mailvelope and S/MIME: <https://github.com/nextcloud/mail/blob/main/README.md>, read 8 October 2026.
    - Nextcloud, two-factor authentication and WebAuthn: <https://docs.nextcloud.com/server/latest/user_manual/en/user_2fa.html>, read 8 October 2026.
    - Nextcloud, Passwords app: <https://apps.nextcloud.com/apps/passwords>, read 8 October 2026.
    - Nextcloud, update check: <https://docs.nextcloud.com/server/latest/admin_manual/configuration_server/config_sample_php_parameters.html>, read 8 October 2026.
    - Nextcloud, usage survey: <https://github.com/nextcloud/survey_client>, read 8 October 2026.
    - Nextcloud Mail, licence: <https://github.com/nextcloud/mail/blob/main/appinfo/info.xml>, read 8 October 2026.

## For technical readers {#for-technical-readers}

- **Sender checks**: Stalwart's `mail-auth`, at arrival, through the system's resolver: SPF (RFC 7208) on the hop where the message entered your provider, DKIM (RFC 6376), DMARC (RFC 7489), ARC (RFC 8617), iprev. Results are prepended as `Authentication-Results` (RFC 8601) under an authserv-id unique to the installation (`sioul-….invalid`); a provider's own results count only under its id, learned from its mail. Verified: DMARC pass, else a valid DKIM signature of the From domain. Forged: DMARC fail under `p=quarantine` or `p=reject`. Not authenticated: SPF and DKIM fail, no DMARC pass, no ARC sealed by your provider or your domains; such a sender is a stranger for every rule of the Porch. More: [Mail, how a sender is checked](mail.md#how-a-sender-is-checked).
- **Borrowed names**: Unicode confusable skeletons (UTS #39), against about fifty brands and public services and your own domains.
- **HTML**: an ammonia allow-list of text tags; `href` the only attribute; http, https and mailto the only schemes; relative links refused; no `<img>`, no style, no script. A link's site is read after any `name@` put before it to mislead.
- **Attachments**: ClamAV (`clamdscan --fdpass`, else `clamscan`; signatures refreshed daily with `freshclam` when the system keeps none) or AMSI; programs only saved; names cleaned of folders, and on Windows of device names and streams; the Mark of the Web (`Zone.Identifier`, ZoneId=3) and `com.apple.quarantine` on what is opened or saved.
- **Transport**: rustls with *ring*, TLS 1.2 and 1.3, the system's root certificates (Mozilla's when a system has none); IMAP and SMTP over TLS from the first byte (RFC 8314) or STARTTLS, with no clear-text option, and a server without STARTTLS refused; CalDAV, CardDAV, Bitwarden and key lookups over HTTPS only. SMTP greets as `[127.0.0.1]`, so that your computer's name stays out of `Received` lines. Google: OAuth 2.0 for native apps (RFC 8252) on a random loopback port, PKCE S256 (RFC 7636), the state compared in constant time; the refresh token in the keyring, the access token in memory.
- **Sharing**: XChaCha20-Poly1305 with a random 192-bit nonce per record, each bound to its device, round, place and clock as associated data. The key comes from Argon2id (64 MiB, 3 passes, a 16-byte salt), and each device's keyring keeps the key, not the passphrase. Notes and papers are gzip-compressed and sealed in 1 MiB pieces under an HKDF-SHA-256 sub-key, named by an HMAC-SHA-256 of their content. Details: [Sharing, for technical readers](sharing.md#for-technical-readers).
- **OpenPGP**: Sequoia, pure-Rust cryptography, standard policy; RFC 9580, PGP/MIME (RFC 3156), inline read. New keys are version 4 on Curve25519 (EdDSA, ECDH), valid three years. Encrypted mail is signed inside, to every recipient and to you. Autocrypt sends your key and takes keys from verified mail only. "Look for their keys" asks the Web Key Directory, then keys.openpgp.org, over HTTPS. Tested both ways with GnuPG 2.4.
- **OpenPGP on a security key**: OpenPGP card 3.4 (YubiKey, Nitrokey) over PC/SC, in shared mode, one transaction per operation, the card reset after. Each signature is verified with the certificate before use, and a certificate is accepted only if it holds the card's own public keys. The PIN is held in memory only (Sequoia's `Password`), forgotten after 15 minutes without use, when the key is pulled out, or at close. No logger is installed, and logging is capped as each program starts, so that the card library's traces, which would include the PIN, are never written. Never the Admin PIN; nothing is written to the card.
- **Sites**: Qt WebEngine's Chromium WebAuthn (FIDO2, CTAP2 over USB, passkeys on the key), with Sioul's own dialog for the account and the PIN; a profile of its own; location, local fonts and pointer lock refused; no certificate error ever accepted by Sioul.
- **Bitwarden**: Sioul's own client, read only. PBKDF2-SHA256 or Argon2id as your account says, held to Bitwarden's own bounds so that a server cannot make your password cheaper to guess; the login sends a hash, never the password; HKDF stretching, AES-256-CBC with HMAC-SHA256 (the MAC checked first, in constant time) or COSE, organisation keys through RSA-OAEP. Keys live in zeroizing memory, and the keyring keeps only the device's token. The second step can be a FIDO2 key, through Bitwarden's own security-key page; passkey login uses the WebAuthn PRF extension; one-time codes follow RFC 6238.
- **Keyring**: the Secret Service, the Windows Credential Manager, the macOS Keychain, Android's KeyStore (AES-GCM, the key never leaving the KeyStore).
- **Files**: Sioul's folders 0700, made or narrowed so as Sioul starts, and its sensitive files 0600, on Linux and macOS.
- **AI agents**: MCP over standard input and output only, so no network port is opened. Codes, sign-in links, IBANs (mod 97), card numbers (Luhn) and French social security numbers are masked; mail and notes come framed as data; each call is logged by its addresses and length, never its words. More: [Using an AI agent](ai-agent.md#for-technical-readers).
- **The shield's AI**: Claude Haiku 4.5 through Anthropic's API, with your key from the keyring; the subject and the first 4,000 characters of each message to that address, unmasked; no redirect followed, so that the key goes to Anthropic alone.
- **No telemetry**: no analytics, crash-report or update-check code exists. The only network destinations in the code are those of the table above.
- **As tested**: the antivirus on Windows (AMSI) follows Microsoft's documented sequence and is built by the project's automatic builds, but has not yet run on Windows; the OpenPGP security key is tested against a software card, and security keys in sites are not yet tried with a real key; Google's sign-in is tried against stand-ins of Google only.
- **The code** is free software, under the GPL-3.0-or-later licence, so that all of this can be checked: [github.com/aurelienpierre/sioul](https://github.com/aurelienpierre/sioul). The Android package is signed with Sioul's own key; the Windows installer is not signed yet, and the macOS image is not notarised yet.
