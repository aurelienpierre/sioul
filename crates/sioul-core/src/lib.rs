// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

//! Sioul's core: what Sioul knows and decides, the same for every interface.
//!
//! Everything the interface shows is decided here, so that the command line,
//! the Qt interface and AI agents all see the same Porch. The interface holds
//! no logic of its own (docs/architecture.md). The core reads and writes plain
//! files (Maildir, vdir, TOML, Markdown) and never uses the network: talking
//! to servers is the work of `sioul-sync`.
//!
//! Sioul has five crates, all in the repository's `crates/` folder:
//!
//! - **sioul-core**, this one: the library at the centre.
//! - **[sioul-sync](../sioul_sync/index.html)**: what talks to the world: mail
//!   servers, calendars and contacts, the keyring, sharing between devices.
//! - **[sioul](../sioul/index.html)** (`crates/sioul-cli`): the command line,
//!   and `sioul mcp`, the server for AI agents.
//! - **[sioul-app](../sioul_app/index.html)**: the window's Rust side, under
//!   its QML pages.
//! - **[sioul-learn](../sioul_learn/index.html)**: the spam filter's training,
//!   on computers only (docs/spam-filter.md).
//!
//! # Where to start reading
//!
//! - **How new mail is judged**: [`porch::triage`] takes a [`card::Card`],
//!   what is known of one message before it is opened, and gives it a lane and
//!   the reasons for it ([`porch::Triaged`]).
//! - **What the window shows**: [`view`] and [`taskview`]. Their functions
//!   return plain data, every sentence already in your language; the window
//!   receives it as JSON and only lays it out. [`view::porch`] makes the Porch.
//! - **Tasks and the plan**: [`tasks`] reads and writes the task files;
//!   [`plan::plan`] works out the plan from them each time it is asked.
//! - **Words**: every sentence goes through [`i18n::Translator`], from the
//!   Fluent files in `crates/sioul-core/locales/` (docs/i18n.md).
//! - **Settings**: [`config::Config`], the configuration file.
//!
//! Names such as docs/porch.md are the design notes in the repository's
//! `docs/` folder. The website shows them too, under the same name:
//! <https://aurelienpierre.github.io/sioul/dev/porch.html>.
//!
//! # Modules, by theme
//!
//! Each module is one file of `crates/sioul-core/src/`: [`porch`] is
//! `porch.rs`.
//!
//! ## What the window shows
//!
//! - [`view`]: the Porch, mail, budgets, accounts, the agenda and contacts, as plain data.
//! - [`taskview`]: the task pages in words: the next step and why, the board, the list, the timeline.
//! - [`dayview`]: the day, seen: today's events and the plan's steps laid into your hours.
//!
//! ## The Porch: new mail, checked and sorted
//!
//! - [`porch`]: where new mail waits, checked and sorted into lanes, until you look.
//! - [`card`]: what the Porch knows about one message before you open it.
//! - [`state`]: what the Porch remembers between windows: where you closed it.
//! - [`trust`]: who really sent a message, from what the servers recorded.
//! - [`lookalike`]: senders who borrow a name.
//! - [`codes`]: one-time codes, password resets, sign-in links and addresses to confirm.
//! - [`payments`]: mail about money: a payment made or received, a bill, an order, a refund.
//! - [`shield`]: the shield of a public address: its mail read first, its tone and topic.
//! - [`rules`]: the vocabulary of sorting: what a rule may propose.
//! - [`text`]: text helpers shared by the detectors.
//! - [`words`]: the words the detectors look for: shipped packs by language and country, and your changes.
//! - [`reach`]: who reaches you, on which channel, and the clock that says what time it is, ahead.
//! - [`attention`]: what reaches you, and when: one matrix of who and what by time, its pipeline, its words.
//! - [`mailnote`]: new mail as a notification, at the times it may come.
//! - [`letters`]: paper letters: a scan read by OCR, understood, and shown as a card.
//! - [`voicemail`]: voicemail that the phone operator sends by mail.
//! - [`spam`]: Sioul's own spam filter, as every device runs it (docs/spam-filter.md); its training
//!   is in [sioul-learn](../sioul_learn/index.html).
//!
//! ## Mail: on disk, read and written
//!
//! - [`maildir`]: messages on disk: a Maildir, or a plain folder of `.eml` files.
//! - [`headers`]: the raw header block, unfolded.
//! - [`mailindex`]: messages by their Message-ID, to follow `mid:` links.
//! - [`mailsearch`]: searching the mail by conditions, here and on the servers.
//! - [`folders`]: mail folders: what each is for, its name as you read it, where it is kept.
//! - [`threads`]: conversations, tied by Message-ID, never by subject alone.
//! - [`reading`]: a message laid out for reading: its text, what it quotes, what it forwards.
//! - [`compose`]: writing: Markdown in, mail out.
//! - [`mailto`]: `mailto:` addresses: the message a link asks to write.
//! - [`handed`]: files, text and `mailto:` links other applications hand Sioul: each a draft.
//! - [`pgp`]: OpenPGP for mail, with Sequoia.
//! - [`securitykey`]: your OpenPGP keys on a security key (a YubiKey): signing and opening there, its certificate checked.
//! - [`unsubscribe`]: leaving a mailing list in one click: when it may be done.
//!
//! ## Tasks, projects, notes and links
//!
//! - [`tasks`]: to-dos (VTODO) in CalDAV task lists, one per file, and what ties them to the rest.
//! - [`plan`]: the plan: what can start, the one next step, when each task could happen.
//! - [`capture`]: quick capture: one line typed, a task made.
//! - [`blocks`]: time blocks: a task pinned to a time, as an event in a calendar.
//! - [`demands`]: what a task or an event asks of you, and what it gives back.
//! - [`capacity`]: what a day can hold, learned from your own days.
//! - [`routines`]: routines: timed steps played one at a time.
//! - [`stopped`]: where you stopped: one line kept when something interrupts you.
//! - [`today`]: today's own choices: how the day is, the steps put off for today.
//! - [`cases`]: cases, the dossiers your admin is organised by (the window calls them projects).
//! - [`project`]: a project, or a case, on one page: everything dated in it.
//! - [`github`]: GitHub's issues and pull requests that are yours, as tasks.
//! - [`notes`]: a folder of Markdown files, read the way Obsidian reads a vault.
//! - [`links`]: links between everything: mail, tasks, events, contacts, notes, budget lines.
//!
//! ## Time and money
//!
//! - [`timelog`]: time spent on tasks, and the focus session running now.
//! - [`timereport`]: time spent, as the Time page and a project's page show it.
//! - [`invoice`]: invoices for work done for someone, from a project's billable time.
//! - [`budget`]: budgets, reserves, and the lines between them.
//! - [`accounts`]: bank accounts, and the budgets each movement goes to.
//! - [`bank`]: the money watch: the bank's exports held against the recurring payments.
//! - [`money`]: exact amounts, and the amounts written in mail.
//! - [`contracts`]: contracts and subscriptions: when each renews, how to stop it.
//! - [`papers`]: the papers wallet: the papers asked again and again, and when each ends.
//!
//! ## Hours, pauses and reminders
//!
//! - [`areas`]: what things are for (work, your admin, leisure), and which time is for what.
//! - [`window`]: working hours: the days and hours work can reach you.
//! - [`quiet`]: the time now (work, admin, a meal, sleep, leisure), and what comes then.
//! - [`needs`]: the day's needs, set first: meals, naps and night sleep.
//! - [`pause`]: the two pauses: Free time, and Paused.
//! - [`everywhere`]: do-not-disturb on every device.
//! - [`reminders`]: reminders before dates: one quiet notification for each.
//! - [`reviews`]: how each day went, as you said it.
//! - [`sounds`]: sounds to rest by or to focus with, all made here.
//!
//! ## Health
//!
//! - [`health`]: prescriptions, medicines and when to take them, pauses to move, a limit on chats.
//! - [`doses`]: each dose that fell due, as a record your devices share.
//!
//! ## Contacts and calendars
//!
//! - [`contacts`]: contacts, one vCard per person.
//! - [`agenda`]: calendars: events from iCalendar files, repeating ones expanded.
//! - [`vdir`]: address books and calendars on disk: one folder per collection, one file per item.
//! - [`lines`]: content lines, the text format that vCard and iCalendar share.
//! - [`phones`]: phone numbers, compared rather than rewritten.
//! - [`places`]: where postal addresses are, to show contacts on a map.
//! - [`duplicates`]: duplicates in the address books, found and cleaned when you ask.
//! - [`overlaps`]: two events at once.
//! - [`capabilities`]: what each kind of server keeps of a task or a contact.
//!
//! ## On a phone
//!
//! - [`calls`]: calls screened on a phone.
//! - [`appnotes`]: other apps' notifications on a phone: shown now, or held until their time.
//!
//! ## Settings, words and files
//!
//! - [`config`]: the configuration file: accounts, working hours, the case store.
//! - [`settings`]: settings, shown where they apply, and written into the configuration.
//! - [`i18n`]: every sentence Sioul shows, in your language.
//! - [`sites`]: sites kept open in Sioul (a bank's secure mailbox, a chat), and what they notified.
//! - [`presets`]: sites people commonly keep, offered when adding one.
//! - [`weather`]: the weather at a place you chose, for the status line.
//! - [`filelock`]: one writer at a time on a file that two parts of Sioul change.

pub mod accounts;
pub mod agenda;
pub mod appnotes;
pub mod areas;
pub mod attention;
pub mod bank;
pub mod blocks;
pub mod budget;
pub mod build;
pub mod calls;
pub mod capabilities;
pub mod capacity;
pub mod capture;
pub mod card;
pub mod cases;
pub mod consent;
pub mod codes;
pub mod compose;
pub mod config;
pub mod contracts;
pub mod dayview;
pub mod demands;
pub mod doses;
pub mod filelock;
pub mod contacts;
pub mod duplicates;
pub mod everywhere;
pub mod folders;
pub mod github;
pub mod handed;
pub mod headers;
pub mod health;
pub mod i18n;
pub mod invoice;
pub mod lines;
pub mod letters;
pub mod links;
pub mod lookalike;
pub mod mailindex;
pub mod mailsearch;
pub mod maildir;
pub mod mailnote;
pub mod mailto;
pub mod money;
pub mod needs;
pub mod overlaps;
pub mod notes;
pub mod payments;
pub mod pgp;
pub mod phonemsgs;
pub mod phones;
pub mod plan;
pub mod places;
pub mod porch;
pub mod presets;
pub mod papers;
pub mod pause;
pub mod quiet;
pub mod reach;
pub mod reviews;
pub mod routines;
pub mod reminders;
pub mod project;
pub mod reading;
pub mod rules;
pub mod securitykey;
pub mod settings;
pub mod shield;
pub mod sounds;
pub mod spam;
pub mod sites;
pub mod state;
pub mod stopped;
pub mod taskview;
// texts: SMS phase (b), read and sent through the phone (docs/texts.md).
pub mod texts;
pub mod tasks;
pub mod text;
pub mod textdraft;
pub mod threads;
pub mod timelog;
pub mod timereport;
pub mod today;
pub mod trust;
pub mod unsubscribe;
pub mod vdir;
pub mod view;
pub mod voicemail;
pub mod weather;
pub mod window;
pub mod words;
