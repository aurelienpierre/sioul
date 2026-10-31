// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

//! Mail accounts and sync, from the terminal: `sioul account …`, `sioul sync`, `sioul watch`.
//!
//! Passwords are typed at a prompt that does not echo them, and go to the
//! system keyring; they never reach the configuration file or the screen.
//! Microsoft's mail takes no password: its address is signed in on
//! Microsoft's page, whose link is printed here, and the access goes to the
//! keyring (docs/microsoft.md).

use crate::{Session, load_store, print_right_now};
use clap::{Subcommand, ValueEnum};
use sioul_core::config::{self, Account, AccountKind, Config, DEFAULT_SYNC_DAYS, Priority, Security};
use sioul_core::porch::{self, KnownSenders, Triaged};
use sioul_core::view;
use sioul_sync::discover::MicrosoftKind;
use sioul_sync::{Control, Found, FoundBy, Learned, Report, SyncError, microsoft, notify, secret};
use std::path::PathBuf;
use std::sync::atomic::AtomicBool;
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
        /// `microsoft`: Microsoft's sign-in, whatever discovery finds (a
        /// Microsoft 365 address whose mail first goes through another service).
        #[arg(long, value_enum)]
        auth: Option<AuthArg>,
        /// Microsoft: the Application (client) ID of an application of your own; no secret.
        #[arg(long)]
        client_id: Option<String>,
        /// Microsoft: its Directory (tenant) ID, when it is registered for one organisation alone.
        #[arg(long)]
        tenant: Option<String>,
    },
    /// Signs a Microsoft account in again on Microsoft's page (one added with a password before: switched to it).
    SignIn {
        id: String,
        /// The Application (client) ID of an application of your own; no secret.
        #[arg(long)]
        client_id: Option<String>,
        /// Its Directory (tenant) ID, when it is registered for one organisation alone.
        #[arg(long)]
        tenant: Option<String>,
    },
    /// A web-only mailbox (Proton without Bridge, a bank): it opens in your browser.
    Portal { id: String, url: String },
    /// Every account, its server, and whether its password is in the keyring.
    List,
    /// Logs in and opens the inbox, read-only.
    Test { id: String },
    /// Replaces the password kept in the keyring (after changing it, or a new app password).
    Password { id: String },
    /// Removes the account and its password. Its mail stays on disk; a
    /// Microsoft account's is deleted unless you say otherwise (asked).
    Remove {
        id: String,
        /// A Microsoft account: keep the mail Sioul keeps on this device, without asking.
        #[arg(long, conflicts_with = "delete_mail")]
        keep_mail: bool,
        /// A Microsoft account: delete the mail Sioul keeps on this device, without asking.
        #[arg(long)]
        delete_mail: bool,
    },
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

/// How an account signs in, when not with a password.
#[derive(Clone, Copy, PartialEq, Eq, ValueEnum)]
pub(crate) enum AuthArg {
    Microsoft,
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
        AccountCommand::Add { address, id, host, port, security, username, days, auth, client_id, tenant } => {
            let wanted = Wanted { host, port, security: security.map(Security::from), username, days, microsoft: auth == Some(AuthArg::Microsoft), client_id, tenant };
            add(s, &address, id, wanted)
        }
        AccountCommand::SignIn { id, client_id, tenant } => sign_in_again(s, &id, client_id.as_deref().unwrap_or(""), tenant.as_deref().unwrap_or("")),
        AccountCommand::Portal { id, url } => portal(s, &id, &url),
        AccountCommand::List => list(s),
        AccountCommand::Test { id } => test(s, &id),
        AccountCommand::Password { id } => password(s, &id),
        AccountCommand::Remove { id, keep_mail, delete_mail } => remove(s, &id, (keep_mail || delete_mail).then_some(delete_mail)),
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
    /// Microsoft's sign-in, whatever discovery finds (`--auth microsoft`).
    microsoft: bool,
    /// Microsoft: an application of your own, and its directory.
    client_id: Option<String>,
    tenant: Option<String>,
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
    // Microsoft's mail: its own sign-in, never a password (none works there).
    let microsoft = match found.by {
        FoundBy::Microsoft(kind) => Some(kind),
        _ if wanted.microsoft || microsoft::is_imap_host(&found.host) => Some(MicrosoftKind::Unknown),
        _ => None,
    };
    if let Some(kind) = microsoft {
        let mut account = Account::imap(&id, &address, microsoft::IMAP_HOST, 993, Security::Tls, None);
        account.sync_days = wanted.days;
        let account = sign_in_with_microsoft(s, account, kind, wanted.client_id.as_deref().unwrap_or(""), wanted.tenant.as_deref().unwrap_or(""))?;
        config::add_imap_account(&s.config_path, &account)?;
        config::set_sign_in(&s.config_path, &account)?;
        println!("{}", s.say("account-added", &[("id", id.clone()), ("path", account.maildir_path().display().to_string())]));
        println!("{}", s.tr.text("ui-syncing", None));
        let report = sioul_sync::sync(&account, "").map_err(|e| e.sentence(&s.tr, &id))?;
        say_report(s, &account, &report);
        learn_provider(s, &account, true);
        return Ok(());
    }
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
    // Google's mail (Gmail, Google Workspace): an app password, never the account's own.
    let google = found.by == sioul_sync::FoundBy::Google || found.host.eq_ignore_ascii_case(sioul_sync::discover::GOOGLE_IMAP);
    let hint = if google { "account-gmail-hint" } else { "account-app-password-hint" };
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

/// Microsoft's page, from the terminal: its link printed, to open in a
/// browser on this computer (its answer comes back here, to a loopback
/// port); Sioul waits five minutes, Ctrl+C stops. The mailbox is tried with
/// the access given before anything is kept (`microsoft::sign_in`); when
/// only the mailbox of the login Microsoft's page signed in opened, not the
/// address's, Sioul asks first, No by default (and No without a terminal).
/// A login another of your accounts signs in as is refused. A work account
/// refused for want of its organisation's approval gets what its
/// administrator does, to forward.
fn sign_in_with_microsoft(s: &Session, account: Account, kind: MicrosoftKind, client_id: &str, tenant: &str) -> Result<Account, String> {
    let address = account.address.clone().unwrap_or_default();
    let label = address.clone();
    let login = account.login().unwrap_or(&address).to_string();
    let client = microsoft::app_for(&login, client_id).map_err(|e| e.sentence(&s.tr, &label))?.ok_or_else(|| s.tr.text("microsoft-cli-no-app", None))?;
    let authority = microsoft::authority_for(&login, kind, tenant).map_err(|e| e.sentence(&s.tr, &label))?;
    println!("{}", s.tr.text("microsoft-cli-open", None));
    let stop = AtomicBool::new(false);
    // The logins your other Microsoft accounts sign in as: never shared.
    let taken: Vec<String> = s.config.every_account().filter(|a| a.is_microsoft() && a.id != account.id).filter_map(|a| a.login().map(|l| l.trim().to_lowercase())).collect();
    let signed = microsoft::sign_in(account, &client, &authority, &taken, |url| println!("\n{url}\n"), &stop).and_then(|signed| {
        // Only the mailbox of the login Microsoft's page signed in opened: kept on your Yes alone.
        let yes = match signed.other_login.as_deref() {
            // Microsoft's words on one line: they never drive the terminal.
            Some(other) => ask_no(s, &s.say("microsoft-other-login-question", &[("address", crate::one_line(&address)), ("login", crate::one_line(other))])).unwrap_or(false),
            None => true,
        };
        signed.answer(yes)
    });
    match signed {
        Ok((account, grant)) => {
            let kind = if grant.personal { "account-signed-in-microsoft-personal" } else { "account-signed-in-microsoft-work" };
            println!("{}", s.say("microsoft-cli-signed-in", &[("account", label), ("how", s.tr.text(kind, None))]));
            Ok(account)
        }
        Err(e) => {
            let said = e.sentence(&s.tr, &label);
            let organisation = matches!(&e, SyncError::Login(d) if d == microsoft::ADMIN) || matches!(&e, SyncError::Said(id, _) if *id == microsoft::DECLINED_WORK);
            if organisation {
                let domain = address.rsplit('@').next().unwrap_or("");
                return Err(format!("{said}\n\n{}", s.say("microsoft-admin-steps", &[("link", microsoft::admin_consent_url(&client, domain))])));
            }
            Err(said)
        }
    }
}

/// `sioul account sign-in <id>`: a Microsoft account signed in again (its
/// access ended, or another application of yours), or one added with a
/// password at Microsoft's server switched to Microsoft's sign-in.
fn sign_in_again(s: &Session, id: &str, client_id: &str, tenant: &str) -> Result<(), String> {
    let account = s.config.every_account().find(|a| a.id == id && a.kind == AccountKind::Imap).ok_or_else(|| s.say("account-unknown", &[("id", id.to_string())]))?.clone();
    let at_microsoft = account.host.as_deref().is_some_and(microsoft::is_imap_host);
    if !account.is_microsoft() && !at_microsoft {
        return Err(s.say("microsoft-cli-not-microsoft", &[("id", id.to_string())]));
    }
    let kind = if account.is_microsoft() && !account.is_microsoft_work() { MicrosoftKind::Personal } else if account.is_microsoft() { MicrosoftKind::Work } else { MicrosoftKind::Unknown };
    let was_password = !account.is_microsoft();
    let old = account.clone();
    let account = sign_in_with_microsoft(s, account, kind, client_id, tenant)?;
    config::set_sign_in(&s.config_path, &account)?;
    // A password kept before, which Microsoft no longer took: gone from the keyring.
    if was_password {
        let _ = secret::forget(&old);
    }
    println!("{}", s.say("microsoft-cli-again", &[("account", id.to_string())]));
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
            parts.push(secret_said(s, account, secret::password(account)));
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

/// What `accounts list` says of an account's secret, from what the keyring
/// gave (`secret::password`): its password there, Google's sign-in (whose
/// access the keyring keeps, no password), none yet, or what went wrong.
fn secret_said(s: &Session, account: &Account, found: Result<String, SyncError>) -> String {
    match found {
        Ok(_) if account.is_microsoft() => s.tr.text("account-signed-in-microsoft", None),
        Ok(_) if sioul_sync::sasl::OAuth::of(account).is_some() => s.tr.text("account-signed-in-google", None),
        Ok(_) => s.tr.text("account-has-password", None),
        Err(SyncError::NoPassword) => s.say("account-no-password", &[("id", account.id.clone())]),
        Err(e) => e.sentence(&s.tr, &account.id),
    }
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
    // Microsoft takes no password, app passwords included: its own sign-in instead.
    if account.is_microsoft() || account.host.as_deref().is_some_and(microsoft::is_imap_host) {
        return Err(s.say("microsoft-cli-no-password", &[("id", id.to_string())]));
    }
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

/// `delete_mail`: a Microsoft account's mail kept on this device deleted
/// (Some(true)) or kept (Some(false)); None: asked, Yes by default (as
/// Microsoft's terms ask an app to delete what it keeps of an account you
/// leave). Other accounts' mail stays, as before.
fn remove(s: &Session, id: &str, delete_mail: Option<bool>) -> Result<(), String> {
    // Switched off, an account can be removed all the same.
    let account = s.config.every_account().find(|a| a.id == id).ok_or_else(|| s.say("account-unknown", &[("id", id.to_string())]))?;
    // The keyring keeps one password per login and server: another account on the same keeps it.
    let shared_password = s.config.every_account().any(|a| a.id != id && a.host == account.host && a.login() == account.login());
    if account.is_microsoft() {
        // Another process's watcher of this account (the window, `sioul watch`) is not stopped from here: said before anything is deleted.
        if delete_mail != Some(false) {
            println!("{}", s.tr.text("microsoft-cli-remove-close", None));
        }
        let delete = delete_mail.map_or_else(|| ask_yes(s, &s.tr.text("ui-remove-microsoft-mail", None)), Ok)?;
        // Nothing to give back to Microsoft: forgotten here, the consent withdrawn on Microsoft's page.
        let page = microsoft::forget(account.login().unwrap_or(id), !account.is_microsoft_work()).map_err(|e| e.sentence(&s.tr, id))?;
        config::remove_account(&s.config_path, id)?;
        if delete {
            sioul_core::maildir::forget_account(account)?;
            sioul_learn::Dirs::standard().forget_account(&account.id).map_err(|e| e.to_string())?;
            println!("{}", s.say("account-removed-microsoft-mail", &[("id", id.to_string()), ("page", page.to_string())]));
        } else {
            println!("{}", s.say("account-removed-microsoft", &[("id", id.to_string()), ("path", account.maildir_path().display().to_string()), ("page", page.to_string())]));
        }
        return Ok(());
    }
    if account.auth.as_deref() == Some("google") {
        // Google's access goes back to it, as the window does: the calendars' or the mail's, each its own.
        let purpose = if account.is_dav() { sioul_sync::google::Purpose::Pim } else { sioul_sync::google::Purpose::Mail };
        sioul_sync::google::revoke_for(purpose, account.address.as_deref().unwrap_or("")).map_err(|e| e.sentence(&s.tr, id))?;
    } else if (account.syncs() || account.is_dav()) && !shared_password {
        secret::forget(account).map_err(|e| e.sentence(&s.tr, id))?;
    }
    config::remove_account(&s.config_path, id)?;
    println!("{}", s.say("account-removed", &[("id", id.to_string()), ("path", account.maildir_path().display().to_string())]));
    Ok(())
}

/// A question answered Yes or No at the terminal, No unless the answer
/// begins with Yes's letter ("y", or the French "o"); without a terminal to
/// ask at, No.
fn ask_no(s: &Session, question: &str) -> Result<bool, String> {
    use std::io::{BufRead, IsTerminal, Write};
    if !std::io::stdin().is_terminal() {
        return Ok(false);
    }
    print!("{question} {} ", s.tr.text("cli-no-yes", None));
    std::io::stdout().flush().map_err(|e| e.to_string())?;
    let mut line = String::new();
    std::io::stdin().lock().read_line(&mut line).map_err(|e| e.to_string())?;
    Ok(said_yes(&line))
}

/// Whether an answer at the terminal is Yes: it begins with "y" (yes) or "o" (oui).
fn said_yes(line: &str) -> bool {
    let answer = line.trim().to_lowercase();
    answer.starts_with('y') || answer.starts_with('o')
}

/// A question answered Yes or No at the terminal, Yes when Enter alone is
/// pressed; without a terminal to ask at, Yes.
fn ask_yes(s: &Session, question: &str) -> Result<bool, String> {
    use std::io::{BufRead, IsTerminal, Write};
    if !std::io::stdin().is_terminal() {
        return Ok(true);
    }
    print!("{question} {} ", s.tr.text("cli-yes-no", None));
    std::io::stdout().flush().map_err(|e| e.to_string())?;
    let mut line = String::new();
    std::io::stdin().lock().read_line(&mut line).map_err(|e| e.to_string())?;
    let answer = line.trim().to_lowercase();
    Ok(!(answer.starts_with('n')))
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
    porch::judge(files, &config.mail_sources(), store.as_ref(), &known, &senders, sioul_core::zone::now().timestamp().as_second())
}

/// Your mail filters on `account`'s server after a fetch (`new`: its inbox's
/// arrivals, maybe none), as every device that fetches runs them
/// (docs/client.md, "Filters"): what they did, and what they could not do,
/// each in a line. Nothing `sioul watch` tells waits for them: it tells codes
/// alone, which no filter touches.
fn filter_on_server(s: &Session, config: &Config, account: &Account, new: &[PathBuf]) {
    let Ok(password) = secret::password(account) else { return };
    let done = sioul_sync::filters::after_fetch(config, account, &password, new, false);
    let time = sioul_core::zone::now().strftime("%H:%M").to_string();
    if !done.acted.is_empty() {
        let mut args = s.tr.counted(done.acted.len());
        args.set("account", account.id.clone());
        let line = s.tr.text("filter-cli-acted", Some(&args));
        println!("{}", s.say("watch-line", &[("time", time.clone()), ("line", line)]));
    }
    if let Some(why) = done.said(&s.tr, account.address.as_deref().unwrap_or(&account.id)) {
        println!("{}", s.say("watch-line", &[("time", time), ("line", why)]));
    }
}

pub(crate) fn watch_command(s: &Session) -> Result<(), String> {
    let accounts: Vec<Account> = syncing(&s.config, None).into_iter().cloned().collect();
    if accounts.is_empty() {
        return Err(s.tr.text("sync-nothing", None));
    }
    let names = accounts.iter().map(|a| a.id.clone()).collect::<Vec<_>>().join(", ");
    println!("{}", s.say("watch-started", &[("accounts", names)]));
    // The spam filter's training by itself, asked each minute as the window asks it, silently.
    crate::spam::by_itself::every_minute(s);
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
        let time = sioul_core::zone::now().strftime("%H:%M").to_string();
        match result {
            Ok(report) if report.new.is_empty() => {
                // Nothing new, but a filter's failure to try again, or another device's mark to look at again.
                let config = Config::load(&s.config_path).unwrap_or_default();
                if let Some(account) = config.account(&id).filter(|a| !report.first && sioul_sync::filters::due(a)) {
                    filter_on_server(s, &config, account, &[]);
                }
            }
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
                if let Some(account) = config.account(&id) {
                    filter_on_server(s, &config, account, &report.new);
                }
                let arrived = triage_files(&config, &report.new);
                print_right_now(s, &arrived);
                // As the matrix of what reaches you says (as usual, at once at any
                // time; "On the Porch only" a choice while you sleep), with its
                // layers read from their files: a code not told waits on the Porch.
                use sioul_core::attention::{self, Attention, Kind, Level, Row, Slots};
                let now = sioul_core::zone::now();
                let stamp = now.timestamp().as_second();
                let overrides = sioul_core::quiet::Overrides::load(&sioul_core::quiet::Overrides::default_path());
                let mode = sioul_core::quiet::mode(&config.week_hours(), &config.time_off, &overrides, &sioul_core::quiet::Blocks::read_now(&now), &now);
                let moment = attention::Now::of(&mode).layers(Slots::load(&Slots::default_path()).at(&now), attention::dnd_from_files(&config, stamp));
                if Attention::of(&config).level(Row::Own(Kind::Codes), &moment) != Level::Now {
                    continue;
                }
                for code in view::codes(&arrived, &s.tr) {
                    if let Err(e) = notify::code(&code.title, &code.body(), Vec::new()) {
                        eprintln!("{}", crate::plain_lines(&e));
                    }
                }
            }
            Err(e) => eprintln!("{}", crate::plain_lines(&s.say("watch-line", &[("time", time), ("line", e.sentence(&s.tr, &id))]))),
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use sioul_core::i18n::Translator;

    /// `accounts list` says how each account signs in: a password, or Google's sign-in.
    #[test]
    fn a_google_account_has_no_password() {
        let s = Session { config: Config::default(), config_path: PathBuf::new(), tr: Translator::new("en") };
        let account = |auth: Option<&str>| Account { auth: auth.map(str::to_string), ..Account::imap("me", "me@example.org", "imap.example.org", 993, Security::Tls, None) };
        // What the keyring gives a Google account is a mark of its sign-in, never a password.
        assert_eq!(secret_said(&s, &account(Some("google")), Ok("google:2026-10-09".into())), "signed in with Google (the access in the keyring)");
        assert_eq!(secret_said(&s, &account(None), Ok("hunter2".into())), "password in the keyring");
        assert_eq!(secret_said(&s, &account(None), Err(SyncError::NoPassword)), "no password yet: `sioul account password me`");
    }

    /// `accounts list` says a Microsoft account signs in with Microsoft;
    /// `account password` refuses one at Microsoft's server, saying what to do.
    #[test]
    fn a_microsoft_account_has_no_password() {
        let s = Session { config: Config::default(), config_path: PathBuf::new(), tr: Translator::new("en") };
        let mut account = Account::imap("me", "me@outlook.com", "outlook.office365.com", 993, Security::Tls, None);
        account.auth = Some("microsoft".into());
        assert_eq!(secret_said(&s, &account, Ok("microsoft:1760000000".into())), "signed in with Microsoft (the access in the keyring)");
        let config: Config = toml::from_str("[[account]]\nid = \"me\"\nkind = \"imap\"\naddress = \"me@outlook.com\"\nhost = \"outlook.office365.com\"\nauth = \"microsoft\"\n").unwrap();
        let s = Session { config, config_path: PathBuf::new(), tr: Translator::new("en") };
        let refused = password(&s, "me").unwrap_err();
        assert!(refused.contains("sioul account sign-in me"), "{refused}");
    }

    /// The review of 15 October 2026, L3: the mailbox of another login than
    /// the address is kept on Yes alone, at the terminal too: Enter alone,
    /// or anything but Yes, is No, and so is no terminal.
    #[test]
    fn another_logins_mailbox_is_no_by_default() {
        use std::io::IsTerminal;
        assert!(said_yes("y\n") && said_yes("Yes") && said_yes("oui\n") && said_yes(" O "));
        assert!(!said_yes("\n") && !said_yes("n") && !said_yes("non") && !said_yes("") && !said_yes("maybe"));
        let s = Session { config: Config::default(), config_path: PathBuf::new(), tr: Translator::new("fr") };
        assert_eq!(s.tr.text("cli-no-yes", None), "[o/N]");
        if !std::io::stdin().is_terminal() {
            assert_eq!(ask_no(&s, "?"), Ok(false));
        }
    }
}
