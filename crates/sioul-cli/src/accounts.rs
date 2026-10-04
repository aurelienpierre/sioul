// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

//! Mail accounts and sync, from the terminal: `sioul account …`, `sioul sync`, `sioul watch`.
//!
//! Passwords are typed at a prompt that does not echo them, and go to the
//! system keyring; they never reach the configuration file or the screen.

use crate::{Session, load_store, print_right_now};
use clap::{Subcommand, ValueEnum};
use jiff::Zoned;
use sioul_core::config::{self, Account, AccountKind, Config, DEFAULT_SYNC_DAYS, Priority, Security};
use sioul_core::porch::{self, KnownSenders, Triaged};
use sioul_core::view;
use sioul_sync::{Control, Found, Learned, Report, SyncError, notify, secret};
use std::path::PathBuf;
use std::sync::{Arc, mpsc};

#[derive(Subcommand)]
pub(crate) enum AccountCommand {
    /// Finds the server from the address, asks for the password, tests it, adds the account and fetches its mail.
    Add {
        address: String,
        /// A short name for the account (default: from the domain).
        #[arg(long)]
        id: Option<String>,
        /// The IMAP server, when it cannot be found or you want another.
        #[arg(long)]
        host: Option<String>,
        #[arg(long)]
        port: Option<u16>,
        #[arg(long, value_enum)]
        security: Option<SecurityArg>,
        /// The login, when it is not the address.
        #[arg(long)]
        username: Option<String>,
        /// Days of mail the first sync brings (default 14).
        #[arg(long)]
        days: Option<u32>,
    },
    /// A web-only mailbox (Proton without Bridge, a bank): it opens in your browser.
    Portal { id: String, url: String },
    /// Every account, its server, and whether its password is in the keyring.
    List,
    /// Logs in and opens the inbox, read-only.
    Test { id: String },
    /// Replaces the password kept in the keyring (after changing it, or a new app password).
    Password { id: String },
    /// Removes the account and its password. Its mail stays on disk.
    Remove { id: String },
    /// Ranks an account's mail against the others': above, average (the default) or below.
    /// Below, its mail skips the screener and waits folded at the bottom of the Porch.
    Priority {
        id: String,
        #[arg(value_enum)]
        priority: PriorityArg,
    },
}

#[derive(Clone, Copy, ValueEnum)]
pub(crate) enum PriorityArg {
    Above,
    Average,
    Below,
}

#[derive(Clone, Copy, ValueEnum)]
pub(crate) enum SecurityArg {
    Tls,
    Starttls,
}

impl From<SecurityArg> for Security {
    fn from(value: SecurityArg) -> Security {
        match value {
            SecurityArg::Tls => Security::Tls,
            SecurityArg::Starttls => Security::Starttls,
        }
    }
}

pub(crate) fn run(s: &Session, command: AccountCommand) -> Result<(), String> {
    match command {
        AccountCommand::Add { address, id, host, port, security, username, days } => {
            let wanted = Wanted { host, port, security: security.map(Security::from), username, days };
            add(s, &address, id, wanted)
        }
        AccountCommand::Portal { id, url } => portal(s, &id, &url),
        AccountCommand::List => list(s),
        AccountCommand::Test { id } => test(s, &id),
        AccountCommand::Password { id } => password(s, &id),
        AccountCommand::Remove { id } => remove(s, &id),
        AccountCommand::Priority { id, priority } => {
            let priority = match priority {
                PriorityArg::Above => Priority::Above,
                PriorityArg::Average => Priority::Average,
                PriorityArg::Below => Priority::Below,
            };
            set_priority(s, &id, priority)
        }
    }
}

/// What was asked on the command line, over what discovery finds.
struct Wanted {
    host: Option<String>,
    port: Option<u16>,
    security: Option<Security>,
    username: Option<String>,
    days: Option<u32>,
}

fn add(s: &Session, address: &str, id: Option<String>, wanted: Wanted) -> Result<(), String> {
    let address = address.trim().to_ascii_lowercase();
    // As in the window: the address's mail only, switched off or not; its calendars may be here already.
    let mail = |a: &&Account| a.address.as_deref() == Some(address.as_str()) && matches!(a.kind, AccountKind::Imap | AccountKind::Jmap);
    if let Some(existing) = s.config.every_account().find(mail) {
        return Err(s.say("account-address-exists", &[("address", address.clone()), ("id", existing.id.clone())]));
    }
    let id = id.unwrap_or_else(|| s.config.free_id(&address));
    if s.config.every_account().any(|a| a.id == id) {
        return Err(s.say("account-exists", &[("id", id)]));
    }
    let found = match &wanted.host {
        // Given by hand: port 0 means "the usual one for the encryption", chosen below.
        Some(host) => Found { host: host.clone(), port: 0, security: Security::Tls, username: address.clone(), by: sioul_sync::FoundBy::Provider, smtp: None },
        None => {
            let found = sioul_sync::discover(&address).map_err(|e| e.sentence(&s.tr, &id))?;
            println!("{}", s.say("account-found", &[("server", server_line(s, &found.host, found.port, found.security)), ("by", s.tr.text(found.by.message_id(), None))]));
            found
        }
    };
    let security = wanted.security.unwrap_or(found.security);
    let port = wanted.port.or((found.port != 0).then_some(found.port)).unwrap_or(if security == Security::Tls { 993 } else { 143 });
    let login = wanted.username.clone().unwrap_or(found.username);
    let mut account = Account::imap(&id, &address, &found.host, port, security, Some(&login));
    account.sync_days = wanted.days;
    if wanted.host.is_some() {
        println!("{}", server_line(s, &found.host, port, security));
    }
    if account.username.is_some() {
        println!("{}", s.say("account-login", &[("login", crate::one_line(&login))]));
    }
    let hint = if sioul_sync::is_gmail(&address) { "account-gmail-hint" } else { "account-app-password-hint" };
    println!("{}", s.tr.text(hint, None));
    let password = sioul_sync::tidy_password(&found.host, &prompt_password(s, &address)?);
    let messages = sioul_sync::test(&account, &password).map_err(|e| e.sentence(&s.tr, &id))?;
    let mut connected = s.tr.counted(messages as usize);
    connected.set("account", id.clone());
    println!("{}", s.tr.text("account-connected", Some(&connected)));
    secret::save(&account, &password).map_err(|e| e.sentence(&s.tr, &id))?;
    config::add_imap_account(&s.config_path, &account)?;
    println!("{}", s.say("account-added", &[("id", id.clone()), ("path", account.maildir_path().display().to_string())]));
    println!("{}", s.tr.text("ui-syncing", None));
    let report = sioul_sync::sync(&account, &password).map_err(|e| e.sentence(&s.tr, &id))?;
    say_report(s, &account, &report);
    learn_provider(s, &account, true);
    Ok(())
}

fn server_line(s: &Session, host: &str, port: u16, security: Security) -> String {
    s.say(
        "account-server",
        &[("host", crate::one_line(host)), ("port", port.to_string()), ("security", s.tr.text(&format!("security-{}", security.as_str()), None))],
    )
}

pub(crate) fn prompt_password(s: &Session, address: &str) -> Result<String, String> {
    // Tests against a local server type nothing (never in Sioul's builds).
    #[cfg(feature = "insecure-test-tls")]
    if let Ok(password) = std::env::var("SIOUL_TEST_PASSWORD") {
        return Ok(password);
    }
    let prompt = s.say("account-password-prompt", &[("address", address.to_string())]);
    let password = rpassword::prompt_password(format!("{prompt} ")).map_err(|e| e.to_string())?;
    Ok(password.trim_end_matches(['\r', '\n']).to_string())
}

fn portal(s: &Session, id: &str, url: &str) -> Result<(), String> {
    // A portal is a site now: its id among the sites, its address encrypted (docs/sites.md).
    if s.config.site(id).is_some() || s.config.account(id).is_some() {
        return Err(s.say("account-exists", &[("id", id.to_string())]));
    }
    if !url.starts_with("https://") {
        return Err(s.tr.text("site-https-only", None));
    }
    config::add_portal(&s.config_path, id, url)?;
    println!("{}", s.say("account-portal-added", &[("id", id.to_string()), ("url", url.to_string())]));
    Ok(())
}

fn list(s: &Session) -> Result<(), String> {
    if s.config.accounts.is_empty() {
        println!("{}", s.tr.text("sync-nothing", None));
        return Ok(());
    }
    for (account, shown) in s.config.accounts.iter().zip(view::accounts(&s.config, &s.tr, &Default::default(), &Default::default())) {
        let mut parts = vec![account.id.clone()];
        parts.extend(account.address.clone());
        if account.kind != AccountKind::Portal {
            parts.push(match secret::password(account) {
                Ok(_) => s.tr.text("account-has-password", None),
                Err(SyncError::NoPassword) => s.say("account-no-password", &[("id", account.id.clone())]),
                Err(e) => e.sentence(&s.tr, &account.id),
            });
        }
        if account.kind == AccountKind::Imap {
            parts.push(s.tr.text(&format!("priority-{}", shown.priority), None));
        }
        println!("{}", crate::one_line(&parts.join(" · ")));
        // "Server: …" in English, "Serveur : …" in French; a row without a label goes on with the one above.
        let colon = if s.tr.language() == "fr" { "\u{202f}:" } else { ":" };
        let mut indent = 4;
        for row in shown.rows {
            if row.label.is_empty() {
                println!("{:indent$}{}", "", crate::one_line(&row.value));
            } else {
                println!("    {}{colon} {}", row.label, crate::one_line(&row.value));
                indent = 4 + row.label.chars().count() + colon.chars().count() + 1;
            }
        }
    }
    Ok(())
}

fn mail_account<'a>(s: &'a Session, id: &str) -> Result<&'a Account, String> {
    s.config.account(id).filter(|a| a.syncs()).ok_or_else(|| s.say("account-unknown", &[("id", id.to_string())]))
}

fn test(s: &Session, id: &str) -> Result<(), String> {
    let account = mail_account(s, id)?;
    let messages = secret::password(account).and_then(|p| sioul_sync::test(account, &p)).map_err(|e| e.sentence(&s.tr, id))?;
    println!("{}", s.tr.text("account-connected", Some(&s.tr.counted(messages as usize))));
    Ok(())
}

fn password(s: &Session, id: &str) -> Result<(), String> {
    let account = mail_account(s, id)?;
    let typed = prompt_password(s, account.address.as_deref().unwrap_or(id))?;
    let password = sioul_sync::tidy_password(account.host.as_deref().unwrap_or(""), &typed);
    let messages = sioul_sync::test(account, &password).map_err(|e| e.sentence(&s.tr, id))?;
    println!("{}", s.tr.text("account-connected", Some(&s.tr.counted(messages as usize))));
    secret::save(account, &password).map_err(|e| e.sentence(&s.tr, id))?;
    println!("{}", s.tr.text("account-password-saved", None));
    Ok(())
}

fn set_priority(s: &Session, id: &str, priority: Priority) -> Result<(), String> {
    if s.config.account(id).is_none() {
        return Err(s.say("account-unknown", &[("id", id.to_string())]));
    }
    config::set_priority(&s.config_path, id, priority)?;
    let label = s.tr.text(&format!("priority-{}", priority.as_str()), None);
    println!("{}", s.say("ui-priority-set", &[("id", id.to_string()), ("priority", label)]));
    Ok(())
}

fn remove(s: &Session, id: &str) -> Result<(), String> {
    // Switched off, an account can be removed all the same.
    let account = s.config.every_account().find(|a| a.id == id).ok_or_else(|| s.say("account-unknown", &[("id", id.to_string())]))?;
    // The keyring keeps one password per login and server: another account on the same keeps it.
    let shared_password = s.config.every_account().any(|a| a.id != id && a.host == account.host && a.login() == account.login());
    if account.auth.as_deref() == Some("google") {
        // Google's access goes back to it, as the window does.
        sioul_sync::google::revoke(account.address.as_deref().unwrap_or("")).map_err(|e| e.sentence(&s.tr, id))?;
    } else if (account.syncs() || account.is_dav()) && !shared_password {
        secret::forget(account).map_err(|e| e.sentence(&s.tr, id))?;
    }
    config::remove_account(&s.config_path, id)?;
    println!("{}", s.say("account-removed", &[("id", id.to_string()), ("path", account.maildir_path().display().to_string())]));
    Ok(())
}

/// "example: two new messages.", or the first sync's "example: 212 messages from the last 14 days."
fn say_report(s: &Session, account: &Account, report: &Report) {
    let mut args = s.tr.counted(report.new.len());
    args.set("account", account.id.clone());
    args.set("days", account.sync_days.unwrap_or(DEFAULT_SYNC_DAYS));
    println!("{}", s.tr.text(if report.first { "sync-first" } else { "sync-new" }, Some(&args)));
}

/// Learns which authserv-id the provider writes, while the account has none.
/// `say_failure` tells when nothing could be learned (once, at the first sync).
fn learn_provider(s: &Session, account: &Account, say_failure: bool) {
    match sioul_sync::learn_provider(&s.config_path, account) {
        Ok(Learned::Id(id)) => println!("{}", s.say("sync-learned", &[("account", account.id.clone()), ("id", crate::one_line(&id))])),
        Ok(Learned::Nothing) if say_failure => println!("{}", s.say("sync-not-learned", &[("account", account.id.clone())])),
        Ok(_) => {}
        Err(e) => eprintln!("{}", crate::plain_lines(&e.to_string())),
    }
}

/// Every account Sioul fetches, or the one asked for.
fn syncing<'a>(config: &'a Config, only: Option<&str>) -> Vec<&'a Account> {
    config.accounts.iter().filter(|a| a.syncs() && only.is_none_or(|id| a.id == id)).collect()
}

pub(crate) fn sync_command(s: &Session, only: Option<&str>) -> Result<(), String> {
    let accounts = syncing(&s.config, only);
    if accounts.is_empty() {
        return Err(match only {
            Some(id) => s.say("account-unknown", &[("id", id.to_string())]),
            None => s.tr.text("sync-nothing", None),
        });
    }
    let mut arrived = Vec::new();
    for account in accounts {
        match secret::password(account).and_then(|p| sioul_sync::sync(account, &p)) {
            Ok(report) => {
                say_report(s, account, &report);
                learn_provider(s, account, report.first);
                if !report.first {
                    arrived.extend(report.new);
                }
            }
            Err(e) => eprintln!("{}", crate::plain_lines(&e.sentence(&s.tr, &account.id))),
        }
    }
    // The configuration may have learned provider ids: read it again before judging.
    let config = Config::load(&s.config_path).unwrap_or_default();
    println!();
    print_right_now(s, &triage_files(&config, &arrived));
    Ok(())
}

/// Judges freshly fetched files as the Porch would.
fn triage_files(config: &Config, files: &[PathBuf]) -> Vec<Triaged> {
    let store = load_store(config);
    let known = KnownSenders::load(&config.known_senders_path());
    let senders = porch::Senders::load(config);
    porch::judge(files, &config.mail_sources(), store.as_ref(), &known, &senders, Zoned::now().timestamp().as_second())
}

pub(crate) fn watch_command(s: &Session) -> Result<(), String> {
    let accounts: Vec<Account> = syncing(&s.config, None).into_iter().cloned().collect();
    if accounts.is_empty() {
        return Err(s.tr.text("sync-nothing", None));
    }
    let names = accounts.iter().map(|a| a.id.clone()).collect::<Vec<_>>().join(", ");
    println!("{}", s.say("watch-started", &[("accounts", names)]));
    // Never stopped here: Ctrl+C ends the process, and the server keeps nothing open.
    let control = Arc::new(Control::default());
    let (sender, receiver) = mpsc::channel::<(String, Result<Report, SyncError>)>();
    for account in accounts {
        let password = match secret::password(&account) {
            Ok(password) => password,
            Err(e) => {
                eprintln!("{}", crate::plain_lines(&e.sentence(&s.tr, &account.id)));
                continue;
            }
        };
        let (sender, control) = (sender.clone(), Arc::clone(&control));
        std::thread::spawn(move || {
            let id = account.id.clone();
            sioul_sync::watch(&account, &password, &control, |result| {
                let _ = sender.send((id.clone(), result));
            });
        });
    }
    drop(sender);
    for (id, result) in receiver {
        let time = Zoned::now().strftime("%H:%M").to_string();
        match result {
            Ok(report) if report.new.is_empty() => {}
            Ok(report) => {
                let config = Config::load(&s.config_path).unwrap_or_default();
                if let Some(account) = config.account(&id) {
                    learn_provider(s, account, report.first);
                }
                let mut args = s.tr.counted(report.new.len());
                args.set("account", id.clone());
                let line = s.tr.text("sync-new", Some(&args));
                println!("{}", s.say("watch-line", &[("time", time), ("line", line)]));
                if report.first {
                    continue;
                }
                let config = Config::load(&s.config_path).unwrap_or_default();
                let arrived = triage_files(&config, &report.new);
                print_right_now(s, &arrived);
                for code in view::codes(&arrived, &s.tr) {
                    if let Err(e) = notify::code(&code.title, &code.body(), None) {
                        eprintln!("{}", crate::plain_lines(&e));
                    }
                }
            }
            Err(e) => eprintln!("{}", crate::plain_lines(&s.say("watch-line", &[("time", time), ("line", e.sentence(&s.tr, &id))]))),
        }
    }
    Ok(())
}
