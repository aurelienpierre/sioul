// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

//! Your consent to the filter learning from a Microsoft address's mail, as
//! Microsoft's terms ask, address by address (docs/microsoft.md, "The spam
//! filter").
//!
//! Under the window's Train now, a sentence names the addresses it learns
//! from (`named`). Pressing it is your consent, for each, and the addresses
//! named at that press are kept on this computer (`consent.toml` beside the
//! corpus, never shared). From then on, every download and every training
//! learns from a Microsoft address's mail only when that address is on the
//! list, whoever starts them: the press itself, training again by itself,
//! the command line, an AI agent's jobs. A Microsoft address added after the
//! last press waits for the next one. A work or school Microsoft address is
//! never learned from, named or not: its mail is its organisation's too.
//! Other addresses are learned from as they always were.
//!
//! The corpus records, for each account, whether it is Microsoft's and its
//! address when it is downloaded, and every configured Microsoft account's
//! at the start of each run (`corpus.rs`), so that what was kept of an
//! address no longer on the list is not read again, even once the account is
//! gone. A folder that recorded nothing yet (kept before kinds were
//! recorded) is read only when the configuration says its account is not
//! Microsoft's (`MicrosoftIds`): a Microsoft one's counts as a work one's.

use crate::{Dirs, LearnError, io_error, now};
use serde::{Deserialize, Serialize};
use sioul_core::config::{Account, Config};
use std::collections::BTreeSet;

/// The addresses named under Train now at its last press.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Consent {
    /// When Train now was last pressed (Unix seconds); 0: never.
    #[serde(default)]
    pub pressed: i64,
    /// The addresses it named then, lowercased.
    #[serde(default)]
    pub addresses: BTreeSet<String>,
}

impl Consent {
    /// The consent kept on this computer; none (nothing consented) when
    /// Train now was never pressed, or its file cannot be read.
    pub fn load(dirs: &Dirs) -> Consent {
        std::fs::read_to_string(dirs.consent()).ok().and_then(|t| toml::from_str(&t).ok()).unwrap_or_default()
    }

    /// Train now pressed, these addresses named under it (`named`): kept in
    /// place of the list before. Written beside, then moved: never half a list.
    pub fn given(dirs: &Dirs, named: impl IntoIterator<Item = String>) -> Result<Consent, LearnError> {
        let consent = Consent { pressed: now(), addresses: named.into_iter().map(|a| a.trim().to_lowercase()).filter(|a| !a.is_empty()).collect() };
        let path = dirs.consent();
        std::fs::create_dir_all(&dirs.data).map_err(|e| io_error(&dirs.data, e))?;
        let text = toml::to_string(&consent).map_err(|e| io_error(&path, e))?;
        let beside = path.with_extension("toml.new");
        std::fs::write(&beside, text).map_err(|e| io_error(&beside, e))?;
        std::fs::rename(&beside, &path).map_err(|e| io_error(&path, e))?;
        Ok(consent)
    }

    /// Whether the filter may download and learn from an account's mail:
    /// any account but Microsoft's; a personal Microsoft one only when its
    /// address is on the list; a work or school one never.
    pub fn allows(&self, account: &Account) -> bool {
        if !account.is_microsoft() {
            return true;
        }
        let kind = if account.is_microsoft_work() { "work" } else { "personal" };
        self.allows_kept(Some(kind), account.address.as_deref())
    }

    /// The same for what the corpus keeps of an account, by what its
    /// download recorded: `microsoft`, its kind ("personal", "work"; None
    /// for any other account), and its address. Anything else than
    /// "personal" counts as work: never read.
    pub fn allows_kept(&self, microsoft: Option<&str>, address: Option<&str>) -> bool {
        match microsoft {
            None => true,
            Some("personal") => address.is_some_and(|a| self.addresses.contains(&a.trim().to_lowercase())),
            Some(_) => false,
        }
    }
}

/// The configuration's Microsoft accounts, by id, switched off or not: what
/// reading the corpus needs to know of a folder whose download recorded
/// nothing (kept before kinds were recorded): one of a Microsoft account
/// counts as a work one's, never read, until a download, or the start of a
/// run, records its kind (the re-check of 15 October 2026, M2-bis).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct MicrosoftIds(BTreeSet<String>);

impl MicrosoftIds {
    pub fn of(config: &Config) -> MicrosoftIds {
        MicrosoftIds(config.every_account().filter(|a| a.is_microsoft()).map(|a| a.id.clone()).collect())
    }

    pub fn contains(&self, id: &str) -> bool {
        self.0.contains(id)
    }
}

/// The addresses Train now learns from, as its sentence names them: every
/// account that syncs, but a work or school Microsoft one.
pub fn named(config: &Config) -> Vec<String> {
    config.accounts.iter().filter(|a| a.syncs() && !(a.is_microsoft() && a.is_microsoft_work())).map(label).collect()
}

/// The work or school Microsoft addresses that sync: never learned from.
pub fn work(config: &Config) -> Vec<String> {
    config.accounts.iter().filter(|a| a.syncs() && a.is_microsoft() && a.is_microsoft_work()).map(label).collect()
}

/// An account's address, or its id when it has none.
fn label(account: &Account) -> String {
    account.address.clone().unwrap_or_else(|| account.id.clone())
}

#[cfg(test)]
mod tests {
    use super::*;
    use sioul_core::config::Security;

    fn microsoft(id: &str, address: &str, kind: Option<&str>) -> Account {
        let mut account = Account::imap(id, address, "outlook.office365.com", 993, Security::Tls, None);
        account.auth = Some("microsoft".into());
        account.microsoft_kind = kind.map(str::to_string);
        account
    }

    /// The review of 15 October 2026, M2: a Microsoft address is learned
    /// from only once named at a press of Train now; a work one never; any
    /// other address as before; the list kept, replaced at each press.
    #[test]
    fn a_microsoft_address_is_learned_from_once_named() {
        let root = std::env::temp_dir().join(format!("sioul-learn-consent-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        let dirs = Dirs::under(&root);
        let plain = Account::imap("home", "me@example.org", "imap.example.org", 993, Security::Tls, None);
        let personal = microsoft("live", "Me@Outlook.example", Some("personal"));
        let work = microsoft("office", "me@contoso.example", Some("work"));
        let config = Config { accounts: vec![plain.clone(), personal.clone(), work.clone()], ..Config::default() };
        // Never pressed: nothing consented; Microsoft's personal address waits, the work one is never named.
        let none = Consent::load(&dirs);
        assert_eq!(none, Consent::default());
        assert!(none.allows(&plain) && !none.allows(&personal) && !none.allows(&work));
        assert_eq!(named(&config), ["me@example.org", "Me@Outlook.example"]);
        assert_eq!(super::work(&config), ["me@contoso.example"]);
        // Pressed with both named (and the work address slipped in): the personal one allowed, the work one still not.
        let given = Consent::given(&dirs, named(&config).into_iter().chain(["me@contoso.example".to_string()])).unwrap();
        assert_eq!(Consent::load(&dirs), given);
        assert!(given.allows(&personal) && given.allows(&plain) && !given.allows(&work));
        // What the corpus recorded: read only while consented.
        assert!(given.allows_kept(Some("personal"), Some("me@outlook.example")) && given.allows_kept(None, None));
        assert!(!given.allows_kept(Some("work"), Some("me@contoso.example")) && !given.allows_kept(Some("personal"), Some("other@outlook.example")) && !given.allows_kept(Some("personal"), None));
        // Pressed again without it (its account gone): no longer allowed, even what was kept of it.
        let again = Consent::given(&dirs, ["me@example.org".to_string()]).unwrap();
        assert!(!again.allows(&personal) && !again.allows_kept(Some("personal"), Some("me@outlook.example")));
        // A Microsoft account without its kind recorded counts as work.
        let unknown = microsoft("old", "me@outlook.example", None);
        assert!(!Consent::given(&dirs, ["me@outlook.example".to_string()]).unwrap().allows(&unknown));
        // A file that cannot be read: nothing consented.
        std::fs::write(dirs.consent(), "addresses = 3").unwrap();
        assert_eq!(Consent::load(&dirs), Consent::default());
        std::fs::remove_dir_all(&root).unwrap();
    }
}
