// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

//! `sioul`: the Porch, cases, admin windows, budgets and mail accounts, from the terminal.
//!
//! The terminal is the first interface because agents and scripts use it too;
//! the Qt interface comes on top of the same core (docs/roadmap.md). Every
//! sentence goes through the translator: the language is the configuration's,
//! else the session's (docs/i18n.md).

mod accounts;
mod dav;
mod mail;
mod mcp;
mod pgp;
mod remind;
mod tasks;

use clap::{Parser, Subcommand};
use jiff::{Timestamp, Zoned, tz::TimeZone};
use sioul_core::budget::{self, Ledger};
use sioul_core::cases::CaseStore;
use sioul_core::config::{self, Config, Source};
use sioul_core::i18n::{self, Translator};
use sioul_core::porch::{self, Context, KnownSenders, Lane, SenderList, Triaged};
use sioul_core::state::{MoneyState, PorchState};
use sioul_core::{maildir, trust, view, window};
use std::path::{Path, PathBuf};
use std::process::ExitCode;

#[derive(Parser)]
#[command(name = "sioul", version, about = "Calm admin: the Porch, cases and admin windows.")]
struct Cli {
    /// Configuration file (default: ~/.config/sioul/config.toml).
    #[arg(long, global = true)]
    config: Option<PathBuf>,
    /// Language of the sentences ("fr", "en"); default: the configuration's, else the session's.
    #[arg(long, global = true)]
    language: Option<String>,
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// What came, checked and sorted. Outside admin windows, only one-time codes show.
    Porch {
        /// Read this Maildir or folder of .eml files instead of the configured accounts.
        #[arg(long)]
        maildir: Vec<PathBuf>,
        /// Show everything now, even outside an admin window.
        #[arg(long)]
        open: bool,
        /// Also show what was there when the Porch was last closed.
        #[arg(long)]
        all: bool,
    },
    /// Closes the Porch on what it shows: what comes next waits for the next window.
    Done,
    /// One message: its card, its lane and the reasons (a developer's view).
    Card { file: PathBuf },
    /// The cases of the case store, and the files they name.
    Cases {
        /// The case store (default: case_store in the configuration).
        #[arg(long)]
        store: Option<PathBuf>,
    },
    /// When the next admin window opens.
    Window,
    /// Budgets and reserves at a glance, and the lines mail proposes.
    Budgets {
        /// The budget file (default: sioul-budgets.toml at the root of the case store).
        #[arg(long)]
        file: Option<PathBuf>,
        /// Read mail about money in this Maildir or folder of .eml files, instead of the accounts'.
        #[arg(long)]
        maildir: Vec<PathBuf>,
    },
    /// Mail accounts: add, list, test, change a password, remove.
    #[command(subcommand)]
    Account(accounts::AccountCommand),
    /// The mail client: folders, messages, what you do to them, writing and sending.
    #[command(subcommand)]
    Mail(mail::MailCommand),
    /// OpenPGP keys: yours and others'.
    #[command(subcommand)]
    Pgp(pgp::PgpCommand),
    /// Contacts and calendars on a CardDAV and CalDAV server: add an account, sync.
    #[command(subcommand)]
    Dav(dav::DavCommand),
    /// Your contacts, or those matching a name, an address, a number.
    Contacts { query: Option<String> },
    /// Adds a contact to the first address book, and sends it.
    Contact {
        #[arg(long)]
        name: String,
        #[arg(long)]
        email: Vec<String>,
        #[arg(long)]
        phone: Vec<String>,
    },
    /// What comes: today, then the next days.
    Agenda {
        #[arg(long, default_value_t = 15, value_parser = clap::value_parser!(i64).range(1..=366))]
        days: i64,
    },
    /// Adds an event to the first calendar, and sends it. Times: "2026-10-05T09:00", or a date with --all-day.
    Event {
        #[arg(long)]
        title: String,
        #[arg(long)]
        start: String,
        #[arg(long)]
        end: Option<String>,
        #[arg(long)]
        all_day: bool,
        #[arg(long)]
        location: Option<String>,
        /// daily, weekly, monthly or yearly.
        #[arg(long)]
        repeat: Option<String>,
    },
    /// Blocks a sender (an address, or @domain for everyone there): their mail is set aside
    /// for good, never shown, never counted. Without an argument, lists who is blocked.
    Block { entry: Option<String> },
    /// Unblocks a sender.
    Unblock { entry: String },
    /// Checks SPF, DKIM, DMARC, ARC and reverse DNS on the mail Sioul has not checked yet
    /// (mail fetched from now on is checked as it arrives).
    Verify {
        /// Check all the mail again, replacing earlier results.
        #[arg(long)]
        again: bool,
    },
    /// Tasks: the next step, the list, the board, the timeline; adding, importing.
    #[command(subcommand)]
    Tasks(tasks::TasksCommand),
    /// A focus session on one task: start, status, stop.
    #[command(subcommand)]
    Focus(tasks::FocusCommand),
    /// What a thing is tied to, both ways: "sioul:task/<UID>", "mid:<Message-ID>", "sioul:note/<path>"…
    Links { uri: String },
    /// Notes of the case store: those matching, or one with --path.
    Notes {
        query: Vec<String>,
        #[arg(long)]
        path: Option<String>,
    },
    /// Fetches new mail, changing nothing on the server: from every account, or one.
    Sync {
        #[arg(long)]
        account: Option<String>,
    },
    /// Stays open and fetches mail as it arrives; verified codes come as notifications.
    Watch,
    /// Reminders before dates: what comes in the next two weeks, and what was told.
    Remind {
        /// Stays open, with Sioul's window closed too: each minute, what is due becomes one
        /// quiet notification, never repeated (the window marks the same ones).
        #[arg(long)]
        watch: bool,
    },
    /// What the shield of a public address makes of its mail: tone, topic, and the words
    /// that weighed. Hostile mail shows neither its sender's name nor its subject.
    Shield {
        /// The account, by its id; every shielded one by default.
        #[arg(long)]
        account: Option<String>,
        /// First lets the AI read what it has not read yet, where the address allows it.
        #[arg(long)]
        read: bool,
    },
    /// Serves Sioul to AI agents (Claude Code, Claude Desktop…) over MCP, on standard input
    /// and output: they read and write this computer's files only, and never send (docs/mcp.md).
    Mcp,
}

/// What every command needs: the configuration, where it lives, and the language.
pub(crate) struct Session {
    pub config: Config,
    pub config_path: PathBuf,
    pub tr: Translator,
}

impl Session {
    /// A sentence with its arguments.
    pub fn say(&self, id: &str, pairs: &[(&str, String)]) -> String {
        let mut args = i18n::args();
        for (key, value) in pairs {
            // A whole number goes as a number, so that "one" and "other" choose by
            // it ("1 message", "2 messages"); one written otherwise ("007") stays as written.
            match value.parse::<i32>() {
                Ok(n) if n.to_string() == *value => args.set(key.to_string(), n),
                _ => args.set(key.to_string(), value.clone()),
            }
        }
        self.tr.text(id, Some(&args))
    }
}

/// Words from elsewhere (mail, invitations, contacts, file names) on one
/// line, as they are shown: a line break or a tab becomes a space, and the
/// other control characters go (a terminal's escape sequences, the marks that
/// turn text right to left), so that they can neither break a line of what is
/// shown, nor pass for it, nor drive the terminal.
pub(crate) fn one_line(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for c in text.chars() {
        match c {
            '\t' | '\n' | '\r' | '\u{b}' | '\u{c}' | '\u{85}' | '\u{2028}' | '\u{2029}' => {
                if !out.ends_with(' ') {
                    out.push(' ');
                }
            }
            c if hidden(c) => {}
            c => out.push(c),
        }
    }
    out.trim().to_string()
}

/// Words from elsewhere that keep their lines (a task's notes, what a server
/// answered): line breaks and tabs stay, the other control characters go, as
/// in `one_line`.
pub(crate) fn plain_lines(text: &str) -> String {
    text.replace("\r\n", "\n")
        .chars()
        .map(|c| if matches!(c, '\r' | '\u{b}' | '\u{c}' | '\u{85}' | '\u{2028}' | '\u{2029}') { '\n' } else { c })
        .filter(|&c| c == '\n' || c == '\t' || !hidden(c))
        .collect()
}

/// A control character (escape sequences start with one), or a mark that
/// turns text right to left: never printed.
fn hidden(c: char) -> bool {
    c.is_control() || matches!(c, '\u{202a}'..='\u{202e}' | '\u{2066}'..='\u{2069}')
}

/// The session's language: its variables (LC_ALL, LC_MESSAGES, LANG) when it
/// has them, as in a Linux terminal; else the system's own setting, as
/// Windows' and macOS' terminals may set none (as sioul-app's backend.rs).
fn session_language() -> String {
    let said = ["LC_ALL", "LC_MESSAGES", "LANG"].iter().any(|v| std::env::var(v).is_ok_and(|l| !l.trim().is_empty()));
    if said {
        return i18n::system_language();
    }
    system_locale().unwrap_or_else(i18n::system_language)
}

/// Windows' language for the user ("fr-FR").
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

fn main() -> ExitCode {
    let cli = Cli::parse();
    // Mail, keys, drafts and caches are kept in folders that are yours alone.
    config::make_private_dirs();
    let config_path = cli.config.clone().unwrap_or_else(config::default_path);
    // No file yet: Sioul's defaults. A file that cannot be read is said, not taken for none.
    let config = Config::load(&config_path).unwrap_or_else(|e| {
        if config_path.exists() {
            eprintln!("{e}");
        }
        Config::default()
    });
    let language = cli.language.clone().or_else(|| config.language.clone()).unwrap_or_else(session_language);
    let session = Session { tr: Translator::new(&language), config, config_path };
    let result = match cli.command {
        Command::Porch { maildir, open, all } => porch_command(&session, &maildir, open, all),
        Command::Done => done_command(&session),
        Command::Card { file } => card_command(&session, &file),
        Command::Cases { store } => cases_command(&session, store),
        Command::Window => window_command(&session),
        Command::Budgets { file, maildir } => budgets_command(&session, file, &maildir),
        Command::Account(command) => accounts::run(&session, command),
        Command::Mail(command) => mail::run(&session, command),
        Command::Dav(command) => dav::run(&session, command),
        Command::Pgp(command) => pgp::run(&session, command),
        Command::Contacts { query } => dav::list_contacts(&session, query.as_deref().unwrap_or("")),
        Command::Contact { name, email, phone } => dav::new_contact(&session, &name, &email, &phone),
        Command::Agenda { days } => dav::list_agenda(&session, days),
        Command::Event { title, start, end, all_day, location, repeat } => {
            let edit = sioul_core::agenda::EventEdit {
                end: end.unwrap_or_else(|| start.clone()),
                title,
                start,
                all_day,
                location: location.unwrap_or_default(),
                repeat: repeat.unwrap_or_default(),
                notes: String::new(),
                ..sioul_core::agenda::EventEdit::default()
            };
            dav::new_event(&session, &edit)
        }
        Command::Block { entry } => block_command(&session, entry.as_deref()),
        Command::Unblock { entry } => unblock_command(&session, &entry),
        Command::Verify { again } => verify_command(&session, again),
        Command::Tasks(command) => tasks::run(&session, command),
        Command::Focus(command) => tasks::focus(&session, command),
        Command::Links { uri } => tasks::links_command(&session, &uri),
        Command::Notes { query, path } => tasks::notes_command(&session, &query.join(" "), path.as_deref()),
        Command::Sync { account } => accounts::sync_command(&session, account.as_deref()),
        Command::Watch => accounts::watch_command(&session),
        Command::Remind { watch: false } => remind::list(&session),
        Command::Remind { watch: true } => remind::watch(&session),
        Command::Shield { account, read } => shield_command(&session, account.as_deref(), read),
        Command::Mcp => mcp::run(&session),
    };
    result.map_or_else(
        |message| {
            // What went wrong may quote a server's answer, or a title from a mail.
            eprintln!("{}", plain_lines(&message));
            ExitCode::FAILURE
        },
        |()| ExitCode::SUCCESS,
    )
}

/// The configured accounts' mail, or the folders given instead.
fn sources(config: &Config, maildirs: &[PathBuf]) -> Vec<Source> {
    if maildirs.is_empty() {
        return config.mail_sources();
    }
    let ids = config.all_trusted_ids();
    maildirs.iter().map(|m| Source { account: None, address: None, folder: m.clone(), trusted_ids: ids.clone(), priority: Default::default(), shielded: false, filed_words: Vec::new() }).collect()
}

pub(crate) fn load_store(config: &Config) -> Option<CaseStore> {
    let root = config.case_store_path()?;
    // With the mail tied to cases, so their conversations follow.
    let ties = sioul_core::links::LocalLinks::load(&sioul_core::links::LocalLinks::default_path());
    CaseStore::load(&root).map_err(|e| eprintln!("{}", plain_lines(&e))).ok().map(|s| s.with_ties(&ties))
}

fn porch_command(s: &Session, maildirs: &[PathBuf], open: bool, all: bool) -> Result<(), String> {
    let sources = sources(&s.config, maildirs);
    if sources.is_empty() {
        return Err(s.tr.text("error-no-mail", None));
    }
    let store = load_store(&s.config);
    let known = KnownSenders::load(&s.config.known_senders_path());
    let senders = porch::Senders::load(&s.config);
    let state = if all { PorchState::default() } else { PorchState::load(&PorchState::default_path()) };
    let now = Zoned::now();
    let triaged = porch::gather(&sources, store.as_ref(), &known, &senders, &state, now.timestamp().as_second());
    print_right_now(s, &triaged);
    let is_open = open || s.config.windows.is_empty() || window::current(&s.config.windows, &now).is_some();
    if is_open {
        print_open_porch(s, &triaged, store.as_ref());
    } else {
        print_closed_porch(s, &now);
    }
    Ok(())
}

fn shield_command(s: &Session, account: Option<&str>, read: bool) -> Result<(), String> {
    use sioul_core::shield::{self, AiCache, Tone};
    let accounts: Vec<_> = s.config.accounts.iter().filter(|a| a.shield && account.is_none_or(|id| a.id == id)).collect();
    if accounts.is_empty() {
        return Err(s.tr.text("shield-none", None));
    }
    if read {
        let done = sioul_sync::shield_ai::read_new(&s.config).map_err(|e| e.sentence(&s.tr, "Anthropic"))?;
        println!("{}", s.say("shield-read", &[("n", done.read.to_string()), ("left", done.left.to_string())]));
    }
    for account in accounts {
        println!("{}", account.address.as_deref().unwrap_or(&account.id));
        let cache = AiCache::load(&account.id);
        // Newest first, as the Porch reads a public address.
        for card in maildir::read_messages(&account.maildir_path()).iter().rev() {
            let by_ai = card.message_id.as_deref().and_then(|id| cache.messages.get(&sioul_core::mailindex::bare_id(id)));
            let assessment = by_ai.cloned().unwrap_or_else(|| shield::assess(&card.subject, &card.excerpt));
            let tone = s.tr.text(&format!("tone-{}", match assessment.tone { Tone::Calm => "calm", Tone::Rude => "rude", Tone::Hostile => "hostile" }), None);
            let (sender, subject) = if assessment.tone == Tone::Hostile {
                let domain = card.from_address.as_deref().and_then(|a| a.rsplit_once('@')).map(|(_, d)| d.to_string()).unwrap_or_default();
                (s.say("hostile-someone-at", &[("domain", domain)]), s.tr.text("hostile-subject", None))
            } else {
                (card.sender().to_string(), card.subject.clone())
            };
            println!("  {tone:<8} {:<16} {} · {}", s.tr.topic(assessment.topic), one_line(&sender), one_line(&subject));
            if !assessment.summary.is_empty() {
                println!("           {}", one_line(&assessment.summary));
            } else if !assessment.words.is_empty() && assessment.tone != Tone::Hostile {
                println!("           {}", one_line(&s.say("shield-words", &[("words", assessment.words.join(", "))])));
            }
        }
    }
    Ok(())
}

fn done_command(s: &Session) -> Result<(), String> {
    let path = PorchState::default_path();
    let mut state = PorchState::load(&path);
    let senders = porch::Senders::load(&s.config);
    let shown = porch::gather(&s.config.mail_sources(), None, &KnownSenders::default(), &senders, &state, Timestamp::now().as_second());
    let newest = PorchState::newest_shown(&shown);
    if newest.is_empty() {
        println!("{}", s.tr.text("done-nothing", None));
        return Ok(());
    }
    state.close(&newest);
    state.save(&path)?;
    println!("{}", s.tr.text("done-closed", None));
    Ok(())
}

/// The one exception to the windows: verified codes, shown whatever the time.
pub(crate) fn print_right_now(s: &Session, triaged: &[Triaged]) {
    let codes: Vec<&Triaged> = triaged.iter().filter(|t| t.lane == Lane::RightNow).collect();
    if codes.is_empty() {
        return;
    }
    println!("{}", s.tr.text("right-now-title", None));
    for t in codes {
        let Some(code) = &t.code else { continue };
        println!("  {}", one_line(&s.tr.right_now(code.kind, t.card.sender(), t.trust, code.code.as_deref(), code.expires_minutes)));
    }
    println!();
}

fn print_closed_porch(s: &Session, now: &Zoned) {
    let next = window::next_opening(&s.config.windows, now);
    let mut args = sioul_core::i18n::args();
    args.set("when", next.map_or_else(String::new, |z| s.tr.when(&z)));
    println!("{}", s.tr.text("porch-closed", Some(&args)));
    println!("{}", s.tr.text("porch-open-hint", None));
}

fn print_open_porch(s: &Session, triaged: &[Triaged], store: Option<&CaseStore>) {
    println!("{}\n", plain_lines(&s.tr.summary(&porch::summarise(triaged), store)));
    for case in store.map_or(&[][..], |st| &st.cases[..]) {
        print_lane(s, &case.title, triaged, &Lane::Case(case.id.clone()), false, store);
    }
    print_lane(s, &s.tr.text("lane-people", None), triaged, &Lane::People, false, store);
    print_lane(s, &s.tr.text("lane-screener", None), triaged, &Lane::Screener, true, store);
    print_lane(s, &s.tr.text("lane-filed", None), triaged, &Lane::Filed, false, store);
    print_lane(s, &s.tr.text("lane-low", None), triaged, &Lane::Low, false, store);
    print_lane(s, &s.tr.text("lane-set-aside", None), triaged, &Lane::SetAside, true, store);
}

fn print_lane(s: &Session, title: &str, triaged: &[Triaged], lane: &Lane, with_reason: bool, store: Option<&CaseStore>) {
    let items: Vec<&Triaged> = triaged.iter().filter(|t| &t.lane == lane).collect();
    if items.is_empty() {
        return;
    }
    println!("{}", one_line(title));
    for t in items {
        // The last reason is the one that chose the lane, or the warning that follows it.
        let reason = match (with_reason, t.reasons.last()) {
            (true, Some(r)) => format!(" ({})", one_line(&s.tr.reason(r, store))),
            _ => String::new(),
        };
        println!("  · {} ({}) · {} · {}{reason}", one_line(t.card.sender()), s.tr.trust(t.trust), one_line(&t.card.subject), date(s, t.card.date));
    }
    println!();
}

fn card_command(s: &Session, file: &Path) -> Result<(), String> {
    let card = maildir::read_one(file).ok_or_else(|| format!("{}: ?", file.display()))?;
    let store = load_store(&s.config);
    let known = KnownSenders::load(&s.config.known_senders_path());
    let senders = porch::Senders::load(&s.config);
    let ids = s.config.all_trusted_ids();
    let own = sioul_core::lookalike::own_domains(s.config.accounts.iter().filter_map(|a| a.address.as_deref()));
    let auth = trust::read_auth_results(&card.headers, &ids);
    let spam = trust::read_spam_verdict(&card.headers);
    let route: Vec<String> = trust::route_ips(&card.headers).iter().map(ToString::to_string).collect();
    let own_addresses = porch::own_addresses(&s.config.mail_sources());
    let ctx = Context { cases: store.as_ref(), known: &known, senders: &senders, trusted_ids: &ids, now: Some(Timestamp::now().as_second()), priority: Default::default(), own_domains: &own, shielded: false, assessments: None, filed_words: &[], own_addresses: &own_addresses };
    let t = porch::triage(card, &ctx);
    println!("From      {} <{}>", one_line(t.card.sender()), one_line(t.card.from_address.as_deref().unwrap_or("?")));
    println!("Subject   {}", one_line(&t.card.subject));
    println!("Date      {}", date(s, t.card.date));
    println!("Trust     {}", s.tr.trust(t.trust));
    println!("Lane      {:?}", t.lane);
    for reason in &t.reasons {
        println!("          · {}", one_line(&s.tr.reason(reason, store.as_ref())));
    }
    println!("Provider  {auth:?}");
    println!("Spam      {spam:?}");
    println!("Route     {}", route.join(" ← "));
    if let Some(code) = &t.code {
        println!("Code      {code:?}");
    }
    if let Some(reading) = sioul_core::reading::read(file) {
        for a in &reading.attachments {
            println!("Attached  {} ({}, {} bytes)", one_line(&a.name), one_line(&a.mime), a.size);
        }
        for part in &reading.parts {
            let line = format!("{part:?}").replace('\n', " ⏎ ");
            println!("Part      {}", line.chars().take(110).collect::<String>());
        }
    }
    Ok(())
}

fn cases_command(s: &Session, store: Option<PathBuf>) -> Result<(), String> {
    let root = store.or_else(|| s.config.case_store_path()).ok_or_else(|| s.tr.text("error-no-store", None))?;
    let store = CaseStore::load(&root)?;
    let missing = store.missing_files();
    for case in &store.cases {
        let status = case.status.as_deref().map_or(String::new(), |st| format!(" [{}]", one_line(st)));
        println!("{} · {}{status}", one_line(&case.id), one_line(&case.title));
        for file in &case.files {
            let gone = missing.iter().any(|(id, f)| id == &case.id && f == file);
            println!("    {}{}", one_line(file), if gone { "  (?)" } else { "" });
        }
    }
    Ok(())
}

fn window_command(s: &Session) -> Result<(), String> {
    if s.config.windows.is_empty() {
        return Err(s.tr.text("error-no-windows", None));
    }
    let now = Zoned::now();
    let mut args = sioul_core::i18n::args();
    let id = match window::current(&s.config.windows, &now) {
        Some((_, closing)) => {
            args.set("time", closing.strftime("%H:%M").to_string());
            "window-open-until"
        }
        None => {
            let next = window::next_opening(&s.config.windows, &now).ok_or_else(|| s.tr.text("window-none", None))?;
            args.set("when", s.tr.when(&next));
            "window-next"
        }
    };
    println!("{}", s.tr.text(id, Some(&args)));
    Ok(())
}

fn budgets_command(s: &Session, file: Option<PathBuf>, maildirs: &[PathBuf]) -> Result<(), String> {
    let path = file
        .or_else(|| s.config.case_store_path().map(|root| root.join(budget::LEDGER)))
        .ok_or_else(|| s.tr.text("error-no-ledger", None))?;
    let ledger = Ledger::load_file(&path)?;
    // With the bank accounts' movements, read from the exports beside the file.
    let ledger = match path.parent().map(sioul_core::bank::Bank::load) {
        Some(Ok(bank)) => ledger.with_bank(&bank),
        _ => ledger,
    };
    let today = Zoned::now().date();
    let mut args = sioul_core::i18n::args();
    args.set("month", s.tr.month_year(today));
    println!("{}", s.tr.text("budgets-title", Some(&args)));
    for b in &ledger.budgets {
        let status = ledger.status(b, today);
        let reserve = status.reserve_transfer.as_ref().map(|(id, _)| ledger.reserve_title(id));
        print_indented(&s.tr.budget_lines(&b.title, b.period, &status, reserve));
    }
    if !ledger.reserves.is_empty() {
        println!("\n{}", s.tr.text("reserves-title", None));
    }
    for r in &ledger.reserves {
        print_indented(&s.tr.reserve_lines(&r.title, &ledger.reserve_status(r, today), today));
    }
    print_mail_lines(s, &ledger, maildirs);
    Ok(())
}

/// The first line of a block at two spaces, its follow-ups at four.
fn print_indented(lines: &[String]) {
    for (i, line) in lines.iter().enumerate() {
        println!("{}{}", if i == 0 { "  " } else { "    " }, one_line(line));
    }
}

/// Mail about money, from every account (or the folders given), and what becomes of it.
fn print_mail_lines(s: &Session, ledger: &Ledger, maildirs: &[PathBuf]) {
    let store = load_store(&s.config);
    let known = KnownSenders::load(&s.config.known_senders_path());
    let senders = porch::Senders::load(&s.config);
    let now = Zoned::now();
    // Every message, the ones the Porch was closed on included: payments are not news.
    let items = porch::gather(&sources(&s.config, maildirs), store.as_ref(), &known, &senders, &PorchState::default(), now.timestamp().as_second());
    let ignored = MoneyState::load(&MoneyState::default_path()).ignored;
    let lines = budget::mail_lines(ledger, &items, &ignored);
    let shown = view::budgets(ledger, &lines, &s.tr, now.date());
    if shown.mail.is_empty() {
        return;
    }
    println!("\n{}", shown.mail_title);
    for row in &shown.mail {
        let budget = row.budget.as_deref().map_or("?", |b| ledger.budget_title(b));
        let amount = if row.amount.is_empty() { "—" } else { row.amount.as_str() };
        println!("  {} · {} · {} · {amount} · {}", row.date, row.kind_label, one_line(&row.party), one_line(budget));
        let doubtful = row.doubtful && row.state == "proposed";
        for note in [row.note.clone(), if doubtful { s.tr.text("note-doubtful", None) } else { String::new() }] {
            if !note.is_empty() {
                println!("      {}", one_line(&note));
            }
        }
    }
}

fn block_command(s: &Session, entry: Option<&str>) -> Result<(), String> {
    let path = s.config.blocked_senders_path();
    let Some(entry) = entry else {
        let entries = SenderList::load(&path).entries();
        if entries.is_empty() {
            println!("{}", s.tr.text("blocked-none", None));
        }
        for e in entries {
            println!("{}", one_line(&e));
        }
        return Ok(());
    };
    porch::set_standing(&s.config, entry, porch::Standing::Blocked)?;
    println!("{}", s.say("ui-blocked", &[("entry", entry.trim().to_ascii_lowercase())]));
    Ok(())
}

fn verify_command(s: &Session, again: bool) -> Result<(), String> {
    for source in s.config.mail_sources() {
        let checked = sioul_sync::verify::verify_folder(&source.folder, again);
        let mut args = s.tr.counted(checked);
        args.set("account", source.account.unwrap_or_default());
        println!("{}", s.tr.text("verify-done", Some(&args)));
    }
    Ok(())
}

fn unblock_command(s: &Session, entry: &str) -> Result<(), String> {
    porch::set_standing(&s.config, entry, porch::Standing::Neutral)?;
    println!("{}", s.say("ui-unblocked", &[("entry", entry.trim().to_ascii_lowercase())]));
    Ok(())
}

pub(crate) fn date(s: &Session, seconds: Option<i64>) -> String {
    seconds
        .and_then(|sec| Timestamp::from_second(sec).ok())
        .map_or_else(|| s.tr.text("no-date", None), |t| s.tr.date(&t.to_zoned(TimeZone::system()), true))
}

#[cfg(test)]
mod terminal_tests {
    use super::*;

    #[test]
    fn what_is_printed_cannot_drive_the_terminal() {
        // A subject with a title change, a clipboard write (OSC 52) and a right-to-left mark.
        let subject = "Invoice\u{1b}]0;pwned\u{7}\u{1b}]52;c;Y2F0\u{7} \u{202e}fdp.exe\r\nBcc: x";
        let line = one_line(subject);
        assert!(!line.contains(['\u{1b}', '\u{7}', '\u{202e}', '\r', '\n']), "{line:?}");
        let lines = plain_lines("First line\r\nsecond\u{1b}[2J\rthird\ttab");
        assert_eq!(lines, "First line\nsecond[2J\nthird\ttab");
    }

    #[test]
    fn numbers_choose_the_plural() {
        let s = Session { config: Config::default(), config_path: PathBuf::new(), tr: Translator::new("en") };
        let one = s.say("shield-read", &[("n", "1".to_string()), ("left", "0".to_string())]);
        assert!(one.contains("1 message;"), "{one}");
        let many = s.say("shield-read", &[("n", "2".to_string()), ("left", "0".to_string())]);
        assert!(many.contains("2 messages;"), "{many}");
    }
}
