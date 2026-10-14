// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

//! Microsoft's mail (Outlook.com, Hotmail, Live, MSN; Microsoft 365 for work
//! or school) signed in on Microsoft's page: Microsoft takes no password and
//! no app password from mail programs any more. Microsoft's page opens in the
//! system's browser, its answer comes back to this device (a loopback port,
//! as for Google), then IMAP and SMTP are given an access token (SASL
//! XOAUTH2, sioul-sync's `sasl.rs`). The mailbox is tried before anything is
//! kept (`microsoft::sign_in`). docs/microsoft.md.

use crate::backend::{QtThread, Shared, config_path, load_config, say, show, start_watcher, tr, want_password};
use cxx_qt_lib::QString;
use sioul_core::config::{self, Account, AccountKind, Security};
use sioul_sync::discover::MicrosoftKind;
use sioul_sync::{SyncError, microsoft, secret};
use std::sync::Arc;
use std::sync::atomic::Ordering;

/// Whether an account is Microsoft's mail: signed in with Microsoft, or
/// added before with a password at Microsoft's server (which no longer takes it).
fn at_microsoft(account: &Account) -> bool {
    account.kind == AccountKind::Imap && (account.is_microsoft() || account.host.as_deref().is_some_and(microsoft::is_imap_host))
}

/// Adds an address's Microsoft mail, signed in on Microsoft's page, or signs
/// its account in again (`again`: its id; an account added with a password
/// at Microsoft's server is switched to Microsoft's sign-in, its password
/// forgotten). `kind`: what discovery found ("personal", "work", "unknown");
/// `client_id` and `tenant`: an application of your own, else the one kept
/// for this address, else Sioul's. On a thread; the form says how it went.
pub(crate) fn add_microsoft_mail(qt: &QtThread, shared: &Arc<Shared>, address: String, kind: String, client_id: String, tenant: String, again: Option<String>) {
    let (qt, shared) = (qt.clone(), Arc::clone(shared));
    shared.oauth_stop.store(false, Ordering::Relaxed);
    std::thread::spawn(move || {
        let address = address.trim().to_ascii_lowercase();
        let config = load_config();
        let outcome = (|| -> Result<Account, String> {
            // Signed in again: the account as it is.
            let existing = match &again {
                Some(id) => Some(config.every_account().find(|a| &a.id == id && at_microsoft(a)).cloned().ok_or_else(|| say("account-unknown", &[("id", id.clone())]))?),
                None => None,
            };
            if existing.is_none()
                && let Some(have) = config.every_account().find(|a| a.address.as_deref() == Some(address.as_str()) && matches!(a.kind, AccountKind::Imap | AccountKind::Jmap))
            {
                return Err(say("account-address-exists", &[("address", address.clone()), ("id", have.id.clone())]));
            }
            let account = existing.clone().unwrap_or_else(|| Account::imap(&config.free_id(&address), &address, microsoft::IMAP_HOST, 993, Security::Tls, None));
            let login = account.login().unwrap_or(&address).to_string();
            // Its kind: what its sign-in found before, else what discovery found.
            let kind = match &existing {
                Some(a) if a.is_microsoft() && !a.is_microsoft_work() => MicrosoftKind::Personal,
                Some(a) if a.is_microsoft() => MicrosoftKind::Work,
                _ => MicrosoftKind::parse(&kind),
            };
            let label = address.clone();
            let app = microsoft::app_for(&login, &client_id).map_err(|e| e.sentence(tr(), &label))?.ok_or_else(|| tr().text("microsoft-no-app", None))?;
            let authority = microsoft::authority_for(&login, kind, &tenant).map_err(|e| e.sentence(tr(), &label))?;
            let opener = qt.clone();
            let show_page = move |url: &str| {
                let url = url.to_string();
                let _ = opener.queue(move |mut sioul| sioul.as_mut().open_url(QString::from(&url)));
            };
            let (account, _) = microsoft::sign_in(account, &app, &authority, show_page, &shared.oauth_stop).map_err(|e| e.sentence(tr(), &label))?;
            match &existing {
                None => {
                    config::add_imap_account(&config_path(), &account)?;
                    config::set_sign_in(&config_path(), &account)?;
                }
                Some(before) => {
                    config::set_sign_in(&config_path(), &account)?;
                    // A password kept before, which Microsoft no longer took: gone from the keyring.
                    if !before.is_microsoft() {
                        let _ = secret::forget(before);
                    }
                }
            }
            Ok(account)
        })();
        let line = outcome.as_ref().ok().map(|a| match &again {
            Some(_) => say("account-signed-in-again-microsoft", &[("account", a.id.clone())]),
            None => say("account-added", &[("id", a.id.clone()), ("path", a.maildir_path().display().to_string())]),
        });
        if let Ok(account) = outcome.as_ref() {
            want_password(&shared, &account.id, None);
            if let Ok(mut statuses) = shared.statuses.lock() {
                statuses.remove(&account.id);
            }
            // Its watcher knew the account as it was (a password, another login): another starts.
            if let Some(control) = shared.watchers.lock().ok().and_then(|mut w| w.remove(&account.id)) {
                control.stop();
            }
            start_watcher(&qt, &shared, account.clone());
        }
        let error = outcome.err();
        let _ = qt.queue(move |mut sioul| {
            sioul.as_mut().set_form_busy(false);
            match (line, error) {
                (Some(line), _) => {
                    sioul.as_mut().set_found(QString::default());
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

/// Whether an application of your own is kept on this device for an
/// address: the form then signs in with it, no id asked.
pub(crate) fn has_own_app(address: &str) -> bool {
    microsoft::own_app(address.trim()).is_some()
}

/// What an organisation's administrator does, once, for Sioul to read a
/// work or school mailbox, with the link that approves the application in
/// use for the organisation: yours kept on this device, else Sioul's, and
/// the directory kept from an earlier sign-in, else the address's domain.
/// Reads the keyring: off the window's thread (discovery's). Empty when
/// there is no application to approve yet.
pub(crate) fn admin_text(address: &str) -> String {
    let address = address.trim().to_ascii_lowercase();
    let Ok(Some(app)) = microsoft::app_for(&address, "") else { return String::new() };
    let tenant = microsoft::grant(&address).filter(|g| !g.personal).map(|g| g.tenant).unwrap_or_else(|| address.rsplit('@').next().unwrap_or("").to_string());
    say("microsoft-admin-steps", &[("link", microsoft::admin_consent_url(&app, &tenant))])
}

/// The same for an application id typed in the form, and the address's
/// domain: no keyring, for the window's thread. Empty while the id typed is
/// not one.
pub(crate) fn admin_text_for(address: &str, client_id: &str) -> String {
    let client_id = client_id.trim();
    if !microsoft::is_app_id(client_id) {
        return String::new();
    }
    let domain = address.trim().to_ascii_lowercase().rsplit('@').next().unwrap_or("").to_string();
    say("microsoft-admin-steps", &[("link", microsoft::admin_consent_url(client_id, &domain))])
}

/// A Microsoft account removed: its access forgotten here (Microsoft offers
/// no way to give it back), and, when you said so, the mail this device
/// keeps for it (its folder, the AI's answers, the spam filter's corpus).
/// The line that says it, with Microsoft's page where you withdraw the consent.
pub(crate) fn remove(account: &Account, delete_mail: bool) -> Result<String, String> {
    let id = account.id.clone();
    let page = microsoft::forget(account.login().unwrap_or(&id), !account.is_microsoft_work()).map_err(|e: SyncError| e.sentence(tr(), &id))?;
    config::remove_account(&config_path(), &id)?;
    if !delete_mail {
        return Ok(say("account-removed-microsoft", &[("id", id), ("path", account.maildir_path().display().to_string()), ("page", page.to_string())]));
    }
    sioul_core::maildir::forget_account(account)?;
    #[cfg(not(target_os = "android"))]
    sioul_learn::Dirs::standard().forget_account(&account.id).map_err(|e| e.to_string())?;
    Ok(say("account-removed-microsoft-mail", &[("id", id), ("page", page.to_string())]))
}
