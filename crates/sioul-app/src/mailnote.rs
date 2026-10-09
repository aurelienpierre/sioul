// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

//! New mail told as a notification, at the times it may come (docs/porch.md,
//! "Notifications"; the rules are `sioul_core::mailnote`). After each fetch
//! (`arrived`), the inbox's arrivals are judged as the Porch judges them and
//! gathered for ten seconds (a minute at most), every account together:
//! then one quiet notification for the batch. What may not come now waits,
//! and comes in one notification when a time begins in which it may
//! (`tick`, each minute): "The Porch opens: …".
//!
//! Only the device you used last tells (the "notices" lease, as for the
//! sites' gathered notification): your phone and your computer fetch the
//! same mail, and the sharing does not carry what each one told. The other
//! marks it told without a word.
//!
//! Without the window too: on a phone, a service of its own fetches the
//! inboxes in another process, without Qt, and calls `arrived` and `tick`
//! here (docs/android.md); the ledger is written under a lock, and a letter
//! is claimed before it is told, so that two processes never tell it twice.

use crate::backend::{QtThread, load_config, tr};
use jiff::Zoned;
use sioul_core::areas::Area;
use sioul_core::attention::{Attention, Level, Now, Output};
use sioul_core::config::Config;
use sioul_core::everywhere::People;
use sioul_core::mailnote::{self, Fate, Ledger, Letter};
use sioul_core::porch::{self, Triaged};
use std::path::PathBuf;
use std::sync::Mutex;
use std::time::{Duration, Instant};

/// Arrivals this close make one batch: the accounts fetched as Sioul starts.
const GATHERED: Duration = Duration::from_secs(10);
/// A batch waits this long at most for others to join it.
const AT_MOST: Duration = Duration::from_secs(60);

/// Sioul's window, when it runs in this process: you at it (for the lease)
/// and "Open" on a computer's notification.
#[derive(Clone)]
pub(crate) struct Window {
    /// "Open" on a computer's notification; a phone's opens through Java.
    #[cfg_attr(target_os = "android", allow(dead_code))]
    pub(crate) qt: QtThread,
    /// When you were last at it (Unix seconds).
    pub(crate) active: i64,
}

/// The letters waiting for their batch to close, in this process.
struct Batch {
    letters: Vec<Letter>,
    first: Option<Instant>,
    last: Option<Instant>,
    window: Option<Window>,
    /// Some waited for their time: "The Porch opens", for the batch whole.
    opens: bool,
    /// A thread waits to tell them.
    waiting: bool,
}

static BATCH: Mutex<Batch> = Mutex::new(Batch { letters: Vec::new(), first: None, last: None, window: None, opens: false, waiting: false });

/// Letters added to this process's batch (`opens`: they waited for their
/// time), told ten seconds after the last added, a minute at most after the first.
fn add_to_batch(letters: Vec<Letter>, window: Option<Window>, opens: bool) {
    let start = {
        let Ok(mut batch) = BATCH.lock() else { return };
        batch.letters.extend(letters);
        batch.first.get_or_insert_with(Instant::now);
        batch.last = Some(Instant::now());
        batch.opens |= opens;
        if window.is_some() {
            batch.window = window;
        }
        !std::mem::replace(&mut batch.waiting, true)
    };
    if start {
        std::thread::spawn(close_batch);
    }
}

/// The moment last seen by `tick`, in this process: a change is a time beginning.
static MOMENT: Mutex<Option<String>> = Mutex::new(None);

/// What decides now whether a message is told: the matrix of what reaches
/// you (`attention`), the moment as it reads it (the time, a slot of time for
/// you, do-not-disturb, Free time's "Nothing at all", the Porch resting
/// after a pause), the Always through list, and whether new mail is told at all.
struct Seen {
    config: Config,
    /// Read when a message is judged, not for the minute's moment: the
    /// contacts' categories are looked at for it.
    senders: std::cell::OnceCell<porch::Senders>,
    attention: Attention,
    now: Now,
    /// Always through: the people on that list, by their address.
    always: People,
    /// Nothing is told with the setting off, nor while the Porch rests after a pause.
    may: bool,
}

impl Seen {
    fn now(config: Config, now: &Zoned) -> Seen {
        let mode = crate::hours::mode_at(now);
        let moment = crate::hours::attention_at(&mode, now);
        let may = config.reminders.mail && !moment.holds.porch_rests;
        Seen { attention: Attention::of(&config), always: People::load(&People::default_path()), config, senders: std::cell::OnceCell::new(), now: moment, may }
    }

    fn senders(&self) -> &porch::Senders {
        self.senders.get_or_init(|| porch::Senders::load(&self.config))
    }

    /// What the matrix says of this message now (`Attention::mail`): its
    /// sender's row as the Porch weighs it, Always through, what the address
    /// it came to is for (unsaid: work's).
    fn decided(&self, t: &Triaged) -> Output {
        let area = t.card.account.as_deref().and_then(|id| self.config.account(id)).and_then(|a| a.area.as_deref()).and_then(Area::parse).unwrap_or(Area::WORK);
        self.attention.mail(t, self.senders(), &self.always, area, &self.now)
    }

    /// Told now, through the system's do-not-disturb, later, or never.
    fn fate(&self, t: &Triaged) -> Fate {
        let out = self.decided(t);
        match out {
            _ if !self.may => Fate::Later,
            out if out.told && out.pierce => Fate::Through,
            out if out.told => Fate::Now,
            out if out.level == Level::Never => Fate::Never,
            _ => Fate::Later,
        }
    }

    /// Whether this message may be told now.
    fn tells(&self, t: &Triaged) -> bool {
        self.may && self.decided(t).told
    }

    fn moment(&self) -> String {
        mailnote::moment(&self.now, self.may)
    }

    /// Messages judged as the Porch judges them: with the projects and their
    /// ties, the senders you let in, who is safe or blocked.
    fn judge(&self, files: &[PathBuf], stamp: i64) -> Vec<Triaged> {
        let (store, known) = self.world();
        porch::judge(files, &self.config.mail_sources(), store.as_ref(), &known, self.senders(), stamp)
    }

    /// The Porch as it is: every message not closed on yet.
    fn porch(&self, stamp: i64) -> Vec<Triaged> {
        let (store, known) = self.world();
        let state = sioul_core::state::PorchState::load(&sioul_core::state::PorchState::default_path());
        porch::gather(&self.config.mail_sources(), store.as_ref(), &known, self.senders(), &state, stamp)
    }

    fn world(&self) -> (Option<sioul_core::projects::ProjectStore>, porch::SenderList) {
        let ties = sioul_core::links::LocalLinks::load(&sioul_core::links::LocalLinks::default_path());
        let store = self.config.notes_root_path().and_then(|root| sioul_core::projects::ProjectStore::load(&root).ok()).map(|s| s.with_ties(&ties));
        (store, porch::KnownSenders::load(&self.config.known_senders_path()))
    }
}

/// After a fetch of an account (`new`: its inbox's arrivals; nothing on its
/// first fetch, which brings two weeks of mail): from a process without the
/// window (a phone's service).
#[cfg_attr(not(target_os = "android"), allow(dead_code))]
pub(crate) fn arrived(account: &str, new: &[PathBuf], first: bool) {
    arrived_from(account, new, first, None);
}

/// The same, from the window's watchers.
pub(crate) fn arrived_in_window(window: Window, account: &str, new: &[PathBuf], first: bool) {
    arrived_from(account, new, first, Some(window));
}

/// What your mail filters could not act on (a folder missing, the server
/// refusing), once this device's filters ran: told as any new mail is, by the
/// same rules, the arrivals having been held back for them
/// (`filters::untold`). Told once only: the ledger knows what was.
#[cfg_attr(not(target_os = "android"), allow(dead_code))]
pub(crate) fn unfiltered(account: &str, files: &[PathBuf]) {
    told(account, files, None, false);
}

/// The same, from the window's watchers.
pub(crate) fn unfiltered_in_window(window: Window, account: &str, files: &[PathBuf]) {
    told(account, files, Some(window), false);
}

fn arrived_from(account: &str, new: &[PathBuf], first: bool, window: Option<Window>) {
    if !first {
        told(account, new, window, true);
    }
}

/// `files` judged and told by the rules of what reaches you; `hold`: what a
/// mail filter would move out of the inbox or mark read held back, for this
/// device's filters to act on first (`unfiltered` tells it if they cannot).
fn told(_account: &str, files: &[PathBuf], window: Option<Window>, hold: bool) {
    if files.is_empty() {
        return;
    }
    let config = load_config();
    if !config.reminders.mail {
        return;
    }
    let now = Zoned::now();
    let stamp = now.timestamp().as_second();
    let newsletters = config.reminders.mail_newsletters;
    let seen = Seen::now(config, &now);
    let judged = seen.judge(files, stamp);
    // What your mail filters would move out of the inbox or mark read waits for them: told after, if they could not.
    let arrivals = if hold { crate::filters::untold(&seen.config, judged, seen.senders()) } else { judged };
    let path = Ledger::default_path();
    // Marked at once: those never told (its sender's row says "not at all"
    // now: on the Porch only), those waiting for their time, and those of the
    // batch, waiting until it is told (Sioul stopped before, they come with
    // "The Porch opens").
    let letters = sioul_core::filelock::with_lock(&path, || {
        let mut ledger = Ledger::load(&path);
        let sorted = mailnote::sort(&arrivals, &ledger, newsletters, |t| seen.fate(t));
        for key in &sorted.never {
            ledger.tell(key, stamp);
        }
        for key in sorted.later.iter().chain(sorted.now.iter().map(|l| &l.key)) {
            ledger.wait(key, stamp);
        }
        ledger.forget_old(stamp);
        if let Err(e) = ledger.save(&path) {
            eprintln!("{e}");
        }
        sorted.now
    });
    if !letters.is_empty() {
        add_to_batch(letters, window, false);
    }
}

/// Every account fetched at once, in a process without the window (a phone's
/// background step, `steps::fetch_mail`): every arrival is known, so the batch
/// is told now, in one notification, not ten seconds on, which kept the phone
/// awake for nothing. The thread waiting for it finds it empty, and ends.
#[cfg_attr(not(target_os = "android"), allow(dead_code))]
pub(crate) fn close_now() {
    let (letters, window, opens) = {
        let Ok(mut batch) = BATCH.lock() else { return };
        batch.first = None;
        batch.last = None;
        (std::mem::take(&mut batch.letters), batch.window.take(), std::mem::take(&mut batch.opens))
    };
    if !letters.is_empty() {
        tell_batch(letters, window, opens);
    }
}

/// Waits for the batch to close (ten seconds without another arrival, a
/// minute at most), then tells it.
fn close_batch() {
    loop {
        std::thread::sleep(Duration::from_secs(1));
        let ready = {
            let Ok(mut batch) = BATCH.lock() else { return };
            let done = batch.last.is_none_or(|last| last.elapsed() >= GATHERED) || batch.first.is_some_and(|first| first.elapsed() >= AT_MOST);
            done.then(|| {
                batch.waiting = false;
                batch.first = None;
                batch.last = None;
                (std::mem::take(&mut batch.letters), batch.window.take(), std::mem::take(&mut batch.opens))
            })
        };
        if let Some((letters, window, opens)) = ready {
            // Told already (`close_now`): nothing left.
            if !letters.is_empty() {
                tell_batch(letters, window, opens);
            }
            return;
        }
    }
}

/// A batch told, its letters claimed first: those still waiting (told by
/// nobody else meanwhile). Nothing may be told now (a pause pressed in the
/// meantime): they keep waiting for their time. Some waited for it
/// (`opens`): "The Porch opens: …", for them all.
fn tell_batch(letters: Vec<Letter>, window: Option<Window>, opens: bool) {
    let now = Zoned::now();
    let stamp = now.timestamp().as_second();
    if !Seen::now(load_config(), &now).may {
        return;
    }
    let mine = claim(letters, &[], stamp);
    if mine.is_empty() || !ours(window.as_ref()) {
        return;
    }
    let (title, body) = if opens { mailnote::opens(tr(), &mine) } else { mailnote::batch(tr(), &mine) };
    show(&title, &body, mine.iter().any(|l| l.through), window.as_ref());
}

/// Each minute: when the moment changed (a time began, waking, a pause or a
/// do-not-disturb over, Sioul starting), the mail that waited and may come
/// now, told in one notification. From a process without the window.
#[cfg_attr(not(target_os = "android"), allow(dead_code))]
pub(crate) fn tick() {
    tick_from(None);
}

/// The same, from the window's minute: off its thread.
pub(crate) fn tick_in_window(window: Window) {
    std::thread::spawn(move || tick_from(Some(window)));
}

fn tick_from(window: Option<Window>) {
    static BUSY: Mutex<()> = Mutex::new(());
    let Some(_busy) = crate::backend::one_at_a_time(&BUSY) else { return };
    let config = load_config();
    if !config.reminders.mail {
        return;
    }
    let now = Zoned::now();
    let stamp = now.timestamp().as_second();
    let newsletters = config.reminders.mail_newsletters;
    let seen = Seen::now(config, &now);
    let moment = seen.moment();
    let changed = MOMENT.lock().is_ok_and(|mut last| last.replace(moment.clone()).as_ref() != Some(&moment));
    if !changed || !seen.may {
        return;
    }
    let path = Ledger::default_path();
    let ledger = Ledger::load(&path);
    if ledger.waiting.is_empty() {
        return;
    }
    let items = seen.porch(stamp);
    let letters = mailnote::waited(&items, &ledger, newsletters, |t| seen.tells(t));
    // Read meanwhile, blocked, set aside: waiting no more.
    let gone = mailnote::gone_by(&items, &ledger, newsletters);
    // With the window, gathered with what its fetches bring meanwhile (Sioul
    // starting): one notification. Without it, told now: the process may end.
    if window.is_some() {
        claim(Vec::new(), &gone, stamp);
        if !letters.is_empty() {
            add_to_batch(letters, window, true);
        }
        return;
    }
    let mine = claim(letters, &gone, stamp);
    if mine.is_empty() || !ours(window.as_ref()) {
        return;
    }
    let (title, body) = mailnote::opens(tr(), &mine);
    show(&title, &body, mine.iter().any(|l| l.through), window.as_ref());
}

/// The letters still waiting in the ledger, marked told by this process
/// (the others are another's to tell, or told); `gone` marked told too.
fn claim(letters: Vec<Letter>, gone: &[String], stamp: i64) -> Vec<Letter> {
    let path = Ledger::default_path();
    sioul_core::filelock::with_lock(&path, || {
        let mut ledger = Ledger::load(&path);
        let mut seen = std::collections::BTreeSet::new();
        let mine: Vec<Letter> = letters.into_iter().filter(|l| ledger.waiting.contains_key(&l.key) && seen.insert(l.key.clone())).collect();
        for key in mine.iter().map(|l| &l.key).chain(gone) {
            ledger.tell(key, stamp);
        }
        if mine.is_empty() && gone.is_empty() {
            return mine;
        }
        match ledger.save(&path) {
            Ok(()) => mine,
            // Not written: told by none, rather than twice.
            Err(e) => {
                eprintln!("{e}");
                Vec::new()
            }
        }
    })
}

/// Whether this device tells: the one you used last (the "notices" lease,
/// renewed by the window; only read without it). Sharing off: this one.
fn ours(window: Option<&Window>) -> bool {
    match window {
        Some(w) => crate::share::keeper("notices", sioul_sync::lease::Rule::FollowsYou, w.active, false).0.mine,
        None => crate::share::looked("notices", sioul_sync::lease::Rule::FollowsYou).mine,
    }
}

/// The notification: the desktop's, with "Open" (the Porch) where it takes
/// buttons, at critical urgency when someone Always through is in it
/// (`through`: it passes the desktop's do-not-disturb); on a phone,
/// Android's, in the quiet "New mail" channel, a tap opening the Porch.
fn show(title: &str, body: &str, through: bool, window: Option<&Window>) {
    #[cfg(target_os = "android")]
    {
        let _ = window;
        crate::eventalarms::mail_note(title, body, through);
    }
    #[cfg(not(target_os = "android"))]
    {
        use cxx_qt_lib::QString;
        let open = window.map(|w| {
            let qt = w.qt.clone();
            let action: Box<dyn FnOnce() + Send> = Box::new(move || {
                let _ = qt.queue(|mut sioul| sioul.as_mut().reminder_opened(QString::from("porch"), QString::default(), QString::default()));
            });
            (tr().text("reminder-open", None), action)
        });
        if let Err(e) = sioul_sync::notify::mail(title, body, through, open) {
            eprintln!("{e}");
        }
    }
}
