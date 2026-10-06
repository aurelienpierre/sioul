# Reminders before dates

Remembering "on the 30th" is what fails most: time-based remembering is impaired in autism and in adult ADHD, much more than remembering on a cue (Landsiedel, Williams & Abbot-Smith 2017; Altgassen, Kretschmer & Kliegel 2014), and reminders work where memory is the bottleneck (Jamieson et al. 2014, d = 1.27). So Sioul carries the dates, and says each one once, quietly.

Code: `crates/sioul-core/src/reminders.rs` (what to remind, when), `crates/sioul-app/src/remind.rs` (the window), `crates/sioul-app/src/eventalarms.rs` (a phone's alarms), `crates/sioul-cli/src/remind.rs` (`sioul remind`).

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
- **Nothing while you sleep**: from winding down to waking, and during a nap ([areas.md](areas.md), "Sleep"), no reminder is told, yours included: each waits for waking, and comes then if it still makes sense (`reminders::Wait::Sleep`). One exception: an event's own reminders (its alarms, Sioul's before it) when the event itself falls in that sleep. An event at 03:00 is a choice you made: it is reminded at 02:45. Doses are Health's, and come all the same unless you asked them to stay silent ([health.md](health.md), "Do not disturb").
- **The pauses** ([pauses.md](pauses.md)): in a **pause**, an event's own reminders come (the event falls in the pause as far as anyone knows: it has not begun), everything else waits for your return (`Wait::Paused`); in **Free time**, an event's own reminders come when the event falls in it (before the night's start or midnight, when Free time ends), the others wait for its end (`Wait::Free`).
- **Not reminded**: tasks done or cancelled; events cancelled, and those you declined (one of your addresses among the guests, "declined"); the day before and Sioul's reminder before for calendars you only read (holidays, subscriptions), their alarms still told; debits that leave by themselves (presets). A payment reminder that ignores the balance can push an account into overdraft (Medina 2021): the money watch says when the account will not hold a debit, which a reminder alone cannot.

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
`cargo test -p sioul-core reminders`: an event the working day before (a short Friday) and at its alarm, a date asked two working days before, a wait over on a weekend told on Monday morning, a bill, each told once; work waiting while work rests; too late, nothing; a task made after its reminder's time, and one done, not reminded; days off moving the working days (in French). Before an event (`before_an_event`): a quarter of an hour, and before a margin of 30 minutes (13:15 for 14:00), the words in English and French, told late at waking; the event's own choice (not this one, 10 minutes, the usual time none); written into the file, read back, changed and taken away; not for a whole day, a calendar you only read, an event cancelled or declined; a time block reminded; an alarm within five minutes told once, at the earlier; an event in the night reminded during sleep, one after waking at waking; the pause and Free time; a repeating event each time, a time moved keeping the series' choice. `what_waits_now`: sleep, the pause, Free time, work resting. In the window: the settings, and the watcher started and stopped from them. `cargo test -p sioul-app eventalarms`: when a held reminder is asked about again.
