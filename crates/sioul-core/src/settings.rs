// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

//! Settings, shown where they apply. Each page has its own few, behind one
//! small button, each with a sentence saying what it changes; there is no long
//! preferences window (docs/client.md, "Calm first"). What is set here is
//! written to the configuration file in place, its comments kept
//! (`config::set_value`), or to the senders' lists.

use crate::config::{Config, SettingValue, WindowValue, set_value};
use crate::i18n::Translator;
use crate::porch::SenderList;
use serde::Serialize;
use std::path::Path;

/// How a setting is edited.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Kind {
    /// On or off.
    Bool,
    /// A whole number, between `min` and `max`.
    Int,
    /// A number with decimals, between `min` and `max` by `step`.
    Float,
    /// One of `choices`.
    Choice,
    /// Some of `choices`, each ticked or not: `value` lists those ticked.
    Picks,
    Text,
    /// Several lines.
    Long,
    /// A folder on disk.
    Folder,
    /// A font family.
    Font,
    /// Minutes for each weekday, Monday first.
    Week,
    /// Hours for each day of the week: on or off, from a start to an end.
    Windows,
    /// Days off: from a day to another.
    #[serde(rename = "timeoff")]
    TimeOff,
    /// Addresses and @domains, one per row.
    Senders,
    /// Words, one per row.
    Words,
    /// What a source is for: work, your admin, leisure, any of them together ("admin+leisure").
    Areas,
    /// A case's routes: domains, addresses, words in the subject or the text.
    Routes,
    /// Kinds of task: each renamed in place or taken away; new ones added.
    Kinds,
    /// Categories in use: each renamed or taken off every task that has it.
    Categories,
    /// Task lists, calendars, address books: renamed in place, deleted when empty.
    Collections,
    /// Nothing to change: what a lane holds (`label`) and how mail lands there (`help`, a line each).
    Note,
    /// A secret kept in the system keyring, typed and never shown: `value` says whether one is kept.
    Secret,
    /// Nothing to change here: a button to where it is changed (`value`: "needs", the Health page's meals and sleep).
    Link,
    /// Round buttons in a grid: `rows` down, `choices` across, one per row;
    /// `value` lists each row's as "row:column"; saved a row at a time
    /// (`<key>.<row>`): what Sioul's own spam filter does with each verdict.
    Radios,
    /// Sioul's own spam filter, the window's own block (`SpamFilter.qml`): on
    /// a computer, "Train now" and what the last training found; on a phone,
    /// where its table comes from. Nothing in `value`.
    Spam,
    /// The mail filters, the window's own block (`MailFilters.qml`): the
    /// list, each in words, its editor, "Try it" and running them on the
    /// inboxes; read and written whole (`config::set_filters`). Nothing in `value`.
    Filters,
}

/// One of a setting's choices.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Choice {
    pub value: SettingValue,
    pub label: String,
}

/// One setting, as its page shows it.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Setting {
    /// Where it is written: `history_weeks`, `reading.size`, `account.<id>.shield`, `window`, `known`, `blocked`.
    pub key: String,
    pub kind: Kind,
    pub label: String,
    /// What it changes, in a sentence.
    pub help: String,
    pub value: SettingValue,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub choices: Vec<Choice>,
    /// A grid's rows (`Kind::Radios`); its columns are the choices.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub rows: Vec<Choice>,
    pub min: f64,
    pub max: f64,
    pub step: f64,
    /// "min", "px": said after the number.
    pub unit: String,
    /// A heading over a group of settings: an account's address.
    pub group: String,
    /// The tab it is in, on a page with tabs (Settings): "look", "hours", "reminders", "files", "invoices".
    #[serde(skip_serializing_if = "String::is_empty")]
    pub section: String,
}

struct Builder<'a> {
    tr: &'a Translator,
    out: Vec<Setting>,
    group: String,
    section: String,
}

impl Builder<'_> {
    /// `id` names the label (`set-<id>`) and the sentence (`set-<id>-help`).
    fn push(&mut self, key: &str, id: &str, kind: Kind, value: SettingValue) -> &mut Setting {
        let label = self.tr.text(&format!("set-{id}"), None);
        let help = self.tr.text(&format!("set-{id}-help"), None);
        self.out.push(Setting { key: key.to_string(), kind, label, help, value, choices: Vec::new(), rows: Vec::new(), min: 0.0, max: 0.0, step: 1.0, unit: String::new(), group: self.group.clone(), section: self.section.clone() });
        let last = self.out.len() - 1;
        &mut self.out[last]
    }

    /// The collections of a kind that can be written, to rename or delete,
    /// named "account/id"; none, no setting.
    fn collections(&mut self, config: &Config, id: &str, kind: crate::vdir::Kind, holding: &str) {
        use crate::capabilities::{Provider, provider_of_collection};
        let named: Vec<crate::config::NamedValue> = crate::vdir::collections(kind)
            .into_iter()
            .filter(|c| !c.read_only && (holding.is_empty() || c.holds(holding)))
            .map(|c| crate::config::NamedValue {
                id: format!("{}/{}/{}", if kind == crate::vdir::Kind::Contacts { "contacts" } else { "calendars" }, c.account, c.id),
                // Google's calendars and address books change on its own pages only.
                locked: if provider_of_collection(config.account(&c.account), &c) == Provider::Google { self.tr.text("google-fixed-collection", None) } else { String::new() },
                detail: { let place = c.place(config); if place.is_empty() { self.tr.text("collection-here", None) } else { place } },
                label: c.name,
            })
            .collect();
        if !named.is_empty() {
            self.push("collections", id, Kind::Collections, SettingValue::Named(named));
        }
    }

    /// The key for the AI, when an address allows it; whether one is kept is the window's to say.
    fn ai_key(&mut self, config: &Config) {
        if config.accounts.iter().any(|a| a.shield && a.shield_ai) {
            self.push("ai_key", "ai-key", Kind::Secret, SettingValue::Bool(false));
        }
    }

    /// A row whose words are given rather than found by an id (the words tab's).
    fn said(&mut self, key: &str, kind: Kind, label: String, help: String, value: SettingValue) -> &mut Setting {
        self.out.push(Setting { key: key.to_string(), kind, label, help, value, choices: Vec::new(), rows: Vec::new(), min: 0.0, max: 0.0, step: 1.0, unit: String::new(), group: self.group.clone(), section: self.section.clone() });
        let last = self.out.len() - 1;
        &mut self.out[last]
    }

    /// A sentence to read, nothing to change.
    fn note(&mut self, about: String, lines: Vec<String>) {
        let key = format!("note.{}", self.out.len());
        self.out.push(Setting { key, kind: Kind::Note, label: about, help: lines.join("\n"), value: SettingValue::Text(String::new()), choices: Vec::new(), rows: Vec::new(), min: 0.0, max: 0.0, step: 1.0, unit: String::new(), group: self.group.clone(), section: self.section.clone() });
    }

    fn range(setting: &mut Setting, min: f64, max: f64, step: f64, unit: &str) {
        setting.min = min;
        setting.max = max;
        setting.step = step;
        setting.unit = unit.to_string();
    }
}

/// The weeks of history a setting offers, and how each reads.
fn history_choices(tr: &Translator, with_default: bool) -> Vec<Choice> {
    let mut out = Vec::new();
    if with_default {
        out.push(Choice { value: SettingValue::Int(-1), label: tr.text("set-like-others", None) });
    }
    for weeks in [1, 2, 4, 13, 26, 52, 0] {
        out.push(Choice { value: SettingValue::Int(weeks), label: tr.text(&format!("history-{weeks}"), None) });
    }
    out
}

/// The settings of one page: "porch", "mail", "agenda", "tasks", "notes",
/// "reading", "contacts", "time", "general". `lists` names the task lists
/// a new task can go into, as ("account/id", name).
pub fn for_view(view: &str, config: &Config, tr: &Translator, lists: &[(String, String)], store: Option<&crate::cases::CaseStore>) -> Vec<Setting> {
    let mut b = Builder { tr, out: Vec::new(), group: String::new(), section: String::new() };
    let int = |n: Option<u32>, default: u32| SettingValue::Int(i64::from(n.unwrap_or(default)));
    let known = || SettingValue::Texts(SenderList::load(&config.known_senders_path()).entries());
    // The Porch's lanes: what the Porch itself decides of where their mail
    // lands. The rest belongs to its owner, said in the lane's rules: an
    // address's rank and shield in Accounts, a project's routes in Projects,
    // who is blocked in Settings.
    if let Some(lane) = view.strip_prefix("lane:") {
        match lane {
            "people" | "screener" => {
                b.push("known", "known", Kind::Senders, known());
            }
            // The automatic senders' words are in Settings ▸ Words, with the others (docs/words.md).
            "filed" => {
                b.push("link.words.senders", "filed-words", Kind::Link, SettingValue::Text("settings:words.senders.automatic".into()));
            }
            _ => {}
        }
        return b.out;
    }
    // A project's routes: the mail that belongs to it, on its page in Projects.
    if let Some(id) = view.strip_prefix("project:") {
        if let Some(case) = store.and_then(|s| s.get(id)) {
            let routes = case.routes.iter().map(crate::cases::RouteValue::from).collect();
            b.push(&format!("case.{}.routes", case.id), "routes", Kind::Routes, SettingValue::Routes(routes));
        }
        return b.out;
    }
    // One address's own settings, on its card in Accounts (its rank, name and
    // signature are on the card itself): what it is for, how much of it, how
    // often its folders are fetched, its shield.
    if let Some(id) = view.strip_prefix("account:") {
        if let Some(account) = config.account(id).filter(|a| a.kind == crate::config::AccountKind::Imap) {
            let key = |field: &str| format!("account.{}.{field}", account.id);
            // Unsaid: work's, so that it never reaches your evenings.
            let area = account.area.as_deref().and_then(crate::areas::Area::parse).unwrap_or(crate::areas::Area::WORK);
            b.push(&key("area"), "account-area", Kind::Areas, SettingValue::Text(area.id()));
            let own = raw_account_history(config, &account.id);
            let s = b.push(&key("history_weeks"), "account-history", Kind::Choice, SettingValue::Int(own.map_or(-1, i64::from)));
            s.choices = history_choices(tr, true);
            let s = b.push(&key("fetch_minutes"), "account-fetch", Kind::Int, SettingValue::Int(account.fetch_minutes.map_or(0, i64::from)));
            Builder::range(s, 0.0, 1440.0, 5.0, "min");
            b.push(&key("shield"), "account-shield", Kind::Bool, SettingValue::Bool(account.shield));
            if account.shield {
                b.push(&key("shield_ai"), "account-shield-ai", Kind::Bool, SettingValue::Bool(account.shield_ai));
            }
        }
        return b.out;
    }
    match view {
        "porch" => {
            // Working hours and days off are Sioul's as a whole: in the settings page.
            b.note(tr.text("set-hours-elsewhere", None), Vec::new());
            // Which projects have their lane here; each is made, renamed and routed in Projects.
            if let Some(store) = store.filter(|s| !s.cases.is_empty()) {
                b.group = tr.text("set-projects-group", None);
                let shown = store.cases.iter().filter(|c| !config.porch.hidden_projects.contains(&c.id)).map(|c| c.id.clone()).collect();
                let s = b.push("porch.projects", "porch-projects", Kind::Picks, SettingValue::Texts(shown));
                s.choices = store.cases.iter().map(|c| Choice { value: SettingValue::Text(c.id.clone()), label: c.title.clone() }).collect();
            }
            // Paper letters: the folder their scans arrive in.
            b.group = tr.text("set-letters-group", None);
            let inbox = config.letters.inbox.clone().unwrap_or_else(|| config.case_store_path().map(|r| crate::letters::Letters::folder(&r).join("inbox").display().to_string()).unwrap_or_default());
            b.push("letters.inbox", "letters-inbox", Kind::Folder, SettingValue::Text(inbox));
            // The calls your phones declined, on the Porch of every device (docs/porch.md, "Calls declined").
            b.group = tr.text("set-calls-group", None);
            b.push("porch.calls", "porch-calls", Kind::Bool, SettingValue::Bool(config.porch.calls));
            // How a message opened here reads: the reading panel's own, shown where messages are read.
            b.group = tr.text("ui-reading", None);
            reading(&mut b, config);
            // Every lane, empty or not, in the order mail is sorted, each with how
            // mail lands there and what the Porch itself decides of it; a
            // project's lane is in its page.
            b.group = tr.text("set-sorting", None);
            b.note(tr.text("rule-order", None), Vec::new());
            for lane in crate::view::lanes(config, store, tr).into_iter().filter(|l| !l.key.starts_with("case:")) {
                b.group = lane.title.clone();
                b.note(lane.about, lane.rules);
                for mut setting in for_view(&format!("lane:{}", lane.key), config, tr, lists, store) {
                    if b.out.iter().any(|s| s.key == setting.key) {
                        continue;
                    }
                    setting.group = lane.title.clone();
                    b.out.push(setting);
                }
            }
        }
        "mail" => {
            b.push("mail.threads", "mail-threads", Kind::Bool, SettingValue::Bool(config.mail.threads));
            let s = b.push("fetch_minutes", "fetch-minutes", Kind::Int, int(config.fetch_minutes, crate::config::DEFAULT_FETCH_MINUTES));
            Builder::range(s, 1.0, 240.0, 1.0, "min");
            // Each address's own: in Accounts, on its card.
            b.note(tr.text("set-accounts-elsewhere", None), Vec::new());
            // The mail filters: one list for every address, their own block.
            b.group = tr.text("set-filters-group", None);
            b.push("mail.filter", "filters", Kind::Filters, SettingValue::Text(String::new()));
            // Sioul's own spam filter: what it does with each verdict (the
            // matrix), how sure it must be.
            b.group = tr.text("set-spam-group", None);
            let actions = config.spam.actions();
            let cells = crate::spam::Class::ALL.iter().map(|c| format!("{}:{}", c.as_str(), actions.of(*c).as_str())).collect();
            let s = b.push("spam.actions", "spam-actions", Kind::Radios, SettingValue::Texts(cells));
            s.rows = crate::spam::Class::ALL.iter().map(|c| Choice { value: SettingValue::Text(c.as_str().into()), label: tr.text(&format!("set-spam-class-{}", c.as_str()), None) }).collect();
            s.choices = crate::spam::Action::ALL.iter().map(|a| Choice { value: SettingValue::Text(a.as_str().into()), label: tr.text(&format!("set-spam-action-{}", a.as_str()), None) }).collect();
            let (spam, unsure) = config.spam.thresholds();
            let hundredths = |t: f32| (f64::from(t) * 100.0).round() / 100.0;
            // Shown as percentages ("%"); each stops a point short of the other: unsure stays below spam.
            let s = b.push("spam.threshold_spam", "spam-threshold", Kind::Float, SettingValue::Float(hundredths(spam)));
            Builder::range(s, (hundredths(unsure) + 0.01).max(0.5), 0.99, 0.01, "%");
            let s = b.push("spam.threshold_unsure", "spam-unsure", Kind::Float, SettingValue::Float(hundredths(unsure)));
            Builder::range(s, 0.05, (hundredths(spam) - 0.01).min(0.95), 0.01, "%");
            // Its training on a computer, where its table comes from on a phone.
            b.push("spam.filter", if cfg!(target_os = "android") { "spam-filter-phone" } else { "spam-filter" }, Kind::Spam, SettingValue::Text(String::new()));
            // How a message reads: the reading panel's own ("Aa" in Notes), shown where messages are read.
            b.group = tr.text("ui-reading", None);
            reading(&mut b, config);
            // The lists you left from a message, newest first: kept on this device (`unsubscribe::Record`).
            let mut left = crate::unsubscribe::Record::load(&crate::unsubscribe::Record::default_path()).lists;
            if !left.is_empty() {
                left.sort_by_key(|l| std::cmp::Reverse(l.at));
                let zone = jiff::tz::TimeZone::system();
                let today = jiff::Timestamp::now().to_zoned(zone.clone()).date();
                let lines = left
                    .iter()
                    .map(|l| {
                        let mut args = crate::i18n::args();
                        args.set("list", l.name.clone());
                        args.set("date", jiff::Timestamp::from_second(l.at).map(|t| tr.day_in(t.to_zoned(zone.clone()).date(), today)).unwrap_or_default());
                        args.set("way", tr.text(&format!("unsubscribe-way-{}", l.way), None));
                        tr.text("unsubscribed-line", Some(&args))
                    })
                    .collect();
                b.group = tr.text("unsubscribed-group", None);
                b.note(tr.text("unsubscribed-note", None), lines);
            }
        }
        // Who is on which list, under Settings ▸ What reaches you ▸ By person
        // (its grids say when each list reaches you): the four lists; the
        // people placed on them; your contacts' categories, each on a list or none.
        "senders" => {
            for (key, id, path) in [
                ("safe", "safe", config.safe_senders_path()),
                ("neutral", "neutral", config.neutral_senders_path()),
                ("restricted", "restricted", config.restricted_senders_path()),
                ("blocked", "blocked-all", config.blocked_senders_path()),
            ] {
                // Addresses and patterns, then numbers as written.
                let list = SenderList::load(&path);
                b.push(key, id, Kind::Senders, SettingValue::Texts(list.entries().into_iter().chain(list.numbers().iter().cloned()).collect()));
            }
            let senders = crate::porch::Senders::load(config);
            let choices = |none: &str| -> Vec<Choice> {
                std::iter::once(Choice { value: SettingValue::Text(String::new()), label: tr.text(none, None) })
                    .chain(crate::porch::Standing::ALL.iter().map(|l| Choice { value: SettingValue::Text(l.as_str().into()), label: tr.text(&format!("sender-one-{}", l.as_str()), None) }))
                    .collect()
            };
            // The people placed on a list themselves (their card): changed here or on their card.
            let mut cards: Vec<String> = Vec::new();
            for list in [&senders.safe, &senders.neutral, &senders.restricted, &senders.blocked] {
                for uid in list.cards() {
                    if !cards.contains(uid) {
                        cards.push(uid.clone());
                    }
                }
            }
            if !cards.is_empty() {
                b.group = tr.text("set-sender-people-group", None);
                b.note(tr.text("set-sender-people-note", None), Vec::new());
                let mut named: Vec<(String, String)> = cards
                    .into_iter()
                    .map(|uid| {
                        let name = senders.contacts.find(&uid).and_then(|i| senders.contacts.card(i)).map(|c| c.name.clone()).filter(|n| !n.is_empty()).unwrap_or_else(|| tr.text("sender-card-elsewhere", None));
                        (name, uid)
                    })
                    .collect();
                named.sort_by_key(|(name, _)| crate::text::fold(name).into_iter().collect::<String>().to_lowercase());
                for (name, uid) in named {
                    let standing = senders.card_standing(&uid).map_or("", |s| s.as_str());
                    let s = b.push(&format!("{}{uid}", crate::porch::CONTACT), "sender-person", Kind::Choice, SettingValue::Text(standing.into()));
                    s.label = name;
                    s.help = String::new();
                    s.choices = choices("sender-no-choice");
                }
            }
            // Your contacts' categories, those in use and those on a list.
            let mut names = senders.contacts.names();
            for list in [&senders.safe, &senders.neutral, &senders.restricted, &senders.blocked] {
                for name in list.categories() {
                    if !names.iter().any(|n| crate::porch::category_key(n) == crate::porch::category_key(name)) {
                        names.push(name.clone());
                    }
                }
            }
            if !names.is_empty() {
                b.group = tr.text("set-sender-categories-group", None);
                b.note(tr.text("set-sender-categories-note", None), Vec::new());
                let choices = choices("sender-no-list");
                for name in names {
                    let standing = senders.category_standing(&name).map_or("", |s| s.as_str());
                    let s = b.push(&format!("{}{name}", crate::porch::CATEGORY), "sender-category", Kind::Choice, SettingValue::Text(standing.into()));
                    s.label = name;
                    s.help = String::new();
                    s.choices = choices.clone();
                }
            }
        }
        // What all addresses share, under them in Accounts: the key for the AI that reads hostile mail.
        "accounts" => {
            b.group = tr.text("set-ai-group", None);
            b.ai_key(config);
        }
        "agenda" => {
            b.collections(config, "calendars", crate::vdir::Kind::Calendars, "VEVENT");
            let s = b.push("agenda.day_start", "day-start", Kind::Int, int(config.agenda.day_start, 7));
            Builder::range(s, 0.0, 12.0, 1.0, "h");
        }
        "tasks" => {
            let office = config.office_hours().iter().map(|w| WindowValue { day: w.day.clone(), start: w.start.clone(), end: w.end_text(), minutes: 0 }).collect();
            b.push("office_hours", "office-hours", Kind::Windows, SettingValue::Windows(office));
            let kinds = config.task_kinds(tr).into_iter().map(|(id, label)| crate::config::NamedValue { id, label, ..Default::default() }).collect();
            b.push("tasks.kind", "task-kinds", Kind::Kinds, SettingValue::Named(kinds));
            b.collections(config, "task-lists", crate::vdir::Kind::Calendars, "VTODO");
            let s = b.push("tasks.estimate", "task-estimate", Kind::Int, int(config.tasks.estimate, crate::plan::Settings::default().default_estimate));
            Builder::range(s, 5.0, 240.0, 5.0, "min");
            let s = b.push("tasks.list", "task-list", Kind::Choice, SettingValue::Text(config.tasks.list.clone().unwrap_or_default()));
            s.choices = std::iter::once(Choice { value: SettingValue::Text(String::new()), label: tr.text("set-task-list-first", None) })
                .chain(lists.iter().map(|(id, name)| Choice { value: SettingValue::Text(id.clone()), label: name.clone() }))
                .collect();
            // Where a task pinned to a time goes, as an event (docs/tasks.md, "Pinned to a time").
            let calendars: Vec<Choice> = crate::vdir::collections(crate::vdir::Kind::Calendars)
                .into_iter()
                .filter(|c| !c.read_only && c.holds("VEVENT"))
                .map(|c| Choice { value: SettingValue::Text(format!("{}/{}", c.account, c.id)), label: c.label(config, tr) })
                .collect();
            let s = b.push("tasks.blocks", "task-blocks", Kind::Choice, SettingValue::Text(config.tasks.blocks.clone().unwrap_or_default()));
            s.choices = std::iter::once(Choice { value: SettingValue::Text(String::new()), label: tr.text("set-task-blocks-own", None) }).chain(calendars).collect();
            b.push("tasks.block_alarms", "task-block-alarms", Kind::Bool, SettingValue::Bool(config.tasks.block_alarms));
            // What a day holds, as the plan learns it from your days (docs/capacity.md).
            b.group = tr.text("set-planning-group", None);
            let s = b.push("planning.start", "planning-start", Kind::Choice, SettingValue::Text(config.planning.start.clone().unwrap_or_default()));
            s.choices = [("", "set-planning-start-as-now"), ("lighter", "set-planning-start-lighter"), ("much-lighter", "set-planning-start-much-lighter")].iter().map(|(v, l)| Choice { value: SettingValue::Text(v.to_string()), label: tr.text(l, None) }).collect();
            let s = b.push("planning.window_days", "planning-window", Kind::Int, SettingValue::Int(i64::from(config.planning.window())));
            Builder::range(s, f64::from(crate::capacity::WINDOW_LEAST), f64::from(crate::capacity::WINDOW_MOST), 1.0, "");
            s.unit = tr.text("set-planning-window-unit", None);
            b.push("planning.even_days", "planning-even", Kind::Bool, SettingValue::Bool(config.planning.even_days));
            b.push("planning.gain_slots", "planning-gain-slots", Kind::Bool, SettingValue::Bool(config.planning.gain_slots()));
            // What a task is for, by its categories (docs/areas.md): the hours it comes in.
            b.group = tr.text("set-task-areas-group", None);
            // Your lists, else those of the languages in use (`words.tasks`).
            let tasks = &crate::words::Words::of(config).tasks;
            b.push("quiet.work", "quiet-work", Kind::Words, SettingValue::Texts(tasks.work.clone()));
            b.push("quiet.personal", "quiet-personal", Kind::Words, SettingValue::Texts(tasks.personal.clone()));
            // GitHub, last and off: its issues and reviews as tasks, once asked.
            b.group = tr.text("set-code-group", None);
            b.push("github.enabled", "github", Kind::Bool, SettingValue::Bool(config.github.enabled));
            if config.github.enabled {
                b.push("github_token", "github-token", Kind::Secret, SettingValue::Bool(false));
                b.push("github.assigned", "github-assigned", Kind::Bool, SettingValue::Bool(config.github.assigned));
                b.push("github.reviews", "github-reviews", Kind::Bool, SettingValue::Bool(config.github.reviews));
                b.push("github.created", "github-created", Kind::Bool, SettingValue::Bool(config.github.created));
                b.push("github.mentioned", "github-mentioned", Kind::Bool, SettingValue::Bool(config.github.mentioned));
            }
        }
        "notes" => {
            b.push("notes_folder", "notes-folder", Kind::Text, SettingValue::Text(config.notes_folder.clone().unwrap_or_else(|| "notes".into())));
        }
        // How long text reads, behind "Aa" where it is read: a message, a note.
        "reading" => reading(&mut b, config),
        "sites" => {
            b.push("bitwarden.email", "bitwarden-email", Kind::Text, SettingValue::Text(config.bitwarden.email.clone().unwrap_or_default()));
            b.push("bitwarden.server", "bitwarden-server", Kind::Text, SettingValue::Text(config.bitwarden.server.clone().unwrap_or_default()));
        }
        "contacts" => {
            b.collections(config, "address-books", crate::vdir::Kind::Contacts, "");
            b.push("map.geocode", "map-geocode", Kind::Bool, SettingValue::Bool(config.map.geocode));
            b.push("map.tiles", "map-tiles", Kind::Text, SettingValue::Text(config.map.tiles.clone().unwrap_or_default()));
            // The country of numbers written without one: "04 65…" found as "+33 4 65…" (phones.rs).
            let s = b.push("contacts.region", "phone-region", Kind::Choice, SettingValue::Text(config.contacts.region.clone().unwrap_or_default()));
            s.choices = phone_regions(tr);
        }
        // Sioul as a whole: what belongs to no single page.
        "parameters" | "general" => {
            b.section = "look".into();
            b.group = tr.text("set-look-group", None);
            let s = b.push("language", "language", Kind::Choice, SettingValue::Text(config.language.clone().unwrap_or_default()));
            s.choices = [("", "set-language-system"), ("fr", "set-language-fr"), ("en", "set-language-en")].iter().map(|(v, l)| Choice { value: SettingValue::Text(v.to_string()), label: tr.text(l, None) }).collect();
            let s = b.push("theme", "theme", Kind::Choice, SettingValue::Text(config.theme.clone().unwrap_or_default()));
            s.choices = [("", "set-theme-system"), ("light", "set-theme-light"), ("dark", "set-theme-dark")].iter().map(|(v, l)| Choice { value: SettingValue::Text(v.to_string()), label: tr.text(l, None) }).collect();
            // The places' names beside their icons, for whoever reads words more easily (main.qml).
            b.push("places_named", "places-named", Kind::Bool, SettingValue::Bool(config.places_named));
            // Passwords shown as they are typed, for whoever needs to see them (the window keeps it, per computer).
            b.push("passwords_shown", "passwords-shown", Kind::Bool, SettingValue::Bool(false));
            // colour: the sites in the screen's own colours, and calmer colours (docs/colour.md);
            // the window says which profile, and leaves both out on a phone (backend.rs).
            b.push("screen_colours", "screen-colours", Kind::Bool, SettingValue::Bool(config.screen_colours.unwrap_or(true)));
            let s = b.push("calmer_colours", "calmer-colours", Kind::Choice, SettingValue::Text(config.calmer_colours.clone().unwrap_or_default()));
            s.choices = [("", "set-calmer-colours-off"), ("little", "set-calmer-colours-little"), ("more", "set-calmer-colours-more")].iter().map(|(v, l)| Choice { value: SettingValue::Text(v.to_string()), label: tr.text(l, None) }).collect();
            // Your folder: notes, and beside them projects, budgets, letters. How far back
            // mail and calendars go is the accounts', at the top of Accounts.
            b.section = "files".into();
            b.group = tr.text("set-files-group", None);
            b.push("case_store", "case-store", Kind::Folder, SettingValue::Text(config.case_store.clone().unwrap_or_default()));
            b.section = "hours".into();
            b.group = tr.text("set-hours-group", None);
            let week = |windows: &[crate::window::AdminWindow]| SettingValue::Windows(windows.iter().map(|w| WindowValue { day: w.day.clone(), start: w.start.clone(), end: w.end_text(), minutes: 0 }).collect());
            // Working hours and hours for your own admin, each its own week; leisure
            // is every other time, meals and sleep come from Health (docs/areas.md).
            let of = |kind: &str| config.windows.iter().filter(|w| w.kind() == kind).cloned().collect::<Vec<_>>();
            b.push("window", "windows", Kind::Windows, week(&of("work")));
            b.push("window.admin", "windows-admin", Kind::Windows, week(&of("admin")));
            b.push("link.needs", "hours-leisure", Kind::Link, SettingValue::Text("needs".into()));
            let off = config.time_off.iter().map(|t| crate::config::TimeOffValue { from: t.from.to_string(), until: t.until.to_string(), label: t.label.clone() }).collect();
            b.push("time_off", "time-off", Kind::TimeOff, SettingValue::TimeOff(off));
            // What reaches you, and when (docs/attention.md): the matrix is the tab's own
            // (`attention::grid`, the window's `reachesView`); these are the switches beside it.
            b.section = "attention".into();
            b.group = String::new();
            // New mail told at the times its row says, once per batch (docs/porch.md, "Notifications").
            b.push("reminders.mail", "reminders-mail", Kind::Bool, SettingValue::Bool(config.reminders.mail));
            b.push("reminders.mail_newsletters", "reminders-mail-newsletters", Kind::Bool, SettingValue::Bool(config.reminders.mail_newsletters));
            // What sites notified, gathered at the gathered times; the Sites row's switch.
            b.push("reminders.gather", "reminders-gather", Kind::Bool, SettingValue::Bool(config.reminders.gather));
            // Free time's "Nothing at all": its card's switch.
            b.push("free_time.nothing", "free-nothing", Kind::Bool, SettingValue::Bool(config.free_time.nothing));
            // What turns do-not-disturb on, on every device (docs/do-not-disturb.md).
            b.push("dnd.button", "dnd-button", Kind::Bool, SettingValue::Bool(config.dnd.button));
            b.push("dnd.focus", "dnd-focus", Kind::Bool, SettingValue::Bool(config.dnd.focus));
            b.push("dnd.pauses", "dnd-pauses", Kind::Bool, SettingValue::Bool(config.dnd.pauses));
            b.push("dnd.sleep", "dnd-sleep", Kind::Bool, SettingValue::Bool(config.dnd.sleep));
            // The words Sioul looks for (docs/words.md): a tab of their own (`WordsTab.qml`).
            b.section = "words".into();
            words_rows(&mut b, config);
            // Reminders before dates; with the window closed, the window says (an entry started with the session).
            b.section = "reminders".into();
            b.group = tr.text("set-reminders-group", None);
            // An event's reminder before it, counted before its margin; each event may say its own (docs/reminders.md).
            let lead = config.reminders.before_event;
            let mut leads = crate::reminders::LEADS.to_vec();
            if !leads.contains(&lead) {
                leads.push(lead);
                leads.sort_unstable();
            }
            let s = b.push("reminders.before_event", "reminders-before", Kind::Choice, SettingValue::Int(i64::from(lead)));
            s.choices = leads.iter().map(|m| Choice { value: SettingValue::Int(i64::from(*m)), label: if *m == 0 { tr.text("set-reminders-before-none", None) } else { crate::reminders::lead_text(tr, *m) } }).collect();
            b.push("reminders.events", "reminders-events", Kind::Bool, SettingValue::Bool(config.reminders.events));
            let s = b.push("reminders.asked_days", "reminders-asked", Kind::Int, SettingValue::Int(i64::from(config.reminders.asked_days)));
            Builder::range(s, 0.0, 10.0, 1.0, "");
            b.push("reminders.waits", "reminders-waits", Kind::Bool, SettingValue::Bool(config.reminders.waits));
            let s = b.push("reminders.payment_days", "reminders-payment", Kind::Int, SettingValue::Int(i64::from(config.reminders.payment_days)));
            Builder::range(s, 0.0, 10.0, 1.0, "");
            b.push("reminders_closed", "reminders-closed", Kind::Bool, SettingValue::Bool(false));
            // The times sites and other apps' automatons are gathered at: their cells say "gathered".
            b.push("reminders.gathered", "reminders-gathered", Kind::Words, SettingValue::Texts(config.reminders.gathered_times()));
            // The two pauses, set up on a calm day (docs/pauses.md): free time, then the pause.
            b.section = "pauses".into();
            b.group = tr.text("set-free-time-group", None);
            b.push("free_time.moves", "free-moves", Kind::Bool, SettingValue::Bool(config.free_time.moves));
            let s = b.push("free_time.latest_after", "free-latest", Kind::Int, SettingValue::Int(i64::from(config.free_time.latest_after)));
            Builder::range(s, 30.0, 360.0, 15.0, "min");
            b.push("free_time.movement", "free-movement", Kind::Bool, SettingValue::Bool(config.free_time.movement(&config.planning)));
            b.group = tr.text("set-pause-group", None);
            // Said once, where the pause is set up (P5).
            b.note(tr.text("set-pause-about", None), Vec::new());
            // What comes during a pause, doses included: What reaches you ▸ By time, the pause's card.
            b.push("link.attention.pause", "pause-attention", Kind::Link, SettingValue::Text("settings:attention.pause".into()));
            b.push("pause.helps", "pause-helps", Kind::Words, SettingValue::Texts(config.pause.helps.clone()));
            b.push("pause.grounding", "pause-grounding", Kind::Text, SettingValue::Text(config.pause.grounding.clone()));
            b.push("pause.breathing", "pause-breathing", Kind::Bool, SettingValue::Bool(config.pause.breathing));
            let s = b.push("pause.pace", "pause-pace", Kind::Int, SettingValue::Int(i64::from(config.pause.pace())));
            Builder::range(s, 3.0, 10.0, 1.0, "");
            let s = b.push("pause.after", "pause-after", Kind::Choice, SettingValue::Text(config.pause.after().id().to_string()));
            s.choices = [("lighter", "set-pause-after-lighter"), ("rest", "set-pause-after-rest"), ("as-is", "set-pause-after-as-is")].iter().map(|(v, l)| Choice { value: SettingValue::Text(v.to_string()), label: tr.text(l, None) }).collect();
            // The emergency number and the crisis line of a country (P13): the phone numbers' country unless chosen.
            let s = b.push("pause.country", "pause-country", Kind::Choice, SettingValue::Text(config.pause.country.clone().unwrap_or_default()));
            let mut countries: Vec<Choice> = crate::pause::CrisisLines::built_in()
                .country
                .iter()
                .map(|c| Choice { value: SettingValue::Text(c.code.clone()), label: tr.text(&format!("set-pause-country-{}", c.code.to_ascii_lowercase()), None) })
                .collect();
            countries.sort_by_cached_key(|c| crate::text::fold(&c.label).into_iter().collect::<String>());
            s.choices = std::iter::once(Choice { value: SettingValue::Text(String::new()), label: tr.text("set-pause-country-usual", None) }).chain(countries).collect();
            // Invoices, made from Time and from Projects: who sends them, how they are numbered, where they go.
            b.section = "invoices".into();
            b.group = tr.text("set-invoice-group", None);
            let invoice = &config.invoice;
            for (field, kind, value) in [
                ("name", Kind::Text, invoice.name.clone()),
                ("address", Kind::Long, invoice.address.clone()),
                ("siret", Kind::Text, invoice.siret.clone()),
                ("vat", Kind::Text, invoice.vat.clone()),
                ("prefix", Kind::Text, invoice.prefix.clone()),
                ("currency", Kind::Text, if invoice.currency.is_empty() { "EUR".into() } else { invoice.currency.clone() }),
                ("payment", Kind::Long, invoice.payment.clone()),
            ] {
                b.push(&format!("invoice.{field}"), &format!("invoice-{field}"), kind, SettingValue::Text(value));
            }
            b.push("invoice.folder", "invoice-folder", Kind::Folder, SettingValue::Text(invoice.folder.clone()));
            let s = b.push("invoice.rate", "invoice-rate", Kind::Float, SettingValue::Float(invoice.rate));
            Builder::range(s, 0.0, 2000.0, 5.0, "/h");
            // AI agents connected with `sioul mcp` (docs/ai.md, docs/mcp.md): what they may read and
            // write. Things in no project, then each project, closed until opened (`consent`).
            b.section = "ai".into();
            b.group = tr.text("set-ai-agents-group", None);
            b.note(tr.text("set-ai-agents-note", None), Vec::new());
            b.push("mcp.outside_projects", "mcp-outside-projects", Kind::Bool, SettingValue::Bool(config.mcp.outside_projects));
            b.push("mcp.texts", "mcp-texts", Kind::Bool, SettingValue::Bool(config.mcp.texts));
            b.push("mcp.spam", "mcp-spam", Kind::Bool, SettingValue::Bool(config.mcp.spam));
            b.group = tr.text("set-ai-projects-group", None);
            let projects = config.case_store_path().and_then(|root| crate::cases::CaseStore::load(&root).ok()).map(|store| store.cases).unwrap_or_default();
            b.note(tr.text(if projects.is_empty() { "set-ai-projects-none" } else { "set-ai-projects-note" }, None), Vec::new());
            for project in projects {
                b.said(&format!("case.{}.ai", project.id), Kind::Bool, project.title.clone(), String::new(), SettingValue::Bool(project.ai));
            }
        }
        _ => {}
    }
    b.out
}

/// The words tab's lines (docs/words.md, Settings ▸ Words, `WordsTab.qml`):
/// each in a family of lines (`words-family-<family>`), named by its id
/// (`words-line-<id>`, and `words-line-<id>-help`: what its words do), with
/// the lists it shows (`words-list-<list>`, dots and underscores as dashes).
/// Only the words that say what a thing is: the grammar around them (months,
/// "your … code", stop words) stays in the packs.
pub const WORD_LINES: &[(&str, &str, &[&str])] = &[
    ("mail", "codes", &["codes.code", "codes.password", "codes.reset", "codes.link", "codes.confirm", "codes.not_yours", "codes.promo"]),
    ("mail", "senders", &["senders.automatic"]),
    ("mail", "replies", &["replies.reply", "replies.forward", "quotes.openings", "quotes.wrote"]),
    ("mail", "folders", &["folders.sent", "folders.drafts", "folders.junk", "folders.trash", "folders.archive"]),
    ("mail", "brands", &["brands.brands", "brands.shared"]),
    ("money", "payments", &["payments.bill", "payments.paid", "payments.received", "payments.order", "payments.refund", "payments.not_payments"]),
    ("money", "bank", &["bank.filler", "accounts.between"]),
    (
        "money",
        "letters",
        &[
            "letters.senders",
            "letters.kinds.formal_notice",
            "letters.kinds.tax_notice",
            "letters.kinds.decision",
            "letters.kinds.reminder",
            "letters.kinds.appointment",
            "letters.kinds.bill",
            "letters.kinds.acknowledgment",
            "letters.kinds.attestation",
            "letters.kinds.contract",
        ],
    ),
    (
        "money",
        "papers",
        &[
            "papers.kinds.passport",
            "papers.kinds.identity",
            "papers.kinds.residence",
            "papers.kinds.driving",
            "papers.kinds.health_cover",
            "papers.kinds.health_card",
            "papers.kinds.tax_notice",
            "papers.kinds.rent_receipt",
            "papers.kinds.bank_details",
            "papers.kinds.payslip",
            "papers.kinds.warranty",
            "papers.kinds.insurance",
            "papers.kinds.certificate",
        ],
    ),
    (
        "money",
        "contracts",
        &[
            "contracts.kinds.rent",
            "contracts.kinds.energy",
            "contracts.kinds.health",
            "contracts.kinds.insurance",
            "contracts.kinds.telecom",
            "contracts.kinds.hosting",
            "contracts.kinds.subscription",
            "contracts.kinds.bank",
        ],
    ),
    ("phone", "approvals", &["approvals.phrases", "approvals.asks", "approvals.asked_about", "approvals.channels"]),
    ("phone", "calls", &["calls.ringing", "calls.missed"]),
    ("phone", "voicemail", &["voicemail.operators"]),
    (
        "tasks",
        "tasks",
        &[
            "tasks.optional",
            "tasks.joy",
            "tasks.movement",
            "capture.kind_verbs.call",
            "capture.kind_verbs.write",
            "capture.kind_verbs.online",
            "capture.kind_verbs.out",
            "capture.kind_verbs.read",
            "capture.kind_verbs.think",
            "capture.kind_verbs.make",
            "capture.kind_words.call",
            "capture.kind_words.write",
            "capture.kind_words.online",
            "capture.kind_words.out",
            "capture.kind_words.read",
            "capture.kind_words.think",
            "capture.kind_words.make",
        ],
    ),
    ("tasks", "spam", &["spam.provider_tags"]),
    ("tasks", "shield", &["shield.threats", "shield.insults", "shield.rude", "shield.topics.work", "shield.topics.support", "shield.topics.press", "shield.topics.thanks", "shield.topics.donation"]),
];

/// Whether a list is one of the words tab's.
fn word_list(path: &str) -> bool {
    WORD_LINES.iter().any(|(_, _, lists)| lists.contains(&path))
}

/// A named list's entries as the tab shows them, one per chip: "Name: word, word".
fn named_chips(entries: &[(String, Vec<String>)]) -> Vec<String> {
    entries.iter().map(|(name, words)| if words.is_empty() { name.clone() } else { format!("{name}: {}", words.join(", ")) }).collect()
}

/// The chips of a named list read back: the name, then its words after a
/// colon, separated by commas. A body named alone ("CAF") is looked for by
/// its name; a brand needs its domains, or every sender naming it would be
/// set aside.
fn named_entries(path: &str, chips: &[String]) -> Result<Vec<(String, Vec<String>)>, String> {
    let mut out = Vec::new();
    for chip in chips {
        let (name, words) = match chip.split_once(':') {
            Some((name, words)) => (name.trim().to_string(), words.split(',').map(|w| w.trim().to_string()).filter(|w| !w.is_empty()).collect::<Vec<_>>()),
            None => (chip.trim().to_string(), Vec::new()),
        };
        if name.is_empty() {
            continue;
        }
        let words = match (words.is_empty(), path) {
            (true, "brands.brands") => return Err(format!("{name}: write its domains after a colon (\"{name}: example.org\")")),
            (true, _) => vec![crate::words::folded(&name)],
            (false, _) => words,
        };
        out.push((name, words));
    }
    Ok(out)
}

/// A named list of a tree of lists (the packs alone, or with your changes).
fn names_at(tree: &crate::words::Node, path: &str) -> Vec<(String, Vec<String>)> {
    match tree.get(path) {
        Some(crate::words::Node::Table(entries)) => entries
            .iter()
            .filter_map(|(name, node)| match node {
                crate::words::Node::List(words) => Some((name.clone(), words.clone())),
                crate::words::Node::Table(_) => None,
            })
            .collect(),
        _ => Vec::new(),
    }
}

/// Names joined as a sentence says them: "French and English", "French, English and German".
fn and_list(tr: &Translator, names: &[String]) -> String {
    let and = tr.text("word-and", None);
    match names {
        [] => String::new(),
        [one] => one.clone(),
        [first @ .., last] => format!("{} {and} {last}", first.join(", ")),
    }
}

/// The words tab (docs/words.md): the languages and countries read, ticked;
/// then each line (`WORD_LINES`) as a group of its own: a note (`words.about.<line>`,
/// its family's name in `unit`) saying what its words do, how many there are
/// and how many you changed, then its lists as Sioul uses them (the packs, your
/// changes, and an older whole list that still wins: `filed_words`), each a
/// row of words (`words.<list>`; a named list's entries as "Name: word, word"),
/// your own words first.
fn words_rows(b: &mut Builder, config: &Config) {
    let tr = b.tr;
    let words = crate::words::Words::of(config);
    let compared = crate::words::compared(config);
    b.group = String::new();
    let s = b.push("words.languages", "words-languages", Kind::Picks, SettingValue::Texts(words.languages.clone()));
    s.choices = crate::words::available_languages().iter().map(|code| Choice { value: SettingValue::Text(code.to_string()), label: tr.text(&format!("words-language-{code}"), None) }).collect();
    let s = b.push("words.countries", "words-countries", Kind::Picks, SettingValue::Texts(words.countries.clone()));
    s.choices = crate::words::available_countries()
        .iter()
        .map(|code| Choice { value: SettingValue::Text(code.to_string()), label: tr.text(&format!("country-{}", code.to_ascii_lowercase()), None) })
        .collect();
    let languages: Vec<String> = words.languages.iter().map(|code| tr.text(&format!("words-language-{code}-in"), None)).collect();
    let languages = and_list(tr, &languages);
    let same = |a: &str, b: &str| crate::words::folded(a) == crate::words::folded(b);
    for (family, line, lists) in WORD_LINES {
        b.group = tr.text(&format!("words-line-{line}"), None);
        let (mut count, mut added, mut removed) = (0usize, 0usize, 0usize);
        let mut rows: Vec<(&str, Vec<String>, bool)> = Vec::new();
        for path in lists.iter().copied() {
            if crate::words::MAPS.contains(&path) {
                let now = names_at(&compared.yours, path);
                let before = names_at(&compared.shipped, path);
                count += now.len();
                for (name, words) in &now {
                    match before.iter().find(|(n, _)| same(n, name)) {
                        None => added += 1,
                        Some((_, shipped)) => {
                            added += usize::from(words.iter().any(|w| !shipped.iter().any(|s| same(s, w))));
                            removed += usize::from(shipped.iter().any(|s| !words.iter().any(|w| same(s, w))));
                        }
                    }
                }
                removed += before.iter().filter(|(n, _)| !now.iter().any(|(name, _)| same(n, name))).count();
                // Your own names first, then the packs'.
                let (yours, shipped): (Vec<(String, Vec<String>)>, Vec<(String, Vec<String>)>) = now.into_iter().partition(|(name, _)| !before.iter().any(|(n, _)| same(n, name)));
                rows.push((path, named_chips(&yours.into_iter().chain(shipped).collect::<Vec<_>>()), true));
            } else {
                // The automatic senders' older whole list still wins while written (`words::older_settings`).
                let now = if path == "senders.automatic" { words.senders.automatic.clone() } else { compared.yours.list(path).to_vec() };
                let before = compared.shipped.list(path);
                count += now.len();
                added += now.iter().filter(|w| !before.iter().any(|s| same(s, w))).count();
                removed += before.iter().filter(|s| !now.iter().any(|w| same(s, w))).count();
                // Your own words first, then the packs'.
                let (yours, shipped): (Vec<String>, Vec<String>) = now.into_iter().partition(|w| !before.iter().any(|s| same(s, w)));
                rows.push((path, yours.into_iter().chain(shipped).collect(), false));
            }
        }
        let mut args = tr.counted(count);
        args.set("languages", languages.clone());
        let mut said = vec![tr.text("words-line-count", Some(&args))];
        if added > 0 {
            said.push(tr.text("words-line-added", Some(&tr.counted(added))));
        }
        if removed > 0 {
            said.push(tr.text("words-line-removed", Some(&tr.counted(removed))));
        }
        let about = format!("words.about.{line}");
        let help = tr.text(&format!("words-line-{line}-help"), None);
        let family = tr.text(&format!("words-family-{family}"), None);
        let s = b.said(&about, Kind::Note, help, said.join(" · "), SettingValue::Text(String::new()));
        s.unit = family;
        for (path, now, named) in rows {
            let label = tr.text(&format!("words-list-{}", path.replace(['.', '_'], "-")), None);
            let help = if named { tr.text(&format!("words-named-{}", path.replace(['.', '_'], "-")), None) } else { String::new() };
            b.said(&format!("words.{path}"), Kind::Words, label, help, SettingValue::Texts(now));
        }
    }
}

/// The countries numbers written without one can be read as: as the system
/// says first (its locale's, else the language's), then each one by name.
fn phone_regions(tr: &Translator) -> Vec<Choice> {
    let name = |code: &str| tr.text(&format!("country-{}", code.to_ascii_lowercase()), None);
    let usual = match crate::phones::usual_region(&tr.text("qt-locale", None)) {
        Some(region) => {
            let mut args = crate::i18n::args();
            args.set("country", name(region.code));
            tr.text("set-phone-region-usual", Some(&args))
        }
        None => tr.text("set-phone-region-none", None),
    };
    let mut countries: Vec<Choice> = crate::phones::REGIONS.iter().map(|r| Choice { value: SettingValue::Text(r.code.to_string()), label: name(r.code) }).collect();
    countries.sort_by_cached_key(|c| crate::text::fold(&c.label).into_iter().collect::<String>());
    std::iter::once(Choice { value: SettingValue::Text(String::new()), label: usual }).chain(countries).collect()
}

fn reading(b: &mut Builder, config: &Config) {
    b.push("reading.family", "reading-family", Kind::Font, SettingValue::Text(config.reading.family.clone()));
    let s = b.push("reading.size", "reading-size", Kind::Int, SettingValue::Int(i64::from(config.reading.size)));
    Builder::range(s, 10.0, 30.0, 1.0, "px");
    let s = b.push("reading.spacing", "reading-spacing", Kind::Float, SettingValue::Float(config.reading.spacing));
    Builder::range(s, 1.0, 2.4, 0.1, "×");
}

/// An account's own history setting, before the global one fills it in (`Config::load`).
fn raw_account_history(config: &Config, id: &str) -> Option<u32> {
    let account = config.accounts.iter().find(|a| a.id == id)?;
    account.history_weeks.filter(|w| Some(*w) != config.history_weeks)
}

/// Changes one setting: in the configuration file, or in a senders' list.
/// The categories tasks have, the most used first, to rename or take off; the
/// window reads the tasks, the core does not.
pub fn categories(tr: &Translator, names: Vec<String>) -> Setting {
    let mut b = Builder { tr, out: Vec::new(), group: String::new(), section: String::new() };
    b.push("task_categories", "task-categories", Kind::Categories, SettingValue::Named(names.into_iter().map(|n| crate::config::NamedValue { id: n.clone(), label: n, ..Default::default() }).collect()));
    b.out.remove(0)
}

pub fn apply(config_path: &Path, config: &Config, key: &str, value: &SettingValue) -> Result<(), String> {
    // The window's JSON `[]` reads as the first kind of list it fits, numbers:
    // the last entry of a list taken away is an empty list of any kind.
    let none = matches!(value, SettingValue::Ints(list) if list.is_empty());
    match key {
        // The words Sioul looks for (docs/words.md): the languages and countries read;
        // a line of the words tab back to the packs; one of its lists as you want it,
        // written as its difference from the packs (`words::write_list`, `write_names`).
        "words.languages" | "words.countries" => {
            let values: &[String] = match value {
                SettingValue::Texts(values) => values,
                _ if none => &[],
                _ => return Err(format!("{key}: a list expected")),
            };
            if key == "words.languages" && values.is_empty() {
                return Err(format!("{key}: Sioul reads at least one language"));
            }
            crate::words::write_setting(config_path, &key["words.".len()..], values)
        }
        _ if key.starts_with("words.reset.") => {
            let line = &key["words.reset.".len()..];
            let (_, _, lists) = WORD_LINES.iter().find(|(_, id, _)| *id == line).ok_or_else(|| format!("{key}: no such line"))?;
            if lists.contains(&"senders.automatic") && config.filed_words.is_some() {
                set_value(config_path, "filed_words", &SettingValue::Text(String::new()))?;
            }
            crate::words::reset_lists(config_path, lists)
        }
        // `filed_words`, the automatic senders' older whole list, as a window
        // before the words tab writes it: as the tab writes it now.
        _ if key == "filed_words" || key.strip_prefix("words.").is_some_and(word_list) => {
            let path = if key == "filed_words" { "senders.automatic" } else { &key["words.".len()..] };
            let wanted: &[String] = match value {
                SettingValue::Texts(values) => values,
                _ if none => &[],
                _ => return Err(format!("{key}: a list expected")),
            };
            // The older whole list goes: the difference says it from now on.
            if path == "senders.automatic" && config.filed_words.is_some() {
                set_value(config_path, "filed_words", &SettingValue::Text(String::new()))?;
            }
            let shipped = crate::words::compared(config).shipped;
            if crate::words::MAPS.contains(&path) {
                crate::words::write_names(config_path, path, &named_entries(path, wanted)?, &names_at(&shipped, path))
            } else {
                crate::words::write_list(config_path, path, wanted, shipped.list(path))
            }
        }
        // Secrets go to the keyring (the window's to do), never into the configuration.
        "ai_key" | "github_token" => Err(format!("{key}: kept in the system keyring, never in the configuration")),
        "tasks.kind" => {
            let SettingValue::Rename(change) = value else { return Err(format!("{key}: a change expected")) };
            crate::config::change_kind(config_path, config, &change.from, &change.to)
        }
        // "calendars/<account>/<id>" or "contacts/…": renamed, or deleted when empty.
        "collections" => {
            let SettingValue::Rename(change) = value else { return Err(format!("{key}: a change expected")) };
            let mut parts = change.from.splitn(3, '/');
            let (Some(kind), Some(account), Some(id)) = (parts.next(), parts.next(), parts.next()) else { return Err(format!("{}: unknown", change.from)) };
            let kind = if kind == "contacts" { crate::vdir::Kind::Contacts } else { crate::vdir::Kind::Calendars };
            let collection = crate::vdir::collections(kind).into_iter().find(|c| c.account == account && c.id == id).ok_or_else(|| format!("{}: unknown", change.from))?;
            if change.to.trim().is_empty() { crate::vdir::delete_collection(&collection) } else { crate::vdir::rename_collection(&collection, &change.to) }
        }
        // A category of your contacts on one list, or on none ("").
        _ if key.starts_with(crate::porch::CATEGORY) => {
            let name = crate::porch::category_of(key).ok_or_else(|| format!("{key}: no category"))?;
            let standing = match value {
                SettingValue::Text(text) if text.is_empty() => None,
                SettingValue::Text(text) => Some(crate::porch::Standing::read(text).ok_or_else(|| format!("{text}: safe, neutral, restricted or blocked"))?),
                _ => return Err(format!("{key}: a list expected")),
            };
            crate::porch::set_category(config, name, standing)
        }
        "known" | "safe" | "neutral" | "restricted" | "blocked" => {
            let path = match key {
                "known" => config.known_senders_path(),
                "safe" => config.safe_senders_path(),
                "neutral" => config.neutral_senders_path(),
                "restricted" => config.restricted_senders_path(),
                _ => config.blocked_senders_path(),
            };
            let wanted: &[String] = match value {
                SettingValue::Texts(wanted) => wanted,
                _ if none => &[],
                _ => return Err(format!("{key}: a list expected")),
            };
            // A category typed here ("category:Friends") goes on this list, out of the others.
            let standing = crate::porch::Standing::read(key);
            if let Some(standing) = standing {
                for name in wanted.iter().filter_map(|e| crate::porch::category_of(e)) {
                    crate::porch::set_category(config, name, Some(standing))?;
                }
            }
            // What the editor shows: addresses and patterns, and on the four
            // lists numbers too; cards and categories have rows of their own.
            use crate::porch::Entry;
            let region = crate::reach::region(config);
            let read = |e: &String| Entry::read(e, region).filter(|e| matches!(e, Entry::Address(_)) || (standing.is_some() && matches!(e, Entry::Number(_))));
            let wanted: Vec<Entry> = wanted.iter().filter_map(read).collect();
            let list = SenderList::load(&path);
            let current: Vec<Entry> = list.entries().iter().chain(list.numbers()).filter_map(read).collect();
            for gone in current.iter().filter(|e| !wanted.iter().any(|w| w.same(e))) {
                SenderList::remove_entry(&path, gone, region)?;
            }
            for new in wanted.iter().filter(|e| !current.iter().any(|c| c.same(e))) {
                // The four lists exclude each other: one list per entry.
                match standing {
                    Some(standing) => crate::porch::set_standing(config, &new.line(), standing)?,
                    None => SenderList::add_entry(&path, new, region)?,
                }
            }
            Ok(())
        }
        // A person placed on a list (their card): on another, or on none; their
        // addresses' and numbers' own entries go, the card deciding for them.
        _ if key.starts_with(crate::porch::CONTACT) => {
            let uid = crate::porch::card_of(key).ok_or_else(|| format!("{key}: no card"))?;
            let standing = match value {
                SettingValue::Text(text) if text.is_empty() => None,
                SettingValue::Text(text) => Some(crate::porch::Standing::read(text).ok_or_else(|| format!("{text}: safe, neutral, restricted or blocked"))?),
                _ => return Err(format!("{key}: a list expected")),
            };
            let card = crate::contacts::all().into_iter().find(|c| c.uid.trim() == uid);
            let addresses: Vec<String> = card.iter().flat_map(|c| c.emails.iter().map(|e| e.value.clone())).collect();
            let numbers: Vec<String> = card.iter().flat_map(|c| c.phones.iter().map(|p| p.value.clone())).collect();
            crate::porch::set_person(config, uid, &addresses, &numbers, standing)
        }
        // The projects ticked: those not ticked are written, so a new project shows.
        "porch.projects" => {
            let shown: &[String] = match value {
                SettingValue::Texts(shown) => shown,
                _ if none => &[],
                _ => return Err(format!("{key}: a list expected")),
            };
            let root = config.case_store_path().ok_or_else(|| format!("{key}: no notes folder"))?;
            let store = crate::cases::CaseStore::load(&root).map_err(|e| e.to_string())?;
            let hidden: Vec<String> = store.cases.iter().map(|c| c.id.clone()).filter(|id| !shown.contains(id)).collect();
            set_value(config_path, "porch.hidden_projects", &SettingValue::Texts(hidden))
        }
        // A project opened to AI agents, or closed: `ai` on its entry in the manifest (docs/ai.md).
        _ if key.strip_prefix("case.").and_then(|rest| rest.strip_suffix(".ai")).is_some_and(|id| !id.is_empty()) => {
            let id = key.strip_prefix("case.").and_then(|rest| rest.strip_suffix(".ai")).unwrap_or_default();
            let SettingValue::Bool(open) = value else { return Err(format!("{key}: on or off expected")) };
            let root = config.case_store_path().ok_or_else(|| format!("{key}: no case store"))?;
            crate::cases::set_ai(&root.join(crate::cases::MANIFEST), id, *open)
        }
        _ if key.starts_with("case.") && key.ends_with(".routes") => {
            let id = key.trim_start_matches("case.").trim_end_matches(".routes");
            let routes: &[crate::cases::RouteValue] = match value {
                SettingValue::Routes(routes) => routes,
                _ if none => &[],
                _ => return Err(format!("{key}: routes expected")),
            };
            let root = config.case_store_path().ok_or_else(|| format!("{key}: no case store"))?;
            crate::cases::set_routes(&root.join(crate::cases::MANIFEST), id, routes)
        }
        // A row of the matrix of what reaches you, its words: read, checked, written whole
        // (`attention::apply`); the older grids' rows ("notify.<kind>", "reach.<row>") land there too.
        _ if key.starts_with("attention.") || key.starts_with("notify.") || key.starts_with("reach.") => crate::attention::apply(config_path, config, key, value),
        // Sioul's own spam filter: one row of the matrix, an action it knows;
        // the three written (an older `mode` read no more, taken out), so that
        // each row keeps what it meant. Thresholds between 0 and 1, the doubt below the spam.
        _ if key.starts_with("spam.actions.") => {
            use crate::spam::{Action, Class};
            let class = Class::read(&key["spam.actions.".len()..]).ok_or_else(|| format!("{key}: spam, unsure or ham"))?;
            let action = match value {
                SettingValue::Text(action) => Action::read(action),
                _ => None,
            }
            .ok_or_else(|| format!("{key}: move, flag or nothing"))?;
            let actions = config.spam.actions().with(class, action);
            for class in Class::ALL {
                set_value(config_path, &format!("spam.action_{}", class.as_str()), &SettingValue::Text(actions.of(class).as_str().into()))?;
            }
            set_value(config_path, "spam.mode", &SettingValue::Text(String::new()))
        }
        "spam.threshold_spam" | "spam.threshold_unsure" => {
            let wanted = match value {
                SettingValue::Float(f) => *f,
                SettingValue::Int(n) => *n as f64,
                _ => return Err(format!("{key}: a number between 0 and 1 expected")),
            };
            if !(wanted.is_finite() && wanted > 0.0 && wanted <= 1.0) {
                return Err(format!("{key}: {wanted} is not between 0 and 1"));
            }
            let (spam, unsure) = config.spam.thresholds();
            let (spam, unsure) = if key == "spam.threshold_spam" { (wanted, f64::from(unsure)) } else { (f64::from(spam), wanted) };
            if unsure >= spam {
                return Err(format!("{key}: unsure ({unsure:.2}) must stay below spam ({spam:.2})"));
            }
            set_value(config_path, key, &SettingValue::Float(wanted))
        }
        // "Like the others": the account's own value goes.
        _ if key.starts_with("account.") && key.ends_with(".history_weeks") && *value == SettingValue::Int(-1) => set_value(config_path, key, &SettingValue::Text(String::new())),
        _ if key.starts_with("account.") && key.ends_with(".fetch_minutes") && *value == SettingValue::Int(0) => set_value(config_path, key, &SettingValue::Text(String::new())),
        _ => set_value(config_path, key, value),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn each_page_has_its_own() {
        let config: Config = toml::from_str("case_store = \"~/Notes\"\n[[account]]\nid = \"a\"\nkind = \"imap\"\naddress = \"jane@example.org\"\nshield = true\nshield_ai = true\n").unwrap();
        let tr = Translator::new("en");
        // Calendars, task lists and address books share one key, each page its own kind of them,
        // and depend on what this computer holds: left out.
        // Your contacts' categories are this computer's: left out too.
        let keys = |view: &str| for_view(view, &config, &tr, &[("acct/plan".into(), "Plan".into())], None).into_iter().filter(|s| s.kind != Kind::Note && s.key != "collections" && !s.key.starts_with(crate::porch::CATEGORY) && !s.key.starts_with(crate::porch::CONTACT)).map(|s| s.key).collect::<Vec<_>>();
        assert_eq!(keys("notes"), vec!["notes_folder"]);
        // Mail's own, then how a message reads: the reading panel's, shown where messages are read.
        assert_eq!(keys("mail"), vec!["mail.threads", "fetch_minutes", "mail.filter", "spam.actions", "spam.threshold_spam", "spam.threshold_unsure", "spam.filter", "reading.family", "reading.size", "reading.spacing"]);
        // An address's own, on its card in Accounts; what all share, under them.
        assert_eq!(keys("account:a"), vec!["account.a.area", "account.a.history_weeks", "account.a.fetch_minutes", "account.a.shield", "account.a.shield_ai"]);
        assert_eq!(keys("accounts"), vec!["ai_key"]);
        assert_eq!(keys("tasks"), vec!["office_hours", "tasks.kind", "tasks.estimate", "tasks.list", "tasks.blocks", "tasks.block_alarms", "planning.start", "planning.window_days", "planning.even_days", "planning.gain_slots", "quiet.work", "quiet.personal", "github.enabled"]);
        assert_eq!(for_view("tasks", &config, &tr, &[("acct/plan".into(), "Plan".into())], None).iter().find(|s| s.key == "tasks.list").unwrap().choices.len(), 2);
        assert_eq!(for_view("lane:filed", &config, &tr, &[], None)[0].kind, Kind::Link);
        // The Porch: its letters and its own sorting; every lane said once, none with another page's settings.
        let porch = for_view("porch", &config, &tr, &[], None);
        let notes = porch.iter().filter(|s| s.kind == Kind::Note).count();
        // Where the hours went, the order, then public, people, screener, filed, less important, the review queue, set aside, hostile.
        assert_eq!(notes, 1 + 1 + 8, "{porch:?}");
        assert_eq!(keys("porch"), vec!["letters.inbox", "porch.calls", "reading.family", "reading.size", "reading.spacing", "known", "link.words.senders"]);
        assert!(porch.iter().any(|s| s.kind == Kind::Note && s.help.contains("Accounts")), "the shield is said to be in Accounts");
        // Sioul as a whole: language and looks, your folder, hours, what reaches you, reminders, pauses, invoices;
        // the words tab apart (`the_words_tab`).
        let parameters: Vec<String> = keys("parameters").into_iter().filter(|k| !k.starts_with("words.")).collect();
        assert_eq!(
            parameters,
            vec![
                // colour: "screen_colours", "calmer_colours".
                "language", "theme", "places_named", "passwords_shown", "screen_colours", "calmer_colours", "case_store", "window", "window.admin", "link.needs", "time_off", "reminders.mail", "reminders.mail_newsletters", "reminders.gather", "free_time.nothing",
                "dnd.button", "dnd.focus", "dnd.pauses", "dnd.sleep", "reminders.before_event", "reminders.events", "reminders.asked_days", "reminders.waits", "reminders.payment_days", "reminders_closed", "reminders.gathered",
                "free_time.moves", "free_time.latest_after", "free_time.movement", "link.attention.pause", "pause.helps", "pause.grounding", "pause.breathing", "pause.pace", "pause.after", "pause.country", "invoice.name", "invoice.address", "invoice.siret", "invoice.vat",
                "invoice.prefix", "invoice.currency", "invoice.payment", "invoice.folder", "invoice.rate", "mcp.outside_projects", "mcp.texts", "mcp.spam"
            ]
        );
        // What reaches you: its switches in their own tab; the matrix is the tab's (`attention::grid`), no setting of it here.
        let attention: Vec<Setting> = for_view("parameters", &config, &tr, &[], None).into_iter().filter(|s| s.section == "attention").collect();
        assert_eq!(attention.iter().map(|s| s.key.as_str()).collect::<Vec<_>>(), ["reminders.mail", "reminders.mail_newsletters", "reminders.gather", "free_time.nothing", "dnd.button", "dnd.focus", "dnd.pauses", "dnd.sleep"]);
        assert!(attention.iter().all(|s| !s.label.starts_with("set-") && !s.help.starts_with("set-")));
        // Nothing is set in two places: a setting has one owner. The reading
        // panel's own ("Aa" in Notes) are shown in Mail's and the Porch's too,
        // where messages are read, as each lane's are in the Porch's.
        let mut seen: std::collections::BTreeMap<String, &str> = std::collections::BTreeMap::new();
        let keys = |view: &str| keys(view).into_iter().filter(|k| view == "reading" || !k.starts_with("reading.")).collect::<Vec<_>>();
        assert_eq!(keys("reading"), vec!["reading.family", "reading.size", "reading.spacing"]);
        // Who is on which list; when each list reaches you is the matrix's (What reaches you ▸ By person).
        assert_eq!(keys("senders"), vec!["safe", "neutral", "restricted", "blocked"]);
        assert_eq!(keys("contacts"), vec!["map.geocode", "map.tiles", "contacts.region"]);
        let regions = for_view("contacts", &config, &tr, &[], None).into_iter().find(|s| s.key == "contacts.region").unwrap().choices;
        assert!(regions.len() > 30 && regions.iter().all(|c| !c.label.starts_with("country-")), "{regions:?}");
        for view in ["porch", "mail", "accounts", "account:a", "senders", "agenda", "tasks", "notes", "reading", "sites", "contacts", "parameters"] {
            for key in keys(view) {
                if let Some(other) = seen.insert(key.clone(), view) {
                    panic!("{key} is set in {other} and in {view}");
                }
            }
        }
        for view in ["notes", "mail", "account:a", "tasks", "porch", "parameters", "sites", "contacts", "agenda"] {
            assert!(for_view(view, &config, &tr, &[], None).iter().all(|s| !s.label.starts_with("set-") && !s.help.starts_with("set-")), "{view}: a sentence is missing");
        }
    }

    /// The words tab: the languages and countries ticked, then each line's
    /// note and lists, every word said in both languages; the automatic
    /// senders' older whole list shown as it wins, then written as a
    /// difference; a named list's chips read back; a line taken back.
    #[test]
    fn the_words_tab() {
        let dir = std::env::temp_dir().join(format!("sioul-words-tab-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("config.toml");
        std::fs::write(&path, "filed_words = [\"noreply\", \"robot\"]\n\n[words]\nlanguages = [\"fr\", \"en\"]\ncountries = [\"FR\"]\n").unwrap();
        let config = Config::load(&path).unwrap();
        for language in ["en", "fr"] {
            let rows: Vec<Setting> = for_view("parameters", &config, &Translator::new(language), &[], None).into_iter().filter(|s| s.section == "words").collect();
            assert_eq!((rows[0].key.as_str(), rows[1].key.as_str()), ("words.languages", "words.countries"));
            assert!(rows[..2].iter().all(|s| !s.choices.is_empty() && s.choices.iter().all(|c| !c.label.starts_with("words-") && !c.label.starts_with("country-"))), "{language}");
            for (_, line, lists) in WORD_LINES {
                let note = rows.iter().find(|s| s.key == format!("words.about.{line}")).unwrap_or_else(|| panic!("{line}"));
                let said = [&note.label, &note.help, &note.unit, &note.group];
                assert!(said.iter().all(|w| !w.is_empty() && !w.starts_with("words-")), "{language}: {line}: {note:?}");
                for list in lists.iter() {
                    let row = rows.iter().find(|s| s.key == format!("words.{list}")).unwrap_or_else(|| panic!("{list}"));
                    assert!(row.kind == Kind::Words && !row.label.starts_with("words-") && !row.help.starts_with("words-"), "{language}: {list}: {row:?}");
                }
            }
        }
        let rows = for_view("parameters", &config, &Translator::new("en"), &[], None);
        let row = |key: &str| rows.iter().find(|s| s.key == key).cloned().unwrap();
        // The older whole list as it wins, your own words first.
        assert_eq!(row("words.senders.automatic").value, SettingValue::Texts(vec!["robot".into(), "noreply".into()]));
        assert!(row("words.about.senders").help.contains("added"), "{:?}", row("words.about.senders"));
        apply(&path, &config, "words.senders.automatic", &SettingValue::Texts(vec!["noreply".into(), "no-reply".into(), "robot".into()])).unwrap();
        let text = std::fs::read_to_string(&path).unwrap();
        assert!(!text.contains("filed_words") && text.contains("[words.senders.automatic]"), "{text}");
        let config = Config::load(&path).unwrap();
        let mut automatic = crate::words::Words::of(&config).senders.automatic.clone();
        automatic.sort();
        assert_eq!(automatic, ["no-reply", "noreply", "robot"]);
        // A brand of yours, with its domains; one without them refused.
        let SettingValue::Texts(mut chips) = row("words.brands.brands").value else { panic!("brands") };
        chips.push("Ma Banque: mabanque.example, mabanque.fr".into());
        apply(&path, &config, "words.brands.brands", &SettingValue::Texts(chips.clone())).unwrap();
        let config = Config::load(&path).unwrap();
        assert!(crate::words::Words::of(&config).brands.brands.iter().any(|(n, d)| n == "Ma Banque" && d.len() == 2));
        chips.push("Sans Domaine".into());
        assert!(apply(&path, &config, "words.brands.brands", &SettingValue::Texts(chips)).is_err());
        // One language at least; then lines taken back to the packs.
        assert!(apply(&path, &config, "words.languages", &serde_json::from_str("[]").unwrap()).is_err());
        apply(&path, &config, "words.languages", &SettingValue::Texts(vec!["en".into()])).unwrap();
        apply(&path, &config, "words.reset.senders", &SettingValue::Bool(true)).unwrap();
        apply(&path, &config, "words.reset.brands", &SettingValue::Bool(true)).unwrap();
        let config = Config::load(&path).unwrap();
        assert_eq!(crate::words::languages(&config), ["en"]);
        let text = std::fs::read_to_string(&path).unwrap();
        assert!(!text.contains("senders") && !text.contains("brands"), "{text}");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn lists_emptied_and_secrets_kept_out() {
        let dir = std::env::temp_dir().join(format!("sioul-apply-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("config.toml");
        std::fs::write(&path, format!("# Mine.\nknown_senders = \"{}\"\n", dir.join("known.txt").display().to_string().replace('\\', "/"))).unwrap();
        let config = Config::load(&path).unwrap();
        apply(&path, &config, "known", &SettingValue::Texts(vec!["jane@example.org".into()])).unwrap();
        assert_eq!(SenderList::load(&config.known_senders_path()).entries(), ["jane@example.org"]);
        // The last one taken away: the window sends `[]`.
        apply(&path, &config, "known", &serde_json::from_str("[]").unwrap()).unwrap();
        assert!(SenderList::load(&config.known_senders_path()).entries().is_empty());
        // The spam filter's thresholds: the doubt stays below the spam, each between 0 and 1.
        assert!(apply(&path, &config, "spam.threshold_unsure", &SettingValue::Float(0.97)).is_err());
        assert!(apply(&path, &config, "spam.threshold_spam", &SettingValue::Float(1.5)).is_err());
        assert!(apply(&path, &config, "spam.actions.spam", &SettingValue::Text("delete".into())).is_err());
        assert!(apply(&path, &config, "spam.actions.maybe", &SettingValue::Text("move".into())).is_err());
        apply(&path, &config, "spam.threshold_spam", &SettingValue::Float(0.9)).unwrap();
        // An older Sioul's mode, "act": spam moved, doubts flagged; a row changed keeps the others as they read.
        set_value(&path, "spam.mode", &SettingValue::Text("act".into())).unwrap();
        let config = Config::load(&path).unwrap();
        use crate::spam::{Action, Actions};
        assert_eq!(config.spam.actions(), Actions { spam: Action::Move, unsure: Action::Flag, ham: Action::Nothing });
        apply(&path, &config, "spam.actions.ham", &SettingValue::Text("flag".into())).unwrap();
        let config = Config::load(&path).unwrap();
        assert_eq!((config.spam.actions(), config.spam.thresholds(), config.spam.mode.as_deref()), (Actions { spam: Action::Move, unsure: Action::Flag, ham: Action::Flag }, (0.9, 0.5), None));
        let matrix = for_view("mail", &config, &Translator::new("en"), &[], None).into_iter().find(|s| s.key == "spam.actions").unwrap();
        assert_eq!((matrix.kind, matrix.rows.len(), matrix.choices.len()), (Kind::Radios, 3, 3));
        assert_eq!(matrix.value, SettingValue::Texts(vec!["spam:move".into(), "unsure:flag".into(), "ham:flag".into()]));
        assert!(apply(&path, &config, "spam.threshold_unsure", &SettingValue::Float(0.9)).is_err());
        // Shown as percentages, each slider stopping a point short of the other.
        let rows = for_view("mail", &config, &Translator::new("en"), &[], None);
        let range = |key: &str| rows.iter().find(|s| s.key == key).map(|s| ((s.min * 100.0).round(), (s.max * 100.0).round(), s.unit.clone())).unwrap();
        assert_eq!((range("spam.threshold_spam"), range("spam.threshold_unsure")), ((51.0, 99.0, "%".to_string()), (5.0, 89.0, "%".to_string())));
        assert!(rows.iter().any(|s| s.key == "spam.filter" && s.kind == Kind::Spam));
        // A secret is never written into the configuration.
        assert!(apply(&path, &config, "github_token", &SettingValue::Int(12345)).is_err());
        assert!(!std::fs::read_to_string(&path).unwrap().contains("github_token"));
        let _ = std::fs::remove_dir_all(&dir);
    }
}
