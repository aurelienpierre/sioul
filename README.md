# Sioul

<img src="data/icons/sioul.svg" width="96" height="96" alt="Sioul's icon: a quill with a pen nib, cream on green.">

*Sioul* [siwl] is Breton for calm, peaceful, silent.

Sioul is a desktop application that gathers your mail, your tasks and your admin in one quiet window, on your own computer. It is designed for people for whom admin hurts: autistic people, people with ADHD, people who are anxious, traumatised, burnt out, depressed, or simply exhausted.

**Website and user guide: [aurelienpierre.github.io/sioul](https://aurelienpierre.github.io/sioul/)**

![Sioul's window: the list of places on the left (Porch, Tasks, Mail, Sites, Agenda, Contacts, Notes, Projects, Time, Budgets, Papers, Health); on the right, the Porch, with a one-time code and its Copy button on top, a chat's news, then the new mail sorted into lanes.](website/docs/assets/screens/porch.png)

## Why

Many neurodivergent people work for themselves, because office life does not fit them. Working for yourself brings more admin, and admin is exactly what ADHD, trauma, depression and burnout make hard. Meanwhile, everything is scattered: mail at several addresses, the "secure mailboxes" of banks, the tax office and health insurers on their own websites, chats, tasks, appointments, medicines, paper letters, and the papers asked again and again.

Most software is built to make you answer faster. Sioul is designed the other way round: from the well-being of the person towards the demands of work, admin and money.

## What it does

- **Mail waits on a porch.** It is checked (genuine or forged), sorted into lanes, and shown in the hours you chose. The codes and links you just asked a site for come at once, quietly.
- **Hours set in advance**: working hours, hours for your own admin, free time. Each address, site, budget and task belongs to one or several of them; the rest waits, out of sight. Outside working hours, work rests.
- **One next step.** Tasks that wait for each other are ordered into the one step to take now, with its reason, placed in the hours meant for it. Nothing is ever overdue. Starting is helped; stopping counts.
- **Working for yourself, from the client's first mail to the paid invoice.** A project per client, whose mail comes to it by itself; time counted while you work (the focus timer, or `1h30` noted after a call); what is left to bill always in view; the invoice in one click, numbered without gaps, with the mentions French law asks for; the money expected in your budget until it is paid; a spreadsheet for your accountant. No time tracker, invoicing service or subscription beside it: [working for clients](https://aurelienpierre.github.io/sioul/guide/clients.html).
- **Everything in one window**: mail, agenda, contacts, notes, projects, time and invoices, budgets and bank accounts, papers and scanned letters, medicines, and the websites you have to check (banks, offices, chats), logged in once.
- **Nothing shouts**: no unread counters, no badges, no red, no sounds, no streaks. "Undo" waits ten seconds after anything is moved, deleted or sent.
- **Yours**: it runs on your computer; your data stays in plain files and in your own accounts. There is no server of ours.

Each choice follows research on attention, stress, avoidance and recovery, much of it with autistic or ADHD people: [what the research says](docs/research.md), and [the research notes](docs/research/README.md) in detail.

## Where it stands

Version 0.0.1: Sioul is young and changes often; it is used every day. Packages for Windows, macOS (Apple silicon and Intel) and Linux (AppImage and Flatpak) are on [the releases page](https://github.com/aurelienpierre/sioul/releases/latest): built and tested by GitHub, used daily on Linux, little tried elsewhere yet. An Android version is being tried ([docs/android.md](docs/android.md)); it is not ready.

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
- **Sharing between your computers** through a Nextcloud, Dropbox, Syncthing or any synced folder, end-to-end encrypted (XChaCha20-Poly1305, the key made from your passphrase by Argon2id). No server of ours.
- **AI agents**, only if you connect one: `sioul mcp` serves an agent such as Claude Code what Sioul keeps on this computer, through the Model Context Protocol. It never sends, deletes or pays.
- **Open standards and plain files**: IMAP, SMTP, CalDAV and CardDAV, tasks linked as RFC 9253 says, Maildir, Markdown, TOML.
- **Rust**, with a **Qt 6** window in QML through CXX-Qt.

## Licence

GPL-3.0-or-later: see [LICENSE](LICENSE). Copyright © 2026 Aurélien Pierre.
