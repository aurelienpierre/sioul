# Sioul

<img src="data/icons/sioul.svg" width="96" height="96" alt="Sioul's icon: a quill with a pen nib, cream on green.">

*Sioul* [siwl] is Breton for calm, peaceful, silent.

**Most software expects people to adapt to it. Sioul adapts the demands of the world to the person.**

Mail, tasks, appointments, bills, the letters institutions leave on their websites, the papers asked again and again: the administrative machinery of modern life assumes someone always available, always at full strength, who remembers how a dozen separate systems relate to each other. Sioul starts from the other end. It begins with what you can give today (your attention, your energy, your health, your rest) and fits the demands of work, institutions and money into what is left.

It is a desktop application for mail, tasks and everyday admin, free software that runs on your own computer: a quiet place between you and that machinery.

**Website and user guide: [aurelienpierre.github.io/sioul](https://aurelienpierre.github.io/sioul/)**

![Sioul's window: the list of places on the left (Porch, Tasks, Mail, Sites, Agenda, Contacts, Notes, Projects, Time, Budgets, Papers, Health); on the right, the Porch, with a one-time code and its Copy button on top, a chat's news, then the new mail sorted into lanes.](website/docs/assets/screens/porch.png)

## The other way round

| Usual software | Sioul |
|---|---|
| starts from your obligations, fills your days with them, and leaves you what remains | starts from your needs and from what today can hold, keeps that time for you, and plans the obligations in what remains |
| shows everything that has arrived, as soon as it arrives | shows what belongs in your attention now, in the hours you chose |
| gives each kind of thing its own program, and leaves you to be the glue between them | ties everything together in one place, and keeps the ties for you |
| counts what is late, overdue, missed | starts the plan again from today, and never counts what was not done |

- **Your needs first, then the work.** Meals, rest and sleep, your hours for work, for your own admin and for yourself, and the time to get ready, get there and come back around each event are kept first. Each day you say how it is (clear, haze or fog); nothing is guessed from what you do. The work goes in what remains, as one next step with its reason. Nothing is ever overdue.
- **A porch between the world and your attention.** Mail, the "secure mailboxes" of banks and offices, chats: everything new waits on the Porch, checked (genuine or forged), sorted, and shown in the hours you chose. The question is not what has arrived, but what belongs in your attention now. The codes you just asked for come at once.
- **The software keeps the links.** A letter, the task it asks for, the appointment, the PDF, the person, the payment: one case, each piece found from the others. You are no longer the glue between a mail program, a calendar, a task list, a folder and a bank's website.
- **Nothing to be afraid of getting wrong.** No unread counts, badges, red, sounds or streaks; ten seconds to undo anything moved, deleted or sent; nothing sent, deleted or paid without you, not even by an AI agent. The words are chosen with the same care: reminders that neither call you ill nor praise you, stopping early said as the ordinary thing it is, no forced cheer, no talking down.

## For whom

For anyone whose capacity is limited or changes from day to day, and for whom admin hurts. Sioul was designed first with autistic people, people with ADHD, and people who are anxious, traumatised, burnt out, depressed or exhausted in mind. The same needs come with long COVID, ME/CFS and other conditions that limit energy, with an eating disorder or irregular eating, with caring for someone, with a bad stretch of life. No diagnosis is needed, and Sioul guesses nothing about your state: you say what you can do, and it plans around that.

Many neurodivergent people work for themselves, because office life does not fit them, and working for yourself brings more admin. So Sioul also carries a small business, from a client's first mail to the paid invoice.

## Research, refusals, and care

Each rule in Sioul comes from a chain: what studies observed, the mechanism behind it, the rule it gives, what Sioul does, and what it refuses to do: [what the research says](docs/research.md), and [the research notes](docs/research/README.md), each study with how strong its evidence is. Sioul refuses, each time with the evidence, what other software does to people: streaks and points, "overdue" counts, repeated reminders, mood and energy journals, guessing capacity or mood from behaviour or a watch, schedules that move things without asking, an AI that sends, books or pays on its own. What is not known yet is whether Sioul itself measurably lightens admin: that is still to be tested with the people who use it.

Sioul is the working counterpart of a book by the same author, *Design and Engineering, in Spite of Open-Source* ([free, in PDF and EPUB](https://editions.aurelienpierre.com/en/concevoir/)), whose conclusion is that to design is to care: "A tool serves its user, or it betrays them. There is no in-between." The same reasoning decides how Sioul is built: your data stays on your computer, in plain files and your own accounts; it travels between your devices sealed, through a folder your own sync app carries; there is no server of ours; the code is free. A tool meant to lift the weight of administrative machinery cannot tie you to a service you cannot leave.

## What it does

- **Mail waits on a porch.** It is checked (genuine or forged), sorted into lanes, and shown in the hours you chose. The codes and links you just asked a site for come at once, quietly.
- **Hours set in advance**: working hours, hours for your own admin, free time. Each address, site, budget and task belongs to one or several of them; the rest waits, out of sight. Outside every hours set, it is rest: only the people you marked safe reach you.
- **One next step.** Tasks that wait for each other are ordered into the one step to take now, with its reason, placed in the hours meant for it. Nothing is ever overdue. Starting is helped; stopping counts. The time to get ready, get there and come back is kept free around events and tasks, never counted as a pause.
- **Meals, rest and sleep first.** You set your meals (how many, when, how long, the time to get them ready), naps, bedtime, waking and the time to wind down. Their times are kept free; the work is planned in what is left. A heads-up a quarter of an hour before ("No new big task"), then one notice at the time, each once. **15 min later**, **Move to…** a time, or **Not today**, today only, without a word asked. A meal that would fall in an event moves after it. Nothing about food, portions or numbers is ever recorded or shown: no "missed", no praise. Sioul does not treat anything; it plans around the needs you set: [meals, rest and sleep](https://aurelienpierre.github.io/sioul/guide/health.html#meals-rest-and-sleep).
- **Medicines**, reminded at their times, on the device you are at; a dose marked on one device is known on the others, and when Sioul cannot know, it says "check first" rather than "not taken".
- **Working for yourself, from the client's first mail to the paid invoice.** A project per client, whose mail comes to it by itself; time counted while you work (the focus timer, with its own notification to pause or stop, or `1h30` noted after a call), also kept to learn how long things really take; what is left to bill always in view; the invoice in one click, numbered without gaps, with the mentions French law asks for; the money expected in your budget until it is paid; a spreadsheet for your accountant. No time tracker, invoicing service or subscription beside it: [working for clients](https://aurelienpierre.github.io/sioul/guide/clients.html).
- **Everything in one window, tied together**: mail, agenda, contacts, notes, projects, time and invoices, budgets and bank accounts, papers and scanned letters, medicines, and the websites you have to check (banks, offices, chats), logged in once.
- **Nothing shouts**: no unread counters, no badges, no red, no sounds, no streaks. "Undo" waits ten seconds after anything is moved, deleted or sent.
- **Notes that stay yours**: a folder of Markdown files, fully compatible with an Obsidian vault and with Nextcloud Notes. Use the same folder in all three, side by side: wikilinks, embeds, tags, front matter and aliases as Obsidian reads them; Nextcloud Notes' `.txt` or `.md` notes and its categories as they are; nothing imported or converted. Sioul ties notes to tasks, mail and events with plain Markdown links: [notes](https://aurelienpierre.github.io/sioul/guide/notes.html#the-same-folder-as-obsidian-and-nextcloud-notes).
- **Yours**: it runs on your computer; your data stays in plain files and in your own accounts. There is no server of ours.

## Where it stands

Version 0.0.1: Sioul is young and changes often; it is used every day. Packages for Windows, macOS (Apple silicon and Intel) and Linux (AppImage and Flatpak) are on [the releases page](https://github.com/aurelienpierre/sioul/releases/latest): built and tested by GitHub, used daily on Linux, little tried elsewhere yet. An Android version is being tried on a phone, with the doses reminded and the sharing working ([docs/android.md](docs/android.md)); it is not ready to install.

It is made by one person, in the open: no support is promised. Questions and reports are welcome in [GitHub issues](https://github.com/aurelienpierre/sioul/issues).

## Building

It needs Rust 1.89 or newer, a C++ compiler, the D-Bus development files, and, for the window, Qt 6.9 or newer with Qt Declarative, Qt WebEngine (with Qt WebChannel and Qt PDF), Qt Multimedia, Qt Positioning, Qt Location and Qt Image Formats. The packages for each system, and building on Windows and macOS: [Install](https://aurelienpierre.github.io/sioul/guide/install.html), and [docs/building.md](docs/building.md).

```
cargo build --release                 # sioul, the command line (no Qt needed)
cargo build --release -p sioul-app    # sioul-app, the window
```

To find it in the application menu:

```
install -Dm755 target/release/sioul-app ~/.local/bin/sioul-app
install -Dm755 target/release/sioul ~/.local/bin/sioul
install -Dm644 data/com.aurelienpierre.Sioul.desktop ~/.local/share/applications/com.aurelienpierre.Sioul.desktop
install -Dm644 data/com.aurelienpierre.Sioul.metainfo.xml ~/.local/share/metainfo/com.aurelienpierre.Sioul.metainfo.xml
mkdir -p ~/.local/share/icons && cp -r data/icons/hicolor ~/.local/share/icons/
```

Try it on invented mail first:

```
cargo run -q -- --config examples/demo.toml porch
cargo run -q -- --config examples/demo.toml card crates/sioul-core/tests/fixtures/porch/03-forged.eml
cargo run -q -- --config examples/demo.toml --language fr budgets --maildir crates/sioul-core/tests/fixtures/money
```

Then the first steps with your own accounts: [First steps](https://aurelienpierre.github.io/sioul/guide/first-steps.html).

## Documentation

- **The user guide**, for using Sioul: [aurelienpierre.github.io/sioul](https://aurelienpierre.github.io/sioul/).
- **The design notes**, for working on it, in [docs/](docs/): the [design](docs/design.md), [what the research says](docs/research.md), the [architecture](docs/architecture.md), [building](docs/building.md), the [roadmap](docs/roadmap.md), and a note for each part. They are also on the website, under [For developers](https://aurelienpierre.github.io/sioul/dev/index.html).
- **The website itself** is in [website/](website/): `website/build.sh` builds it with Zensical.

## For technical readers

- **Every message checked on arrival**: SPF, DKIM, DMARC, ARC and reverse DNS, by Sioul itself. Forged mail is set aside with the reason; names borrowed from brands are caught, even written with look-alike letters.
- **Attachments scanned before they open**, by your system's antivirus: ClamAV on Linux and macOS, Microsoft Defender (through AMSI) on Windows.
- **HTML mail made safe**: nothing remote loads, nothing runs.
- **Security keys and Bitwarden**: WebAuthn and FIDO2 keys (a YubiKey) in the sites you keep; logins filled from Bitwarden, read by Sioul itself and never written.
- **OpenPGP**: signing and encrypting as you send, with Autocrypt and the Web Key Directory.
- **Sharing between your devices**, a phone included: each device keeps its own data; a folder that any sync app carries (Nextcloud, Dropbox, Syncthing, Google Drive, OneDrive…) passes changes between them, each device writing only its own file, sealed end to end (XChaCha20-Poly1305, the key made from your passphrase by Argon2id); notes and papers file by file; earlier versions kept on each device. No server of ours. [How it works, and what it protects](https://aurelienpierre.github.io/sioul/guide/sharing.html); the design: [docs/database.md](docs/database.md).
- **AI agents**, only if you connect one: `sioul mcp` serves an agent such as Claude Code what Sioul keeps on this computer, through the Model Context Protocol. It never sends, deletes or pays.
- **Open standards and plain files**: IMAP, SMTP, CalDAV and CardDAV, tasks linked as RFC 9253 says, Maildir, TOML, and notes in Markdown, compatible with Obsidian vaults and Nextcloud Notes.
- **Rust**, with a **Qt 6** window in QML through CXX-Qt.

## Licence

GPL-3.0-or-later: see [LICENSE](LICENSE). Copyright © 2026 Aurélien Pierre.
