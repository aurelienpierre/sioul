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

- **Once, on every device.** What a rule makes has an identifier worked out from the rule's id and the triggering object's address: a task's UID (`task_uid`, a name-based UUID as the routes' tasks have), a paper's id (`paper_id`, with the file's digest too), a paper's file name (`kept_name`, its digest's first eight characters after its stem). Its bytes are stamped from the message's arrival, never from a device's clock (a route's task too), so that two devices that both act before either read the other write the same bytes: their servers see one task, and each device's Undo knows it. A file already there is found, not written again. Every act is written in a shared record (`wiring::acts`): each device's own log, `$XDG_STATE_HOME/sioul/rules/acts/<device>.jsonl`, a JSON line per act (rule, object, act, result, whether it wrote, its depth in a chain, a task's list and digest), never a word of the mail. Every device reads every log before it acts: a rule never acts twice on one object.
- **Undone everywhere.** An act undone (`engine::undo`) writes a line `undone`; so does "not for this one" (`not_for_this_one`), and a file the antivirus refused: the rule never acts on that object again, on any device. Undo's line also says `back` and names the result it took back (one paper of a message's two, not both): it is a state, not a one-off. At each pass, and when the record arrives from another device (the window on `changes::Kind::Rules`, the phone at each step, even once no rule is left), each device that wrote that same result takes its own copy back (`Engine::take_back`: its task, its tie, its topic's word, its paper, while each is as the rule left it), once, and writes a line `taken`: what you give again by hand afterwards stays. "Not for this one" takes nothing back: what was made stays where it is, on every device; so does what changed since an Undo found too late. A tie taken apart by hand or a task deleted stays recorded as done, so it is never made again either.
- **Refusals wait.** A rule that matched but could not act for a reason that may pass (a write that failed, no task list, a model or a project gone, the notices kept by another device, a phone's moment that does not let a notice come) leaves a pending line; each pass tries it again for a week (`Engine::settle`, `acts::PENDING_MS`), finding its message again here. Each refusal is said once per rule; final ones (your own word on a topic, a code's own notice, a file refused, a chain at its end) are said and not tried again.
- **Never on what was there before.** At an address's first fetch nothing runs (`on_arrival` with `first`); a rule acts on what reached the provider from its `since` on (the message's file time, else its Date), and never on mail more than a week old when it is fetched (`engine::NEWEST`). "Also the last 30 days" (`for_already_here`) is offered with the list of what it would touch (`preview`), and acts once on each, never where a rule doing the same acted already (a rule removed and made again).
- **Never past the Porch's guards.** Only mail the Porch lets through from a sender something proves (`engine::held`): never what it sets aside (forged, a blocked sender, a borrowed name, your provider's spam no route protects), hostile mail, what it holds (a stranger in the screener, a stranger to a public address), what nothing proves comes from its sender, what your own spam filter caught, nor what lies in a junk or a trash folder (by the role the account's folders saved at its last sync give it, the server's mark first, whatever the provider calls the folder; by its name only when none is saved); never your own mail. When you let a sender in, the rules run on what they sent this week (the window's "Let in"). No rule acts on mail "from anyone" in this version.
- **Nothing kept from agents leaks through what a rule makes.** A task or a paper is in its message's project unless the rule names another, and agents' consent follows what was made of a message (`how = "made"`: in the message's projects, kept with its account); a message holding a code gives nothing of its subject to a title, a paper's name or a notice, and the numbers agents never see (IBANs, card numbers, social security numbers: `masking`) are hidden there too.
- **Never what a rule may not do** (design, part 5): an action that sends, deletes, moves money, changes a setting or another rule, or touches health is refused by the engine whatever a rule says (`declare::forbidden`), as is a read-only list; a meeting with other guests is refused by `agenda::guard` beneath every event's write.
- **No loops.** A rule never acts on what it made itself, and a chain of rules stops at three steps (`declare::CHAIN`): an act records its depth, one more than the deepest act that made its object.
- **Said once, with Undo.** Each act comes back as a `Done`: its line, made by the one function that says every rule's sentence (`wiring::words`), and its record with the files it wrote (`agentwrites::Said`). The window says it in the status line with Undo, which calls `engine::undo`: a task taken out while it is as made, a tie taken apart, a topic's word taken back while it is still the rule's, a paper and its file taken out while the file is as kept and the paper not filled in since (its dates, a note, a renewal planned), the file left when another paper names it; "not for this one" is written either way.

## The five actions of phase 1

When a message arrives:

- **Tie it to a project** (a project, or "its project"): the tie routes write, `how = "project"` in `links.toml`; found when it is there.
- **Give it a topic**: a word on the message in this device's topics' log (`topics::labels`), never over a word of yours on it (refused then). Its Undo writes `forget`.
- **Keep its attachments in Papers**: which attachments (PDF by default, any, or one kind; whose name holds some words), the paper's kind (else guessed from the file's name, then the subject, as "Keep in papers" guesses it), its name as a pattern with inserts (`{sender}` the sender's card's name, else the name it gives, else its address; `{month}` the month and year it came; `{year}`; `{subject}`; `{file}`), whom it is for. A file is kept only when its bytes are what its kind says (`content_agrees`: a "PDF" that is a page is not), no larger than 25 MB, never a program, never a page, an SVG or a document with macros unless the rule's "name" names it. Every file is read by the antivirus before any is kept (`engine::Scanned`, sioul-sync's `antivirus::for_rules`): one refused keeps none of the message's files, says "not for this one" for good, and takes out a paper of those bytes kept unchecked elsewhere. Where no antivirus can be asked (a phone, a computer without one: `Engine::can_scan`; or none answering), the file is kept marked `unchecked` on its paper ("Not checked by an antivirus"); a device with an antivirus reads every such paper it meets (`Engine::scan_unchecked`, once a run, each known by its file's size and time; a phone never looks): clean, the mark goes; refused, the paper and its file are taken out on every device, said once with its name. Each file is kept once, under a name carrying its digest (`kept_name`: two files of one name are two papers, a name from mail made safe): by its paper id, or by its content when a paper of the same file exists already (kept by this rule elsewhere, by another rule, by hand), which then gets the tie instead, and is read by an antivirus if it never was. The paper is tied to its message (`how = "made"`), to the movement the money reader joined that message to, when it did, and to the person it is for. A copy that fails (a full disk) keeps nothing of the message: what the pass kept goes out again, and the act waits whole. The message itself is never changed.
- **Make a task from a model**: a general model (`[[model]]` of the rules' list) or a project's own (`<project>/<model>`); in the model's list, else the usual one; in the project the rule names, else the message's, else the model's; its title the subject with the rule's subject words and their brackets taken off and the model's prefix in front (or a fixed title; the model's name alone for a message holding a code); asked so many working days after the message came; its estimate and supplies the model's; `X-SIOUL-ORIGIN` and `X-SIOUL-RULE` on it; tied to its message (`how = "made"`). Nothing of the mail's text is copied.
- **Tell**: a notice to know about the message ("now", as the moment allows: never in a pause or a do-not-disturb, waiting while nobody is at the device; on a phone too), posted by the device that keeps the notices: another device leaves a pending line, told where the lease moves within the week; a phone whose moment makes the notice wait (`Engine::moment`, `notice::decide`) records nothing as told and leaves a pending line too, told at a later step; or quietly, said in the status line and kept in the rule's history only. A code is never said in it, and a message holding one is told by its own notice, never again by a rule. Nothing is told for mail already here.

## Where rules run

`wiring::arrival::arrived` is the one function the window's arrival chain, the phone's background step and `sioul watch` call with what a fetch brought: the window, the inbox's arrivals and those of its other folders (`report.new`, `report.elsewhere`); the phone's background step and `sioul watch`, the inbox's alone (`report.new`). It makes the ties to projects the Porch's lanes give, the tasks the projects' routes make (`routetasks`), and runs the rules, after `Engine::settle` (what was undone elsewhere taken back, what waits tried again, the papers kept unchecked read). The phone's background step runs it after its exchange, so that what another device did or undid is known first, and settles at every step even without new mail. Until phase 1 the phone's background step tied nothing to projects and made no task; it now does what a computer does. Each writes only in Sioul's own folders, under each file's lock, which every process of the device shares: `links.toml`, the task lists' files (pushed to their server at the window's next sync of that account, which the background process does not start), the topics' log, the papers' list and folder, the record of acts. The window says what was made with Undo and tells the accounts whose lists got a task; the phone's background step posts a rule's notice through Notices.java and says nothing else (the rules' history holds each act with its Undo); `sioul watch` prints each act and tells nothing.

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

- **What a device does not know is kept byte for byte and never run**: a rule that names a type, an event, a field, a test, a value of a closed list, an action, a parameter or a key this Sioul does not know (a newer one wrote it), or that does not read at all. It may be turned on or off (its `on` alone written into its table) or removed, never edited here. Every other rule, model and line of the file stays as it was when one changes: only the changed entry's table is written anew (mail filters keep unknown filters the same way; a project's routes did not, and lost what an older Sioul did not know). Between devices, the sharing merges the file entry by entry (`share::RULES`, keyed by id) and writes it in its own layout: every key of every rule travels, a newer Sioul's included, but comments and the keys' order do not (test: sioul-sync's `your_rules_wait_for_a_device_that_has_them`).
- **A newer format** (`version` above `FORMAT`): the list is shown, and nothing in it runs nor is written.
- **The sharing**: the list (`share::RULES`, `data/rules.toml`, each `[[rule]]` and `[[model]]` by its id) and the record of acts (`share::RULES_ACTS`, `state/rules/acts/`, each device's log) travel sealed with the settings once every other device says in its entry that it has them (`devices::Entry::rules`, `share::rules_shared`), then stay in. Until then each device keeps its rules and runs them alone.
- **A device on 0.1.0** has no rules: it neither receives the list nor reads it, runs nothing of it, and holds no record of it that would wait at every exchange. It keeps running its projects' routes, route tasks, filters and topics exactly as before; the ties and tasks a newer device's rules make reach it as any tie or task does (`links.toml`, CalDAV), and it shows them. A rule made on a newer device runs on the newer devices only. The papers travel to it too, and it knows no mark "not checked by an antivirus": when it rewrites the wallet (any paper changed there), every paper's mark is gone from the file, so for a while a paper a rule kept unchecked reads as any other, on every device. The newer devices put the mark back from the record of acts at their next settle (`Engine::restore_marks`: the act that kept the paper says `unchecked`), while the paper's file is the very one the rule kept and no antivirus has read it clean since (a line `mark-off`, also written once its file is replaced); an antivirus then reads it as before. The 0.1.0 device itself never shows the mark (test: `a_mark_an_older_sioul_dropped_is_put_back`).

## What exists, read as rules

`wiring::readers` shows what exists today in the same shape, read-only, each with where it is edited today, and the engine never runs them: each project's routes ("tie it to this project"), the routes that make tasks ("make a task from its model"), the budgets' mail rules ("plan a movement in this budget", phase 3), mail's filters (one rule per action, their accounts a condition, `stop` said), and the topics' sender rules (Always, Never). Nothing is migrated in phase 1: they are folded into the list in one migration once every device knows it (phase 4).

## Presets

What Sioul does by itself (each automatic act of its modules) is shown as presets (`declare::presets`): rules in the same shape, each with its stable id, the setting that turns it on or off today, its parameters that are settings today, and the phase in which its mechanism moves into the engine; each keeps running where and how it runs today. A change to a preset writes the setting it has today. `declare::from_preset` gives a draft rule from a preset the engine runs now, remembering where it came from; never from one whose module still does what it says (the codes' own notice), which would tell twice.

## What phase 1 covers

The declarations of every type (only "a message arrives" and five actions available), the rules' list, the record of acts, the engine with its guards, the readers, the presets, and the one arrival function in the window, the phone's background step and `sioul watch`. Settings ▸ Rules, the editor and "Make a rule from this…" are the window's. Tasks and events as triggers come in phase 2; money, papers, contacts, supplies and the clock in phase 3; mail's filters and the old formats folded into the list in phase 4.

Tests: `crates/sioul-core/src/wiring/` (`store_tests.rs`, `engine_tests.rs`, `readers_tests.rs`, and each module's own).
