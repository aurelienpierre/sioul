// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

//! The mail client from the terminal: `sioul mail folders | list | act | send`.
//! The window offers the same with "Undo" before anything reaches the server;
//! here, a command is already a deliberate act.

use crate::Session;
use clap::{Subcommand, ValueEnum};
use sioul_core::compose::{self, Draft, DraftKind};
use sioul_core::config::Account;
use sioul_core::spam::labels;
use sioul_core::{maildir, unsubscribe};
use sioul_sync::mailbox::{self, Action};
use sioul_sync::{SyncError, secret};
use std::path::PathBuf;

#[derive(Subcommand)]
pub(crate) enum MailCommand {
    /// An account's folders, as last listed (`sioul sync` lists them again).
    Folders { account: String },
    /// A folder's messages, newest first: date, sender, subject, flags and file.
    List {
        account: String,
        /// The folder, by its name as listed; the inbox by default.
        folder: Option<String>,
    },
    /// Does something to a message, on the server then here.
    Act {
        /// The message's file, as `list` shows it.
        file: PathBuf,
        #[arg(value_enum)]
        action: ActionArg,
        /// For `move`: the folder, by its name as listed.
        #[arg(long)]
        to: Option<String>,
    },
    /// The lists of the mail you called spam that offer one click from a
    /// verified sender, and how many others do not and why; nothing is contacted.
    LeaveSpam {
        /// Leaves them, one POST per list, one after the other.
        #[arg(long)]
        go: bool,
    },
    /// Writes and sends a message; the body is Markdown.
    Send {
        #[arg(long)]
        account: String,
        #[arg(long)]
        to: Vec<String>,
        #[arg(long)]
        cc: Vec<String>,
        #[arg(long)]
        subject: Option<String>,
        /// The text, in Markdown.
        #[arg(long)]
        body: String,
        #[arg(long)]
        attach: Vec<PathBuf>,
        /// Answers this message (its file), quoting it below the text.
        #[arg(long)]
        reply: Option<PathBuf>,
        /// Answers this message to everyone it went to.
        #[arg(long)]
        reply_all: Option<PathBuf>,
        /// Forwards this message, with its attachments.
        #[arg(long)]
        forward: Option<PathBuf>,
        /// Signs it with your OpenPGP key.
        #[arg(long)]
        sign: bool,
        /// Encrypts it to every recipient's key.
        #[arg(long)]
        encrypt: bool,
    },
}

#[derive(Clone, Copy, ValueEnum)]
pub(crate) enum ActionArg {
    Read,
    Unread,
    Flag,
    Unflag,
    Trash,
    Junk,
    Archive,
    NotJunk,
    /// Not spam: marked `$NotJunk`, it stays where it is (mail set aside as spam goes back to its lane).
    NotSpam,
    Move,
}

pub(crate) fn run(s: &Session, command: MailCommand) -> Result<(), String> {
    match command {
        MailCommand::Folders { account } => {
            for folder in mailbox::folders(&mail_account(s, &account)?.id) {
                // Folder names are the server's.
                println!("{} · {} · {}", s.tr.text(folder.role.message_id(), None), crate::one_line(&folder.display), crate::one_line(&folder.name));
            }
            Ok(())
        }
        MailCommand::List { account, folder } => list(s, &account, folder.as_deref()),
        MailCommand::Act { file, action, to } => act(s, &file, action, to),
        MailCommand::LeaveSpam { go } => {
            leave_spam(s, go);
            Ok(())
        }
        MailCommand::Send { account, to, cc, subject, body, attach, reply, reply_all, forward, sign, encrypt } => {
            let account = mail_account(s, &account)?.clone();
            let own = own_addresses(s);
            let answered = [(reply, DraftKind::Reply), (reply_all, DraftKind::ReplyAll), (forward, DraftKind::Forward)]
                .into_iter()
                .find_map(|(file, kind)| file.map(|f| (f, kind)));
            let mut draft = match answered {
                Some((file, kind)) => compose::answer(&file, kind, &account.id, &own).ok_or_else(|| s.tr.text("mail-message-gone", None))?,
                None => Draft::new(&account.id),
            };
            draft.to.extend(to.iter().flat_map(|t| compose::split_addresses(t)));
            draft.cc.extend(cc.iter().flat_map(|c| compose::split_addresses(c)));
            if let Some(subject) = subject {
                draft.subject = subject;
            }
            draft.body = body;
            draft.add_signature(account.signature.as_deref());
            draft.attachments = attach;
            draft.sign = sign;
            draft.encrypt = encrypt;
            send_draft(s, &account, &draft)
        }
    }
}

fn mail_account<'a>(s: &'a Session, id: &str) -> Result<&'a Account, String> {
    s.config.account(id).filter(|a| a.syncs()).ok_or_else(|| s.say("account-unknown", &[("id", id.to_string())]))
}

/// Your addresses, never answered to.
pub(crate) fn own_addresses(s: &Session) -> Vec<String> {
    s.config.accounts.iter().filter_map(|a| a.address.clone()).collect()
}

fn list(s: &Session, account: &str, folder: Option<&str>) -> Result<(), String> {
    let account = mail_account(s, account)?;
    let folders = mailbox::folders(&account.id);
    let missing = |name: &str| s.say("mail-no-such-folder", &[("folder", name.to_string()), ("account", account.id.clone())]);
    let chosen = match folder {
        Some(name) => folders.iter().find(|f| f.name == name || f.display == name).ok_or_else(|| missing(name))?,
        None => folders.iter().find(|f| f.local.is_empty()).ok_or_else(|| missing("INBOX"))?,
    };
    let mut cards = maildir::read_messages(&account.maildir_path().join(&chosen.local));
    cards.reverse();
    for card in cards {
        let path = card.path.clone().unwrap_or_default();
        let letters = maildir::flags_of(&path);
        // The flags' letters, then the keywords kept, by their names: "S $NotJunk".
        let keywords = maildir::KEYWORDS.iter().filter(|(letter, _)| letters.contains(*letter)).map(|(_, name)| name.to_string());
        let flags = std::iter::once(letters.chars().filter(char::is_ascii_uppercase).collect::<String>()).chain(keywords).filter(|f| !f.is_empty()).collect::<Vec<_>>().join(" ");
        // Sender and subject as the mail wrote them, never a terminal's escape sequences.
        println!("{} · {} · {} · [{flags}] · {}", crate::date(s, card.date), crate::one_line(card.sender()), crate::one_line(&card.subject), crate::one_line(&path.display().to_string()));
    }
    Ok(())
}

fn act(s: &Session, file: &std::path::Path, action: ActionArg, to: Option<String>) -> Result<(), String> {
    let account = s.config.account_of(file).ok_or_else(|| s.say("mail-not-in-accounts", &[("file", file.display().to_string())]))?;
    let action = match action {
        ActionArg::Read => Action::Read(true),
        ActionArg::Unread => Action::Read(false),
        ActionArg::Flag => Action::Flag(true),
        ActionArg::Unflag => Action::Flag(false),
        ActionArg::Trash => Action::Trash,
        ActionArg::Junk => Action::Junk,
        ActionArg::Archive => Action::Archive,
        ActionArg::NotJunk => Action::NotJunk,
        ActionArg::NotSpam => Action::NotSpam,
        ActionArg::Move => Action::Move(to.ok_or_else(|| s.tr.text("mail-move-needs-to", None))?),
    };
    let password = secret::password(account).map_err(|e| e.sentence(&s.tr, &account.id))?;
    mailbox::act(account, &password, file, &action).map_err(|e| e.sentence(&s.tr, &account.id))?;
    println!("{}", s.tr.text("mail-done", None));
    Ok(())
}

/// The lists of the mail you called spam (docs/client.md, "Unsubscribing"):
/// each with its sender's domain, and with `go` what became of it, a list
/// after another; then what was not to leave. Never reached by an agent.
fn leave_spam(s: &Session, go: bool) {
    let path = unsubscribe::Record::default_path();
    let found = sioul_sync::unsubscribe::spam_lists_here(&s.config, &labels::read_all(), &unsubscribe::Record::load(&path), None);
    for line in leave_spam_lines(s, &found, go.then_some(&path), &|list, record| sioul_sync::unsubscribe::leave(list, record)) {
        println!("{line}");
    }
}

/// What `leave_spam` prints: `leave` runs only with a record to write to.
fn leave_spam_lines(s: &Session, found: &unsubscribe::SpamLists, go: Option<&std::path::PathBuf>, leave: &dyn Fn(&unsubscribe::SpamList, &std::path::Path) -> Result<sioul_sync::unsubscribe::Outcome, SyncError>) -> Vec<String> {
    let mut lines = Vec::new();
    for list in &found.lists {
        // The name, the domain and the outcome came from a message: never a terminal's escape sequences.
        let named = format!("{} · {}", crate::one_line(&list.list.name), crate::one_line(list.domain()));
        lines.push(match go {
            Some(record) => format!("{named} · {}", crate::one_line(&sioul_sync::unsubscribe::sentence(&list.list.name, &leave(list, record), &s.tr))),
            None => named,
        });
    }
    if found.lists.is_empty() {
        lines.push(s.tr.text("leave-spam-none", None));
    } else if go.is_none() {
        lines.push(s.tr.text("leave-spam-dry", None));
    }
    lines.extend(found.not.lines(&s.tr));
    lines
}

/// Sends a draft, and forgets it once sent; kept when sending failed.
pub(crate) fn send_draft(s: &Session, account: &Account, draft: &Draft) -> Result<(), String> {
    let password = secret::password(account).map_err(|e| e.sentence(&s.tr, &account.id))?;
    match sioul_sync::send::send_draft(&s.config_path, account, &password, draft, &s.tr) {
        Ok(()) => {
            draft.discard();
            println!("{}", s.tr.text("mail-sent", None));
            Ok(())
        }
        Err(e @ SyncError::NotFiled(_)) => {
            draft.discard();
            println!("{}", crate::plain_lines(&e.sentence(&s.tr, &account.id)));
            Ok(())
        }
        // Not to everyone: the draft stays for those missed alone, and the exit says it failed.
        Err(e @ SyncError::PartlySent { .. }) => {
            if let SyncError::PartlySent { missed, .. } = &e {
                let mut kept = draft.clone();
                kept.to_those_missed(missed);
                let _ = kept.save();
            }
            Err(e.sentence(&s.tr, &account.id))
        }
        Err(e) => {
            let _ = draft.save();
            Err(e.sentence(&s.tr, &account.id))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sioul_core::config::Config;
    use sioul_core::i18n::Translator;
    use sioul_sync::unsubscribe::Outcome;
    use std::cell::RefCell;
    use std::path::{Path, PathBuf};

    fn a_list(name: &str, domain: &str) -> unsubscribe::SpamList {
        unsubscribe::SpamList { list: unsubscribe::List { key: format!("list-id:{domain}"), name: name.into() }, url: format!("https://{domain}/u"), address: format!("news@{domain}"), message_id: format!("m@{domain}") }
    }

    /// Without `--go` nothing is contacted; with it, one POST per list, each outcome on its own line.
    #[test]
    fn it_lists_and_only_go_contacts() {
        let s = Session { config: Config::default(), config_path: PathBuf::new(), tr: Translator::new("en") };
        let found = unsubscribe::SpamLists { lists: vec![a_list("Type\x1b[31m & Pixels", "letters.example"), a_list("Old News", "old.example")], not: unsubscribe::NotLeft { not_one_click: 2, gone: 1, ..Default::default() } };
        let posted = RefCell::new(Vec::new());
        let leave = |list: &unsubscribe::SpamList, _: &Path| {
            posted.borrow_mut().push(list.url.clone());
            Ok(if list.url.contains("old") { Outcome::Refused(404) } else { Outcome::Done })
        };
        let lines = leave_spam_lines(&s, &found, None, &leave);
        assert!(posted.borrow().is_empty(), "nothing contacted");
        assert_eq!(lines[1], "Old News · old.example");
        assert!(!lines[0].contains('\x1b') && lines[0].ends_with(" · letters.example"), "{lines:?}");
        assert_eq!(&lines[2..], ["Nothing was contacted. `sioul mail leave-spam --go` leaves these lists.", "2 lists offer no one click.", "1 message you called spam is no longer on this device."]);
        let record = PathBuf::from("/stand-in/unsubscribed.toml");
        let lines = leave_spam_lines(&s, &found, Some(&record), &leave);
        assert_eq!(*posted.borrow(), ["https://letters.example/u", "https://old.example/u"]);
        assert!(lines[0].contains("· letters.example · Unsubscribed from") && !lines[0].contains('\x1b'), "{lines:?}");
        assert_eq!(lines[1], "Old News · old.example · Not unsubscribed from Old News: its server answered 404.");
        assert_eq!(&lines[2..], ["2 lists offer no one click.", "1 message you called spam is no longer on this device."]);
        // Nothing to leave.
        let lines = leave_spam_lines(&s, &unsubscribe::SpamLists::default(), Some(&record), &leave);
        assert_eq!(lines, ["No list to leave among the mail you called spam."]);
    }
}
