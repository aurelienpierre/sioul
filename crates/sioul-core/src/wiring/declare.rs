// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

//! The declarations (docs/rules.md, "The declarations"): each type of object
//! says what can happen to it (`EventDecl`), what can be read on it for a
//! condition (`FieldDecl`, each with its tests and the kind of value they
//! take) and what can be done to it (`ActionDecl`, each with its parameters,
//! what it writes, whether it can be undone, and how it is made once). Data
//! in code: the editor reads them through `catalogue`, the store checks a
//! rule against them (`known`), the engine runs only what they say is
//! available in this Sioul's phase (`PHASE`).
//!
//! What a later phase brings is declared already, `phase` above `PHASE`:
//! the editor shows it as coming, never offers it, and the engine never
//! runs it. A rule that names something not declared at all (a newer Sioul
//! wrote it) is kept and not run (`store`).

use crate::i18n::Translator;
use serde::Serialize;

/// The phase of the wiring this Sioul runs: what is declared with a phase
/// above it is shown as coming.
pub const PHASE: u8 = 1;

/// The longest chain of rules: a rule acting on what another rule made, at
/// most this many steps from the first object (design, decision 5).
pub const CHAIN: u8 = 3;

/// What a value is: what the editor asks, and how the store reads it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum Value {
    /// Free text, compared case and accents aside.
    Text,
    /// Whole words or phrases, several (any of them).
    Words,
    /// Addresses, several.
    Addresses,
    /// Domains, several: each covers its subdomains.
    Domains,
    /// One of a list (`FieldDecl::choices`).
    Choice,
    /// Days of the week (`rules::DAYS`).
    Days,
    /// A time of day, "18:00".
    Time,
    /// A day, "2026-10-01".
    Date,
    /// A size, "5 MB".
    Size,
    /// A topic, by its id.
    Topic,
    /// A project, by its id.
    Project,
    /// A contact card, by its UID.
    Contact,
    /// One of your mail accounts, by its id.
    Account,
    /// A task list or a calendar, "account/id".
    List,
    /// A model of task (`store::Model`), by its id; a project's, `"<project>/<model>"`.
    Model,
    /// A whole number.
    Number,
    /// How a notice reaches you (`LEVELS`).
    Level,
    /// Which attachments: "any", or a kind of attachment (`FILE_TYPES`).
    FileType,
    /// A paper's kind (`papers::Kind`), by its id.
    PaperKind,
    /// A person your admin is done for (`persons`), by id; "me" for you.
    Person,
    /// Words with inserts from the object: {sender}, {month}, {year}, {subject}, {file}.
    Pattern,
    /// None: "has an attachment".
    None,
}

/// The inserts a pattern takes (`Value::Pattern`), in the order the editor offers them.
pub const INSERTS: &[&str] = &["sender", "month", "year", "subject", "file"];

/// Which attachments `keep-attachments` keeps: any, or one kind (`rules::KINDS`), PDF first.
pub const FILE_TYPES: &[&str] = &["pdf", "any", "image", "document", "archive", "calendar", "audio", "video", "text"];

/// What an action writes: the engine's guard reads it (`forbidden`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum Writes {
    /// A tie between two things (`links.toml`).
    Tie,
    /// A task, through the vdir's one door.
    Task,
    /// A word on a message's topic (`topics::labels`).
    Topic,
    /// A notice of Sioul's own.
    Notice,
    /// A mark on a message on its server: read, flagged, a keyword.
    MailMark,
    /// A message moved on its server: a folder, the archive, the spam folder.
    MailFolder,
    /// A message into the trash: a deletion, which no rule may do.
    Trash,
    /// An event, through the vdir's one door and `agenda::guard`.
    Event,
    /// A money movement planned: a record of yours, never a payment.
    Movement,
    /// A paper kept from a message or a file.
    Paper,
    /// A supply's movement (used, received).
    Supply,
    /// A call let ring or declined (the phone's screening).
    Call,
    /// A paper letter read (its text, its kind).
    Letter,
    /// A state of the device: do-not-disturb, a pause.
    Mode,
    /// A message sent: no rule may.
    Send,
    /// Money paid or moved: no rule may.
    Pay,
    /// A setting or a rule: no rule may.
    Setting,
    /// Health's records: no rule may.
    Health,
}

/// Why the engine never does what an action writes, whatever a rule says
/// (design, part 5): a message id; none for what a rule may write.
pub fn forbidden(writes: Writes) -> Option<&'static str> {
    match writes {
        Writes::Send => Some("rules-refused-sends"),
        Writes::Trash => Some("rules-refused-deletes"),
        Writes::Pay => Some("rules-refused-pays"),
        Writes::Setting => Some("rules-refused-setting"),
        Writes::Health => Some("rules-refused-health"),
        Writes::Tie | Writes::Task | Writes::Topic | Writes::Notice | Writes::MailMark | Writes::MailFolder | Writes::Event | Writes::Movement | Writes::Paper | Writes::Supply | Writes::Call | Writes::Letter | Writes::Mode => None,
    }
}

/// One choice of a field's value: what Sioul recognises in a message, a circle…
#[derive(Debug, Clone, Copy)]
pub struct Choice {
    pub id: &'static str,
    pub phase: u8,
}

/// A test a field takes, and the value it then asks.
#[derive(Debug, Clone, Copy)]
pub struct Test {
    pub id: &'static str,
    pub value: Value,
}

/// What can be read on an object, for a condition.
#[derive(Debug, Clone, Copy)]
pub struct FieldDecl {
    pub id: &'static str,
    /// Its tests, the usual one first.
    pub tests: &'static [Test],
    /// The values a `Value::Choice` takes.
    pub choices: &'static [Choice],
    /// The choices grow as Sioul learns to recognise more.
    pub open: bool,
    /// It holds only for a sender something proves (the Porch's `Unproven`
    /// reads as nobody, a stranger to your circles).
    pub proof: bool,
    pub phase: u8,
}

/// Something that can happen to an object.
#[derive(Debug, Clone, Copy)]
pub struct EventDecl {
    pub id: &'static str,
    pub phase: u8,
}

/// A parameter of an action.
#[derive(Debug, Clone, Copy)]
pub struct ParamDecl {
    pub id: &'static str,
    pub value: Value,
    pub required: bool,
    /// Its value may be typed or chosen.
    pub fixed: bool,
    /// The fields of the triggering object it may be taken from ("its subject").
    pub from: &'static [&'static str],
}

/// Something that can be done to an object.
#[derive(Debug, Clone, Copy)]
pub struct ActionDecl {
    pub id: &'static str,
    pub params: &'static [ParamDecl],
    pub writes: Writes,
    /// It can be undone (Undo in the status line, and later from the rule's history).
    pub undo: bool,
    /// How it is made once on every device, a message id.
    pub once: &'static str,
    /// What it refuses, message ids.
    pub refuses: &'static [&'static str],
    pub phase: u8,
}

/// A type of object.
#[derive(Debug, Clone, Copy)]
pub struct TypeDecl {
    pub id: &'static str,
    /// The optional feature it belongs to: a type whose feature is off
    /// declares nothing to the rules' editor. None for every type for now.
    pub feature: Option<&'static str>,
    pub events: &'static [EventDecl],
    pub fields: &'static [FieldDecl],
    pub actions: &'static [ActionDecl],
    pub phase: u8,
}

impl TypeDecl {
    pub fn event(&self, id: &str) -> Option<&'static EventDecl> {
        let events: &'static [EventDecl] = self.events;
        events.iter().find(|e| e.id == id)
    }

    pub fn field(&self, id: &str) -> Option<&'static FieldDecl> {
        let fields: &'static [FieldDecl] = self.fields;
        fields.iter().find(|f| f.id == id)
    }

    pub fn action(&self, id: &str) -> Option<&'static ActionDecl> {
        let actions: &'static [ActionDecl] = self.actions;
        actions.iter().find(|a| a.id == id)
    }
}

impl FieldDecl {
    pub fn test(&self, id: &str) -> Option<Test> {
        self.tests.iter().copied().find(|t| t.id == id)
    }

    pub fn choice(&self, id: &str) -> Option<Choice> {
        self.choices.iter().copied().find(|c| c.id == id)
    }
}

impl ActionDecl {
    pub fn param(&self, id: &str) -> Option<&'static ParamDecl> {
        let params: &'static [ParamDecl] = self.params;
        params.iter().find(|p| p.id == id)
    }
}

// ---------------------------------------------------------------- tests and values

const fn test(id: &'static str, value: Value) -> Test {
    Test { id, value }
}

const TEXT_TESTS: &[Test] = &[test("contains", Value::Text), test("not-contains", Value::Text), test("is", Value::Text), test("is-not", Value::Text)];
const CHOICE_TESTS: &[Test] = &[test("is", Value::Choice), test("is-not", Value::Choice)];
const BODY_TESTS: &[Test] = &[test("has-words", Value::Words), test("contains", Value::Text), test("not-contains", Value::Text)];
const SUBJECT_TESTS: &[Test] = &[test("has-words", Value::Words), test("contains", Value::Text), test("not-contains", Value::Text), test("is", Value::Text), test("is-not", Value::Text)];
const PRESENCE_TESTS: &[Test] = &[test("exists", Value::None), test("missing", Value::None), test("contains", Value::Text), test("not-contains", Value::Text)];

/// The circles a sender may be in for you (`rules::WHO`).
pub const CIRCLES: &[Choice] = &[Choice { id: "known", phase: 1 }, Choice { id: "safe", phase: 1 }, Choice { id: "neutral", phase: 1 }, Choice { id: "restricted", phase: 1 }, Choice { id: "stranger", phase: 1 }];

/// The kinds of attachment (`rules::KINDS`).
pub const ATTACHMENT_KINDS: &[Choice] = &[
    Choice { id: "pdf", phase: 1 },
    Choice { id: "image", phase: 1 },
    Choice { id: "document", phase: 1 },
    Choice { id: "archive", phase: 1 },
    Choice { id: "calendar", phase: 1 },
    Choice { id: "audio", phase: 1 },
    Choice { id: "video", phase: 1 },
    Choice { id: "text", phase: 1 },
];

/// What Sioul recognises in a message (`message::kinds_of`): an open list,
/// fed by the recognisers that exist; those of phase 2 come as words in
/// Settings ▸ Words.
pub const KINDS: &[Choice] = &[
    Choice { id: "code", phase: 1 },
    Choice { id: "password", phase: 1 },
    Choice { id: "password-reset", phase: 1 },
    Choice { id: "sign-in-link", phase: 1 },
    Choice { id: "confirmation", phase: 1 },
    Choice { id: "invitation", phase: 1 },
    Choice { id: "newsletter", phase: 1 },
    Choice { id: "payment-received", phase: 1 },
    Choice { id: "payment-made", phase: 1 },
    Choice { id: "order", phase: 1 },
    Choice { id: "bill", phase: 1 },
    Choice { id: "refund", phase: 1 },
    Choice { id: "payment-refused", phase: 2 },
    Choice { id: "sign-in-notice", phase: 2 },
    Choice { id: "site-waits", phase: 2 },
    Choice { id: "parcel", phase: 2 },
    Choice { id: "bounce", phase: 2 },
    Choice { id: "renewal", phase: 2 },
    Choice { id: "reply", phase: 2 },
];

/// How a rule's notice reaches you: now (a notice to know, as the moment
/// allows: never in a pause or a do-not-disturb, waiting while nobody is
/// at the device), or quietly (said in the status line and kept in the
/// rule's history, no notification).
pub const LEVELS: &[Choice] = &[Choice { id: "now", phase: 1 }, Choice { id: "quiet", phase: 1 }];

const fn field(id: &'static str, tests: &'static [Test], proof: bool) -> FieldDecl {
    FieldDecl { id, tests, choices: &[], open: false, proof, phase: 1 }
}

const fn later(id: &'static str, tests: &'static [Test], phase: u8) -> FieldDecl {
    FieldDecl { id, tests, choices: &[], open: false, proof: false, phase }
}

const fn param(id: &'static str, value: Value, required: bool) -> ParamDecl {
    ParamDecl { id, value, required, fixed: true, from: &[] }
}

const fn event(id: &'static str, phase: u8) -> EventDecl {
    EventDecl { id, phase }
}

// ---------------------------------------------------------------- the types

/// A message (mail).
const MESSAGE: TypeDecl = TypeDecl {
    id: "message",
    feature: None,
    phase: 1,
    events: &[event("arrives", 1), event("is-sent", 2), event("is-said-spam", 3)],
    fields: &[
        field("sender", &[test("is", Value::Addresses)], true),
        field("sender-domain", &[test("is", Value::Domains)], true),
        field("from", TEXT_TESTS, true),
        field("reply-to", TEXT_TESTS, true),
        FieldDecl { id: "circle", tests: CHOICE_TESTS, choices: CIRCLES, open: false, proof: true, phase: 1 },
        field("card", &[test("is", Value::Contact), test("exists", Value::None), test("missing", Value::None)], true),
        field("to", TEXT_TESTS, false),
        field("cc", TEXT_TESTS, false),
        field("account", &[test("is", Value::Account), test("is-not", Value::Account)], false),
        field("subject", SUBJECT_TESTS, false),
        field("text", BODY_TESTS, false),
        field("anywhere", &[test("contains", Value::Text), test("not-contains", Value::Text)], false),
        field("attachment", PRESENCE_TESTS, false),
        FieldDecl { id: "attachment-type", tests: CHOICE_TESTS, choices: ATTACHMENT_KINDS, open: false, proof: false, phase: 1 },
        field("list", PRESENCE_TESTS, false),
        field("size", &[test("above", Value::Size), test("below", Value::Size)], false),
        field("date", &[test("after", Value::Date), test("before", Value::Date), test("between", Value::Date)], false),
        field("weekday", &[test("is", Value::Days), test("is-not", Value::Days)], false),
        field("hour", &[test("after", Value::Time), test("before", Value::Time), test("between", Value::Time)], false),
        field("topic", &[test("is", Value::Topic), test("is-not", Value::Topic)], false),
        field("project", &[test("is", Value::Project), test("is-not", Value::Project)], false),
        FieldDecl { id: "kind", tests: CHOICE_TESTS, choices: KINDS, open: true, proof: false, phase: 1 },
        // The amount a message about money states (`payments::detect`): what a planned movement takes.
        later("amount", &[test("above", Value::Number), test("below", Value::Number)], 3),
        // Your spam filter's verdict, which the Porch reads (phase 3: the filter itself stays a learner, never a rule).
        FieldDecl { id: "spam", tests: CHOICE_TESTS, choices: &[Choice { id: "spam", phase: 3 }, Choice { id: "doubt", phase: 3 }], open: false, proof: false, phase: 3 },
        // The folder it arrived in: the inbox, or your provider's spam folder.
        later("folder", &[test("is", Value::Text)], 3),
    ],
    actions: &[
        ActionDecl {
            id: "tie-to-project",
            params: &[ParamDecl { id: "project", value: Value::Project, required: true, fixed: true, from: &["project"] }],
            writes: Writes::Tie,
            undo: true,
            once: "rules-once-tie",
            refuses: &["rules-refused-no-project"],
            phase: 1,
        },
        ActionDecl {
            id: "give-topic",
            params: &[param("topic", Value::Topic, true)],
            writes: Writes::Topic,
            undo: true,
            once: "rules-once-topic",
            refuses: &["rules-refused-no-topic", "rules-refused-your-word", "rules-refused-no-id"],
            phase: 1,
        },
        ActionDecl {
            id: "keep-attachments",
            params: &[
                ParamDecl { id: "type", value: Value::FileType, required: false, fixed: true, from: &[] },
                param("name", Value::Text, false),
                param("kind", Value::PaperKind, false),
                param("title", Value::Pattern, false),
                param("for", Value::Person, false),
            ],
            writes: Writes::Paper,
            undo: true,
            once: "rules-once-paper",
            refuses: &["rules-refused-infected", "rules-refused-too-large", "rules-refused-write", "rules-refused-no-papers"],
            phase: 1,
        },
        ActionDecl { id: "move-to-folder", params: &[param("folder", Value::Text, true)], writes: Writes::MailFolder, undo: true, once: "rules-once-server", refuses: &[], phase: 4 },
        ActionDecl { id: "archive", params: &[], writes: Writes::MailFolder, undo: true, once: "rules-once-server", refuses: &[], phase: 4 },
        ActionDecl { id: "junk", params: &[], writes: Writes::MailFolder, undo: true, once: "rules-once-server", refuses: &[], phase: 4 },
        ActionDecl { id: "mark-read", params: &[], writes: Writes::MailMark, undo: true, once: "rules-once-server", refuses: &[], phase: 4 },
        ActionDecl { id: "flag", params: &[], writes: Writes::MailMark, undo: true, once: "rules-once-server", refuses: &[], phase: 4 },
        ActionDecl { id: "keyword", params: &[param("name", Value::Text, true)], writes: Writes::MailMark, undo: true, once: "rules-once-server", refuses: &[], phase: 4 },
        ActionDecl { id: "trash", params: &[], writes: Writes::Trash, undo: false, once: "rules-once-server", refuses: &["rules-refused-deletes"], phase: 4 },
        ActionDecl { id: "never-topic", params: &[param("topic", Value::Topic, true)], writes: Writes::Topic, undo: true, once: "rules-once-topic", refuses: &[], phase: 4 },
        ActionDecl { id: "tie-to-task", params: &[], writes: Writes::Tie, undo: true, once: "rules-once-tie", refuses: &[], phase: 3 },
        ActionDecl { id: "tie-to-site", params: &[], writes: Writes::Tie, undo: true, once: "rules-once-tie", refuses: &[], phase: 3 },
        // Leaving a list sends a request or a message: never a rule's (a press of yours, as today).
        ActionDecl { id: "leave-list", params: &[], writes: Writes::Send, undo: true, once: "rules-once-server", refuses: &["rules-refused-sends"], phase: 4 },
    ],
};

/// A task.
const TASK: TypeDecl = TypeDecl {
    id: "task",
    feature: None,
    phase: 2,
    events: &[event("is-made", 2), event("is-done", 2), event("is-put-off", 2), event("comes-due", 2)],
    fields: &[
        later("title", &[test("has-words", Value::Words)], 2),
        later("list", &[test("is", Value::List)], 2),
        later("project", &[test("is", Value::Project), test("is-not", Value::Project)], 2),
        later("tag", &[test("is", Value::Text)], 2),
        later("contact", &[test("is", Value::Contact)], 2),
        later("draft", &[test("exists", Value::None), test("missing", Value::None)], 2),
    ],
    actions: &[
        ActionDecl {
            id: "make",
            params: &[
                param("model", Value::Model, true),
                ParamDecl { id: "project", value: Value::Project, required: false, fixed: true, from: &["project"] },
                ParamDecl { id: "title", value: Value::Text, required: false, fixed: true, from: &["subject"] },
                param("days", Value::Number, false),
            ],
            writes: Writes::Task,
            undo: true,
            once: "rules-once-made",
            refuses: &["rules-refused-no-model", "rules-refused-no-list", "rules-refused-read-only", "rules-refused-no-project"],
            phase: 1,
        },
        ActionDecl { id: "mark-done", params: &[], writes: Writes::Task, undo: true, once: "rules-once-record", refuses: &["rules-refused-read-only"], phase: 2 },
        ActionDecl { id: "set-date", params: &[param("date", Value::Date, true)], writes: Writes::Task, undo: true, once: "rules-once-record", refuses: &["rules-refused-read-only"], phase: 2 },
        ActionDecl { id: "tag", params: &[param("tag", Value::Text, true)], writes: Writes::Task, undo: true, once: "rules-once-record", refuses: &["rules-refused-read-only"], phase: 2 },
        ActionDecl { id: "move-to-list", params: &[param("list", Value::List, true)], writes: Writes::Task, undo: true, once: "rules-once-record", refuses: &["rules-refused-read-only"], phase: 2 },
        ActionDecl { id: "add-step", params: &[param("title", Value::Text, true)], writes: Writes::Task, undo: true, once: "rules-once-made", refuses: &["rules-refused-read-only"], phase: 2 },
        ActionDecl { id: "set-aside", params: &[], writes: Writes::Task, undo: true, once: "rules-once-record", refuses: &["rules-refused-read-only"], phase: 2 },
        ActionDecl { id: "estimate", params: &[], writes: Writes::Task, undo: true, once: "rules-once-record", refuses: &["rules-refused-read-only"], phase: 3 },
    ],
};

/// An event (an appointment).
const EVENT: TypeDecl = TypeDecl {
    id: "event",
    feature: None,
    phase: 2,
    events: &[event("is-made", 2), event("moves", 2), event("starts-soon", 2), event("is-over", 2)],
    fields: &[
        later("title", &[test("has-words", Value::Words)], 2),
        later("calendar", &[test("is", Value::List)], 2),
        later("project", &[test("is", Value::Project), test("is-not", Value::Project)], 2),
        later("guests", &[test("exists", Value::None), test("missing", Value::None)], 2),
    ],
    actions: &[
        ActionDecl { id: "make", params: &[param("calendar", Value::List, true), param("title", Value::Text, true)], writes: Writes::Event, undo: true, once: "rules-once-made", refuses: &["rules-refused-read-only"], phase: 2 },
        ActionDecl { id: "move", params: &[param("minutes", Value::Number, true)], writes: Writes::Event, undo: true, once: "rules-once-record", refuses: &["rules-refused-meeting", "rules-refused-read-only"], phase: 2 },
        ActionDecl { id: "pause", params: &[], writes: Writes::Event, undo: true, once: "rules-once-record", refuses: &["rules-refused-meeting"], phase: 3 },
        ActionDecl { id: "answer", params: &[], writes: Writes::Event, undo: true, once: "rules-once-record", refuses: &["rules-refused-meeting"], phase: 3 },
    ],
};

/// A money movement: a record of yours, never a payment. Here for the
/// budgets' mail rules, which `readers` shows (phase 3).
const MOVEMENT: TypeDecl = TypeDecl {
    id: "movement",
    feature: None,
    phase: 3,
    events: &[event("appears", 3), event("did-not-leave", 3), event("day-comes", 3)],
    fields: &[],
    actions: &[
        ActionDecl { id: "join", params: &[], writes: Writes::Movement, undo: true, once: "rules-once-record", refuses: &[], phase: 3 },
        ActionDecl {
            id: "plan",
            params: &[
                param("budget", Value::Text, true),
                ParamDecl { id: "direction", value: Value::Choice, required: true, fixed: true, from: &[] },
                ParamDecl { id: "amount", value: Value::Text, required: false, fixed: true, from: &["amount"] },
                param("label", Value::Text, false),
                param("preset", Value::Text, false),
            ],
            writes: Writes::Movement,
            undo: true,
            once: "rules-once-made",
            refuses: &[],
            phase: 3,
        },
    ],
};

/// The types the presets name whose events and actions come later (phase 3),
/// with no fields yet.
const fn later_type(id: &'static str, events: &'static [EventDecl], actions: &'static [ActionDecl]) -> TypeDecl {
    TypeDecl { id, feature: None, phase: 3, events, fields: &[], actions }
}

const fn later_action(id: &'static str, writes: Writes) -> ActionDecl {
    ActionDecl { id, params: &[], writes, undo: true, once: "rules-once-record", refuses: &[], phase: 3 }
}

const SUPPLY: TypeDecl = later_type("supply", &[event("runs-low", 3), event("comes-in", 3)], &[later_action("take-out", Writes::Supply)]);
const CALL: TypeDecl = later_type("call", &[event("rings", 3), event("is-missed", 3), event("is-answered", 3)], &[later_action("decline", Writes::Call)]);
const PAPER: TypeDecl = later_type("paper", &[event("renewal-nears", 3)], &[later_action("keep", Writes::Paper)]);
const LETTER: TypeDecl = later_type("letter", &[event("scan-arrives", 3)], &[later_action("read", Writes::Letter)]);
const CONTRACT: TypeDecl = later_type("contract", &[event("notice-nears", 3)], &[]);
const INVOICE: TypeDecl = later_type("invoice", &[event("is-issued", 3), event("is-due", 3)], &[]);
const SITE: TypeDecl = later_type("site", &[event("offers-file", 3), event("notifies", 3)], &[]);
const CLOCK: TypeDecl = later_type("clock", &[event("moment-begins", 3), event("day-comes", 3)], &[]);

/// Sioul itself: the action that belongs to no object.
const SIOUL: TypeDecl = TypeDecl {
    id: "none",
    feature: None,
    phase: 1,
    events: &[],
    fields: &[],
    actions: &[
        ActionDecl {
            id: "tell",
            params: &[ParamDecl { id: "level", value: Value::Level, required: false, fixed: true, from: &[] }, param("words", Value::Text, false)],
            writes: Writes::Notice,
            undo: false,
            once: "rules-once-tell",
            refuses: &[],
            phase: 1,
        },
        later_action("do-not-disturb", Writes::Mode),
    ],
};

/// Every type, in the order the editor offers them.
pub const TYPES: &[TypeDecl] = &[MESSAGE, TASK, EVENT, MOVEMENT, SUPPLY, CALL, PAPER, LETTER, CONTRACT, INVOICE, SITE, CLOCK, SIOUL];

/// A type by its id.
pub fn of(id: &str) -> Option<&'static TypeDecl> {
    TYPES.iter().find(|t| t.id == id)
}

/// Whether this Sioul runs what is declared with `phase`.
pub fn available(phase: u8) -> bool {
    phase <= PHASE
}

// ---------------------------------------------------------------- the catalogue

/// The declarations as the window reads them, in the person's words.
#[derive(Debug, Clone, Serialize)]
pub struct Catalogue {
    /// The store's format this Sioul writes (`store::FORMAT`).
    pub format: u32,
    /// The phase it runs.
    pub phase: u8,
    pub types: Vec<TypeOut>,
    /// The levels of `tell`.
    pub levels: Vec<ChoiceOut>,
}

#[derive(Debug, Clone, Serialize)]
pub struct TypeOut {
    pub id: &'static str,
    /// The optional feature it belongs to (`TypeDecl::feature`).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub feature: Option<&'static str>,
    pub name: String,
    pub available: bool,
    pub phase: u8,
    pub events: Vec<Named>,
    pub fields: Vec<FieldOut>,
    pub actions: Vec<ActionOut>,
}

#[derive(Debug, Clone, Serialize)]
pub struct Named {
    pub id: &'static str,
    pub name: String,
    pub available: bool,
    pub phase: u8,
}

#[derive(Debug, Clone, Serialize)]
pub struct ChoiceOut {
    pub id: &'static str,
    pub name: String,
    pub available: bool,
    pub phase: u8,
}

#[derive(Debug, Clone, Serialize)]
pub struct TestOut {
    pub id: &'static str,
    pub name: String,
    pub value: Value,
}

#[derive(Debug, Clone, Serialize)]
pub struct FieldOut {
    pub id: &'static str,
    pub name: String,
    pub tests: Vec<TestOut>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub choices: Vec<ChoiceOut>,
    pub open: bool,
    pub proof: bool,
    pub available: bool,
    pub phase: u8,
}

#[derive(Debug, Clone, Serialize)]
pub struct ParamOut {
    pub id: &'static str,
    pub name: String,
    pub value: Value,
    pub required: bool,
    pub fixed: bool,
    /// The fields of the triggering object it may be taken from, with their names.
    pub from: Vec<Named>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ActionOut {
    pub id: &'static str,
    pub name: String,
    pub params: Vec<ParamOut>,
    pub writes: Writes,
    pub undo: bool,
    pub once: String,
    pub refuses: Vec<String>,
    pub available: bool,
    pub phase: u8,
}

/// The name of a field of a type, in words ("its sender's domain").
pub fn field_name(tr: &Translator, kind: &str, field: &str) -> String {
    tr.text(&format!("rules-field-{kind}-{field}"), None)
}

/// The name of a test ("is", "has the words").
pub fn test_name(tr: &Translator, test: &str) -> String {
    tr.text(&format!("rules-test-{test}"), None)
}

/// The name of one of a field's choices ("a code", "safe").
pub fn choice_name(tr: &Translator, field: &str, choice: &str) -> String {
    tr.text(&format!("rules-choice-{field}-{choice}"), None)
}

/// The declarations, each named in the person's language.
pub fn catalogue(tr: &Translator) -> Catalogue {
    let types = TYPES
        .iter()
        .map(|t| TypeOut {
            id: t.id,
            feature: t.feature,
            name: tr.text(&format!("rules-type-{}", t.id), None),
            available: available(t.phase) || t.actions.iter().any(|a| available(a.phase)),
            phase: t.phase,
            events: t.events.iter().map(|e| Named { id: e.id, name: tr.text(&format!("rules-event-{}-{}", t.id, e.id), None), available: available(e.phase), phase: e.phase }).collect(),
            fields: t
                .fields
                .iter()
                .map(|f| FieldOut {
                    id: f.id,
                    name: field_name(tr, t.id, f.id),
                    tests: f.tests.iter().map(|x| TestOut { id: x.id, name: test_name(tr, x.id), value: x.value }).collect(),
                    choices: f.choices.iter().map(|c| ChoiceOut { id: c.id, name: choice_name(tr, f.id, c.id), available: available(c.phase), phase: c.phase }).collect(),
                    open: f.open,
                    proof: f.proof,
                    available: available(f.phase),
                    phase: f.phase,
                })
                .collect(),
            actions: t
                .actions
                .iter()
                .map(|a| ActionOut {
                    id: a.id,
                    name: tr.text(&format!("rules-action-{}-{}", t.id, a.id), None),
                    params: a
                        .params
                        .iter()
                        .map(|p| ParamOut {
                            id: p.id,
                            name: tr.text(&format!("rules-param-{}", p.id), None),
                            value: p.value,
                            required: p.required,
                            fixed: p.fixed,
                            from: p.from.iter().map(|f| Named { id: f, name: field_name(tr, "message", f), available: true, phase: 1 }).collect(),
                        })
                        .collect(),
                    writes: a.writes,
                    undo: a.undo,
                    once: tr.text(a.once, None),
                    refuses: a.refuses.iter().map(|r| tr.text(r, None)).collect(),
                    available: available(a.phase) && forbidden(a.writes).is_none(),
                    phase: a.phase,
                })
                .collect(),
        })
        .collect();
    Catalogue { format: super::store::FORMAT, phase: PHASE, types, levels: LEVELS.iter().map(|c| ChoiceOut { id: c.id, name: tr.text(&format!("rules-level-{}", c.id), None), available: true, phase: c.phase }).collect() }
}

// ---------------------------------------------------------------- presets

/// A parameter of a preset that is a setting today, by its key in the configuration.
#[derive(Debug, Clone, Serialize)]
pub struct PresetParam {
    pub param: &'static str,
    pub key: &'static str,
}

/// What Sioul does by itself today, said as a rule (design, part 6, the
/// owner's word of 24 November: the existing behaviours are presets of the
/// wiring). Its rule has the shape of a stored one, as far as the
/// declarations can say it, so that the one sentence function says it; it
/// keeps running where and how it runs today (`runs`), and moves into the
/// engine in its `phase`. In phase 1 a change to a preset writes the setting
/// it has today (`setting`, `params`): one truth, nothing new.
#[derive(Debug, Clone, Serialize)]
pub struct Preset {
    /// Stable: a rule made from it remembers it (`store::Rule::preset`).
    pub id: &'static str,
    /// Which of Sioul's automatic acts it is, by a short reference ("A.12": mail, B tasks, C the clock, D events, E calls, F money, G papers and letters, I sites).
    pub entry: &'static str,
    /// Its rule, id `"preset:<id>"`, on as Sioul ships it.
    pub rule: super::store::Rule,
    /// The setting that turns it on or off today, by its key; none when nothing does.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub setting: Option<&'static str>,
    /// Its parameters that are settings today.
    pub params: Vec<PresetParam>,
    /// "as-today": its own module runs it, the engine never does.
    pub runs: &'static str,
    /// The phase in which it moves into the engine.
    pub phase: u8,
    /// A rule can start from it now ("Add a rule… ▸ start from a preset"):
    /// the engine runs what it says.
    pub offered: bool,
}

/// A preset as data: its rule's parts, by ids.
struct PresetDecl {
    id: &'static str,
    entry: &'static str,
    when: (&'static str, &'static str),
    conditions: &'static [(&'static str, &'static str, &'static [&'static str])],
    then: (&'static str, &'static str),
    /// Each parameter: a fixed value, or `"from:<field>"`.
    with: &'static [(&'static str, &'static str)],
    setting: Option<&'static str>,
    params: &'static [(&'static str, &'static str)],
    phase: u8,
}

const CODES: &[&str] = &["code", "password", "password-reset", "sign-in-link", "confirmation"];
const MONEY: &[&str] = &["payment-received", "payment-made", "order", "bill", "refund"];

const fn preset(id: &'static str, entry: &'static str, when: (&'static str, &'static str), then: (&'static str, &'static str), phase: u8) -> PresetDecl {
    PresetDecl { id, entry, when, conditions: &[], then, with: &[], setting: None, params: &[], phase }
}

/// What Sioul does by itself, each automatic act of its modules: those a
/// message, a task or an event triggers first, then the others.
/// Left out, being no "when this happens, do that": the Porch's lanes and
/// circles (readings), the notices' model and the matrix (what reaches you
/// keeps its own place), the plan's reading of hours, health's needs (health
/// is never a rule's), the sharing and the devices' dispatch, the
/// trainings, files handed by other applications, the extensions' refresh.
const PRESETS: &[PresetDecl] = &[
    PresetDecl { with: &[("project", "from:project")], ..preset("conversation-follows-project", "A.2", ("message", "arrives"), ("message", "tie-to-project"), 3) },
    PresetDecl { conditions: &[("sender-domain", "is", &["github.com"])], setting: Some("github.enabled"), ..preset("github-mail", "A.4", ("message", "arrives"), ("message", "tie-to-task"), 3) },
    PresetDecl { conditions: &[("spam", "is", &["spam"])], setting: Some("spam.actions"), ..preset("spam-filter-moves", "A.8", ("message", "arrives"), ("message", "junk"), 4) },
    PresetDecl { conditions: &[("folder", "is", &["junk"]), ("circle", "is", &["known"])], with: &[("folder", "inbox")], setting: Some("spam.proven"), ..preset("proven-mail-brought-back", "A.9", ("message", "arrives"), ("message", "move-to-folder"), 4) },
    PresetDecl { with: &[("topic", "from:topic")], ..preset("topics-given", "A.11", ("message", "arrives"), ("message", "give-topic"), 4) },
    PresetDecl { conditions: &[("kind", "is", CODES)], with: &[("level", "now")], ..preset("codes-told", "A.12", ("message", "arrives"), ("none", "tell"), 1) },
    PresetDecl { with: &[("level", "now")], setting: Some("reminders.mail"), params: &[("newsletters", "reminders.mail_newsletters")], ..preset("new-mail-told", "A.14", ("message", "arrives"), ("none", "tell"), 3) },
    PresetDecl { setting: Some("mail.leave_on_spam"), ..preset("spam-leaves-list", "A.6", ("message", "is-said-spam"), ("message", "leave-list"), 4) },
    PresetDecl { conditions: &[("kind", "is", &["invitation"])], ..preset("invitation-replies-read", "D.5", ("message", "arrives"), ("event", "answer"), 3) },
    PresetDecl { conditions: &[("kind", "is", MONEY)], with: &[("amount", "from:amount")], ..preset("money-mail-planned", "F.1", ("message", "arrives"), ("movement", "plan"), 3) },
    PresetDecl { conditions: &[("kind", "is", MONEY)], ..preset("money-mail-joined", "F.2", ("message", "arrives"), ("movement", "join"), 3) },
    preset("site-announced", "I.4", ("message", "arrives"), ("message", "tie-to-site"), 3),
    preset("supplies-used", "B.1", ("task", "is-done"), ("supply", "take-out"), 3),
    PresetDecl { params: &[("list", "supplies.list")], ..preset("resupply-task", "B.2", ("supply", "runs-low"), ("task", "make"), 3) },
    preset("repeating-task-next-turn", "B.3", ("task", "is-done"), ("task", "set-date"), 3),
    preset("bigger-task-closes", "B.4", ("task", "is-done"), ("task", "mark-done"), 2),
    PresetDecl { with: &[("level", "now")], setting: Some("reminders.waits"), ..preset("wait-over-told", "B.5", ("task", "is-done"), ("none", "tell"), 2) },
    preset("time-blocks-follow", "B.6", ("task", "is-put-off"), ("event", "move"), 3),
    preset("first-estimate", "B.7", ("task", "is-made"), ("task", "estimate"), 3),
    PresetDecl { with: &[("level", "now")], params: &[("days", "reminders.asked_days")], ..preset("date-asked-reminded", "C.3", ("task", "comes-due"), ("none", "tell"), 2) },
    PresetDecl { with: &[("level", "now")], params: &[("minutes", "reminders.before_event")], ..preset("event-reminded", "C.3", ("event", "starts-soon"), ("none", "tell"), 2) },
    preset("preparation-set-aside", "D.1", ("event", "is-over"), ("task", "set-aside"), 2),
    preset("paired-task-follows", "D.2", ("event", "moves"), ("task", "set-date"), 2),
    PresetDecl { with: &[("level", "now")], ..preset("child-event-asked", "D.3", ("event", "is-made"), ("none", "tell"), 3) },
    preset("child-event-follow-up", "D.3", ("event", "is-over"), ("task", "make"), 3),
    PresetDecl { with: &[("level", "now")], ..preset("conflict-said", "D.4", ("event", "moves"), ("none", "tell"), 3) },
    preset("series-paused", "D.6", ("clock", "day-comes"), ("event", "pause"), 3),
    preset("availability-mirrored", "D.7", ("event", "is-made"), ("event", "make"), 3),
    preset("plan-fills-days", "C.1", ("clock", "day-comes"), ("task", "set-date"), 3),
    preset("do-not-disturb-by-reasons", "C.7", ("clock", "moment-begins"), ("none", "do-not-disturb"), 3),
    PresetDecl { with: &[("level", "now")], ..preset("work-hours-over", "C.11", ("clock", "moment-begins"), ("none", "tell"), 3) },
    PresetDecl { with: &[("level", "now")], ..preset("books-thresholds", "C.12", ("invoice", "is-issued"), ("none", "tell"), 3) },
    preset("calls-screened", "E.1", ("call", "rings"), ("call", "decline"), 3),
    preset("missed-call-task", "E.2", ("call", "is-missed"), ("task", "make"), 3),
    preset("call-closes-task", "E.2", ("call", "is-answered"), ("task", "mark-done"), 3),
    preset("bank-line-joined", "F.3", ("movement", "appears"), ("movement", "join"), 3),
    preset("budget-presets-counted", "F.4", ("movement", "day-comes"), ("movement", "plan"), 3),
    PresetDecl { with: &[("level", "now")], ..preset("contract-notice-told", "F.6", ("contract", "notice-nears"), ("none", "tell"), 3) },
    PresetDecl { with: &[("level", "now")], params: &[("days", "reminders.payment_days")], ..preset("invoice-due-told", "F.7", ("invoice", "is-due"), ("none", "tell"), 3) },
    preset("letter-read", "G.1", ("letter", "scan-arrives"), ("letter", "read"), 3),
    PresetDecl { with: &[("level", "now")], ..preset("paper-renewal-told", "G.2", ("paper", "renewal-nears"), ("none", "tell"), 3) },
    preset("site-file-kept", "G.3", ("site", "offers-file"), ("paper", "keep"), 3),
];

/// Presets the engine could run but whose own module runs them already:
/// a rule made from one would tell twice.
const BESIDE_ITS_OWN: &[&str] = &["codes-told"];

/// The rule a preset says.
fn preset_rule(p: &PresetDecl) -> super::store::Rule {
    use super::store::{Condition, Param, Rule, Then, When};
    let conditions = p.conditions.iter().map(|(field, test, values)| Condition { field: (*field).into(), test: (*test).into(), value: values.iter().map(|v| (*v).to_string()).collect(), until: String::new() }).collect();
    let with = p.with.iter().map(|(key, value)| ((*key).to_string(), value.strip_prefix("from:").map_or_else(|| Param::Text((*value).to_string()), |field| Param::From { from: field.to_string() }))).collect();
    Rule {
        id: format!("preset:{}", p.id),
        on: true,
        when: When { kind: p.when.0.into(), event: p.when.1.into(), conditions, ..When::default() },
        then: Then { kind: p.then.0.into(), action: p.then.1.into(), with },
        preset: p.id.to_string(),
        ..Rule::default()
    }
}

/// Every preset, those a message, a task or an event triggers first.
pub fn presets() -> Vec<Preset> {
    PRESETS
        .iter()
        .map(|p| {
            let rule = preset_rule(p);
            // A preset whose own module still tells what it says (the codes' notice) is never offered beside it.
            let offered = available(p.phase) && rule.problem().is_none() && !BESIDE_ITS_OWN.contains(&p.id);
            Preset { id: p.id, entry: p.entry, rule, setting: p.setting, params: p.params.iter().map(|&(param, key)| PresetParam { param, key }).collect(), runs: "as-today", phase: p.phase, offered }
        })
        .collect()
}

/// A draft rule from a preset, off, not stored, remembering where it came
/// from: only for a preset the engine runs now (`Preset::offered`).
pub fn from_preset(id: &str) -> Option<super::store::Rule> {
    let preset = presets().into_iter().find(|p| p.id == id && p.offered)?;
    Some(super::store::Rule { id: String::new(), on: false, since: 0, ..preset.rule })
}

// ---------------------------------------------------------------- from an example

/// A draft rule from one message ("Make a rule from this…"): off, not
/// stored; when a message arrives from the same domain (only when something
/// proves its sender), of the kind Sioul recognises in it, in its project
/// when it has one. Its "then" is left to the person. None when the message
/// cannot be read or the Porch sets it aside.
pub fn example(file: &std::path::Path, config: &crate::config::Config) -> Option<super::store::Rule> {
    use super::store::{Condition, Rule, Then, When};
    let now = crate::zone::now().timestamp().as_second();
    let ties = crate::links::LocalLinks::load(&crate::links::LocalLinks::default_path());
    let store = config.places().and_then(|places| crate::projects::ProjectStore::open(&places).ok()).map(|s| s.with_ties(&ties));
    let senders = crate::porch::Senders::load(config);
    let judged = crate::porch::judge(&[file.to_path_buf()], &config.mail_sources(), store.as_ref(), &crate::porch::SenderList::default(), &senders, now).into_iter().next()?;
    if super::engine::held(&judged) {
        return None;
    }
    let words = crate::words::Words::of(config);
    let seen = super::message::seen(&judged, &senders, store.as_ref(), &words, None);
    let mut conditions = Vec::new();
    // Its sender's domain, or its address when the domain is a provider millions
    // share (a friend at a webmail) or the sender is someone you have a card for:
    // the rule then reads "from this person", never "from everyone there".
    if seen.proven()
        && let Some(domain) = seen.card().sender_domain()
    {
        conditions.push(sender_condition(&domain, seen.sender(), &words.brands.shared, !seen.cards.is_empty()));
    }
    // What it is rather than how it is sent: a bill, a payment, an order before "a newsletter".
    if let Some(kind) = seen.kinds.iter().find(|k| **k != "newsletter").or(seen.kinds.first()) {
        conditions.push(Condition { field: "kind".into(), test: "is".into(), value: vec![(*kind).to_string()], until: String::new() });
    }
    if let Some(project) = seen.projects.first() {
        conditions.push(Condition { field: "project".into(), test: "is".into(), value: vec![project.clone()], until: String::new() });
    }
    Some(Rule { on: false, when: When { kind: "message".into(), event: "arrives".into(), conditions, ..When::default() }, then: Then::default(), ..Rule::default() })
}

/// Who a rule made from an example is about: the sender's domain, or its
/// address when the domain is a provider millions share (a friend at a
/// webmail) or the sender is someone you keep a card for, so that the rule
/// reads "from this person", never "from everyone there".
fn sender_condition(domain: &str, address: Option<String>, shared: &[String], has_card: bool) -> super::store::Condition {
    let domain = domain.to_ascii_lowercase();
    let common = shared.iter().any(|p| super::message::domain_matches(&domain, p));
    match address.filter(|_| common || has_card) {
        Some(address) => super::store::Condition { field: "sender".into(), test: "is".into(), value: vec![address], until: String::new() },
        None => super::store::Condition { field: "sender-domain".into(), test: "is".into(), value: vec![domain], until: String::new() },
    }
}

// ---------------------------------------------------------------- the editor's choices

/// One thing to choose, by its id and its name.
#[derive(Debug, Clone, Serialize)]
pub struct Pick {
    pub id: String,
    pub name: String,
}

/// A task list or a calendar to choose.
#[derive(Debug, Clone, Serialize)]
pub struct ListChoice {
    /// "account/id".
    pub id: String,
    pub name: String,
    pub read_only: bool,
}

/// What the editor's values are chosen among, on this device.
#[derive(Debug, Clone, Serialize)]
pub struct Choices {
    pub projects: Vec<Pick>,
    pub topics: Vec<Pick>,
    pub accounts: Vec<Pick>,
    /// Task lists, those that can be written first.
    pub lists: Vec<ListChoice>,
    /// The general models, then each project's (`"<project>/<model>"`).
    pub models: Vec<super::store::ModelRef>,
    pub levels: Vec<ChoiceOut>,
    pub circles: Vec<ChoiceOut>,
    pub kinds: Vec<ChoiceOut>,
    pub attachment_types: Vec<ChoiceOut>,
    /// Which attachments `keep-attachments` keeps: "pdf" first, "any", the kinds.
    pub file_types: Vec<Pick>,
    /// The papers' kinds (`papers::Kind`), named as the Papers page names them.
    pub paper_kinds: Vec<Pick>,
    /// Whom a paper is for: you first ("me"), then the persons your admin is done for.
    pub persons: Vec<Pick>,
}

/// The choices of the editor's values, as this device has them.
pub fn choices(config: &crate::config::Config, store: Option<&crate::projects::ProjectStore>, rules: &super::store::Rules, tr: &Translator) -> Choices {
    let named = |field: &str, list: &[Choice]| -> Vec<ChoiceOut> { list.iter().map(|c| ChoiceOut { id: c.id, name: choice_name(tr, field, c.id), available: available(c.phase), phase: c.phase }).collect() };
    let mut lists: Vec<ListChoice> = crate::tasks::lists().into_iter().map(|c| ListChoice { id: format!("{}/{}", c.account, c.id), name: c.name, read_only: c.read_only }).collect();
    lists.sort_by_key(|l| l.read_only);
    Choices {
        projects: store.map(|s| s.projects.iter().map(|p| Pick { id: p.id.clone(), name: p.title.clone() }).collect()).unwrap_or_default(),
        topics: config.topics.iter().map(|t| Pick { id: t.id.clone(), name: t.name.clone() }).collect(),
        accounts: config.accounts.iter().map(|a| Pick { id: a.id.clone(), name: a.address.clone().unwrap_or_else(|| a.id.clone()) }).collect(),
        lists,
        models: super::store::models(rules, store),
        levels: LEVELS.iter().map(|c| ChoiceOut { id: c.id, name: tr.text(&format!("rules-level-{}", c.id), None), available: true, phase: c.phase }).collect(),
        circles: named("circle", CIRCLES),
        kinds: named("kind", KINDS),
        attachment_types: named("attachment-type", ATTACHMENT_KINDS),
        file_types: FILE_TYPES.iter().map(|t| Pick { id: (*t).to_string(), name: if *t == "any" { tr.text("rules-file-type-any", None) } else { choice_name(tr, "attachment-type", t) } }).collect(),
        paper_kinds: crate::papers::Kind::ALL.iter().map(|k| Pick { id: k.id().to_string(), name: tr.text(&format!("paper-kind-{}", k.id()), None) }).collect(),
        persons: std::iter::once(Pick { id: crate::persons::ME.to_string(), name: tr.text("rules-for-me", None) })
            .chain(crate::persons::Persons::load(&crate::persons::Persons::default_path()).persons.into_iter().map(|p| Pick { id: p.id, name: p.name }))
            .collect(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Phase 1 offers what the design says: a message arrives; tie it to a
    /// project, give it a topic, keep its attachments in Papers, make a task
    /// from a model, tell. The rest is
    /// declared as coming; nothing that sends, deletes, pays, or changes a
    /// setting or health is ever offered.
    #[test]
    fn phase_one_offers_five_actions_on_one_event() {
        let events: Vec<(&str, &str)> = TYPES.iter().flat_map(|t| t.events.iter().filter(|e| available(e.phase)).map(move |e| (t.id, e.id))).collect();
        assert_eq!(events, [("message", "arrives")]);
        let actions: Vec<(&str, &str)> = TYPES.iter().flat_map(|t| t.actions.iter().filter(|a| available(a.phase) && forbidden(a.writes).is_none()).map(move |a| (t.id, a.id))).collect();
        assert_eq!(actions, [("message", "tie-to-project"), ("message", "give-topic"), ("message", "keep-attachments"), ("task", "make"), ("none", "tell")]);
        for writes in [Writes::Send, Writes::Trash, Writes::Pay, Writes::Setting, Writes::Health] {
            assert!(forbidden(writes).is_some(), "{writes:?}");
        }
        assert_eq!(of("message").and_then(|m| m.action("trash")).map(|a| forbidden(a.writes)), Some(Some("rules-refused-deletes")));
        // Each field's tests are ones a condition can read, its choices unique.
        for t in TYPES {
            for f in t.fields {
                assert!(!f.tests.is_empty(), "{}.{}", t.id, f.id);
                let mut ids: Vec<&str> = f.choices.iter().map(|c| c.id).collect();
                ids.dedup();
                assert_eq!(ids.len(), f.choices.len());
            }
        }
        assert_eq!(of("task").and_then(|t| t.action("make")).and_then(|a| a.param("title")).map(|p| p.from), Some(&["subject"][..]));
    }

    /// From an example, a friend at a webmail, or someone with a card, is
    /// named by their address; an office by its domain.
    #[test]
    fn an_example_names_a_person_by_address_and_an_office_by_domain() {
        let shared = vec!["gmail.com".to_string()];
        let friend = sender_condition("gmail.com", Some("ann@gmail.com".into()), &shared, false);
        assert_eq!((friend.field.as_str(), friend.value.join(",")), ("sender", "ann@gmail.com".to_string()));
        let known = sender_condition("smallshop.example", Some("bob@smallshop.example".into()), &shared, true);
        assert_eq!(known.field, "sender");
        let office = sender_condition("Impots.Example", Some("no-reply@impots.example".into()), &shared, false);
        assert_eq!((office.field.as_str(), office.value.join(",")), ("sender-domain", "impots.example".to_string()));
    }

    /// Every setting a preset names, its own switch or a parameter's, is one
    /// Sioul declares on some page (`settings::for_view`): a preset's link to
    /// where it is set never points at nothing (the view's review, must-fix 8).
    #[test]
    fn every_preset_s_setting_is_declared() {
        let config: crate::config::Config = toml::from_str("case_store = \"~/Notes\"\n[[account]]\nid = \"a\"\nkind = \"imap\"\naddress = \"jane@example.org\"\n").unwrap();
        let tr = crate::i18n::Translator::new("en");
        let pages = ["accounts", "agenda", "bitwarden", "contacts", "general", "letters", "mail", "money", "notes", "papers", "parameters", "porch", "reading", "senders", "sites", "supplies", "tasks", "texts"];
        let declared: std::collections::BTreeSet<String> = pages.iter().flat_map(|page| crate::settings::for_view(page, &config, &tr, &[("acct/plan".into(), "Plan".into())], None)).map(|s| s.key).collect();
        let named: Vec<(&str, &str)> = presets().iter().flat_map(|p| p.setting.iter().copied().chain(p.params.iter().map(|q| q.key)).map(move |key| (p.id, key)).collect::<Vec<_>>()).collect();
        assert!(!named.is_empty());
        for (preset, key) in named {
            assert!(declared.contains(key), "{preset}: {key} is declared on no page");
        }
    }

    /// Every name the catalogue gives exists in English and French.
    #[test]
    fn every_declaration_is_named_in_both_languages() {
        for language in ["en", "fr"] {
            let tr = Translator::new(language);
            let catalogue = serde_json::to_string(&catalogue(&tr)).unwrap();
            assert!(!catalogue.contains("rules-"), "{language}: a name missing in {catalogue}");
        }
    }
}
