---
description: Your admin stays usable without Sioul. Sioul connects standard protocols and files (IMAP, CalDAV, CardDAV, WebDAV, Markdown notes, PDF files, images, audio) and has no private file format; it gathers only the management of your admin and the links between things, in one application on each system; its extensions turn closed services into ordinary calendars and address books that anyone can share. What opens each kind of data without Sioul, what is Sioul's own and how to take it out, and where this does not hold yet.
---

# Your admin stays usable without Sioul {#without-sioul}

Interoperability is the ground of Sioul. Four things follow from it:

- **Sioul connects what is standard.** Your mail comes through IMAP and leaves through SMTP; your agenda and your tasks are on a CalDAV server, your contacts on a CardDAV server; the sharing between your devices can go through a WebDAV server, such as your Nextcloud. Your notes are Markdown files, your papers and letters are PDF files and images, a voice memo is an audio file.
- **There is no private file format.** By design, everything stays readable and writable by the other programs that handle the same standard formats.
- **Sioul gathers only the management of your admin, and the links between things.** Your mail, agenda, tasks, contacts, notes, papers and money come together in one application, which keeps the ties between them, and that application is the same on each system you use: Linux, Windows, macOS and Android (iOS not yet). Your data itself stays in its standard places.
- **Extensions turn closed services into standard calendars and address books.** An extension reads a service that offers no standard, such as a school's timetable or a salon's bookings, and writes what it reads into a calendar or an address book of your own, on your own server. You can share that calendar with people who do not use Sioul: they read it and, when you let them change it, write in it with their own programs, as with any calendar ([below](#a-calendar-an-extension-fills)).

If Sioul stopped tomorrow, your mail, your agenda, your contacts, your notes and your papers would still be where they are, and the programs you choose would open them. Where this does not hold yet is said plainly at the end of this page ([below](#where-it-does-not-hold-yet)).

## What opens your data without Sioul {#what-opens-your-data}

| What | Kept as | Opened without Sioul by |
|---|---|---|
| Mail | on your mail server (IMAP), and on your device as one file per message (Maildir), sealed on a computer ([below](#what-is-sioul-s-own)) | any mail program: Thunderbird, your phone's mail app, your provider's webmail; on a computer, mail programs that read Maildir, such as notmuch or mutt, from the server or from `sioul store export` |
| Drafts | in Sioul (sealed on a computer), and a copy of each in your mail server's Drafts folder (an encrypted draft's encrypted to your own key) | any mail program, in its Drafts |
| Agenda and tasks | on your calendar server (CalDAV), and on your device as one file per event or task (iCalendar, `.ics`), sealed on a computer | any calendar program: Thunderbird, your phone's calendar through DAVx⁵, Nextcloud's Calendar and Tasks, Tasks.org; on a computer, khal, from the server or from `sioul store export` |
| Contacts | on your contacts server (CardDAV), and on your device as one file per card (vCard, `.vcf`), sealed on a computer | any address book: your phone's, Thunderbird's, Nextcloud's; on a computer, khard, from the server or from `sioul store export` |
| Notes | a folder of Markdown files that you choose, outside Sioul's own folders | Obsidian, Nextcloud Notes, any text editor |
| Papers and paper letters | the files as you gave them (PDF, images), in folders of your own (`Documents/Papers` and `Documents/Letters` unless you chose others); a letter's text read from its scan, as plain text | any file manager, PDF reader or image viewer |
| Invoices you issue | a PDF/A-3 file, which carries its Factur-X electronic invoice (the European standard EN 16931) when the invoice has what that standard asks (under France's VAT franchise without a VAT number, your SIREN stands where it asks for a tax number), checked by an outside validator each time Sioul's way of making invoices changes | any PDF reader; accounting programs and e-invoicing platforms that read Factur-X |
| Your texts | on your phone, in Android's own store | your phone's messages app, and the backup apps that copy it |
| Your bank's lines | your bank's own export (OFX, CAMT.053 or CSV), which Sioul reads | your bank, a spreadsheet |

Sioul keeps a copy of your mail, agenda and contacts on each device, so that it works without waiting for the network. The server keeps the original, and every other program you use on the same account sees the same mailbox, the same agenda and the same contacts: what each one sees of what Sioul did is in [Works with](compatibility.md#other-apps-on-the-same-accounts).

## What is Sioul's own, and the way out {#what-is-sioul-s-own}

Some things exist only because Sioul keeps them for you: your projects and the mail that comes to each by itself, budgets, money movements and contracts, supplies, client profiles, the time spent on tasks, your medicines, your people, the ties between things, your rules and filters, your settings, your drafts. Sioul keeps them as text files (TOML, a format made to be read and written by hand), never in a database, and on a computer it seals them on the disk with everything else it keeps there, so that a lost disk or a backup in other hands gives nothing:

- **The way out, whole**: on a computer, `sioul store export --to <folder>` writes every file Sioul keeps in clear into an empty folder of yours, each in its standard format and in the layout below (Maildir, `.ics`, `.vcf`, TOML, JSON); a client's file only when you add `--clients`: its description and notes in Markdown, its records as TOML, and, beside them, each client's records as a SQLite database (`records.sqlite`, a table per kind of record, a column per field) and as one CSV table per kind, which any database tool or spreadsheet opens. Notmuch, mutt, khal, khard, vdirsyncer and any text editor read what it writes.
- **Where**: on Linux, `~/.config/sioul/` (settings), `~/.local/share/sioul/` (data; Sioul's lists in its `records/` folder) and `~/.local/state/sioul/`; the same on Windows and macOS in the system's own folders ([Privacy and security](privacy-security.md#what-stays-on-your-computer)). On a computer they hold sealed files; on a phone, plain files in its private storage.
- **In your events, tasks and contacts**, Sioul writes a few lines of its own (their names start with `X-SIOUL-`: the time kept around an appointment, a task's project, its ties). The calendar standard asks every program to keep such lines; another program shows a plain event or task, with its time, its title and its place.
- **From the command line**, on a computer, without the window: `sioul invoices export` gives every invoice as a spreadsheet, or one invoice as its PDF; `sioul receipts --csv` the receipts book; `sioul movements --json` every money movement; `sioul links <address> --chain --json` the ties of a thing; `sioul projects` your projects.
- **What is only machinery** has nothing to take out: the spam filter's table is made again by its training, and the caches are made again from the rest.

Every kind of data, where it lives, its format and its way out, checked line by line in Sioul's code: [the developers' note](../dev/formats.md).

## Between your devices {#between-your-devices}

Your mail, and the agendas and address books on your servers, travel by those servers, never through Sioul's sharing: every program sees them there.

What only Sioul keeps travels between your devices through the [sharing folder](sharing.md), sealed end to end on your device before it leaves: no server, and no one who holds the folder, can read it. What is inside is the same plain files: once your other device opens a change, it writes it into the same TOML files, and your notes and papers into the same Markdown, PDF and image files. So each of your devices holds your data in the open formats above (your texts aside: [below](#where-it-does-not-hold-yet)), and any one of your computers is a way out (a phone keeps them in its private storage: [below](#where-it-does-not-hold-yet)). The sealed folder on its own opens only in Sioul, with your passphrase: keep at least one device, or a backup of one device's folders.

Your notes and papers are plain files on your server when your own sync app carries their folder (Nextcloud's client, Syncthing, Dropbox). Carried by Sioul's sharing instead (on a phone whose sync app does not carry the notes folder), they are sealed there like the rest.

## A calendar an extension fills {#a-calendar-an-extension-fills}

An extension, such as Planity's or Skolengo's, writes its events into a calendar of yours, which you choose in [Settings ▸ Extensions](settings.md#extensions). It is an ordinary calendar on your server: share it from Agenda ⚙ ▸ Your agendas ([Your agendas on their servers](agenda.md#agendas-on-their-servers)) or from your server's own page: those you let change it read it and write in it with any calendar program, without Sioul; anyone else can subscribe to its read-only link. Choose a calendar on your server rather than one on this device only, which no one else can reach.

Sioul reads the service again when it starts, then every three hours while its window is open, and on a phone in its background step. Each time, for the days the service answered:

- **An event someone changed** (its title, a note, its time) stays as they left it, as long as the service does not change that event. When the service changes it (the school moves the lesson), Sioul writes it again from the service, and their change is replaced. The event's description says so: "a change made here is replaced when Skolengo changes this event."
- **An event someone deleted** comes back, as long as the service still has it: the timetable is the school's.
- **An event the service no longer has** is taken out, even if someone changed it.
- **An event someone turned into a meeting**, by inviting guests, is left as it is from then on: the extension never changes a meeting, since its server would tell every guest.
- **Events others add to that calendar themselves** are never read as the extension's, never changed, never removed.

Sioul knows its events by their identifier, which every program keeps, and notes on each, in a line of its own, the service's version it wrote. Most programs keep that line, as the calendar standard asks; if a program removes it, another of your devices that runs the extension may write the event again from the service, and the change made in that program is replaced.

**What kind of appointment, and with whom.** An extension's events carry, after its name, categories of their own (Planity: the services booked), all on one line, which DAVx⁵ on a phone, Thunderbird and Nextcloud's calendar read. An appointment whose client has a card in your address books names that card in the event itself, the calendar standard's own way (`CONTACT`, the client's name and the card's identifier), never as a guest: no one is invited. Other programs keep that line, though none of those checked shows it yet ([the developers' note](../dev/research/calendar-contacts.md)); Sioul reads it, from whichever program wrote it, as a tie between the event and the card.

An address book an extension fills works the same way, more gently: Sioul only adds to a card what it lacks, never removes a line, never replaces what someone changed, and never deletes a card.

**Nothing goes back to the service.** A change made in the calendar, in Sioul or in another program, stays in the calendar alone, until the service changes that event (above): Sioul does not write to Planity or to Skolengo, and editing both ways is not built. To change a booking or a lesson for good, change it at the service.

## On which systems {#on-which-systems}

Linux, Windows, macOS and Android: GitHub builds Sioul for each of them at every change of its code, and runs its tests on Linux, Windows and macOS. It is used every day on Linux and on an Android phone. iOS: not yet. [Install](install.md)

## Where it does not hold yet {#where-it-does-not-hold-yet}

- **Sioul's archive of your texts**, on each of your devices, is sealed, so that a copy is unreadable without your sharing's key. Your phone's messages app stays the record of your texts; but a text you delete on the phone stays only in Sioul's archive, and no other program opens it.
- **Your notes and papers on a server** are plain files there only when your own sync app carries their folder: Sioul does not yet write them to a WebDAV server as plain files itself. Carried by Sioul's sharing, they are sealed there, and Nextcloud Notes or Obsidian elsewhere cannot open them ([above](#between-your-devices)).
- **On an Android phone**, Sioul's own files are in its private storage, which other apps cannot open. Your notes go there too until you choose a notes folder: choose one in the phone's shared storage (in Documents, for example) on the Notes page's ⚙, and other apps can open your notes. The papers', the letters' and the invoices' folders can be there too (chosen in Papers ▸ ⚙, Letters ▸ ⚙ and Settings ▸ Invoices). The command line (above) is a computer's: a phone has none.
- **The sealed sharing folder** opens only in Sioul, with your passphrase ([above](#between-your-devices)).
- **Your rules and mail filters** run only in Sioul; another program keeps their file but does not run them.
- **Sioul's own files are sealed on a computer** (its settings, lists and records, its copies of your mailboxes, agendas and address books): other programs read them through your servers, or through what `sioul store export` writes ([above](#what-is-sioul-s-own)). Your notes, papers and letters stay plain files, outside, for Obsidian, Nextcloud Notes and your system, unless you place one of their folders inside Sioul's own: it is then sealed with the rest, and the window says what that gains and what it loses.
