# Reminders before dates

Remembering "on the 30th" is what fails most: time-based remembering is impaired in autism and in adult ADHD, much more than remembering on a cue (Landsiedel, Williams & Abbot-Smith 2017; Altgassen, Kretschmer & Kliegel 2014), and reminders work where memory is the bottleneck (Jamieson et al. 2014, d = 1.27). So Sioul carries the dates, and says each one once, quietly.

Code: `crates/sioul-core/src/reminders.rs` (what to remind, when), `crates/sioul-core/src/notify.rs` (what each kind of notification does at each time, below), `crates/sioul-app/src/remind.rs` (the window), `crates/sioul-app/src/eventalarms.rs` (a phone's alarms), `crates/sioul-cli/src/remind.rs` (`sioul remind`).

## What is reminded, when
| What | When | Waits while work rests? |
|---|---|---|
| An event, before it | 15 minutes before it, counted before its time to get ready and get there (Settings ▸ Reminders and notifications ▸ **Before an event**; each event its own): "14:00 · Dentist, in 15 minutes", the place below | no |
| An event, the working day before | half an hour before work ends, on the last working day before it: "Mon 12 Oct 09:00 · Dentist", the place below | no |
| An event's own alarms (VALARM, set by you, your phone or whoever invited you) | at their time | no |
| A date asked (a task's due date) | when work starts, 2 working days before (Settings ▸ Reminders) | unless the task is yours (quiet time keeps it) |
| A wait over (a step done, then "wait 14 days for the answer") | when the wait ends, at the next working hours | the same |
| A payment planned (a bill, a tax: a planned line of a budget) | when work starts, 2 working days before | no |

- **Working days** are those of your working hours, less days off; without working hours, Monday to Friday, 9:00 to 17:00.
- **Once**: each reminder is marked when told (`$XDG_STATE_HOME/sioul/reminded/`, a small file each, forgotten after two months). Never repeated, no count of what was missed, no red.
- **Not for what you just made**: a task made after its reminder's time, an event changed after it, is not reminded: you just saw it.
- **Late, but not too late**: a computer asleep at the time reminds on waking, while it still makes sense: before the event begins, before the day asked ends, within a week of a wait's end.
- **Nothing while you sleep**, as usual ([the notification matrix](#what-comes-when-the-notification-matrix)): from winding down to waking, and during a nap ([areas.md](areas.md), "Sleep"), no reminder is told, yours included: each waits for waking, and comes then if it still makes sense (`reminders::Wait::Sleep`). One exception: an event's own reminders (its alarms, Sioul's before it) when the event itself falls in that sleep. An event at 03:00 is a choice you made: it is reminded at 02:45. Doses are Health's, and come all the same unless you asked them to stay silent ([health.md](health.md), "Do not disturb").
- **The pauses** ([pauses.md](pauses.md)), as usual: in a **pause**, an event's own reminders come (the event falls in the pause as far as anyone knows: it has not begun), everything else waits for your return (`Wait::Paused`); in **Free time**, an event's own reminders come when the event falls in it (before the night's start or midnight, when Free time ends), the others wait for its end (`Wait::Free`).
- **Not reminded**: tasks done or cancelled; events cancelled, and those you declined (one of your addresses among the guests, "declined"); the day before and Sioul's reminder before for calendars you only read (holidays, subscriptions), their alarms still told; debits that leave by themselves (presets). A payment reminder that ignores the balance can push an account into overdraft (Medina 2021): the money watch says when the account will not hold a debit, which a reminder alone cannot.

## What comes when: the notification matrix
Who may reach you ([porch.md](porch.md)) says *from whom* mail, calls and messages come at each time; `sioul_core::notify` says *what* each kind of Sioul's notifications does at each time: a matrix the person sets in Settings ▸ Reminders and notifications ▸ **When each comes** (`crates/sioul-app/qml/NotifyGrid.qml`). The owner's words: "we need to complete the user-config matrix mapping events with timeframes, so far they map only contact lists with timeframes". Its usual values are what Sioul did before it, cell for cell (`notify::usual`).

**Columns.** The time, as `quiet::mode` decides it (`notify::Now::of`): the pause first, then sleep (from winding down, naps), Free time, a meal, then work, admin, leisure. No hours set count as work and admin together, and so do work and admin hours open at once: the least strict of the two cells, as who × when lends them. Two layers on top of the time: a slot of time for you (`hours::quiet_slot`, known where the window runs) and do-not-disturb from its switch or a focus session (`everywhere::Now::gates`; read from the files by `notify::dnd_from_files` where there is no window: `sioul remind --watch`, `sioul watch`). With a layer, the strictest cell wins. Do-not-disturb held for sleep or a pause is no layer: their own columns apply.

**Values** (`notify::Cell`, least strict first): `now` at once; `list-any` and `list`, during do-not-disturb, its list whatever who × when says or with it (people rows only); `event`, an event's own when the event begins within the time now (a sleep's block, Free time's end, the hours' end; never offered for the pause, whose end is unknown, nor a layer); `gathered`, at the gathered times; `later`, it waits for a time it may come in and comes then if it still makes sense; `never`, dropped (a code stays on the Porch, new mail is marked told unsaid).

| Kind (row) | Work | Admin | Leisure | Meals | Sleep | Pause | Free time | Time for you | Do not disturb |
|---|---|---|---|---|---|---|---|---|---|
| Codes and links asked for (`codes`) | now (fixed) | now (fixed) | now (fixed) | now (fixed) | never | never | now (fixed) | now (fixed) | now (fixed) |
| Doses (`doses`) | now (fixed) | now (fixed) | now (fixed) | now (fixed) | now¹ | now¹ | now (fixed) | now (fixed) | now (fixed) |
| The alarm at waking (`wake`) | now (fixed) | now (fixed) | now (fixed) | now (fixed) | now (fixed) | now (fixed) | now (fixed) | now (fixed) | now (fixed) |
| An event's alarms (`alarms`) | now | now | now | now | event | now (fixed) | event | now | now (fixed) |
| Reminders before an event (`before`) | now | now | now | now | event | now | event | now | now |
| Events, the working day before (`day-before`) | now | now | now | now | later | later | later | now | now |
| Dates, waits, payments, papers (`dates`) | now² | now² | now² | now² | later | later | later | now | now |
| Meals, naps and the night (`needs`) | now | now | now | now | never³ | never | never | now | now |
| The pause to move (`move`) | now | now | now | now | never | never | never | never | never |
| Work hours are over (`work-over`) | never | now | now | now | never | never | never | now | now |
| The time running (`time`) | now | now | now | now | now | now | now | now | now |
| Your watch's offers (`watch`) | now | never | never | never | never | never | never | now | now |
| New mail (`mail`) | now | now | now | now | later | later | later | later | list |
| Sites' notifications (`sites`) | gathered | gathered | gathered | gathered | later | later | later | later | later |
| A site in real time (`sites-live`) | now | now | now | now | later | later | later | later | later |
| A call in a site (`site-calls`) | now | now | now | now | later | later | later | now | later |
| Messages from people, other apps (`app-people`) | now | now | now | now | now | now | now | now | list |
| Automatons, other apps (`app-automatons`) | gathered | gathered | gathered | gathered | later | later | later | later | later |
| Apps set to At once (`app-at-once`) | now | now | now | now | later | later | later | later | later |

¹ As the older switches say until the row says otherwise: `[reminders] doses_in_sleep`, `[pause] doses` (both on, unless set off). ² Work's dates wait while work rests (`Wait::Work`): an area's rule, not the matrix's. ³ The night's or a nap's own notice as it begins comes whatever sleep's column says: it says that sleep begins; not in a pause, nor during a meeting.

**Fixed** (`notify::lock`, greyed in the grid, a press says why): doses at once but in sleep and a pause, never dropped (a dose comes at its time; only the two times of the older switches may hold it); codes at once but in sleep and a pause (asked a moment ago, valid minutes); an event's alarms during a pause and do-not-disturb (both promise that alarms still come, P7); the alarm at waking, always. The choices each cell may take: `notify::choices`.

**About people** (`Cell::admits`): new mail (`mailnote` in the window: `Seen::tells`) and other apps' messages on a phone (`appnotes::person`). Both matrices must let a notification through: `now`, the sender's row of who × when ticked now (mail also to an address for now, but from the safe); `list`, that and on do-not-disturb's list (as before the matrix); `list-any`, on the list whatever who × when says, as calls already were; `later` and `never`, nobody. "The people on my list get through" off: the list holds nobody. Free time narrows who × when to the safe (or nobody, "Nothing at all") whatever the cell. Calls are no notification: the Calls grid and the list's floor decide them ([android.md](android.md)).

**Kept** in `config.toml`, `[notify]`, one row a key, a list of column words as `[reach]` writes its rows: a bare column is `now`, `column:value` another value, a column not named keeps its usual value; a word not understood, a value not offered there, a fixed cell left aside; of two words for one column the later counts. Sioul writes a row whole (nine words, `notify::apply`), and with the doses' row the two older switches beside it, for an older Sioul on another device. One key per row: the sharing carries a row as one entry, the later wins, as `[reach]`'s rows.

```toml
[notify]
mail = ["work", "admin", "leisure", "meals", "sleep:later", "pause:later", "free", "slot:later", "dnd:list-any"]
```

**Outside the matrix**, unchanged: what a thing is for and whether now is for it ([areas.md](areas.md)); nothing said during a meeting (a meal's notice, the end of work); the Porch resting after a pause holds new mail; the switches that say whether a kind is told at all (`[reminders] mail`, `mail_newsletters`, `gather`, `events`, `asked_days`, `waits`, `payment_days`, `before_event`, Health's per-block quiet, movement, watch offers); each site's real time, each app's kind and area, each conversation's "Always through"; which device tells; what a platform cannot show (codes on a phone: the card).

**Where each is decided**: codes, `backend::notify_codes` and `sioul watch`; doses, `hours::doses_silent`, `woke_from` (and on a phone `alarm_decide`), the home card's line, the pause's screen and the do-not-disturb modes' dose channel (`everywhere::asks`); reminders, `reminders::Holds` (`ready_in`, `to_tell`; on a phone `eventalarms`, which asks again when the time holding them ends); Health's notices, `health::needs_tick`; the pause to move, `movement_tick`; the watch, `watch_offer`; the end of work, `reviews::notice_at` (`reviews::offer_any_time`); the time running, `timenote::post` (a phone's note follows when Sioul runs or one of its buttons is pressed, not from the background service); new mail, `mailnote::Seen` (and `steps::mail_due`, which fetches in the background only when mail may come); sites, `sites::gather_tick` and `notified`; other apps, `appnotes::Ask` (`next_gathering`, `at_once`, `person`).

## Before an event
- **The usual time**: Settings ▸ Reminders and notifications ▸ **Before an event**: none, 5, 10, 15 (the default), 30 minutes, 1 or 2 hours (`[reminders] before_event`, in minutes). It is counted before the event's margin "before" (getting ready, getting there: `X-SIOUL-BEFORE`, [tasks.md](tasks.md), "In the standards"): an event at 14:00 with 30 minutes to get there is reminded at 13:15 by default.
- **Each event its own**: **Remind** in the event's form (folded with its details), and in its details on the Agenda: as usual, not this one, 5 minutes … 2 hours before. For every time a repeating event comes.
- **What it says**: "14:00 · Dentist, in 45 minutes", then, with a margin, "Getting ready, getting there: from 13:30.", then the place; with **Open** where the platform's notifications take buttons. Said as it is when told: at waking, the time left is less, and "now" when the margin has begun. Another day than the event's: its day too ("Mon 12 Oct 01:00 · …").
- **Each time**: a repeating event is reminded at each time it comes (`before:<UID>:<start>`); a time moved by itself (RECURRENCE-ID) keeps the series' choice unless it says its own.
- **Not for**: a whole day (the working day before tells it; a quarter of an hour before midnight would only wake you), "not this one", events cancelled or declined, calendars you only read unless the event asks. A task's time block is an event like any other ([tasks.md](tasks.md), "Pinned to a time").
- **Kept in the event's own file**: `X-SIOUL-REMIND`, absent as usual, `NONE` for not this one, `15` for minutes, beside `X-SIOUL-BEFORE`. Not a VALARM: your other calendar apps (your phone's, Thunderbird) would ring a VALARM too, and Sioul's reminder would come twice; a VALARM cannot say "not this one"; and Sioul tells the alarms others write already. A Sioul property travels with the event to your other devices running Sioul (CalDAV servers keep unknown properties), and other apps leave it alone.
- **Once per moment**: an alarm the event carries and Sioul's reminder within five minutes of each other are told once, at the earlier; further apart, both come (an alarm a day before, Sioul's a quarter of an hour before).
- **On each device**: your computer and your phone each tell it, the phone in its quiet "Events" channel. Not "the device you used last", as for new mail: a reminder held back on the phone because the computer was used a moment ago, then shown on an empty desk, is an appointment missed; a quiet reminder twice costs less. Within one computer, the window and `sioul remind --watch` share the marks: once.

## With the window closed
- While the window is open, it looks each minute.
- **Settings ▸ Reminders ▸ With Sioul's window closed** writes an entry your session starts at login (`~/.config/autostart/sioul-reminders.desktop`; on macOS `~/Library/LaunchAgents/org.sioul.reminders.plist`) and starts it now: `sioul remind --watch`, a small watcher that reads the same files each minute and tells the same reminders. Nothing else runs; no mail is fetched. Unticked, the entry goes and the watcher stops. One watcher at a time (a lock in `$XDG_STATE_HOME/sioul/remind.lock`).
- Both may run: whichever marks a reminder first tells it.
- Not on Windows yet: reminders come while the window is open.

## In the terminal
- `sioul remind`: what comes in the next two weeks, and what was told.
- `sioul remind --watch`: the watcher.

## The notification
One quiet desktop notification, no sound. From the window it carries "Open" where the desktop's notifications take buttons (Linux): the task, the event or the budget, shown; on Windows and macOS, and from the watcher, it is its text alone.

**On a phone**, the events' reminders come (before an event, the working day before, its alarms), Sioul open or not: Android's alarm clock is given them ahead and asks Sioul at their time ([android.md](android.md), "Events"), in the quiet "Events" channel, a tap opening the event. The other reminders (dates asked, waits, payments, papers, contracts, the money watch) come on a computer only.

## Tested
`cargo test -p sioul-core reminders`: an event the working day before (a short Friday) and at its alarm, a date asked two working days before, a wait over on a weekend told on Monday morning, a bill, each told once; work waiting while work rests; too late, nothing; a task made after its reminder's time, and one done, not reminded; days off moving the working days (in French). Before an event (`before_an_event`): a quarter of an hour, and before a margin of 30 minutes (13:15 for 14:00), the words in English and French, told late at waking; the event's own choice (not this one, 10 minutes, the usual time none); written into the file, read back, changed and taken away; not for a whole day, a calendar you only read, an event cancelled or declined; a time block reminded; an alarm within five minutes told once, at the earlier; an event in the night reminded during sleep, one after waking at waking; the pause and Free time; a repeating event each time, a time moved keeping the series' choice. `what_waits_now`: sleep, the pause, Free time, work resting, and what `Holds` reads of them. `the_usual_matrix_holds_as_before`: every kind of reminder at every time, as the rule before the matrix decided; `the_matrix_changes_what_waits`: a reminder before an event during sleep and in a pause, an event's alarms held, the working day before during do-not-disturb, dates in the evening. `cargo test -p sioul-core notify`: the usual matrix cell by cell (`the_usual_values_are_what_sioul_did`), and against the code it replaced at every time, with and without each layer (`the_usual_matrix_decides_as_the_code_did`); rows read from the configuration (French words, a value not offered, a fixed cell, the older switches); times together and layers; fixed cells; a row written whole and the doses' older switches beside it; the grid's words in English and French. `cargo test -p sioul-core appnotes`: do-not-disturb's list whatever who may reach you says, messages held in Free time, automatons not gathered in working hours. In the window: the settings, and the watcher started and stopped from them. `cargo test -p sioul-app eventalarms`: when a held reminder is asked about again.
