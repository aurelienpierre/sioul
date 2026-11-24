# Rules: when this happens, do that

A rule is one sentence: **when** something happens to an object of a type, and its fields say so, **then** do one action to an object, with parameters that are fixed or taken from the first object. The rules are one list, which grows and shrinks; what Sioul already does by itself is shown in the same shape, as presets. This note is for developers: the model, the declarations, the engine's guarantees, the store and older devices, and what phase 1 covers. The code is `crates/sioul-core/src/wiring/`.

## The model

Each type of object declares three lists, as data in code (`wiring::declare`):

- **What can happen to it** (its events): a message arrives, is sent; a task is made, is done, is put off, comes due; an event is made, moves, starts soon, is over; and so on for money movements, supplies, calls, papers, letters, contracts, invoices, sites and the clock.
- **What can be read on it** (its fields, for conditions), each with its tests and the kind of value they take (`Value`: words, addresses, domains, a choice, days, a time, a size, a topic, a project…), and whether the condition holds only for a sender something proves.
- **What can be done to it** (its actions), each with its typed parameters (fixed, or taken from the triggering object: "its subject", "its project"), what it writes (`Writes`), whether it can be undone, how it is made once, and what it refuses.

What a later phase brings is declared already with its phase: the editor shows it as coming, never offers it, and the engine never runs it. `declare::catalogue(tr)` gives it all to the window in the person's words; the editor is built from it, never written by hand for a pair of types.

A rule (`wiring::store::Rule`): its id; on or off; `since`, the moment it acts from; one `when` (a type, an event, conditions, all of them or one); one `then` (a type, an action, its parameters); the preset it was made from, if any. One action per rule: rules that share a "when" are shown together, and stored one by one.

## A message's fields

The union of the four ways Sioul said "which mail" before the rules (a project's route, a budget's mail rule, a mail filter, the senders' lists), each read as the place it comes from reads it (`wiring::message`):

- **Its sender**: its address, its domain (which covers its subdomains), its contact card, who it is to you (known, safe, neutral, restricted, a stranger), From and Reply-To. These hold only for a sender something proves (`rules::facts_of`, the Porch's `Admission::Unproven`): a sender nothing proves is nobody, a stranger.
- **What it says**: whole words of its subject and its text (case and accents aside, as a route reads them), or a part of them (as a filter reads them); To and Cc; the address it came to; its attachments' names and kinds; its list (List-Id); its size; the day, weekday and time it came.
- **What Sioul knows of it**: its topic; its projects (the Porch's lane, every route that takes it, its ties and its conversation); and **what Sioul recognises in it**, an open list fed by the recognisers that exist: a code, a temporary password, a reset, a sign-in link, an address to confirm (`codes`); an invitation (a calendar attached); a newsletter; money received, a payment made (a receipt), an order, a bill, a refund (`payments::detect`, the money reader's own). A payment refused, a sign-in notice, something waiting on a site, a parcel, a bounce, a renewal and a reply come in phase 2.

## How rules run: made to converge, not to fire

A rule does not fire on a signal that one device may miss. For each object handed to it that matches its "when", its result must exist (`wiring::engine`):

- **Once, on every device.** What a rule makes has an identifier worked out from the rule's id and the triggering object's address: a task's UID (`task_uid`, a name-based UUID as the routes' tasks have), a paper's id (`paper_id`, with the file's digest too). Two devices that both act before either read the other write the same thing; a file already there is found, not written again. Every act is written in a shared record (`wiring::acts`): each device's own log, `$XDG_STATE_HOME/sioul/rules/acts/<device>.jsonl`, a JSON line per act (rule, object, act, result, whether it wrote, its depth in a chain, a task's list and digest), never a word of the mail. Every device reads every log before it acts: a rule never acts twice on one object.
- **Not for this one.** An act undone (`engine::undo`) writes a line `undone`; so does "not for this one" (`not_for_this_one`), and a file the antivirus refused. The rule never acts on that object again, on any device. A tie taken apart by hand or a task deleted stays recorded as done, so it is never made again either.
- **Never on what was there before.** At an address's first fetch nothing runs (`on_arrival` with `first`); a rule acts on what reached the provider from its `since` on (the message's file time, else its Date). "Also the last 30 days" (`for_already_here`) is offered with the list of what it would touch (`preview`), and acts once on each.
- **Never past the Porch's guards.** Only mail the Porch admits: never what it sets aside (forged, a blocked sender, a borrowed name, your provider's spam no route protects), hostile mail, nor what your own spam filter caught (`set_aside`); never your own mail.
- **Never what a rule may not do** (design, part 5): an action that sends, deletes, moves money, changes a setting or another rule, or touches health is refused by the engine whatever a rule says (`declare::forbidden`), as is a read-only list; a meeting with other guests is refused by `agenda::guard` beneath every event's write.
- **No loops.** A rule never acts on what it made itself, and a chain of rules stops at three steps (`declare::CHAIN`): an act records its depth, one more than the deepest act that made its object.
- **Said once, with Undo.** Each act comes back as a `Done`: its line, made by the one function that says every rule's sentence (`wiring::words`), and its record with the files it wrote (`agentwrites::Said`). The window says it in the status line with Undo, which calls `engine::undo`: a task taken out while it is as made, a tie taken apart, a topic's word taken back while it is still the rule's, a paper and its file taken out while the file is as kept; "not for this one" is written either way.

## The five actions of phase 1

When a message arrives:

- **Tie it to a project** (a project, or "its project"): the tie routes write, `how = "project"` in `links.toml`; found when it is there.
- **Give it a topic**: a word on the message in this device's topics' log (`topics::labels`), never over a word of yours on it (refused then). Its Undo writes `forget`.
- **Keep its attachments in Papers**: which attachments (PDF by default, any, or one kind; whose name holds some words), the paper's kind (else guessed from the file's name, then the subject, as "Keep in papers" guesses it), its name as a pattern with inserts (`{sender}` the sender's card's name, else the name it gives, else its address; `{month}` the month and year it came; `{year}`; `{subject}`; `{file}`), whom it is for. Never a program. Every file is checked by the antivirus before any is kept (`engine::Scanned`, sioul-sync's `antivirus::for_rules`): one refused keeps none of the message's files, and says "not for this one" for good; on a computer where no antivirus answers (none there, the file too large), nothing is kept, since a rule cannot ask as "Keep in papers" asks; on a phone, where no antivirus can be asked, the file is kept unchecked, as "Keep in papers" keeps it there. Each file is kept once: by its paper id, or by its content when a paper of the same file exists already (kept by this rule elsewhere, by another rule, by hand), which then gets the tie instead. The paper is tied to its message (`how = "made"`), to the movement the money reader joined that message to, when it did, and to the person it is for. The message itself is never changed.
- **Make a task from a model**: a general model (`[[model]]` of the rules' list) or a project's own (`<project>/<model>`); in the model's list, else the usual one; its title the subject with the rule's subject words and their brackets taken off and the model's prefix in front (or a fixed title); asked so many working days after the message came; its estimate and supplies the model's; `X-SIOUL-ORIGIN` and `X-SIOUL-RULE` on it; tied to its message (`how = "made"`). Nothing of the mail's text is copied.
- **Tell**: a notice to know about the message ("now", as the moment allows: never in a pause or a do-not-disturb, waiting while nobody is at the device), posted by the device that keeps the notices; or quietly, said and kept in the rule's history only. A code is never said in it. Nothing is told for mail already here.

## Where rules run

`wiring::arrival::arrived` is the one function the window's arrival chain, the phone's background step and `sioul watch` call with what a fetch brought (`report.new` and the arrivals in other folders): the ties to projects the Porch's lanes give, the tasks the projects' routes make (`routetasks`), and the rules. Until phase 1 the phone's background step tied nothing to projects and made no task; it now does what a computer does. Each writes only in Sioul's own folders, under each file's lock, which every process of the device shares: `links.toml`, the task lists' files (pushed to their server at the window's next sync of that account, which the background process does not start), the topics' log, the papers' list and folder, the record of acts. The window says what was made with Undo and tells the accounts whose lists got a task; the phone's background step posts a rule's notice through Notices.java and says nothing else (the rules' history holds each act with its Undo); `sioul watch` prints each act and tells nothing.

## The store, and older devices

`$XDG_DATA_HOME/sioul/rules.toml` (`wiring::store`), read, changed and written under its lock:

```toml
# Your rules: when this happens, do that (docs/rules.md).

version = 1

[[rule]]
id = "0b6c2a64-…"
on = true
since = 1795435200
when.type = "message"
when.event = "arrives"
when.if = [
    { field = "sender-domain", test = "is", value = ["impots.example"] },
    { field = "kind", test = "is", value = ["bill"] },
]
then.type = "message"
then.action = "keep-attachments"
then.with = { title = "{sender} bill, {month}", type = "pdf" }

[[model]]
id = "pay-a-bill"
name = "Pay a bill"
prefix = "Pay:"
business_days = 3
```

- **What a device does not know is kept byte for byte and never run**: a rule that names a type, an event, a field, a test, a value of a closed list, an action, a parameter or a key this Sioul does not know (a newer one wrote it), or that does not read at all. It may be turned on or off (its `on` alone written into its table) or removed, never edited here. Every other rule, model and line of the file stays as it was when one changes: only the changed entry's table is written anew (mail filters keep unknown filters the same way; a project's routes did not, and lost what an older Sioul did not know).
- **A newer format** (`version` above `FORMAT`): the list is shown, and nothing in it runs nor is written.
- **The sharing**: the list (`share::RULES`, `data/rules.toml`, each `[[rule]]` and `[[model]]` by its id) and the record of acts (`share::RULES_ACTS`, `state/rules/acts/`, each device's log) travel sealed with the settings once every other device says in its entry that it has them (`devices::Entry::rules`, `share::rules_shared`), then stay in. Until then each device keeps its rules and runs them alone.
- **A device on 0.1.0** has no rules: it neither receives the list nor reads it, runs nothing of it, and holds no record of it that would wait at every exchange. It keeps running its projects' routes, route tasks, filters and topics exactly as before; the ties and tasks a newer device's rules make reach it as any tie or task does (`links.toml`, CalDAV), and it shows them. A rule made on a newer device runs on the newer devices only.

## What exists, read as rules

`wiring::readers` shows what exists today in the same shape, read-only, each with where it is edited today, and the engine never runs them: each project's routes ("tie it to this project"), the routes that make tasks ("make a task from its model"), the budgets' mail rules ("plan a movement in this budget", phase 3), mail's filters (one rule per action, their accounts a condition, `stop` said), and the topics' sender rules (Always, Never). Nothing is migrated in phase 1: they are folded into the list in one migration once every device knows it (phase 4).

## Presets

What Sioul does by itself (the inventory's 68 automatic acts) is shown as presets (`declare::presets`): rules in the same shape, each with its stable id, the setting that turns it on or off today, its parameters that are settings today, and the phase in which its mechanism moves into the engine; each keeps running where and how it runs today. A change to a preset writes the setting it has today. `declare::from_preset` gives a draft rule from a preset the engine runs now, remembering where it came from.

## The view

Settings ▸ Rules, after What reaches you (`ParametersPage.qml`'s section `rules`; `RulesTab.qml`, made when the tab is first shown). The window's side is `crates/sioul-app/src/wiring.rs`, its invokables in `backend.rs` (`rulesView`, `rulesCatalogue`, `rulesChoices`, `ruleSentence`, `rulePreview`, `saveRule`, `setRuleOn`, `removeRule`, `undoRuleAct`, `ruleNotForThisOne`, `ruleFromMessage`, `ruleFromPreset`, `rulesStarts`, `saveRuleModel`) and two signals, `rulesPreviewed` (a preview found off the window's thread) and `rulesChanged` (`changes::Kind::Rules`: the list or the record of acts written, here or by the sharing).

**One sentence function.** Every sentence the view shows is made by `wiring::words`: `sentence` (the list, the editor's summary, the status line on saving, turning on or off, removing), its halves `when_part` and `then_part` (a "when" shared by several rules is said once, `when_shared`, each rule under it by what it does), `when_heading` (the group), `act_line` (what a rule did, in its history and in the status line). A sentence is built from whole phrases, `rule-*` in both `.ftl` files: one per type and event (`rule-heading-message-arrives`), one per type and field with its tests as a Fluent selector (`rule-if-message-subject`), one per recognised kind said as the verb (`rule-says-bill`: "When a message from … brings a bill"), one per type and action, with `$same` choosing "it" (the object that triggered the rule) or "its message" (another object) (`rule-do-message-tie-to-project`), and optional parts for parameters (`rule-do-task-make-days`). Values are inserted whole: typed words quoted ("“avis”", « avis »), ids named (`Names`: projects, topics, models, accounts, lists, contacts, budgets, messages' subjects), days, dates and circles in words. What has no phrase (a type, a field or an action a newer Sioul declared) falls back on the declarations' names (`rule-if-any`, `rule-do-any`): a sentence is always made, never an id. French keeps its agreements because each phrase belongs to one type: "le rattacher", "la marquer comme faite", "une tâche … demandée". `words::tests` checks each kind of rule in English and French.

**The list.** `wiring::view` reads the rules' list (`store::Rules`), the record of acts (`acts::Acts`, five acts per rule, the messages named from the index the window already holds, never read on the window's thread), and what exists elsewhere through `readers::read_all` (projects' routes and the tasks they make, budgets' mail rules, mail filters, topics' senders). Lines are grouped by their heading, in the order of the declarations, and inside a group by their "when" said in words: rules that say the same "when" are shown together. A rule of yours has its switch (`set_on`: turned on, it acts from then), Edit, ✕ (removed at once, Undo in the status line through `mail::offer_back`, which puts it back with `Rules::put`), and "What it did lately" in a `Fold` (made when first opened), each act with Undo (`engine::undo`: put back while it is as the rule left it) and Not for this one (`engine::not_for_this_one`). A rule kept elsewhere has no switch and no Edit: it says where it is kept (`rules-kept-*`) and a `Link` opens its editor there through `reminderOpened` (a project's page; `mail-settings:mail.filter` and `mail-settings:topics.list`, Mail ⚙ at that setting, main.qml's `openMailSetting`; Money for a budget's mail rules, which have no editor yet). Nothing of them is copied.

**The editor** (`RuleEditor.qml`, a `SafeDialog` made on first use, its foot `DialogFoot`) is built from `declare::catalogue` alone: a type and an event (`RuleCombo.qml`, what is not available greyed with "(later)" and never chosen), conditions (a field, a test, a value whose control `RuleValue.qml` chooses by the test's `value`: one field for words, addresses, domains, text, a size or a time, several separated by commas; a date; days to tick; a number; a drop-down for a choice or for the person's own things, from `rulesChoices`, `declare::choices` by value kind), every condition or one of them; one action across all types; its parameters, each "Chosen here" or "From what triggered it" (the declared `from` fields); a model of task of your own named in place when none fits (a `Fold` under the model's drop-down: its name, prefix, list and days asked, `saveRuleModel`, `Rules::put_model`). Each change asks `ruleSentence` (the sentence and `Rule::problem` in words), and half a second later `rulePreview`: `engine::preview` over `engine::recent_files` of the last 30 days, on a thread of its own, answered with the rule as it was asked so that an older answer is left aside. "Also for these messages, once each" is unticked; Save calls `saveRule`, which puts the rule through `Rules::change`/`put`, says it in the status line, and, when ticked, runs `engine::for_already_here` on a thread and says how many it did.

**Starts.** "Add a rule…" (`RuleStart.qml`): from nothing; from a message of the last 30 days the window knows (none of your own addresses), found by words of its subject or sender, then `declare::example` (its sender's domain only when proven, its recognised kind, its project; none for mail the Porch set aside); from a preset the engine runs now (`declare::from_preset`); while none is, one quiet line says that starting from a preset comes later, where they would be listed. A message's menu (`Reader.qml`'s ⋮, the Porch's row menu `NoticeRow`'s `more`) has "Make a rule from this…" (main.qml's `ruleFromMail`), never on mail set aside or held for review; the older "Make a task of mail like this…", "Make it a filter…" and "Always this topic for this sender" stay as they are in phase 1.

**Presets.** The second tab (`Tabs.qml`, shown when the core declares presets) groups `declare::presets` as the first groups rules, each a line of its sentence; its switch where a setting turns it on or off today (the setting's own row, `settings::for_view`, saved with `setSetting`: one truth); "Details" (`PresetCard.qml`) where it has more settings or can be copied: its settings as `SettingRow` draws them everywhere, a line when its details cannot be changed yet, and "Copy to make my own" for a preset the engine runs now (none in this version: the button is not drawn). Its on/off switch stays on its line, never repeated in the card.

**Tests and pictures.** `tst_rules.qml` (the list grouped, a rule switched off and removed, what it did lately, a rule kept elsewhere, the presets' lines, the editor following a declaration added in the test, a parameter from the message, the preview, Save, the three starts, nothing past a phone's width); `tst_settingstabs.qml` knows the new section. `tools/demo/run.sh --release OUT en rules [phone]` takes the list, what a rule did lately (`make-demo.py` writes one act of the rule `demo-taxes`, which the step list saves), the editor at each step, a rule from a message, "Add a rule…", the presets and a preset's card.

## What phase 1 covers

The declarations of every type (only "a message arrives" and five actions available), the rules' list, the record of acts, the engine with its guards, the readers, the presets, and the one arrival function in the window, the phone's background step and `sioul watch`. Settings ▸ Rules, the editor and "Make a rule from this…" are the window's. Tasks and events as triggers come in phase 2; money, papers, contacts, supplies and the clock in phase 3; mail's filters and the old formats folded into the list in phase 4.

Tests: `crates/sioul-core/src/wiring/` (`store_tests.rs`, `engine_tests.rs`, `readers_tests.rs`, and each module's own).
