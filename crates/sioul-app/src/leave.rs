// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

//! Leaving the lists of the mail you called spam, from Mail ▸ ⚙ ▸ Lists you
//! left (docs/client.md, "Unsubscribing"; `LeaveSpam.qml`): the rule is
//! `unsubscribe::spam_lists`, what it reads `sioul_sync::unsubscribe`. Found
//! off the window's thread; "Leave these lists" waits its ten seconds with
//! Undo, as one work (`mail::schedule_leave`), and asks the rule again then,
//! since your word may have changed meanwhile, here or on another device.

use crate::backend::{QtThread, Shared, load_config, say, tell, tr};
use cxx_qt_lib::QString;
use sioul_core::unsubscribe::{Record, SpamList};
use sioul_sync::SyncError;
use sioul_sync::unsubscribe::Outcome;
use std::collections::BTreeMap;
use std::path::Path;
use std::sync::{Arc, Mutex};

/// One list, as the sheet names it.
#[derive(serde::Serialize)]
struct Named {
    key: String,
    name: String,
    domain: String,
    /// Why the last try did not leave it, with the Reader's own sentence; else "".
    why: String,
}

/// What the row and its sheet show.
#[derive(serde::Serialize)]
struct Found {
    count: usize,
    row: String,
    button: String,
    lists: Vec<Named>,
    /// A run just ended: the settings read again the lists you left.
    ended: bool,
}

/// Why a list was not left at the last try, by its key, for the sheet.
static LAST_TRY: Mutex<BTreeMap<String, String>> = Mutex::new(BTreeMap::new());

/// The lists to leave as they stand now, brought by `leave_spam_found`.
pub(crate) fn look(qt: &QtThread, shared: &Arc<Shared>, ended: bool) {
    let (qt, shared) = (qt.clone(), Arc::clone(shared));
    std::thread::spawn(move || {
        let said = crate::mail::said_now(&shared);
        let found = sioul_sync::unsubscribe::spam_lists_here(&load_config(), &said, &Record::load(&Record::default_path()), None);
        let count = found.lists.len();
        let last = LAST_TRY.lock().map(|last| last.clone()).unwrap_or_default();
        let lists = found.lists.iter().map(|l| Named { key: l.list.key.clone(), name: l.list.name.clone(), domain: l.domain().to_string(), why: last.get(&l.list.key).cloned().unwrap_or_default() }).collect();
        let n = [("n", count.to_string())];
        let shown = Found { count, row: say("leave-spam-row", &n), button: say("leave-spam-go", &n), lists, ended };
        let json = serde_json::to_string(&shown).unwrap_or_default();
        let _ = qt.queue(move |mut sioul| sioul.as_mut().leave_spam_found(QString::from(&json)));
    });
}

/// "Leave these lists": the lists named by `keys` (JSON), after ten seconds to undo. A demo sends nothing.
pub(crate) fn run(qt: &QtThread, shared: &Arc<Shared>, keys: &str) {
    let keys: Vec<String> = serde_json::from_str(keys).unwrap_or_default();
    if keys.is_empty() {
        return;
    }
    if crate::backend::offline() {
        return tell(qt, shared, tr().text("unsubscribe-demo", None));
    }
    let line = say("leave-spam-pending", &[("n", keys.len().to_string())]);
    crate::mail::schedule_leave(qt, shared, keys, line);
}

/// The run, its ten seconds gone: those of `keys` that still meet the rule,
/// one after the other; what the status line says of it all.
pub(crate) fn perform(keys: &[String], shared: &Shared) -> String {
    let record = Record::default_path();
    let found = sioul_sync::unsubscribe::spam_lists_here(&load_config(), &crate::mail::said_now(shared), &Record::load(&record), None);
    let chosen: Vec<&SpamList> = found.lists.iter().filter(|l| keys.contains(&l.list.key)).collect();
    leave_each(&chosen, &record, &sioul_sync::unsubscribe::leave)
}

/// Each list in turn, whatever became of the one before; each one's outcome
/// kept for the sheet, the line said of them all.
fn leave_each(lists: &[&SpamList], record: &Path, leave: &dyn Fn(&SpamList, &Path) -> Result<Outcome, SyncError>) -> String {
    let (mut left, mut silent, mut refused) = (0, 0, 0);
    for list in lists {
        let outcome = leave(list, record);
        match &outcome {
            Ok(Outcome::Done) => left += 1,
            Ok(Outcome::Refused(_) | Outcome::Elsewhere(_)) => refused += 1,
            Ok(Outcome::TimedOut) | Err(_) => silent += 1,
        }
        if let Ok(mut last) = LAST_TRY.lock() {
            if matches!(outcome, Ok(Outcome::Done)) {
                last.remove(&list.list.key);
            } else {
                last.insert(list.list.key.clone(), sioul_sync::unsubscribe::sentence(&list.list.name, &outcome, tr()));
            }
        }
    }
    say("leave-spam-ended", &[("left", left.to_string()), ("silent", silent.to_string()), ("refused", refused.to_string())])
}

#[cfg(test)]
mod tests {
    use super::*;
    use sioul_core::unsubscribe::List;
    use std::cell::RefCell;

    fn list(name: &str) -> SpamList {
        SpamList { list: List { key: format!("list-id:{name}.example"), name: name.into() }, url: format!("https://{name}.example/u"), address: format!("news@{name}.example"), message_id: format!("m@{name}.example") }
    }

    fn ended(left: i32, silent: i32, refused: i32, tr: &sioul_core::i18n::Translator) -> String {
        let mut args = sioul_core::i18n::args();
        args.set("left", left);
        args.set("silent", silent);
        args.set("refused", refused);
        tr.text("leave-spam-ended", Some(&args))
    }

    /// Each list in turn, one line for the end; a failure keeps its sentence
    /// for the sheet until a later try takes it; each language counts as it should.
    #[test]
    fn one_line_for_the_end_of_a_run() {
        let (done, gone, moved, late, down) = (list("done"), list("gone"), list("moved"), list("late"), list("down"));
        let asked = RefCell::new(Vec::new());
        let leave = |l: &SpamList, _: &Path| {
            asked.borrow_mut().push(l.list.name.clone());
            match l.list.name.as_str() {
                "done" => Ok(Outcome::Done),
                "gone" => Ok(Outcome::Refused(404)),
                "moved" => Ok(Outcome::Elsewhere("tracker.example".into())),
                "late" => Ok(Outcome::TimedOut),
                _ => Err(SyncError::Network("no route".into())),
            }
        };
        let line = leave_each(&[&done, &gone, &moved, &late, &down], Path::new("/stand-in/unsubscribed.toml"), &leave);
        assert_eq!(*asked.borrow(), ["done", "gone", "moved", "late", "down"], "one after the other, a failure stopping nothing");
        assert_eq!(line, ended(1, 2, 2, tr()));
        let last = LAST_TRY.lock().unwrap().clone();
        assert!(!last.contains_key("list-id:done.example"));
        assert!(last["list-id:gone.example"].contains("404") && last["list-id:moved.example"].contains("tracker.example"), "{last:?}");
        // Taken at a later try: its sentence goes.
        let line = leave_each(&[&gone], Path::new("/stand-in/unsubscribed.toml"), &|_, _| Ok(Outcome::Done));
        assert_eq!(line, ended(1, 0, 0, tr()));
        assert!(!LAST_TRY.lock().unwrap().contains_key("list-id:gone.example"));
        let en = sioul_core::i18n::Translator::new("en");
        assert_eq!(ended(1, 2, 2, &en), "Left 1 list; 2 did not answer; 2 refused.");
        assert_eq!(ended(1, 0, 0, &en), "Left 1 list.");
        assert_eq!(ended(2, 1, 0, &en), "Left 2 lists; 1 did not answer.");
        assert_eq!(ended(0, 0, 3, &en), "No list left; 3 refused.");
        let fr = sioul_core::i18n::Translator::new("fr");
        assert_eq!(ended(2, 1, 0, &fr), "2 listes quittées\u{202f}; 1 n’a pas répondu.");
        assert_eq!(ended(1, 0, 2, &fr), "1 liste quittée\u{202f}; 2 ont refusé.");
        assert_eq!(ended(0, 2, 1, &fr), "Aucune liste quittée\u{202f}; 2 n’ont pas répondu\u{202f}; 1 a refusé.");
    }
}
