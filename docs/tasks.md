# Tasks, notes, links and focus

A plan written in prose is a wall: every line is there at once, and the next step has to be found again each time. Sioul turns the plan into tasks that wait for each other, shows the one step to take now with the reason, and keeps every task tied to what it needs: its notes, the mail it came from, the people, the drafts, the case. The research behind each choice is in [research.md](research.md), findings 6 to 20.

## Calm first, for tasks
1. **One next step, picked for you, with its reason.** "Now" shows one step, why it comes now ("its date is 30 October: three weeks left"; "it frees two other steps"), and the one after it. Equal steps are not shown as a choice to make: Sioul picks, and says it picked. Two other choices wait behind one click.
2. **Nothing is ever overdue.** A day to start that passed says nothing: the task is simply free. The date asked from outside (DUE) is said as time left; once passed, as "Date asked: 1 October", never in red, never counted. The plan always starts from today.
3. **"Not now" is honoured without argument.** It puts the step's whole stream off until tomorrow (the tasks tied to it by what they wait for or by being steps of the same task), so what comes next is free to start, not its own next step; streams are ranked by how soon their dates come. A new day starts clean.
4. **Optional stays optional.** A task tagged `joy` (only if you want to) is offered in a fold, never proposed as the next step; one tagged `someday` (when you say so) is parked at the end of the list. Neither takes room in the plan.
5. **Starting is the hard part, so it is helped.** "Start" offers two minutes first; "What makes it hard?" gives one matching help per answer: its notes and messages opened, a step added, two minutes to start, a lighter day.
6. **Time made visible, without pressure.** The focus window stays on top while you work elsewhere: a disc drains in a neutral colour, with no ticking and no sound. Two minutes before the end it says it is time to find a stopping point; at the end, "Keep going" or "Stop here, it counts". Stopping offers one line, "next time, start by…", shown when the task comes back.
7. **How much a day holds is yours to say.** "How is today?": clear, haze or fog. Haze keeps less for today; fog shows only small steps, and one step is a full day. Nothing is inferred from what you do.
8. **Finishing says what it changed.** "Done. This frees: Send the registered letter." Once, in the status line.
9. **Nothing counts what you did not do.** No streaks, no postponement counters, no score. "Done this week" shows what got done.
10. **The plan proposes; your dates stay yours.** The days the plan gives each task are computed each time, from today, and never written into the tasks. Only what you set (a day to start, a date asked, an order) is stored. The plan is made again when Sioul starts, every twelve hours and at midnight: a task not done by its planned day moves on, and what waits for it follows.
11. **"Done for today" closes the day, early or not.** One click, no question; "Undo" waits in the status line. Today's room goes to zero and the plan flows on into the next days with room, never above a day's room; nothing is written into the tasks. A screen, read in ten seconds, says when work comes back, what got done or worked on (only if something was), that everything else has its place, the first step when work comes back (the line left when a session stopped, else the plan's first step, one tap to say it your way), a date asked before then if there is one (with "Ask for more time", or leave it), and what still gets through. No count of what was not done, no history of early stops, no "are you sure". The next working day opens on that step, never on yesterday. Then quiet time until work comes back ([porch.md](porch.md)): "Rest of the day", only what is yours; a thought noted then waits, out of sight, for work to come back. Findings 21 to 23 of [research.md](research.md).
12. **Offices have hours.** A task marked "Needs an open office" is proposed only while offices are open (Tasks ⚙, Monday to Friday 9:00–17:00 by default) and planned only on those days; on a Saturday, the next step is one you can do.

## In the standards
Tasks are CalDAV tasks (VTODO, RFC 5545 §3.6.2) in task lists on your calendar server, so any client and any device sees them. What ties them together is written into them, in RFC 9253's terms where it exists:

| What | Written as |
|---|---|
| A task | a VTODO in a task list (a calendar whose `supported-calendar-component-set` holds VTODO) |
| A step of a bigger task | `RELATED-TO;RELTYPE=PARENT:<UID>`, in the step |
| Waits for another task | `RELATED-TO;RELTYPE=DEPENDS-ON:<UID>`, in the one that waits. With a gap ("the answer comes within two weeks"): `RELATED-TO;RELTYPE=FINISHTOSTART;GAP=P14D:<UID>` in the one that comes first, as RFC 9253 §4 places temporal links. Both are read; `NEXT` too. |
| Can start from | `DTSTART` (a date, or a time in UTC) |
| The date asked | `DUE` |
| How long it takes | `ESTIMATED-DURATION:PT15M` (draft-ietf-calext-ical-tasks) |
| Who does it, what it is about | `CATEGORIES`: `you`, `proxy` (someone with you or for you), `joy`, `someday`, and topics |
| Its kind: a call, writing, online (a form), going out, reading, thinking, making, or one of yours (Tasks ⚙: renamed, taken away, added) | `CONCEPT:tag:aurelienpierre.com,2026:sioul/task-type/call` (RFC 9253 §7.3, a tag URI, RFC 4151); other applications' concepts are left as they are |
| Needs an open office | `CONCEPT:tag:aurelienpierre.com,2026:sioul/needs/office-hours` |
| Its case | `REFID:<case id>` ([case-store.md](case-store.md)) |
| Its notes, the mail it came from, its drafts, an event | `LINK;LINKREL=describedby` (a note), `via` (what it was made from), `related`; `VALUE=URI` for a note, a message or a draft, `VALUE=UID` for an event |
| The people and offices involved | `CONTACT;ALTREP="sioul:contact/<UID>":<name>` |
| Order among equals | `PRIORITY` (1 first) |
| Time kept before and after it, in minutes: getting there, getting ready, coming back (events too) | `X-SIOUL-BEFORE:20`, `X-SIOUL-AFTER:15` |
| What it costs and what it gives back, 0 to 10 each as you rate them, the rated ones only (events too) | `X-SIOUL-COST:COGNITIVE=3;EMOTIONAL=5;ANXIETY=7`, `X-SIOUL-GAIN:6` |
| Comes back | `RRULE:FREQ=…`; done, it moves to its next turn after today, as tasks.org does |
| Done | `STATUS:COMPLETED`, `COMPLETED`, `PERCENT-COMPLETE:100` |

- **Editing keeps the rest**: tasks are changed line by line, as contacts and events are; what Sioul does not edit (alarms, another application's fields) comes back exactly; a form saved unchanged writes nothing.
- **New lists** are created on the server with MKCALENDAR (RFC 4791 §5.3.1), new address books with an extended MKCOL (RFC 5689), at the next sync; until then they wait here, and a sync never deletes them. A list in the account `local` stays on this computer.
- **Things are named by addresses**: `mid:<Message-ID>` for a message (RFC 2392); `sioul:task/<UID>`, `sioul:event/<UID>`, `sioul:contact/<UID>`, `sioul:note/<path>`, `sioul:draft/<id>`, `sioul:case/<id>`, `sioul:budget/<budget>/<line>`.
- **Each link lives in the thing that makes it**: a task or an event (above), a note (a Markdown link, or its front matter: `task:`, `event:`, `mail:`, `links:`), a draft (its `links`), a budget line (its `links`). What none of them can hold (a message is never changed) goes in `$XDG_DATA_HOME/sioul/links.toml`. Links are read both ways: every thing shows what it points at and what points at it.
- **Time spent** is kept in plain files, one per month (`$XDG_DATA_HOME/sioul/time/2026-10.toml`), not in CalDAV: it is your record, not the task's. The session running now is kept too, so closing the window loses nothing.

## The plan
- **Order**: Kahn's algorithm over the open tasks; a step waits for what its bigger task waits for, and a bigger task ends with its last step. Tasks that wait for each other in a loop are found (Tarjan's strongly connected components) and said: "These wait for each other: A, B. One of them has to go first."
- **The next step**, among the tasks free to start: the one started; then the one whose latest start comes first (its date, or a later task's, minus the work in between); then your PRIORITY; then the one that frees the most others; then the smaller.
- **Days**: each open task goes into the first days with room, in that order, never before today, never before what it waits for (plus its gap). Room is your hours ([areas.md](areas.md)), each kind for its own tasks: work hours for work, admin hours for your admin, free time for leisure; admin without hours of its own goes to work hours, work without hours to admin hours. No second budget: Sioul can run your work too, so the plan may fill your hours. The events in them are taken out, with a pause of five minutes before and after each, and each step leaves a pause of five minutes after it. A step up to an hour is never cut; a longer one is cut in parts of a quarter of an hour at least. Margins are not pauses: a task's room is its length plus the time kept before and after it, and the plan cuts it by that room as it cuts any step (`plan.rs`, `whole`): an hour or less in all goes in one go, more may go in parts over several days. The day never cuts a task that has margins (going there twice is not the same step): it lays today's part in one piece, its margins at both ends, in the first gap that holds it, or says it does not fit (`dayview.rs`). The rule and the plan differ there (below, "Where it stands"). An event's margins are taken out before its pauses. No hours set at all: Monday to Friday, 9:00 to 17:00, for everything. Today's room starts now and is scaled by the weather (haze 60 %, fog 30 %). Days off and today once closed have no room; a task that needs an office goes only on days offices open. A task whose plan ends after its date says so once, calmly, with what would help: doing it sooner, making it smaller, handing it over.
- **Streams**: tasks tied by what they wait for or by being steps of the same task form a stream (union-find); "Not now" sets a whole stream aside for the day, and the next step comes from another stream.
- **Waiting on others**: a task done starts the clock of what waits for it with a gap; the next one waits until then, and says until when.
- **Consistent constraints**: the next step is never a task that waits for one still open, nor one whose day is ahead, even when it was started too soon (it stays in "Started", and says what it waits for); never a call to an office that is closed now.
- **An office's own hours**: a task that needs an open office takes, when you say them, its office's opening hours, in its panel: open from a day to a day, from a time to a time, and a second range for a lunch break ("mo-fr 09:00-12:00, 14:00-17:00", `X-SIOUL-OFFICE-HOURS`). It is then proposed only while that office is open, planned on its days, and the note says when it opens next; without them, offices' usual hours (`[[office_hours]]`, Monday to Friday 9:00–17:00).
- **What it is for**: work, your admin, leisure, any of them together (`X-SIOUL-AREA`), else as its tags say; the task pages show what fits the hours now ([areas.md](areas.md)).

## Places
- **Where you stopped**: one line, left whenever something interrupts you (a pause to move taken, a meal's or the night's notice and its **Options…**, the pause to move's notice and its **Where I stopped…**, the end of the day, a timer stopped, or **New ▸ Where I stopped…** at any time), shown again at the top of the Porch and of this page, and in the focus window, until "Done" (`sioul_core::stopped`, `StoppedCard.qml`; carried to your other devices). Stopping then costs no fear of forgetting where you were: a cue to resume cuts the time to get back into a task (Trafton et al. 2003; Leroy & Glomb 2018). A timer stopped without a word keeps the line left at its pause.
- **The time running, in the system's notifications**: while a session runs, one notification says its task and since when ("Since 14:02, 25 min chosen"; paused, "Paused, 12 min so far"), with **Pause** (**Go on**) and **Stop**, the focus window's own buttons; a click on it opens the task. It follows the session from wherever it changes (the window, the notification, another device, `sioul focus`), after each reading of the tasks and at the window's minute (`timenote.rs`). On a Linux desktop, the desktop's (`sioul_sync::notify::ongoing`, on a D-Bus connection of its own, which hears its buttons): resident, without sound, at normal urgency, for Plasma shows a low one nowhere by default; where the server keeps what it shows (Plasma's history, GNOME's tray), it goes there after its popup, buttons alive, elsewhere it never expires; taken away when Sioul quits. On Windows and macOS, none yet: `notify::ongoing` does nothing there, and the focus window alone shows the time. On a phone, Android's, with a chronometer, its buttons working without the window ([android.md](android.md), "The time running").
- **Now**: the next step and why, its estimate and where you stopped, "Start", "Done", "Not now", "What makes it hard?"; after that, the next one in a line; folded: two other choices, what is started, what you might do if you want, what got done this week.
- **List**: every open task, a bigger task followed by its steps, in the plan's order, grouped by case or by list; a search; done tasks on request; the optional ones last.
- **Board**: free to start, started, waiting, done (two weeks). Cards move by dragging, with a mouse or a touchpad (`acceptedDevices`): to "Started", to "Done", back to "Free"; on a touch screen a drag scrolls the board. "Waiting" is decided by what each task waits for, and each card says what. More than three started: one line asks whether to finish or park one.
- **Timeline** (a Gantt chart), on demand: each open task on its days, the date asked as a small diamond, days without room shaded, one case or all.
- **A task, open on the right**: start, done, not now; its steps and one more in a line; what it waits for (removable, and one more found by its title); what it frees; folded: its dates, length, case, tags, how it comes back, its notes in Markdown; then everything tied to it, and "Write an email" (a draft to the people it involves, linked both ways) or "Make a note".
- **One line makes a task**, from any place on the page: "Call the tax office tomorrow ~15m #taxes {30/10}". At the end of the line, in French or English: the day to start ("tomorrow", "vendredi", "next week", "dans 3 jours", "30/10", "30 octobre"), `~` how long, `#` a case or a tag, `{…}` the date asked; "@day" anywhere, and "@call", "@write", "@online", "@out", "@read", "@think", "@make" (or "@appel", "@écrire", "@enligne", "@dehors", "@lire", "@réfléchir", "@faire") for its kind. Without one, a first word that says it plainly gives the kind ("Call…", "Remplir…"); "Ask…" says nothing, as it could be a call or a message. What was understood shows as chips before Enter.
- **One kind, one category**: two choices at the top of the page show only those tasks in every view, Now included ("the next call"); the plan stays whole, so a call that waits for a message still waits. The choices (and the list's grouping, done tasks, one case) are kept between sessions (`$XDG_STATE_HOME/sioul/tasks-view.toml`); searches are not.
- **Kinds and categories are yours** (Tasks ⚙): a kind is renamed in place, taken away (tasks keep it) or added (`[[tasks.kind]]`, its id written in the task's concept, "@" and its name in one line); a category is renamed on every task that has it (a name already used merges them) or, after a second word, taken off them all.
- **Notes**: the case store, read as Obsidian reads a vault: `[[wikilinks]]` (found by name anywhere, the same folder first, then the shortest path, then aliases), Markdown links, `#tags`, front matter, links back. Nextcloud Notes' folder works as well: its categories are folders, and its `.txt` notes (its default) are read and written as Markdown like `.md` ones, keeping their extension (`notes::kind_of_file`, `notes::text_stem`); new notes are `.md`. Search first, the notes changed lately on top, then every note as one list (each with its folder) or as a tree of folders, folded until opened (the choice is kept). A note opens read, its links followed with a click wherever they lead; editing is one switch away. Unticked `- [ ]` lines are one click from becoming tasks, linked to their note.
- **Pictures, PDFs and sounds are notes too**: listed beside the Markdown files (`png`, `jpg`, `gif`, `webp`, `svg`; `pdf`; `ogg`, `opus`, `mp3`, `m4a`, `wav`, `flac`…), opened in place (a picture fitted to the page, a PDF's pages with QtPdf, a sound with play, pause and where it is), and tied like any note (`sioul:note/<path>`). `![[scan.png]]` shows the picture inside a note, at most as wide as the page; any other embed is a link. "Record an audio memo" records into `<notes>/memos/` as Opus, the microphone open only while it records, and opens it as a note.
- **Renaming and deleting notes** (right click): a new name takes the notes that name it along (`[[name]]`, `![[name.png]]`, Markdown links by path, `sioul:note/` addresses), and the tasks and ties that link to it; deleting moves it into the vault's `.trash`, as Obsidian does, "Undo" offered.
- **Lists, calendars and address books** (Tasks ⚙, Agenda ⚙, Contacts ⚙): renamed in place, here and then on the server (`PROPPATCH`); an empty one deleted, here and then on the server, unless something was put in it there meanwhile (`DELETE`); one holding anything stays.
- **Everything made from everything**, linked both ways: a task, an event or a note from a message; a note for an event (dated, its guests listed), then sent to the guests; a task to prepare an event; a task from a note's line; an email from a task. Every message, event, contact, task and note shows what it is tied to.
- **Links by hand**: every card ends with "Add ▾" (a task, an event, a note, a message, or a sender into the contacts) and "Link to…", which finds anything by a few words of its title, of one kind or all (`World::search`). Right click on a task, an event, a note, a contact, a message, a Porch line or a budget line offers the same; right click on a tie undoes it. A tie is written where it can be held (`Loaded::tie`): in the task when one end is a task (so the server and your other devices carry it), else in an event whose calendar can be written, else in a note's front matter (`links:`), else in `links.toml`. Undoing takes it out wherever it is written; a step, a wait, a guest or a link in a note's text is changed where it is, and that is said.

## Command line
```
sioul tasks [now]                      the next step, why, and the one after it
sioul tasks list [--by case|list] [--done] [words]
sioul tasks board | timeline [--case <id>] | show <task>
sioul tasks add "Call the tax office tomorrow ~15m #taxes" [--list <account/id>] [--parent <task>] [--after <task>]
sioul tasks done | start | not-now <task>     (a task: its UID, or the start of its title)
sioul tasks weather clear|haze|fog
sioul tasks lists | new-list <account|local> <name> [--events]
sioul tasks import <file.toml> [--dry-run] [--no-sync]
sioul dav new-book <account> <name>
sioul focus start <task> [--minutes 25] | status | stop [--done] [--note "…"]
sioul links <address>                  what a thing is tied to, both ways
sioul notes [words] | notes --path <path in the store>
```

### Importing
`sioul tasks import` reads a TOML file of tasks, offices and drafts, for agents and scripts. Each entry has a key; UIDs are the file's `prefix` and the key, so importing again changes what the file made, line by line, instead of adding it twice. What you added since (a link, a tag, a tick) stays.

```toml
list = "account/plan"      # where new tasks go (else the first list made for tasks)
book = "account/admin"     # where new contacts go
prefix = "plan-"

[[contact]]
key = "office"
name = "Tax office"
phones = ["+33 3 00 00 00 00"]
address = "1 rue de l'Exemple, 75001 Paris"

[[draft]]                  # kept in Sioul; nothing is sent
key = "ask"
account = "mail-account-id"
to = ["office@example.org"]
subject = "Late returns"
body = "…"

[[task]]
key = "send"
title = "Send the registered letter"
parent = "returns"         # a key, or "uid:<UID>"
after = ["gather"]         # waits for
after_gap = { letter = 14 }  # waits 14 days after "letter" is done
start = "2026-10-15"       # can start from
due = "2026-10-30"         # the date asked
estimate = 20
priority = 3
tags = ["you", "admin"]
cases = ["taxes"]
note = ["admin/letters.md"]                   # notes of the case store
links = [{ uri = "mid:abc@example.org", rel = "via" }]
contacts = ["office"]
drafts = ["ask"]
done = "2026-10-01"        # already done, on that day
```

## From the research, built and next
| Idea | Where it stands |
|---|---|
| One next step, picked, with its reason; ties picked and said | built |
| No overdue: plan date and date asked apart | built |
| Two-minute start, stopping counts | built |
| "What makes it hard?", one help each | built, without the AI |
| A breadcrumb when stopping | built: one line on stopping, shown when the task comes back |
| The day's weather sets its room; fog shows small steps | built |
| What a task takes (energy accounting, Toudal & Attwood; Structured's Energy Monitor without points): light, the usual, heavy, or "it gives back" (a walk, music), in the panel's "More" (`X-SIOUL-ENERGY`). A day takes two heavy tasks when clear, one in haze, none in fog: the next goes to a day that can take it; in fog a heavy task is the next step only when nothing lighter is free, and says so. What gives back takes no room and is never proposed: after a heavy step done within two hours, it is offered on Now. No gauge, no score, no red (`plan.rs`, `taskview::now`) | built |
| Time made visible: a draining disc, a heads-up before the end, no sound | built |
| What a task or an event asks, and what it gives back: minutes kept before and after it (getting there, getting ready, coming back), kept free in the plan and shown apart in the day ("Around: …"), never counted as a pause; three costs from 0 to 10 (cognitive, emotional, anxiety; COPSOQ and the demand–control tradition keep cognitive and emotional demands apart, aversiveness predicts putting off) and a gain from 0 to 10, each unsaid until you say it, in the task panel's "More" and the event's form. One difficulty from them: their mean with the highest counted twice, so that one frightening call is not drowned by two easy parts (`demands.rs`) | built as data: written, read, edited. Next: a daily budget per cost learned from your past days, the gain as a floor to reach rather than a sum to spend, heavy then light, and estimates corrected by the ratio of time spent to time guessed (log-ratio, about two weeks' memory, per kind pulled toward your overall ratio, the margin held for the whole day, never per task) |
| A time budget near a date asked (Shovel's "cushion"): "Until Wednesday 7 October: about 30 min of steps, 3 h of room." On Now for the next date asked within a week, in a task's panel within three weeks; further away it is noise. What must be done by then counts what each task waits for and its steps (a bigger task only through them); room is each day's minutes, today's weather and days off counted. When it does not fit: "More time asked, or a lighter plan?" (`plan::cushion`) | built |
| Finishing says what it freed | built |
| Capture in one line, its parse shown as chips | built, inside the window (a system-wide shortcut later) |
| Done this week, never streaks | built |
| What a task is tied to, at the point of action | built, both ways, on every object |
| A Waiting column saying what each waits for; a line past three started | built |
| Splitting a task into steps at a chosen granularity, by AI | next: steps are added by hand today |
| Company: a quiet presence, an AI at start and end, a "stuck" button to the proxy | next |
| Cues from events ("after the Porch window") rather than clocks | next |
| A calm "still relevant?" pass for long-untouched tasks | next |
| The admin window played as a routine; routines of your own (Routinery: timed steps played one at a time, the next one said before it comes) | built: Tasks ▸ Routines. One step a line with its minutes ("10 min Open the Porch", "Make tea 5"), kept in the configuration (`[[routine]]`, so they travel with your settings). Played in a small window on top: the step, its time draining in a neutral colour, "Then: …", the routine as dots; Done, Open (what the step opens), +5 min, Skip, Pause, Stop, none counted. When the time is up it moves on by itself only if the routine says so; else it waits and says so, without a sound. The admin window's routine is made from what is there: the Porch, the plan's next step, a line where you stopped (`routines.rs`, `RoutinePlayer.qml`) |
| The day, seen (Tiimo, Structured; visual supports cut transition time, Dettmer et al. 2000) | built: Tasks ▸ The day. Today's events at their times, the steps the plan gives today laid from now into the hours of their kind (work, your admin, free time, shown as bands), around the events, five minutes' pause after each step and around each event, a step up to an hour in one go, a longer one going on after a break; each card as tall as its text, the day scrolled to the current hour; a line where now is; the step under way marked, the next outlined; a task done today stays where it ended, ticked and dimmed (a quarter of an hour to an hour long, by its estimate); what does not fit before the day ends keeps its place in the plan, said in one line. It follows the clock: the line moves each minute, and the day is laid again from now every five minutes and as soon as Sioul comes back, with what another device marked done or noted. A layout to look at, never a schedule: nothing is written into the tasks (`dayview.rs`, `DayView.qml`) |
| Personal "sides" offered when a dreaded step starts | next |
| Estimates as personal ranges from the time spent | next: time is recorded, not yet used |
| A weekly fresh start | next |
| Three presets instead of settings | next |

## Where it stands
| | |
|---|---|
| Done | tasks as VTODO with RFC 9253 links, edited line by line; the plan (order, loops, next step, days with room, gaps, streams, days off, office days, made again every twelve hours and at midnight); Now, List, Board, Timeline; the task panel; capture in one line; the focus window and the time log; the day's weather and "Not now" by stream; "Done for today" and quiet time; office hours; the plan in your hours of each kind, around events, with pauses; the day laid out; kinds and categories edited; joy and someday; notes as a vault, edited and linked; links both ways on tasks, events, mail, contacts and notes; tasks, events and notes made from each other; task lists and address books created on the server; import for agents; reminders before dates asked and waits over, once each ([reminders.md](reminders.md)) |
| Next | the ideas marked "next" above; changing one turn of a repeating task; a note's history with git |
| Rule and code differ | a task with margins is meant never to be cut; the plan still cuts one whose length and margins pass an hour, as any long step (`plan.rs`, `whole` counts the margins in the minutes but has no exception for them); only the day keeps it in one piece |
