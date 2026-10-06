// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

//! Google's mail (Gmail, Google Workspace) signed in on Google's page, with
//! a key of your own: Google gives its mail's scope, a restricted one, to no
//! key of Sioul's yet. Google's page opens in the system's browser, its
//! answer comes back to this device (a loopback port, as for the calendars),
//! then IMAP and SMTP are given an access token (SASL XOAUTH2, sioul-sync's
//! `sasl.rs`). The other way, an app password, is the usual mail form
//! (`backend::add_mail`), with Google's words. docs/google.md, "Mail".

use crate::backend::{QtThread, Shared, config_path, load_config, say, show, start_watcher, tr, want_password};
use crate::pim::google_sentence;
use cxx_qt_lib::QString;
use sioul_core::config::{self, Account, AccountKind, Security};
use sioul_sync::discover::{GOOGLE_IMAP, GOOGLE_SMTP};
use sioul_sync::google::{self, Purpose};
use std::sync::Arc;
use std::sync::atomic::Ordering;

/// The key to sign in with: the one given, else yours kept on this device
/// (the mail's, else the calendars'). Sioul's own cannot read mail: said so.
fn key(address: &str, client_id: &str, client_secret: &str) -> Result<(String, String), String> {
    if !client_id.trim().is_empty() {
        return Ok((client_id.trim().to_string(), client_secret.trim().to_string()));
    }
    google::own_key(address).ok_or_else(|| if google::built_in().is_some() { tr().text("google-mail-built-in", None) } else { say("google-mail-no-key", &[("address", address.to_string())]) })
}

/// Whether a key of your own is kept on this device for an address: the
/// form then signs in with it, no ID or secret asked.
pub(crate) fn has_own_key(address: &str) -> bool {
    google::own_key(address.trim()).is_some()
}

/// Adds an address's Google mail, signed in on Google's page, or signs its
/// account in again (`again`: its id). Google's page opens in the browser;
/// the account's server is tried with the access given before anything is
/// kept. On a thread; the form says how it went.
pub(crate) fn add_google_mail(qt: &QtThread, shared: &Arc<Shared>, address: String, client_id: String, client_secret: String, again: Option<String>) {
    let (qt, shared) = (qt.clone(), Arc::clone(shared));
    shared.google_stop.store(false, Ordering::Relaxed);
    std::thread::spawn(move || {
        let address = address.trim().to_ascii_lowercase();
        let config = load_config();
        let outcome = (|| -> Result<Account, String> {
            // Signed in again: the account as it is, signed in with Google already.
            let existing = match &again {
                Some(id) => Some(config.every_account().find(|a| &a.id == id && a.kind == AccountKind::Imap && a.auth.as_deref() == Some("google")).cloned().ok_or_else(|| say("account-unknown", &[("id", id.clone())]))?),
                None => None,
            };
            if existing.is_none()
                && let Some(have) = config.every_account().find(|a| a.address.as_deref() == Some(address.as_str()) && matches!(a.kind, AccountKind::Imap | AccountKind::Jmap))
            {
                return Err(say("account-address-exists", &[("address", address.clone()), ("id", have.id.clone())]));
            }
            let (client_id, client_secret) = key(&address, &client_id, &client_secret)?;
            let account = existing.clone().unwrap_or_else(|| {
                let mut account = Account::imap(&config.free_id(&address), &address, GOOGLE_IMAP, 993, Security::Tls, None);
                account.auth = Some("google".into());
                account.smtp_host = Some(GOOGLE_SMTP.into());
                account.smtp_port = Some(465);
                account.smtp_security = Security::Tls;
                account
            });
            let id = account.id.clone();
            let sign_in = google::begin_for(Purpose::Mail, &client_id, &address).map_err(|e| google_sentence(&e, &id))?;
            let url = sign_in.url.clone();
            let _ = qt.queue(move |mut sioul| sioul.as_mut().open_url(QString::from(&url)));
            google::finish(sign_in, &client_id, &client_secret, &address, &shared.google_stop).map_err(|e| google_sentence(&e, &id))?;
            // The server tried with the access just given, before anything is kept.
            if let Err(e) = sioul_sync::test(&account, "") {
                if existing.is_none() {
                    // Nothing kept: the access goes back, unless the calendars share its key.
                    let _ = google::revoke_for(Purpose::Mail, &address);
                }
                return Err(e.sentence(tr(), &id));
            }
            if existing.is_none() {
                config::add_imap_account(&config_path(), &account)?;
                config::set_smtp(&config_path(), &id, GOOGLE_SMTP, 465, Security::Tls)?;
            }
            Ok(account)
        })();
        let line = outcome.as_ref().ok().map(|a| match &again {
            Some(_) => say("account-signed-in-again", &[("account", a.id.clone())]),
            None => say("account-added", &[("id", a.id.clone()), ("path", a.maildir_path().display().to_string())]),
        });
        if let Ok(account) = outcome.as_ref() {
            want_password(&shared, &account.id, None);
            if let Ok(mut statuses) = shared.statuses.lock() {
                statuses.remove(&account.id);
            }
            // A watcher waiting after a refusal tries again at once; none, one starts.
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
