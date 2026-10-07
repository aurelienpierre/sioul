// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

//! The mail client from the terminal: `sioul mail folders | list | act | send`.
//! The window offers the same with "Undo" before anything reaches the server;
//! here, a command is already a deliberate act.

use crate::Session;
use clap::{Subcommand, ValueEnum};
use sioul_core::compose::{self, Draft, DraftKind};
use sioul_core::config::Account;
use sioul_core::maildir;
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
        Err(e) => {
            let _ = draft.save();
            Err(e.sentence(&s.tr, &account.id))
        }
    }
}
