// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

//! The object behind the window.
//!
//! It hands QML the core's views as JSON (`sioul_core::view`), already in your
//! language, and runs everything slow on background threads: fetching mail,
//! finding servers, testing logins. Results come back to Qt's thread through
//! `qt_thread().queue`. Each mail account has a watcher that keeps its inbox
//! open with IDLE, so a verified code becomes a notification within seconds.

use cxx_qt::{CxxQtType, Threading};
use cxx_qt_lib::QString;
use jiff::Zoned;
use sioul_core::budget::{self, Ledger, MailLine};
use sioul_core::card::ImapOrigin;
use sioul_core::cases::CaseStore;
use sioul_core::config::{self, Account, Config, Priority, Security};
use sioul_core::i18n::{self, Translator};
use sioul_core::porch::{self, KnownSenders, SenderList, Triaged};
use sioul_core::state::{MoneyState, PorchState};
use sioul_core::{maildir, reading, view};
use crate::{crypto, mail, pim, work};
use sioul_sync::antivirus::{self, Verdict};
use sioul_sync::{Control, Learned, Report, SyncError, notify, secret};
use std::collections::{BTreeMap, BTreeSet};
use std::hash::{Hash, Hasher};
use std::path::{Path, PathBuf};
use std::pin::Pin;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex, OnceLock};

#[cxx_qt::bridge]
pub mod qobject {
    unsafe extern "C++" {
        include!("cxx-qt-lib/qstring.h");
        type QString = cxx_qt_lib::QString;
    }

    #[auto_cxx_name]
    extern "RustQt" {
        #[qobject]
        #[qml_element]
        #[qproperty(QString, porch)]
        #[qproperty(QString, budgets)]
        #[qproperty(QString, accounts)]
        #[qproperty(QString, blocked)]
        #[qproperty(QString, status)]
        #[qproperty(bool, busy)]
        #[qproperty(QString, found)]
        #[qproperty(QString, scouted)]
        #[qproperty(bool, scouting)]
        #[qproperty(QString, form_error)]
        #[qproperty(bool, form_busy)]
        #[qproperty(QString, mail_accounts)]
        #[qproperty(QString, mail_folder)]
        #[qproperty(QString, drafts)]
        #[qproperty(QString, undo_line)]
        #[qproperty(QString, contacts)]
        #[qproperty(QString, agenda)]
        #[qproperty(QString, tasks)]
        #[qproperty(QString, notes)]
        #[qproperty(QString, focus_session)]
        #[qproperty(QString, reading)]
        #[qproperty(bool, realtime)]
        #[qproperty(QString, mode)]
        #[qproperty(QString, forecast)]
        #[qproperty(QString, places_found)]
        type Sioul = super::SioulRust;

        /// A sentence of the interface, in your language.
        #[qinvokable]
        fn text(self: &Sioul, id: &QString) -> QString;

        /// The same, with one named argument.
        #[qinvokable]
        fn text_with(self: &Sioul, id: &QString, name: &QString, value: &QString) -> QString;

        /// A sentence with several arguments, given as a JSON object of names and values.
        #[qinvokable]
        fn text_args(self: &Sioul, id: &QString, args: &QString) -> QString;

        /// Shows what is on disk, then starts a watcher per mail account.
        #[qinvokable]
        fn start(self: Pin<&mut Sioul>);

        /// Computes the views again: windows open and close, codes expire.
        #[qinvokable]
        fn refresh(self: Pin<&mut Sioul>);

        /// Fetches mail now, and restarts watchers that stopped.
        #[qinvokable]
        fn sync_now(self: Pin<&mut Sioul>);

        /// Opens the Porch outside an admin window, until "Done for now".
        #[qinvokable]
        fn open_anyway(self: Pin<&mut Sioul>);

        /// Closes the Porch on what it shows.
        #[qinvokable]
        fn done(self: Pin<&mut Sioul>);

        /// A message laid out for reading, as JSON (`view::MessageView`); empty if it cannot be read.
        #[qinvokable]
        fn message(self: &Sioul, key: &QString) -> QString;

        /// Has the antivirus check an attachment, then opens it with the desktop's application.
        #[qinvokable]
        fn open_attachment(self: Pin<&mut Sioul>, key: &QString, index: i32);

        /// Has the antivirus check an attachment, then copies it into the downloads folder.
        #[qinvokable]
        fn save_attachment(self: Pin<&mut Sioul>, key: &QString, index: i32);

        /// Opens, saves (1) or keeps as a paper (2) an attachment without a check: there is no antivirus,
        /// and you said to go on.
        #[qinvokable]
        fn attachment_unchecked(self: Pin<&mut Sioul>, key: &QString, index: i32, what: i32);

        /// Has the antivirus check an attachment, then keeps it in the papers wallet; `paper_kept` follows.
        #[qinvokable]
        fn keep_attachment_as_paper(self: Pin<&mut Sioul>, key: &QString, index: i32);

        /// The papers wallet, as JSON (`papers::view`).
        #[qinvokable]
        fn papers(self: &Sioul) -> QString;

        /// A paper saved (new when `id` is empty) from the form's JSON; returns what went wrong, else "".
        #[qinvokable]
        fn save_paper(self: Pin<&mut Sioul>, id: &QString, edit: &QString) -> QString;

        /// A paper taken out of the wallet, its file left where it is; returns what went wrong, else "".
        #[qinvokable]
        fn remove_paper(self: Pin<&mut Sioul>, id: &QString) -> QString;

        /// A paper's renewal planned as a task; returns what went wrong, else "".
        #[qinvokable]
        fn plan_renewal(self: Pin<&mut Sioul>, id: &QString) -> QString;

        /// Lets a sender in: they leave the screener.
        #[qinvokable]
        fn let_in(self: Pin<&mut Sioul>, address: &QString);

        /// Blocks an address, or `@domain`: their mail is set aside for good.
        #[qinvokable]
        fn block(self: Pin<&mut Sioul>, entry: &QString);

        #[qinvokable]
        fn unblock(self: Pin<&mut Sioul>, entry: &QString);

        /// An address or a pattern marked "safe" (any hour), "neutral" (working
        /// hours, the default) or "blocked"; out of the two other lists.
        #[qinvokable]
        fn set_standing(self: Pin<&mut Sioul>, entry: &QString, standing: &QString);

        /// Where an address stands: "safe", "neutral" or "blocked".
        #[qinvokable]
        fn standing(self: &Sioul, address: &QString) -> QString;

        /// Writes the line a message about money stands for into a budget.
        #[qinvokable]
        fn add_mail_line(self: Pin<&mut Sioul>, key: &QString, budget: &QString);

        /// Leaves a message about money out of the budgets.
        #[qinvokable]
        fn ignore_mail_line(self: Pin<&mut Sioul>, key: &QString);

        /// Finds the IMAP server of an address; the answer comes in `found`.
        #[qinvokable]
        fn discover(self: Pin<&mut Sioul>, address: &QString);

        /// An account switched on or off: off, it keeps its settings and is
        /// neither synced nor shown. Returns what went wrong, if anything.
        #[qinvokable]
        fn set_account_enabled(self: Pin<&mut Sioul>, id: &QString, on: bool) -> QString;

        /// What the server of an address offers (mail, calendars and contacts,
        /// a Nextcloud's apps), into `scouted`, as JSON.
        #[qinvokable]
        fn scout_account(self: Pin<&mut Sioul>, address: &QString);

        /// Mail added to an address that has calendars and contacts already, with their password.
        #[qinvokable]
        fn add_mail_like(self: Pin<&mut Sioul>, address: &QString, host: &QString, port: i32, security: &QString, login: &QString);

        /// Tests the login, keeps the password in the keyring, adds the account.
        #[qinvokable]
        fn add_account(self: Pin<&mut Sioul>, address: &QString, host: &QString, port: i32, security: &QString, login: &QString, password: &QString);

        /// Adds a web-only mailbox.
        #[qinvokable]
        fn add_portal(self: Pin<&mut Sioul>, name: &QString, url: &QString);

        /// Removes an account and its password; its mail stays on disk.
        #[qinvokable]
        fn remove_account(self: Pin<&mut Sioul>, id: &QString);

        /// An account's password given on this device (one come from another
        /// device arrives without it): tested with its server, then kept;
        /// `account_password_done` says how it went.
        #[qinvokable]
        fn set_account_password(self: Pin<&mut Sioul>, id: &QString, password: &QString);

        /// Ranks an account's mail: "above", "average" or "below".
        #[qinvokable]
        fn set_priority(self: Pin<&mut Sioul>, id: &QString, priority: &QString);

        /// Shows a folder of an account: the last two weeks, or `all`; `query` searches it.
        #[qinvokable]
        fn open_folder(self: Pin<&mut Sioul>, account: &QString, folder: &QString, all: bool, query: &QString);

        /// A message was opened: it is marked read.
        #[qinvokable]
        fn opened(self: Pin<&mut Sioul>, key: &QString);

        /// Does something to a message: "read", "unread", "flag", "unflag" at once;
        /// "archive", "trash", "junk", "not-junk", "move" (to `target`) after ten seconds to undo.
        #[qinvokable]
        fn act(self: Pin<&mut Sioul>, key: &QString, action: &QString, target: &QString);

        /// The same act on several messages (a JSON list of keys), under one "Undo".
        #[qinvokable]
        fn act_many(self: Pin<&mut Sioul>, keys: &QString, action: &QString);

        /// Messages (a JSON list of keys) into a folder of an account, theirs or another.
        #[qinvokable]
        fn move_messages(self: Pin<&mut Sioul>, keys: &QString, account: &QString, folder: &QString);

        /// Cancels the newest act still waiting.
        #[qinvokable]
        fn undo(self: Pin<&mut Sioul>);

        /// The window closes: what waits is done now.
        #[qinvokable]
        fn flush(self: Pin<&mut Sioul>);

        /// Starts a draft ("new", "reply", "reply-all", "forward"); returns its name, or "" when it cannot.
        #[qinvokable]
        fn compose(self: Pin<&mut Sioul>, kind: &QString, key: &QString, account: &QString) -> QString;

        /// A draft as the writing window shows it, as JSON.
        #[qinvokable]
        fn draft(self: &Sioul, id: &QString) -> QString;

        /// Saves what the writing window holds (JSON); returns "Saved at 14:02", or what went wrong.
        #[qinvokable]
        fn save_draft(self: Pin<&mut Sioul>, edit: &QString) -> QString;

        /// Attaches a file (a `file://` URL); returns what went wrong, if anything.
        #[qinvokable]
        fn attach(self: Pin<&mut Sioul>, id: &QString, url: &QString) -> QString;

        /// Takes an attachment off a draft: one added, or one a forward carries.
        #[qinvokable]
        fn detach(self: Pin<&mut Sioul>, id: &QString, index: i32, forwarded: bool);

        /// What the Markdown becomes, for the preview.
        #[qinvokable]
        fn preview(self: &Sioul, markdown: &QString) -> QString;

        /// Sends after ten seconds to undo; returns what stops it now, else "".
        #[qinvokable]
        fn send(self: Pin<&mut Sioul>, id: &QString) -> QString;

        /// Deletes a draft after ten seconds to undo.
        #[qinvokable]
        fn discard(self: Pin<&mut Sioul>, id: &QString);

        /// The writing window closed.
        #[qinvokable]
        fn draft_closed(self: Pin<&mut Sioul>, id: &QString);

        /// Where a message is: its account and folder, as JSON.
        #[qinvokable]
        fn place(self: &Sioul, key: &QString) -> QString;

        /// The message as it came, for "Show the source".
        #[qinvokable]
        fn source(self: &Sioul, key: &QString) -> QString;

        /// Your name as recipients see it, and your signature.
        #[qinvokable]
        fn set_writing(self: Pin<&mut Sioul>, id: &QString, name: &QString, signature: &QString);

        /// For development and documentation: the folder in `SIOUL_GRAB`, where
        /// the window saves each page as an image before quitting; empty otherwise.
        #[qinvokable]
        fn grab_folder(self: &Sioul) -> QString;

        /// Shows the contacts matching `query` (names, addresses, numbers, organisations).
        #[qinvokable]
        fn search_contacts(self: Pin<&mut Sioul>, query: &QString);

        /// A contact, every field, as JSON; empty when it cannot be read.
        #[qinvokable]
        fn contact(self: &Sioul, key: &QString) -> QString;

        /// Saves the contact form (JSON); an empty key makes a new contact.
        /// Returns {"key": …} or {"error": …}.
        #[qinvokable]
        fn save_contact(self: Pin<&mut Sioul>, key: &QString, edit: &QString) -> QString;

        /// Deletes a contact after ten seconds to undo.
        #[qinvokable]
        fn delete_contact(self: Pin<&mut Sioul>, key: &QString);

        /// Shows `days` days of the agenda from `from` ("2026-10-05"; empty: today).
        #[qinvokable]
        fn show_days(self: Pin<&mut Sioul>, from: &QString, days: i32);

        /// The calendars a new event can go into, as JSON.
        #[qinvokable]
        fn calendars(self: &Sioul) -> QString;

        /// An event as its form shows it, as JSON.
        #[qinvokable]
        fn event(self: &Sioul, key: &QString) -> QString;

        /// Saves the event form (JSON); an empty key makes a new event in `calendar`.
        /// Returns what went wrong, else "".
        #[qinvokable]
        fn save_event(self: Pin<&mut Sioul>, key: &QString, edit: &QString, calendar: &QString) -> QString;

        /// Deletes an event, or only the occurrence starting at `start`, after ten seconds to undo.
        #[qinvokable]
        fn delete_event(self: Pin<&mut Sioul>, key: &QString, start: f64, only_this: bool);

        /// Addresses from the contacts for what is typed in an address field, as JSON.
        #[qinvokable]
        fn completions(self: &Sioul, typed: &QString) -> QString;

        /// The contact with this address, by file; empty when none.
        #[qinvokable]
        fn contact_for(self: &Sioul, address: &QString) -> QString;

        /// Makes a contact of a sender.
        #[qinvokable]
        fn add_sender(self: Pin<&mut Sioul>, name: &QString, address: &QString);

        /// The invitation a message carries, as JSON; empty when none.
        #[qinvokable]
        fn invitation(self: &Sioul, key: &QString) -> QString;

        /// Answers an invitation: "accepted", "tentative", "declined"; "add" or "remove" for events sent without a question.
        #[qinvokable]
        fn answer_invitation(self: Pin<&mut Sioul>, key: &QString, answer: &QString);

        /// How far back mail and the agenda reach, in weeks; 0 for everything.
        #[qinvokable]
        fn history_weeks(self: &Sioul) -> i32;

        /// The hour the day and week planning open on.
        #[qinvokable]
        fn day_start(self: &Sioul) -> i32;

        /// Sets how far back mail and the agenda reach; the older mail comes at the next fetch.
        #[qinvokable]
        fn set_history(self: Pin<&mut Sioul>, weeks: i32);

        /// Adds a contacts-and-calendars account (CardDAV, CalDAV); the server address may be empty.
        #[qinvokable]
        fn add_dav(self: Pin<&mut Sioul>, address: &QString, url: &QString, login: &QString, password: &QString);

        /// Adds a Google account with your OAuth client: Google's page opens in
        /// the browser, Sioul waits for its answer. `again`: an account's id, signed in again.
        #[qinvokable]
        fn add_google(self: Pin<&mut Sioul>, address: &QString, client_id: &QString, client_secret: &QString, again: &QString);

        /// Stops waiting for Google's page.
        #[qinvokable]
        fn cancel_google(self: Pin<&mut Sioul>);

        /// Your OpenPGP keys and others', as JSON.
        #[qinvokable]
        fn pgp_keys(self: &Sioul) -> QString;

        /// Makes a key for one of your addresses.
        #[qinvokable]
        fn pgp_generate(self: Pin<&mut Sioul>, address: &QString);

        /// Imports keys from a file (`file://`); returns what went wrong, else "".
        #[qinvokable]
        fn pgp_import(self: Pin<&mut Sioul>, url: &QString, passphrase: &QString) -> QString;

        /// Saves a public key into the downloads folder.
        #[qinvokable]
        fn pgp_export(self: Pin<&mut Sioul>, fingerprint: &QString);

        /// Removes a key; yours are kept aside, never deleted.
        #[qinvokable]
        fn pgp_remove(self: Pin<&mut Sioul>, fingerprint: &QString);

        /// Looks up the keys a draft's recipients miss (their domain, then keys.openpgp.org).
        #[qinvokable]
        fn pgp_lookup(self: Pin<&mut Sioul>, draft: &QString);

        /// The task page's choices: the list grouped "case" or "list", done tasks, a search, one case.
        #[qinvokable]
        fn show_tasks(self: Pin<&mut Sioul>, by: &QString, done: bool, query: &QString, case_id: &QString);

        /// Tasks of one kind ("call"…) and one category only; "" for all. Kept for next time.
        #[qinvokable]
        fn filter_tasks(self: Pin<&mut Sioul>, kind: &QString, category: &QString);

        /// At rest, the tasks all the same ("Show anyway"), until the page is left.
        #[qinvokable]
        fn show_tasks_anyway(self: Pin<&mut Sioul>, on: bool);

        /// Reads tasks and notes again and shows them.
        #[qinvokable]
        fn refresh_work(self: Pin<&mut Sioul>);

        /// One task in full, with its steps and what it is tied to, as JSON.
        #[qinvokable]
        fn task(self: &Sioul, uid: &QString) -> QString;

        /// Saves the task form (JSON); an empty UID makes a new task in `list`.
        /// Returns {"uid": …} or {"error": …}.
        #[qinvokable]
        fn save_task(self: Pin<&mut Sioul>, uid: &QString, edit: &QString, list: &QString) -> QString;

        /// What a typed line says (its chips), before it becomes a task, as JSON.
        #[qinvokable]
        fn capture(self: &Sioul, line: &QString) -> QString;

        /// A task from a typed line; a step of `parent` when given. Returns {"uid"} or {"error"}.
        #[qinvokable]
        fn add_task(self: Pin<&mut Sioul>, line: &QString, parent: &QString, list: &QString) -> QString;

        /// "completed", "in-process", "needs-action", "cancelled"; returns what it changed, in a sentence.
        #[qinvokable]
        fn set_task_status(self: Pin<&mut Sioul>, uid: &QString, status: &QString) -> QString;

        /// The task waits for `other`, or no longer; returns what went wrong, else "".
        #[qinvokable]
        fn set_waits(self: Pin<&mut Sioul>, uid: &QString, other: &QString, wait: bool) -> QString;

        /// Puts a task off until tomorrow.
        #[qinvokable]
        fn not_now(self: Pin<&mut Sioul>, uid: &QString);

        /// How today is: "clear", "haze", "fog".
        #[qinvokable]
        fn set_weather(self: Pin<&mut Sioul>, weather: &QString);

        /// Deletes a task after ten seconds to undo.
        #[qinvokable]
        fn delete_task(self: Pin<&mut Sioul>, uid: &QString);

        /// Open tasks whose title holds `query`, `except` left out, as JSON.
        #[qinvokable]
        fn search_tasks(self: &Sioul, query: &QString, except: &QString) -> QString;

        /// Makes a task list in an account ("local": kept here only); returns "account/id", or what went wrong.
        #[qinvokable]
        fn new_list(self: Pin<&mut Sioul>, account: &QString, name: &QString) -> QString;

        /// The accounts a task list can be made in, as JSON.
        #[qinvokable]
        fn list_accounts(self: &Sioul) -> QString;

        /// Starts a focus session on a task.
        #[qinvokable]
        fn focus_start(self: Pin<&mut Sioul>, uid: &QString, minutes: i32);

        /// Pauses the focus session, or goes on.
        #[qinvokable]
        fn focus_pause(self: Pin<&mut Sioul>);

        /// More minutes for the focus session.
        #[qinvokable]
        fn focus_extend(self: Pin<&mut Sioul>, minutes: i32);

        /// Ends the focus session, kept with where you stopped; returns what it changed.
        #[qinvokable]
        fn focus_stop(self: Pin<&mut Sioul>, done: bool, note: &QString) -> QString;

        /// Shows the notes matching `query`.
        #[qinvokable]
        fn search_notes(self: Pin<&mut Sioul>, query: &QString);

        /// One note: text, HTML, tags, checkboxes, what is tied to it, as JSON.
        #[qinvokable]
        fn note(self: &Sioul, path: &QString) -> QString;

        /// Saves a note; returns what went wrong, else "".
        #[qinvokable]
        fn save_note(self: Pin<&mut Sioul>, path: &QString, text: &QString) -> QString;

        /// A new note in the notes folder, linking `links` (a JSON list); returns its path.
        #[qinvokable]
        fn create_note(self: Pin<&mut Sioul>, title: &QString, links: &QString) -> QString;

        /// What a thing is tied to, both ways, as JSON.
        #[qinvokable]
        fn related(self: &Sioul, uri: &QString) -> QString;

        /// Ties two things, in the one that can hold the link, else locally; returns what the line says.
        #[qinvokable]
        fn link_things(self: Pin<&mut Sioul>, from: &QString, to: &QString) -> QString;

        /// Undoes a tie wherever it is written; returns what the line says.
        #[qinvokable]
        fn unlink_things(self: Pin<&mut Sioul>, from: &QString, to: &QString) -> QString;

        /// Things to tie `from` to, matching `query`, of `kind` ("" for all), as JSON.
        #[qinvokable]
        fn search_things(self: &Sioul, query: &QString, kind: &QString, from: &QString) -> QString;

        /// The address of a thing known by its file or id: "mail" (a message's file), "note", "task", "event", "contact".
        #[qinvokable]
        fn uri_of(self: &Sioul, kind: &QString, id: &QString) -> QString;

        /// Whether a file, by its name, path or address, is one the desktop would start: a program, a script, an installer.
        #[qinvokable]
        fn is_program(self: &Sioul, name: &QString) -> bool;

        /// The window's first frame drawn: said with how long the start took, when timed (`crate::timing`).
        #[qinvokable]
        fn first_frame(self: &Sioul);

        /// A step of the start, said with how long the start took so far, when timed (`crate::timing`).
        #[qinvokable]
        fn mark(self: &Sioul, what: &QString);

        /// Whether this system keeps accounts Sioul may be shown (Android's: Murena, Google…).
        #[qinvokable]
        fn phone_accounts(self: &Sioul) -> bool;

        /// Android's own chooser of the phone's accounts; the one chosen comes back by `phone_account_chosen`.
        #[qinvokable]
        fn choose_phone_account(self: Pin<&mut Sioul>);

        /// A budget opened: its ledger for the period around `anchor` ("2026-10-05"),
        /// its balance by `step` ("day", "week", "month", "year"), as JSON.
        #[qinvokable]
        fn budget_detail(self: &Sioul, id: &QString, anchor: &QString, step: &QString) -> QString;

        /// A folder kept here, or left on the server only (its copy here goes);
        /// returns what went wrong, else "".
        #[qinvokable]
        fn keep_folder(self: Pin<&mut Sioul>, account: &QString, folder: &QString, kept: bool) -> QString;

        /// A new folder on the server; the status line says how it went.
        #[qinvokable]
        fn create_folder(self: Pin<&mut Sioul>, account: &QString, name: &QString);

        /// An empty folder taken off the server; the status line says how it went.
        #[qinvokable]
        fn delete_folder(self: Pin<&mut Sioul>, account: &QString, folder: &QString);

        /// A noise made here ("white", "pink", "brown"), as a file URL; made the first time.
        #[qinvokable]
        fn noise_url(self: &Sioul, kind: &QString) -> QString;

        /// Your recordings to rest by, from the notes' `sounds` folder, as JSON.
        #[qinvokable]
        fn calm_sounds(self: &Sioul) -> QString;

        /// A task moved to another list ("account/id"), its steps with it; returns
        /// {"uid"}, {"error"}, or {"losses"} to confirm first.
        #[qinvokable]
        fn move_task(self: Pin<&mut Sioul>, uid: &QString, list: &QString, confirmed: bool) -> QString;

        /// A contact moved to another address book ("account/id"); returns {"key"},
        /// {"error"}, or {"losses"} to confirm first.
        #[qinvokable]
        fn move_contact(self: Pin<&mut Sioul>, key: &QString, book: &QString, confirmed: bool) -> QString;

        /// The address books a contact can go into, as JSON.
        #[qinvokable]
        fn address_books(self: &Sioul) -> QString;

        /// Places by name for the weather; the answer comes in `places_found`.
        #[qinvokable]
        fn find_places(self: Pin<&mut Sioul>, name: &QString);

        /// The weather's place; an empty name takes the weather away.
        #[qinvokable]
        fn set_weather_place(self: Pin<&mut Sioul>, name: &QString, latitude: f64, longitude: f64);

        /// The health page: today's doses, the medicines, the prescriptions, as JSON.
        #[qinvokable]
        fn health_page(self: &Sioul) -> QString;

        /// A medicine made (`id` empty) or changed (JSON); returns {"id"} or {"error"}.
        #[qinvokable]
        fn save_medicine(self: Pin<&mut Sioul>, id: &QString, edit: &QString) -> QString;

        /// A prescription made (`id` empty) or changed (JSON); returns {"id"} or {"error"}.
        #[qinvokable]
        fn save_prescription(self: Pin<&mut Sioul>, id: &QString, edit: &QString) -> QString;

        /// A medicine or a prescription taken out; returns what went wrong, else "".
        #[qinvokable]
        fn remove_health(self: Pin<&mut Sioul>, id: &QString) -> QString;

        /// A prescription's medicines fetched today; returns what went wrong, else "".
        #[qinvokable]
        fn refilled(self: Pin<&mut Sioul>, id: &QString) -> QString;

        /// A dose marked taken, or not, as it happens: taken late or early, it
        /// may move the next ones (a medicine counted from its last dose).
        #[qinvokable]
        fn set_dose_taken(self: Pin<&mut Sioul>, key: &QString, taken: bool);

        /// A dose not taken, said afterwards: not asked again.
        #[qinvokable]
        fn dose_not_taken(self: Pin<&mut Sioul>, key: &QString);

        /// A dose taken late, at `time` ("09:30"); `move_next` moves the next
        /// doses of a medicine taken every few hours by as much. Returns what went wrong, else "".
        #[qinvokable]
        fn dose_taken_at(self: Pin<&mut Sioul>, key: &QString, time: &QString, move_next: bool) -> QString;

        /// What the late dose's question shows, as JSON.
        #[qinvokable]
        fn dose_info(self: &Sioul, key: &QString) -> QString;

        /// Meals, naps and the night, for the Health page, as JSON.
        #[qinvokable]
        fn needs(self: &Sioul) -> QString;

        /// Meals, naps and the night saved; returns what went wrong, else "".
        #[qinvokable]
        fn save_needs(self: Pin<&mut Sioul>, edit: &QString) -> QString;

        /// A block skipped today, or not; returns what went wrong, else "".
        #[qinvokable]
        fn skip_need(self: Pin<&mut Sioul>, key: &QString, skip: bool) -> QString;

        /// The doses due while Sioul was closed, for the Porch, as JSON: [{key, time, name, dose}].
        #[qinvokable]
        fn missed_doses(self: &Sioul) -> QString;

        /// You are at this window now: what follows you (medicines' reminders) comes here.
        #[qinvokable]
        fn touch(self: Pin<&mut Sioul>);

        /// Put away on a phone: what was marked goes out, and the other devices
        /// learn this one marks nothing until it is back (docs/health.md, "Knowing").
        #[qinvokable]
        fn going_away(self: Pin<&mut Sioul>);

        /// Back on a phone: the claims say so at once, before the next minute.
        #[qinvokable]
        fn back_here(self: Pin<&mut Sioul>);

        /// The pause to move, the chats' limit: "movement.minutes", "chats.enabled"…
        #[qinvokable]
        fn set_health(self: Pin<&mut Sioul>, key: &QString, value: &QString) -> QString;

        /// Minutes of focus before the pause to move; 0 when off.
        #[qinvokable]
        fn movement_minutes(self: &Sioul) -> i32;

        /// One more minute in a chat; whether chats are covered now.
        #[qinvokable]
        fn chat_minute(self: Pin<&mut Sioul>) -> bool;

        /// Whether chats are covered now.
        #[qinvokable]
        fn chats_covered(self: &Sioul) -> bool;

        /// Where a new audio memo is recorded, as a file URL; "" without a notes folder.
        #[qinvokable]
        fn memo_url(self: &Sioul) -> QString;

        /// A memo recorded: the notes are read again; returns its path in the notes.
        #[qinvokable]
        fn memo_recorded(self: Pin<&mut Sioul>, url: &QString) -> QString;

        /// A note (a picture, a PDF, a sound) moved to the vault's trash, "Undo"
        /// offered; returns what went wrong, else "".
        #[qinvokable]
        fn trash_note(self: Pin<&mut Sioul>, path: &QString) -> QString;

        /// A note renamed, what names it following; returns {"path"} or {"error"}.
        #[qinvokable]
        fn rename_note(self: Pin<&mut Sioul>, path: &QString, name: &QString) -> QString;

        /// A new note in a folder ("" for the notes' own); returns its path.
        #[qinvokable]
        fn create_note_in(self: Pin<&mut Sioul>, folder: &QString, title: &QString) -> QString;

        /// A new folder of notes in `parent`; returns what went wrong, else "".
        #[qinvokable]
        fn make_folder(self: Pin<&mut Sioul>, parent: &QString, name: &QString) -> QString;

        /// A folder renamed, its notes and their links following; returns {"path"} or {"error"}.
        #[qinvokable]
        fn rename_folder(self: Pin<&mut Sioul>, path: &QString, name: &QString) -> QString;

        /// An empty folder of notes taken out; returns what went wrong, else "".
        #[qinvokable]
        fn remove_folder(self: Pin<&mut Sioul>, path: &QString) -> QString;

        /// A case or project taken out of the list; returns what went wrong, else "".
        #[qinvokable]
        fn remove_project(self: Pin<&mut Sioul>, id: &QString) -> QString;

        /// A budget made (`id` empty) or changed (JSON); returns {"id"} or {"error"}.
        #[qinvokable]
        fn save_budget(self: Pin<&mut Sioul>, id: &QString, edit: &QString) -> QString;

        /// A budget, or one of its lines (by its address), taken out; returns what went wrong.
        #[qinvokable]
        fn remove_budget(self: Pin<&mut Sioul>, what: &QString) -> QString;

        /// Notes as a tree of folders, or as one list.
        #[qinvokable]
        fn set_notes_tree(self: Pin<&mut Sioul>, tree: bool);

        /// How a page was left ("sites-narrow", "notes-recent"…), kept between sessions.
        #[qinvokable]
        fn view_flag(self: &Sioul, name: &QString) -> bool;

        #[qinvokable]
        fn set_view_flag(self: Pin<&mut Sioul>, name: &QString, value: bool);


        /// A movement added by hand, once or recurring (JSON); returns what went wrong, else "".
        #[qinvokable]
        fn add_movement(self: Pin<&mut Sioul>, edit: &QString) -> QString;

        /// A project's billable time from one day to another, written as a
        /// spreadsheet (CSV) at a file address; returns what went wrong, else "".
        #[qinvokable]
        fn export_time_csv(self: Pin<&mut Sioul>, project: &QString, from: &QString, to: &QString, target: &QString) -> QString;

        /// Paper letters not done with, as JSON (`letters::view`).
        #[qinvokable]
        fn letters(self: &Sioul) -> QString;

        /// A paper letter's date as a task; returns what went wrong, else "".
        #[qinvokable]
        fn letter_task(self: Pin<&mut Sioul>, id: &QString) -> QString;

        /// A paper letter's appointment in the agenda; returns what went wrong, else "".
        #[qinvokable]
        fn letter_event(self: Pin<&mut Sioul>, id: &QString) -> QString;

        /// Done with a paper letter: its project set (if `project`), its scan filed; returns what went wrong, else "".
        #[qinvokable]
        fn letter_done(self: Pin<&mut Sioul>, id: &QString, project: &QString) -> QString;

        /// The money watch, as JSON (`bank::view`).
        #[qinvokable]
        fn bank(self: &Sioul) -> QString;

        /// A bank export taken in: {"ok", "said"}.
        #[qinvokable]
        fn import_bank(self: Pin<&mut Sioul>, file: &QString) -> QString;

        /// An export taken in as one bank account's; returns {"ok", "said"}.
        #[qinvokable]
        fn import_bank_into(self: Pin<&mut Sioul>, file: &QString, account: &QString) -> QString;

        /// A bank account made (`id` empty) or changed, from the dialog's JSON; returns what went wrong, if anything.
        #[qinvokable]
        fn save_bank_account(self: Pin<&mut Sioul>, id: &QString, edit: &QString) -> QString;

        #[qinvokable]
        fn remove_bank_account(self: Pin<&mut Sioul>, id: &QString) -> QString;

        /// One movement placed by hand: "budget:<id>", "none", or "" (as the rules say).
        #[qinvokable]
        fn place_movement(self: Pin<&mut Sioul>, account: &QString, movement: &QString, choice: &QString) -> QString;

        /// A rule for movements made (`place` below zero) or changed.
        #[qinvokable]
        fn save_bank_rule(self: Pin<&mut Sioul>, place: i32, edit: &QString) -> QString;

        #[qinvokable]
        fn remove_bank_rule(self: Pin<&mut Sioul>, place: i32) -> QString;

        /// A reserve made (`id` empty) or changed: its balance, its floor, its delay.
        #[qinvokable]
        fn save_reserve(self: Pin<&mut Sioul>, id: &QString, edit: &QString) -> QString;

        /// Contracts and subscriptions, as JSON (`contracts::view`).
        #[qinvokable]
        fn contracts(self: &Sioul) -> QString;

        /// A contract saved (new when `id` is empty) from the form's JSON; returns what went wrong, else "".
        #[qinvokable]
        fn save_contract(self: Pin<&mut Sioul>, id: &QString, edit: &QString) -> QString;

        /// A contract taken out; returns what went wrong, else "".
        #[qinvokable]
        fn remove_contract(self: Pin<&mut Sioul>, id: &QString) -> QString;

        /// The letter that stops a contract, drafted: {"draft"} or {"error"}.
        #[qinvokable]
        fn contract_letter(self: Pin<&mut Sioul>, id: &QString) -> QString;

        /// What a mail says of a contract (its subject, its sender), to start the form, as JSON.
        #[qinvokable]
        fn contract_from(self: &Sioul, subject: &QString, from: &QString) -> QString;

        /// The routines, as JSON: the admin window's, then yours (`work::routines`).
        #[qinvokable]
        fn routines(self: &Sioul) -> QString;

        /// A routine saved (new when `id` is empty): its title, its steps one a line, whether it moves on by itself.
        #[qinvokable]
        fn save_routine(self: Pin<&mut Sioul>, id: &QString, title: &QString, steps: &QString, by_itself: bool) -> QString;

        /// A routine taken out.
        #[qinvokable]
        fn remove_routine(self: Pin<&mut Sioul>, id: &QString) -> QString;

        /// Sharing with your other computers, as JSON (`share::status`); `folder` is the one being chosen, if any.
        #[qinvokable]
        fn share_status(self: &Sioul, folder: &QString) -> QString;

        /// Starts sharing through a folder, sealed with a passphrase (typed twice the first time); returns what went wrong, else "".
        #[qinvokable]
        fn start_sharing(self: Pin<&mut Sioul>, folder: &QString, passphrase: &QString, again: &QString) -> QString;

        /// Stops sharing on this computer; returns what went wrong, else "".
        #[qinvokable]
        fn stop_sharing(self: Pin<&mut Sioul>) -> QString;

        /// Exchanges with the other computers now.
        #[qinvokable]
        fn share_now(self: Pin<&mut Sioul>);

        /// Folders already shared through by your other devices (they hold a seal), as JSON: ["path", …].
        #[qinvokable]
        fn share_candidates(self: &Sioul) -> QString;

        /// Whether Sioul may read and write your files by their path (Android: "All files access").
        #[qinvokable]
        fn files_access(self: &Sioul) -> bool;

        /// Projects and budgets travel through the sharing too, or not; returns what went wrong, else "".
        #[qinvokable]
        fn set_share_projects(self: Pin<&mut Sioul>, on: bool) -> QString;

        /// A folder's own folders, for Sioul's folder browser, as JSON: {"path", "parent", "folders", "readable"}.
        #[qinvokable]
        fn folders_in(self: &Sioul, path: &QString) -> QString;

        /// Android: its own switch for "All files access" (or the older question before Android 11).
        #[qinvokable]
        fn ask_files_access(self: Pin<&mut Sioul>);

        /// A line of the budgets' file changed: label, amount, date; returns what went wrong, else "".
        #[qinvokable]
        fn change_line(self: Pin<&mut Sioul>, uri: &QString, label: &QString, amount: f64, date: &QString) -> QString;

        /// The sites kept in Sioul, as JSON.
        #[qinvokable]
        fn site_list(self: &Sioul) -> QString;

        /// A site's notification: shown at once in real time, else kept for the Porch.
        #[qinvokable]
        fn site_notified(self: Pin<&mut Sioul>, id: &QString, title: &QString, text: &QString);

        /// A site opened: what it notified is seen.
        #[qinvokable]
        fn site_seen(self: Pin<&mut Sioul>, id: &QString);

        /// What the sites notified, for the Porch, as JSON.
        #[qinvokable]
        fn site_notices(self: &Sioul) -> QString;

        /// The site a sender's mail announces: {"id", "name"}, else "".
        #[qinvokable]
        fn site_for_sender(self: &Sioul, sender: &QString) -> QString;

        /// A site's setting: "realtime", "muted", "background" (true or false), or "site" (its kind).
        #[qinvokable]
        fn set_site(self: Pin<&mut Sioul>, id: &QString, field: &QString, value: &QString) -> QString;

        /// The sites offered when adding one, for a country and a region ("" for yours): JSON.
        #[qinvokable]
        fn site_presets(self: &Sioul, country: &QString, region: &QString) -> QString;

        /// A site added: {"name", "url", "category", "area", "announced_by"}; returns {"id"} or {"error"}.
        #[qinvokable]
        fn add_site(self: Pin<&mut Sioul>, edit: &QString) -> QString;

        /// The usual sites as a menu: countries, their groups, the sites (JSON).
        #[qinvokable]
        fn site_presets_tree(self: &Sioul) -> QString;

        /// Several usual sites pinned at once (a JSON list); returns {"added", "error"}.
        #[qinvokable]
        fn add_sites(self: Pin<&mut Sioul>, rows: &QString) -> QString;

        /// A site moved one place up (-1) or down (1); returns what went wrong, else "".
        #[qinvokable]
        fn move_site(self: Pin<&mut Sioul>, id: &QString, delta: i32) -> QString;

        /// A site taken out of Sioul; its sign-ins stay in its profile until cleared.
        #[qinvokable]
        fn remove_site(self: Pin<&mut Sioul>, id: &QString) -> QString;

        /// Whether this build carries Sioul's own Google key: then no key of your own is needed.
        #[qinvokable]
        fn google_built_in(self: &Sioul) -> bool;

        /// The camera, microphone and speaker of calls in sites, by their names: {"camera", "microphone", "speaker"}.
        #[qinvokable]
        fn call_devices(self: &Sioul) -> QString;

        /// One of them chosen ("camera", "microphone", "speaker"); "" for the system's own.
        #[qinvokable]
        fn set_call_device(self: Pin<&mut Sioul>, which: &QString, name: &QString) -> QString;

        /// "missing", "unauthenticated", "locked" or "unlocked".
        #[qinvokable]
        fn bitwarden_state(self: &Sioul) -> QString;

        /// Unlocks Bitwarden; returns what went wrong, else "".
        #[qinvokable]
        fn bitwarden_unlock(self: Pin<&mut Sioul>, password: &QString, provider: i32, code: &QString) -> QString;

        /// Locks Bitwarden again: its logins leave memory.
        #[qinvokable]
        fn bitwarden_lock(self: Pin<&mut Sioul>);

        /// Sends the e-mail second step's code; returns what went wrong, else "".
        #[qinvokable]
        fn bitwarden_send_code(self: Pin<&mut Sioul>, password: &QString) -> QString;

        /// Begins a passkey login: {"page", "script"} for the key's window, or {"error"}.
        #[qinvokable]
        fn bitwarden_passkey_begin(self: Pin<&mut Sioul>) -> QString;

        /// The key's answer from that window: the vault opened with it; {"ok"} or {"error"}.
        #[qinvokable]
        fn bitwarden_passkey(self: Pin<&mut Sioul>, answer: &QString) -> QString;

        /// The logins for an address, and the vault's for a search: {"site", "matches", "found"} or {"error"}.
        #[qinvokable]
        fn bitwarden_logins(self: &Sioul, url: &QString, query: &QString) -> QString;

        /// One login of the vault, for an address: {"username", "password", "code"} or {"error"}.
        #[qinvokable]
        fn bitwarden_login(self: &Sioul, url: &QString, id: &QString) -> QString;

        /// Contacts on the map: {"allowed", "tiles", "pins", "waiting", "locating"}.
        #[qinvokable]
        fn map_view(self: &Sioul) -> QString;

        /// Where one address is: {"lat", "lon"}, else "".
        #[qinvokable]
        fn place_of(self: &Sioul, address: &QString) -> QString;

        /// Allows placing addresses (when `allow`), then places those not asked yet, one a second.
        #[qinvokable]
        fn locate_addresses(self: Pin<&mut Sioul>, allow: bool);

        /// Work time or quiet time again: called each minute; the pages follow when it changes.
        #[qinvokable]
        fn refresh_mode(self: Pin<&mut Sioul>);

        /// Work stays in view for `minutes` more, whatever the hours.
        #[qinvokable]
        fn work_a_while(self: Pin<&mut Sioul>, minutes: i32);

        /// Done for today: the day is closed at once ("Undo" in the status line),
        /// today's tasks flow on, work rests until it comes back. Returns the
        /// screen to show after, as JSON.
        #[qinvokable]
        fn done_for_the_day(self: Pin<&mut Sioul>) -> QString;

        /// The first step for when work comes back, in your words.
        #[qinvokable]
        fn set_first_step(self: Pin<&mut Sioul>, text: &QString);

        /// The first step put away.
        #[qinvokable]
        fn clear_first_step(self: Pin<&mut Sioul>);

        /// Back to the usual hours: overrides taken off.
        #[qinvokable]
        fn usual_hours(self: Pin<&mut Sioul>);

        /// "Work now", ticked or not: work shown, whatever the hours, until
        /// you untick it, Sioul closes, or the next working day is over.
        #[qinvokable]
        fn set_work_now(self: Pin<&mut Sioul>, on: bool);

        /// The cases and projects, for their list, as JSON.
        #[qinvokable]
        fn project_rows(self: &Sioul) -> QString;

        /// One project's page, as JSON.
        #[qinvokable]
        fn project_page(self: &Sioul, id: &QString) -> QString;

        /// Saves a case or project from its form (a new one when `id` is empty); returns {"id"} or {"error"}.
        #[qinvokable]
        fn save_project(self: Pin<&mut Sioul>, id: &QString, edit: &QString) -> QString;

        /// The Time page: "week", "month" or "year" around `anchor` ("2026-10-05"), one project or all.
        #[qinvokable]
        fn time_page(self: &Sioul, period: &QString, anchor: &QString, project: &QString) -> QString;

        /// Notes time by hand (JSON: day, at, minutes, project, task, note, unbilled); returns what went wrong.
        #[qinvokable]
        fn note_time(self: Pin<&mut Sioul>, edit: &QString) -> QString;

        /// Takes out time noted by hand; returns what went wrong.
        #[qinvokable]
        fn remove_time(self: Pin<&mut Sioul>, key: &QString) -> QString;

        /// The invoice of a project's unbilled hours: {"number", "html", "pdf"} or {"error"}.
        #[qinvokable]
        fn make_invoice(self: Pin<&mut Sioul>, id: &QString) -> QString;

        /// Invoices made on this computer from now on; returns what to say.
        #[qinvokable]
        fn take_invoices(self: Pin<&mut Sioul>) -> QString;

        /// An invoice made before, to print again: {"html", "pdf"} or {"error"}.
        #[qinvokable]
        fn invoice_again(self: &Sioul, number: &QString) -> QString;

        /// An invoice paid, or not; returns what went wrong.
        #[qinvokable]
        fn set_invoice_paid(self: Pin<&mut Sioul>, number: &QString, paid: bool) -> QString;

        /// Something new tied to `from`: "task", "note", "mail"; returns {"uid"}, {"path"}, {"draft"} or {"error"}.
        #[qinvokable]
        fn make_linked(self: Pin<&mut Sioul>, kind: &QString, from: &QString, key: &QString, start: f64) -> QString;

        /// A task from a message. Returns {"uid"} or {"error"}.
        #[qinvokable]
        fn task_from_mail(self: Pin<&mut Sioul>, key: &QString) -> QString;

        /// A note from a message; returns its path.
        #[qinvokable]
        fn note_from_mail(self: Pin<&mut Sioul>, key: &QString) -> QString;

        /// A note for an event's occurrence starting at `start`; returns its path.
        #[qinvokable]
        fn note_from_event(self: Pin<&mut Sioul>, key: &QString, start: f64) -> QString;

        /// A task to prepare an event. Returns {"uid"} or {"error"}.
        #[qinvokable]
        fn task_from_event(self: Pin<&mut Sioul>, key: &QString, start: f64) -> QString;

        /// A task from a note's checkbox line. Returns {"uid"} or {"error"}.
        #[qinvokable]
        fn task_from_line(self: Pin<&mut Sioul>, path: &QString, line: i32) -> QString;

        /// A draft to the people a task involves; returns its id.
        #[qinvokable]
        fn draft_for_task(self: Pin<&mut Sioul>, uid: &QString) -> QString;

        /// A draft sending a note to its event's guests; returns its id.
        #[qinvokable]
        fn mail_note(self: Pin<&mut Sioul>, path: &QString) -> QString;

        /// A page's settings ("porch", "mail", "agenda", "tasks", "notes", "reading", "contacts", "time", "general"), as JSON.
        #[qinvokable]
        fn settings(self: &Sioul, view: &QString) -> QString;

        /// Changes one setting (its value as JSON); returns what went wrong, else "".
        #[qinvokable]
        fn set_setting(self: Pin<&mut Sioul>, key: &QString, value: &QString) -> QString;

        /// "dark" or "light" when `SIOUL_THEME` forces one, for the window's images; else "" (the setting decides).
        #[qinvokable]
        fn forced_theme(self: &Sioul) -> QString;

        /// Real time on or off: every folder of every address fetched each minute, and the Porch open.
        #[qinvokable]
        fn set_realtime_mode(self: Pin<&mut Sioul>, on: bool);

        /// Images of a phone's screen (`SIOUL_GRAB_PHONE`): its size and its layout.
        #[qinvokable]
        fn grab_phone(self: &Sioul) -> bool;

        /// Which steps the window takes while saving images: `SIOUL_GRAB_STEPS`,
        /// "pages" by default, or "actions" to act on mail and send some (test accounts only).
        #[qinvokable]
        fn grab_steps(self: &Sioul) -> QString;
    }

    #[auto_cxx_name]
    unsafe extern "RustQt" {
        /// The notification's "copy" button was pressed.
        #[qsignal]
        fn copy_requested(self: Pin<&mut Sioul>, text: QString);

        /// An account was added: the form can be emptied.
        #[qsignal]
        fn account_added(self: Pin<&mut Sioul>);

        /// An account's password given here: kept (`problem` empty), else why not.
        #[qsignal]
        fn account_password_done(self: Pin<&mut Sioul>, id: QString, problem: QString);

        /// Paper letters were read: the Porch shows them again.
        #[qsignal]
        fn letters_changed(self: Pin<&mut Sioul>);

        /// A reminder's "Open" was pressed: what it is about ("task", "event", "budget"), shown.
        #[qsignal]
        fn reminder_opened(self: Pin<&mut Sioul>, kind: QString, uri: QString, key: QString);

        /// An attachment passed the antivirus: the desktop opens it.
        #[qsignal]
        fn open_url(self: Pin<&mut Sioul>, url: QString);

        /// No antivirus answered: the window asks before opening or saving, with how to install one.
        #[qsignal]
        fn scan_unavailable(self: Pin<&mut Sioul>, key: QString, index: i32, what: i32, name: QString, hint: QString);

        /// An attachment was kept in the papers wallet: the form, to say what it is.
        #[qsignal]
        fn paper_kept(self: Pin<&mut Sioul>, file: QString, title: QString, kind: QString);

        /// A draft's window opens again: "Undo" after sending or discarding.
        #[qsignal]
        fn compose_requested(self: Pin<&mut Sioul>, id: QString);

        /// Keys were found or made: windows showing them read them again.
        #[qsignal]
        fn keys_changed(self: Pin<&mut Sioul>);

        /// A tie was made or undone: what shows ties reads them again.
        #[qsignal]
        fn links_changed(self: Pin<&mut Sioul>);

        /// Addresses were placed on the map.
        #[qsignal]
        fn places_changed(self: Pin<&mut Sioul>);

        /// A site notified something, or was seen.
        #[qsignal]
        fn sites_changed(self: Pin<&mut Sioul>);

        /// An account chosen among the phone's: its name (an address, usually) and its kind
        /// ("com.google", "e.foundation.webdav.eelo"…); both empty when none was.
        #[qsignal]
        fn phone_account_chosen(self: Pin<&mut Sioul>, name: QString, kind: QString);
    }

    impl cxx_qt::Threading for Sioul {}
}

use qobject::Sioul;
pub(crate) type QtThread = cxx_qt::CxxQtThread<Sioul>;

/// Android: the window waiting for the phone's account chooser to answer.
#[cfg(target_os = "android")]
static PHONE_CHOOSER: std::sync::Mutex<Option<QtThread>> = std::sync::Mutex::new(None);

#[cfg(target_os = "android")]
unsafe extern "C" {
    /// Opens Android's chooser of the phone's accounts (android/main.cpp).
    fn sioul_android_choose_account();
    /// Whether Sioul may reach your files by their path ("All files access").
    fn sioul_android_files_access() -> bool;
    /// Android's own switch for it.
    fn sioul_android_ask_files_access();
}

/// Android: the account chosen in the phone's chooser, or two empty texts
/// when none was; called by android/main.cpp, on Android's thread.
///
/// # Safety
/// `name` and `kind` are null, or zero-terminated UTF-8 texts valid for the call.
#[cfg(target_os = "android")]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn sioul_android_account_chosen(name: *const std::ffi::c_char, kind: *const std::ffi::c_char) {
    // SAFETY: as the caller promises.
    let read = |text: *const std::ffi::c_char| if text.is_null() { String::new() } else { unsafe { std::ffi::CStr::from_ptr(text) }.to_string_lossy().into_owned() };
    let (name, kind) = (read(name), read(kind));
    let waiting = PHONE_CHOOSER.lock().ok().and_then(|mut w| w.take());
    if let Some(qt) = waiting {
        let _ = qt.queue(move |mut sioul| sioul.as_mut().phone_account_chosen(QString::from(&name), QString::from(&kind)));
    }
}

#[derive(Default)]
pub struct SioulRust {
    porch: QString,
    budgets: QString,
    accounts: QString,
    blocked: QString,
    status: QString,
    busy: bool,
    found: QString,
    scouted: QString,
    scouting: bool,
    form_error: QString,
    form_busy: bool,
    mail_accounts: QString,
    mail_folder: QString,
    drafts: QString,
    undo_line: QString,
    contacts: QString,
    agenda: QString,
    tasks: QString,
    notes: QString,
    focus_session: QString,
    reading: QString,
    realtime: bool,
    mode: QString,
    forecast: QString,
    places_found: QString,
    shared: Arc<Shared>,
}

/// What the background threads and the window share.
#[derive(Default)]
pub(crate) struct Shared {
    /// "Open it anyway", until "Done for now".
    opened_anyway: AtomicBool,
    /// Per account, the last thing its watcher said, and whether it is a problem.
    pub(crate) statuses: Mutex<BTreeMap<String, (String, bool)>>,
    /// Accounts whose password is wanted: none here yet, or refused (`SyncError::wants_password`).
    pub(crate) password_wanted: Mutex<BTreeSet<String>>,
    /// Per account, the newest message the window shows: what "Done for now" closes.
    shown: Mutex<BTreeMap<String, ImapOrigin>>,
    /// The running watchers.
    pub(crate) watchers: Mutex<BTreeMap<String, Arc<Control>>>,
    /// Accounts whose provider id was looked for this session.
    learned: Mutex<BTreeSet<String>>,
    /// Views are computed on threads and may finish out of order: only the newest is shown.
    generation: AtomicU64,
    shown_generation: AtomicU64,
    /// Attachments the antivirus found clean this session, by content.
    clean: Mutex<BTreeSet<u64>>,
    /// The folder the mail page shows.
    pub(crate) open_folder: Mutex<Option<mail::OpenFolder>>,
    /// What waits ten seconds for "Undo".
    pub(crate) pending: Mutex<Vec<Arc<mail::Pending>>>,
    pub(crate) mail_generation: AtomicU64,
    pub(crate) mail_shown_generation: AtomicU64,
    /// When the status line last said what became of something you did: a
    /// routine "Mail fetched at…" does not cover it for a while.
    said_at: Mutex<Option<std::time::Instant>>,
    /// The contacts' search and the agenda's days.
    pub(crate) pim: Mutex<pim::PimState>,
    pub(crate) pim_generation: AtomicU64,
    pub(crate) pim_shown_generation: AtomicU64,
    /// The running syncs of contacts and calendars, by account.
    pub(crate) dav_watchers: Mutex<BTreeMap<String, Arc<Control>>>,
    /// The task and notes pages' choices.
    pub(crate) work: Mutex<work::WorkState>,
    pub(crate) work_generation: AtomicU64,
    pub(crate) work_shown_generation: AtomicU64,
    /// Everything links reach, read after the last change.
    pub(crate) loaded: Mutex<Option<Arc<sioul_core::links::Loaded>>>,
    /// Real time: every folder each minute, the Porch open.
    pub(crate) realtime: AtomicBool,
    /// Addresses are being placed on the map.
    pub(crate) locating: AtomicBool,
    /// Bitwarden's session, once unlocked; in memory only.
    pub(crate) bitwarden: Mutex<Option<crate::sites::Vault>>,
    /// A forecast is being fetched.
    pub(crate) weather_fetching: AtomicBool,
    /// Accounts whose older mail waits for room on the disk, already said.
    pub(crate) held_back: Mutex<std::collections::BTreeSet<String>>,
    /// Cancel on the Google sign-in: its wait for the browser ends.
    pub(crate) google_stop: AtomicBool,
    /// GitHub's issues being brought, when asked.
    pub(crate) github: Mutex<Option<Arc<sioul_sync::Control>>>,
    /// When you were last at this window (Unix seconds): reminders follow you.
    pub(crate) active: std::sync::atomic::AtomicI64,
    /// When the plan was last made again on the clock (Unix seconds), and for which day.
    pub(crate) planned_at: std::sync::atomic::AtomicI64,
    pub(crate) planned_day: std::sync::atomic::AtomicI64,
    /// One computation at a time for each set of pages (`coalesced`).
    views_job: Job,
    pub(crate) mail_job: Job,
    pub(crate) pim_job: Job,
    pub(crate) work_job: Job,
}

/// The plan is made again every twelve hours, and when the day changes: a task
/// not done by its planned day moves on, and what waits for it follows.
const REPLAN_EVERY: i64 = 12 * 3600;

/// How long what you did stays in the status line before routine news.
const SAID_FOR: std::time::Duration = std::time::Duration::from_secs(30);

pub(crate) fn config_path() -> PathBuf {
    config::default_path()
}

pub(crate) fn load_config() -> Config {
    Config::load(&config_path()).unwrap_or_default()
}

/// One translator for the session: the configuration's language, else the session's.
pub(crate) fn tr() -> &'static Translator {
    static TRANSLATOR: OnceLock<Translator> = OnceLock::new();
    TRANSLATOR.get_or_init(|| Translator::new(&load_config().language.unwrap_or_else(session_language)))
}

/// The session's language: its variables (LC_ALL, LC_MESSAGES, LANG) when it
/// has them, as on Linux; else the system's own setting, as Qt reads it on
/// Windows and macOS, where a window started from the desktop gets none.
fn session_language() -> String {
    let said = ["LC_ALL", "LC_MESSAGES", "LANG"].iter().any(|v| std::env::var(v).is_ok_and(|l| !l.trim().is_empty()));
    if said {
        return i18n::system_language();
    }
    system_locale().unwrap_or_else(i18n::system_language)
}

/// Windows' language for the user ("fr-FR"), as `QLocale::system()` reads it.
#[cfg(windows)]
fn system_locale() -> Option<String> {
    #[link(name = "kernel32")]
    unsafe extern "system" {
        fn GetUserDefaultLocaleName(name: *mut u16, size: i32) -> i32;
    }
    // LOCALE_NAME_MAX_LENGTH characters, the final zero included.
    let mut name = [0u16; 85];
    // SAFETY: a buffer of `name.len()` UTF-16 characters, as the function asks; it writes no further.
    let written = unsafe { GetUserDefaultLocaleName(name.as_mut_ptr(), name.len() as i32) };
    let length = usize::try_from(written).ok().filter(|n| *n > 1 && *n <= name.len())?;
    Some(String::from_utf16_lossy(&name[..length - 1]))
}

/// The first of the languages chosen in macOS's settings ("fr-FR").
#[cfg(target_os = "macos")]
fn system_locale() -> Option<String> {
    // `defaults` prints them as a list: ( "fr-FR", "en-FR" ).
    let out = std::process::Command::new("defaults").args(["read", "-g", "AppleLanguages"]).output().ok()?;
    String::from_utf8_lossy(&out.stdout).split('"').nth(1).map(str::to_string).filter(|l| !l.is_empty())
}

/// Elsewhere the session's variables say it all.
#[cfg(not(any(windows, target_os = "macos")))]
fn system_locale() -> Option<String> {
    None
}

pub(crate) fn say(id: &str, pairs: &[(&str, String)]) -> String {
    let mut args = i18n::args();
    for (key, value) in pairs {
        // A whole number goes as a number, so that "one" and "other" choose by
        // it ("1 message", "2 messages"); one written otherwise ("007") stays as written.
        match value.parse::<i32>() {
            Ok(n) if n.to_string() == *value => args.set(key.to_string(), n),
            _ => args.set(key.to_string(), value.clone()),
        }
    }
    tr().text(id, Some(&args))
}

pub(crate) fn json(value: &impl serde::Serialize) -> String {
    serde_json::to_string(value).unwrap_or_default()
}

/// A file's address for Qt and the desktop: "file:///home/me/A%20b.pdf", and
/// "file:///C:/Users/me/A%20b.pdf" on Windows; what an address cannot hold as
/// is ("#", "?", spaces, accents) percent-encoded, so the file itself is reached.
pub(crate) fn file_url(path: &Path) -> String {
    fn encoded(path: &str) -> String {
        path.bytes().map(|b| if b.is_ascii_alphanumeric() || b"/-._~!$&'()*+,;=:@".contains(&b) { char::from(b).to_string() } else { format!("%{b:02X}") }).collect()
    }
    let text = path.to_string_lossy().into_owned();
    #[cfg(windows)]
    let text = windows_slashed(&text);
    match text.strip_prefix("//") {
        // "//server/share/…": a network share, its server named (Windows).
        Some(share) if cfg!(windows) => format!("file://{}", encoded(share)),
        _ => format!("file://{}", encoded(&text)),
    }
}

/// A Windows path as an address writes it: "C:\a" → "/C:/a", "\\server\s" →
/// "//server/s", without the "\\?\" `canonicalize` puts in front.
#[cfg(any(windows, test))]
fn windows_slashed(text: &str) -> String {
    let plain = match text.strip_prefix(r"\\?\UNC\") {
        Some(rest) => format!(r"\\{rest}"),
        None => text.strip_prefix(r"\\?\").unwrap_or(text).to_string(),
    };
    let slashed = plain.replace('\\', "/");
    if slashed.starts_with("//") { slashed } else { format!("/{slashed}") }
}

/// Back from an address to a Windows path: "/C:/a" → "C:\a", "server/s" → "\\server\s".
#[cfg(any(windows, test))]
fn windows_path(decoded: &str) -> String {
    let bytes = decoded.as_bytes();
    let path = if bytes.len() >= 3 && bytes[0] == b'/' && bytes[1].is_ascii_alphabetic() && bytes[2] == b':' {
        decoded[1..].to_string()
    } else if decoded.starts_with('/') {
        decoded.to_string()
    } else {
        format!("//{decoded}")
    };
    path.replace('/', "\\")
}

/// The file a `file://` address names (from a file dialog, a drop), its
/// percent-escapes undone; "file:///C:/…" is "C:\…" on Windows. Anything
/// else is a path already.
pub(crate) fn local_path(url: &str) -> PathBuf {
    let Some(rest) = url.strip_prefix("file://") else { return PathBuf::from(url) };
    // "file://localhost/…" is this computer too.
    let rest = rest.strip_prefix("localhost/").map_or_else(|| rest.to_string(), |r| format!("/{r}"));
    let decoded = crate::projects::percent_decoded(&rest);
    #[cfg(windows)]
    let decoded = windows_path(&decoded);
    PathBuf::from(decoded)
}

/// The lock of something done one at a time, when nothing holds it; a panic
/// in an earlier run (bad data in a file) does not stop it for the session.
pub(crate) fn one_at_a_time(lock: &Mutex<()>) -> Option<std::sync::MutexGuard<'_, ()>> {
    match lock.try_lock() {
        Ok(guard) => Some(guard),
        Err(std::sync::TryLockError::Poisoned(poisoned)) => Some(poisoned.into_inner()),
        Err(std::sync::TryLockError::WouldBlock) => None,
    }
}

/// A page computed on a thread, one computation at a time: asked again while
/// one runs, it runs once more after it, with everything changed meanwhile.
/// Fetches and changes ask often; a thread each would pile up on a slow disk.
#[derive(Default)]
pub(crate) struct Job {
    running: AtomicBool,
    again: AtomicBool,
}

/// Runs `work` on a thread for the job `which` picks out (see `Job`).
pub(crate) fn coalesced(shared: &Arc<Shared>, which: fn(&Shared) -> &Job, work: impl Fn(&Arc<Shared>) + Send + 'static) {
    let job = which(shared);
    job.again.store(true, Ordering::SeqCst);
    if job.running.swap(true, Ordering::SeqCst) {
        return;
    }
    let shared = Arc::clone(shared);
    std::thread::spawn(move || {
        let job = which(&shared);
        loop {
            while job.again.swap(false, Ordering::SeqCst) {
                // A panic on bad data leaves the next computations free to run.
                let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| work(&shared)));
            }
            job.running.store(false, Ordering::SeqCst);
            // Asked between the two lines above: once more, unless another thread took it.
            if !job.again.load(Ordering::SeqCst) || job.running.swap(true, Ordering::SeqCst) {
                break;
            }
        }
    });
}

/// How long text reads, for the window's theme.
pub(crate) fn reading_json() -> String {
    let config = load_config();
    let reading = config.reading;
    serde_json::json!({ "family": reading.family, "size": reading.size, "spacing": reading.spacing, "theme": config.theme.unwrap_or_default() }).to_string()
}

/// What the Porch and the budgets read: the configuration and its lists.
struct World {
    config: Config,
    store: Option<CaseStore>,
    known: SenderList,
    /// Who is safe, neutral or blocked.
    senders: porch::Senders,
}

impl World {
    fn load() -> World {
        let config = load_config();
        World {
            // With the mail tied to cases, so their conversations follow.
            store: config.case_store_path().and_then(|root| CaseStore::load(&root).ok()).map(|s| s.with_ties(&sioul_core::links::LocalLinks::load(&sioul_core::links::LocalLinks::default_path()))),
            known: KnownSenders::load(&config.known_senders_path()),
            senders: porch::Senders::load(&config),
            config,
        }
    }

    /// The mail waiting in the Porch; with `state` empty, all the mail kept.
    fn gather(&self, state: &PorchState) -> Vec<Triaged> {
        let now = Zoned::now().timestamp().as_second();
        porch::gather(&self.config.mail_sources(), self.store.as_ref(), &self.known, &self.senders, state, now)
    }

    fn ledger(&self) -> Option<Ledger> {
        // With the bank accounts' movements counted in their budgets.
        self.config.case_store_path().and_then(|root| Ledger::load_with_bank(&root).ok())
    }

    fn ledger_path(&self) -> Option<PathBuf> {
        self.config.case_store_path().map(|root| root.join(budget::LEDGER))
    }

    /// Every message about money, and what becomes of it.
    fn mail_lines(&self, ledger: &Ledger) -> Vec<MailLine> {
        let ignored = MoneyState::load(&MoneyState::default_path()).ignored;
        budget::mail_lines(ledger, &self.gather(&PorchState::default()), &ignored)
    }

    /// One message file judged as the Porch judges it, only inside the accounts' own mail.
    fn judge(&self, key: &str) -> Option<(PathBuf, Triaged)> {
        let path = maildir::locate(Path::new(key))?.canonicalize().ok()?;
        let sources = self.config.mail_sources();
        let inside = sources.iter().any(|s| s.folder.canonicalize().is_ok_and(|f| path.starts_with(f)));
        if !inside {
            return None;
        }
        let now = Zoned::now().timestamp().as_second();
        let canonical: Vec<_> = sources
            .into_iter()
            .map(|mut s| {
                s.folder = s.folder.canonicalize().unwrap_or(s.folder);
                s
            })
            .collect();
        let triaged = porch::judge(std::slice::from_ref(&path), &canonical, self.store.as_ref(), &self.known, &porch::Senders::default(), now);
        triaged.into_iter().next().map(|t| (path, t))
    }
}

/// Everything the window shows, computed off Qt's thread.
struct Views {
    porch: String,
    budgets: String,
    accounts: String,
    blocked: String,
    shown: BTreeMap<String, ImapOrigin>,
}

/// Whether Sioul stays off the network: `SIOUL_DEMO` set, for a demo profile
/// and its pictures. Nothing is fetched or sent by itself; what is kept on
/// this computer is shown.
pub(crate) fn offline() -> bool {
    std::env::var_os("SIOUL_DEMO").is_some()
}

/// Where the forecast is kept between fetches.
fn weather_cache() -> PathBuf {
    config::state_dir().join("weather.json")
}

/// A forecast is kept half an hour.
const WEATHER_FRESH: i64 = 30 * 60;

/// The weather applet, from the forecast kept, fetched again when it is half
/// an hour old; nothing without a place.
pub(crate) fn update_weather(qt: &QtThread, shared: &Arc<Shared>) {
    let config = load_config();
    let (Some(place), Some(latitude), Some(longitude)) = (config.weather.place.clone(), config.weather.latitude, config.weather.longitude) else {
        let _ = qt.queue(|mut sioul| sioul.as_mut().set_forecast(QString::default()));
        return;
    };
    let now = Zoned::now();
    let kept: Option<sioul_core::weather::Forecast> = std::fs::read_to_string(weather_cache()).ok().and_then(|t| serde_json::from_str(&t).ok());
    let shown = move |forecast: &sioul_core::weather::Forecast| {
        let view = sioul_core::weather::view(forecast, &Zoned::now(), tr());
        serde_json::json!({ "place": place, "view": view, "credit": tr().text("weather-credit", None) }).to_string()
    };
    if let Some(forecast) = kept.as_ref() {
        let line = shown(forecast);
        let _ = qt.queue(move |mut sioul| sioul.as_mut().set_forecast(QString::from(&line)));
    }
    let stale = kept.as_ref().is_none_or(|f| now.timestamp().as_second() - f.fetched >= WEATHER_FRESH);
    if !stale || offline() || shared.weather_fetching.swap(true, Ordering::Relaxed) {
        return;
    }
    let (qt, shared) = (qt.clone(), Arc::clone(shared));
    std::thread::spawn(move || {
        if let Ok(forecast) = sioul_sync::weather::forecast(latitude, longitude, Zoned::now().timestamp().as_second()) {
            if let Ok(text) = serde_json::to_string(&forecast) {
                let _ = std::fs::create_dir_all(config::state_dir());
                let _ = std::fs::write(weather_cache(), text);
            }
            let line = shown(&forecast);
            let _ = qt.queue(move |mut sioul| sioul.as_mut().set_forecast(QString::from(&line)));
        }
        shared.weather_fetching.store(false, Ordering::Relaxed);
    });
}

/// Whether work rests now.
pub(crate) fn quiet_now() -> bool {
    mode_now().quiet
}

/// The time now: what the hours are for, and until when (docs/areas.md).
pub(crate) fn mode_now() -> sioul_core::quiet::Mode {
    let config = load_config();
    let overrides = sioul_core::quiet::Overrides::load(&sioul_core::quiet::Overrides::default_path());
    sioul_core::quiet::mode(&config.week_hours(), &config.time_off, &overrides, &Zoned::now())
}

/// Whether something for `area` comes forward now.
pub(crate) fn in_view_now(area: sioul_core::areas::Area) -> bool {
    let mode = mode_now();
    sioul_core::areas::in_view(area, mode.time, mode.week)
}

/// Work time or quiet time, as the window shows it.
/// "Work now" taken back, as Sioul starts or closes; the rest of the overrides kept.
fn end_work_now() {
    let path = sioul_core::quiet::Overrides::default_path();
    let mut overrides = sioul_core::quiet::Overrides::load(&path);
    if overrides.work_now.take().is_some()
        && let Err(e) = overrides.save(&path)
    {
        eprintln!("{e}");
    }
}

pub(crate) fn mode_json() -> String {
    let config = load_config();
    let now = Zoned::now();
    let overrides = sioul_core::quiet::Overrides::load(&sioul_core::quiet::Overrides::default_path());
    let mode = sioul_core::quiet::mode(&config.week_hours(), &config.time_off, &overrides, &now);
    let until = mode.until.as_ref().map(|z| sioul_core::quiet::until_text(tr(), z, &now)).unwrap_or_default();
    let mut args = sioul_core::i18n::args();
    args.set("until", until.clone());
    args.set("label", if mode.label.trim().is_empty() { "none".to_string() } else { mode.label.clone() });
    let reason = serde_json::to_value(&mode.reason).ok().and_then(|v| v.as_str().map(str::to_string)).unwrap_or_default();
    let line = match mode.reason {
        sioul_core::quiet::Reason::NoHours | sioul_core::quiet::Reason::Working => String::new(),
        sioul_core::quiet::Reason::TimeOff => tr().text("mode-time-off", Some(&args)),
        sioul_core::quiet::Reason::WorkingLate => tr().text("mode-working-late", Some(&args)),
        sioul_core::quiet::Reason::WorkNow => tr().text("mode-work-now-line", Some(&args)),
        // Admin hours within free time: both said.
        sioul_core::quiet::Reason::AdminTime if matches!(mode.time, sioul_core::areas::Time::Several(open) if open.leisure) => tr().text("mode-admin-leisure", Some(&args)),
        sioul_core::quiet::Reason::AdminTime => tr().text("mode-admin", Some(&args)),
        sioul_core::quiet::Reason::LeisureTime => tr().text("mode-leisure", Some(&args)),
        // Evening, night, a day without hours: rest, until the next hours of any kind.
        _ if mode.rests() => tr().text("mode-rest", Some(&args)),
        // A day closed early.
        _ => tr().text("mode-quiet", Some(&args)),
    };
    // The day closed today can be taken back, that day only.
    let today = overrides.closed_today(&now);
    // Which hours are set: the Porch asks for them while none are.
    let set = |kind: &str| config.windows.iter().any(|w| w.kind() == kind);
    serde_json::json!({ "quiet": mode.quiet, "rest": mode.rests(), "time": mode.time.id(), "reason": reason, "until": until, "line": line, "hours": !config.week_hours().is_empty(), "work_hours": set("work"), "admin_hours": set("admin"), "leisure_hours": set("leisure"), "work_now": mode.reason == sioul_core::quiet::Reason::WorkNow, "today": today }).to_string()
}

fn compute(shared: &Shared) -> Views {
    let world = World::load();
    let now = Zoned::now();
    let hidden = mail::hidden_files(shared);
    let mut items = world.gather(&PorchState::load(&PorchState::default_path()));
    items.retain(|t| t.card.path.as_ref().is_none_or(|p| !hidden.contains(p)));
    // What the hours are for: codes and the senders you marked safe always; the
    // rest as its address is for (an address you did not say: work's).
    let overrides = sioul_core::quiet::Overrides::load(&sioul_core::quiet::Overrides::default_path());
    let mode = sioul_core::quiet::mode(&world.config.week_hours(), &world.config.time_off, &overrides, &now);
    if mode.time != sioul_core::areas::Time::Any {
        let area_of = |t: &sioul_core::porch::Triaged| t.card.account.as_deref().and_then(|id| world.config.account(id)).and_then(|a| a.area.as_deref()).and_then(sioul_core::areas::Area::parse).unwrap_or(sioul_core::areas::Area::WORK);
        items.retain(|t| sioul_core::quiet::mail_in_view(t, &world.senders, area_of(t), mode.time, mode.week));
    }
    // At rest no project shows: what your safe senders wrote about one comes among the people you know.
    if mode.rests() {
        for t in &mut items {
            if matches!(t.lane, sioul_core::porch::Lane::Case(_)) {
                t.lane = sioul_core::porch::Lane::People;
            }
        }
    }
    let mut porch = view::porch(&items, &world.config, world.store.as_ref(), tr(), &now, shared.opened_anyway.load(Ordering::Relaxed) || mode.quiet);
    if mode.quiet {
        porch.status = serde_json::from_str::<serde_json::Value>(&mode_json()).ok().and_then(|v| v["line"].as_str().map(str::to_string)).unwrap_or_default();
    }
    // Closed, the Porch shows only codes: "Done for now" has nothing to close.
    let shown = if porch.open { PorchState::newest_shown(&items) } else { BTreeMap::new() };
    let budgets = world.ledger().map_or_else(String::new, |mut ledger| {
        // Budgets as their area goes: leisure money ("personal"), work's, the rest admin.
        if mode.time != sioul_core::areas::Time::Any {
            // "personal" was the word for leisure money before areas had three.
            let area_of = |b: &sioul_core::budget::Budget| match b.area.as_deref() {
                Some("personal") => sioul_core::areas::Area::LEISURE,
                Some(other) => sioul_core::areas::Area::parse(other).unwrap_or(sioul_core::areas::Area::ADMIN),
                None => sioul_core::areas::Area::ADMIN,
            };
            ledger.budgets.retain(|b| sioul_core::areas::in_view(area_of(b), mode.time, mode.week));
        }
        let mail = world.mail_lines(&ledger);
        json(&view::budgets(&ledger, &mail, tr(), now.date()))
    });
    let statuses = shared.statuses.lock().map(|s| s.clone()).unwrap_or_default();
    let wanted = shared.password_wanted.lock().map(|w| w.clone()).unwrap_or_default();
    Views {
        porch: json(&porch),
        budgets,
        accounts: json(&view::accounts(&world.config, tr(), &statuses, &wanted)),
        blocked: json(&world.senders.blocked.entries()),
        shown,
    }
}

/// Computes the views on a thread and shows them, unless newer ones came first.
pub(crate) fn show(qt: &QtThread, shared: &Arc<Shared>) {
    mail::show_mail(qt, shared);
    let qt = qt.clone();
    coalesced(shared, |s| &s.views_job, move |shared| {
        let generation = shared.generation.fetch_add(1, Ordering::Relaxed) + 1;
        let views = compute(shared);
        let _ = qt.queue(move |mut sioul| {
            let shared = Arc::clone(&sioul.rust().shared);
            if shared.shown_generation.fetch_max(generation, Ordering::Relaxed) > generation {
                return;
            }
            if let Ok(mut shown) = shared.shown.lock() {
                *shown = views.shown;
            }
            sioul.as_mut().set_porch(QString::from(&views.porch));
            sioul.as_mut().set_budgets(QString::from(&views.budgets));
            sioul.as_mut().set_accounts(QString::from(&views.accounts));
            sioul.as_mut().set_blocked(QString::from(&views.blocked));
        });
    });
}

pub(crate) fn set_status(qt: &QtThread, line: String) {
    let _ = qt.queue(move |mut sioul| sioul.as_mut().set_status(QString::from(&line)));
}

/// Says what became of something you did; it stays a while (`SAID_FOR`).
pub(crate) fn tell(qt: &QtThread, shared: &Shared, line: String) {
    if let Ok(mut said) = shared.said_at.lock() {
        *said = Some(std::time::Instant::now());
    }
    set_status(qt, line);
}

/// Starts a watcher for every mail account that has none running.
fn start_watchers(qt: &QtThread, shared: &Arc<Shared>) -> usize {
    if offline() {
        return 0;
    }
    let accounts: Vec<Account> = load_config().accounts.into_iter().filter(Account::syncs).collect();
    let count = accounts.len();
    for account in accounts {
        start_watcher(qt, shared, account);
    }
    count
}

/// Syncs for the accounts not started yet this session: those another device's
/// settings just brought. One that stopped (a password refused) is not tried
/// again here: retries can lock an account.
pub(crate) fn start_new_watchers(qt: &QtThread, shared: &Arc<Shared>) {
    if offline() {
        return;
    }
    let seen = |id: &str| shared.statuses.lock().is_ok_and(|s| s.contains_key(id)) || shared.watchers.lock().is_ok_and(|w| w.contains_key(id)) || shared.dav_watchers.lock().is_ok_and(|w| w.contains_key(id));
    for account in load_config().accounts.into_iter().filter(|a| !seen(&a.id)) {
        if account.is_dav() {
            crate::pim::start_dav_watcher(qt, shared, account);
        } else if account.syncs() {
            start_watcher(qt, shared, account);
        }
    }
}

/// An account's password given on this device: tested with its server (its
/// mail, or its calendars and contacts), kept in the keyring, its sync started
/// again; `account_password_done` says how it went.
fn give_password(qt: &QtThread, shared: &Arc<Shared>, id: String, password: String) {
    let (qt, shared) = (qt.clone(), Arc::clone(shared));
    std::thread::spawn(move || {
        let config = load_config();
        let outcome = match config.every_account().find(|a| a.id == id).cloned() {
            None => Err(say("account-unknown", &[("id", id.clone())])),
            Some(account) => {
                let host = account.host.clone().unwrap_or_default();
                let password = sioul_sync::tidy_password(&host, &password);
                let tested = if account.is_dav() {
                    let address = account.address.clone().unwrap_or_default();
                    let login = account.login().unwrap_or(address.as_str()).to_string();
                    sioul_sync::dav::test(&address, &login, &password, account.url.as_deref(), None).map(|_| ())
                } else {
                    sioul_sync::test(&account, &password).map(|_| ())
                };
                tested.and_then(|()| secret::save(&account, &password)).map_err(|e| e.sentence(tr(), &account.id)).map(|()| account)
            }
        };
        let problem = match outcome {
            Ok(account) => {
                want_password(&shared, &account.id, None);
                if let Ok(mut statuses) = shared.statuses.lock() {
                    statuses.remove(&account.id);
                }
                if account.is_dav() {
                    crate::pim::start_dav_watcher(&qt, &shared, account);
                } else {
                    start_watcher(&qt, &shared, account);
                }
                String::new()
            }
            Err(e) => e,
        };
        let _ = qt.queue(move |mut sioul| {
            sioul.as_mut().set_form_busy(false);
            if problem.is_empty() {
                sioul.as_mut().set_status(QString::from(&say("account-password-kept", &[("account", id.clone())])));
            }
            sioul.as_mut().account_password_done(QString::from(&id), QString::from(&problem));
        });
        show(&qt, &shared);
    });
}

/// A mail account added: tested with its password (given, or, `None`, the one
/// its address's calendars and contacts use), then kept; its watcher started.
#[allow(clippy::too_many_arguments)]
fn add_mail(qt: &QtThread, shared: &Arc<Shared>, address: String, host: String, port: u16, security: Security, login: String, password: Option<String>) {
    let (qt, shared) = (qt.clone(), Arc::clone(shared));
    std::thread::spawn(move || {
        let config = load_config();
        // The same address may have calendars and contacts already: only its mail counts.
        let outcome = match config.every_account().find(|a| a.address.as_deref() == Some(address.as_str()) && matches!(a.kind, config::AccountKind::Imap | config::AccountKind::Jmap)) {
            Some(existing) => Err(say("account-address-exists", &[("address", address.clone()), ("id", existing.id.clone())])),
            None => {
                let password = match password {
                    Some(p) => Ok(p),
                    None => config
                        .every_account()
                        .find(|a| a.address.as_deref() == Some(address.as_str()) && a.kind == config::AccountKind::Dav)
                        .ok_or_else(|| say("account-unknown", &[("id", address.clone())]))
                        .and_then(|dav| secret::password(dav).map_err(|e| e.sentence(tr(), &dav.id))),
                };
                password.and_then(|password| {
                    let id = config.free_id(&address);
                    let account = Account::imap(&id, &address, &host, port, security, Some(&login));
                    let password = sioul_sync::tidy_password(&host, &password);
                    sioul_sync::test(&account, &password)
                        .and_then(|_| secret::save(&account, &password))
                        .map_err(|e| e.sentence(tr(), &id))
                        .and_then(|()| config::add_imap_account(&config_path(), &account))
                        .map(|()| account)
                })
            }
        };
        let added = outcome.as_ref().ok().map(|a| say("account-added", &[("id", a.id.clone()), ("path", a.maildir_path().display().to_string())]));
        if let Ok(account) = outcome.as_ref() {
            start_watcher(&qt, &shared, account.clone());
        }
        let error = outcome.err();
        let _ = qt.queue(move |mut sioul| {
            sioul.as_mut().set_form_busy(false);
            match (added, error) {
                (Some(line), _) => {
                    sioul.as_mut().set_found(QString::default());
                    sioul.as_mut().set_scouted(QString::default());
                    sioul.as_mut().set_status(QString::from(&line));
                    sioul.as_mut().account_added();
                }
                (None, Some(e)) => sioul.as_mut().set_form_error(QString::from(&e)),
                (None, None) => {}
            }
        });
        show(&qt, &shared);
    });
}

/// The pace of an address: its own, else everyone's.
fn pace_of(config: &Config, account: &Account) -> std::time::Duration {
    let minutes = account.fetch_minutes.or(config.fetch_minutes).unwrap_or(config::DEFAULT_FETCH_MINUTES);
    std::time::Duration::from_secs(u64::from(minutes.max(1)) * 60)
}

/// Gives every running watcher its pace from the configuration, and real time.
pub(crate) fn update_paces(shared: &Shared) {
    let config = load_config();
    let realtime = shared.realtime.load(Ordering::Relaxed);
    let Ok(watchers) = shared.watchers.lock() else { return };
    for account in config.accounts.iter() {
        if let Some(control) = watchers.get(&account.id) {
            control.set_pace(pace_of(&config, account));
            control.set_realtime(realtime);
        }
    }
}

fn start_watcher(qt: &QtThread, shared: &Arc<Shared>, account: Account) {
    let control = Arc::new(Control::default());
    control.set_pace(pace_of(&load_config(), &account));
    control.set_realtime(shared.realtime.load(Ordering::Relaxed));
    {
        let Ok(mut watchers) = shared.watchers.lock() else { return };
        if let Some(running) = watchers.get(&account.id) {
            // Already watching: fetch now instead.
            running.nudge();
            return;
        }
        watchers.insert(account.id.clone(), Arc::clone(&control));
    }
    let (qt, shared) = (qt.clone(), Arc::clone(shared));
    std::thread::spawn(move || {
        match secret::password(&account) {
            Ok(password) => sioul_sync::watch(&account, &password, &control, |result| reported(&qt, &shared, &account, result)),
            Err(e) => reported(&qt, &shared, &account, Err(e)),
        }
        // Gone, unless another watcher replaced this one meanwhile.
        if let Ok(mut watchers) = shared.watchers.lock()
            && watchers.get(&account.id).is_some_and(|c| Arc::ptr_eq(c, &control))
        {
            watchers.remove(&account.id);
        }
    });
}

/// Notes whether an account's password is wanted after what its sync said:
/// none here, or refused. Its card then offers to give one.
pub(crate) fn want_password(shared: &Shared, id: &str, error: Option<&SyncError>) {
    if let Ok(mut wanted) = shared.password_wanted.lock() {
        if error.is_some_and(SyncError::wants_password) {
            wanted.insert(id.to_string());
        } else {
            wanted.remove(id);
        }
    }
}

/// What a watcher said: the account's status, the provider's id to learn,
/// notifications for new codes, and the views again.
fn reported(qt: &QtThread, shared: &Arc<Shared>, account: &Account, result: Result<Report, SyncError>) {
    let (line, problem) = match &result {
        Ok(report) if report.first => {
            let mut args = tr().counted(report.new.len());
            args.set("account", account.id.clone());
            args.set("days", account.sync_days.unwrap_or(config::DEFAULT_SYNC_DAYS));
            (tr().text("sync-first", Some(&args)), false)
        }
        Ok(_) => (say("ui-synced-at", &[("time", Zoned::now().strftime("%H:%M").to_string())]), false),
        Err(e) => (e.sentence(tr(), &account.id), true),
    };
    if let Ok(mut statuses) = shared.statuses.lock() {
        statuses.insert(account.id.clone(), (line.clone(), problem));
    }
    want_password(shared, &account.id, result.as_ref().err());
    if let Ok(report) = &result {
        if !report.new.is_empty() {
            learn(qt, shared, &account.id);
            tie_to_cases(&report.new);
        }
        if !report.elsewhere.is_empty() {
            tie_to_cases(&report.elsewhere);
        }
        // Older mail waits for room: said once a session, calmly.
        if report.held_back
            && shared.held_back.lock().is_ok_and(|mut said| said.insert(account.id.clone()))
        {
            let free = sioul_sync::disk::space(&account.maildir_path()).map_or(0, |(free, _)| free);
            set_status(qt, say("sync-held-back", &[("account", account.id.clone()), ("free", tr().decimal((free as f64 / f64::from(1u32 << 30)) as f32))]));
        }
        if !report.first && !report.new.is_empty() {
            notify_codes(qt, &report.new);
        }
        read_shielded(qt, &account.id);
    }
    // Routine news waits while the line says what became of something you did.
    let recent = shared.said_at.lock().ok().and_then(|s| *s).is_some_and(|at| at.elapsed() < SAID_FOR);
    let quiet = recent && !problem;
    let _ = qt.queue(move |mut sioul| {
        sioul.as_mut().set_busy(false);
        if !quiet {
            sioul.as_mut().set_status(QString::from(&line));
        }
    });
    show(qt, shared);
}

/// Mail just fetched that a case takes, by its routes or its conversation:
/// tied to the case, so its project shows it whatever its routes need, and
/// the replies that come later follow it.
fn tie_to_cases(files: &[PathBuf]) {
    let config = load_config();
    let path = sioul_core::links::LocalLinks::default_path();
    let mut ties = sioul_core::links::LocalLinks::load(&path);
    let before = ties.links.len();
    // GitHub's notification mail, to its issue's task.
    crate::github::tie_mail(&config, files, &mut ties);
    if let Some(store) = config.case_store_path().and_then(|root| CaseStore::load(&root).ok()).map(|s| s.with_ties(&ties)) {
        let senders = porch::Senders::load(&config);
        for t in porch::judge(files, &config.mail_sources(), Some(&store), &KnownSenders::default(), &senders, Zoned::now().timestamp().as_second()) {
            if let (porch::Lane::Case(id), Some(mid)) = (&t.lane, t.card.message_id.as_deref()) {
                ties.add(&sioul_core::links::mail_uri(mid), &sioul_core::links::case_uri(id), "case");
            }
        }
    }
    if ties.links.len() != before {
        let _ = ties.save(&path);
    }
}

/// A case's routes changed: the mail already here that only its text or an
/// attachment ties to it, tied now, on a thread (routes on the sender and the
/// subject are matched again each time the project shows).
fn retie_case(id: String) {
    std::thread::spawn(move || {
        let config = load_config();
        let Some(case) = config.case_store_path().and_then(|root| CaseStore::load(&root).ok()).and_then(|s| s.get(&id).cloned()) else { return };
        let routes: Vec<&sioul_core::cases::Route> = case.routes.iter().filter(|r| r.needs_body()).collect();
        if routes.is_empty() {
            return;
        }
        let path = sioul_core::links::LocalLinks::default_path();
        let mut ties = sioul_core::links::LocalLinks::load(&path);
        let before = ties.links.len();
        for account in config.accounts.iter().filter(|a| a.syncs()) {
            for file in sioul_core::mailindex::message_files(&account.maildir_path()) {
                let Some(card) = maildir::read_one(&file) else { continue };
                if let Some(mid) = card.message_id.as_deref()
                    && routes.iter().any(|r| r.explain(&card).is_some())
                {
                    ties.add(&sioul_core::links::mail_uri(mid), &sioul_core::links::case_uri(&case.id), "case");
                }
            }
        }
        if ties.links.len() != before {
            let _ = ties.save(&path);
        }
    });
}

/// A shielded address that allows the AI: what it has not read yet, read now,
/// before the Porch shows it again. Nothing is sent without a key.
fn read_shielded(qt: &QtThread, id: &str) {
    let config = load_config();
    if !config.account(id).is_some_and(|a| a.shield && a.shield_ai) {
        return;
    }
    if let Err(e) = sioul_sync::shield_ai::read_new(&config) {
        set_status(qt, e.sentence(tr(), "Anthropic"));
    }
}

/// Learns the provider's authserv-id once per session, while it is unknown.
fn learn(qt: &QtThread, shared: &Shared, id: &str) {
    if shared.learned.lock().map_or(true, |l| l.contains(id)) {
        return;
    }
    let Some(account) = load_config().account(id).cloned() else { return };
    match sioul_sync::learn_provider(&config_path(), &account) {
        Ok(Learned::TooEarly) => {}
        Ok(learned) => {
            if let Ok(mut done) = shared.learned.lock() {
                done.insert(id.to_string());
            }
            match learned {
                Learned::Id(provider) => set_status(qt, say("sync-learned", &[("account", id.to_string()), ("id", provider)])),
                Learned::Nothing => set_status(qt, say("sync-not-learned", &[("account", id.to_string())])),
                _ => {}
            }
        }
        Err(e) => set_status(qt, e),
    }
}

/// One quiet notification per verified code among new mail, with a copy button.
fn notify_codes(qt: &QtThread, files: &[PathBuf]) {
    let config = load_config();
    let now = Zoned::now().timestamp().as_second();
    let senders = porch::Senders::load(&config);
    // Codes are decided before cases and screening: neither is needed here.
    let judged = porch::judge(files, &config.mail_sources(), None, &KnownSenders::default(), &senders, now);
    for code in view::codes(&judged, tr()) {
        let copy = code.code.clone().map(|text| {
            let qt = qt.clone();
            let action: Box<dyn FnOnce() + Send> = Box::new(move || {
                let _ = qt.queue(move |sioul| sioul.copy_requested(QString::from(&text)));
            });
            (tr().text("notify-copy", None), action)
        });
        if let Err(e) = notify::code(&code.title, &code.body(), copy) {
            set_status(qt, e);
        }
    }
}

/// Where attachments are written to be opened: `<cache>/sioul/attachments/<message>/`.
fn attachment_cache(key: &str) -> PathBuf {
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    key.hash(&mut hasher);
    config::cache_dir().join("attachments").join(format!("{:016x}", hasher.finish()))
}

/// Attachments written to be opened in an earlier session, taken out of the
/// cache once a day old: a decrypted one would stay readable there otherwise.
fn forget_opened_attachments() {
    let Ok(entries) = std::fs::read_dir(config::cache_dir().join("attachments")) else { return };
    let day = std::time::Duration::from_secs(86_400);
    for entry in entries.filter_map(Result::ok) {
        if entry.metadata().and_then(|m| m.modified()).ok().and_then(|t| t.elapsed().ok()).is_some_and(|age| age > day) {
            let _ = std::fs::remove_dir_all(entry.path());
        }
    }
}

/// A file come by mail marked as the system marks what comes from the
/// Internet: the Mark of the Web on Windows, the quarantine on macOS. Office
/// then opens it in Protected View, Gatekeeper checks it. A disk that cannot
/// keep the mark leaves the file as it is.
fn mark_from_mail(file: &Path) {
    #[cfg(windows)]
    {
        let mut stream = file.as_os_str().to_owned();
        stream.push(":Zone.Identifier");
        let _ = std::fs::write(stream, "[ZoneTransfer]\r\nZoneId=3\r\n");
    }
    #[cfg(target_os = "macos")]
    {
        let stamp = format!("0081;{:x};Sioul;", jiff::Timestamp::now().as_second());
        let _ = std::process::Command::new("xattr").args(["-w", "com.apple.quarantine", &stamp]).arg(file).output();
    }
    #[cfg(not(any(windows, target_os = "macos")))]
    let _ = file;
}

/// The downloads folder the desktop names.
fn downloads() -> PathBuf {
    config::downloads_dir()
}

/// `name`, else "name (2).ext", "name (3).ext"… so nothing is overwritten.
fn free_path(folder: &Path, name: &str) -> PathBuf {
    let wanted = folder.join(name);
    if !wanted.exists() {
        return wanted;
    }
    let (stem, extension) = match name.rsplit_once('.') {
        Some((stem, ext)) if !stem.is_empty() => (stem.to_string(), format!(".{ext}")),
        _ => (name.to_string(), String::new()),
    };
    (2..).map(|n| folder.join(format!("{stem} ({n}){extension}"))).find(|p| !p.exists()).unwrap_or(wanted)
}

/// An attachment's name as one file in a folder. The core takes any path out
/// of it; Windows also reads "C:x" as a drive, "a:b" as a hidden stream and
/// "CON", "NUL"… as devices: those characters become "_", those names get one
/// in front, and the dots and spaces it would drop at the end go.
fn one_file_name(name: &str) -> String {
    let odd = |c: char| matches!(c, '/' | '\\') || (cfg!(windows) && matches!(c, ':' | '<' | '>' | '"' | '|' | '?' | '*'));
    let mut name: String = name.chars().map(|c| if odd(c) { '_' } else { c }).collect();
    if cfg!(windows) {
        name = name.trim_end_matches(['.', ' ']).to_string();
        let stem = name.split('.').next().unwrap_or("").trim_end().to_ascii_uppercase();
        let numbered = stem.len() == 4 && (stem.starts_with("COM") || stem.starts_with("LPT")) && stem.as_bytes()[3].is_ascii_digit();
        if numbered || matches!(stem.as_str(), "CON" | "PRN" | "AUX" | "NUL") {
            name.insert(0, '_');
        }
    }
    if matches!(name.as_str(), "" | "." | "..") { "attachment".into() } else { name }
}

/// What happens to an attachment once the antivirus found it clean. One the
/// desktop would run rather than show (`links::is_program`: programs, scripts,
/// shortcuts, installers, disk images) is only saved.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Afterwards {
    Open,
    Save,
    Paper,
}

impl Afterwards {
    fn of(what: i32) -> Afterwards {
        match what {
            1 => Afterwards::Save,
            2 => Afterwards::Paper,
            _ => Afterwards::Open,
        }
    }
}

/// Writes an attachment to the cache, has the antivirus check it, then opens or
/// saves it; a threat, or no antivirus, and the copy is deleted.
fn checked_attachment(qt: &QtThread, shared: &Arc<Shared>, key: String, index: i32, afterwards: Afterwards, unchecked: bool) {
    let (qt, shared) = (qt.clone(), Arc::clone(shared));
    std::thread::spawn(move || {
        let world = World::load();
        // Set aside, a message's attachments are not opened.
        let judged = world.judge(&key).filter(|(_, t)| t.lane != porch::Lane::SetAside);
        let subject = judged.as_ref().map(|(_, t)| t.card.subject.clone()).unwrap_or_default();
        let found = judged.and_then(|(path, _)| reading::attachment_from(&crypto::readable(&path)?, u32::try_from(index).ok()?));
        let Some((name, bytes)) = found else {
            set_status(&qt, tr().text("ui-attachments-closed", None));
            return;
        };
        let name = one_file_name(&name);
        // A program is never started from a mail, checked or not: an antivirus
        // may not know a new one yet. Saved, it is yours to run.
        if afterwards != Afterwards::Save && sioul_core::links::is_program(&name) {
            set_status(&qt, say("attachment-program", &[("name", name)]));
            return;
        }
        let folder = attachment_cache(&key);
        let file = folder.join(&name);
        if std::fs::create_dir_all(&folder).and_then(|()| std::fs::write(&file, &bytes)).is_err() {
            set_status(&qt, say("scan-error", &[("name", name), ("detail", folder.display().to_string())]));
            return;
        }
        mark_from_mail(&file);
        set_status(&qt, say("scan-running", &[("name", name.clone())]));
        // Keyed anew each session: a file cannot be made to pass for one found clean.
        static KEYS: OnceLock<std::collections::hash_map::RandomState> = OnceLock::new();
        let content = std::hash::BuildHasher::hash_one(KEYS.get_or_init(Default::default), &bytes);
        let known_clean = shared.clean.lock().is_ok_and(|c| c.contains(&content));
        // Opened without a check: you said so, knowing no antivirus is here.
        let verdict = if known_clean || unchecked { Verdict::Clean } else { antivirus::scan(&file) };
        let line = match verdict {
            Verdict::Clean => {
                if !unchecked
                    && let Ok(mut clean) = shared.clean.lock()
                {
                    clean.insert(content);
                }
                match afterwards {
                    Afterwards::Open => {
                        let url = file_url(&file);
                        let _ = qt.queue(move |sioul| sioul.open_url(QString::from(&url)));
                        say("scan-clean", &[("name", name)])
                    }
                    Afterwards::Paper => {
                        let kept = crate::papers::keep_checked(&file, &name, &subject);
                        let _ = std::fs::remove_file(&file);
                        match kept {
                            Ok((path, title, kind)) => {
                                let _ = qt.queue(move |mut sioul| sioul.as_mut().paper_kept(QString::from(&path), QString::from(&title), QString::from(kind)));
                                say("papers-kept", &[("name", name)])
                            }
                            Err(e) => say("scan-error", &[("name", name), ("detail", e)]),
                        }
                    }
                    Afterwards::Save => {
                        let target = free_path(&downloads(), &name);
                        let saved = std::fs::create_dir_all(downloads()).and_then(|()| std::fs::copy(&file, &target));
                        let _ = std::fs::remove_file(&file);
                        match saved {
                            Ok(_) => {
                                mark_from_mail(&target);
                                say("ui-saved", &[("path", target.display().to_string())])
                            }
                            Err(e) => say("scan-error", &[("name", name), ("detail", e.to_string())]),
                        }
                    }
                }
            }
            Verdict::Infected(threat) => {
                let _ = std::fs::remove_file(&file);
                say("scan-infected", &[("name", name), ("threat", threat)])
            }
            // No antivirus here: asked before anything opens, with how to install one.
            Verdict::Unavailable(problem) => {
                let _ = std::fs::remove_file(&file);
                let what = match afterwards {
                    Afterwards::Open => 0,
                    Afterwards::Save => 1,
                    Afterwards::Paper => 2,
                };
                let (key, hint, shown) = (key.clone(), antivirus::install_hint(), name.clone());
                let _ = qt.queue(move |mut sioul| sioul.as_mut().scan_unavailable(QString::from(&key), index, what, QString::from(&shown), QString::from(&hint)));
                say("scan-unavailable-short", &[("detail", problem)])
            }
        };
        set_status(&qt, line);
    });
}

/// The settings found, as the form shows them.
#[derive(serde::Serialize)]
struct FoundView {
    host: String,
    port: u16,
    security: &'static str,
    login: String,
    /// "Found: …, from your provider's own settings."
    by: String,
    /// Gmail's app password, or the general advice.
    hint: String,
    /// Where to make an app password, when the provider is known for wanting one.
    help_url: Option<&'static str>,
}

impl qobject::Sioul {
    pub(crate) fn shared(&self) -> Arc<Shared> {
        Arc::clone(&self.rust().shared)
    }

    fn text(&self, id: &QString) -> QString {
        QString::from(&tr().text(&id.to_string(), None))
    }

    fn text_args(&self, id: &QString, args: &QString) -> QString {
        let given: serde_json::Map<String, serde_json::Value> = serde_json::from_str(&args.to_string()).unwrap_or_default();
        let pairs: Vec<(&str, String)> = given.iter().map(|(k, v)| (k.as_str(), v.as_str().map_or_else(|| v.to_string(), str::to_string))).collect();
        QString::from(&say(&id.to_string(), &pairs))
    }

    fn text_with(&self, id: &QString, name: &QString, value: &QString) -> QString {
        QString::from(&say(&id.to_string(), &[(&name.to_string(), value.to_string())]))
    }

    fn start(mut self: Pin<&mut Self>) {
        // "Work now" lasts while Sioul is open: a session that ended without saying so ends it.
        end_work_now();
        // Sites an older file kept among the accounts move to their own list, once, a copy kept beside.
        let path = config_path();
        if std::fs::read_to_string(&path).is_ok_and(|text| text.contains("kind = \"portal\"")) {
            let _ = std::fs::copy(&path, path.with_file_name("config-before-sites.toml"));
            if let Err(e) = config::migrate_sites(&path) {
                eprintln!("{e}");
            }
        }
        self.as_mut().set_reading(QString::from(&reading_json()));
        self.as_mut().set_mode(QString::from(&mode_json()));
        // Your sites' own icons, from the start rather than at the first minute.
        crate::sites::favicon_tick(&self.qt_thread());
        update_weather(&self.qt_thread(), &self.shared());
        let (qt, shared) = (self.qt_thread(), self.shared());
        // The task pages as you left them.
        if let Ok(mut state) = shared.work.lock() {
            *state = work::WorkState::load();
        }
        show(&qt, &shared);
        pim::show_pim(&qt, &shared);
        work::show_work(&qt, &shared);
        start_watchers(&qt, &shared);
        // Taking pictures of your own pages syncs nothing; the test build does, for its scripted runs.
        if std::env::var_os("SIOUL_GRAB").is_none() || cfg!(feature = "insecure-test-tls") {
            pim::start_dav_watchers(&qt, &shared);
            crate::github::start(&qt, &shared);
        }
        // Sioul's own antivirus signatures, when the system has none: kept current once a day.
        if !offline() {
            std::thread::spawn(|| {
                let _ = antivirus::refresh_signatures();
            });
        }
        // Attachments opened in an earlier session (decrypted ones too) leave the cache.
        forget_opened_attachments();
        // Mail fetched before Sioul checked senders itself is checked once,
        // quietly; the checks ask DNS, so not in a demo.
        if offline() {
            return;
        }
        let (qt, shared) = (qt.clone(), Arc::clone(&shared));
        std::thread::spawn(move || {
            let checked: usize = load_config().mail_sources().iter().map(|s| sioul_sync::verify::verify_folder(&s.folder, false)).sum();
            if checked > 0 {
                show(&qt, &shared);
            }
        });
    }

    fn refresh(self: Pin<&mut Self>) {
        show(&self.qt_thread(), &self.shared());
    }

    /// Everything again: what is on this computer (notes, tasks, the plan,
    /// the weather) read at once; mail, agenda, tasks, contacts and GitHub fetched.
    fn sync_now(mut self: Pin<&mut Self>) {
        let (qt, shared) = (self.qt_thread(), self.shared());
        crate::share::exchange(&qt, &shared);
        show(&qt, &shared);
        pim::show_pim(&qt, &shared);
        work::show_work(&qt, &shared);
        update_weather(&qt, &shared);
        let calendars = pim::start_dav_watchers(&qt, &shared);
        crate::github::start(&qt, &shared);
        if start_watchers(&qt, &shared) == 0 && calendars == 0 {
            self.as_mut().set_status(QString::from(&tr().text("sync-nothing", None)));
            return;
        }
        self.as_mut().set_busy(true);
        self.as_mut().set_status(QString::from(&tr().text("ui-refreshing", None)));
    }

    fn open_anyway(self: Pin<&mut Self>) {
        let shared = self.shared();
        shared.opened_anyway.store(true, Ordering::Relaxed);
        show(&self.qt_thread(), &shared);
    }

    fn done(mut self: Pin<&mut Self>) {
        let shared = self.shared();
        let newest = shared.shown.lock().map(|s| s.clone()).unwrap_or_default();
        let path = PorchState::default_path();
        let mut state = PorchState::load(&path);
        state.close(&newest);
        let line = match state.save(&path) {
            Ok(()) => tr().text(if newest.is_empty() { "done-nothing" } else { "done-closed" }, None),
            Err(e) => e,
        };
        shared.opened_anyway.store(false, Ordering::Relaxed);
        self.as_mut().set_status(QString::from(&line));
        show(&self.qt_thread(), &shared);
    }

    fn message(&self, key: &QString) -> QString {
        let world = World::load();
        let own = mail::own_addresses(&world.config);
        let shown = world.judge(&key.to_string()).and_then(|(path, triaged)| {
            let raw = std::fs::read(&path).ok()?;
            crypto::learn_autocrypt(&raw, &triaged);
            let (opened, protection) = match crypto::open(&raw) {
                Some((opened, protection)) => (opened, Some(protection)),
                None => (None, None),
            };
            let mut shown = view::message_from(opened.as_deref().unwrap_or(&raw), &path, &triaged, &own, tr(), protection)?;
            shown.role = mail::locate(&key.to_string()).and_then(|(account, file)| sioul_sync::mailbox::folder_of(&account, &file)).map(|f| f.role);
            Some(shown)
        });
        QString::from(&shown.map_or_else(String::new, |m| json(&m)))
    }

    fn open_attachment(mut self: Pin<&mut Self>, key: &QString, index: i32) {
        self.as_mut().set_status(QString::from(&tr().text("scan-running-any", None)));
        checked_attachment(&self.qt_thread(), &self.shared(), key.to_string(), index, Afterwards::Open, false);
    }

    fn save_attachment(mut self: Pin<&mut Self>, key: &QString, index: i32) {
        self.as_mut().set_status(QString::from(&tr().text("scan-running-any", None)));
        checked_attachment(&self.qt_thread(), &self.shared(), key.to_string(), index, Afterwards::Save, false);
    }

    fn attachment_unchecked(self: Pin<&mut Self>, key: &QString, index: i32, what: i32) {
        checked_attachment(&self.qt_thread(), &self.shared(), key.to_string(), index, Afterwards::of(what), true);
    }

    fn keep_attachment_as_paper(mut self: Pin<&mut Self>, key: &QString, index: i32) {
        self.as_mut().set_status(QString::from(&tr().text("scan-running-any", None)));
        checked_attachment(&self.qt_thread(), &self.shared(), key.to_string(), index, Afterwards::Paper, false);
    }

    fn papers(&self) -> QString {
        QString::from(&crate::papers::view())
    }

    fn save_paper(self: Pin<&mut Self>, id: &QString, edit: &QString) -> QString {
        QString::from(&crate::papers::save(&id.to_string(), &edit.to_string()))
    }

    fn remove_paper(self: Pin<&mut Self>, id: &QString) -> QString {
        QString::from(&crate::papers::remove(&id.to_string()))
    }

    fn plan_renewal(self: Pin<&mut Self>, id: &QString) -> QString {
        QString::from(&crate::papers::plan_renewal(&self.qt_thread(), &self.shared(), &id.to_string()))
    }

    fn let_in(mut self: Pin<&mut Self>, address: &QString) {
        let address = address.to_string();
        let line = match KnownSenders::let_in(&load_config().known_senders_path(), &address) {
            Ok(()) => say("ui-let-in-done", &[("address", address)]),
            Err(e) => e,
        };
        self.as_mut().set_status(QString::from(&line));
        show(&self.qt_thread(), &self.shared());
    }

    fn block(self: Pin<&mut Self>, entry: &QString) {
        self.set_standing(entry, &QString::from("blocked"));
    }

    fn unblock(self: Pin<&mut Self>, entry: &QString) {
        self.set_standing(entry, &QString::from("neutral"));
    }

    fn set_standing(mut self: Pin<&mut Self>, entry: &QString, standing: &QString) {
        let entry = entry.to_string().trim().to_ascii_lowercase();
        let Some(standing) = porch::Standing::read(&standing.to_string()) else { return };
        let line = match porch::set_standing(&load_config(), &entry, standing) {
            Ok(()) => say(&format!("sender-now-{}", standing.as_str()), &[("entry", porch::normalize(&entry).unwrap_or(entry.clone()).trim_start_matches("*@").to_string())]),
            Err(e) => e,
        };
        self.as_mut().set_status(QString::from(&line));
        let (qt, shared) = (self.qt_thread(), self.shared());
        show(&qt, &shared);
        pim::show_pim(&qt, &shared);
    }

    fn standing(&self, address: &QString) -> QString {
        QString::from(porch::Senders::load(&load_config()).standing(&address.to_string()).as_str())
    }

    fn add_mail_line(mut self: Pin<&mut Self>, key: &QString, budget: &QString) {
        let (key, budget) = (key.to_string(), budget.to_string());
        let world = World::load();
        let outcome = (|| {
            let ledger = world.ledger().ok_or_else(|| tr().text("ui-no-budgets", None))?;
            let path = world.ledger_path().ok_or_else(|| tr().text("ui-no-budgets", None))?;
            let mail = world.mail_lines(&ledger);
            let found = mail.iter().find(|l| l.key == key).ok_or_else(|| tr().text("note-no-amount", None))?;
            let line = found.line(&budget).ok_or_else(|| tr().text("note-no-amount", None))?;
            let origin = say("budget-origin-mail", &[("subject", found.subject.clone()), ("sender", found.sender.clone()), ("date", Zoned::now().date().to_string())]);
            budget::record_line(&path, &line, &origin)?;
            Ok::<String, String>(say("ui-line-added", &[("budget", ledger.budget_title(&budget).to_string())]))
        })();
        let line = outcome.unwrap_or_else(|e| e);
        self.as_mut().set_status(QString::from(&line));
        show(&self.qt_thread(), &self.shared());
    }

    fn ignore_mail_line(mut self: Pin<&mut Self>, key: &QString) {
        let path = MoneyState::default_path();
        let mut state = MoneyState::load(&path);
        state.ignored.insert(key.to_string());
        let line = match state.save(&path) {
            Ok(()) => tr().text("ui-line-ignored", None),
            Err(e) => e,
        };
        self.as_mut().set_status(QString::from(&line));
        show(&self.qt_thread(), &self.shared());
    }

    fn discover(mut self: Pin<&mut Self>, address: &QString) {
        let address = address.to_string().trim().to_ascii_lowercase();
        let qt = self.qt_thread();
        self.as_mut().set_form_busy(true);
        self.as_mut().set_form_error(QString::default());
        self.as_mut().set_found(QString::default());
        std::thread::spawn(move || {
            let result = sioul_sync::discover(&address).map(|found| {
                let server = say(
                    "account-server",
                    &[
                        ("host", found.host.clone()),
                        ("port", found.port.to_string()),
                        ("security", tr().text(&format!("security-{}", found.security.as_str()), None)),
                    ],
                );
                let gmail = sioul_sync::is_gmail(&address);
                json(&FoundView {
                    by: say("account-found", &[("server", server), ("by", tr().text(found.by.message_id(), None))]),
                    hint: tr().text(if gmail { "account-gmail-hint" } else { "account-app-password-hint" }, None),
                    help_url: gmail.then_some(sioul_sync::GMAIL_APP_PASSWORDS),
                    host: found.host,
                    port: found.port,
                    security: found.security.as_str(),
                    login: found.username,
                })
            });
            let _ = qt.queue(move |mut sioul| {
                sioul.as_mut().set_form_busy(false);
                match result {
                    Ok(found) => sioul.as_mut().set_found(QString::from(&found)),
                    Err(e) => sioul.as_mut().set_form_error(QString::from(&e.sentence(tr(), &address))),
                }
            });
        });
    }

    fn add_account(mut self: Pin<&mut Self>, address: &QString, host: &QString, port: i32, security: &QString, login: &QString, password: &QString) {
        let (address, host, login, password) = (address.to_string().trim().to_ascii_lowercase(), host.to_string().trim().to_string(), login.to_string().trim().to_string(), password.to_string());
        let security = if security.to_string() == "starttls" { Security::Starttls } else { Security::Tls };
        let port = u16::try_from(port).ok().filter(|p| *p > 0).unwrap_or(if security == Security::Tls { 993 } else { 143 });
        self.as_mut().set_form_busy(true);
        self.as_mut().set_form_error(QString::default());
        add_mail(&self.qt_thread(), &self.shared(), address, host, port, security, login, Some(password));
    }

    fn add_mail_like(mut self: Pin<&mut Self>, address: &QString, host: &QString, port: i32, security: &QString, login: &QString) {
        let (address, host, login) = (address.to_string().trim().to_ascii_lowercase(), host.to_string().trim().to_string(), login.to_string().trim().to_string());
        let security = if security.to_string() == "starttls" { Security::Starttls } else { Security::Tls };
        let port = u16::try_from(port).ok().filter(|p| *p > 0).unwrap_or(if security == Security::Tls { 993 } else { 143 });
        self.as_mut().set_form_busy(true);
        self.as_mut().set_form_error(QString::default());
        add_mail(&self.qt_thread(), &self.shared(), address, host, port, security, login, None);
    }

    fn set_account_enabled(mut self: Pin<&mut Self>, id: &QString, on: bool) -> QString {
        let id = id.to_string();
        let config = load_config();
        let Some(account) = config.every_account().find(|a| a.id == id).cloned() else { return QString::from(&say("account-unknown", &[("id", id)])) };
        if let Err(e) = config::set_enabled(&config_path(), &id, on) {
            return QString::from(&e);
        }
        let (qt, shared) = (self.qt_thread(), self.shared());
        if account.kind == config::AccountKind::Dav {
            // Its calendars and address books leave the pages while it is off.
            sioul_core::vdir::set_account_off(&id, !on);
            if on {
                pim::start_dav_watchers(&qt, &shared);
            } else {
                pim::stop_dav_watcher(&shared, &id);
            }
        } else if on {
            start_watchers(&qt, &shared);
        } else if let Some(control) = shared.watchers.lock().ok().and_then(|mut w| w.remove(&id)) {
            control.stop();
        }
        self.as_mut().set_status(QString::from(&say(if on { "account-on" } else { "account-off" }, &[("id", id)])));
        show(&qt, &shared);
        pim::show_pim(&qt, &shared);
        mail::show_mail(&qt, &shared);
        QString::default()
    }

    fn scout_account(mut self: Pin<&mut Self>, address: &QString) {
        let address = address.to_string().trim().to_ascii_lowercase();
        let qt = self.qt_thread();
        self.as_mut().set_scouting(true);
        self.as_mut().set_scouted(QString::default());
        std::thread::spawn(move || {
            let config = load_config();
            let mine = |kind: config::AccountKind| config.every_account().find(|a| a.address.as_deref().map(str::to_lowercase).as_deref() == Some(address.as_str()) && a.kind == kind).cloned();
            let (mail_have, dav_have) = (mine(config::AccountKind::Imap), mine(config::AccountKind::Dav));
            // A Nextcloud's apps are asked with the password its calendars use.
            let password = dav_have.as_ref().and_then(|a| secret::password(a).ok());
            let login = dav_have.as_ref().and_then(|a| a.login().map(str::to_string)).unwrap_or_else(|| address.clone());
            let found = sioul_sync::scout::scout(&address, dav_have.as_ref().and_then(|a| a.host.as_deref()), password.as_deref().map(|p| (login.as_str(), p)));
            let mail = found.mail.as_ref().map(|m| {
                let server = say("account-server", &[("host", m.host.clone()), ("port", m.port.to_string()), ("security", tr().text(&format!("security-{}", m.security.as_str()), None))]);
                serde_json::json!({ "host": m.host, "port": m.port, "security": m.security.as_str(), "login": m.username, "line": server, "have": mail_have.as_ref().map(|a| a.id.clone()) })
            });
            let dav = found.dav.as_ref().map(|url| serde_json::json!({ "url": url, "have": dav_have.as_ref().map(|a| a.id.clone()) }));
            let nextcloud = found.nextcloud.as_ref().map(|n| {
                let apps: Vec<serde_json::Value> = n
                    .apps
                    .iter()
                    .filter_map(|a| sioul_sync::scout::known_app(a))
                    .map(|(id, in_sioul)| serde_json::json!({ "id": id, "title": tr().text(&format!("scout-app-{id}"), None), "in_sioul": in_sioul }))
                    .collect();
                serde_json::json!({ "product": n.product, "version": n.version, "server": n.server, "apps": apps, "asked": password.is_some() })
            });
            let answer = serde_json::json!({ "address": address, "mail": mail, "dav": dav, "nextcloud": nextcloud, "dav_password": password.is_some() }).to_string();
            let _ = qt.queue(move |mut sioul| {
                sioul.as_mut().set_scouting(false);
                sioul.as_mut().set_scouted(QString::from(&answer));
            });
        });
    }

    fn add_portal(mut self: Pin<&mut Self>, name: &QString, url: &QString) {
        let (id, url) = (config::slug(&name.to_string()), url.to_string().trim().to_string());
        let config = load_config();
        let outcome = if config.site(&id).is_some() {
            Err(say("account-exists", &[("id", id.clone())]))
        } else if id.is_empty() || !url.starts_with("https://") {
            // A site now, and a site's sign-in never travels in clear.
            Err(tr().text("site-https-only", None))
        } else {
            config::add_site(&config_path(), &id, &name.to_string(), &url).map(|()| say("account-portal-added", &[("id", id.clone()), ("url", url.clone())]))
        };
        match outcome {
            Ok(line) => self.as_mut().set_status(QString::from(&line)),
            Err(e) => self.as_mut().set_form_error(QString::from(&e)),
        }
        show(&self.qt_thread(), &self.shared());
    }

    fn set_account_password(mut self: Pin<&mut Self>, id: &QString, password: &QString) {
        self.as_mut().set_form_busy(true);
        give_password(&self.qt_thread(), &self.shared(), id.to_string(), password.to_string());
    }

    fn remove_account(mut self: Pin<&mut Self>, id: &QString) {
        let id = id.to_string();
        let shared = self.shared();
        if let Some(control) = shared.watchers.lock().ok().and_then(|mut w| w.remove(&id)) {
            control.stop();
        }
        if let Ok(mut statuses) = shared.statuses.lock() {
            statuses.remove(&id);
        }
        let config = load_config();
        // Switched off, an account can be removed all the same.
        let line = match config.every_account().find(|a| a.id == id) {
            None => say("account-unknown", &[("id", id.clone())]),
            Some(account) => {
                if account.is_dav() {
                    pim::stop_dav_watcher(&shared, &id);
                }
                // The keyring keeps one password per login and server: another
                // account on the same (a cPanel host's mail and calendars) keeps it.
                let shared_password = config.every_account().any(|a| a.id != id && a.host == account.host && a.login() == account.login());
                let forgotten = if account.auth.as_deref() == Some("google") {
                    // Google's access goes back to it.
                    sioul_sync::google::revoke(account.address.as_deref().unwrap_or("")).map_err(|e| e.sentence(tr(), &id))
                } else if (account.syncs() || account.is_dav()) && !shared_password {
                    secret::forget(account).map_err(|e| e.sentence(tr(), &id))
                } else {
                    Ok(())
                };
                match forgotten.and_then(|()| config::remove_account(&config_path(), &id)) {
                    Ok(()) => say("account-removed", &[("id", id.clone()), ("path", account.maildir_path().display().to_string())]),
                    Err(e) => e,
                }
            }
        };
        self.as_mut().set_status(QString::from(&line));
        show(&self.qt_thread(), &shared);
    }

    fn set_priority(mut self: Pin<&mut Self>, id: &QString, priority: &QString) {
        let id = id.to_string();
        let line = match Priority::parse(&priority.to_string()) {
            Some(priority) => match config::set_priority(&config_path(), &id, priority) {
                Ok(()) => say("ui-priority-set", &[("id", id), ("priority", tr().text(&format!("priority-{}", priority.as_str()), None))]),
                Err(e) => e,
            },
            None => return,
        };
        self.as_mut().set_status(QString::from(&line));
        show(&self.qt_thread(), &self.shared());
    }

    fn open_folder(self: Pin<&mut Self>, account: &QString, folder: &QString, all: bool, query: &QString) {
        let shared = self.shared();
        if let Ok(mut open) = shared.open_folder.lock() {
            *open = Some(mail::OpenFolder { account: account.to_string(), folder: folder.to_string(), all, query: query.to_string() });
        }
        mail::show_mail(&self.qt_thread(), &shared);
    }

    fn opened(self: Pin<&mut Self>, key: &QString) {
        mail::opened(&self.qt_thread(), &self.shared(), &key.to_string());
    }

    fn act(self: Pin<&mut Self>, key: &QString, action: &QString, target: &QString) {
        mail::act(&self.qt_thread(), &self.shared(), &key.to_string(), &action.to_string(), &target.to_string());
    }

    fn act_many(self: Pin<&mut Self>, keys: &QString, action: &QString) {
        let keys: Vec<String> = serde_json::from_str(&keys.to_string()).unwrap_or_default();
        mail::act_many(&self.qt_thread(), &self.shared(), &keys, &action.to_string());
    }

    fn move_messages(self: Pin<&mut Self>, keys: &QString, account: &QString, folder: &QString) {
        let keys: Vec<String> = serde_json::from_str(&keys.to_string()).unwrap_or_default();
        mail::move_many(&self.qt_thread(), &self.shared(), &keys, &account.to_string(), &folder.to_string());
    }

    fn undo(mut self: Pin<&mut Self>) {
        if let Some(draft) = mail::undo(&self.qt_thread(), &self.shared()) {
            self.as_mut().compose_requested(QString::from(&draft));
        }
    }

    fn flush(self: Pin<&mut Self>) {
        mail::flush(&self.shared());
        end_work_now();
    }

    fn compose(mut self: Pin<&mut Self>, kind: &QString, key: &QString, account: &QString) -> QString {
        match mail::compose(&kind.to_string(), &key.to_string(), &account.to_string()) {
            Ok(id) => {
                mail::show_mail(&self.qt_thread(), &self.shared());
                QString::from(&id)
            }
            Err(e) => {
                self.as_mut().set_status(QString::from(&e));
                QString::default()
            }
        }
    }

    fn draft(&self, id: &QString) -> QString {
        QString::from(&mail::draft(&id.to_string()).unwrap_or_default())
    }

    fn save_draft(self: Pin<&mut Self>, edit: &QString) -> QString {
        let line = mail::save_draft(&edit.to_string()).unwrap_or_else(|e| e);
        mail::show_mail(&self.qt_thread(), &self.shared());
        QString::from(&line)
    }

    fn attach(self: Pin<&mut Self>, id: &QString, url: &QString) -> QString {
        QString::from(&mail::attach(&id.to_string(), &url.to_string()).err().unwrap_or_default())
    }

    fn detach(mut self: Pin<&mut Self>, id: &QString, index: i32, forwarded: bool) {
        if let Err(e) = mail::detach(&id.to_string(), usize::try_from(index).unwrap_or(usize::MAX), forwarded) {
            self.as_mut().set_status(QString::from(&e));
        }
    }

    fn preview(&self, markdown: &QString) -> QString {
        QString::from(&sioul_core::compose::markdown_html(&markdown.to_string()))
    }

    fn send(self: Pin<&mut Self>, id: &QString) -> QString {
        QString::from(&mail::send(&self.qt_thread(), &self.shared(), &id.to_string()).unwrap_or_default())
    }

    fn discard(self: Pin<&mut Self>, id: &QString) {
        mail::discard(&self.qt_thread(), &self.shared(), &id.to_string());
    }

    fn draft_closed(self: Pin<&mut Self>, id: &QString) {
        mail::closed(&self.qt_thread(), &self.shared(), &id.to_string());
    }

    fn place(&self, key: &QString) -> QString {
        QString::from(&mail::place(&key.to_string()))
    }

    fn source(&self, key: &QString) -> QString {
        QString::from(&mail::source(&key.to_string()))
    }

    fn set_writing(mut self: Pin<&mut Self>, id: &QString, name: &QString, signature: &QString) {
        let id = id.to_string();
        let line = match config::set_writing(&config_path(), &id, &name.to_string(), &signature.to_string()) {
            Ok(()) => say("ui-writing-saved", &[("id", id)]),
            Err(e) => e,
        };
        self.as_mut().set_status(QString::from(&line));
        show(&self.qt_thread(), &self.shared());
    }

    fn grab_folder(&self) -> QString {
        QString::from(&std::env::var("SIOUL_GRAB").unwrap_or_default())
    }

    fn search_contacts(self: Pin<&mut Self>, query: &QString) {
        let shared = self.shared();
        if let Ok(mut state) = shared.pim.lock() {
            state.query = query.to_string();
        }
        pim::show_pim(&self.qt_thread(), &shared);
    }

    fn contact(&self, key: &QString) -> QString {
        QString::from(&pim::contact(&key.to_string()))
    }

    fn save_contact(self: Pin<&mut Self>, key: &QString, edit: &QString) -> QString {
        let answer = match pim::save_contact(&self.qt_thread(), &self.shared(), &key.to_string(), &edit.to_string()) {
            Ok(key) => serde_json::json!({ "key": key }),
            Err(error) => serde_json::json!({ "error": error }),
        };
        QString::from(&answer.to_string())
    }

    fn delete_contact(self: Pin<&mut Self>, key: &QString) {
        pim::delete_contact(&self.qt_thread(), &self.shared(), &key.to_string());
    }

    fn show_days(self: Pin<&mut Self>, from: &QString, days: i32) {
        let shared = self.shared();
        if let Ok(mut state) = shared.pim.lock() {
            state.from = from.to_string().parse().unwrap_or_else(|_| Zoned::now().date());
            // Up to five years: the agenda's past reaches as far back as the history setting.
            state.days = i64::from(days.clamp(1, 5 * 366));
        }
        pim::show_pim(&self.qt_thread(), &shared);
    }

    fn calendars(&self) -> QString {
        QString::from(&pim::calendars())
    }

    fn event(&self, key: &QString) -> QString {
        QString::from(&pim::event(&key.to_string()))
    }

    fn save_event(self: Pin<&mut Self>, key: &QString, edit: &QString, calendar: &QString) -> QString {
        let result = pim::save_event(&self.qt_thread(), &self.shared(), &key.to_string(), &edit.to_string(), &calendar.to_string());
        QString::from(&result.err().unwrap_or_default())
    }

    fn delete_event(self: Pin<&mut Self>, key: &QString, start: f64, only_this: bool) {
        pim::delete_event(&self.qt_thread(), &self.shared(), &key.to_string(), start as i64, only_this);
    }

    fn completions(&self, typed: &QString) -> QString {
        QString::from(&pim::completions(&typed.to_string()))
    }

    fn contact_for(&self, address: &QString) -> QString {
        QString::from(&pim::contact_for(&address.to_string()))
    }

    fn add_sender(mut self: Pin<&mut Self>, name: &QString, address: &QString) {
        if let Err(e) = pim::add_sender(&self.qt_thread(), &self.shared(), &name.to_string(), &address.to_string()) {
            self.as_mut().set_status(QString::from(&e));
        }
    }

    fn invitation(&self, key: &QString) -> QString {
        QString::from(&pim::invitation(&key.to_string()))
    }

    fn answer_invitation(mut self: Pin<&mut Self>, key: &QString, answer: &QString) {
        // Sending waits on the network: on a thread.
        let (qt, shared, key, answer) = (self.qt_thread(), self.shared(), key.to_string(), answer.to_string());
        self.as_mut().set_status(QString::from(&tr().text("invitation-answering", None)));
        std::thread::spawn(move || {
            if let Err(e) = pim::answer(&qt, &shared, &key, &answer) {
                tell(&qt, &shared, e);
            }
        });
    }

    fn history_weeks(&self) -> i32 {
        i32::try_from(load_config().history_weeks.unwrap_or(config::DEFAULT_HISTORY_WEEKS)).unwrap_or(2)
    }

    fn day_start(&self) -> i32 {
        i32::try_from(load_config().agenda.day_start.unwrap_or(7).min(23)).unwrap_or(7)
    }

    fn set_history(mut self: Pin<&mut Self>, weeks: i32) {
        let weeks = u32::try_from(weeks.max(0)).unwrap_or(0);
        let line = match config::set_history(&config_path(), weeks) {
            Ok(()) if weeks == 0 => tr().text("ui-history-everything", None),
            Ok(()) => say("ui-history-set", &[("weeks", weeks.to_string())]),
            Err(e) => e,
        };
        self.as_mut().set_status(QString::from(&line));
        // The watchers fetch now: the older mail comes with this round.
        let (qt, shared) = (self.qt_thread(), self.shared());
        start_watchers(&qt, &shared);
        show(&qt, &shared);
        pim::show_pim(&qt, &shared);
    }

    fn add_dav(mut self: Pin<&mut Self>, address: &QString, url: &QString, login: &QString, password: &QString) {
        self.as_mut().set_form_busy(true);
        self.as_mut().set_form_error(QString::default());
        pim::add_account(&self.qt_thread(), &self.shared(), address.to_string(), url.to_string(), login.to_string(), password.to_string());
    }

    fn add_google(mut self: Pin<&mut Self>, address: &QString, client_id: &QString, client_secret: &QString, again: &QString) {
        self.as_mut().set_form_busy(true);
        self.as_mut().set_form_error(QString::default());
        let again = Some(again.to_string()).filter(|a| !a.is_empty());
        pim::add_google_account(&self.qt_thread(), &self.shared(), address.to_string(), client_id.to_string(), client_secret.to_string(), again);
    }

    fn cancel_google(self: Pin<&mut Self>) {
        self.shared().google_stop.store(true, Ordering::Relaxed);
    }

    fn pgp_keys(&self) -> QString {
        QString::from(&crypto::keys_view())
    }

    fn pgp_generate(mut self: Pin<&mut Self>, address: &QString) {
        // Making a key takes a moment (its passphrase is hashed with Argon2): on a thread.
        let (qt, shared, address) = (self.qt_thread(), self.shared(), address.to_string());
        self.as_mut().set_status(QString::from(&tr().text("pgp-making", None)));
        std::thread::spawn(move || {
            let line = crypto::generate(&address).unwrap_or_else(|e| e);
            tell(&qt, &shared, line);
            let _ = qt.queue(|mut sioul| sioul.as_mut().keys_changed());
        });
    }

    fn pgp_import(mut self: Pin<&mut Self>, url: &QString, passphrase: &QString) -> QString {
        match crypto::import(&url.to_string(), &passphrase.to_string()) {
            Ok(line) => {
                self.as_mut().set_status(QString::from(&line));
                self.as_mut().keys_changed();
                QString::default()
            }
            Err(e) => QString::from(&e),
        }
    }

    fn pgp_export(mut self: Pin<&mut Self>, fingerprint: &QString) {
        let line = match crypto::export(&fingerprint.to_string(), &downloads()) {
            Ok(path) => say("ui-saved", &[("path", path.display().to_string())]),
            Err(e) => e,
        };
        self.as_mut().set_status(QString::from(&line));
    }

    fn pgp_remove(mut self: Pin<&mut Self>, fingerprint: &QString) {
        let line = match sioul_core::pgp::remove(&fingerprint.to_string()) {
            Ok(()) => tr().text("pgp-removed", None),
            Err(e) => e,
        };
        self.as_mut().set_status(QString::from(&line));
        self.as_mut().keys_changed();
    }

    fn pgp_lookup(mut self: Pin<&mut Self>, draft: &QString) {
        let (qt, shared, id) = (self.qt_thread(), self.shared(), draft.to_string());
        self.as_mut().set_status(QString::from(&tr().text("pgp-looking", None)));
        std::thread::spawn(move || {
            let Some(draft) = sioul_core::compose::Draft::by_id(&id) else { return };
            tell(&qt, &shared, crypto::lookup(&draft));
            let _ = qt.queue(|mut sioul| sioul.as_mut().keys_changed());
        });
    }

    fn show_tasks(self: Pin<&mut Self>, by: &QString, done: bool, query: &QString, case_id: &QString) {
        work::set_view(&self.qt_thread(), &self.shared(), &by.to_string(), done, &query.to_string(), &case_id.to_string());
    }

    fn filter_tasks(self: Pin<&mut Self>, kind: &QString, category: &QString) {
        work::set_filter(&self.qt_thread(), &self.shared(), &kind.to_string(), &category.to_string());
    }

    fn refresh_work(self: Pin<&mut Self>) {
        work::show_work(&self.qt_thread(), &self.shared());
    }

    fn show_tasks_anyway(self: Pin<&mut Self>, on: bool) {
        work::show_anyway(&self.qt_thread(), &self.shared(), on);
    }

    fn task(&self, uid: &QString) -> QString {
        QString::from(&work::task(&self.shared(), &uid.to_string()))
    }

    fn save_task(self: Pin<&mut Self>, uid: &QString, edit: &QString, list: &QString) -> QString {
        QString::from(&work::save(&self.qt_thread(), &self.shared(), &uid.to_string(), &edit.to_string(), &list.to_string()))
    }

    fn capture(&self, line: &QString) -> QString {
        QString::from(&work::captured(&self.shared(), &line.to_string()))
    }

    fn add_task(self: Pin<&mut Self>, line: &QString, parent: &QString, list: &QString) -> QString {
        QString::from(&work::add(&self.qt_thread(), &self.shared(), &line.to_string(), &parent.to_string(), &list.to_string()))
    }

    fn set_task_status(self: Pin<&mut Self>, uid: &QString, status: &QString) -> QString {
        QString::from(&work::set_status(&self.qt_thread(), &self.shared(), &uid.to_string(), &status.to_string()))
    }

    fn set_waits(self: Pin<&mut Self>, uid: &QString, other: &QString, wait: bool) -> QString {
        QString::from(&work::set_waits(&self.qt_thread(), &self.shared(), &uid.to_string(), &other.to_string(), wait))
    }

    fn not_now(self: Pin<&mut Self>, uid: &QString) {
        work::not_now(&self.qt_thread(), &self.shared(), &uid.to_string());
    }

    fn set_weather(self: Pin<&mut Self>, weather: &QString) {
        work::set_weather(&self.qt_thread(), &self.shared(), &weather.to_string());
    }

    fn delete_task(self: Pin<&mut Self>, uid: &QString) {
        work::delete(&self.qt_thread(), &self.shared(), &uid.to_string());
    }

    fn search_tasks(&self, query: &QString, except: &QString) -> QString {
        QString::from(&work::search(&self.shared(), &query.to_string(), &except.to_string()))
    }

    fn new_list(self: Pin<&mut Self>, account: &QString, name: &QString) -> QString {
        QString::from(&work::new_list(&self.qt_thread(), &self.shared(), &account.to_string(), &name.to_string()))
    }

    fn list_accounts(&self) -> QString {
        QString::from(&work::list_accounts())
    }

    fn focus_start(self: Pin<&mut Self>, uid: &QString, minutes: i32) {
        work::focus_start(&self.qt_thread(), &self.shared(), &uid.to_string(), minutes);
    }

    fn focus_pause(self: Pin<&mut Self>) {
        work::focus_pause(&self.qt_thread(), &self.shared());
    }

    fn focus_extend(self: Pin<&mut Self>, minutes: i32) {
        work::focus_extend(&self.qt_thread(), &self.shared(), minutes);
    }

    fn focus_stop(self: Pin<&mut Self>, done: bool, note: &QString) -> QString {
        QString::from(&work::focus_stop(&self.qt_thread(), &self.shared(), done, &note.to_string()))
    }

    fn search_notes(self: Pin<&mut Self>, query: &QString) {
        work::search_notes(&self.qt_thread(), &self.shared(), &query.to_string());
    }

    fn note(&self, path: &QString) -> QString {
        QString::from(&work::note(&self.shared(), &path.to_string()))
    }

    fn save_note(self: Pin<&mut Self>, path: &QString, text: &QString) -> QString {
        QString::from(&work::save_note(&self.qt_thread(), &self.shared(), &path.to_string(), &text.to_string()))
    }

    fn create_note(self: Pin<&mut Self>, title: &QString, links: &QString) -> QString {
        QString::from(&work::create_note(&self.qt_thread(), &self.shared(), &title.to_string(), &links.to_string()))
    }

    fn related(&self, uri: &QString) -> QString {
        QString::from(&work::related(&self.shared(), &uri.to_string()))
    }

    fn link_things(mut self: Pin<&mut Self>, from: &QString, to: &QString) -> QString {
        let line = work::link(&self.qt_thread(), &self.shared(), &from.to_string(), &to.to_string());
        self.as_mut().set_status(QString::from(&line));
        QString::from(&line)
    }

    fn unlink_things(mut self: Pin<&mut Self>, from: &QString, to: &QString) -> QString {
        let line = work::unlink(&self.qt_thread(), &self.shared(), &from.to_string(), &to.to_string());
        self.as_mut().set_status(QString::from(&line));
        QString::from(&line)
    }

    fn search_things(&self, query: &QString, kind: &QString, from: &QString) -> QString {
        QString::from(&work::search_things(&self.shared(), &query.to_string(), &kind.to_string(), &from.to_string()))
    }

    fn uri_of(&self, kind: &QString, id: &QString) -> QString {
        QString::from(&work::uri_of(&kind.to_string(), &id.to_string()))
    }

    fn is_program(&self, name: &QString) -> bool {
        sioul_core::links::is_program(&name.to_string())
    }

    fn first_frame(&self) {
        crate::timing("the first frame drawn");
    }

    fn mark(&self, what: &QString) {
        crate::timing(&what.to_string());
    }

    fn phone_accounts(&self) -> bool {
        cfg!(target_os = "android")
    }

    fn choose_phone_account(self: Pin<&mut Self>) {
        #[cfg(target_os = "android")]
        {
            if let Ok(mut waiting) = PHONE_CHOOSER.lock() {
                *waiting = Some(self.qt_thread());
            }
            // SAFETY: android/main.cpp's, called on Qt's thread; it answers through
            // `sioul_android_account_chosen`, on whatever thread Android gives.
            unsafe { sioul_android_choose_account() };
        }
        #[cfg(not(target_os = "android"))]
        let _ = self;
    }

    fn budget_detail(&self, id: &QString, anchor: &QString, step: &QString) -> QString {
        let world = World::load();
        let today = Zoned::now().date();
        let Some(ledger) = world.ledger() else { return QString::default() };
        let Some(found) = ledger.budgets.iter().find(|b| b.id == id.to_string()) else { return QString::default() };
        let anchor = anchor.to_string().parse().unwrap_or(today);
        QString::from(&json(&view::budget_detail(&ledger, found, anchor, &step.to_string(), tr(), today)))
    }

    fn keep_folder(self: Pin<&mut Self>, account: &QString, folder: &QString, kept: bool) -> QString {
        let (id, folder) = (account.to_string(), folder.to_string());
        let config = load_config();
        let Some(account) = config.account(&id).cloned() else { return QString::default() };
        let mut skipped = account.skip_folders.clone();
        skipped.retain(|f| f != &folder);
        if !kept {
            skipped.push(folder.clone());
        }
        if let Err(e) = config::set_value(&config_path(), &format!("account.{id}.skip_folders"), &config::SettingValue::Texts(skipped)) {
            return QString::from(&e);
        }
        let (qt, shared) = (self.qt_thread(), self.shared());
        std::thread::spawn(move || {
            if !kept && let Err(e) = sioul_sync::mailbox::forget_folder(&account, &folder) {
                set_status(&qt, e.sentence(tr(), &account.id));
            }
            if let Some(control) = shared.watchers.lock().ok().and_then(|w| w.get(&account.id).cloned()) {
                control.nudge();
            }
            mail::show_mail(&qt, &shared);
        });
        QString::default()
    }

    fn create_folder(self: Pin<&mut Self>, account: &QString, name: &QString) {
        let (id, name) = (account.to_string(), name.to_string());
        let (qt, shared) = (self.qt_thread(), self.shared());
        std::thread::spawn(move || {
            let Some(account) = load_config().account(&id).cloned() else { return };
            let made = sioul_sync::secret::password(&account).and_then(|p| sioul_sync::mailbox::create_folder(&account, &p, &name));
            match made {
                Ok(()) => tell(&qt, &shared, say("folder-made", &[("name", name.clone())])),
                Err(e) => tell(&qt, &shared, e.sentence(tr(), &account.id)),
            }
            mail::show_mail(&qt, &shared);
        });
    }

    fn delete_folder(self: Pin<&mut Self>, account: &QString, folder: &QString) {
        let (id, folder) = (account.to_string(), folder.to_string());
        let (qt, shared) = (self.qt_thread(), self.shared());
        std::thread::spawn(move || {
            let Some(account) = load_config().account(&id).cloned() else { return };
            let gone = sioul_sync::secret::password(&account).and_then(|p| sioul_sync::mailbox::delete_folder(&account, &p, &folder));
            match gone {
                Ok(()) => tell(&qt, &shared, say("folder-deleted", &[("name", folder.clone())])),
                Err(sioul_sync::SyncError::Message(m)) if m == sioul_sync::mailbox::NOT_EMPTY => tell(&qt, &shared, say("folder-not-empty", &[("name", folder.clone())])),
                Err(e) => tell(&qt, &shared, e.sentence(tr(), &account.id)),
            }
            mail::show_mail(&qt, &shared);
        });
    }

    fn trash_note(self: Pin<&mut Self>, path: &QString) -> QString {
        QString::from(&work::trash_note(&self.qt_thread(), &self.shared(), &path.to_string()))
    }

    fn rename_note(self: Pin<&mut Self>, path: &QString, name: &QString) -> QString {
        QString::from(&work::rename_note(&self.qt_thread(), &self.shared(), &path.to_string(), &name.to_string()))
    }

    fn create_note_in(self: Pin<&mut Self>, folder: &QString, title: &QString) -> QString {
        QString::from(&work::create_note_in(&self.qt_thread(), &self.shared(), &folder.to_string(), &title.to_string()))
    }

    fn make_folder(self: Pin<&mut Self>, parent: &QString, name: &QString) -> QString {
        QString::from(&work::make_folder(&self.qt_thread(), &self.shared(), &parent.to_string(), &name.to_string()))
    }

    fn rename_folder(self: Pin<&mut Self>, path: &QString, name: &QString) -> QString {
        QString::from(&work::rename_folder(&self.qt_thread(), &self.shared(), &path.to_string(), &name.to_string()))
    }

    fn remove_folder(self: Pin<&mut Self>, path: &QString) -> QString {
        QString::from(&work::remove_folder(&self.qt_thread(), &self.shared(), &path.to_string()))
    }

    fn remove_project(self: Pin<&mut Self>, id: &QString) -> QString {
        QString::from(&crate::projects::remove(&self.qt_thread(), &self.shared(), &id.to_string()))
    }

    fn save_budget(self: Pin<&mut Self>, id: &QString, edit: &QString) -> QString {
        QString::from(&crate::projects::save_budget(&self.qt_thread(), &self.shared(), &id.to_string(), &edit.to_string()))
    }

    fn letters(&self) -> QString {
        QString::from(&crate::letters::view())
    }

    fn letter_task(self: Pin<&mut Self>, id: &QString) -> QString {
        QString::from(&crate::letters::make_task(&self.qt_thread(), &self.shared(), &id.to_string()))
    }

    fn letter_event(self: Pin<&mut Self>, id: &QString) -> QString {
        QString::from(&crate::letters::make_event(&self.qt_thread(), &self.shared(), &id.to_string()))
    }

    fn letter_done(self: Pin<&mut Self>, id: &QString, project: &QString) -> QString {
        QString::from(&crate::letters::done(&id.to_string(), &project.to_string()))
    }

    fn bank(&self) -> QString {
        QString::from(&crate::bank::view())
    }

    fn import_bank(self: Pin<&mut Self>, file: &QString) -> QString {
        let (ok, said) = crate::bank::import(&file.to_string());
        // The budgets count the bank accounts' movements.
        show(&self.qt_thread(), &self.shared());
        QString::from(&serde_json::json!({ "ok": ok, "said": said }).to_string())
    }

    fn import_bank_into(self: Pin<&mut Self>, file: &QString, account: &QString) -> QString {
        let (ok, said) = crate::bank::import_into(&file.to_string(), &account.to_string());
        show(&self.qt_thread(), &self.shared());
        QString::from(&serde_json::json!({ "ok": ok, "said": said }).to_string())
    }

    fn save_bank_account(self: Pin<&mut Self>, id: &QString, edit: &QString) -> QString {
        let problem = crate::bank::save_account(&id.to_string(), &edit.to_string());
        show(&self.qt_thread(), &self.shared());
        QString::from(&problem)
    }

    fn remove_bank_account(self: Pin<&mut Self>, id: &QString) -> QString {
        let problem = crate::bank::remove_account(&id.to_string());
        show(&self.qt_thread(), &self.shared());
        QString::from(&problem)
    }

    fn place_movement(self: Pin<&mut Self>, account: &QString, movement: &QString, choice: &QString) -> QString {
        let problem = crate::bank::place_movement(&account.to_string(), &movement.to_string(), &choice.to_string());
        show(&self.qt_thread(), &self.shared());
        QString::from(&problem)
    }

    fn save_bank_rule(self: Pin<&mut Self>, place: i32, edit: &QString) -> QString {
        let problem = crate::bank::save_rule(place, &edit.to_string());
        show(&self.qt_thread(), &self.shared());
        QString::from(&problem)
    }

    fn remove_bank_rule(self: Pin<&mut Self>, place: i32) -> QString {
        let problem = crate::bank::remove_rule(place);
        show(&self.qt_thread(), &self.shared());
        QString::from(&problem)
    }

    fn save_reserve(self: Pin<&mut Self>, id: &QString, edit: &QString) -> QString {
        let problem = crate::bank::save_reserve(&id.to_string(), &edit.to_string());
        show(&self.qt_thread(), &self.shared());
        QString::from(&problem)
    }

    fn contracts(&self) -> QString {
        QString::from(&crate::contracts::view())
    }

    fn save_contract(self: Pin<&mut Self>, id: &QString, edit: &QString) -> QString {
        QString::from(&crate::contracts::save(&id.to_string(), &edit.to_string()))
    }

    fn remove_contract(self: Pin<&mut Self>, id: &QString) -> QString {
        QString::from(&crate::contracts::remove(&id.to_string()))
    }

    fn contract_letter(self: Pin<&mut Self>, id: &QString) -> QString {
        QString::from(&crate::contracts::letter(&id.to_string()))
    }

    fn contract_from(&self, subject: &QString, from: &QString) -> QString {
        QString::from(&crate::contracts::from_card(&subject.to_string(), &from.to_string()))
    }

    fn routines(&self) -> QString {
        QString::from(&work::routines(&self.shared()))
    }

    fn save_routine(self: Pin<&mut Self>, id: &QString, title: &QString, steps: &QString, by_itself: bool) -> QString {
        QString::from(&work::save_routine(&id.to_string(), &title.to_string(), &steps.to_string(), by_itself))
    }

    fn remove_routine(self: Pin<&mut Self>, id: &QString) -> QString {
        QString::from(&work::remove_routine(&id.to_string()))
    }

    fn share_status(&self, folder: &QString) -> QString {
        QString::from(&crate::share::status(&folder.to_string()))
    }

    fn start_sharing(self: Pin<&mut Self>, folder: &QString, passphrase: &QString, again: &QString) -> QString {
        let problem = crate::share::start(&folder.to_string(), &passphrase.to_string(), &again.to_string());
        if problem.is_empty() {
            crate::share::exchange(&self.qt_thread(), &self.shared());
        }
        QString::from(&problem)
    }

    fn stop_sharing(self: Pin<&mut Self>) -> QString {
        QString::from(&crate::share::stop())
    }

    fn share_candidates(&self) -> QString {
        QString::from(&json(&crate::share::candidates()))
    }

    fn set_share_projects(self: Pin<&mut Self>, on: bool) -> QString {
        let problem = crate::share::set_projects(on);
        if problem.is_empty() {
            crate::share::exchange(&self.qt_thread(), &self.shared());
        }
        QString::from(&problem)
    }

    fn folders_in(&self, path: &QString) -> QString {
        QString::from(&crate::share::folders_in(&path.to_string()))
    }

    fn files_access(&self) -> bool {
        #[cfg(target_os = "android")]
        {
            // SAFETY: android/main.cpp's, asking Android through Java.
            unsafe { sioul_android_files_access() }
        }
        #[cfg(not(target_os = "android"))]
        true
    }

    fn ask_files_access(self: Pin<&mut Self>) {
        // SAFETY: android/main.cpp's, on Qt's thread.
        #[cfg(target_os = "android")]
        unsafe {
            sioul_android_ask_files_access()
        };
        let _ = self;
    }

    fn share_now(self: Pin<&mut Self>) {
        crate::share::exchange(&self.qt_thread(), &self.shared());
    }

    fn export_time_csv(mut self: Pin<&mut Self>, project: &QString, from: &QString, to: &QString, target: &QString) -> QString {
        match crate::projects::export_time_csv(&self.shared(), &project.to_string(), &from.to_string(), &to.to_string(), &target.to_string()) {
            Ok(path) => {
                self.as_mut().set_status(QString::from(&say("time-exported", &[("path", path)])));
                QString::default()
            }
            Err(e) => QString::from(&e),
        }
    }

    fn change_line(self: Pin<&mut Self>, uri: &QString, label: &QString, amount: f64, date: &QString) -> QString {
        QString::from(&crate::projects::change_line(&self.qt_thread(), &self.shared(), &uri.to_string(), &label.to_string(), amount, &date.to_string()))
    }

    fn remove_budget(self: Pin<&mut Self>, what: &QString) -> QString {
        QString::from(&crate::projects::remove_budget(&self.qt_thread(), &self.shared(), &what.to_string()))
    }

    fn noise_url(&self, kind: &QString) -> QString {
        match sioul_core::sounds::noise_file(&config::cache_dir().join("sounds"), &kind.to_string()) {
            Ok(path) => QString::from(&file_url(&path)),
            Err(_) => QString::default(),
        }
    }

    fn calm_sounds(&self) -> QString {
        QString::from(&work::calm_sounds(&self.shared()))
    }

    fn move_task(self: Pin<&mut Self>, uid: &QString, list: &QString, confirmed: bool) -> QString {
        QString::from(&work::move_task(&self.qt_thread(), &self.shared(), &uid.to_string(), &list.to_string(), confirmed))
    }

    fn move_contact(self: Pin<&mut Self>, key: &QString, book: &QString, confirmed: bool) -> QString {
        QString::from(&pim::move_contact(&self.qt_thread(), &self.shared(), &key.to_string(), &book.to_string(), confirmed))
    }

    fn address_books(&self) -> QString {
        QString::from(&pim::books())
    }

    fn find_places(self: Pin<&mut Self>, name: &QString) {
        let (qt, name) = (self.qt_thread(), name.to_string());
        let language = tr().text("qt-locale", None).chars().take(2).collect::<String>();
        std::thread::spawn(move || {
            let found = sioul_sync::weather::places(&name, &language).map(|p| json(&p)).unwrap_or_else(|e| serde_json::json!({ "error": e.sentence(tr(), "Open-Meteo") }).to_string());
            let _ = qt.queue(move |mut sioul| sioul.as_mut().set_places_found(QString::from(&found)));
        });
    }

    fn set_weather_place(mut self: Pin<&mut Self>, name: &QString, latitude: f64, longitude: f64) {
        let name = name.to_string();
        let path = config_path();
        let written = if name.trim().is_empty() {
            ["weather.place", "weather.latitude", "weather.longitude"].iter().try_for_each(|k| config::set_value(&path, k, &config::SettingValue::Text(String::new())))
        } else {
            config::set_value(&path, "weather.place", &config::SettingValue::Text(name))
                .and_then(|()| config::set_value(&path, "weather.latitude", &config::SettingValue::Float(latitude)))
                .and_then(|()| config::set_value(&path, "weather.longitude", &config::SettingValue::Float(longitude)))
        };
        if let Err(e) = written {
            self.as_mut().set_status(QString::from(&e));
        }
        let _ = std::fs::remove_file(weather_cache());
        self.as_mut().set_forecast(QString::default());
        update_weather(&self.qt_thread(), &self.shared());
    }

    fn health_page(&self) -> QString {
        QString::from(&crate::health::page())
    }

    fn save_medicine(self: Pin<&mut Self>, id: &QString, edit: &QString) -> QString {
        QString::from(&crate::health::save_medicine(&id.to_string(), &edit.to_string()))
    }

    fn save_prescription(self: Pin<&mut Self>, id: &QString, edit: &QString) -> QString {
        QString::from(&crate::health::save_prescription(&id.to_string(), &edit.to_string()))
    }

    fn remove_health(self: Pin<&mut Self>, id: &QString) -> QString {
        QString::from(&crate::health::remove(&id.to_string()))
    }

    fn refilled(self: Pin<&mut Self>, id: &QString) -> QString {
        QString::from(&crate::health::refilled(&id.to_string()))
    }

    fn set_dose_taken(self: Pin<&mut Self>, key: &QString, taken: bool) {
        let problem = crate::health::set_taken(&key.to_string(), taken);
        if !problem.is_empty() {
            tell(&self.qt_thread(), &self.shared(), problem);
        }
        // Your other computers know at once.
        crate::share::exchange(&self.qt_thread(), &self.shared());
    }

    fn dose_not_taken(self: Pin<&mut Self>, key: &QString) {
        let problem = crate::health::not_taken(&key.to_string());
        if !problem.is_empty() {
            tell(&self.qt_thread(), &self.shared(), problem);
        }
        crate::share::exchange(&self.qt_thread(), &self.shared());
    }

    fn dose_taken_at(self: Pin<&mut Self>, key: &QString, time: &QString, move_next: bool) -> QString {
        let problem = crate::health::taken_late(&key.to_string(), &time.to_string(), move_next);
        if problem.is_empty() {
            crate::share::exchange(&self.qt_thread(), &self.shared());
        }
        QString::from(&problem)
    }

    fn dose_info(&self, key: &QString) -> QString {
        QString::from(&crate::health::dose_info(&key.to_string()))
    }

    fn needs(&self) -> QString {
        QString::from(&crate::health::needs_page())
    }

    fn save_needs(self: Pin<&mut Self>, edit: &QString) -> QString {
        let problem = crate::health::save_needs(&edit.to_string());
        // The plan goes around them at once.
        crate::work::show_work(&self.qt_thread(), &self.shared());
        QString::from(&problem)
    }

    fn skip_need(self: Pin<&mut Self>, key: &QString, skip: bool) -> QString {
        QString::from(&crate::health::skip_today(&key.to_string(), skip))
    }

    fn missed_doses(&self) -> QString {
        QString::from(&crate::health::missed())
    }

    fn going_away(self: Pin<&mut Self>) {
        crate::share::closing();
    }

    fn back_here(self: Pin<&mut Self>) {
        let now = jiff::Timestamp::now().as_second();
        self.shared().active.store(now, std::sync::atomic::Ordering::Relaxed);
        let _ = crate::share::keeper("health", sioul_sync::lease::Rule::FollowsYou, now, false);
    }

    fn touch(self: Pin<&mut Self>) {
        self.shared().active.store(jiff::Timestamp::now().as_second(), std::sync::atomic::Ordering::Relaxed);
    }

    fn set_health(self: Pin<&mut Self>, key: &QString, value: &QString) -> QString {
        QString::from(&crate::health::set_setting(&key.to_string(), &value.to_string()))
    }

    fn movement_minutes(&self) -> i32 {
        crate::health::movement_minutes()
    }

    fn chat_minute(self: Pin<&mut Self>) -> bool {
        crate::health::chat_minute()
    }

    fn chats_covered(&self) -> bool {
        crate::health::chats_covered()
    }

    fn memo_url(&self) -> QString {
        QString::from(&work::memo_url())
    }

    fn memo_recorded(self: Pin<&mut Self>, url: &QString) -> QString {
        let (qt, shared) = (self.qt_thread(), self.shared());
        work::show_work(&qt, &shared);
        QString::from(&work::note_path_of(&url.to_string()))
    }

    fn set_notes_tree(self: Pin<&mut Self>, tree: bool) {
        work::set_notes_tree(&self.qt_thread(), &self.shared(), tree);
    }

    fn view_flag(&self, name: &QString) -> bool {
        work::view_flag(&self.shared(), &name.to_string())
    }

    fn set_view_flag(self: Pin<&mut Self>, name: &QString, value: bool) {
        work::set_view_flag(&self.shared(), &name.to_string(), value);
    }

    fn add_movement(mut self: Pin<&mut Self>, edit: &QString) -> QString {
        #[derive(serde::Deserialize)]
        struct Movement {
            /// "once" or "recurring".
            kind: String,
            budget: String,
            label: String,
            amount: f64,
            #[serde(default)]
            date: String,
            /// Recurring: "month" or "year", the day, the month of a yearly one, from, until.
            #[serde(default)]
            every: String,
            #[serde(default)]
            day: i8,
            #[serde(default)]
            month: Option<i8>,
            #[serde(default)]
            from: String,
            #[serde(default)]
            until: String,
            #[serde(default)]
            estimate: bool,
        }
        let world = World::load();
        let outcome = (|| {
            let movement: Movement = serde_json::from_str(&edit.to_string()).map_err(|e| e.to_string())?;
            let ledger = world.ledger().ok_or_else(|| tr().text("ui-no-budgets", None))?;
            let path = world.ledger_path().ok_or_else(|| tr().text("ui-no-budgets", None))?;
            if movement.amount == 0.0 || !movement.amount.is_finite() {
                return Err(tr().text("budget-add-bad-amount", None));
            }
            let today = Zoned::now().date();
            let origin = say("budget-origin-hand", &[("date", today.to_string())]);
            if movement.kind == "recurring" {
                let preset = budget::PresetEdit {
                    budget: movement.budget.clone(),
                    label: movement.label.clone(),
                    amount: movement.amount,
                    every: movement.every.clone(),
                    day: movement.day,
                    month: movement.month,
                    from: movement.from.clone(),
                    until: movement.until.clone(),
                    estimate: movement.estimate,
                };
                budget::record_preset(&path, &preset, &origin)?;
            } else {
                let date: jiff::civil::Date = movement.date.parse().unwrap_or(today);
                let line = budget::Line {
                    budget: movement.budget.clone(),
                    date,
                    amount: sioul_core::money::Money::from_units(movement.amount),
                    label: movement.label.trim().to_string(),
                    planned: date > today,
                    reserve: None,
                    links: Vec::new(),
                    preset: None,
                };
                budget::record_line(&path, &line, &origin)?;
            }
            Ok::<String, String>(say("budget-added", &[("budget", ledger.budget_title(&movement.budget).to_string())]))
        })();
        let (line, problem) = match outcome {
            Ok(line) => (line, String::new()),
            Err(e) => (e.clone(), e),
        };
        self.as_mut().set_status(QString::from(&line));
        show(&self.qt_thread(), &self.shared());
        QString::from(&problem)
    }

    fn site_list(&self) -> QString {
        QString::from(&crate::sites::list())
    }

    fn site_notified(self: Pin<&mut Self>, id: &QString, title: &QString, text: &QString) {
        crate::sites::notified(&self.qt_thread(), &self.shared(), &id.to_string(), &title.to_string(), &text.to_string());
    }

    fn site_seen(self: Pin<&mut Self>, id: &QString) {
        crate::sites::seen(&self.qt_thread(), &id.to_string());
    }

    fn site_notices(&self) -> QString {
        QString::from(&crate::sites::waiting())
    }

    fn site_for_sender(&self, sender: &QString) -> QString {
        QString::from(&crate::sites::announced_by(&sender.to_string()))
    }

    fn set_site(mut self: Pin<&mut Self>, id: &QString, field: &QString, value: &QString) -> QString {
        let (field, value) = (field.to_string(), value.to_string());
        let setting = match field.as_str() {
            "realtime" | "muted" | "background" | "microphone" | "camera" | "screen" => config::SettingValue::Bool(value == "true"),
            "site" | "area" | "name" => config::SettingValue::Text(value.trim().to_string()),
            // Your categories, as a JSON list of words.
            "categories" => config::SettingValue::Texts(serde_json::from_str::<Vec<String>>(&value).unwrap_or_default().into_iter().map(|c| c.trim().to_string()).filter(|c| !c.is_empty()).collect()),
            // Only an encrypted address: a site's sign-in never travels in clear.
            "url" if value.trim().starts_with("https://") => config::SettingValue::Text(value.trim().to_string()),
            "url" => return QString::from(&tr().text("site-https-only", None)),
            _ => return QString::default(),
        };
        let result = config::set_value(&config_path(), &format!("site.{}.{field}", id), &setting);
        self.as_mut().sites_changed();
        QString::from(&result.err().unwrap_or_default())
    }

    fn site_presets(&self, country: &QString, region: &QString) -> QString {
        QString::from(&crate::sites::presets(&country.to_string(), &region.to_string()))
    }

    fn add_site(mut self: Pin<&mut Self>, edit: &QString) -> QString {
        let answer = crate::sites::add(&edit.to_string());
        self.as_mut().sites_changed();
        QString::from(&answer)
    }

    fn site_presets_tree(&self) -> QString {
        QString::from(&crate::sites::presets_tree())
    }

    fn add_sites(mut self: Pin<&mut Self>, rows: &QString) -> QString {
        let answer = crate::sites::add_many(&rows.to_string());
        self.as_mut().sites_changed();
        QString::from(&answer)
    }

    fn move_site(mut self: Pin<&mut Self>, id: &QString, delta: i32) -> QString {
        let said = crate::sites::move_site(&id.to_string(), i64::from(delta));
        self.as_mut().sites_changed();
        QString::from(&said)
    }

    fn remove_site(mut self: Pin<&mut Self>, id: &QString) -> QString {
        let problem = crate::sites::remove(&id.to_string());
        self.as_mut().sites_changed();
        QString::from(&problem)
    }

    fn google_built_in(&self) -> bool {
        sioul_sync::google::built_in().is_some()
    }

    fn call_devices(&self) -> QString {
        QString::from(&serde_json::to_string(&load_config().calls).unwrap_or_default())
    }

    fn set_call_device(mut self: Pin<&mut Self>, which: &QString, name: &QString) -> QString {
        let which = which.to_string();
        if !matches!(which.as_str(), "camera" | "microphone" | "speaker") {
            return QString::default();
        }
        let name = name.to_string().trim().to_string();
        let result = config::set_value(&config_path(), &format!("calls.{which}"), &config::SettingValue::Text(name));
        self.as_mut().sites_changed();
        QString::from(&result.err().unwrap_or_default())
    }

    fn bitwarden_state(&self) -> QString {
        QString::from(&crate::sites::bitwarden_state(&self.shared()))
    }

    fn bitwarden_unlock(self: Pin<&mut Self>, password: &QString, provider: i32, code: &QString) -> QString {
        QString::from(&crate::sites::bitwarden_unlock(&self.shared(), &password.to_string(), provider, &code.to_string()))
    }

    fn bitwarden_lock(self: Pin<&mut Self>) {
        crate::sites::bitwarden_lock(&self.shared());
    }

    fn bitwarden_send_code(self: Pin<&mut Self>, password: &QString) -> QString {
        QString::from(&crate::sites::bitwarden_send_code(&password.to_string()))
    }

    fn bitwarden_passkey_begin(self: Pin<&mut Self>) -> QString {
        QString::from(&crate::sites::bitwarden_passkey_begin())
    }

    fn bitwarden_passkey(self: Pin<&mut Self>, answer: &QString) -> QString {
        QString::from(&crate::sites::bitwarden_passkey(&self.shared(), &answer.to_string()))
    }

    fn bitwarden_logins(&self, url: &QString, query: &QString) -> QString {
        QString::from(&crate::sites::bitwarden_logins(&self.shared(), &url.to_string(), &query.to_string()))
    }

    fn bitwarden_login(&self, url: &QString, id: &QString) -> QString {
        QString::from(&crate::sites::bitwarden_login(&self.shared(), &url.to_string(), &id.to_string()))
    }

    fn map_view(&self) -> QString {
        QString::from(&crate::map::view(&self.shared()))
    }

    fn place_of(&self, address: &QString) -> QString {
        QString::from(&crate::map::place_of(&address.to_string()))
    }

    fn locate_addresses(mut self: Pin<&mut Self>, allow: bool) {
        if allow && !load_config().map.geocode {
            if let Err(e) = config::set_value(&config_path(), "map.geocode", &config::SettingValue::Bool(true)) {
                self.as_mut().set_status(QString::from(&e));
                return;
            }
        }
        crate::map::locate(&self.qt_thread(), &self.shared());
    }

    fn refresh_mode(mut self: Pin<&mut Self>) {
        // The plan, made again on the clock.
        let now = Zoned::now();
        let (stamp, day) = (now.timestamp().as_second(), i64::from(now.date().year()) * 1000 + i64::from(now.date().day_of_year()));
        let shared = self.shared();
        let last = shared.planned_at.load(Ordering::Relaxed);
        if last == 0 {
            shared.planned_at.store(stamp, Ordering::Relaxed);
            shared.planned_day.store(day, Ordering::Relaxed);
        } else if stamp - last >= REPLAN_EVERY || shared.planned_day.load(Ordering::Relaxed) != day {
            shared.planned_at.store(stamp, Ordering::Relaxed);
            shared.planned_day.store(day, Ordering::Relaxed);
            work::show_work(&self.qt_thread(), &shared);
        }
        // Doses to remind, errands to make: off the window's thread.
        let (qt, shared_tick) = (self.qt_thread(), self.shared());
        std::thread::spawn(move || crate::health::tick(&qt, &shared_tick));
        // What changed here goes to your other computers, theirs comes in.
        crate::share::exchange(&self.qt_thread(), &self.shared());
        // Invoices stay with the computer that makes them: its claim renewed.
        crate::projects::keep_invoices(&self.shared());
        // What sites notified, gathered at the times you set.
        crate::sites::gather_tick(&self.qt_thread(), &self.shared());
        // Your sites' own icons, when missing or a week old.
        crate::sites::favicon_tick(&self.qt_thread());
        // Dates coming: told once each.
        crate::remind::tick(&self.qt_thread());
        // Paper letters scanned: read, to wait for the Porch.
        crate::letters::tick(&self.qt_thread(), &self.shared());
        // The weather follows the hours; fetched again when half an hour old.
        update_weather(&self.qt_thread(), &self.shared());
        let line = mode_json();
        if self.as_ref().mode().to_string() != line {
            let was_quiet = self.as_ref().mode().to_string().contains("\"quiet\":true");
            self.as_mut().set_mode(QString::from(&line));
            // Work came or went: what the pages show follows.
            if was_quiet != line.contains("\"quiet\":true") {
                let (qt, shared) = (self.qt_thread(), self.shared());
                show(&qt, &shared);
                work::show_work(&qt, &shared);
            }
        }
    }

    fn work_a_while(self: Pin<&mut Self>, minutes: i32) {
        let until = Zoned::now().timestamp().as_second() + i64::from(minutes.max(1)) * 60;
        self.change_overrides(|o| {
            o.rest_until = None;
            o.work_until = Some(until);
        });
    }

    pub(crate) fn done_for_the_day(mut self: Pin<&mut Self>) -> QString {
        let (qt, shared) = (self.qt_thread(), self.shared());
        let closed = work::closing(&shared);
        let path = sioul_core::quiet::Overrides::default_path();
        let previous = sioul_core::quiet::Overrides::load(&path);
        let mut next = previous.clone();
        next.work_until = None;
        next.rest_until = Some(closed.back.timestamp().as_second());
        next.rest_from = Some(Zoned::now().timestamp().as_second());
        next.first_step = Some(closed.first.text.clone()).filter(|s| !s.is_empty());
        next.first_step_task = Some(closed.first.uid.clone()).filter(|s| !s.is_empty());
        next.first_step_day = Some(closed.back.date());
        if let Err(e) = next.save(&path) {
            self.as_mut().set_status(QString::from(&e));
            return QString::default();
        }
        // A session running stops here, and counts.
        if sioul_core::timelog::running().is_some() {
            work::focus_stop(&qt, &shared, false, "");
        }
        let screen = json(&closed.closing);
        mail::rest(&qt, &shared, previous, serde_json::from_str::<serde_json::Value>(&screen).ok().and_then(|v| v["put_away"].as_str().map(str::to_string)).unwrap_or_default());
        self.as_mut().set_mode(QString::from(&mode_json()));
        show(&qt, &shared);
        work::show_work(&qt, &shared);
        QString::from(&screen)
    }

    fn set_first_step(self: Pin<&mut Self>, text: &QString) {
        let text = text.to_string().trim().to_string();
        self.change_overrides(|o| {
            if o.first_step.as_deref() != Some(text.as_str()) {
                // Your own words: the task it named may not be the one any more.
                o.first_step_task = None;
                o.first_step = Some(text).filter(|t| !t.is_empty());
            }
        });
    }

    fn clear_first_step(self: Pin<&mut Self>) {
        self.change_overrides(|o| {
            o.first_step = None;
            o.first_step_task = None;
        });
    }

    fn usual_hours(self: Pin<&mut Self>) {
        self.change_overrides(|o| {
            o.work_until = None;
            o.rest_until = None;
            o.work_now = None;
        });
    }

    fn set_work_now(self: Pin<&mut Self>, on: bool) {
        let config = load_config();
        let work: Vec<_> = config.windows.iter().filter(|w| w.kind() == "work").cloned().collect();
        let until = sioul_core::quiet::end_of_next_workday(&work, &config.time_off, &Zoned::now()).timestamp().as_second();
        self.change_overrides(|o| {
            // Unticked: back to the usual hours, a little longer included.
            o.work_until = None;
            if on {
                o.rest_until = None;
                o.work_now = Some(until);
            } else {
                o.work_now = None;
            }
        });
    }

    /// The overrides changed, then every page shown again for the time it is.
    fn change_overrides(mut self: Pin<&mut Self>, change: impl FnOnce(&mut sioul_core::quiet::Overrides)) {
        let path = sioul_core::quiet::Overrides::default_path();
        let mut overrides = sioul_core::quiet::Overrides::load(&path);
        change(&mut overrides);
        // What the status line said of the time before is old news now.
        let said = overrides.save(&path).err().unwrap_or_default();
        self.as_mut().set_status(QString::from(&said));
        self.as_mut().set_mode(QString::from(&mode_json()));
        let (qt, shared) = (self.qt_thread(), self.shared());
        show(&qt, &shared);
        work::show_work(&qt, &shared);
    }

    fn project_rows(&self) -> QString {
        QString::from(&crate::projects::rows(&self.shared()))
    }

    fn project_page(&self, id: &QString) -> QString {
        QString::from(&crate::projects::page(&self.shared(), &id.to_string()))
    }

    fn save_project(self: Pin<&mut Self>, id: &QString, edit: &QString) -> QString {
        QString::from(&crate::projects::save(&self.qt_thread(), &self.shared(), &id.to_string(), &edit.to_string()))
    }

    fn time_page(&self, period: &QString, anchor: &QString, project: &QString) -> QString {
        QString::from(&crate::projects::time(&self.shared(), &period.to_string(), &anchor.to_string(), &project.to_string()))
    }

    fn note_time(self: Pin<&mut Self>, edit: &QString) -> QString {
        QString::from(&crate::projects::note_time(&self.qt_thread(), &self.shared(), &edit.to_string()))
    }

    fn remove_time(self: Pin<&mut Self>, key: &QString) -> QString {
        QString::from(&crate::projects::remove_time(&self.qt_thread(), &self.shared(), &key.to_string()))
    }

    fn take_invoices(self: Pin<&mut Self>) -> QString {
        QString::from(&crate::projects::take_invoices(&self.shared()))
    }

    fn make_invoice(self: Pin<&mut Self>, id: &QString) -> QString {
        QString::from(&crate::projects::make_invoice(&self.qt_thread(), &self.shared(), &id.to_string()))
    }

    fn invoice_again(&self, number: &QString) -> QString {
        QString::from(&crate::projects::invoice_again(&number.to_string()))
    }

    fn set_invoice_paid(self: Pin<&mut Self>, number: &QString, paid: bool) -> QString {
        QString::from(&crate::projects::set_paid(&self.qt_thread(), &self.shared(), &number.to_string(), paid))
    }

    fn make_linked(self: Pin<&mut Self>, kind: &QString, from: &QString, key: &QString, start: f64) -> QString {
        QString::from(&work::make_linked(&self.qt_thread(), &self.shared(), &kind.to_string(), &from.to_string(), &key.to_string(), start))
    }

    fn task_from_mail(self: Pin<&mut Self>, key: &QString) -> QString {
        QString::from(&work::task_from_mail(&self.qt_thread(), &self.shared(), &key.to_string()))
    }

    fn note_from_mail(self: Pin<&mut Self>, key: &QString) -> QString {
        QString::from(&work::note_from_mail(&self.qt_thread(), &self.shared(), &key.to_string()))
    }

    fn note_from_event(self: Pin<&mut Self>, key: &QString, start: f64) -> QString {
        QString::from(&work::note_from_event(&self.qt_thread(), &self.shared(), &key.to_string(), start))
    }

    fn task_from_event(self: Pin<&mut Self>, key: &QString, start: f64) -> QString {
        QString::from(&work::task_from_event(&self.qt_thread(), &self.shared(), &key.to_string(), start))
    }

    fn task_from_line(self: Pin<&mut Self>, path: &QString, line: i32) -> QString {
        QString::from(&work::task_from_line(&self.qt_thread(), &self.shared(), &path.to_string(), line))
    }

    fn draft_for_task(self: Pin<&mut Self>, uid: &QString) -> QString {
        QString::from(&work::draft_for_task(&self.qt_thread(), &self.shared(), &uid.to_string()))
    }

    fn mail_note(self: Pin<&mut Self>, path: &QString) -> QString {
        QString::from(&work::mail_note(&self.qt_thread(), &self.shared(), &path.to_string()))
    }

    fn settings(&self, view: &QString) -> QString {
        let config = load_config();
        let lists: Vec<(String, String)> = sioul_core::tasks::lists().into_iter().filter(|c| !c.read_only).map(|c| (format!("{}/{}", c.account, c.id), c.label(&config, tr()))).collect();
        let store = config.case_store_path().and_then(|root| CaseStore::load(&root).ok());
        let mut rows = sioul_core::settings::for_view(&view.to_string(), &config, tr(), &lists, store.as_ref());
        // The categories are the tasks': the core does not read them.
        if view.to_string() == "tasks" {
            let categories = work::categories_in_use(&self.shared());
            if !categories.is_empty() {
                let at = rows.iter().position(|r| r.key == "tasks.kind").map_or(rows.len(), |i| i + 1);
                rows.insert(at, sioul_core::settings::categories(tr(), categories));
            }
        }
        // Whether the keyring keeps a key: the core does not read the keyring.
        for row in rows.iter_mut().filter(|r| r.key == "ai_key") {
            row.value = config::SettingValue::Bool(sioul_sync::shield_ai::api_key().is_some());
        }
        for row in rows.iter_mut().filter(|r| r.key == "github_token") {
            row.value = config::SettingValue::Bool(sioul_sync::github::token().is_some());
        }
        // Whether the session starts the reminders' watcher: the desktop's own entry says.
        for row in rows.iter_mut().filter(|r| r.key == "reminders_closed") {
            row.value = config::SettingValue::Bool(crate::remind::background());
        }
        for row in rows.iter_mut().filter(|r| r.key == "passwords_shown") {
            row.value = config::SettingValue::Bool(work::view_flag(&self.shared(), "passwords-shown"));
        }
        QString::from(&json(&rows))
    }

    fn set_setting(mut self: Pin<&mut Self>, key: &QString, value: &QString) -> QString {
        let key = key.to_string();
        let parsed: Result<config::SettingValue, String> = serde_json::from_str(&value.to_string()).map_err(|e| e.to_string());
        let result = parsed.and_then(|v| match (key.as_str(), v) {
            // The AI's key goes to the keyring, never to the configuration; an empty one is forgotten.
            ("ai_key", config::SettingValue::Text(secret)) => sioul_sync::shield_ai::save_api_key(&secret).map_err(|e| e.sentence(tr(), "Anthropic")),
            ("github_token", config::SettingValue::Text(secret)) => sioul_sync::github::save_token(&secret).map_err(|e| e.sentence(tr(), "GitHub")),
            ("task_categories", config::SettingValue::Rename(change)) => work::rename_category(&self.qt_thread(), &self.shared(), &change.from, &change.to),
            ("reminders_closed", config::SettingValue::Bool(on)) => crate::remind::set_background(on),
            ("passwords_shown", config::SettingValue::Bool(on)) => {
                work::set_view_flag(&self.shared(), "passwords-shown", on);
                Ok(())
            }
            (_, v) => sioul_core::settings::apply(&config_path(), &load_config(), &key, &v),
        });
        if let Err(e) = result {
            return QString::from(&e);
        }
        // What the setting changes is shown again.
        let (qt, shared) = (self.qt_thread(), self.shared());
        if let Some(case) = key.strip_prefix("case.").and_then(|k| k.strip_suffix(".routes")) {
            retie_case(case.to_string());
        }
        // A list, a calendar, an address book renamed or deleted: told to the server now.
        if key == "collections"
            && let Some(account) = serde_json::from_str::<serde_json::Value>(&value.to_string()).ok().and_then(|v| v["from"].as_str().and_then(|f| f.split('/').nth(1)).map(str::to_string))
        {
            pim::nudge(&shared, &account);
        }
        if key.starts_with("reading.") || key == "theme" {
            self.as_mut().set_reading(QString::from(&reading_json()));
        }
        if key.contains("fetch_minutes") {
            update_paces(&shared);
        }
        // Hours or days off changed: what the time is for now, and what the Porch asks.
        if key.starts_with("window") || key == "time_off" {
            self.as_mut().set_mode(QString::from(&mode_json()));
        }
        if key.starts_with("mail.") {
            mail::show_mail(&qt, &shared);
        }
        // GitHub: started, brought again, or stopped.
        if key.starts_with("github") {
            if load_config().github.enabled {
                crate::github::start(&qt, &shared);
            } else {
                crate::github::stop(&shared);
            }
        }
        show(&qt, &shared);
        pim::show_pim(&qt, &shared);
        work::show_work(&qt, &shared);
        QString::default()
    }

    fn forced_theme(&self) -> QString {
        QString::from(&std::env::var("SIOUL_THEME").unwrap_or_default())
    }

    fn set_realtime_mode(mut self: Pin<&mut Self>, on: bool) {
        let shared = self.shared();
        shared.realtime.store(on, Ordering::Relaxed);
        shared.opened_anyway.store(on || shared.opened_anyway.load(Ordering::Relaxed), Ordering::Relaxed);
        update_paces(&shared);
        self.as_mut().set_realtime(on);
        let line = tr().text(if on { "realtime-on" } else { "realtime-off" }, None);
        self.as_mut().set_status(QString::from(&line));
        show(&self.qt_thread(), &shared);
    }

    fn grab_phone(&self) -> bool {
        std::env::var_os("SIOUL_GRAB_PHONE").is_some()
    }

    fn grab_steps(&self) -> QString {
        let steps = std::env::var("SIOUL_GRAB_STEPS").unwrap_or_else(|_| "pages".into());
        // The other steps archive, delete and send mail, answer invitations:
        // against test servers, from the test build only. "demo" and "phone" take pictures alone.
        if cfg!(feature = "insecure-test-tls") || steps == "demo" || steps == "phone" {
            return QString::from(&steps);
        }
        QString::from("pages")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn file_addresses_reach_the_file() {
        assert_eq!(file_url(Path::new("/tmp/Réunion #2 100%.pdf")), "file:///tmp/R%C3%A9union%20%232%20100%25.pdf");
        assert_eq!(local_path(&file_url(Path::new("/tmp/Réunion #2 100%.pdf"))), PathBuf::from("/tmp/Réunion #2 100%.pdf"));
        assert_eq!(local_path("file://localhost/tmp/a%20b"), PathBuf::from("/tmp/a b"));
        assert_eq!(local_path("/tmp/plain"), PathBuf::from("/tmp/plain"));
        // Windows' forms, both ways.
        assert_eq!(windows_slashed(r"C:\Users\me\a b.pdf"), "/C:/Users/me/a b.pdf");
        assert_eq!(windows_slashed(r"\\?\C:\Users\me"), "/C:/Users/me");
        assert_eq!(windows_slashed(r"\\server\share\a"), "//server/share/a");
        assert_eq!(windows_slashed(r"\\?\UNC\server\share\a"), "//server/share/a");
        assert_eq!(windows_path("/C:/Users/me/a b.pdf"), r"C:\Users\me\a b.pdf");
        assert_eq!(windows_path("server/share/a"), r"\\server\share\a");
    }

    #[test]
    fn programs_are_not_opened() {
        for name in ["setup.exe", "Facture.PDF.exe", "invoice.exe.", "invoice.exe ", "run.sh", "x.desktop", "a.lnk", "b.js", "c.terminal", "d.iso"] {
            assert!(sioul_core::links::is_program(name), "{name}");
        }
        for name in ["facture.pdf", "photo.jpeg", "notes.txt", "archive.zip", "exe", "README"] {
            assert!(!sioul_core::links::is_program(name), "{name}");
        }
    }

    #[test]
    fn new_sentences_read_in_both_languages() {
        for language in ["en", "fr"] {
            let tr = Translator::new(language);
            for id in ["attachment-program", "budget-origin-mail", "budget-origin-hand", "note-outside-notes", "setting-unknown-key", "reminders-entry-name", "reminders-entry-comment", "bank-rule-gone", "budget-line-gone", "mail-no-such-folder", "mail-not-in-accounts", "mail-move-needs-to", "list-read-only-mark", "import-bad-key"] {
                assert_ne!(tr.text(id, None), id, "{language}: {id}");
            }
        }
    }

    #[test]
    fn attachment_names_stay_in_their_folder() {
        assert_eq!(one_file_name("a/b\\c.pdf"), "a_b_c.pdf");
        assert_eq!(one_file_name(".."), "attachment");
        assert_eq!(one_file_name(""), "attachment");
        if cfg!(windows) {
            assert_eq!(one_file_name("C:evil.txt"), "C_evil.txt");
            assert_eq!(one_file_name("con.txt"), "_con.txt");
        } else {
            assert_eq!(one_file_name("C:evil.txt"), "C:evil.txt");
        }
    }
}
