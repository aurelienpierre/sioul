// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

//! Proven mail from someone you deal with ("vouched" here; docs/spam-filter.md,
//! "Proven mail your provider called spam", part 2 of 4 November 2026): a
//! message verified for its From (`Trust::Verified`: DMARC passed for its
//! domain, or a signature of that domain passed, by Sioul's own check first),
//! whose From's organisational domain (`trust::organisational`) is one you
//! dealt with; at a domain millions share (the list `Config::own_domains`
//! leaves out), never the domain, only the exact address you dealt with.
//! Any one dealing is enough:
//!
//! - **Sent**: you wrote to an address there (the To, Cc and Bcc of the
//!   newest `SENT_READ` messages of each Sent folder);
//! - **Book**: a card of your address books has an address there;
//! - **Site**: one of your Sites names it (its `announced_by` domains, its address);
//! - **Contract**: a contract or subscription of yours names it (its way to stop it, a page or an address);
//! - **Kept**: you kept its mail: a message from there outside every Junk
//!   folder, the trash, drafts and Sent that you read, answered or flagged, or
//!   keep in a folder of yours or the archive;
//! - **Not spam**: you said Not spam to one of its messages (the label log, on
//!   any device; another mail program's `$NotJunk`).
//!
//! Your mail counts for its last `DEALT_DAYS`, as each folder's index holds
//! it (`mailcache`; `GATHER_FILL` messages it does not know yet read at each
//! gathering, as the mailbox fills it).
//!
//! Your own word comes first: when your newest word on that domain's mail is
//! Spam, it is no domain you deal with, whatever else says so, until you say
//! Not spam again. The order of words, all of Sioul's: your own word >
//! blocked > forged > vouched > your filter > your provider.

use crate::card::Card;
use crate::config::Config;
use crate::folders::Role;
use crate::trust::{self, Trust};
use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

/// What is done with proven mail your provider called spam (the matrix's row,
/// `[spam] proven`): brought back into the inbox as it arrives, with Undo
/// (unsaid); left in its Junk folder, marked; nothing.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum Proven {
    #[default]
    BringBack,
    Mark,
    Nothing,
}

impl Proven {
    pub const ALL: [Proven; 3] = [Proven::BringBack, Proven::Mark, Proven::Nothing];

    /// As written: "bring-back", "mark", "nothing".
    pub fn as_str(self) -> &'static str {
        match self {
            Proven::BringBack => "bring-back",
            Proven::Mark => "mark",
            Proven::Nothing => "nothing",
        }
    }

    pub fn read(text: &str) -> Option<Proven> {
        Proven::ALL.into_iter().find(|p| p.as_str() == text.trim())
    }
}

/// How you dealt with a domain (the first kind found is kept, for the words and the tests).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Dealing {
    Sent,
    Book,
    Site,
    Contract,
    Kept,
    NotSpam,
}

/// The organisational domains you deal with, the exact addresses you deal
/// with at a domain millions share, and those your own word refused.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Dealings {
    domains: BTreeMap<String, Dealing>,
    /// At a shared domain, the address itself, lower case: never its domain.
    addresses: BTreeMap<String, Dealing>,
    /// Your newest word on its mail said spam: domains, and addresses at shared ones.
    refused: BTreeSet<String>,
    /// Domains millions share: never vouched as domains.
    shared: BTreeSet<String>,
}

/// An address's or a name's organisational domain, lower case; none for a public suffix or nothing.
fn organisational_of(text: &str) -> Option<String> {
    let domain = text.rsplit_once('@').map_or(text, |(_, d)| d).trim().trim_matches(['<', '>', '.', ' ']).to_ascii_lowercase();
    (!domain.is_empty() && domain.contains('.')).then(|| trust::organisational(&domain)).flatten()
}

/// An address as compared: lower case, without its angle brackets; none for what is not one.
fn address_of(text: &str) -> Option<String> {
    let address = text.trim().trim_matches(['<', '>', ' ']).to_ascii_lowercase();
    address.split_once('@').is_some_and(|(local, domain)| !local.is_empty() && domain.contains('.')).then_some(address)
}

impl Dealings {
    /// What a dealing or a word is kept under for `text` (an address, a host,
    /// a domain): its organisational domain; at a domain millions share, the
    /// address itself, and nothing for the domain alone.
    fn key_of(&self, text: &str) -> Option<String> {
        let domain = organisational_of(text)?;
        if self.shared.contains(&domain) { address_of(text) } else { Some(domain) }
    }

    /// A dealing with `text` (an address, a host, a domain; `key_of`); the first kind kept.
    pub fn with(&mut self, text: &str, how: Dealing) {
        let Some(key) = self.key_of(text) else { return };
        let kept = if key.contains('@') { &mut self.addresses } else { &mut self.domains };
        kept.entry(key).or_insert(how);
    }

    /// Your newest word on the mail of `text` (its domain, or the address at a shared one) said spam.
    pub fn refuse(&mut self, text: &str) {
        if let Some(key) = self.key_of(text) {
            self.refused.insert(key);
        }
    }

    /// A domain millions share (`words::BrandWords::shared`): never vouched as
    /// a domain, only address by address. Said before any dealing.
    pub fn share(&mut self, text: &str) {
        if let Some(domain) = organisational_of(text) {
            self.shared.insert(domain);
        }
    }

    /// Nothing dealt with: nothing is vouched.
    pub fn is_empty(&self) -> bool {
        self.domains.is_empty() && self.addresses.is_empty()
    }

    /// How you dealt with the sender `text`, if you deal with it now: by its
    /// domain, at a shared domain by its exact address; never one your newest
    /// word refused.
    pub fn of(&self, text: &str) -> Option<Dealing> {
        let key = self.key_of(text)?;
        if self.refused.contains(&key) {
            return None;
        }
        if key.contains('@') { self.addresses.get(&key).copied() } else { self.domains.get(&key).copied() }
    }


    /// Every dealing found, from the configuration's addresses, books, sites
    /// and contracts, and from the mail kept here and the label logs (see the
    /// module). Only what each folder's index holds is read (`mailcache`):
    /// nothing is parsed for it, but the headers of your newest sent mail.
    pub fn gather(config: &Config) -> Dealings {
        let mut dealings = Dealings::default();
        for domain in &crate::words::Words::of(config).brands.shared {
            dealings.share(domain);
        }
        let own: BTreeSet<String> = config.accounts.iter().filter_map(|a| a.address.as_deref()).map(str::to_ascii_lowercase).collect();
        // The address books: every address on a card.
        let book = crate::porch::Book::load(crate::reach::region(config));
        for index in 0..book.len() {
            for address in book.card(index).map(|c| c.addresses.as_slice()).unwrap_or_default() {
                dealings.with(address, Dealing::Book);
            }
        }
        // Your Sites, and your contracts' ways to stop them.
        for site in crate::sites::sites(config) {
            for domain in &site.announced_by {
                dealings.with(domain, Dealing::Site);
            }
            dealings.with(&crate::sites::host_of(&site.url), Dealing::Site);
        }
        if let Some(root) = config.notes_root_path()
            && let Ok(contracts) = crate::contracts::Contracts::load(&root)
        {
            for contract in &contracts.list {
                let cancel = contract.cancel.trim();
                let place = if cancel.contains("://") { crate::sites::host_of(cancel) } else { cancel.to_string() };
                dealings.with(&place, Dealing::Contract);
            }
        }
        // Your mail of the last DEALT_DAYS, as each folder's index holds it (GATHER_FILL
        // messages it does not know yet read at each gathering, as the mailbox reads them):
        // who you wrote to, what you kept, by Message-ID.
        let mut domain_of_id: HashMap<String, String> = HashMap::new();
        let since = jiff::Timestamp::now().as_second() - DEALT_DAYS * 86_400;
        let mut budget = GATHER_FILL;
        for account in config.accounts.iter().filter(|a| a.syncs()) {
            let root = account.maildir_path();
            for folder in crate::folders::saved(&account.id) {
                let recent = crate::mailcache::recent(&account.id, &root, &folder.local, since, budget);
                budget = budget.saturating_sub(recent.read);
                let lines = recent.lines;
                if folder.role == Role::Sent {
                    for address in sent_to(&lines) {
                        dealings.with(&address, Dealing::Sent);
                    }
                    continue;
                }
                for (path, line) in lines.iter() {
                    let Some(from) = line.from_address.as_deref().filter(|f| !own.contains(*f)) else { continue };
                    if let Some(id) = line.message_id.as_deref() {
                        domain_of_id.insert(crate::mailindex::bare_id(id), from.to_string());
                    }
                    let flags = crate::maildir::flags_of(path);
                    let keywords = crate::maildir::keywords_of(path);
                    if keywords.not_junk && !keywords.junk {
                        dealings.with(from, Dealing::NotSpam);
                    }
                    let kept = match folder.role {
                        Role::Junk | Role::Trash | Role::Drafts | Role::Sent => false,
                        Role::Archive | Role::Other => true,
                        Role::Inbox | Role::All => flags.contains('S') || flags.contains('R') || flags.contains('F'),
                    };
                    if kept {
                        dealings.with(from, Dealing::Kept);
                    }
                }
            }
        }
        // Your words, every device's, the newest on each domain (an address at a shared one) deciding.
        let mut newest: HashMap<String, (crate::spam::labels::Label, String)> = HashMap::new();
        for entry in crate::spam::labels::read_all() {
            let Some(from) = entry.message_id.as_deref().and_then(|id| domain_of_id.get(id)) else { continue };
            if let Some(key) = dealings.key_of(from) {
                newest.insert(key, (entry.label, from.clone()));
            }
        }
        for (label, from) in newest.into_values() {
            match label {
                crate::spam::labels::Label::Ham => dealings.with(&from, Dealing::NotSpam),
                crate::spam::labels::Label::Spam => dealings.refuse(&from),
            }
        }
        dealings
    }
}

/// The newest of your sent messages read for their recipients, each account's.
const SENT_READ: usize = 2_000;

/// How far back your mail makes a dealing: a little over a year.
const DEALT_DAYS: i64 = 400;

/// Messages no index knows yet, read at each gathering over every folder
/// (the mailbox's `STEP_FILL`): the indexes fill as the mailbox's do.
const GATHER_FILL: usize = crate::mailbox::STEP_FILL;

/// The recipients' addresses of the sent mail in `lines` (a Sent folder's),
/// lower case, its newest `SENT_READ`: each message's header block read
/// once in this process, kept by its file.
fn sent_to(lines: &[(PathBuf, crate::mailcache::Line)]) -> Vec<String> {
    static READ: Mutex<Option<HashMap<PathBuf, Vec<String>>>> = Mutex::new(None);
    let mut newest: Vec<&(PathBuf, crate::mailcache::Line)> = lines.iter().collect();
    newest.sort_by_key(|(_, line)| std::cmp::Reverse(line.arrived));
    newest.truncate(SENT_READ);
    let mut read = READ.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
    let read = read.get_or_insert_with(HashMap::new);
    let mut out = Vec::new();
    for (path, _) in newest {
        let domains = read.entry(path.clone()).or_insert_with(|| {
            let Some(raw) = crate::mailcache::headers_of(path) else { return Vec::new() };
            let Some(headers) = mail_parser::MessageParser::default().parse_headers(&raw[..]) else { return Vec::new() };
            [headers.to(), headers.cc(), headers.bcc()].into_iter().flatten().flat_map(|a| a.iter()).filter_map(|a| a.address()).filter_map(address_of).collect()
        });
        out.extend(domains.iter().cloned());
    }
    out
}

/// Whether the message on `card` is proven mail from someone you deal with:
/// verified for its From (`trust`, as the Porch judged it), its From's
/// organisational domain one you deal with (`dealings`).
pub fn vouched(card: &Card, trust: Trust, dealings: &Dealings) -> bool {
    trust == Trust::Verified && card.from_address.as_deref().is_some_and(|from| dealings.of(from).is_some())
}

/// Whether the domain of `text` is one millions share, by `words`
/// (`brands.shared`): never vouched, its mail no dealing.
pub fn shared(words: &crate::words::Words, text: &str) -> bool {
    let Some(domain) = organisational_of(text) else { return false };
    words.brands.shared.iter().any(|s| organisational_of(s).as_deref() == Some(domain.as_str()))
}

/// What the sender `from` is dealt with by, as `Dealings` keeps it with
/// `words`' shared domains: its organisational domain, or at a domain
/// millions share its exact address, lower case.
pub fn sender_key(words: &crate::words::Words, from: &str) -> Option<String> {
    if shared(words, from) { address_of(from) } else { organisational_of(from) }
}

/// How far the sender of the message on `card` is proven, as the Porch
/// judges it (`porch::triage`, `view::mail_item`): the checks of the ids
/// trusted for its account (`trusted`, Sioul's own first), a list's DMARC
/// failure excused by a seal of someone answering for it (`own_domains`'
/// seals among them), never for a code.
pub fn judged(card: &Card, trusted: &[String], own_domains: &[String]) -> Trust {
    let results = trust::read_auth_results(&card.headers, trusted);
    let failed = results.as_ref().is_some_and(|r| r.dmarc == Some(trust::Outcome::Fail));
    let code = card.is_list && failed && card.code.unwrap_or_else(|| crate::codes::detect_message(&crate::words::current().codes, card).is_some());
    let excused = (card.is_list && !code).then(|| trust::list_excused(results.as_ref(), &card.headers, &trust::sealers(trusted, own_domains))).flatten();
    trust::judge_sender(results.as_ref(), excused.as_ref(), card.sender_domain()).0
}

/// How long the dealings gathered are used before they are gathered again.
const FRESH: std::time::Duration = std::time::Duration::from_secs(120);

type Gathered = Option<(std::time::Instant, Arc<Dealings>)>;
static CURRENT: Mutex<Gathered> = Mutex::new(None);

/// The dealings of this process: gathered from the configuration
/// (`config::default_path`, the one Sioul reads), again two minutes later at
/// the soonest, or at once after `forget`; none when it cannot be read.
pub fn current() -> Arc<Dealings> {
    let mut kept = CURRENT.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
    if let Some((at, dealings)) = kept.as_ref()
        && at.elapsed() < FRESH
    {
        return Arc::clone(dealings);
    }
    let dealings = Arc::new(Config::load(&crate::config::default_path()).map(|c| Dealings::gather(&c)).unwrap_or_default());
    *kept = Some((std::time::Instant::now(), Arc::clone(&dealings)));
    dealings
}

/// The dealings gathered again at the next ask: after your word on a message (Spam, Not spam).
pub fn forget() {
    *CURRENT.lock().unwrap_or_else(std::sync::PoisonError::into_inner) = None;
}

/// `vouched`, with this process's dealings (`current`).
pub fn vouched_now(card: &Card, trust: Trust) -> bool {
    trust == Trust::Verified && card.from_address.is_some() && vouched(card, trust, &current())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn card(from: &str) -> Card {
        Card::from_bytes(format!("From: Someone <{from}>\r\nSubject: Hello\r\n\r\nHi.\r\n").as_bytes()).unwrap()
    }

    /// Each kind of dealing vouches for its organisational domain, a
    /// subdomain too; a domain millions share never; an unverified sender
    /// never; your newest word Spam refuses it, Not spam brings it back.
    #[test]
    fn each_dealing_vouches_but_shared_domains_and_unproven_senders_never() {
        let mut dealings = Dealings::default();
        dealings.share("gmail.example");
        for (text, how) in [("notification@notif.post.example", Dealing::Kept), ("billing@energy.example", Dealing::Contract), ("bank.example", Dealing::Site), ("ana@friends.example", Dealing::Book), ("iris@client.example", Dealing::Sent), ("news@club.example", Dealing::NotSpam), ("someone@gmail.example", Dealing::Book)] {
            dealings.with(text, how);
        }
        assert_eq!(dealings.of("other@post.example"), Some(Dealing::Kept), "the organisational domain");
        assert_eq!(dealings.of("x@mail.bank.example"), Some(Dealing::Site));
        for (from, how) in [("letters@energy.example", Dealing::Contract), ("ana@friends.example", Dealing::Book), ("iris@client.example", Dealing::Sent), ("news@club.example", Dealing::NotSpam)] {
            assert_eq!(dealings.of(from), Some(how), "{from}");
            assert!(vouched(&card(from), Trust::Verified, &dealings), "{from}");
        }
        // A domain millions share: never as a domain; the exact address you dealt with, yes.
        assert_eq!(dealings.of("someone@gmail.example"), Some(Dealing::Book), "the address itself");
        assert!(vouched(&card("Someone@Gmail.example"), Trust::Verified, &dealings), "its case aside");
        assert_eq!(dealings.of("other@gmail.example"), None, "another address there: no dealing");
        assert!(!vouched(&card("other@gmail.example"), Trust::Verified, &dealings));
        dealings.with("gmail.example", Dealing::Site);
        assert_eq!(dealings.of("third@gmail.example"), None, "the shared domain itself is never a dealing");
        dealings.refuse("someone@gmail.example");
        assert_eq!(dealings.of("someone@gmail.example"), None, "your Spam on that address refuses it alone");
        // Unproven: never, the domain shown being anyone's to write.
        assert!(!vouched(&card("notification@notif.post.example"), Trust::Unverified, &dealings));
        assert!(!vouched(&card("notification@notif.post.example"), Trust::Forged, &dealings));
        // A stranger: no dealing.
        assert!(!vouched(&card("prize@lottery.example"), Trust::Verified, &dealings));
        // Your word Spam on its mail: refused.
        dealings.refuse("ads@club.example");
        assert!(!vouched(&card("news@club.example"), Trust::Verified, &dealings));
    }

    /// The Porch's lane (`porch::provider_sets_aside`): your provider's verdict
    /// sets a stranger's mail aside, never proven mail from someone you deal
    /// with, which takes its own lane; an unverified sender's, or a domain you
    /// do not deal with, still.
    #[test]
    fn your_provider_s_verdict_alone_never_sets_vouched_mail_aside() {
        let mut dealings = Dealings::default();
        dealings.with("commande@shop.post.example", Dealing::Kept);
        let verdict = crate::trust::SpamVerdict { flagged: true, score: Some(5.3), source: "SpamAssassin" };
        let notice = card("notification@notif.post.example");
        assert_eq!(crate::porch::provider_sets_aside(&notice, Trust::Verified, verdict.clone(), &dealings), None);
        assert!(matches!(crate::porch::provider_sets_aside(&notice, Trust::Unverified, verdict.clone(), &dealings), Some((crate::porch::Lane::SetAside, crate::porch::Reason::Spam { .. }))));
        assert!(matches!(crate::porch::provider_sets_aside(&card("prize@lottery.example"), Trust::Verified, verdict, &dealings), Some((crate::porch::Lane::SetAside, crate::porch::Reason::Spam { .. }))));
    }

    /// Gathered from a stand-in profile: an address book's card, a site, a
    /// contract, mail read in the inbox, mail kept in a folder, mail sent, a
    /// `$NotJunk` message, your words in the label log (Spam on one domain,
    /// Not spam on another); a message unread in the inbox and the Junk
    /// folder's mail are no dealing.
    #[test]
    fn dealings_are_gathered_from_your_mail_books_sites_contracts_and_words() {
        crate::config::stand_ins_only();
        let base = crate::config::data_dir().join(format!("vouched-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&base);
        let id = format!("home-vouched-{}", std::process::id());
        let mut account = crate::config::Account::imap(&id, "noa@home.example", "imap.example.org", 993, crate::config::Security::Tls, None);
        account.maildir = Some(base.join("mail").display().to_string());
        let folders = [("INBOX", "", Role::Inbox), ("Sent", ".Sent", Role::Sent), ("Junk", ".Junk", Role::Junk), ("Banque", ".Banque", Role::Other)];
        for (_, local, _) in folders {
            for sub in ["tmp", "new", "cur"] {
                std::fs::create_dir_all(account.maildir_path().join(local).join(sub)).unwrap();
            }
        }
        let saved: String = folders.iter().map(|(name, local, role)| format!("[[folder]]\nname = \"{name}\"\ndisplay = \"{name}\"\nrole = \"{}\"\nlocal = \"{local}\"\n", serde_json::to_value(role).unwrap().as_str().unwrap())).collect();
        std::fs::create_dir_all(crate::folders::saved_path(&id).parent().unwrap()).unwrap();
        std::fs::write(crate::folders::saved_path(&id), saved).unwrap();
        let put = |local: &str, uid: u32, from: &str, to: &str, flags: &str| {
            let dir = account.maildir_path().join(local).join(if flags.is_empty() { "new" } else { "cur" });
            let name = format!("1793700000.U7-{uid}.sioul{}", if flags.is_empty() { String::new() } else { format!("{}2,{flags}", crate::maildir::INFO) });
            std::fs::write(dir.join(name), format!("Message-ID: <m{uid}@example.org>\r\nFrom: {from}\r\nTo: {to}\r\nSubject: S{uid}\r\nDate: Wed, 04 Nov 2026 09:00:00 +0100\r\n\r\nText.\r\n")).unwrap();
        };
        put("", 1, "Orders <commande@shop.post.example>", "noa@home.example", "S");
        put("", 2, "Unread <x@unread.example>", "noa@home.example", "");
        put(".Banque", 3, "Bank <info@bank.example>", "noa@home.example", "");
        put(".Sent", 4, "Noa <noa@home.example>", "Iris <iris@client.example>, cc@other.example", "S");
        put(".Junk", 5, "Ads <ads@spam.example>", "noa@home.example", "S");
        put(".Junk", 6, "Club <news@club.example>", "noa@home.example", &format!("S{}", crate::maildir::NOT_JUNK));
        put("", 7, "Shop <deals@deals.example>", "noa@home.example", "S");
        // Your words: Spam on deals.example's message (newest), Not spam on spam.example's.
        let root = crate::spam::labels::state();
        let log = crate::spam::labels::own_log(&root, "desk");
        let said = |uid: u32, at: i64, source| crate::spam::labels::Entry { at, account: id.clone(), folder: "INBOX".into(), uidvalidity: 7, uid, message_id: Some(format!("m{uid}@example.org")), label: crate::spam::labels::Source::label(source), source };
        crate::spam::labels::append_to(&log, &said(7, 100, crate::spam::labels::Source::Junk)).unwrap();
        crate::spam::labels::append_to(&log, &said(5, 200, crate::spam::labels::Source::NotSpam)).unwrap();
        let mut site = crate::config::Account::imap("site", "", "", 0, crate::config::Security::Tls, None);
        site.kind = crate::config::AccountKind::Portal;
        site.url = Some("https://espace.energy.example/login".into());
        let config = Config { accounts: vec![account], sites: vec![site], ..Config::default() };
        let dealings = Dealings::gather(&config);
        let of = |from: &str| dealings.of(from);
        assert_eq!(of("notification@notif.post.example"), Some(Dealing::Kept), "read in the inbox: kept");
        assert_eq!(of("info@bank.example"), Some(Dealing::Kept), "in a folder of yours");
        assert_eq!(of("iris@client.example"), Some(Dealing::Sent));
        assert_eq!(of("someone@other.example"), Some(Dealing::Sent), "in copy");
        assert_eq!(of("news@club.example"), Some(Dealing::NotSpam), "$NotJunk in the Junk folder");
        assert_eq!(of("ads@spam.example"), Some(Dealing::NotSpam), "your Not spam");
        assert_eq!(of("billing@energy.example"), Some(Dealing::Site));
        assert_eq!(of("x@unread.example"), None, "unread in the inbox: no dealing");
        assert_eq!(of("deals@deals.example"), None, "read, but your newest word is Spam");
        let _ = std::fs::remove_dir_all(&base);
        let _ = std::fs::remove_file(crate::folders::saved_path(&id));
        let _ = std::fs::remove_file(&log);
        if let Some(dir) = crate::mailcache::account_dir(&id) {
            let _ = std::fs::remove_dir_all(dir);
        }
    }
}
