---
title: A calm place for mail, tasks and admin
description: Sioul gathers your mail, tasks, agenda, money and papers in one quiet window, on your own computer. Designed for autistic, ADHD, burnt-out and exhausted people, from research on what helps them.
---

# Sioul

*Sioul* [siwl] is Breton for calm, peaceful, silent.

Sioul is a desktop application that gathers your mail, your tasks and your admin in one quiet window, on your own computer.

It is designed for people for whom admin hurts: autistic people, people with ADHD, people who are anxious, traumatised, burnt out, depressed, or simply exhausted.

<figure markdown="span">
  [![Sioul's window. On the left, the list of places: Porch, Tasks, Mail, Sites, Agenda, Contacts, Notes, Projects, Time, Budgets, Papers, Health. On the right, the Porch: this week's payments in one line, a one-time code with its Copy button, a chat's news, then the new mail sorted into lanes.](assets/screens/porch.png){ loading=lazy }](assets/screens/porch.png "Open the picture at full size")
  <figcaption>The Porch, in working hours: codes on top, then the new mail, sorted into lanes.</figcaption>
</figure>

## Why it exists

Many neurodivergent people choose to work for themselves. Office life does not fit them: the open-plan office, the noise, the constant interruptions.

Working for yourself brings more admin than a salaried job: quotes and invoices, taxes, social contributions, clients, the bank. And admin is exactly what ADHD, trauma, depression and burnout make hard.

It is not laziness. Putting off a letter protects your mood for now (Sirois & Pychyl 2013). Administrative burden weighs most on people with the fewest resources left, executive function and health among them (Christensen et al. 2020).

Meanwhile, modern life scatters everything:

- mail arrives at several addresses;
- banks, the tax office and health insurers keep their letters in "secure mailboxes" on their own websites, and only send a mail saying that something is waiting there;
- chats live in the browser;
- tasks, appointments, medicines, paper letters and the papers asked again and again (an identity card, the last tax notice, rent receipts) each live somewhere else.

Most software is built to make you answer faster: counts, red badges, sounds, streaks. Sioul starts from the other end. It is designed from the well-being of the person towards the demands of work, admin and money, not to fit the latest management method.

## What it does

<div class="grid cards" markdown>

-   :lucide-inbox:{ .lg .middle } __Mail waits on a porch__

    ---

    New mail is checked (genuine or forged), sorted, and shown in the hours you chose. The codes and links you just asked a site for come at once, quietly.

    [The Porch](guide/porch.md)

-   :lucide-list-checks:{ .lg .middle } __One next step__

    ---

    Tasks that wait for each other are ordered into the one step to take now, with its reason. Nothing is ever overdue. Starting is helped, and stopping counts.

    [Tasks](guide/tasks.md)

-   :lucide-sunrise:{ .lg .middle } __Hours set in advance__

    ---

    Working hours, hours for your own admin, free time. Each address, site, budget and task belongs to one or several of them. The rest waits, out of sight.

    [Hours](guide/hours.md)

-   :lucide-folder-open:{ .lg .middle } __Everything in one place__

    ---

    Mail, agenda, contacts, notes, projects, time and invoices, budgets and bank accounts, papers and paper letters, medicines, and the websites you have to check.

    [The window](guide/first-steps.md#the-window)

-   :lucide-moon:{ .lg .middle } __Nothing shouts__

    ---

    No unread counters, no badges, no red, no sounds, no streaks. Nothing moves under your eyes. "Undo" waits ten seconds after anything is moved, deleted or sent.

    [Privacy and security](guide/privacy-security.md)

-   :lucide-house:{ .lg .middle } __Yours__

    ---

    It runs on your computer. Your data stays in plain files and in your own accounts. There is no server of ours, and nothing goes to the developer.

    [Privacy policy](privacy.md)

</div>

Your Google calendars, contacts and tasks can come too. If you sign in with your Google account, Sioul reads and writes them, only to show them beside the rest of your admin and to save the changes you make there, and keeps a copy on your computer. Nothing goes to the developer. What it reads, where it keeps it, and how to take the access back: [privacy policy](privacy.md#google-calendars-contacts-and-tasks).

## Built on research

Each choice in Sioul follows what studies found about attention, stress, avoidance and recovery, several of them with autistic or ADHD people. A few of them:

- **Fewer, predictable looks at mail.** Checking mail three times a day lowered daily stress in a randomised trial (Kushlev & Dunn 2015). Batching notifications three times a day improved attention and mood, while people given no notifications at all felt *more* anxious (Fitz et al. 2019). So Sioul shows mail in hours you choose, and always says when they come.
- **Interruptions cost, even when ignored.** A notification left unanswered still disrupts attention (Stothart, Mitchum & Yehnert 2015). So there are no pop-ups, sounds or badges, and the layout never moves under your eyes.
- **Starting is the hard part.** Autistic inertia is a difficulty acting on intentions, eased by outside scaffolding (Buckle et al. 2021). With ADHD, help belongs at the point of performance (Barkley 2012). So Sioul picks one small next step, and says why.
- **Progress, never streaks.** Broken streaks lower later engagement, more so when people blame themselves (Silverman & Barasch 2023). So Sioul shows what got done, and never counts what did not.
- **Evenings are for recovery.** Detaching from work after hours goes with less exhaustion (Wendsche & Lohmann-Haislah 2017), and merely expecting work mail in the evening does harm (Becker et al. 2021). So work rests outside your working hours.

All the findings, with their sources and the rule each one gives: [what the research says](dev/research.md). Each study in detail, with how strong its evidence is: [the research notes](dev/research/README.md).

## What it looks like

<div class="grid" markdown>

<figure markdown="span">
  [![The Tasks page on "Now": one step, the reason it comes now, and buttons to start it, mark it done, or put it off until tomorrow.](assets/screens/tasks-now.png){ loading=lazy }](assets/screens/tasks-now.png "Open the picture at full size")
  <figcaption>Tasks: one next step, and why.</figcaption>
</figure>

<figure markdown="span">
  [![The Sites page: the sites for these hours listed on the left (a chat, with a dot for its news, and video calls), then those of other hours (a bank, the tax office, a health insurer), shown muted, and Pin a site and Usual sites below.](assets/screens/sites.png){ loading=lazy }](assets/screens/sites.png "Open the picture at full size")
  <figcaption>Sites: secure mailboxes and chats, logged in once.</figcaption>
</figure>

<figure markdown="span">
  [![The Budgets page: three budgets (work, household duties, leisure), each a card with "On track" on its own line, how much of the month is gone, then one figure per line.](assets/screens/budgets.png){ loading=lazy }](assets/screens/budgets.png "Open the picture at full size")
  <figcaption>Budgets: on track or not, said in words.</figcaption>
</figure>

<figure markdown="span">
  [![The Hours tab of the Settings page: working hours, then hours for your admin, each day of the week ticked or not, from a start to an end.](assets/screens/settings-hours.png){ loading=lazy }](assets/screens/settings-hours.png "Open the picture at full size")
  <figcaption>Hours: work, your admin, free time.</figcaption>
</figure>

</div>

## Where to start

1. [Install Sioul](guide/install.md). For now it is built from its sources, on Linux first.
2. [Take the first steps](guide/first-steps.md): add your mail, your calendars and contacts, your Google account, the websites you check.
3. [Set your hours](guide/hours.md), so that work, admin and rest each have their time.
4. Then read about [the Porch](guide/porch.md), where new mail waits.

## Where it stands

Sioul is young (version 0.0.1) and changes often; it is used every day. It runs on Linux. Windows and macOS versions are built by a workflow on GitHub, but have not been tried yet. There are no packages yet.

It is made by one person, in the open: no support is promised. Questions and reports are welcome in [GitHub issues](https://github.com/aurelienpierre/sioul/issues).

## For technical readers

- **Every message checked on arrival**: SPF, DKIM, DMARC, ARC and reverse DNS, by Sioul itself. Forged mail is set aside with the reason. Names borrowed from brands are caught, even when written with look-alike letters.
- **Attachments scanned before they open**, by your system's antivirus: ClamAV on Linux and macOS, Microsoft Defender (through AMSI) on Windows.
- **HTML mail made safe**: nothing remote loads, nothing runs.
- **Security keys and Bitwarden**: WebAuthn and FIDO2 keys (a YubiKey) work in the sites you keep. Logins are filled from Bitwarden, read by Sioul itself and never written.
- **OpenPGP**: signing and encrypting as you send, with Autocrypt and the Web Key Directory.
- **Sharing between your computers** through a Nextcloud, Dropbox, Syncthing or any synced folder, end-to-end encrypted (XChaCha20-Poly1305, the key made from your passphrase by Argon2id). No server of ours.
- **Open standards and plain files**: IMAP, SMTP, CalDAV and CardDAV, tasks linked as RFC 9253 says, Maildir, Markdown, TOML.
- **Free software**, under the GPL-3.0-or-later licence, written in Rust, with a Qt 6 window.

<p class="sioul-quiet" markdown>For developers: [the design notes, the architecture, and how to build and test](dev/index.md).</p>
