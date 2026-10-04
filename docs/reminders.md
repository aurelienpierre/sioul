# Reminders before dates

Remembering "on the 30th" is what fails most: time-based remembering is impaired in autism and in adult ADHD, much more than remembering on a cue (Landsiedel, Williams & Abbot-Smith 2017; Altgassen, Kretschmer & Kliegel 2014), and reminders work where memory is the bottleneck (Jamieson et al. 2014, d = 1.27). So Sioul carries the dates, and says each one once, quietly.

Code: `crates/sioul-core/src/reminders.rs` (what to remind, when), `crates/sioul-app/src/remind.rs` (the window), `crates/sioul-cli/src/remind.rs` (`sioul remind`).

## What is reminded, when
| What | When | Waits while work rests? |
|---|---|---|
| An event | half an hour before work ends, on the last working day before it: "Mon 12 Oct 09:00 · Dentist", the place below | no |
| An event's own alarms (VALARM, set by you, your phone or whoever invited you) | at their time | no |
| A date asked (a task's due date) | when work starts, 2 working days before (Settings ▸ Reminders) | unless the task is yours (quiet time keeps it) |
| A wait over (a step done, then "wait 14 days for the answer") | when the wait ends, at the next working hours | the same |
| A payment planned (a bill, a tax: a planned line of a budget) | when work starts, 2 working days before | no |

- **Working days** are those of your working hours, less days off; without working hours, Monday to Friday, 9:00 to 17:00.
- **Once**: each reminder is marked when told (`$XDG_STATE_HOME/sioul/reminded/`, a small file each, forgotten after two months). Never repeated, no count of what was missed, no red.
- **Not for what you just made**: a task made after its reminder's time, an event changed after it, is not reminded: you just saw it.
- **Late, but not too late**: a computer asleep at the time reminds on waking, while it still makes sense: before the event begins, before the day asked ends, within a week of a wait's end.
- **Not reminded**: tasks done or cancelled; events cancelled; the day before for calendars you only read (holidays, subscriptions), their alarms still told; debits that leave by themselves (presets). A payment reminder that ignores the balance can push an account into overdraft (Medina 2021): the money watch says when the account will not hold a debit, which a reminder alone cannot.

## With the window closed
- While the window is open, it looks each minute.
- **Settings ▸ Reminders ▸ With Sioul's window closed** writes an entry your session starts at login (`~/.config/autostart/sioul-reminders.desktop`; on macOS `~/Library/LaunchAgents/org.sioul.reminders.plist`) and starts it now: `sioul remind --watch`, a small watcher that reads the same files each minute and tells the same reminders. Nothing else runs; no mail is fetched. Unticked, the entry goes and the watcher stops. One watcher at a time (a lock in `$XDG_STATE_HOME/sioul/remind.lock`).
- Both may run: whichever marks a reminder first tells it.
- Not on Windows yet: reminders come while the window is open.

## In the terminal
- `sioul remind`: what comes in the next two weeks, and what was told.
- `sioul remind --watch`: the watcher.

## The notification
One quiet desktop notification, no sound. From the window it carries "Open": the task, the event or the budget, shown.

## Tested
`cargo test -p sioul-core reminders`: an event the working day before (a short Friday) and at its alarm, a date asked two working days before, a wait over on a weekend told on Monday morning, a bill, each told once; work waiting while work rests; too late, nothing; a task made after its reminder's time, and one done, not reminded; days off moving the working days (in French). In the window: the settings, and the watcher started and stopped from them.
