// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

//! Other apps' notifications on a phone (docs/android.md, "Notifications from
//! other apps"). Android's listener (AppNotes.java, in a process of its own,
//! ":listener") asks here, for each notification of another app, whether it
//! comes now or how long it waits (`sioul_appnotes_decide`), and, when
//! something moved the times (a pause ended, the hours or the choices
//! changed), how long what it holds waits now (`sioul_appnotes_review`). The
//! rules are `sioul_core::appnotes`; what reaches you when, `sioul_core::attention`,
//! with today's slots of time for you from their file (`hours::slots`): Time
//! for you holds in this process too.
//!
//! Also here, for the window: Settings ▸ Other apps (`setup`, `change`), and
//! the Porch's line on what waits and what came back (`porch_line`). On a
//! computer, nothing of it runs: the listener is the phone's.
//!
//! And the channels of other apps Sioul makes silent itself, once Android
//! links it to one of your devices (`sioul_appnotes_channels`: what to make
//! silent and give back, `appnotes::channels_plan`; each one's importance
//! noted first, `appnotes::Silenced`).

use crate::backend::{load_config, tr};
use crate::everywhere::Gated;
use jiff::Zoned;
use serde_json::{Value, json};
use sioul_core::appnotes::{self, AppKind, Ask, Cause, Choices, Decision, Gate, Gathering, Kind, Ledger, Live, Own, Posted, Sender, Through, Why};
use sioul_core::areas::Area;
use sioul_core::attention::Column;
use sioul_core::config::Config;
use sioul_core::everywhere::People;
use sioul_core::porch::{By, Senders};
use sioul_core::reach::{Channel, Clock, Who};
use std::ffi::c_char;
use std::path::Path;
use std::sync::Mutex;
use std::time::SystemTime;

#[cfg(target_os = "android")]
// SAFETY: declared as android/main.cpp defines them: extern "C", the same types.
unsafe extern "C" {
    /// AppNotes.call (android/main.cpp): a verb and its JSON, a JSON answer or null.
    fn sioul_android_appnotes(verb: *const c_char, json: *const c_char) -> *mut c_char;
    /// An answer of `sioul_android_appnotes` given back.
    fn sioul_android_dnd_free(text: *mut c_char);
}

/// Java's answer to `verb` (AppNotes.call); null elsewhere, or without one.
fn java(verb: &str, json: &str) -> Value {
    #[cfg(target_os = "android")]
    {
        use std::ffi::{CStr, CString};
        let (Ok(verb), Ok(json)) = (CString::new(verb), CString::new(json)) else { return Value::Null };
        // SAFETY: two zero-terminated texts, valid for the call.
        let answer = unsafe { sioul_android_appnotes(verb.as_ptr(), json.as_ptr()) };
        if answer.is_null() {
            return Value::Null;
        }
        // SAFETY: a zero-terminated text from sioul_android_appnotes, read before it is given back.
        let text = unsafe { CStr::from_ptr(answer) }.to_string_lossy().to_string();
        // SAFETY: given back once, as main.cpp asks; not read after.
        unsafe { sioul_android_dnd_free(answer) };
        serde_json::from_str(&text).unwrap_or(Value::Null)
    }
    #[cfg(not(target_os = "android"))]
    {
        let _ = (verb, json);
        Value::Null
    }
}

/// Whether Android lets Sioul hold other apps' notifications here
/// (notification access): on a phone, as Java says now; None elsewhere, or
/// when Java cannot say (its library not loaded in this process).
pub(crate) fn access_here() -> Option<bool> {
    java("access", "{}").as_bool()
}

/// When a file last changed; none when it is not there.
fn changed(path: &Path) -> Option<SystemTime> {
    std::fs::metadata(path).and_then(|m| m.modified()).ok()
}

/// The clock as it stands, kept a minute in the listener's process: read
/// again sooner when the pause or free time, Health, the hours or the
/// settings change. Asked at every notification.
fn live(config: &Config, now: &Zoned) -> Live {
    type Key = (i64, [Option<SystemTime>; 4]);
    static KEPT: Mutex<Option<(Key, Live)>> = Mutex::new(None);
    let files = [
        sioul_core::quiet::Overrides::default_path(),
        sioul_core::health::Health::default_path(),
        sioul_core::needs::Days::default_path(),
        sioul_core::config::config_dir().join("config.toml"),
    ];
    let key: Key = (now.timestamp().as_second() / 60, files.each_ref().map(|f| changed(f)));
    if let Ok(kept) = KEPT.lock()
        && let Some((known, live)) = kept.as_ref()
        && *known == key
    {
        return live.clone();
    }
    // On this phone: what an app is for is read against the phone's cells (`Clock::on`).
    let live = Live { clock: Clock::load(config, now).on(crate::dispatch::lines()) };
    if let Ok(mut kept) = KEPT.lock() {
        *kept = Some((key, live.clone()));
    }
    live
}

/// Who wrote, as your address books and lists say (`porch::Senders`): none
/// when Sioul cannot tell (nobody named; a name in no card, or one anyone can
/// take; a number or an address none of Sioul's lists knows while the
/// phone's address book could not be asked), which `appnotes::decide` judges
/// as a contact's, never a stranger's (held apps phase 1, A1); a stranger
/// only for a number or an address that neither Sioul's lists nor the
/// phone's address book, asked, knows. Whether their card is one of your
/// people's (`porch::Judged::circle`: by a number or an address, never by a
/// name, which anyone can take, an SMS's sender id included: review L1); and
/// whether the Always through list holds them (by a number or an address,
/// never a name).
fn who_of(sender: &Sender, senders: &Senders, always: &People, config: &Config) -> (Option<Who>, bool, bool) {
    match sender {
        Sender::Known { numbers, emails, contact, asked } => {
            let by_number = numbers.iter().map(|n| senders.judge_number(n)).find(|j| j.by != By::Default);
            let by_address = emails.iter().map(|e| senders.judge(e)).find(|j| j.who != Who::Stranger);
            let region = sioul_core::reach::region(config);
            let admitted = numbers.iter().any(|n| always.admits_number(n, region)) || emails.iter().any(|e| always.admits_address(e));
            // In the phone's address book though not in Sioul's: someone you know. In neither, the phone's
            // address book asked: a stranger. Not asked: nobody can tell.
            let unknown = if *contact {
                Some(Who::Neutral)
            } else if *asked {
                Some(Who::Stranger)
            } else {
                None
            };
            match by_number.or(by_address) {
                Some(judged) => (Some(judged.who), judged.circle, admitted),
                None => (unknown, false, admitted),
            }
        }
        // A name anyone can take may only hold more, and proves no one of your people (L1); any other
        // name says nothing of who wrote (A1).
        Sender::Named { name, book } => (appnotes::named(senders.judge_name(name).map(|judged| judged.who), *book), false, false),
        Sender::Unknown => (None, false, false),
    }
}

/// The why the tab says of a person's message from someone Sioul could not
/// tell, judged as a contact's (`appnotes::unidentified`; "appnotes-why-unknown").
const UNKNOWN: &str = "unknown";

/// Reachable's end, for the listener's question (`Ask::reachable`, Unix
/// seconds): while it lasts, everyone not blocked comes at once (part 3).
fn reachable_until() -> Option<i64> {
    crate::everywhere::reachable().map(|r| r.until.unwrap_or(i64::MAX))
}

/// The why of a text gathered because its number is one ARCEP keeps for
/// machines (`appnotes::why_text`, "appnotes-why-machine").
const MACHINE: &str = "machine";

/// Whether a sender's number comes from a root ARCEP keeps for automated
/// systems (`screening::Numbers`, the call screening's data), while the call
/// screening sends automated callers to voicemail (its `automated`).
fn machine_number(sender: &Sender, config: &Config, now: &Zoned) -> bool {
    let Sender::Known { numbers, .. } = sender else { return false };
    if sioul_core::screening::Screening::of(config).automated != sioul_core::screening::Spam::Voicemail {
        return false;
    }
    let roots = sioul_core::screening::shipped().automated(now.date());
    let region = sioul_core::reach::region(config);
    numbers.iter().any(|n| sioul_core::screening::national(&sioul_core::phones::key(n, region)).is_some_and(|national| sioul_core::screening::starts_with_any(&national, &roots)))
}

/// What a notification's source is for: the app's, as you set it; else, for
/// mail, the address it came to, when it is one of your accounts'
/// (`account_of`): the area it says, or, unsaid, work's hours, none of them
/// closed for your safe senders (`true`: `Talk::to_unsaid`), as Sioul's own
/// notice of the same message. An address none of your accounts holds: none.
fn area_of(posted: &Posted, kind: &Kind, choices: &Choices, config: &Config) -> (Option<Area>, bool) {
    if let Some(area) = choices.app(&posted.package).area {
        return (Some(area), false);
    }
    let Kind::People(talk) = kind else { return (None, false) };
    match account_of(&config.accounts, &talk.to) {
        Some(account) => match sioul_core::mailbox::said_area(account) {
            Some(area) => (Some(area), false),
            None => (Some(Area::WORK), true),
        },
        None => (None, false),
    }
}

/// Which of your accounts an address a mail app shows came to (review of
/// phase 2, M6): the account of that address or user name, a "+tag" left
/// aside (jane+news@work.example is jane@work.example); else the one
/// account of its domain, an alias delivered there (support@work.example);
/// none when the app shows no address, or several accounts share the domain.
fn account_of<'a>(accounts: &'a [sioul_core::config::Account], to: &str) -> Option<&'a sioul_core::config::Account> {
    let to = to.trim().to_lowercase();
    let (local, domain) = to.rsplit_once('@')?;
    let bare = format!("{}@{domain}", local.split_once('+').map_or(local, |(base, _)| base));
    let mail: Vec<&sioul_core::config::Account> = accounts.iter().filter(|a| a.address.is_some()).collect();
    let names = |a: &sioul_core::config::Account| [a.address.as_deref(), a.username.as_deref()].into_iter().flatten().map(|s| s.trim().to_lowercase()).collect::<Vec<_>>();
    if let Some(account) = mail.iter().find(|a| names(a).iter().any(|n| *n == to || *n == bare)) {
        return Some(account);
    }
    let mut same = mail.iter().filter(|a| names(a).iter().any(|n| n.rsplit_once('@').is_some_and(|(_, d)| d == domain)));
    match (same.next(), same.next()) {
        (Some(account), None) => Some(account),
        _ => None,
    }
}

/// Do-not-disturb's switch or a busy stretch, holding now: when it ends,
/// when that is known (a layer of the matrix), and which of them holds (for
/// the log).
fn gate() -> Option<Gated> {
    crate::everywhere::gated()
}

/// One notification decided (`AppNotes.decide`): {"hold": milliseconds (0:
/// it comes now), "why", "quiet", "people", "waited", "changed"}: `quiet`, held, its return comes without a
/// sound, in Sioul's quiet window (`appnotes::Held::quiet`: an automaton's,
/// held for good, a person's message; docs/android.md); `people`, held, a
/// person's message, planned on every Android version (held apps phase 1,
/// B2); `waited`, a person's message come back at its time, which Java tells
/// once (`appnotes::waited`); `changed`, held now or back from being
/// held, the line of what Sioul holds to write again (`shade`). What it says is read here and dropped; the ledger
/// keeps its key hashed, its app's and conversation's names, and its time.
/// Only for the apps you send to your computers, while that part of the
/// sharing is on, its messages also go, sealed (`phonemsgs::note`; "shared":
/// a line was written, Java asks for a step soon).
fn decide(json: &str) -> String {
    decide_at(json, &sioul_core::zone::now())
}

/// `decide` at `now` (the tests' fixed clock).
fn decide_at(json: &str, now: &Zoned) -> String {
    let Ok(posted) = serde_json::from_str::<Posted>(json) else { return "null".into() };
    let config = load_config();
    // What reaches you when (Settings ▸ What reaches you); people's messages worked out once from it (A2).
    let attention = sioul_core::attention::Attention::of(&config);
    let choices = settled_choices(&attention, true);
    let kind = appnotes::classify(&posted, &choices, &sioul_core::words::Words::of(&config));
    let now = now.clone();
    let stamp = now.timestamp().as_second();
    let gate = gate();
    let senders = Senders::load(&config);
    let (who, circle, admitted) = match &kind {
        Kind::People(talk) => who_of(&talk.sender, &senders, &People::load(&People::default_path()), &config),
        _ => (None, false, false),
    };
    // A code from someone you blocked gains nothing of Sioul's: no notice of its own through
    // do-not-disturb, no line for your computers (its sender read as the notification would be
    // without its code). Whether its app's own notification is held too is the owner's to decide.
    let blocked_code = matches!(kind, Kind::Code)
        && !posted.redacted
        && matches!(appnotes::classify(&posted, &choices, &sioul_core::words::Words::default()), Kind::People(talk) if who_of(&talk.sender, &senders, &People::load(&People::default_path()), &config).0 == Some(Who::Blocked));
    // The person's own rows it reads, after its conversation's and before its app's (decision 16):
    // by the card's UID, each number's key, each address; by name only as the phone's address book names.
    let persons = match &kind {
        Kind::People(talk) => match &talk.sender {
            Sender::Known { numbers, emails, .. } => senders.person_rows(numbers, emails),
            Sender::Named { name, book: true } => senders.person_rows_named(name),
            _ => Vec::new(),
        },
        _ => Vec::new(),
    };
    // A text from a number nobody knows, from a root ARCEP keeps for machines: an automaton's, gathered
    // with them, as the call screening sends their calls to voicemail (`screening`, unless set otherwise);
    // said so in its why and its line (review of part 2, F11).
    // Nobody knowing it here (a stranger, or the phone's address book not asked: A1), a number kept for machines is a machine's.
    let machine = matches!(&kind, Kind::People(talk) if talk.sms && matches!(who, None | Some(Who::Stranger)) && machine_number(&talk.sender, &config, &now));
    let mut kind = if machine { Kind::Automaton { site: String::new() } } else { kind };
    let (who, circle, admitted) = if matches!(kind, Kind::People(_)) { (who, circle, admitted) } else { (None, false, false) };
    // Sioul cannot tell who wrote: judged as a contact, and said so (A1).
    let unknown = appnotes::unidentified(&kind, who);
    let (area, unsaid) = area_of(&posted, &kind, &choices, &config);
    if let Kind::People(talk) = &mut kind {
        talk.to_unsaid = unsaid;
    }
    let live = live(&config, &now);
    let gathered = config.reminders.gathered_times();
    // Today's slots of time for you, as the window wrote them; where they begin and end.
    let slots = crate::hours::slots();
    let slot = |at: &Zoned| slots.at(at);
    let also = slots.edges();
    // Free time's "Nothing at all".
    let nothing = sioul_core::pause::nothing_now(&sioul_core::quiet::Overrides::load(&sioul_core::quiet::Overrides::default_path()), &config.free_time);
    // Its own rows in the matrix, when set: its conversation's, then its app's.
    let conversation = match &kind {
        Kind::People(talk) => talk.conversation.as_ref().map(|c| c.key.clone()).unwrap_or_default(),
        _ => String::new(),
    };
    let sources = sioul_core::attention::source_rows_with(&posted.package, &conversation, &persons);
    let reachable = reachable_until();
    // Android silences it now (Sioul's mode or yours): a text whose cell says ◑ comes, without a sound (decision 13).
    let silenced = posted.intercepted;
    let ask = Ask { now: &now, clock: &live, attention: &attention, gathered: &gathered, area, gate: gate.map(|g| Gate { until: g.until }), slot_at: &slot, nothing, also: &also, sources: &sources, reachable, silenced, circle };
    let hash = appnotes::key_hash(&posted.key);
    // A conversation you answered from a computer in the last twenty minutes, or a number
    // texted from one, by the SMS app: at once, but in a pause (`replies`).
    let paused = live.clock.mode(&now).paused();
    let answered = !paused
        && match &kind {
            Kind::People(talk) => {
                talk.conversation.as_ref().is_some_and(|c| crate::replies::answered_lately(&c.key))
                    || ((talk.sms || posted.sms_app) && matches!(&talk.sender, Sender::Known { numbers, .. } if crate::replies::texted_lately(&numbers.iter().map(|n| sioul_core::phones::key(n, sioul_core::reach::region(&config))).collect::<Vec<_>>())))
            }
            _ => false,
        };
    // A code of an earlier message still valid: the notification that shows it stays (second review of the codes, fix 2).
    let earlier_code = earlier_code_lasts(json, &posted, &sioul_core::words::Words::of(&config), stamp);
    let path = Ledger::default_path();
    let (decision, released, returning, quiet, people, waited) = sioul_core::filelock::with_lock(&path, || {
        let mut ledger = Ledger::load(&path);
        let returning = ledger.returning(&hash, stamp);
        let mut decision = appnotes::decide(&kind, who, admitted, &choices, &ask);
        // A later text never takes a still valid code out of the shade: its notification is let through as the
        // code's was; nothing more is told (it is no code itself). Never for what is held for good (the blocked).
        if earlier_code && decision.until.is_some() && decision.why != Why::Never {
            decision = Decision::through(Why::Code);
        }
        if answered {
            decision = decision.answered();
        }
        // Back from a release you asked (Reachable, or "Let what your phone holds through"): held
        // before it, it comes now, once; never the blocked nor a conversation set to never, and
        // never during a pause, when everything Sioul shows is held (review of part 3, F5).
        if returning
            && !paused
            && let Some(held) = ledger.held.get(&hash)
        {
            decision = decision.released(held, &ledger);
        }
        // Back after its gathering, by Sioul's quiet window moved off one of its own reminders or by
        // Android's safety net minutes later: it comes with the gathering it waited for.
        if returning && let Some(held) = ledger.held.get(&hash) {
            decision = decision.on_time(held, stamp, appnotes::gathers_held_now(held, &ask));
        }
        // Back at its time and held again within minutes: not again and again.
        if returning
            && let Some(until) = decision.until
            && decision.why != Why::Never
        {
            let short = until - stamp < appnotes::AGAIN_SHORT;
            let again = if short { ledger.held.get(&hash).map_or(0, |h| h.again) + 1 } else { 0 };
            if again > appnotes::AGAIN_MAX {
                decision = Decision::through(Why::Again);
            } else if let Some(held) = ledger.held.get_mut(&hash) {
                held.again = again;
            }
        }
        // Back at its time and let come: one released, said in the log with what is still held. A person's
        // message that waited, come at its time: Java tells it once with the others of its window (B2).
        let mut released = None;
        let waited = returning && ledger.held.get(&hash).is_some_and(|held| appnotes::waited(held, &decision));
        if returning && decision.until.is_none() {
            let kind = ledger.held.get(&hash).map_or("other apps", kind_of);
            ledger.came_back(&hash, stamp);
            let mut counted = vec![Counted { kind, until: None, why: decision.why }];
            counted.extend(ledger.held.values().filter(|h| h.until > stamp).map(|h| Counted::held(h, h.until)));
            released = Some(counted);
        }
        ledger.note(&posted, &kind, who, circle, admitted, area, &decision, stamp);
        if let Some(held) = ledger.held.get_mut(&hash) {
            held.persons = persons.clone();
        }
        if machine && decision.why == Why::Gathered
            && let Some(app) = ledger.apps.get_mut(&posted.package)
        {
            app.why = MACHINE.to_string();
        }
        // Someone Sioul could not tell, judged as a contact (A1): the tab says so, never as a contact's.
        if unknown
            && matches!(decision.why, Why::Allowed | Why::Waiting)
            && let Some(app) = ledger.apps.get_mut(&posted.package)
        {
            app.why = UNKNOWN.to_string();
        }
        // Its return to come without a sound (an automaton's, held for good, a person's message): Java holds it
        // a few minutes past its time and brings it back itself, in a quiet window (`QuietWindow`); a person's
        // on every Android version, so that its return is told once (B2).
        let quiet = decision.until.is_some() && ledger.held.get(&hash).is_some_and(appnotes::Held::quiet);
        let people = decision.until.is_some() && ledger.held.get(&hash).is_some_and(appnotes::Held::person);
        ledger.forget_old(stamp, &choices);
        if let Err(e) = ledger.save(&path) {
            eprintln!("Notes: {e}");
        }
        (decision, released, returning, quiet, people, waited)
    });
    // A channel Sioul made silent that rings again: given its sound back by hand in Android's settings,
    // left as set (`appnotes::Kept::by_hand`).
    given_back_by_hand(&posted, stamp);
    // The phone's messages on your computers: its lines written when its app sends them (`phonemsgs`);
    // a reply from a computer handed lately, looked for in it as shown (`replies`).
    let shared = crate::phonemsgs::note(json, &posted, &kind, if blocked_code { Some(Who::Blocked) } else { who }, circle, admitted, &decision) | crate::replies::heard(json, &posted);
    let hold = decision.until.map_or(0, |until| (until - stamp).max(1) * 1000);
    let why = if machine && decision.why == Why::Gathered { MACHINE } else { decision.why.id() };
    // A channel that rang while held, Sioul making the gathered apps silent itself: Java asks what to change now.
    let channels = choices.silence && decision.until.is_some() && posted.alerted && !posted.intercepted && !posted.channel.is_empty();
    // Held now, or back from being held: Java writes the line of what Sioul holds again (held apps phase 1, B1).
    let changed = returning || decision.until.is_some();
    let mut answer = json!({ "hold": hold, "why": why, "shared": shared, "quiet": quiet, "people": people, "waited": waited, "channels": channels, "changed": changed });
    // A code let through while Android's do-not-disturb silences its app's notification: Sioul's
    // own notice of it, for Java to post on the channel that passes Sioul's modes (`code_notice`).
    if matches!(kind, Kind::Code)
        && posted.intercepted
        && decision.until.is_none()
        && !blocked_code
        && let Some(notice) = code_notice(&posted, &sioul_core::words::Words::of(&config), stamp, said_at(json, &posted, stamp))
    {
        answer["code"] = notice;
    }
    if let Some(counted) = released {
        let columns = column_words(&columns_now(&live, &now, slot(&now), gate.is_some()), nothing && live.clock.mode(&now).free(), gate.as_ref());
        answer["line"] = json!(log_line("release", &counted, &columns.iter().map(String::as_str).collect::<Vec<_>>(), &now));
    } else if decision.until.is_some() {
        // Held: a line of its own, with what decided it (`appnotes::cause`), a machine's number said.
        let cause = appnotes::cause(&posted.package, &kind, who, admitted, &choices, &ask).map(|mut cause| {
            if machine && cause.gathered == Some(Gathering::Automaton) {
                cause.gathered = Some(Gathering::Machine);
            }
            cause
        });
        answer["line"] = json!(hold_line(if returning { "held again" } else { "held" }, kind_word(&kind), &decision, cause.as_ref(), gate.as_ref(), &now));
    } else if unknown && decision.why == Why::Allowed {
        // Let through from someone Sioul could not tell, as a contact's (A1): said, so that it is never taken
        // for a contact's, without a name.
        answer["line"] = json!(pass_line(kind_word(&kind)));
    }
    answer.to_string()
}

/// Sioul's own notice of a code another app brought (a text's, a mail app's),
/// for a notification Android's do-not-disturb silences (`Posted::intercepted`:
/// one of Sioul's modes, whose policy lets through only the people it names,
/// or the person's own): to do, passing Sioul's modes (`pierce`), so that a
/// code asked for at night or in a pause still sounds once (the owner's word
/// of 24 November 2026). Its title says from whom, as safe to say
/// (`phonemsgs::code_sender`: never a name that may hold the code); its body
/// the code and how long it lasts, with Copy; on the lock screen its title
/// alone, never the code (`code_note`'s rule). The app's own notification
/// stays, silent, in the shade: Java takes Sioul's away with it. Its tag is
/// the app's notification's, hashed. Its life counts from when its message
/// was sent (`said`, Unix seconds): none once it no longer lasts.
fn code_notice(posted: &Posted, words: &sioul_core::words::Words, now: i64, said: i64) -> Option<Value> {
    use sioul_core::notice::{Act, Kind as Of, Notice};
    let found = appnotes::code_of(posted, words);
    // From whom, as safe to say; the app's name in place of a sender that may hold a code: "A code in Message".
    let (sender, app) = sioul_core::phonemsgs::code_sender_or_app(posted, words);
    let title = if app { crate::backend::say("appnotes-code-title-app", &[("app", sender)]) } else { crate::backend::say("appnotes-code-title", &[("sender", sender)]) };
    let lasts = found.as_ref().map_or(sioul_core::codes::CodeKind::Code.usual_minutes(), sioul_core::codes::OneTimeCode::lasts_minutes);
    let code = found.as_ref().and_then(|c| c.code.clone());
    let validity = found.as_ref().and_then(|c| c.expires_minutes).map(|minutes| tr().validity(minutes));
    let body = match (&code, validity) {
        (Some(code), Some(validity)) => format!("{code}, {validity}"),
        (Some(code), None) => code.clone(),
        // A copy Android redacted: a code came; else an approval (a sign-in, a payment), no code in it.
        (None, _) if posted.redacted => tr().text("appnotes-code-body-open", None),
        (None, _) => tr().text("appnotes-code-body-approval", None),
    };
    // Its life counts from when its message was sent: a notification posted again long after (a restart) says nothing.
    let until = said + i64::from(lasts) * 60;
    if until <= now {
        return None;
    }
    let mut notice = Notice::new("", Of::Own(sioul_core::attention::Kind::Codes), title.clone(), body).until(Some(until));
    if let Some(code) = code {
        notice = notice.action(tr().text("notify-copy", None), Act::Copy(code));
    }
    crate::eventalarms::notice_json(&format!("code:{}", appnotes::key_hash(&posted.key)), &notice, true, Some((&title, "")), now)
}

/// Whether an earlier message of someone else in a notification hands over
/// a code still valid at `now` (Unix seconds), its life counted from its own
/// time, else from the notification's `when`, else from its posting; never
/// when none of them is said.
fn earlier_code_lasts(json: &str, posted: &Posted, words: &sioul_core::words::Words, now: i64) -> bool {
    let codes = appnotes::earlier_codes(posted, words);
    if codes.is_empty() {
        return false;
    }
    let extra = sioul_core::phonemsgs::Extra::read(json);
    codes.iter().any(|(at, code)| {
        let from = [*at, extra.when, extra.posted].into_iter().find(|ms| *ms > 0);
        from.is_some_and(|ms| ms / 1000 + i64::from(code.lasts_minutes()) * 60 > now)
    })
}

/// When a notification's message was sent (Unix seconds): its newest
/// message of someone else's own time, else the notification's `when`,
/// else when Android posted it, never later than `now`; `now` when Java
/// says none.
fn said_at(json: &str, posted: &Posted, now: i64) -> i64 {
    let extra = sioul_core::phonemsgs::Extra::read(json);
    let message = appnotes::newest_from_others(posted).into_iter().map(|i| posted.messages[i].at).max();
    [message, Some(extra.when), Some(extra.posted)].into_iter().flatten().find(|ms| *ms > 0).map_or(now, |ms| (ms / 1000).min(now))
}

/// The listener's line when a person's message from someone Sioul could not
/// tell comes at once, judged as a contact's (held apps phase 1, A1): "Notes:
/// passed: messages 1; why: sender unknown (judged as a contact)." Never a
/// name, a number nor an app.
fn pass_line(kind: &str) -> String {
    format!("Notes: passed: {kind} 1; why: {UNKNOWN_WHY}.")
}

/// The log's words for a sender Sioul could not tell (`appnotes::Cause::unknown`).
const UNKNOWN_WHY: &str = "sender unknown (judged as a contact)";

/// A notification on a channel Sioul made silent, ringing again
/// (importance 3 or more): you gave it its sound back in Android's settings,
/// so Sioul leaves it as you set it and never makes it silent again. A
/// minute's grace after Sioul changed it: Android may have ranked one
/// before the change.
fn given_back_by_hand(posted: &Posted, stamp: i64) {
    // A conversation's own channel ranks it: its importance says nothing of its app's channel.
    let ranked_here = posted.ranked_channel.is_empty() || posted.ranked_channel == posted.channel;
    if posted.importance < 3 || posted.channel.is_empty() || !ranked_here {
        return;
    }
    let path = appnotes::Silenced::default_path();
    if !appnotes::Silenced::load(&path).is_ok_and(|s| s.holds(&posted.package, &posted.channel)) {
        return;
    }
    let result = sioul_core::filelock::with_lock(&path, || {
        let mut silenced = appnotes::Silenced::load(&path)?;
        let Some(kept) = silenced.app.get_mut(&posted.package).and_then(|channels| channels.get_mut(&posted.channel)) else { return Ok(()) };
        if kept.by_hand || stamp - kept.at <= 60 {
            return Ok(());
        }
        kept.by_hand = true;
        silenced.save(&path)
    });
    if let Err(e) = result {
        eprintln!("Notes: {e}");
    }
}

/// The listener's questions on the channels Sioul makes silent itself
/// (`AppNotes.channels`, on its worker; option 2 of
/// docs/research/android-holding.md): "plan" {linked} → {silence:
/// [{package, channel}], restore: [{package, channel, importance}], unlink}
/// (`appnotes::channels_plan`; those given back by hand and no longer
/// wanted forgotten here); "keep" {package, channel, name, was} → {go}: its
/// importance written down before Java changes it, so that it can always be
/// given back, never over a file of a later Sioul or one that does not
/// read; "forget" {package, channel}: given back, gone, or never changed ("":
/// every channel of the app). Kept on this phone alone.
fn channels(verb: &str, json: &str) -> String {
    let asked: Value = serde_json::from_str(json).unwrap_or(Value::Null);
    let path = appnotes::Silenced::default_path();
    let package = asked["package"].as_str().unwrap_or_default().to_string();
    let channel = asked["channel"].as_str().unwrap_or_default().to_string();
    match verb {
        "plan" => {
            let linked = asked["linked"].as_bool().unwrap_or(false);
            // Choices that do not read are no word to give every sound back: nothing changes until they read.
            let choices = match Choices::read(&Choices::default_path()) {
                Ok(choices) => choices,
                Err(e) => {
                    eprintln!("Notes: {e}");
                    return json!({ "silence": [], "restore": [], "unlink": false }).to_string();
                }
            };
            let ledger = Ledger::load(&Ledger::default_path());
            let plan = sioul_core::filelock::with_lock(&path, || {
                let mut silenced = match appnotes::Silenced::load(&path) {
                    Ok(silenced) => silenced,
                    Err(e) => {
                        eprintln!("Notes: {e}");
                        return appnotes::ChannelPlan::default();
                    }
                };
                let plan = appnotes::channels_plan(&ledger, &choices, &silenced, linked);
                if !plan.forget.is_empty() {
                    for (package, channel) in &plan.forget {
                        silenced.forget(package, channel);
                    }
                    if let Err(e) = silenced.save(&path) {
                        eprintln!("Notes: {e}");
                    }
                }
                plan
            });
            json!({
                "silence": plan.silence.iter().map(|(package, channel)| json!({ "package": package, "channel": channel })).collect::<Vec<_>>(),
                "restore": plan.restore.iter().map(|(package, channel, importance)| json!({ "package": package, "channel": channel, "importance": importance })).collect::<Vec<_>>(),
                "unlink": plan.unlink,
            })
            .to_string()
        }
        "keep" => {
            let name = asked["name"].as_str().unwrap_or_default().to_string();
            let was = asked["was"].as_i64().and_then(|w| i32::try_from(w).ok());
            let go = !package.is_empty()
                && !channel.is_empty()
                && was.is_some()
                && sioul_core::filelock::with_lock(&path, || {
                    let mut silenced = appnotes::Silenced::load(&path)?;
                    silenced.keep(&package, &channel, &name, was.unwrap_or_default(), jiff::Timestamp::now().as_second());
                    silenced.save(&path)
                })
                .map_err(|e| eprintln!("Notes: {e}"))
                .is_ok();
            json!({ "go": go }).to_string()
        }
        "forget" => {
            if !package.is_empty() {
                let result = sioul_core::filelock::with_lock(&path, || {
                    let mut silenced = appnotes::Silenced::load(&path)?;
                    silenced.forget(&package, &channel);
                    silenced.save(&path)
                });
                if let Err(e) = result {
                    eprintln!("Notes: {e}");
                }
            }
            "{}".to_string()
        }
        _ => "null".to_string(),
    }
}

/// The hashes of the keys Java names (`appnotes::key_hash`), as the ledger keeps them.
fn hashes(keys: &Value) -> std::collections::BTreeSet<String> {
    keys.as_array().map(|keys| keys.iter().filter_map(Value::as_str).map(appnotes::key_hash).collect()).unwrap_or_default()
}

/// What Sioul says in the phone's shade of what it holds (held apps phase 1;
/// AppNotes.java and HeldNotices.java post it), the listener's questions on
/// its worker. Apps' names only, never who wrote nor a conversation:
/// - "line" `{snoozed: [keys]}` → `{text, public, show, channel}`: the standing
///   line (B1, `appnotes::held_line`) of what Android holds now, without a
///   sound; `text` "" when nothing will come back, holding is off or the line
///   is turned off (`Choices::shade`), and the line goes; `show` the words of
///   Show now, "" during a pause, when a release would wait for its end.
/// - "show" `{}` → `{released}`: Show now pressed, a release of this device as
///   "Let what your phone holds through" (`everywhere::release_here`), never
///   during a pause; the review Java runs then lets through what was held
///   before it, never the blocked nor a conversation set to never.
/// - "waited" `{count, apps: [names]}` → `{text, public, channel}`: people's
///   messages come back at their time, told once with a sound (B2,
///   `appnotes::waited_line`): "Four messages waited: Signal and WhatsApp."
/// - "lost" `{snoozed, active: [keys]}` → `{text, public, count, packages}`: at
///   the listener's first look after the phone started (B3), what the ledger
///   says is held and Android neither holds nor shows was lost: said once
///   (`appnotes::lost_line`), then forgotten (`Ledger::lost`); `text` "" when
///   nothing was; `packages`, their apps, for a tap to open the one.
fn shade(verb: &str, json: &str) -> String {
    shade_at(verb, json, &sioul_core::zone::now())
}

/// `shade` at `now` (the tests' fixed clock).
fn shade_at(verb: &str, json: &str, now: &Zoned) -> String {
    let asked: Value = serde_json::from_str(json).unwrap_or(Value::Null);
    let tr = tr();
    match verb {
        "line" => {
            let choices = Choices::load(&Choices::default_path());
            let text = if choices.hold && choices.shade { appnotes::held_line(tr, &Ledger::load(&Ledger::default_path()), &hashes(&asked["snoozed"]), now).unwrap_or_default() } else { String::new() };
            let paused = live(&load_config(), now).clock.mode(now).paused();
            json!({
                "text": text,
                "public": tr.text("appnotes-shade-public", None),
                "show": if paused { String::new() } else { tr.text("appnotes-shade-show", None) },
                "channel": tr.text("appnotes-shade-channel", None),
            })
            .to_string()
        }
        "show" => {
            // Never during a pause, when everything Sioul shows is held (review of part 3, F5): none offered then.
            let released = !live(&load_config(), now).clock.mode(now).paused() && crate::everywhere::release_here(now.timestamp().as_millisecond()).map_err(|e| eprintln!("Notes: {e}")).is_ok();
            json!({ "released": released }).to_string()
        }
        "waited" => {
            let count = usize::try_from(asked["count"].as_u64().unwrap_or(0)).unwrap_or(usize::MAX);
            let mut apps: Vec<String> = Vec::new();
            for name in asked["apps"].as_array().into_iter().flatten().filter_map(Value::as_str) {
                let name = name.split_whitespace().collect::<Vec<_>>().join(" ");
                if !name.is_empty() && !apps.contains(&name) {
                    apps.push(name);
                }
            }
            let text = if count == 0 || apps.is_empty() { String::new() } else { appnotes::waited_line(tr, count, &apps) };
            json!({ "text": text, "public": tr.text("appnotes-waited-public", None), "channel": tr.text("appnotes-waited-channel", None) }).to_string()
        }
        "lost" => {
            let mut present = hashes(&asked["snoozed"]);
            present.extend(hashes(&asked["active"]));
            let stamp = now.timestamp().as_second();
            let path = Ledger::default_path();
            let lost = sioul_core::filelock::with_lock(&path, || {
                let mut ledger = Ledger::load(&path);
                ledger.forget_old(stamp, &Choices::load(&Choices::default_path()));
                let lost = ledger.lost(&present);
                if let Err(e) = ledger.save(&path) {
                    eprintln!("Notes: {e}");
                }
                lost
            });
            let (mut labels, mut packages): (Vec<String>, Vec<String>) = (Vec::new(), Vec::new());
            for held in &lost {
                if !labels.contains(&held.label) {
                    labels.push(held.label.clone());
                }
                if !packages.contains(&held.app) {
                    packages.push(held.app.clone());
                }
            }
            let text = if lost.is_empty() { String::new() } else { appnotes::lost_line(tr, lost.len(), &labels) };
            json!({ "text": text, "public": tr.text("appnotes-lost-public", None), "count": lost.len(), "packages": packages }).to_string()
        }
        _ => "null".to_string(),
    }
}

/// What Android holds for Sioul now, worked out again (`AppNotes.review`):
/// {"snooze": [{"key", "ms", "quiet", "people"}], "quiet": [{"key", "at", "people"}], "line"}:
/// those whose time moved by more than a minute (1 s: it may come now),
/// `quiet` when their return comes without a sound; every held one whose
/// return comes so, with its time (milliseconds; now: it comes now), from
/// which Java plans its quiet windows (`QuietWindow`), an automaton's, held
/// for good, a person's message (`people`: planned on every Android version,
/// held apps phase 1, B2), and what a release you asked lets through; and the line the
/// listener writes in the phone's log (`log_line`). Marks of notifications
/// Android no longer holds (their app took them away, or they came back
/// unseen) are let go.
fn review(json: &str) -> String {
    review_at(json, &sioul_core::zone::now())
}

/// `review` at `now` (the tests' fixed clock).
fn review_at(json: &str, now: &Zoned) -> String {
    let asked: Value = serde_json::from_str(json).unwrap_or(Value::Null);
    let keys: Vec<String> = asked["snoozed"].as_array().map(|a| a.iter().filter_map(|k| k.as_str().map(str::to_string)).collect()).unwrap_or_default();
    // Why the review runs, as Java says it ("files", "connected"): a word of its own, never a name.
    let reason: String = asked["reason"].as_str().unwrap_or_default().chars().filter(char::is_ascii_lowercase).take(16).collect();
    // The phone's do-not-disturb silences now, as Java read its filter: a text whose cell says ◑
    // comes then, without a sound (decision 13); an older Java says nothing, read as not silenced.
    let silenced = asked["silenced"].as_bool().unwrap_or(false);
    // The phone just started, what Android lost not said yet (B3, `shade`'s "lost"): the marks of what it no
    // longer holds are kept until then.
    let keep = asked["keep"].as_bool().unwrap_or(false);
    let config = load_config();
    let attention = sioul_core::attention::Attention::of(&config);
    let choices = settled_choices(&attention, true);
    let now = now.clone();
    let stamp = now.timestamp().as_second();
    let gate = gate();
    let live = live(&config, &now);
    let gathered = config.reminders.gathered_times();
    let slots = crate::hours::slots();
    let slot = |at: &Zoned| slots.at(at);
    let also = slots.edges();
    let nothing = sioul_core::pause::nothing_now(&sioul_core::quiet::Overrides::load(&sioul_core::quiet::Overrides::default_path()), &config.free_time);
    // A conversation answered from a computer in the last twenty minutes: what Sioul holds of it comes now, but in a pause (`replies`).
    let paused = live.clock.mode(&now).paused();
    let reachable = reachable_until();
    // A release asked since the last one applied (Reachable pressed on any device, or "Let what your phone
    // holds through" on a computer): what was held before it comes now, once (part 3).
    // Never during a pause: the stamps wait for its end, then apply once (F5).
    let releases = crate::everywhere::release_stamps();
    let talks = released_talks();
    let path = Ledger::default_path();
    let (again, quiet, counted, ahead) = sioul_core::filelock::with_lock(&path, || {
        let mut ledger = Ledger::load(&path);
        let (_, ahead) = if paused { (false, Vec::new()) } else { ledger.apply_releases(&releases, stamp) };
        let present: std::collections::BTreeSet<String> = keys.iter().map(|k| appnotes::key_hash(k)).collect();
        ledger.held.retain(|hash, held| keep || present.contains(hash) || stamp - held.since < 60);
        let mut again = Vec::new();
        let mut quiet = Vec::new();
        let mut counted = Vec::new();
        for key in &keys {
            let hash = appnotes::key_hash(key);
            let Some(held) = ledger.held.get(&hash).cloned() else { continue };
            let sources = sioul_core::attention::source_rows_with(&held.app, &held.conversation, &held.persons);
            let ask = Ask { now: &now, clock: &live, attention: &attention, gathered: &gathered, area: held.area, gate: gate.map(|g| Gate { until: g.until }), slot_at: &slot, nothing, also: &also, sources: &sources, reachable, silenced, circle: held.circle };
            // Its gathering come, a little late (the quiet window moved off one of Sioul's own reminders):
            // with it, never until the next.
            let mut decided = appnotes::again(&held, &choices, &ask).on_time(&held, stamp, appnotes::gathers_held_now(&held, &ask));
            if !paused && crate::replies::answered_lately(&held.conversation) {
                decided = decided.answered();
            }
            // Held before the release: comes now (snoozed again for a second, never cancelled:
            // a reply from a computer still finds it), every kind, but what is held for good;
            // never during a pause.
            if !paused {
                decided = decided.released(&held, &ledger);
                // Its conversation let through now from the card (`release_talk`): what was held before then.
                if decided.until.is_some() && decided.why != Why::Never && talks.get(&held.conversation).is_some_and(|at| held.first_held() <= *at) {
                    decided = Decision::through(Why::Released);
                }
            }
            let until = decided.until.unwrap_or(stamp);
            counted.push(Counted { why: decided.why, ..Counted::held(&held, until) });
            // Held for good now (its conversation set to never since): kept so, so that the log never
            // counts it as waiting for a time (review of part 3, I8).
            if decided.why == Why::Never
                && let Some(kept) = ledger.held.get_mut(&hash)
            {
                kept.kind = "never".to_string();
            }
            // Its return without a sound: an automaton's, held for good, a person's message (told once then,
            // B2), or let through by a release you asked (you are at a screen then).
            let silent = ledger.held.get(&hash).is_some_and(appnotes::Held::quiet) || decided.why == Why::Released;
            // A person's message: planned on every Android version, its return told once (B2).
            let person = ledger.held.get(&hash).is_some_and(appnotes::Held::person);
            if silent {
                quiet.push(json!({ "key": key, "at": until.max(stamp) * 1000, "people": person }));
            }
            if (until - held.until).abs() > 60 {
                if let Some(kept) = ledger.held.get_mut(&hash) {
                    kept.until = until;
                }
                again.push(json!({ "key": key, "ms": (until - stamp).max(1) * 1000, "quiet": silent, "people": person }));
            }
        }
        ledger.forget_old(stamp, &choices);
        if let Err(e) = ledger.save(&path) {
            eprintln!("Notes: {e}");
        }
        (again, quiet, counted, ahead.len())
    });
    let event = if reason.is_empty() { "review".to_string() } else { format!("review ({reason})") };
    let columns = column_words(&columns_now(&live, &now, slot(&now), gate.is_some()), nothing && live.clock.mode(&now).free(), gate.as_ref());
    let mut line = log_line(&event, &counted, &columns.iter().map(String::as_str).collect::<Vec<_>>(), &now);
    // A device's release stamp more than a day ahead of this phone's clock: passed over, said once (F3).
    if ahead > 0 {
        line.push_str(&format!(" A release asked from {ahead} device(s) more than a day ahead of this phone's clock: passed over."));
    }
    json!({ "snooze": again, "quiet": quiet, "line": line }).to_string()
}

/// This phone's choices, people's messages worked out once when unset and
/// written (held apps phase 1, A2: held only where anything was ever chosen
/// for other apps here, `Choices::work_out_people`), under the choices' lock,
/// from the file as it is then. A file that does not read is never written
/// over: it reads as no choice at all, worked out in memory. `write`: the
/// phone's own (the listener, the tab, the access card on a phone); a
/// computer works it out in memory, where nothing is held.
fn settled_choices(attention: &sioul_core::attention::Attention, write: bool) -> Choices {
    let path = Choices::default_path();
    let mut choices = Choices::load(&path);
    if choices.people.is_some() {
        return choices;
    }
    if write {
        let written = sioul_core::filelock::with_lock(&path, || -> Result<Choices, String> {
            let mut choices = Choices::read(&path)?;
            if choices.work_out_people(attention) {
                choices.save(&path)?;
            }
            Ok(choices)
        });
        match written {
            Ok(settled) => return settled,
            Err(e) => eprintln!("Notes: {e}"),
        }
    }
    choices.work_out_people(attention);
    choices
}

/// This phone's choices as the window reads them (`settled_choices`): written
/// on a phone only.
pub(crate) fn choices_here() -> Choices {
    settled_choices(&sioul_core::attention::Attention::of(&load_config()), cfg!(target_os = "android"))
}

// ---------------------------------------------------------------- the log

/// A notification Sioul holds, as the listener's log line counts it
/// (`log_line`): the kind of its channel, until when it waits (none, or a
/// time past: it comes now), and why.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Counted {
    kind: &'static str,
    until: Option<i64>,
    why: Why,
}

impl Counted {
    /// As the ledger keeps it, waiting until `until`: held for good (the
    /// blocked, a conversation set to never) or until its time.
    fn held(held: &appnotes::Held, until: i64) -> Counted {
        Counted { kind: kind_of(held), until: Some(until), why: if held.kind == "never" { Why::Never } else { Why::Waiting } }
    }
}

/// The kind of channel a held notification came on, as the log says it:
/// calls, messages, mail, or other apps (an automaton's, an app set to come
/// at once); never its app nor who wrote.
fn kind_of(held: &appnotes::Held) -> &'static str {
    match held.via.as_str() {
        "calls" => "calls",
        "messages" => "messages",
        "mail" => "mail",
        _ => "other apps",
    }
}

/// What reaches you now, as the matrix reads it: the time's column (or two,
/// work and admin at once), then its layers, a slot of time for you and
/// do-not-disturb (its switch or a busy stretch).
fn columns_now(live: &Live, now: &Zoned, slot: bool, dnd: bool) -> Vec<Column> {
    let moment = sioul_core::attention::Now::of(&live.clock.mode(now)).layers(slot, dnd);
    let mut columns = moment.times.clone();
    if moment.slot {
        columns.push(Column::Slot);
    }
    if moment.dnd {
        columns.push(Column::Dnd);
    }
    columns
}

/// The matrix's columns as the log says them: their ids ("leisure", "slot"),
/// Free time's "Nothing at all" when it holds ("free (nothing at all)"), and
/// do-not-disturb with what holds it ("dnd (busy)", "dnd (switch)").
fn column_words(columns: &[Column], nothing: bool, gated: Option<&Gated>) -> Vec<String> {
    columns
        .iter()
        .map(|column| match column {
            Column::Free if nothing => "free (nothing at all)".to_string(),
            Column::Dnd => match gated {
                Some(Gated { busy: true, switch: true, .. }) => "dnd (switch and busy)".to_string(),
                Some(Gated { busy: true, .. }) => "dnd (busy)".to_string(),
                Some(Gated { switch: true, .. }) => "dnd (switch)".to_string(),
                _ => column.id().to_string(),
            },
            _ => column.id().to_string(),
        })
        .collect()
}

/// A time as the log says it: "22:00" today, "tomorrow 07:00", else "2026-10-20 09:00".
fn at_words(at: i64, now: &Zoned) -> String {
    match jiff::Timestamp::from_second(at).map(|t| t.to_zoned(now.time_zone().clone())) {
        Err(_) => "an unknown time".to_string(),
        Ok(at) if at.date() == now.date() => at.strftime("%H:%M").to_string(),
        Ok(at) if now.date().tomorrow().is_ok_and(|tomorrow| tomorrow == at.date()) => at.strftime("tomorrow %H:%M").to_string(),
        Ok(at) => at.strftime("%Y-%m-%d %H:%M").to_string(),
    }
}

/// The kind of channel a notification came on, as the log says it (`kind_of`).
fn kind_word(kind: &Kind) -> &'static str {
    match kind {
        Kind::People(talk) => match talk.via {
            Channel::Calls => "calls",
            Channel::Messages => "messages",
            Channel::Mail => "mail",
        },
        _ => "other apps",
    }
}

/// The listener's line at a hold (docs/android.md, "In the phone's log"):
/// the kind of its channel, until when it waits, or for good, and what
/// decided it (`appnotes::cause`): its conversation's or its app's own row,
/// a gathering and what gathers it, what it is for, the columns of the
/// matrix that hold it now (do-not-disturb with what holds it); and, when
/// Sioul could not tell who wrote, "sender unknown (judged as a contact)"
/// (held apps phase 1, A1). "Notes: held: messages 1 until 10:50; why: dnd
/// (busy)." Held again on its return at its time: "held again". Never a
/// name, a number, an app nor who wrote.
fn hold_line(event: &str, kind: &str, decision: &Decision, cause: Option<&Cause>, gated: Option<&Gated>, now: &Zoned) -> String {
    let until = match decision.until {
        _ if decision.why == Why::Never => "for good".to_string(),
        Some(until) => format!("until {}", at_words(until, now)),
        None => "until now".to_string(),
    };
    let mut why: Vec<String> = Vec::new();
    if let Some(cause) = cause {
        if cause.for_good {
            why.push("never".into());
        }
        match cause.own {
            Some(Own::Conversation) => why.push("its conversation's row".into()),
            Some(Own::Person) => why.push("their own row".into()),
            Some(Own::App) => why.push("its app's row".into()),
            None => {}
        }
        match cause.gathered {
            Some(Gathering::Automaton) => why.push("gathered".into()),
            Some(Gathering::App) => why.push("gathered (its app)".into()),
            Some(Gathering::Conversation) => why.push("gathered (its conversation)".into()),
            Some(Gathering::Named) => why.push("gathered (a sender named only)".into()),
            Some(Gathering::Machine) => why.push("gathered (a number kept for machines)".into()),
            Some(Gathering::Time) => why.push("gathered (messages by day)".into()),
            None => {}
        }
        if let Some(area) = cause.area {
            why.push(if cause.device { format!("area ({}), not for this device", area.id()) } else { format!("area ({})", area.id()) });
        }
        if !cause.columns.is_empty() {
            why.push(column_words(&cause.columns, cause.nothing, gated).join(" + "));
        }
        if cause.unknown {
            why.push(UNKNOWN_WHY.into());
        }
    }
    let why = if why.is_empty() { "unknown".to_string() } else { why.join(", ") };
    format!("Notes: {event}: {kind} 1 {until}; why: {why}.")
}

/// The listener's line in the phone's log (`adb logcat -s sioul`), at each
/// review of what is held and at each release (docs/android.md,
/// "Notifications from other apps"): how many notifications are held, and how
/// many come now, by the kind of their channel; why, the matrix's column now
/// and its layers ("leisure + dnd (switch)"), or Always through when that let
/// one come; when the next one comes back. Never a name, a number, an app nor
/// a word of a message: kinds, counts and times only. At each hold, a line of
/// its own (`hold_line`).
fn log_line(event: &str, counted: &[Counted], columns: &[&str], now: &Zoned) -> String {
    let stamp = now.timestamp().as_second();
    let (released, held): (Vec<&Counted>, Vec<&Counted>) = counted.iter().partition(|c| c.until.is_none_or(|until| until <= stamp));
    let by_kind = |items: &[&Counted]| {
        let kinds: Vec<String> = ["calls", "messages", "mail", "other apps"].iter().map(|kind| (kind, items.iter().filter(|c| c.kind == *kind).count())).filter(|(_, n)| *n > 0).map(|(kind, n)| format!("{kind} {n}")).collect();
        if kinds.is_empty() { items.len().to_string() } else { format!("{} ({})", items.len(), kinds.join(", ")) }
    };
    let why = if released.iter().any(|c| c.why == Why::Always) {
        "Always through".to_string()
    } else if released.iter().any(|c| c.why == Why::Answered) {
        "answered from a computer".to_string()
    } else if released.iter().any(|c| c.why == Why::Released) {
        "released as you asked".to_string()
    } else if released.iter().any(|c| c.why == Why::PeopleOff) {
        "people's messages not held".to_string()
    } else if columns.is_empty() {
        "unknown".to_string()
    } else {
        columns.join(" + ")
    };
    // Held for good (the blocked, a conversation set to never) comes back at no time of the matrix's.
    let next = held.iter().filter(|c| c.why != Why::Never).filter_map(|c| c.until).min().filter(|at| jiff::Timestamp::from_second(*at).is_ok()).map_or_else(|| "none".to_string(), |at| at_words(at, now));
    format!("Notes: {event}: held {}; released {}; why: {why}; next release {next}.", by_kind(&held), by_kind(&released))
}

/// `decide`, for Java (android/main.cpp); a panic answers "null", and the
/// notification comes as it was sent. Given back to `sioul_string_free`.
///
/// # Safety
/// `json` is null, or a zero-terminated text valid for the call.
// SAFETY: no other symbol of the program has this name.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn sioul_appnotes_decide(json: *const c_char) -> *mut c_char {
    // SAFETY: `json` is null or a C string valid for the call, as this function's contract says.
    let json = unsafe { crate::alarms::key_of(json) };
    let answer = std::panic::catch_unwind(|| decide(&json)).unwrap_or_else(|_| "null".to_string());
    crate::alarms::handed(answer)
}

/// `review`, for Java; "null" when it fails, and what is held keeps its time.
///
/// # Safety
/// `json` is null, or a zero-terminated text valid for the call.
// SAFETY: no other symbol of the program has this name.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn sioul_appnotes_review(json: *const c_char) -> *mut c_char {
    // SAFETY: `json` is null or a C string valid for the call, as this function's contract says.
    let json = unsafe { crate::alarms::key_of(json) };
    let answer = std::panic::catch_unwind(|| review(&json)).unwrap_or_else(|_| "null".to_string());
    crate::alarms::handed(answer)
}

/// `channels`, for Java: a verb and its JSON; "null" when it fails, and no
/// channel changes.
///
/// # Safety
/// `verb` and `json` are null, or zero-terminated texts valid for the call.
// SAFETY: no other symbol of the program has this name.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn sioul_appnotes_channels(verb: *const c_char, json: *const c_char) -> *mut c_char {
    // SAFETY: `verb` and `json` are null or C strings valid for the call, as this function's contract says.
    let (verb, json) = unsafe { (crate::alarms::key_of(verb), crate::alarms::key_of(json)) };
    let answer = std::panic::catch_unwind(|| channels(&verb, &json)).unwrap_or_else(|_| "null".to_string());
    crate::alarms::handed(answer)
}

/// `shade`, for Java: a verb and its JSON; "null" when it fails, and the
/// shade stays as it is.
///
/// # Safety
/// `verb` and `json` are null, or zero-terminated texts valid for the call.
// SAFETY: no other symbol of the program has this name.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn sioul_appnotes_shade(verb: *const c_char, json: *const c_char) -> *mut c_char {
    // SAFETY: `verb` and `json` are null or C strings valid for the call, as this function's contract says.
    let (verb, json) = unsafe { (crate::alarms::key_of(verb), crate::alarms::key_of(json)) };
    let answer = std::panic::catch_unwind(|| shade(&verb, &json)).unwrap_or_else(|_| "null".to_string());
    crate::alarms::handed(answer)
}

// ---------------------------------------------------------------- the window

/// The conversations let through now from the card (`release_talk`), by
/// their hashed key, with when (Unix seconds): this phone's own file, never
/// shared, holding no title; the listener reviews when it changes.
const TALKS_FILE: &str = "app-notes-talks.toml";

fn talks_path() -> std::path::PathBuf {
    sioul_core::config::state_dir().join(TALKS_FILE)
}

fn released_talks() -> std::collections::BTreeMap<String, i64> {
    std::fs::read_to_string(talks_path()).ok().and_then(|t| toml::from_str(&t).ok()).unwrap_or_default()
}

/// The conversations the phone holds now, for the card (ReachCard.qml): {key,
/// title, app} each, the soonest first. Their titles come from the phone's
/// own ledger and go to its window alone: never shared, never on a computer
/// (decision 16; none there, where nothing is held).
pub(crate) fn held_talks() -> Vec<Value> {
    if !cfg!(target_os = "android") {
        return Vec::new();
    }
    let path = Ledger::default_path();
    if !path.exists() {
        return Vec::new();
    }
    let ledger = Ledger::load(&path);
    let now = jiff::Timestamp::now().as_second();
    let mut held: Vec<&appnotes::Held> = ledger.held.values().filter(|h| h.kind == "people" && !h.conversation.is_empty() && h.until > now).collect();
    held.sort_by_key(|h| h.until);
    let mut out: Vec<Value> = Vec::new();
    for h in held {
        if out.iter().any(|t| t["key"] == h.conversation.as_str()) {
            continue;
        }
        let title = ledger.conversations.get(&h.conversation).map(|c| c.title.split_whitespace().collect::<Vec<_>>().join(" ")).filter(|t| !t.is_empty()).unwrap_or_else(|| h.label.clone());
        out.push(json!({ "key": h.conversation, "title": title, "app": h.label }));
    }
    out
}

/// "Let it through now" on a held conversation (the card): what is held of
/// it comes at the listener's next review, which this file's change brings.
pub(crate) fn release_talk(key: &str) -> String {
    let now = jiff::Timestamp::now().as_second();
    let mut talks = released_talks();
    talks.retain(|_, at| now - *at < 2 * 86_400);
    talks.insert(key.to_string(), now);
    let path = talks_path();
    let written = toml::to_string(&talks).map_err(|e| e.to_string()).and_then(|text| {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        }
        sioul_core::filelock::replace(&path, text.as_bytes()).map_err(|e| format!("{}: {e}", path.display()))
    });
    match written {
        Ok(()) => tr().text("reach-card-let-it-done", None),
        Err(e) => e,
    }
}

/// Which row and which cells "Let them through then" sets for a held
/// conversation (`let_talk_then`): the person's own row by their card, else
/// by their number, else the conversation's; under do-not-disturb that layer,
/// in a slot of time for you that one, during work Work and Admin together,
/// else the time now.
fn talk_then(held: &appnotes::Held, now: &sioul_core::attention::Now) -> (String, Vec<Column>) {
    // A group's, or a number nobody knows (whose calls stay the screening's): the conversation's row,
    // never one member's (review of part 6, F5).
    let person = if held.group {
        None
    } else {
        held.persons.iter().find(|p| p.starts_with("person.contact:")).or_else(|| held.persons.iter().find(|p| p.starts_with("person.tel:")).filter(|_| !matches!(held.who.as_str(), "" | "stranger")))
    };
    let id = person.cloned().unwrap_or_else(|| format!("{}{}", sioul_core::attention::CONVERSATION_ROW, held.conversation));
    let columns: Vec<Column> = if now.dnd {
        vec![Column::Dnd]
    } else if now.slot {
        vec![Column::Slot]
    } else if now.times.iter().any(|c| matches!(c, Column::Work | Column::Admin)) {
        vec![Column::Work, Column::Admin]
    } else {
        now.times.clone()
    };
    (id, columns)
}

/// "Let them through then" on a held conversation (the card; the proposal's
/// 3.7): the cell of what holds it now set to at once, on the person's own
/// row when a card or a number knows them (their card's first), else on the
/// conversation's: under do-not-disturb, that layer; during work, Work and
/// Admin together; else the time now. Said in a sentence.
pub(crate) fn let_talk_then(key: &str) -> String {
    let path = Ledger::default_path();
    let ledger = Ledger::load(&path);
    // The newest of it still held (review of part 6, F5).
    let stamp = jiff::Timestamp::now().as_second();
    let Some(held) = ledger.held.values().filter(|h| h.conversation == key && h.until > stamp).max_by_key(|h| h.since) else { return String::new() };
    let (id, columns) = talk_then(held, &crate::hours::attention_here());
    let words: Vec<String> = columns.iter().map(|c| c.id().to_string()).collect();
    if let Err(e) = sioul_core::attention::apply_change(&crate::backend::config_path(), &format!("attention.{id}"), None, &sioul_core::config::SettingValue::Texts(words)) {
        return e;
    }
    let time = columns.iter().map(|c| sioul_core::attention::column_label(tr(), *c)).collect::<Vec<_>>().join(", ");
    let mut args = sioul_core::i18n::args();
    args.set("time", time);
    // Their messages and calls on their own row; this conversation's messages alone on its row.
    tr().text(if sioul_core::attention::is_person(&id) { "reach-card-let-then-done" } else { "reach-card-let-then-done-talk" }, Some(&args))
}

/// The Porch's line on other apps (`appnotes::porch_line`); "" when nothing
/// waits nor came back lately, and on a computer (no listener there).
pub(crate) fn porch_line() -> String {
    let path = Ledger::default_path();
    if !path.exists() {
        return String::new();
    }
    appnotes::porch_line(tr(), &Ledger::load(&path), &sioul_core::zone::now())
}

/// The Porch's JSON with its line on other apps (`apps_line`), when there is one.
pub(crate) fn with_line(porch: String) -> String {
    let line = porch_line();
    if line.is_empty() {
        return porch;
    }
    match serde_json::from_str::<Value>(&porch) {
        Ok(Value::Object(mut fields)) => {
            fields.insert("apps_line".into(), Value::String(line));
            Value::Object(fields).to_string()
        }
        _ => porch,
    }
}

/// A row of the tab, as `SettingRow.qml` shows it: {key, kind, label, help, value, choices}.
fn row(key: &str, kind: &str, label: &str, help: &str, value: Value, choices: Vec<(Value, String)>) -> Value {
    json!({
        "key": key,
        "kind": kind,
        "label": label,
        "help": help,
        "value": value,
        "choices": choices.into_iter().map(|(value, label)| json!({ "value": value, "label": label })).collect::<Vec<_>>(),
    })
}

/// The table of other apps by time (What reaches you ▸ Exceptions): each app
/// seen lately a row of the matrix, by name, and the apps' rows set that this
/// device has not seen (a computer's, set on the phone: by the name written
/// with them); then the conversations chosen for (a row or a way through)
/// and the ten seen last ("Title · App"), never a wall of them. Whether
/// some were left out. `AttentionGrid.qml` draws it.
fn by_time(attention: &sioul_core::attention::Attention, ledger: &Ledger, choices: &Choices) -> (Value, bool) {
    use sioul_core::attention::{APP_ROW, CONVERSATION_ROW, SourceLine};
    let tr = tr();
    let apps_group = tr.text("attention-source-group-apps", None);
    let talks_group = tr.text("attention-source-group-conversations", None);
    let mut apps: Vec<SourceLine> = ledger.apps.iter().map(|(package, seen)| SourceLine { id: format!("{APP_ROW}{package}"), label: seen.label.clone(), group: apps_group.clone(), group_id: "app-rows".into() }).collect();
    // Set elsewhere (on the phone, for a computer), or no longer seen here: by the name written with it.
    for id in attention.sources().keys().filter(|id| id.starts_with(APP_ROW)) {
        if !apps.iter().any(|a| &a.id == id) {
            let name = attention.source_name(id);
            let label = if name.is_empty() { id[APP_ROW.len()..].to_string() } else { name.to_string() };
            apps.push(SourceLine { id: id.clone(), label, group: apps_group.clone(), group_id: "app-rows".into() });
        }
    }
    apps.sort_by_cached_key(|a| sioul_core::text::fold(&a.label).into_iter().collect::<String>());
    let mut talks: Vec<(&String, &appnotes::SeenTalk)> = ledger.conversations.iter().collect();
    talks.sort_by_key(|(_, t)| -t.seen);
    let total = talks.len();
    let chosen = |key: &str| choices.conversation.contains_key(key) || attention.source(&format!("{CONVERSATION_ROW}{key}")).is_some();
    let talks: Vec<(&String, &appnotes::SeenTalk)> = talks.into_iter().enumerate().filter(|(i, (key, _))| *i < 10 || chosen(key)).map(|(_, t)| t).collect();
    let more = talks.len() < total;
    let mut lines = apps;
    lines.extend(talks.into_iter().map(|(key, seen)| SourceLine { id: format!("{CONVERSATION_ROW}{key}"), label: format!("{} · {}", seen.title, seen.label), group: talks_group.clone(), group_id: "conversation-rows".into() }));
    (serde_json::to_value(sioul_core::attention::source_grid(attention, tr, &lines)).unwrap_or(Value::Null), more)
}

/// "09:00, 13:00 and 18:00".
fn times_in_words(times: &[String]) -> String {
    let times: Vec<String> = appnotes::times_of(times).iter().map(|t| t.strftime("%H:%M").to_string()).collect();
    match times.as_slice() {
        [] => String::new(),
        [one] => one.clone(),
        [rest @ .., last] => format!("{} {} {last}", rest.join(", "), tr().text("word-and", None)),
    }
}

/// Settings ▸ Other apps, as JSON (AppNotesSetup.qml): whether this is a
/// phone, the access and its steps, the switch, the times, what rang while
/// held, then a row for each app, conversation and site seen.
pub(crate) fn setup() -> String {
    let android = cfg!(target_os = "android");
    let config = load_config();
    let choices = choices_here();
    let ledger = Ledger::load(&Ledger::default_path());
    let now = sioul_core::zone::now();
    let tr = tr();
    let text = |id: &str| tr.text(id, None);
    let mut times = sioul_core::i18n::args();
    times.set("times", times_in_words(&config.reminders.gathered_times()));
    let heard = if ledger.heard > 0 {
        let mut args = sioul_core::i18n::args();
        let at = jiff::Timestamp::from_second(ledger.heard).map(|t| t.to_zoned(now.time_zone().clone())).unwrap_or_else(|_| now.clone());
        args.set("when", tr.when(&at));
        tr.text("appnotes-heard", Some(&args))
    } else {
        String::new()
    };
    let mut rang: Vec<Value> = Vec::new();
    for (package, app) in &ledger.apps {
        for (channel, r) in &app.rang {
            let mut args = sioul_core::i18n::args();
            args.set("app", app.label.clone());
            args.set("channel", if r.name.is_empty() { channel.clone() } else { r.name.clone() });
            rang.push(json!({ "package": package, "channel": channel, "line": tr.text("appnotes-rang-line", Some(&args)) }));
        }
    }
    // Android's link to one of your devices (a companion-device association): whether this phone offers
    // it, and whether Sioul holds one; then what Sioul made silent with it, by app.
    let companion = if android { java("companion", "{}") } else { Value::Null };
    let silenced = appnotes::Silenced::load(&appnotes::Silenced::default_path()).unwrap_or_default();
    let quieted: Vec<Value> = silenced
        .app
        .iter()
        .filter_map(|(package, channels)| {
            let names: Vec<String> = channels.iter().filter(|(_, kept)| !kept.by_hand).map(|(id, kept)| if kept.name.is_empty() { id.clone() } else { kept.name.clone() }).collect();
            if names.is_empty() {
                return None;
            }
            let label = ledger.apps.get(package).map(|a| a.label.clone()).or_else(|| choices.app.get(package).map(|c| c.label.clone()).filter(|l| !l.is_empty())).unwrap_or_else(|| package.clone());
            let mut args = sioul_core::i18n::args();
            args.set("app", label);
            args.set("channels", names.join(", "));
            Some(json!({ "package": package, "line": tr.text("appnotes-silenced-line", Some(&args)) }))
        })
        .collect();
    let kinds: Vec<(Value, String)> = AppKind::ALL.iter().map(|k| (json!(k.id()), text(&format!("appnotes-kind-{}", k.id())))).collect();
    let attention = sioul_core::attention::Attention::of(&config);
    let (grid, more) = by_time(&attention, &ledger, &choices);
    let mut apps: Vec<(&String, &appnotes::SeenApp)> = ledger.apps.iter().collect();
    apps.sort_by_cached_key(|(_, a)| sioul_core::text::fold(&a.label).into_iter().collect::<String>());
    let apps: Vec<Value> = apps
        .into_iter()
        .map(|(package, seen)| {
            let chosen = choices.app(package);
            let why = appnotes::why_text(tr, &seen.why, seen.until, &now);
            json!({
                "package": package,
                // Its own row of the matrix, in one sentence: "At once: Work; held: Leisure…".
                "times": sioul_core::attention::source_sentence(&attention, tr, &format!("{}{package}", sioul_core::attention::APP_ROW)),
                "kind": row(&format!("app.{package}.kind"), "choice", &seen.label, &why, json!(chosen.kind.id()), kinds.clone()),
                "area": row(&format!("app.{package}.area"), "areas", &text("appnotes-area"), "", json!(chosen.area.map(Area::id).unwrap_or_default()), Vec::new()),
            })
        })
        .collect();
    let throughs: Vec<(Value, String)> = Through::ALL.iter().map(|t| (json!(t.id()), text(&format!("appnotes-through-{}", t.id())))).collect();
    let mut talks: Vec<(&String, &appnotes::SeenTalk)> = ledger.conversations.iter().collect();
    talks.sort_by_key(|(_, t)| -t.seen);
    // What Android says of each: marked Priority there (it rings through Sioul's modes), its page.
    let android_says = if android { java("conversations", "{}") } else { Value::Null };
    let conversations: Vec<Value> = talks
        .into_iter()
        .map(|(key, seen)| {
            let label = format!("{} · {}", seen.title, seen.label);
            let help = if seen.group { text("appnotes-group") } else { String::new() };
            let mut said = row(&format!("conversation.{key}"), "choice", &label, &help, json!(choices.through(key).id()), throughs.clone());
            said["talk"] = json!(key);
            said["important"] = json!(android_says["conversations"][key.as_str()]["important"] == true);
            said
        })
        .collect();
    let at_once = vec![(json!(false), text("appnotes-site-gathered")), (json!(true), text("appnotes-site-at-once"))];
    let sites: Vec<Value> = ledger
        .sites
        .iter()
        .map(|(host, seen)| {
            let app = ledger.apps.get(&seen.app).map_or(seen.app.as_str(), |a| a.label.as_str());
            row(&format!("site.{host}"), "choice", &format!("{host} · {app}"), "", json!(choices.site.get(host).is_some_and(|s| s.at_once)), at_once.clone())
        })
        .collect();
    json!({
        "android": android,
        "access": java("access", "{}") == Value::Bool(true),
        "restricted": java("restricted", "{}") == Value::Bool(true),
        "contacts": java("contacts", "{}") == Value::Bool(true),
        "hold": row("hold", "bool", &text("appnotes-hold"), &text("appnotes-hold-help"), json!(choices.hold), Vec::new()),
        // People's messages held too, by who writes and when (A2): under the switch above, while it is on.
        "people": row("people", "bool", &text("appnotes-people"), &text("appnotes-people-help"), json!(choices.people.unwrap_or(false)), Vec::new()),
        // A line in the phone's shade while something is held, without a sound (B1): on unless turned off.
        "shade": row("shade", "bool", &text("appnotes-shade"), &text("appnotes-shade-help"), json!(choices.shade), Vec::new()),
        "times": tr.text("appnotes-times", Some(&times)),
        "heard": heard,
        "rang": rang,
        // Stopped: while some sounds are still to give back (the listener does it once bound and linked), said
        // so; then that it is done.
        "link": {
            "available": companion["available"] == true,
            "linked": companion["linked"] == true,
            "on": choices.silence,
            "giving_back": !choices.silence && !quieted.is_empty(),
            "given_back": choices.stopped && quieted.is_empty(),
        },
        "silenced": quieted,
        "apps": apps,
        "conversations": conversations,
        "sites": sites,
        "grid": grid,
        "more": more,
    })
    .to_string()
}

/// A choice of the tab, or one of Android's pages (`AppNotesSetup.act`): the tab again, as JSON.
/// "set" {key, value}: "hold", "people", "shade", `app.<package>.kind`, `app.<package>.area`,
/// `app.<package>.sound` ("keep": its sound given back, never made silent
/// by Sioul; "silence": as usual), `conversation.<key>`, `site.<host>`;
/// "open-access", "open-info", "open-app" {package}, "open-channel"
/// {package, channel}; "contacts"; "link": Sioul makes the gathered apps
/// silent itself, Android's question for the link to your computer asked;
/// "unlink": every sound given back, then the link let go (by the listener,
/// which watches the choices); "silence-channel" {package, channel}: that
/// channel made silent by Sioul, whatever its app.
pub(crate) fn change(verb: &str, json: &str) -> String {
    let asked: Value = serde_json::from_str(json).unwrap_or(Value::Null);
    // People's messages worked out from what was chosen before this change, never after it (A2).
    let attention = sioul_core::attention::Attention::of(&load_config());
    match verb {
        "set" => {
            let key = asked["key"].as_str().unwrap_or_default().to_string();
            let value = asked["value"].clone();
            let path = Choices::default_path();
            let result = sioul_core::filelock::with_lock(&path, || {
                let mut choices = Choices::load(&path);
                choices.work_out_people(&attention);
                let ledger = Ledger::load(&Ledger::default_path());
                set(&mut choices, &ledger, &key, &value);
                choices.save(&path)
            });
            if let Err(e) = result {
                eprintln!("Notes: {e}");
            }
        }
        // Android's pages; a conversation's by its key (`talk`), to mark it Priority there.
        "open-access" | "open-info" | "open-app" | "open-channel" | "open-conversation" => {
            java(verb, json);
        }
        "contacts" => crate::steps::ask_contacts(),
        // The link to your computer: your word kept first, then Android's question (AppNotes.call, "associate").
        "link" | "unlink" => {
            let on = verb == "link";
            let path = Choices::default_path();
            let result = sioul_core::filelock::with_lock(&path, || {
                let mut choices = Choices::read(&path)?;
                choices.work_out_people(&attention);
                choices.silence = on;
                choices.stopped = !on;
                choices.save(&path)
            });
            match result {
                Err(e) => eprintln!("Notes: {e}"),
                Ok(()) if on => {
                    java("associate", "{}");
                }
                Ok(()) => {}
            }
        }
        "silence-channel" => {
            let package = asked["package"].as_str().unwrap_or_default().to_string();
            let channel = asked["channel"].as_str().unwrap_or_default().to_string();
            if !package.is_empty() && !channel.is_empty() {
                let path = Choices::default_path();
                let result = sioul_core::filelock::with_lock(&path, || {
                    let mut choices = Choices::load(&path);
                    choices.work_out_people(&attention);
                    let ledger = Ledger::load(&Ledger::default_path());
                    name_silent(&mut choices, &ledger, &package, &channel);
                    choices.save(&path)
                });
                if let Err(e) = result {
                    eprintln!("Notes: {e}");
                }
            }
        }
        // A row of the table by time: its words whole ("attention.app.<package>", nine words), an app's name with them.
        "row" => {
            let key = asked["key"].as_str().unwrap_or_default();
            let mut words: Vec<String> = asked["words"].as_array().map(|w| w.iter().filter_map(|w| w.as_str().map(str::to_string)).collect()).unwrap_or_default();
            if let Some(package) = key.strip_prefix("attention.").and_then(|id| id.strip_prefix(sioul_core::attention::APP_ROW))
                && let Some(seen) = Ledger::load(&Ledger::default_path()).apps.get(package)
            {
                words.push(format!("{}{}", sioul_core::attention::NAME, seen.label));
            }
            // What the row showed: only the cells changed from it are set, over the row as it is now.
            let shown: Option<Vec<String>> = asked["shown"].as_array().map(|w| w.iter().filter_map(|w| w.as_str().map(str::to_string)).collect());
            let shown = shown.map(|mut shown| {
                shown.extend(words.iter().filter(|w| w.starts_with(sioul_core::attention::NAME)).cloned());
                sioul_core::config::SettingValue::Texts(shown)
            });
            if let Err(e) = sioul_core::attention::apply_change(&crate::backend::config_path(), key, shown.as_ref(), &sioul_core::config::SettingValue::Texts(words)) {
                eprintln!("Notes: {e}");
            }
        }
        _ => {}
    }
    setup()
}

/// A channel you asked Sioul to make silent itself ("Make it silent",
/// linked to your computer): named in its app's choice, its sound no longer
/// kept.
fn name_silent(choices: &mut Choices, ledger: &Ledger, package: &str, channel: &str) {
    let mut chosen = choices.app(package);
    if !chosen.silent.iter().any(|c| c == channel) {
        chosen.silent.push(channel.to_string());
    }
    chosen.keep_sound = false;
    chosen.label = ledger.apps.get(package).map(|a| a.label.clone()).unwrap_or(chosen.label);
    choices.app.insert(package.to_string(), chosen);
}

/// One choice written: holding, people's messages, the line in the shade, the app's kind, area or
/// sound, a conversation's way through, a site at once. What says "as
/// usual" is taken out.
fn set(choices: &mut Choices, ledger: &Ledger, key: &str, value: &Value) {
    if key == "hold" {
        choices.hold = value.as_bool().unwrap_or(true);
    } else if key == "people" {
        if let Some(on) = value.as_bool() {
            choices.people = Some(on);
        }
    } else if key == "shade" {
        if let Some(on) = value.as_bool() {
            choices.shade = on;
        }
    } else if let Some(rest) = key.strip_prefix("app.") {
        let Some((package, field)) = rest.rsplit_once('.') else { return };
        let mut chosen = choices.app(package);
        match field {
            "kind" => chosen.kind = value.as_str().and_then(AppKind::read).unwrap_or_default(),
            "area" => chosen.area = value.as_str().and_then(Area::parse),
            // Its sound given back: the channels you named forgotten too, so that nothing makes it silent.
            "sound" => {
                chosen.keep_sound = value.as_str() == Some("keep");
                if chosen.keep_sound {
                    chosen.silent.clear();
                }
            }
            _ => return,
        }
        chosen.label = ledger.apps.get(package).map(|a| a.label.clone()).unwrap_or(chosen.label);
        if chosen.is_usual() {
            choices.app.remove(package);
        } else {
            choices.app.insert(package.to_string(), chosen);
        }
    } else if let Some(conversation) = key.strip_prefix("conversation.") {
        let through = value.as_str().and_then(Through::read).unwrap_or_default();
        if through == Through::Usual {
            choices.conversation.remove(conversation);
        } else {
            let seen = ledger.conversations.get(conversation);
            let title = seen.map(|s| s.title.clone()).or_else(|| choices.conversation.get(conversation).map(|c| c.title.clone())).unwrap_or_default();
            let app = seen.map(|s| s.label.clone()).or_else(|| choices.conversation.get(conversation).map(|c| c.app.clone())).unwrap_or_default();
            choices.conversation.insert(conversation.to_string(), appnotes::TalkChoice { through, title, app });
        }
    } else if let Some(host) = key.strip_prefix("site.") {
        if value.as_bool().unwrap_or(false) {
            choices.site.insert(host.to_string(), appnotes::SiteChoice { at_once: true });
        } else {
            choices.site.remove(host);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A mail app's address finds its account (review of phase 2, M6): the
    /// address or the user name, a "+tag" aside, else the one account of its
    /// domain (an alias); its area as said, or unsaid: work's hours, none
    /// closed for safe senders; an address no account holds has none.
    #[test]
    fn a_mail_app_s_address_finds_its_account() {
        let config: Config = toml::from_str(
            "[[account]]\nid = \"work\"\nkind = \"imap\"\naddress = \"jane@work.example\"\nhost = \"imap.work.example\"\narea = \"work\"\n\
             [[account]]\nid = \"home\"\nkind = \"imap\"\naddress = \"jane@home.example\"\nusername = \"j.doe@home.example\"\nhost = \"imap.home.example\"\n\
             [[account]]\nid = \"a\"\nkind = \"imap\"\naddress = \"one@shared.example\"\nhost = \"imap.shared.example\"\n\
             [[account]]\nid = \"b\"\nkind = \"imap\"\naddress = \"two@shared.example\"\nhost = \"imap.shared.example\"\n",
        )
        .unwrap();
        let id = |to: &str| account_of(&config.accounts, to).map(|a| a.id.as_str());
        assert_eq!(id("Jane@Work.example"), Some("work"));
        assert_eq!(id("jane+news@work.example"), Some("work"), "a +tag");
        assert_eq!(id("support@work.example"), Some("work"), "an alias of the one account of its domain");
        assert_eq!(id("j.doe@home.example"), Some("home"), "its user name");
        assert_eq!(id("three@shared.example"), None, "two accounts share the domain");
        assert_eq!((id("someone@elsewhere.example"), id("Work"), id("")), (None, None, None));
        let talk = |to: &str| Kind::People(appnotes::Talk { via: Channel::Mail, sender: Sender::Unknown, conversation: None, to: to.into(), to_unsaid: false, sms: false });
        let posted = Posted { package: "ch.protonmail.android".into(), ..Posted::default() };
        let choices = Choices::default();
        assert_eq!(area_of(&posted, &talk("support@work.example"), &choices, &config), (Some(Area::WORK), false));
        assert_eq!(area_of(&posted, &talk("jane@home.example"), &choices, &config), (Some(Area::WORK), true), "unsaid: work's hours, open to safe senders");
        assert_eq!(area_of(&posted, &talk("someone@elsewhere.example"), &choices, &config), (None, false));
    }

    /// Decision 16 (the proposal's 3.7): "Let them through then" sets the
    /// person's own row by their card, else their number, else the
    /// conversation's, at what holds the text now: do-not-disturb; work and
    /// admin together; the time.
    #[test]
    fn let_them_through_then_sets_the_cell_of_what_held_it() {
        use sioul_core::attention::Now;
        let held = |persons: &[&str]| appnotes::Held { conversation: "talk-1".into(), persons: persons.iter().map(|p| p.to_string()).collect(), kind: "people".into(), ..Default::default() };
        let work = Now::time(Column::Work);
        assert_eq!(talk_then(&held(&["person.tel:+33199001234", "person.contact:card-1"]), &work), ("person.contact:card-1".to_string(), vec![Column::Work, Column::Admin]));
        assert_eq!(talk_then(&appnotes::Held { who: "neutral".into(), ..held(&["person.noa@example.org", "person.tel:+33199001234"]) }, &Now::time(Column::Leisure)).0, "person.tel:+33199001234");
        // A number nobody knows, and a group: the conversation's row, never one member's (review of part 6, F5).
        assert_eq!(talk_then(&appnotes::Held { who: "stranger".into(), ..held(&["person.tel:+33199001234"]) }, &Now::time(Column::Leisure)).0, "conversation.talk-1");
        assert_eq!(talk_then(&appnotes::Held { group: true, ..held(&["person.contact:card-1"]) }, &Now::time(Column::Leisure)).0, "conversation.talk-1");
        assert_eq!(talk_then(&held(&[]), &Now::time(Column::Leisure)), ("conversation.talk-1".to_string(), vec![Column::Leisure]), "no card nor number: the conversation's");
        assert_eq!(talk_then(&held(&["person.contact:card-1"]), &Now::time(Column::Work).layers(false, true)).1, vec![Column::Dnd]);
    }

    /// Review of part 3, F5: no release during a pause, when everything
    /// Sioul shows is held. Reachable pressed on the laptop seven minutes ago,
    /// heard by the phone only now, five minutes into a pause: its review
    /// lets nothing through, nor does a notification back at its time; the
    /// pause over, the next review lets through, once, what was held before
    /// the press.
    #[test]
    fn no_release_during_a_pause_it_waits_for_its_end() {
        // Writes Sioul's files where it reads them: never the person's own (review of 36002ea).
        sioul_core::config::stand_ins_only();
        const PEOPLE: &str = "0|com.google.android.apps.messaging|2|sms|10001";
        const SHOP_KEY: &str = "0|com.shop.example|1|null|10500";
        const SHOP: &str = r#"{"key":"0|com.shop.example|1|null|10500","package":"com.shop.example","app":"Shop","category":"promo","channel":"offers","title":"-30 % sur les chaussures","text":"Jusqu'à dimanche seulement","alerted":true}"#;
        let _files = crate::everywhere::FILES.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
        crate::everywhere::files_cleared();
        // People's messages held on this phone (held apps phase 1, A2), as before that switch.
        Choices { people: Some(true), ..Choices::default() }.save(&Choices::default_path()).unwrap();
        let (stamp, ms) = (jiff::Timestamp::now().as_second(), jiff::Timestamp::now().as_millisecond());
        let held = |app: &str, kind: &str, via: &str, who: &str, since: i64, until: i64| appnotes::Held { app: app.into(), label: app.into(), kind: kind.into(), via: via.into(), who: who.into(), circle: false, admitted: false, group: false, conversation: String::new(), site: String::new(), area: None, since, until, first: 0, again: 0, persons: Vec::new() };
        let mut ledger = Ledger::default();
        ledger.held.insert(appnotes::key_hash(PEOPLE), held("com.google.android.apps.messaging", "people", "messages", "neutral", stamp - 3600, stamp + 7200));
        ledger.held.insert(appnotes::key_hash(SHOP_KEY), held("com.shop.example", "gathered", "", "", stamp - 1800, stamp + 7200));
        ledger.save(&Ledger::default_path()).unwrap();
        let mut overrides = sioul_core::quiet::Overrides { paused_since: Some(stamp - 300), ..Default::default() };
        overrides.save(&sioul_core::quiet::Overrides::default_path()).unwrap();
        let laptop = sioul_core::everywhere::Device { kind: "computer".into(), pressed: ms - 420_000, mode: "reachable".into(), until: ms + 3_600_000, at: ms - 420_000, ..Default::default() };
        sioul_core::everywhere::Switch { device: [("laptop-1".to_string(), laptop)].into_iter().collect(), ..Default::default() }.save(&sioul_core::everywhere::Switch::default_path()).unwrap();
        let released = |answer: &str| -> Vec<String> {
            let v: serde_json::Value = serde_json::from_str(answer).unwrap();
            v["snooze"].as_array().unwrap().iter().filter(|s| s["ms"].as_i64() == Some(1000)).map(|s| s["key"].as_str().unwrap().to_string()).collect()
        };
        let all = json!({ "snoozed": [PEOPLE, SHOP_KEY], "reason": "files" }).to_string();
        assert!(released(&review(&all)).is_empty(), "nothing let through during the pause");
        assert!(Ledger::load(&Ledger::default_path()).released_by.is_empty(), "the stamp waits");
        // The shop's notification back at its time during the pause: held, never "released".
        let mut ledger = Ledger::load(&Ledger::default_path());
        ledger.held.get_mut(&appnotes::key_hash(SHOP_KEY)).unwrap().until = stamp;
        ledger.save(&Ledger::default_path()).unwrap();
        let back: serde_json::Value = serde_json::from_str(&decide(SHOP)).unwrap();
        assert_ne!(back["why"], "released", "{back}");
        // The pause over: what was held before the press comes, once, the shop's too, though
        // it came back and was held again meanwhile (it was first held before the press).
        overrides.paused_ended = Some(stamp);
        overrides.save(&sioul_core::quiet::Overrides::default_path()).unwrap();
        assert_eq!(released(&review(&all)), vec![PEOPLE.to_string(), SHOP_KEY.to_string()]);
        assert!(released(&review(&all)).is_empty(), "once");
        crate::everywhere::files_cleared();
    }

    /// The quiet window (option 1 of docs/research/android-holding.md): what
    /// Java brings back itself, without a sound, and when, on a fixed clock
    /// (Tuesday 6 October 2026, the gathered times 09:00, 13:00 and 18:00). At
    /// 12:58 a shop's notification is held two minutes, quiet; a person's comes
    /// at once (no hours set here). A review then lists the shop's at 13:00
    /// exactly, and a person's message held until 18:00, which may come now,
    /// at once, quiet, as a person's (held apps phase 1, B2: it comes back in
    /// the window and Sioul tells it once). At 13:02,
    /// its gathering past (its window moved off one of Sioul's reminders), it
    /// comes now, quiet, never at 18:00; at 13:20, past `LATE`, at 18:00. At
    /// 13:30, "Let what your phone holds through" pressed on a computer at
    /// 13:29 lets a person's message through quiet, at once.
    #[test]
    fn what_comes_back_quiet_and_when() {
        sioul_core::config::stand_ins_only();
        const PEOPLE: &str = "0|com.google.android.apps.messaging|2|sms|10001";
        const SHOP_KEY: &str = "0|com.shop.example|1|null|10500";
        const SHOP: &str = r#"{"key":"0|com.shop.example|1|null|10500","package":"com.shop.example","app":"Shop","category":"promo","channel":"offers","title":"-30 % sur les chaussures","text":"Jusqu'à dimanche seulement","alerted":true}"#;
        const NOA: &str = r#"{"key":"0|org.example.chat|7|c0ffee12|10100","package":"org.example.chat","app":"Chat","category":"msg","template":"android.app.Notification$MessagingStyle","title":"Noa Martin","shortcut":"c0ffee12","messages":[{"sender":{"name":"Noa Martin","uri":"tel:%2B33199001234"},"text":"Tu passes ce soir ?"}]}"#;
        let _files = crate::everywhere::FILES.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
        crate::everywhere::files_cleared();
        // People's messages held on this phone (held apps phase 1, A2), as before that switch.
        Choices { people: Some(true), ..Choices::default() }.save(&Choices::default_path()).unwrap();
        let at = |time: &str| -> Zoned { format!("2026-10-06T{time}+02:00[Europe/Paris]").parse().unwrap() };
        let ms = |time: &str| at(time).timestamp().as_millisecond();
        let second = |time: &str| at(time).timestamp().as_second();
        // Decided at 12:58: the shop's, two minutes, quiet, no person's; a person's at once (no hours set here).
        let shop: serde_json::Value = serde_json::from_str(&decide_at(SHOP, &at("12:58:00"))).unwrap();
        assert_eq!((shop["hold"].as_i64(), shop["quiet"].as_bool(), shop["why"].as_str(), shop["people"].as_bool()), (Some(120_000), Some(true), Some("gathered"), Some(false)), "{shop}");
        let noa: serde_json::Value = serde_json::from_str(&decide_at(NOA, &at("12:58:00"))).unwrap();
        assert_eq!((noa["hold"].as_i64(), noa["quiet"].as_bool(), noa["waited"].as_bool()), (Some(0), Some(false), Some(false)), "{noa}");
        let _ = std::fs::remove_file(Ledger::default_path());
        let held = |app: &str, kind: &str, via: &str, who: &str, until: i64| appnotes::Held { app: app.into(), label: app.into(), kind: kind.into(), via: via.into(), who: who.into(), since: second("12:00:00"), until, ..Default::default() };
        let shop_until = |until: i64| {
            let mut ledger = Ledger::load(&Ledger::default_path());
            ledger.held.get_mut(&appnotes::key_hash(SHOP_KEY)).unwrap().until = until;
            ledger.save(&Ledger::default_path()).unwrap();
        };
        let mut ledger = Ledger::default();
        ledger.held.insert(appnotes::key_hash(PEOPLE), held("com.google.android.apps.messaging", "people", "messages", "neutral", second("18:00:00")));
        ledger.held.insert(appnotes::key_hash(SHOP_KEY), held("com.shop.example", "gathered", "", "", second("13:00:00")));
        ledger.save(&Ledger::default_path()).unwrap();
        let all = json!({ "snoozed": [PEOPLE, SHOP_KEY], "reason": "files" }).to_string();
        let quiet = |answer: &serde_json::Value| -> Vec<(String, i64)> { answer["quiet"].as_array().unwrap().iter().map(|q| (q["key"].as_str().unwrap().to_string(), q["at"].as_i64().unwrap())).collect() };
        let moved = |answer: &serde_json::Value, key: &str| answer["snooze"].as_array().unwrap().iter().find(|s| s["key"] == key).map(|s| (s["ms"].as_i64().unwrap(), s["quiet"].as_bool().unwrap()));
        // 12:58: the shop's at 13:00 exactly, quiet, not moved; the person's, who may come now, now, quiet, as a
        // person's (B2: planned on every Android version, told once).
        let reviewed: serde_json::Value = serde_json::from_str(&review_at(&all, &at("12:58:00"))).unwrap();
        assert_eq!(quiet(&reviewed), [(PEOPLE.to_string(), ms("12:58:00")), (SHOP_KEY.to_string(), ms("13:00:00"))], "{reviewed}");
        assert_eq!(moved(&reviewed, SHOP_KEY), None, "{reviewed}");
        assert_eq!(moved(&reviewed, PEOPLE), Some((1000, true)), "{reviewed}");
        let person_of = |answer: &serde_json::Value, key: &str| answer["quiet"].as_array().unwrap().iter().find(|q| q["key"] == key).map(|q| q["people"] == true);
        assert_eq!((person_of(&reviewed, PEOPLE), person_of(&reviewed, SHOP_KEY)), (Some(true), Some(false)), "{reviewed}");
        // 13:02, two minutes after its gathering: now, quiet, never at 18:00.
        let reviewed: serde_json::Value = serde_json::from_str(&review_at(&all, &at("13:02:00"))).unwrap();
        assert!(quiet(&reviewed).contains(&(SHOP_KEY.to_string(), ms("13:02:00"))), "{reviewed}");
        assert_eq!(moved(&reviewed, SHOP_KEY), Some((1000, true)), "{reviewed}");
        // 13:20, past LATE: at the next gathering, 18:00, quiet.
        shop_until(second("13:00:00"));
        let reviewed: serde_json::Value = serde_json::from_str(&review_at(&all, &at("13:20:00"))).unwrap();
        assert!(quiet(&reviewed).contains(&(SHOP_KEY.to_string(), ms("18:00:00"))), "{reviewed}");
        assert_eq!(moved(&reviewed, SHOP_KEY), Some((ms("18:00:00") - ms("13:20:00"), true)), "{reviewed}");
        // 13:30: the person's conversation gathered with the automatons (so held until 18:00), and
        // "Let what your phone holds through" pressed on a computer at 13:29: it comes now, quiet.
        let mut choices = Choices::default();
        choices.conversation.insert("talk-1".into(), appnotes::TalkChoice { through: Through::Gathered, ..Default::default() });
        choices.save(&Choices::default_path()).unwrap();
        let mut ledger = Ledger::load(&Ledger::default_path());
        ledger.held.get_mut(&appnotes::key_hash(PEOPLE)).unwrap().conversation = "talk-1".into();
        ledger.save(&Ledger::default_path()).unwrap();
        let laptop = sioul_core::everywhere::Device { kind: "computer".into(), release: ms("13:29:00"), at: ms("13:29:00"), ..Default::default() };
        sioul_core::everywhere::Switch { device: [("laptop-1".to_string(), laptop)].into_iter().collect(), ..Default::default() }.save(&sioul_core::everywhere::Switch::default_path()).unwrap();
        let reviewed: serde_json::Value = serde_json::from_str(&review_at(&all, &at("13:30:00"))).unwrap();
        assert!(quiet(&reviewed).contains(&(PEOPLE.to_string(), ms("13:30:00"))), "{reviewed}");
        assert_eq!(moved(&reviewed, PEOPLE), Some((1000, true)), "{reviewed}");
        assert!(reviewed["line"].as_str().unwrap().contains("released as you asked"), "{reviewed}");
        crate::everywhere::files_cleared();
    }

    /// The line in the phone's shade (held apps phase 1, B1): of what Android
    /// holds now, apps' names only, held for good never; gone with the
    /// switch or holding off. Show now: a release of this phone, which the
    /// review then applies, quiet, never to the blocked; during a pause none
    /// is offered nor written.
    #[test]
    fn the_line_in_the_shade_and_show_now() {
        sioul_core::config::stand_ins_only();
        const SIGNAL: &str = "0|org.thoughtcrime.securesms|3|null|10300";
        const SHOP_KEY: &str = "0|com.shop.example|1|null|10500";
        const BLOCKED: &str = "0|com.whatsapp|9|null|10400";
        let _files = crate::everywhere::FILES.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
        crate::everywhere::files_cleared();
        Choices { people: Some(true), ..Choices::default() }.save(&Choices::default_path()).unwrap();
        let now = sioul_core::zone::now();
        let stamp = now.timestamp().as_second();
        let held = |app: &str, label: &str, kind: &str, who: &str, until: i64| appnotes::Held { app: app.into(), label: label.into(), kind: kind.into(), via: if kind == "gathered" { String::new() } else { "messages".into() }, who: who.into(), conversation: "talk-noa".into(), since: stamp - 3600, until, ..Default::default() };
        let mut ledger = Ledger::default();
        ledger.held.insert(appnotes::key_hash(SIGNAL), held("org.thoughtcrime.securesms", "Signal", "people", "neutral", stamp + 7200));
        ledger.held.insert(appnotes::key_hash(SHOP_KEY), held("com.shop.example", "Shop", "gathered", "", stamp + 7200));
        ledger.held.insert(appnotes::key_hash(BLOCKED), held("com.whatsapp", "WhatsApp", "never", "blocked", stamp + appnotes::FOR_GOOD));
        ledger.conversations.insert("talk-noa".into(), appnotes::SeenTalk { app: "org.thoughtcrime.securesms".into(), label: "Signal".into(), title: "Noa Martin".into(), group: false, seen: stamp });
        ledger.save(&Ledger::default_path()).unwrap();
        let line = |keys: &[&str]| -> serde_json::Value { serde_json::from_str(&shade_at("line", &json!({ "snoozed": keys }).to_string(), &now)).unwrap() };
        let said = line(&[SIGNAL, SHOP_KEY, BLOCKED]);
        let text = said["text"].as_str().unwrap();
        assert!(text.contains("Shop") && text.contains("Signal"), "{said}");
        for never in ["WhatsApp", "Noa", "Martin", "neutral", "blocked"] {
            assert!(!text.contains(never), "{never}: {said}");
        }
        assert_eq!(said["show"], tr().text("appnotes-shade-show", None), "{said}");
        assert_eq!(said["channel"], tr().text("appnotes-shade-channel", None));
        assert!(!said["public"].as_str().unwrap().contains("Shop"), "no app on the lock screen: {said}");
        // Only what Android holds: the shop's taken away by its app, only Signal named.
        let text = line(&[SIGNAL, BLOCKED])["text"].as_str().unwrap().to_string();
        assert!(text.contains("Signal") && !text.contains("Shop"), "{text}");
        assert_eq!(line(&[BLOCKED])["text"], "", "held for good alone: no line");
        // The switch off, or holding off: no line.
        Choices { people: Some(true), shade: false, ..Choices::default() }.save(&Choices::default_path()).unwrap();
        assert_eq!(line(&[SIGNAL, SHOP_KEY])["text"], "");
        Choices { people: Some(true), hold: false, ..Choices::default() }.save(&Choices::default_path()).unwrap();
        assert_eq!(line(&[SIGNAL, SHOP_KEY])["text"], "");
        Choices { people: Some(true), ..Choices::default() }.save(&Choices::default_path()).unwrap();
        // Show now: this phone's release written; the review lets through, quiet, what was held before it, never the blocked.
        let shown: serde_json::Value = serde_json::from_str(&shade_at("show", "{}", &now)).unwrap();
        assert_eq!(shown["released"], true, "{shown}");
        assert!(sioul_core::everywhere::Switch::read(&sioul_core::everywhere::Switch::default_path()).unwrap().release_stamps().iter().any(|(_, at)| *at >= now.timestamp().as_millisecond()));
        let reviewed: serde_json::Value = serde_json::from_str(&review_at(&json!({ "snoozed": [SIGNAL, SHOP_KEY, BLOCKED], "reason": "show" }).to_string(), &now)).unwrap();
        let now_ms = stamp * 1000;
        let quiet: Vec<(String, i64)> = reviewed["quiet"].as_array().unwrap().iter().map(|q| (q["key"].as_str().unwrap().to_string(), q["at"].as_i64().unwrap())).collect();
        let now_too: Vec<&str> = reviewed["snooze"].as_array().unwrap().iter().filter(|s| s["ms"].as_i64() == Some(1000)).filter_map(|s| s["key"].as_str()).collect();
        // The shop's released, quiet; the person's comes now as well (no hours set here: they may reach you now).
        assert!(quiet.contains(&(SHOP_KEY.to_string(), now_ms)) && now_too.contains(&SHOP_KEY) && now_too.contains(&SIGNAL), "{reviewed}");
        assert!(!now_too.contains(&BLOCKED) && !quiet.iter().any(|(key, at)| key == BLOCKED && *at <= now_ms), "never the blocked: {reviewed}");
        assert!(reviewed["line"].as_str().unwrap().contains("released as you asked"), "{reviewed}");
        // Then nothing left to come back: the line goes.
        assert_eq!(line(&[SIGNAL, SHOP_KEY, BLOCKED])["text"], "");
        // During a pause: no Show now offered, and one pressed (a line shown before) writes nothing.
        crate::everywhere::files_cleared();
        Choices { people: Some(true), ..Choices::default() }.save(&Choices::default_path()).unwrap();
        ledger.save(&Ledger::default_path()).unwrap();
        sioul_core::quiet::Overrides { paused_since: Some(stamp - 300), ..Default::default() }.save(&sioul_core::quiet::Overrides::default_path()).unwrap();
        let in_pause = line(&[SIGNAL, SHOP_KEY]);
        assert!(!in_pause["text"].as_str().unwrap().is_empty() && in_pause["show"] == "", "{in_pause}");
        let pressed: serde_json::Value = serde_json::from_str(&shade_at("show", "{}", &now)).unwrap();
        assert_eq!(pressed["released"], false);
        assert!(sioul_core::everywhere::Switch::read(&sioul_core::everywhere::Switch::default_path()).unwrap().release_stamps().is_empty(), "nothing written during a pause");
        crate::everywhere::files_cleared();
    }

    /// One sound at each return (held apps phase 1, B2): a person's message
    /// held in a pause is held quiet, as a person's (Java plans it on every
    /// Android version); back at its time, the pause over, it comes and Java
    /// is told it waited; an automaton's back at its time never is. The
    /// notice's words: the count and the apps' names, each once.
    #[test]
    fn a_person_s_message_held_comes_back_quiet_and_is_told() {
        sioul_core::config::stand_ins_only();
        const NOA_KEY: &str = "0|org.example.chat|7|c0ffee12|10100";
        const NOA: &str = r#"{"key":"0|org.example.chat|7|c0ffee12|10100","package":"org.example.chat","app":"Chat","category":"msg","template":"android.app.Notification$MessagingStyle","title":"Noa Martin","shortcut":"c0ffee12","messages":[{"sender":{"name":"Noa Martin","uri":"tel:%2B33199001234"},"text":"Tu passes ce soir ?"}]}"#;
        const SHOP_KEY: &str = "0|com.shop.example|1|null|10500";
        const SHOP: &str = r#"{"key":"0|com.shop.example|1|null|10500","package":"com.shop.example","app":"Shop","category":"promo","channel":"offers","title":"-30 % sur les chaussures","text":"Jusqu'à dimanche seulement","alerted":true}"#;
        let _files = crate::everywhere::FILES.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
        crate::everywhere::files_cleared();
        Choices { people: Some(true), ..Choices::default() }.save(&Choices::default_path()).unwrap();
        let now = sioul_core::zone::now();
        let stamp = now.timestamp().as_second();
        let decided = |json: &str| -> serde_json::Value { serde_json::from_str(&decide_at(json, &now)).unwrap() };
        // In a pause: held, quiet, as a person's; not come, so nothing waited yet.
        let mut overrides = sioul_core::quiet::Overrides { paused_since: Some(stamp - 600), ..Default::default() };
        overrides.save(&sioul_core::quiet::Overrides::default_path()).unwrap();
        let held = decided(NOA);
        assert!(held["hold"].as_i64().unwrap() > 0, "{held}");
        assert_eq!((held["quiet"].as_bool(), held["people"].as_bool(), held["waited"].as_bool(), held["changed"].as_bool()), (Some(true), Some(true), Some(false), Some(true)), "{held}");
        // The pause over, back at its time: it comes, and it waited.
        overrides.paused_ended = Some(stamp - 60);
        overrides.save(&sioul_core::quiet::Overrides::default_path()).unwrap();
        let mut ledger = Ledger::load(&Ledger::default_path());
        ledger.held.get_mut(&appnotes::key_hash(NOA_KEY)).unwrap().until = stamp;
        ledger.held.insert(appnotes::key_hash(SHOP_KEY), appnotes::Held { app: "com.shop.example".into(), label: "Shop".into(), kind: "gathered".into(), since: stamp - 3600, until: stamp, ..Default::default() });
        ledger.save(&Ledger::default_path()).unwrap();
        let back = decided(NOA);
        assert_eq!((back["hold"].as_i64(), back["waited"].as_bool(), back["people"].as_bool(), back["changed"].as_bool()), (Some(0), Some(true), Some(false), Some(true)), "{back}");
        // An automaton's back at its time: never told, whether it comes now or waits for its gathering.
        assert_eq!(decided(SHOP)["waited"], false);
        // A person's message never held: nothing waited.
        assert_eq!(decided(NOA)["waited"], false);
        // The notice's words: counted, each app once, as Rust says them; nothing when none came back.
        let words = |json: serde_json::Value| -> serde_json::Value { serde_json::from_str(&shade_at("waited", &json.to_string(), &now)).unwrap() };
        let told = words(json!({ "count": 4, "apps": [" Signal ", "WhatsApp", "Signal"] }));
        assert_eq!(told["text"], appnotes::waited_line(tr(), 4, &["Signal".to_string(), "WhatsApp".to_string()]), "{told}");
        assert_eq!((told["public"].as_str(), told["channel"].as_str()), (Some(tr().text("appnotes-waited-public", None).as_str()), Some(tr().text("appnotes-waited-channel", None).as_str())));
        assert_eq!(words(json!({ "count": 0, "apps": ["Signal"] }))["text"], "");
        assert_eq!(words(json!({ "count": 2, "apps": [] }))["text"], "");
        crate::everywhere::files_cleared();
    }

    /// After a restart (held apps phase 1, B3): while the listener has not
    /// looked yet, a review keeps the marks of what Android no longer holds
    /// (`keep`); the look then says what Android neither holds nor shows,
    /// by app, with their packages for a tap, and forgets it; a second look
    /// says nothing. Without `keep`, a review lets those marks go, as before.
    #[test]
    fn what_a_restart_lost_is_said_then_forgotten() {
        sioul_core::config::stand_ins_only();
        const SIGNAL_1: &str = "0|org.thoughtcrime.securesms|3|null|10300";
        const SIGNAL_2: &str = "0|org.thoughtcrime.securesms|4|null|10300";
        const SHOP_KEY: &str = "0|com.shop.example|1|null|10500";
        const BLOCKED: &str = "0|com.whatsapp|9|null|10400";
        let _files = crate::everywhere::FILES.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
        crate::everywhere::files_cleared();
        Choices { people: Some(true), ..Choices::default() }.save(&Choices::default_path()).unwrap();
        let now = sioul_core::zone::now();
        let stamp = now.timestamp().as_second();
        let held = |app: &str, label: &str, kind: &str| appnotes::Held { app: app.into(), label: label.into(), kind: kind.into(), via: "messages".into(), who: "neutral".into(), conversation: "talk-noa".into(), since: stamp - 3600, until: stamp + 7200, ..Default::default() };
        let mut ledger = Ledger::default();
        ledger.held.insert(appnotes::key_hash(SIGNAL_1), held("org.thoughtcrime.securesms", "Signal", "people"));
        ledger.held.insert(appnotes::key_hash(SIGNAL_2), held("org.thoughtcrime.securesms", "Signal", "people"));
        ledger.held.insert(appnotes::key_hash(SHOP_KEY), held("com.shop.example", "Shop", "gathered"));
        ledger.held.insert(appnotes::key_hash(BLOCKED), appnotes::Held { kind: "never".into(), who: "blocked".into(), until: stamp + appnotes::FOR_GOOD, ..held("com.whatsapp", "WhatsApp", "people") });
        ledger.conversations.insert("talk-noa".into(), appnotes::SeenTalk { app: "org.thoughtcrime.securesms".into(), label: "Signal".into(), title: "Noa Martin".into(), group: false, seen: stamp });
        ledger.save(&Ledger::default_path()).unwrap();
        // The phone just started: Android holds only the shop's (its app posted it again). A review meanwhile keeps the rest.
        let _ = review_at(&json!({ "snoozed": [SHOP_KEY], "reason": "connected", "keep": true }).to_string(), &now);
        assert_eq!(Ledger::load(&Ledger::default_path()).held.len(), 4, "kept until the look");
        let looked: serde_json::Value = serde_json::from_str(&shade_at("lost", &json!({ "snoozed": [SHOP_KEY], "active": [] }).to_string(), &now)).unwrap();
        assert_eq!(looked["count"], 2, "{looked}");
        assert_eq!(looked["packages"], json!(["org.thoughtcrime.securesms"]), "{looked}");
        assert_eq!(looked["text"], appnotes::lost_line(tr(), 2, &["Signal".to_string()]), "{looked}");
        for never in ["WhatsApp", "Noa", "Shop"] {
            assert!(!looked["text"].as_str().unwrap().contains(never), "{never}: {looked}");
        }
        assert!(!looked["public"].as_str().unwrap().contains("Signal"));
        let kept = Ledger::load(&Ledger::default_path());
        assert_eq!(kept.held.keys().cloned().collect::<Vec<_>>(), [appnotes::key_hash(SHOP_KEY)], "the lost forgotten, the blocked's with them, the shop's kept");
        let again: serde_json::Value = serde_json::from_str(&shade_at("lost", &json!({ "snoozed": [SHOP_KEY], "active": [] }).to_string(), &now)).unwrap();
        assert_eq!((again["text"].as_str(), again["count"].as_u64()), (Some(""), Some(0)), "once: {again}");
        // One the phone shows (its app posted it again, its time past) was not lost either.
        ledger.save(&Ledger::default_path()).unwrap();
        let shown: serde_json::Value = serde_json::from_str(&shade_at("lost", &json!({ "snoozed": [SHOP_KEY], "active": [SIGNAL_1, SIGNAL_2] }).to_string(), &now)).unwrap();
        assert_eq!(shown["count"], 0, "{shown}");
        // Without the look pending, a review lets go of what Android no longer holds, as before.
        ledger.save(&Ledger::default_path()).unwrap();
        let _ = review_at(&json!({ "snoozed": [SHOP_KEY], "reason": "files" }).to_string(), &now);
        assert_eq!(Ledger::load(&Ledger::default_path()).held.len(), 1);
        crate::everywhere::files_cleared();
    }

    #[test]
    fn choices_set_from_the_tab() {
        let mut choices = Choices::default();
        let mut ledger = Ledger::default();
        ledger.apps.insert("com.discord".into(), appnotes::SeenApp { label: "Discord".into(), ..Default::default() });
        ledger.conversations.insert("c1".into(), appnotes::SeenTalk { app: "com.whatsapp".into(), label: "WhatsApp".into(), title: "École".into(), group: true, seen: 1 });
        set(&mut choices, &ledger, "app.com.discord.kind", &json!("automaton"));
        set(&mut choices, &ledger, "app.com.discord.area", &json!("leisure"));
        assert_eq!(choices.app("com.discord"), appnotes::AppChoice { kind: AppKind::Automaton, area: Some(Area::LEISURE), label: "Discord".into(), ..Default::default() });
        set(&mut choices, &ledger, "app.com.discord.kind", &json!("usual"));
        set(&mut choices, &ledger, "app.com.discord.area", &json!(""));
        assert!(choices.app.is_empty(), "as usual: taken out");
        set(&mut choices, &ledger, "conversation.c1", &json!("always"));
        assert_eq!(choices.conversation["c1"], appnotes::TalkChoice { through: Through::Always, title: "École".into(), app: "WhatsApp".into() });
        set(&mut choices, &ledger, "conversation.c1", &json!("usual"));
        assert!(choices.conversation.is_empty());
        set(&mut choices, &ledger, "site.forum.example.net", &json!(true));
        assert!(choices.site["forum.example.net"].at_once);
        set(&mut choices, &ledger, "site.forum.example.net", &json!(false));
        assert!(choices.site.is_empty());
        set(&mut choices, &ledger, "hold", &json!(false));
        assert!(!choices.hold);
        // People's messages held too (held apps phase 1, A2): on and off; a value that is no switch leaves it.
        set(&mut choices, &ledger, "people", &json!(false));
        assert_eq!(choices.people, Some(false));
        set(&mut choices, &ledger, "people", &json!(true));
        set(&mut choices, &ledger, "people", &json!("yes"));
        assert_eq!(choices.people, Some(true));
        // The line in the shade (B1): on unless turned off; a value that is no switch leaves it.
        assert!(choices.shade);
        set(&mut choices, &ledger, "shade", &json!(false));
        set(&mut choices, &ledger, "shade", &json!("on"));
        assert!(!choices.shade);
        set(&mut choices, &ledger, "shade", &json!(true));
        assert!(choices.shade);
        // A channel named to be made silent by Sioul; then its app's sound given back: named no more, kept.
        name_silent(&mut choices, &ledger, "com.discord", "dm");
        name_silent(&mut choices, &ledger, "com.discord", "dm");
        assert_eq!(choices.app("com.discord").silent, ["dm"]);
        assert_eq!(choices.app("com.discord").label, "Discord");
        set(&mut choices, &ledger, "app.com.discord.sound", &json!("keep"));
        assert!(choices.app("com.discord").keep_sound && choices.app("com.discord").silent.is_empty());
        set(&mut choices, &ledger, "app.com.discord.sound", &json!("silence"));
        assert!(choices.app.is_empty(), "as usual again: taken out");
    }

    /// The listener's questions on the channels Sioul makes silent itself
    /// (option 2): a shop seen gathered, its channel rung while held; linked
    /// and asked, the plan names it; its importance noted before Java changes
    /// it; noted, asked no more; the shop let through at once, given back as
    /// it was; forgotten once given back. A file that does not read: no
    /// plan, nothing noted over it. A channel rung again since, given its
    /// sound back by hand: left as set.
    #[test]
    fn the_listener_s_channel_questions() {
        sioul_core::config::stand_ins_only();
        let _files = crate::everywhere::FILES.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
        crate::everywhere::files_cleared();
        let silenced_path = appnotes::Silenced::default_path();
        let _ = std::fs::remove_file(&silenced_path);
        let stamp = jiff::Timestamp::now().as_second();
        let mut ledger = Ledger::default();
        let rang = [("offers".to_string(), appnotes::Rang { name: "Offers".into(), at: stamp, gathered: true })].into_iter().collect();
        ledger.apps.insert("com.shop.example".into(), appnotes::SeenApp { label: "Shop".into(), seen: stamp, gathered: stamp, rang, ..Default::default() });
        ledger.save(&Ledger::default_path()).unwrap();
        let on = Choices { silence: true, ..Choices::default() };
        on.save(&Choices::default_path()).unwrap();
        let ask = |verb: &str, asked: serde_json::Value| -> serde_json::Value { serde_json::from_str(&channels(verb, &asked.to_string())).unwrap() };
        assert_eq!(ask("plan", json!({ "linked": false })), json!({ "silence": [], "restore": [], "unlink": false }), "no link, nothing");
        assert_eq!(ask("plan", json!({ "linked": true }))["silence"], json!([{ "package": "com.shop.example", "channel": "offers" }]));
        assert_eq!(ask("keep", json!({ "package": "com.shop.example", "channel": "offers", "name": "Offers", "was": 4 })), json!({ "go": true }));
        assert_eq!(ask("keep", json!({ "package": "com.shop.example", "channel": "offers" })), json!({ "go": false }), "no importance, nothing noted");
        assert!(appnotes::Silenced::load(&silenced_path).unwrap().holds("com.shop.example", "offers"));
        assert_eq!(ask("plan", json!({ "linked": true }))["silence"], json!([]), "noted: asked no more");
        // The shop let through at once: given back as it was.
        let mut at_once = on.clone();
        at_once.app.insert("com.shop.example".into(), appnotes::AppChoice { kind: AppKind::AtOnce, ..Default::default() });
        at_once.save(&Choices::default_path()).unwrap();
        assert_eq!(ask("plan", json!({ "linked": true }))["restore"], json!([{ "package": "com.shop.example", "channel": "offers", "importance": 4 }]));
        ask("forget", json!({ "package": "com.shop.example", "channel": "offers" }));
        assert!(appnotes::Silenced::load(&silenced_path).unwrap().app.is_empty());
        // Turned off, nothing left: the link let go.
        Choices::default().save(&Choices::default_path()).unwrap();
        assert_eq!(ask("plan", json!({ "linked": true }))["unlink"], true);
        // Rung again on a channel Sioul made silent a while ago: given back by hand, left; within the minute, not yet.
        let mut silenced = appnotes::Silenced::default();
        silenced.keep("com.shop.example", "offers", "Offers", 4, stamp - 3600);
        silenced.keep("com.shop.example", "news", "News", 3, stamp);
        silenced.save(&silenced_path).unwrap();
        let rung = |channel: &str| Posted { package: "com.shop.example".into(), channel: channel.into(), importance: 4, ..Default::default() };
        given_back_by_hand(&rung("offers"), stamp);
        given_back_by_hand(&rung("news"), stamp);
        given_back_by_hand(&Posted { importance: 2, ..rung("news") }, stamp + 120);
        let read = appnotes::Silenced::load(&silenced_path).unwrap();
        assert!(!read.holds("com.shop.example", "offers") && read.holds("com.shop.example", "news"));
        // A file that does not read: no plan, nothing written over it.
        std::fs::write(&silenced_path, "format = [broken").unwrap();
        on.save(&Choices::default_path()).unwrap();
        assert_eq!(ask("plan", json!({ "linked": true })), json!({ "silence": [], "restore": [], "unlink": false }));
        assert_eq!(ask("keep", json!({ "package": "com.shop.example", "channel": "offers", "name": "Offers", "was": 4 })), json!({ "go": false }));
        assert_eq!(std::fs::read_to_string(&silenced_path).unwrap(), "format = [broken");
        let _ = std::fs::remove_file(&silenced_path);
        crate::everywhere::files_cleared();
    }

    /// The table of other apps by time: each app seen, by name, and an app's row
    /// set elsewhere by the name written with it; the conversations chosen for
    /// and the ten seen last, never more; each row's cells as set or as usual.
    #[test]
    fn the_table_by_time() {
        let mut ledger = Ledger::default();
        ledger.apps.insert("com.discord".into(), appnotes::SeenApp { label: "Discord".into(), ..Default::default() });
        ledger.apps.insert("org.example.news".into(), appnotes::SeenApp { label: "Actualités".into(), ..Default::default() });
        for i in 0..14 {
            ledger.conversations.insert(format!("c{i:02}"), appnotes::SeenTalk { app: "com.discord".into(), label: "Discord".into(), title: format!("Salon {i}"), group: true, seen: i });
        }
        let mut choices = Choices::default();
        choices.conversation.insert("c00".into(), appnotes::TalkChoice { through: Through::Always, ..Default::default() });
        let mut attention = sioul_core::attention::Attention::usual();
        attention.set_source("app.com.discord", sioul_core::attention::Column::Work, sioul_core::attention::Level::Now).unwrap();
        attention.set_source("app.org.example.chat", sioul_core::attention::Column::Leisure, sioul_core::attention::Level::Later).unwrap();
        attention.name_source("app.org.example.chat", "Chat");
        attention.set_source("conversation.c01", sioul_core::attention::Column::Sleep, sioul_core::attention::Level::Later).unwrap();
        let (grid, more) = by_time(&attention, &ledger, &choices);
        let ids: Vec<&str> = grid["rows"].as_array().unwrap().iter().map(|r| r["id"].as_str().unwrap()).collect();
        let labels: Vec<&str> = grid["rows"].as_array().unwrap().iter().map(|r| r["label"].as_str().unwrap()).collect();
        assert_eq!(&ids[..3], ["app.org.example.news", "app.org.example.chat", "app.com.discord"], "by name: Actualités, Chat, Discord");
        assert_eq!(&labels[..3], ["Actualités", "Chat", "Discord"]);
        // The ten seen last, and the two chosen for among the older ones.
        let talks: Vec<&str> = ids.iter().filter(|id| id.starts_with("conversation.")).copied().collect();
        assert_eq!(talks.len(), 12);
        assert!(talks.contains(&"conversation.c00") && talks.contains(&"conversation.c01") && !talks.contains(&"conversation.c02") && talks[0] == "conversation.c13");
        assert!(more, "two conversations left out");
        let discord = &grid["rows"][2];
        assert_eq!(discord["cells"][0]["value"], "now");
        assert_eq!(discord["cells"][1]["value"], "as");
        assert_eq!(discord["choices"].as_array().unwrap().len(), 4);
        assert_eq!(grid["columns"].as_array().unwrap().len(), 9);
        // Nothing seen, nothing set: no table.
        let (none, more) = by_time(&sioul_core::attention::Attention::usual(), &Ledger::default(), &Choices::default());
        assert!(none["rows"].as_array().unwrap().is_empty() && !more);
    }

    /// The listener's line in the phone's log: how many are held and how many
    /// come now, by the kind of their channel, why (the matrix's column and
    /// layers, or Always through), and the next release; never a name, a
    /// number, an app nor a conversation, whatever the ledger holds.
    #[test]
    fn the_log_line_says_kinds_counts_and_times_only() {
        let now: Zoned = "2026-10-11T21:30:00+02:00[Europe/Paris]".parse().unwrap();
        let stamp = now.timestamp().as_second();
        let held = |app: &str, label: &str, via: &str, who: &str, conversation: &str, kind: &str, until: i64| appnotes::Held { app: app.into(), label: label.into(), kind: kind.into(), via: via.into(), who: who.into(), conversation: conversation.into(), since: stamp - 3_600, until, ..Default::default() };
        let ledger = [
            held("org.example.chat", "Chat de Noa", "messages", "safe", "c0ffee12", "people", stamp + 30 * 60),
            held("org.example.chat", "Chat de Noa", "messages", "neutral", "c0ffee13", "people", stamp + 9 * 3_600 + 30 * 60),
            held("org.example.mail", "Courrier 01 99 00", "mail", "stranger", "", "people", stamp + 10 * 3_600),
            held("org.example.news", "Actualités", "", "", "", "gathered", stamp + 11 * 3_600),
            held("org.example.spam", "Bloqué", "messages", "blocked", "", "never", stamp + 7 * 86_400),
        ];
        let counted: Vec<Counted> = ledger.iter().map(|h| Counted::held(h, h.until)).collect();
        let review = log_line("review (files)", &counted, &["leisure", "dnd"], &now);
        assert_eq!(review, "Notes: review (files): held 5 (messages 3, mail 1, other apps 1); released 0; why: leisure + dnd; next release 22:00.");
        // One come back by Always through, the rest still held; the next release tomorrow (held for good aside).
        let mut released = vec![Counted { kind: "messages", until: None, why: Why::Always }];
        released.extend(counted.iter().skip(1).copied());
        let release = log_line("release", &released, &["sleep"], &now);
        assert_eq!(release, "Notes: release: held 4 (messages 2, mail 1, other apps 1); released 1 (messages 1); why: Always through; next release tomorrow 07:00.");
        // A review that lets one come at its time (a pause over): released now, the column why.
        let mut over = counted.clone();
        over[3].until = Some(stamp);
        assert!(log_line("review (files)", &over, &["leisure"], &now).contains("released 1 (other apps 1); why: leisure;"));
        // Nothing held: said so, with the columns now.
        assert_eq!(log_line("review (connected)", &[], &["work", "admin"], &now), "Notes: review (connected): held 0; released 0; why: work + admin; next release none.");
        // Never a name, a number, an app, a conversation nor who wrote.
        for line in [review, release] {
            for private in ["org.example", "Noa", "Courrier", "01 99 00", "Actualités", "Bloqué", "c0ffee", "safe", "neutral", "stranger", "blocked"] {
                assert!(!line.contains(private), "{line} says {private}");
            }
        }
        // Do-not-disturb with what holds it, Free time's "Nothing at all".
        let switch = Gated { until: None, busy: false, switch: true };
        let columns = column_words(&[Column::Free, Column::Dnd], true, Some(&switch));
        assert_eq!(columns, ["free (nothing at all)", "dnd (switch)"]);
        assert_eq!(column_words(&[Column::Leisure, Column::Dnd], true, None), ["leisure", "dnd"], "nothing at all says nothing outside Free time");
    }

    /// The listener's line at each hold (decision 2 of the overrides, 18
    /// October 2026): the kind of its channel, until when it waits or for
    /// good, and what decided it, never who: the proposal's case (a
    /// contact's text at work held while busy), the switch and both,
    /// a text named only, an automaton in the night, an app's and a
    /// conversation's own rows, what it is for, held for good, held again.
    /// Then a real hold, decided and said: no name, number, app, conversation
    /// nor list in it, whatever the notification said.
    #[test]
    fn the_hold_line_says_what_decided_it_and_names_no_one() {
        let now: Zoned = "2026-10-14T10:20:00+02:00[Europe/Paris]".parse().unwrap();
        let stamp = now.timestamp().as_second();
        let focus = Gated { until: Some(stamp + 30 * 60), busy: true, switch: false };
        let both = Gated { until: None, busy: true, switch: true };
        let line = |event: &str, kind: &str, until: Option<i64>, why: Why, cause: Cause, gated: Option<&Gated>| hold_line(event, kind, &Decision { until, why }, Some(&cause), gated, &now);
        let columns = |columns: &[Column]| Cause { columns: columns.to_vec(), ..Cause::default() };
        let in_minutes = |m: i64| Some(stamp + m * 60);
        assert_eq!(line("held", "messages", in_minutes(30), Why::Waiting, columns(&[Column::Dnd]), Some(&focus)), "Notes: held: messages 1 until 10:50; why: dnd (busy).");
        assert_eq!(line("held", "calls", in_minutes(30), Why::Waiting, columns(&[Column::Dnd]), Some(&both)), "Notes: held: calls 1 until 10:50; why: dnd (switch and busy).");
        assert_eq!(line("held", "messages", in_minutes(100), Why::Gathered, Cause { gathered: Some(Gathering::Named), ..Cause::default() }, None), "Notes: held: messages 1 until 12:00; why: gathered (a sender named only).");
        assert_eq!(line("held", "messages", in_minutes(100), Why::Gathered, Cause { gathered: Some(Gathering::Machine), ..Cause::default() }, None), "Notes: held: messages 1 until 12:00; why: gathered (a number kept for machines).");
        // A person's message by day at the gathered times (held apps phase 1, A3), with the time's column.
        assert_eq!(line("held", "messages", in_minutes(100), Why::Waiting, Cause { gathered: Some(Gathering::Time), columns: vec![Column::Leisure], ..Cause::default() }, None), "Notes: held: messages 1 until 12:00; why: gathered (messages by day), leisure.");
        assert_eq!(line("held", "other apps", Some(stamp + 22 * 3600 + 40 * 60), Why::Gathered, Cause { gathered: Some(Gathering::Automaton), columns: vec![Column::Sleep], ..Cause::default() }, None), "Notes: held: other apps 1 until tomorrow 09:00; why: gathered, sleep.");
        assert_eq!(line("held", "messages", in_minutes(400), Why::Times, Cause { own: Some(Own::App), columns: vec![Column::Work, Column::Admin], ..Cause::default() }, None), "Notes: held: messages 1 until 17:00; why: its app's row, work + admin.");
        assert_eq!(line("held", "messages", in_minutes(400), Why::Times, Cause { own: Some(Own::Conversation), gathered: None, columns: vec![Column::Work], ..Cause::default() }, None), "Notes: held: messages 1 until 17:00; why: its conversation's row, work.");
        // A person's own row (review of part 6, F6): never "its app's row", never who.
        assert_eq!(line("held", "messages", in_minutes(400), Why::Times, Cause { own: Some(Own::Person), columns: vec![Column::Work], ..Cause::default() }, None), "Notes: held: messages 1 until 17:00; why: their own row, work.");
        assert_eq!(line("held", "messages", in_minutes(100), Why::Gathered, Cause { gathered: Some(Gathering::Conversation), ..Cause::default() }, None), "Notes: held: messages 1 until 12:00; why: gathered (its conversation).");
        assert_eq!(line("held", "other apps", in_minutes(100), Why::Gathered, Cause { gathered: Some(Gathering::App), ..Cause::default() }, None), "Notes: held: other apps 1 until 12:00; why: gathered (its app).");
        assert_eq!(line("held", "mail", in_minutes(400), Why::Waiting, Cause { area: Some(Area::LEISURE), ..Cause::default() }, None), "Notes: held: mail 1 until 17:00; why: area (leisure).");
        assert_eq!(line("held", "messages", Some(stamp + 7 * 86_400), Why::Never, Cause { for_good: true, ..Cause::default() }, None), "Notes: held: messages 1 for good; why: never.");
        assert_eq!(line("held again", "messages", Some(stamp + 3 * 86_400), Why::Waiting, columns(&[Column::Pause]), None), "Notes: held again: messages 1 until 2026-10-17 10:20; why: pause.");
        assert_eq!(hold_line("held", "messages", &Decision { until: in_minutes(5), why: Why::Waiting }, None, None, &now), "Notes: held: messages 1 until 10:25; why: unknown.");
        // A real one: a contact's text at work, held while busy, decided and said as the listener does.
        struct Work;
        impl appnotes::TimeSource for Work {
            fn mode(&self, _: &Zoned) -> sioul_core::quiet::Mode {
                let week = sioul_core::areas::Week { work_hours: true, admin_hours: true, meals: true, sleep: true };
                sioul_core::quiet::Mode { quiet: false, time: sioul_core::areas::Time::Work, week, reason: sioul_core::quiet::Reason::Working, until: None, back: None, label: String::new(), here: sioul_core::areas::Here::default() }
            }
            fn starts(&self, _: &Zoned, _: &Zoned) -> Vec<Zoned> {
                Vec::new()
            }
        }
        let posted: Posted = serde_json::from_str(r#"{"key":"0|org.example.chat|7|c0ffee12|10100","package":"org.example.chat","app":"Chat de Noa","category":"msg","template":"android.app.Notification$MessagingStyle","title":"Noa Martin","shortcut":"c0ffee12","messages":[{"sender":{"name":"Noa Martin","uri":"tel:%2B33199001234"},"text":"Tu passes ce soir ?"}]}"#).unwrap();
        let choices = Choices::default();
        let kind = appnotes::classify(&posted, &choices, sioul_core::words::Words::builtin_ref());
        let attention = sioul_core::attention::Attention::usual();
        let gathered = vec!["09:00".to_string(), "12:00".into(), "15:00".into(), "18:00".into()];
        let never = |_: &Zoned| false;
        let sources = sioul_core::attention::source_rows(&posted.package, "");
        let ask = Ask { now: &now, clock: &Work, attention: &attention, gathered: &gathered, area: None, gate: Some(Gate { until: focus.until }), slot_at: &never, nothing: false, also: &[], sources: &sources, reachable: None, silenced: false, circle: false };
        let decision = appnotes::decide(&kind, Some(Who::Neutral), false, &choices, &ask);
        assert_eq!(decision.until, focus.until, "held until the session's end");
        let cause = appnotes::cause(&posted.package, &kind, Some(Who::Neutral), false, &choices, &ask);
        let said = hold_line("held", kind_word(&kind), &decision, cause.as_ref(), Some(&focus), &now);
        assert_eq!(said, "Notes: held: messages 1 until 10:50; why: dnd (busy).");
        for private in ["org.example", "Noa", "Martin", "01 99 00", "+33", "Chat", "c0ffee", "ce soir", "safe", "neutral", "stranger", "blocked", "restricted"] {
            assert!(!said.contains(private), "{said} says {private}");
        }
    }

    /// Held apps phase 1, A1: who wrote, as the listener judges them. A
    /// number none of Sioul's lists knows is a stranger's only once the
    /// phone's address book was asked and found nothing; not asked, nobody
    /// can tell (none: judged as a contact); in the phone's address book, a
    /// contact. A name in no card and nobody named: none. A held or a passed
    /// message from someone Sioul could not tell says so in the log, never a
    /// name: decided as the listener does, on a fixed clock, a bare name in a
    /// chat app at 20:30 passes, and at 23:30 waits for waking.
    #[test]
    fn a_stranger_is_a_number_nobody_knows_once_the_phone_s_book_was_asked() {
        let config = Config::default();
        let (senders, always) = (Senders::default(), People::default());
        let number = |contact: bool, asked: bool| Sender::Known { numbers: vec!["+33 1 99 00 12 34".into()], emails: Vec::new(), contact, asked };
        let who = |sender: &Sender| who_of(sender, &senders, &always, &config).0;
        assert_eq!(who(&number(false, true)), Some(Who::Stranger), "asked, found nowhere");
        assert_eq!(who(&number(false, false)), None, "not asked: nobody can tell");
        assert_eq!(who(&number(true, false)), Some(Who::Neutral), "the phone's contact");
        assert_eq!(who(&Sender::Named { name: "Noa".into(), book: false }), None);
        assert_eq!(who(&Sender::Named { name: "Noa".into(), book: true }), None);
        assert_eq!(who(&Sender::Unknown), None);
        // The lines, never a name.
        assert_eq!(pass_line("messages"), "Notes: passed: messages 1; why: sender unknown (judged as a contact).");
        let now: Zoned = "2026-10-06T23:30:00+02:00[Europe/Paris]".parse().unwrap();
        let cause = Cause { columns: vec![Column::Sleep], unknown: true, ..Cause::default() };
        let line = hold_line("held", "messages", &Decision::held(now.timestamp().as_second() + 7 * 3600 + 1800, Why::Waiting), Some(&cause), None, &now);
        assert_eq!(line, "Notes: held: messages 1 until tomorrow 07:00; why: sleep, sender unknown (judged as a contact).");
    }

    /// Held apps phase 1, A1, in the listener: a chat app's bare name that no
    /// card has, on a Tuesday evening after work, decided as a contact's: let
    /// through (a stranger's would wait), said in the log and in the tab's
    /// why as someone Sioul could not tell, never named.
    #[test]
    fn a_bare_name_is_decided_as_a_contact_s_and_said_so() {
        sioul_core::config::stand_ins_only();
        let _files = crate::everywhere::FILES.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
        crate::everywhere::files_cleared();
        let _ = std::fs::remove_file(Ledger::default_path());
        // People's messages held on this phone (held apps phase 1, A2): who wrote decides.
        Choices { people: Some(true), ..Choices::default() }.save(&Choices::default_path()).unwrap();
        // Work on Tuesdays from 09:00 to 17:00, on this test's thread alone: 20:37 is leisure.
        let dir = sioul_core::config::data_dir().join(format!("held-apps-a1-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let config = dir.join("config.toml");
        std::fs::write(&config, "language = \"en\"\n\n[[window]]\nday = \"tuesday\"\nstart = \"09:00\"\nend = \"17:00\"\n").unwrap();
        crate::backend::TEST_CONFIG.with(|own| *own.borrow_mut() = Some(config.clone()));
        const SIGNAL: &str = r#"{"key":"0|org.thoughtcrime.securesms|3|null|10300","package":"org.thoughtcrime.securesms","app":"Signal","category":"msg","template":"android.app.Notification$MessagingStyle","title":"Noa","shortcut":"7","is_conversation":true,"messages":[{"sender":{"name":"Noa","key":"7"},"text":"Tu dors ?"}]}"#;
        let evening: Zoned = "2026-10-06T20:37:00+02:00[Europe/Paris]".parse().unwrap();
        let decided: serde_json::Value = serde_json::from_str(&decide_at(SIGNAL, &evening)).unwrap();
        assert_eq!((decided["hold"].as_i64(), decided["why"].as_str()), (Some(0), Some("allowed")), "{decided}");
        assert_eq!(decided["line"], "Notes: passed: messages 1; why: sender unknown (judged as a contact).", "{decided}");
        assert_eq!(Ledger::load(&Ledger::default_path()).apps["org.thoughtcrime.securesms"].why, UNKNOWN);
        crate::backend::TEST_CONFIG.with(|own| *own.borrow_mut() = None);
        let _ = std::fs::remove_dir_all(&dir);
        let _ = std::fs::remove_file(Ledger::default_path());
        let _ = std::fs::remove_file(Choices::default_path());
        crate::everywhere::files_cleared();
    }

    /// Held apps phase 1, A2: people's messages worked out once and written,
    /// under the choices' lock: a phone where nothing was ever chosen for
    /// other apps stops holding them, one where an app was keeps holding
    /// them, and a choice made later never moves it; the tab shows it and
    /// sets it; a file that does not read is never written over.
    #[test]
    fn people_unset_is_worked_out_once_and_written() {
        sioul_core::config::stand_ins_only();
        let _files = crate::everywhere::FILES.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
        // A configuration of this test's own, with no row of its own for any app.
        let dir = sioul_core::config::data_dir().join(format!("held-apps-a2-people-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("config.toml"), "language = \"en\"\n").unwrap();
        crate::backend::TEST_CONFIG.with(|own| *own.borrow_mut() = Some(dir.join("config.toml")));
        let path = Choices::default_path();
        let usual = sioul_core::attention::Attention::usual();
        let _ = std::fs::remove_file(&path);
        assert_eq!(settled_choices(&usual, true).people, Some(false), "nothing chosen here");
        assert!(std::fs::read_to_string(&path).unwrap().contains("people = false"), "written");
        // An app chosen before this version: held as before, written so.
        std::fs::write(&path, "[app.\"com.discord\"]\nkind = \"automaton\"\n").unwrap();
        assert_eq!(settled_choices(&usual, true).people, Some(true));
        assert!(std::fs::read_to_string(&path).unwrap().contains("people = true"));
        // Worked out once: an app chosen later never moves it.
        std::fs::write(&path, "people = false\n").unwrap();
        change("set", &json!({ "key": "app.com.discord.kind", "value": "automaton" }).to_string());
        assert_eq!(Choices::load(&path).people, Some(false));
        // Unset, the first change works it out from what was chosen before it.
        std::fs::write(&path, "hold = true\n").unwrap();
        change("set", &json!({ "key": "site.forum.example.net", "value": true }).to_string());
        assert_eq!(Choices::load(&path).people, Some(false), "the site was chosen after");
        // Set from the tab, and shown there.
        change("set", &json!({ "key": "people", "value": true }).to_string());
        assert_eq!(Choices::load(&path).people, Some(true));
        let shown: Value = serde_json::from_str(&setup()).unwrap();
        assert_eq!((shown["people"]["key"].as_str(), shown["people"]["kind"].as_str(), shown["people"]["value"].as_bool()), (Some("people"), Some("bool"), Some(true)));
        for said in [&shown["people"]["label"], &shown["people"]["help"]] {
            assert!(!said.as_str().unwrap().starts_with("appnotes-"), "{said}");
        }
        // A file that does not read: worked out in memory, never written over.
        std::fs::write(&path, "hold = [broken").unwrap();
        assert_eq!(settled_choices(&usual, true).people, Some(false));
        assert_eq!(std::fs::read_to_string(&path).unwrap(), "hold = [broken");
        let _ = std::fs::remove_file(&path);
        crate::backend::TEST_CONFIG.with(|own| *own.borrow_mut() = None);
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// Held apps phase 1, A2, in the listener: people's messages not held on
    /// this phone. A stranger's text on a Tuesday evening comes at once, said
    /// "people-off", with no line of a hold; what was held of a person before
    /// comes at the next review, the log saying "people's messages not held".
    #[test]
    fn with_people_off_the_listener_lets_people_through() {
        sioul_core::config::stand_ins_only();
        let _files = crate::everywhere::FILES.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
        crate::everywhere::files_cleared();
        let _ = std::fs::remove_file(Ledger::default_path());
        Choices { people: Some(false), ..Choices::default() }.save(&Choices::default_path()).unwrap();
        let dir = sioul_core::config::data_dir().join(format!("held-apps-a2-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let config = dir.join("config.toml");
        std::fs::write(&config, "language = \"en\"\n\n[[window]]\nday = \"tuesday\"\nstart = \"09:00\"\nend = \"17:00\"\n").unwrap();
        crate::backend::TEST_CONFIG.with(|own| *own.borrow_mut() = Some(config.clone()));
        const TEXT: &str = r#"{"key":"0|com.google.android.apps.messaging|5|sms|10001","package":"com.google.android.apps.messaging","app":"Messages","category":"msg","template":"android.app.Notification$MessagingStyle","title":"+33 1 99 00 12 35","messages":[{"sender":{"name":"+33 1 99 00 12 35"},"text":"Bonsoir"}],"sms_app":true,"contacts":true}"#;
        const PEOPLE: &str = "0|com.google.android.apps.messaging|2|sms|10001";
        let evening: Zoned = "2026-10-06T20:41:00+02:00[Europe/Paris]".parse().unwrap();
        let stamp = evening.timestamp().as_second();
        let decided: serde_json::Value = serde_json::from_str(&decide_at(TEXT, &evening)).unwrap();
        assert_eq!((decided["hold"].as_i64(), decided["why"].as_str()), (Some(0), Some("people-off")), "{decided}");
        assert!(decided.get("line").is_none(), "{decided}");
        let mut ledger = Ledger::default();
        ledger.held.insert(appnotes::key_hash(PEOPLE), appnotes::Held { app: "com.google.android.apps.messaging".into(), label: "Messages".into(), kind: "people".into(), via: "messages".into(), who: "stranger".into(), since: stamp - 3600, until: stamp + 12 * 3600, ..Default::default() });
        ledger.save(&Ledger::default_path()).unwrap();
        let reviewed: serde_json::Value = serde_json::from_str(&review_at(&json!({ "snoozed": [PEOPLE], "reason": "files" }).to_string(), &evening)).unwrap();
        // Quiet, as a person's (held apps phase 1, B2): it comes back in Sioul's window, and is not told, since
        // turning people's messages off was your doing on this phone (`appnotes::waited`).
        assert_eq!(reviewed["snooze"], json!([{ "key": PEOPLE, "ms": 1000, "people": true, "quiet": true }]), "{reviewed}");
        assert!(reviewed["line"].as_str().unwrap().contains("released 1 (messages 1); why: people's messages not held;"), "{reviewed}");
        crate::backend::TEST_CONFIG.with(|own| *own.borrow_mut() = None);
        let _ = std::fs::remove_dir_all(&dir);
        let _ = std::fs::remove_file(Ledger::default_path());
        let _ = std::fs::remove_file(Choices::default_path());
        crate::everywhere::files_cleared();
    }

    /// The text of 24 November 2026 (its shape, an invented code, amount and
    /// shop), from a short number on a Tuesday evening, the phone's
    /// do-not-disturb holding it (`intercepted`): let through at once, with
    /// Sioul's own notice of it to do, passing Sioul's modes, its code ready
    /// to copy, its lock screen showing its title alone, never the code, nor
    /// the short number (it may hold a code: the app's name instead). Not
    /// silenced: let through, no notice of Sioul's (the app's own sounds).
    #[test]
    fn a_text_s_code_comes_at_once_and_sounds_through_do_not_disturb() {
        sioul_core::config::stand_ins_only();
        let _files = crate::everywhere::FILES.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
        crate::everywhere::files_cleared();
        let _ = std::fs::remove_file(Ledger::default_path());
        let dir = sioul_core::config::data_dir().join(format!("codes-through-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let config = dir.join("config.toml");
        // The window in English, the words read in French and English (Settings ▸ Words).
        std::fs::write(&config, "language = \"en\"\n\n[words]\nlanguages = [\"fr\", \"en\"]\n\n[[window]]\nday = \"tuesday\"\nstart = \"09:00\"\nend = \"17:00\"\n").unwrap();
        crate::backend::TEST_CONFIG.with(|own| *own.borrow_mut() = Some(config.clone()));
        const PAYMENT: &str = r#"{"key":"0|foundation.e.message|7|null|10123","package":"foundation.e.message","app":"Message","channel":"notifications_default","category":"msg","title":"36123","text":"ExamplePay: 482913 est le code de votre transaction de 23,90 EUR chez Boutique Exemple. Gardez ce code pour vous","sms_app":true,"intercepted":true,"importance":4,"alerted":true}"#;
        let evening: Zoned = "2026-10-06T21:40:00+02:00[Europe/Paris]".parse().unwrap();
        let decided: serde_json::Value = serde_json::from_str(&decide_at(PAYMENT, &evening)).unwrap();
        assert_eq!((decided["hold"].as_i64(), decided["why"].as_str()), (Some(0), Some("code")), "{decided}");
        let code = &decided["code"];
        assert_eq!(code["tag"], json!(format!("code:{}", appnotes::key_hash("0|foundation.e.message|7|null|10123"))), "{decided}");
        assert_eq!((code["class"].as_str(), code["pierce"].as_bool(), code["kind"].as_str()), (Some("actionable"), Some(true), Some("codes")), "{code}");
        // In the window's language, as this process read it once (`backend::tr`): not always the test configuration's.
        let title = crate::backend::say("appnotes-code-title-app", &[("app", "Message".to_string())]);
        assert_eq!(code["title"], json!(title), "the short number may hold a code: the app's name");
        assert_eq!(code["body"], json!("482913"));
        assert_eq!(code["actions"], json!([{ "label": tr().text("notify-copy", None), "copy": "482913" }]));
        assert_eq!(code["public"], json!({ "title": title, "body": "" }), "never the code on the lock screen");
        assert!(code["ends"].as_i64().is_some_and(|ends| ends > 0), "{code}");
        // Not silenced by do-not-disturb: the app's own notification sounds; nothing of Sioul's.
        let heard = PAYMENT.replace("\"intercepted\":true", "\"intercepted\":false");
        let decided: serde_json::Value = serde_json::from_str(&decide_at(&heard, &evening)).unwrap();
        assert_eq!((decided["hold"].as_i64(), decided["why"].as_str()), (Some(0), Some("code")), "{decided}");
        assert!(decided.get("code").is_none(), "{decided}");
        // Posted again three hours after its message (a restart): its code is over, nothing of Sioul's;
        // half an hour's code an hour after its message neither; ten minutes after, its notice, lasting
        // from the message's time (should-fix 2, d).
        let at = |minutes: i64| (evening.timestamp().as_second() - minutes * 60) * 1000;
        for (minutes, said) in [(180, false), (60, false), (10, true)] {
            let late = PAYMENT.replace("\"intercepted\":true", &format!("\"intercepted\":true,\"when\":{}", at(minutes)));
            let decided: serde_json::Value = serde_json::from_str(&decide_at(&late, &evening)).unwrap();
            assert_eq!(decided.get("code").is_some(), said, "{minutes} minutes: {decided}");
            if said {
                assert_eq!(decided["code"]["ends"].as_i64(), Some(at(minutes) + 30 * 60_000), "{decided}");
            }
        }
        // The same sender's next text, no code, while the code is unread (second review of the codes, fix 2): the
        // notification that shows the code a minute old stays let through, nothing more told; three hours old, the
        // code is over and the conversation is held as any automaton's.
        let code_text = "ExamplePay: 482913 est le code de votre transaction de 23,90 EUR chez Boutique Exemple. Gardez ce code pour vous";
        let receipt = "Votre paiement de 23,90 EUR a été accepté.";
        for (minutes, through) in [(1, true), (180, false)] {
            let two = json!({
                "key": "0|foundation.e.message|7|null|10123", "package": "foundation.e.message", "app": "Message", "channel": "notifications_default",
                "category": "msg", "template": "android.app.Notification$MessagingStyle", "title": "36123", "text": receipt, "sms_app": true, "intercepted": true,
                "messages": [{ "text": code_text, "sender": { "name": "36123" }, "at": at(minutes) }, { "text": receipt, "sender": { "name": "36123" }, "at": at(0) }],
            })
            .to_string();
            let decided: serde_json::Value = serde_json::from_str(&decide_at(&two, &evening)).unwrap();
            assert_eq!(decided["hold"].as_i64() == Some(0), through, "{minutes} minutes: {decided}");
            assert!(decided.get("code").is_none(), "nothing more told: {decided}");
        }
        crate::backend::TEST_CONFIG.with(|own| *own.borrow_mut() = None);
        let _ = std::fs::remove_dir_all(&dir);
        let _ = std::fs::remove_file(Ledger::default_path());
        crate::everywhere::files_cleared();
    }

    /// Review of 25 November 2026, must-fix 3: a blocked number's text worded
    /// as a code gains nothing of Sioul's: under do-not-disturb, no notice of
    /// Sioul's own; no line for the computers (the decision handed to
    /// `phonemsgs::note` names them blocked). Its app's own notification is
    /// let through as before (the owner's decision 4).
    #[test]
    fn a_blocked_number_s_code_gains_nothing() {
        sioul_core::config::stand_ins_only();
        let _files = crate::everywhere::FILES.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
        crate::everywhere::files_cleared();
        let _ = std::fs::remove_file(Ledger::default_path());
        let dir = sioul_core::config::data_dir().join(format!("codes-blocked-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let blocked = dir.join("blocked-senders.txt");
        std::fs::write(&blocked, "tel:+33199001234\n").unwrap();
        let config = dir.join("config.toml");
        std::fs::write(&config, format!("language = \"en\"\nblocked_senders = \"{}\"\n\n[words]\nlanguages = [\"fr\", \"en\"]\n\n[[window]]\nday = \"tuesday\"\nstart = \"09:00\"\nend = \"17:00\"\n", blocked.display())).unwrap();
        crate::backend::TEST_CONFIG.with(|own| *own.borrow_mut() = Some(config.clone()));
        const BLOCKED: &str = r#"{"key":"0|foundation.e.message|9|null|10123","package":"foundation.e.message","app":"Message","category":"msg","title":"+33 1 99 00 12 34","text":"Code : 4821","sms_app":true,"contacts":true,"intercepted":true,"importance":4,"alerted":true}"#;
        let evening: Zoned = "2026-10-06T21:40:00+02:00[Europe/Paris]".parse().unwrap();
        let decided: serde_json::Value = serde_json::from_str(&decide_at(BLOCKED, &evening)).unwrap();
        assert_eq!((decided["hold"].as_i64(), decided["why"].as_str()), (Some(0), Some("code")), "{decided}");
        assert!(decided.get("code").is_none(), "no notice of Sioul's own for a blocked number: {decided}");
        // The same words from a number nobody blocked: Sioul's notice, through do-not-disturb.
        let free = BLOCKED.replace("+33 1 99 00 12 34", "+33 1 99 00 56 78");
        let decided: serde_json::Value = serde_json::from_str(&decide_at(&free, &evening)).unwrap();
        assert_eq!(decided["code"]["body"], json!("4821"), "{decided}");
        // And `phonemsgs::note`'s rule: no line of a blocked sender's.
        let posted: Posted = serde_json::from_str(BLOCKED).unwrap();
        let words = sioul_core::words::Words::of(&load_config());
        let notice = sioul_core::phonemsgs::Notice { posted: &posted, extra: &sioul_core::phonemsgs::Extra::default(), kind: &Kind::Code, who: Some(Who::Blocked), circle: false, listed: false, decision: &Decision::through(Why::Code), send: sioul_core::phonemsgs::Send::Words, now_ms: evening.timestamp().as_millisecond(), region: None, words: &words };
        assert!(sioul_core::phonemsgs::lines_of(&notice).is_empty());
        crate::backend::TEST_CONFIG.with(|own| *own.borrow_mut() = None);
        let _ = std::fs::remove_dir_all(&dir);
        let _ = std::fs::remove_file(Ledger::default_path());
        crate::everywhere::files_cleared();
    }

    /// A text from a root ARCEP keeps for machines is an automaton's, while
    /// the call screening sends automated callers to voicemail: 01 63, a
    /// root for machines that ARCEP had attributed to no operator yet (so the
    /// number is nobody's); a fiction number is no machine's. The numbers'
    /// country set, as a French phone's language says it (with no locale, as
    /// on CI, it would be Britain's, where 01 63 is no machine's).
    #[test]
    fn a_machine_s_text_is_an_automaton_s() {
        let now: Zoned = "2026-10-20T10:00:00+02:00[Europe/Paris]".parse().unwrap();
        let config: Config = toml::from_str("[contacts]\nregion = \"FR\"\n").unwrap();
        let machine = Sender::Known { numbers: vec!["01 63 12 34 56".into()], emails: Vec::new(), contact: false, asked: false };
        assert!(machine_number(&machine, &config, &now));
        assert!(!machine_number(&Sender::Known { numbers: vec!["01 99 00 12 34".into()], emails: Vec::new(), contact: false, asked: false }, &config, &now));
        assert!(!machine_number(&Sender::Named { name: "AMELI".into(), book: true }, &config, &now));
        let as_unknown: Config = toml::from_str("[contacts]\nregion = \"FR\"\n[screening]\nautomated = \"unknown\"\n").unwrap();
        assert!(!machine_number(&machine, &as_unknown, &now), "set as any number nobody knows");
    }

    #[test]
    fn nothing_reaches_java_on_a_computer() {
        assert_eq!(java("access", "{}"), Value::Null);
        assert_eq!(access_here(), None, "a computer says nothing of a phone's access");
        assert_eq!(times_in_words(&["18:00".into(), "9:00".into(), "13:00".into()]), format!("09:00, 13:00 {} 18:00", tr().text("word-and", None)));
    }
}
