# Design

Sioul adapts the demands of the world to the person, rather than the person to a model of productivity. Usual software goes from obligations to a schedule, and leaves the person whatever time remains; Sioul goes the other way: from the person's needs and what today can hold (meals, rest, sleep, hours, the margins around events, a day said clear, hazy or foggy), to the time kept for them, then to the obligations that fit. It shows what belongs in the person's attention now rather than what has arrived, and it keeps the ties between things (the case is the unit, not the program) so that the person is no longer the glue between them.

It is designed first for people for whom admin hurts, and for anyone whose capacity is limited or changes. Its rule: **nothing enters without your consent; letters wait outside; deadlines live in the plan, not in your face; a written channel is always open.** Every choice below comes from that rule, and from the research in [research.md](research.md). The detailed notes behind it, each study with its evidence and the rule it gives, are in [research/README.md](research/README.md).

Three tests for anything added:
- **It takes admin work off the person**, rather than moving it elsewhere. An assistant that lists seventeen things to deal with has moved the work; one that settles sixteen and asks one question has removed it.
- **Its complexity stays on Sioul's side.** More state, more syncing, more ways to fail are Sioul's to absorb; none of it reaches the screen as a setting to understand or a state to watch.
- **It is something software may do to a person.** The refusals of [research/life-admin.md](research/life-admin.md) (streaks, overdue counts, nags, mood or capacity guessed from behaviour, actions taken alone) hold for every new feature.

## Six places
- **The Porch**: everything new, from every account and portal, waits here until an admin window. It is checked (genuine or forged), sorted into cases and summarised. New senders wait in the screener until you let them in.
- **Cases**: one page per case (a tax return, a health-cover request, a bill), with its mail, portal letters, tasks, events, contacts, notes and documents, a timeline, and what comes next. The detailed record lives in your own Markdown files ([case-store.md](case-store.md)).
- **Next**: the one next step, this week's admin windows, and the Gantt chart on demand.
- **Outbox**: drafts, and what each one waits for. Sending is a deliberate key, never automatic.
- **Portals**, the Sites page now: the web-only mailboxes of banks, tax offices, health insurers, and Proton without Bridge, each logged in once and kept so, in one persistent browser profile for every site ([sites.md](sites.md)).
- **Calendar and people**: calendars, contacts, the map.

## Admin windows
- **Defaults**: the Porch shows each address's mail in the hours that address is for: your working hours, your hours for your own admin, and leisure every other time; with no hours set at all, everything comes ([areas.md](areas.md)). Outside them the Porch says when it opens next. Mail keeps syncing in the background.
- **The exception: short-lived secrets.** One-time codes, passwords, password resets, sign-in links and addresses to confirm come from a request you just made, and expire within minutes or hours.
  - Sioul shows them at once, from automatic addresses (no-reply) too: one quiet notification, no sound, the code with a key to copy it, and how long it stays valid. The message does not open the rest of the Porch. After expiry the code is hidden.
  - Fake "your code" messages are a phishing trick: a forged one is set aside, and one from a sender that is not verified comes with a warning to use it only if you just asked that site for it.
- **What you send yourself**, from one of your addresses to another (a file from your phone), comes in any hour, never screened, when it is verified: a forged own address is a classic trick.
- **Optional exception**: a short list of people who may reach you anytime. Empty by default.
- **Opening**: a few sentences, then one card at a time. Small numbers are written as words; there are no badges.
- **Closing**: "Done for today". It shows what got done, and says "nothing else needs you before Friday" when that is true.

## Safe opening
Before the message itself, each card shows:
- **who sent it**: verified, not verified, or forged, with the reason;
- **the case** it belongs to;
- **a plain-language line** about what it is;
- **what it asks**, by when (with any legal deadline extracted), and what happens if nothing is done;
- **a suggested next step**.

From the card, one key does each of:
- **the full text**;
- **"open with the companion"**: an AI explains the message ([ai.md](ai.md));
- **"hand to my helper"**: forward it with the summary to someone who helps you with admin;
- **"set aside"**: back to the Porch, unread.

## Trust
- **Authentication**:
  - DKIM verified at arrival, and the result stored (keys rotate);
  - SPF and DMARC evaluated on the address your own provider received the message from (the trust boundary in the `Received` chain);
  - ARC for forwarded mail;
  - the provider's `Authentication-Results`, trusted only under its own server id (RFC 8601).
  - Sioul checks them itself, with Stalwart's `mail-auth`, as each message arrives (`crates/sioul-sync/src/verify.rs`); the provider's results count only under its own id.
- **Lookalike senders**:
  - a registry of genuine domains, learned from authenticated mail you confirmed, plus a shared starter list for public services and banks;
  - Unicode confusables (UTS #39) and near-spellings.
- **Reputation**: Spamhaus for the sending address, through its free Data Query Service key (its public mirrors refuse queries made through public DNS resolvers), and domain blocklists for the links.
- **The provider's spam verdicts**: SpamAssassin's `X-Spam-*`, rspamd's `X-Spamd-Result`.
- **Your own spam filter**, trained on your computer from your own mail (your Junk folders, your "junk / not junk" and "Not spam"): a linear model on its words and on facts read from its headers (fastText's classifier), folded into a small table every device reads; never for people you know, codes or projects. The window says only that your own filter judged a message, not why, by the author's decision; `sioul spam why` prints what weighed, in a terminal ([spam-filter.md](spam-filter.md)).
- **Newsletters**: one-click unsubscribe (RFC 8058).

## Portals
- **An embedded Chromium** (QtWebEngine), one persistent profile for every site, with the sessions kept ([sites.md](sites.md)).
- **The notification emails** ("you have a new message in your space") become "a letter waits at <portal>" in the right case.
- **"File this page"** saves a portal letter as a dated PDF into its case.
- **No passwords stored by Sioul**: a password manager or the system keyring. No scraping.

## Tasks, the next step, the Gantt chart
As built, with the research it follows: [tasks.md](tasks.md).

- **Tasks are CalDAV tasks (VTODO)** on your server, so they are portable and visible on other devices.
- **The links use RFC 9253** (iCalendar relationships):
  - `RELATED-TO;RELTYPE=FINISHTOSTART` for "this waits for that", with `GAP` for "two weeks after";
  - `DEPENDS-ON` for looser dependencies;
  - `REFID` groups a case's tasks and events;
  - `LINK` points to emails (`mid:` URIs, RFC 2392), contacts and notes.
- **The solver**:
  - it orders tasks topologically (Kahn's algorithm), and shows any loop calmly instead of hiding it;
  - among the tasks free to start, it picks the one started, then the one whose latest start comes first (the date asked minus the chain behind it), then your priority, then the one that frees the most others, then the smaller ([tasks.md](tasks.md), "The plan");
  - it places tasks into the hours of their kind (working hours for work, your hours for your own admin), each day filled to what it holds for you ([capacity.md](capacity.md)). Near a date asked, it says how much work and how much room are left before it, and when the work does not fit, it asks: "More time asked, or a lighter plan?"
- **Waiting on others**: a task can wait for an incoming letter. When the letter arrives in its case, the task unblocks.
- **The Gantt chart** is computed from durations and links: soft bars, a today line, deadlines as marks, slack as faint extensions, all driven by keyboard.

## Calendar
- **CalDAV**, and invitations by mail (iTIP/iMIP): accept or decline from the card.
- **`.ics` attachments** open as a preview with any conflicts.
- **Dates found in letters** become proposed deadlines.
- **An event can become an email.**
- **Reminders** before events, dates asked and payments: each said once, by one quiet notification, with the window closed too ([reminders.md](reminders.md)).

## Contacts and the map
- **CardDAV, vCard 4.**
- **Deduplication**: normalise (case, accents, phone numbers in international format, addresses), pair candidates (a shared email or phone is certain; close names are a maybe), merge field by field, with undo and history.
- **The map**: OpenStreetMap, geocoded with Nominatim at one request a second, with a cache. Tiles offline or from OpenFreeMap: the OpenStreetMap tile servers refuse heavy use.

## Notes and drafts
- **Each case has Markdown files** with front matter (`refid`, links), readable in any editor, Obsidian included; the folder can be a git repository of your own (a note's history through git is next: [tasks.md](tasks.md)).
- **Drafts are Markdown files** whose front matter holds `to`, `cc`, `subject`, `account`, `attach`, `waits-for` and `case`.

## The status line
One sentence about what happened last, with "Undo" for ten seconds; in quiet time, when work comes back, and the way back behind a click ([porch.md](porch.md)); the work day or the day offered to close at its time ([reviews.md](reviews.md)); do-not-disturb ([do-not-disturb.md](do-not-disturb.md)); the sound button ([sounds.md](sounds.md)); the weather at a place you chose, in one colour: now and the next two hours in the line, the next four on a click, then the parts of the days to come (this evening, tonight, tomorrow morning…), from Open-Meteo, credited there, the place's coordinates (two decimals) the only thing sent, every half hour at most; at its end, Free time and Pause, always in the same place ([pauses.md](pauses.md)). Health has its own place ([health.md](health.md)).

On a computer, the status line is the window's title bar, across the whole width at the top: a system's title bar only took a line of height to say "Sioul", and the status line, at the bottom beside the places, was not the window's width. At its left end, over the places, the button that shows their names or keeps their icons only (F9); the window's buttons, minimize, maximize or restore, and close, on the side and in the order the system puts them: KDE Plasma's kwinrc (`[org.kde.kdecoration2]` `ButtonsOnLeft`, `ButtonsOnRight`), GNOME's `button-layout` (Cinnamon's and MATE's own first, Budgie, Unity and Pantheon reading GNOME's), Xfce's xfwm4 `button_layout`; on the left on macOS, on the right on Windows and elsewhere (`src/desktop.rs`). Dragged on its free space, the sentence included, it moves the window, the system doing the moving (snapping, tiling); a double click maximizes or restores; the window's edges resize it; a one-pixel line in the theme's colour marks them while the window is neither maximized nor full screen. Full screen (F11) keeps it: it is the status line. While a pause covers the window, only the window's buttons stay (`qml/TitleBar.qml`, `qml/WindowEdges.qml`). On a phone, the status line stays at the bottom.

Narrow (a phone, a window as narrow), every button of the line is its icon alone, as wide as the icon and an even margin on each side, its name said to screen readers and at a long press (do-not-disturb's and Free time's long press open their menu); the sentence takes the rest of the line, and when it is cut short, a tap shows it whole, as the pointer resting on it does on a computer (`qml/StatusLine.qml`, `qml/LineButton.qml`).

## Keyboard and senses
- **Everything by keyboard**: one key per action, Escape always goes back, key hints on screen, focus always visible, screen readers through Qt's accessibility layer. A command palette and an editable keymap: not built.
- **The places as icons, their names when wanted**: on the left of the window, a narrow column of icons, one per place, in a fixed order; the place shown is marked by a soft tint of the accent and a short bar, never a badge or a count. Icons alone can be ambiguous, and some people read words more easily ([research.md](research.md), rule 10): each icon's name and key show when the pointer rests on it, when the keyboard reaches it and at a long press on a touch screen, screen readers read them, and Settings ▸ Display ▸ "Show the places' names beside their icons" widens the column to icons with their names. The button at the title bar's left end, over the column (at the column's foot on a tablet, level with the status line there), or F9, switches between the icons alone and the icons with their names, the same setting; the column never hides, so a place stays one click away. Apart at the bottom, two buttons close the work day and the whole day at any hour ([reviews.md](reviews.md)); under them the accounts, the settings and refresh, their icons alone, on one row when the names show. On a phone, the places come over the page from the left with their names (`qml/Places.qml`, `qml/RailButton.qml`).
- **The window, on a computer**: it opens maximized, with its own title bar, the status line (above); F11 shows it full screen and back. An icon in the system tray shows or hides it; closing the window hides it there while Sioul goes on (mail, reminders, medicines, the sharing), quitting being the icon's menu or Ctrl+Q. Without a tray, closing quits. Other windows closing (a draft, the focus window) never quit Sioul, even while its window is in the tray. Plasma's tray menu is made of widgets: on a computer, Sioul is a widgets application (`cpp/application.cpp`, Qt Widgets linked there only); a phone has no tray and its build carries no Qt Widgets (`qml-desktop/Tray.qml`, Qt.labs.platform, kept out of the phone's build).
- **Passwords seen when needed**: every password, passphrase and key field has an eye at its end (reached with Tab, named for screen readers) that shows or hides what is typed; Settings ▸ "Show passwords as you type" shows them from the start, on this device (`qml/PasswordField.qml`).
- **Senses**:
  - a muted palette with even lightness steps (OKLab), no pure white or black;
  - no red for lateness; forged mail is marked by a shape and a word, not an alarm colour;
  - no sounds, badges or counts; the system's setting to reduce motion is not read yet;
  - the layout never reflows when mail arrives;
  - light and dark themes; for long text (notes, mail, a task's notes), the font, its size and the line spacing are yours to choose, a font made for dyslexia among them when it is installed; Sioul bundles none.
- **Language**: plain and literal, no idioms. "Waiting", "time left: four weeks", never a red "overdue". Every sentence is translated ([i18n.md](i18n.md)).
- **Low-energy days**: a filter that shows only the small tasks, or "nothing is needed today" when that is true.

## Prior art: what Sioul takes and avoids

Mail clients, task tools and one research prototype each solved part of what Sioul needs, and each also showed what to avoid. Each entry says what Sioul takes, what it avoids or does differently, and where that stands in Sioul now. The products' claims were checked at their own pages on 8 October 2026; the sources close the section. Comparisons of features, for readers, belong to the user guide.

- **Thunderbird** sets an account up from the address alone: the provider's own configuration, else Mozilla's database of providers (the ISPDB), else a guess. Sioul takes the same order, over HTTPS only, tells the ISPDB the domain and nothing else, and says a guess is a guess ([porch.md](porch.md), "Done"; [compatibility.md](compatibility.md)). It also takes invitations answered from the message that brings them (iMIP). It avoids what Thunderbird does by default: an alert for each new message with its sender, subject and first words, a sound on Windows and Linux, unread counts in the folder list and, on Windows, a count on the taskbar. Sioul gathers new mail into one quiet notification at the times the person allows, its number said in words, and marks a folder with something new by a small dot, never a number ([porch.md](porch.md), "Notifications"; [client.md](client.md)).
- **HEY** (37signals) screens new senders: the first message from a stranger waits until the person decides whether to hear from them again. Sioul's screener does the same ([porch.md](porch.md)) on the addresses the person already has; HEY is a closed, paid service, used through its own apps, which fetches no other account by IMAP and which no other client can read. HEY also shelves receipts (the Paper Trail) and messages to answer (Reply Later) in piles of their own. Sioul folds automatic mail into one lane, reads payments out of it as proposed budget lines ([accounting.md](accounting.md)), keeps documents in the papers wallet ([papers.md](papers.md)), and turns a message to answer into a task that the plan places in the person's hours ([tasks.md](tasks.md)): a pile still has to be looked through, while a step has its time.
- **Superhuman Mail**, which Grammarly bought in 2025 before taking the name Superhuman for the whole company, is driven from the keyboard: every action has a key, and Cmd+K opens a command palette that also teaches the key. Sioul takes the keyboard: each place has its key, shown with its name when the pointer or the keyboard reaches its icon, writing and replying have theirs, and Escape closes what is open. The command palette and the editable keymap listed in "Keyboard and senses" are not built. Sioul avoids what Superhuman's keyboard serves: speed as the value ("twice as fast"), and Inbox Zero as the goal, rewarded with images when reached. Sioul's "Done for now" closes the Porch on what it showed, and closing the day says "nothing else needs you before Friday" when that is true. Superhuman Mail's AI may send on its user's behalf, and its users can see when their recipients read a message; in Sioul no agent sends ([ai.md](ai.md)), and a message shown loads nothing remote, so that no sender learns from it when it was read ([porch.md](porch.md), "Reading a message").
- **aerc, mutt, notmuch and Himalaya** work with mail kept in plain files and give scripts a way in. notmuch indexes and tags mail for search, from a library and a command line, and neither fetches nor sends; Himalaya is a command line of stateless commands, with JSON for scripts; aerc and mutt are clients run in a terminal and driven by keys. Sioul takes the files and the command line: mail in Maildir, which notmuch and mutt can read, and `sioul`, a command line for scripts and agents ([architecture.md](architecture.md)). It avoids the terminal as the only way in, and keys learnt by heart. Tags on mail are not built: a message is tied to its case instead ([case-store.md](case-store.md)).
- **Kontact (with Akonadi) and Evolution** put mail, calendars, contacts and tasks side by side. Sioul takes the integration, in one window. It avoids the layer they put between programs and data: Akonadi is a desktop-wide server whose database (MySQL by default) holds copies of the data and starts with any program that uses it; Evolution's data server is a single database for contacts, calendars, tasks and notes. Sioul keeps plain files that other programs read as they are (Maildir, vdir, tasks with their links inside, Markdown), and writes each link into the thing that makes it ([architecture.md](architecture.md), "Storage"; [tasks.md](tasks.md), "In the standards").
- **Tiimo**, Apple's iPhone App of the Year 2025, is a visual planner built, its makers say, by and for neurodivergent people, each feature shaped with users, clinicians, educators and researchers; no outcome study of it was found ([research/capacity-budget.md](research/capacity-budget.md)). Sioul takes the day seen as a whole (Tasks ▸ The day), and a timer that drains in a neutral colour and warns before the end ([tasks.md](tasks.md)); the AI that breaks a task into steps is not built. It avoids Tiimo's streaks, trophies and mood tracking, which Sioul refuses on the research's evidence ([research/life-admin.md](research/life-admin.md)), and a planner that knows nothing of the mail and letters that bring the work: in Sioul a task can be made from the message or the scanned letter that asks for it, tied to it ([tasks.md](tasks.md); [porch.md](porch.md), "Paper letters").
- **Goblin Tools** breaks a task into steps with one button, the number of steps set by how hard the task feels (its "spiciness"), and says plainly that its answers are "only guesswork". Sioul takes both. The split at a chosen granularity, its lines kept as ordinary steps of the task, is not built: steps are added by hand. An AI's work stays a proposal: a draft written by a connected agent waits in Drafts until the person sends it ([mcp.md](mcp.md)). It avoids lists that stand apart: Goblin's lists now sync between devices and leave by export, but none is tied to the mail or the case it came from. In Sioul a step is a task in the person's own task list, under its bigger task.
- **Taskwarrior** lets a task depend on others, and ranks a task that blocks others higher and one that is blocked lower. Sioul takes the dependencies, written into the task as RFC 9253 says ([tasks.md](tasks.md), "In the standards"), and counts what a step frees when it chooses the next one. It avoids the single score: Taskwarrior's urgency is a weighted sum in which an overdue date also weighs. Sioul chooses in a fixed order (the step started, then the latest start, then the person's priority, then what it frees, then the smaller), says the reason in words, and has no overdue state ([tasks.md](tasks.md), "The plan").
- **TaskJuggler** schedules a project against the capacity of its resources: working hours, leave, shifts, levelling. Sioul takes capacity-aware scheduling for one person: each task goes into the first days with room, in the person's own hours of its kind and around events, and each day is filled to 85 % of what it holds, learned from the days the person described ([tasks.md](tasks.md), "The plan"; [capacity.md](capacity.md)). It avoids the project office: TaskJuggler's plans are written in a description language, for "serious project managers"; Sioul's tasks come from a message, a line or a form, and its plan proposes days without writing them into the tasks.
- **Vikunja** is open-source task management with relations between tasks (subtask, blocking, precedes) and a Gantt chart. Sioul takes both: waits between tasks, and the Timeline, a Gantt chart shown on demand ([tasks.md](tasks.md), "Places"). It avoids relations that only its own server knows: Vikunja's CalDAV, still in alpha, carries only parent and child, so its blocking relations stay in its own database; and Vikunja reads no mail. Sioul writes every wait into the task, on the person's own CalDAV server, where other clients keep it. Vikunja also highlights overdue tasks in every view; Sioul marks the date asked with a small diamond and never says overdue.
- **Nextcloud Tasks** stores a subtask as a `RELATED-TO` line in the subtask, pointing to its parent, and changes only that line, "so we don't overwrite RELTYPE=CHILD/SIBLING entries". Sioul writes steps the same way, with `RELTYPE=PARENT`, reads a bigger task's `CHILD` lines too, and keeps every line it does not edit, so the two can share a task list ([compatibility.md](compatibility.md)). Whether Sioul's costs and ratings survive editing there is still to test ([capacity.md](capacity.md)).
- **Obsidian** keeps notes as plain Markdown files in a vault, a folder that other editors can open. Sioul reads the same folder as Obsidian does: wikilinks found the same way, embeds, tags, front matter, links back, deletion into the vault's `.trash` ([case-store.md](case-store.md); [tasks.md](tasks.md), "Places"). It avoids what does not travel: Obsidian's program is closed, its terms forbid modifying it or trying to discover its source code, and it grows by thousands of plugins. Sioul is compatible with the files, not with the program, and has no plugin system.
- **Taskmaster**, a research prototype from PARC (Bellotti, Ducheneaut, Howard & Smith 2003), recast mail as task management: its "thrasks" gathered a thread's messages, sent ones included, with their attachments, links and drafts, into one collection that could be renamed, split and merged. Its users rated its warning bars highest of its features (4.4 on a scale of 5): each showed the time left before a deadline in green and the rest in red, the red growing as the date came. Sioul takes the collection as the case ([case-store.md](case-store.md)), and the time left, said in words ("three weeks left") and never in red. Taskmaster also showed what a second tool costs: built as an add-on beside Outlook, it made some users file everything twice ("I felt I had to do the work twice"), which limited its use. Sioul is a complete client on the person's own accounts, and what it files reaches the server, where other clients see it ([client.md](client.md); [compatibility.md](compatibility.md)). In 2006, an article co-written by one of its authors still called it "only a proof-of-concept prototype".

None of them combines what Sioul combines: mail checked, genuine or forged, and held until the hours the person chose; the mail, the websites' secure mailboxes, the tasks, notes and people of one matter gathered into its case; steps that wait for each other, kept in the person's own CalDAV server and planned within the hours and the capacity the person gives; plain files on the person's own devices; and AI agents that may draft but never send ([mcp.md](mcp.md)).

Sources, read on 8 October 2026:
- Mozilla, "Thunderbird:Autoconfiguration": https://wiki.mozilla.org/Thunderbird:Autoconfiguration ; Thunderbird's default preferences, `mailnews/mailnews.js`: https://github.com/mozilla/releases-comm-central/blob/master/mailnews/mailnews.js ; its invitation bar, `calendar/base/content/imip-bar.js`: https://github.com/mozilla/releases-comm-central/blob/master/calendar/base/content/imip-bar.js
- HEY, "Features": https://www.hey.com/features/ ; "Frequently asked questions about HEY": https://www.hey.com/faqs/
- Superhuman, "Superhuman Mail": https://superhuman.com/mail ; "How to build a remarkable command palette" (2021): https://blog.superhuman.com/how-to-build-a-remarkable-command-palette/ ; Rahul Vohra, "7 principles of game design" (2021): https://blog.superhuman.com/game-design-not-gamification/ ; Grammarly, "Grammarly to Acquire Superhuman to Accelerate Its AI Productivity Platform" (2025): https://www.grammarly.com/blog/company/grammarly-to-acquire-superhuman/
- aerc: https://aerc-mail.org/ ; Mutt: http://www.mutt.org/ ; notmuch, "Notmuch -- Just an email system": https://notmuchmail.org/ ; Himalaya, README: https://github.com/pimalaya/himalaya
- Kontact: https://kontact.kde.org/ ; KDE UserBase, "Akonadi": https://userbase.kde.org/Akonadi ; Evolution Data Server, README: https://github.com/GNOME/evolution-data-server
- Apple, "Apple unveils the winners of the 2025 App Store Awards" (4 December 2025): https://www.apple.com/newsroom/2025/12/apple-unveils-the-winners-of-the-2025-app-store-awards/ ; Tiimo: https://www.tiimoapp.com/ ; "About": https://www.tiimoapp.com/about
- Goblin Tools, "About": https://goblin.tools/About ; "Magic ToDo": https://goblin.tools/ToDo
- Taskwarrior, "Urgency": https://taskwarrior.org/docs/urgency/ ; task(1): https://taskwarrior.org/docs/man/task.1/
- TaskJuggler, "About TaskJuggler": https://taskjuggler.org/
- Vikunja, "Features": https://vikunja.io/features/ ; "Task Relations": https://vikunja.io/help/task-relations ; "Views": https://vikunja.io/help/views ; "CalDAV": https://vikunja.io/help/caldav
- Nextcloud Tasks, `src/models/task.js`: https://github.com/nextcloud/tasks/blob/main/src/models/task.js
- Obsidian, "How Obsidian stores data": https://obsidian.md/help/data-storage ; "Terms of Service": https://obsidian.md/terms
- Bellotti, V., Ducheneaut, N., Howard, M. & Smith, I. (2003). Taking email to task: the design and evaluation of a task management centered email tool. *Proceedings of the SIGCHI Conference on Human Factors in Computing Systems* (CHI '03, Ft. Lauderdale, Florida), ACM, 345–352. https://doi.org/10.1145/642611.642672 ; the authors' copy, archived in 2004: https://web.archive.org/web/20040718101912/http://www2.parc.com/csl/members/nicolas/documents/CHI2003.pdf
- Gupta, A., Sharda, R., Ducheneaut, N., Zhao, J. L. & Weber, R. (2006). E-mail management: a techno-managerial research perspective. *Communications of the Association for Information Systems* 17, 941–961. https://doi.org/10.17705/1cais.01743

## The name

*Sioul* [siwl] is a Breton adjective: calm, peaceful, silent, quiet; figuratively, discreet ([Wiktionnaire, "sioul"](https://fr.wiktionary.org/wiki/sioul)). Wiktionnaire's example for the figurative sense, from Jules Gros's *Le trésor du breton parlé* (1970), is "Hennez a zo eun den sioul": that one is a discreet man, who does not repeat what he is told. Wiktionnaire relates the word to the Middle Breton *sioulic*, "under one's breath, in secret".

Its family says the same:
- *sioulder*, calm, silence, quiet ([Wiktionnaire, "sioulder"](https://fr.wiktionary.org/wiki/sioulder));
- *sioulded*, calm, quiet ([Wiktionnaire, "sioulded"](https://fr.wiktionary.org/wiki/sioulded));
- *sioulaat*, to calm, or to calm down. Wiktionnaire lists it among the words derived from *sioul* but has no entry for it; the Breton Wiktionary has one ([Wikeriadur, "sioulaat"](https://br.wiktionary.org/wiki/sioulaat)).

Three names were weighed on 2 October 2026:
- **Sioul**: one syllable, and it says what the application is for.
- **Goustadik**: Breton for "very slowly", from *goustad* (slowly, gently, in a low voice) and the suffix *-ik* ([Wiktionnaire, "goustadik"](https://fr.wiktionary.org/wiki/goustadik); ["goustad"](https://fr.wiktionary.org/wiki/goustad)). Wiktionnaire's example, from Fañch al Lae's *Bilzig* (1925): "Goustad, goustadik, ar vag a dostae ouz ar merkou", slowly, very slowly, the boat was nearing the seamarks. It is long to type, and hard to spell for anyone who does not speak Breton. The study proposed it as a motto, or as the name of the slowest mode; neither exists.
- **Midgrey**: the 18 % grey of photographers, the neutral reference that light meters and cameras are calibrated against ([Wikipedia, "Middle gray"](https://en.wikipedia.org/wiki/Middle_gray)).

What was free, checked on 2 October 2026 (crates.io, Flathub, GitHub's repository names, domain names):
- **Sioul**: crates.io had no crate of that name, and Flathub no application; 18 GitHub repositories had the word in their name, none of them with a star; sioul.org, sioul.app, sioul.dev, sioul.eu and sioul.net were unregistered, and sioul.fr was registered by someone else.
- **Goustadik**: free on crates.io and GitHub, and goustadik.org was unregistered.
- **Midgrey**: free in the same checks.

The author chose Sioul the same day.
