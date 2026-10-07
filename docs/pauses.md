# The two pauses: Free time and Pause

Two buttons at the end of the status line put Sioul on hold, for two different moments:

- **Free time** (« Temps libre »): you take a good moment, the weather or the energy there is now. Until you come back, now is leisure whatever the hour; only your safe senders reach you; leisure is offered, never a list to finish. The working time it takes moves the end of today's work later by as much, within limits Sioul keeps.
- **Pause** (« En pause »): a moment overwhelms you. Everything Sioul shows is held, on all your devices; the window is covered with a few lines; nothing is asked, nothing sent, nothing moved into the evening. Coming back, the rest of today is lighter.

The research behind them: [research/global-pause.md](research/global-pause.md) (criteria **GP1–GP21**) and [research/emergency-pause.md](research/emergency-pause.md) (**P1–P31**). Sioul is not a medical tool and not an emergency service: the pause detects nothing, watches nothing, treats nothing; it acts only when you press it (P5, P29–P30).

Code:
- `crates/sioul-core/src/pause.rs`: the settings, the state, the end of work moved, the return, the screen, the numbers;
- `crates/sioul-core/src/quiet.rs`: the two pauses among the times (`Reason::FreeTime`, `Reason::Paused`, `Reason::Extended`), what may notify (the matrix's pause and Free time columns, `attention`, [attention.md](attention.md));
- `crates/sioul-core/data/crisis-lines.toml`: the emergency numbers and crisis lines;
- `crates/sioul-app/src/pauses.rs`: the window's side; `crates/sioul-app/src/dnd.rs`: the system's do-not-disturb;
- `qml/PauseCover.qml` (the pause's screen), `qml/PauseSetup.qml` (its setup), the buttons in `qml/main.qml`, the offers in `qml/TasksPage.qml`.

Each rule is marked:
- **O**: the owner's decision;
- **R**: from research, its criterion named;
- **G**: a guess, a default to tune;
- **T**: to test with the people concerned before it is trusted (P31).

## The owner's decisions

- **Names** (O): « Temps libre » and « En pause »; in English "Free time" and "Paused", the button "Pause". The research's names were "global pause" and "emergency pause"; "Emergency" and "SOS" stay out of every text (P28).
- **No sending of any kind** (O): no call, SMS or mail from the pause, not even optional, for now. What helps is the person's own, set up beforehand. So P3, P4 and P18–P21 do not apply, apart from P21's first half: nothing about a pause reaches the developer (no server, no telemetry, no log). The research's undo window before a message (P6) has nothing to delay.
- **The words** (O): literal and comforting: facts about Sioul, never claims about you; no "we", no idiom, no question, no instruction, no exclamation mark, no countdown, about 40 words at most (P11–P12). "Holding the fort" was the idea; its literal form is "Sioul is keeping everything on hold".
- **Do-not-disturb** (O): both pauses use the system's do-not-disturb, with exceptions set beforehand, on each platform as it allows (below).
- **Everything else** follows the research.

## Where the state lives

- **In `quiet.toml`, the sharing's "time" part** (O asked to choose; chosen here). The pauses are overrides of what now is, as "Done for today" and "Work now" already are, in the same file; the time part carries it with the plan's other files, and every device that plans needs it to lay the same day (Free time's moved end, the lighter day after a pause). A pause pressed on one device is a pause on all (P8), once the sharing has carried it: at each device's next exchange, a minute at most when the window is open; a phone with Sioul put away follows when it is opened again (Android freezes an app in the back).
- **Two stamps per pause**, `free_since`/`free_ended`, `paused_since`/`paused_ended`: on while the press is later than the end. Never a key taken away: the sharing merges key by key, and a pause is never half on (G).
- **The setup** is in the configuration, `[free_time]` and `[pause]`, with the settings part.
- A device whose time part is off pauses alone.

## Which comes first

The pause, then sleep, then Free time, then meals, then the hours' own overrides ("Done for today", "A little longer", "Work now"), then the hours (`quiet::mode`).

| Now | Time | What still comes |
|---|---|---|
| Paused | held: as during sleep, nothing new shows | dose reminders, unless the pause's setup holds them too; your alarms, and your events' own alarms and lead reminders for an event that falls in the pause ([reminders.md](reminders.md)) (P7) |
| Sleep (the night, a nap) | sleep | doses, as [health.md](health.md) says |
| Free time | leisure, whatever the hour | your safe senders' mail, or nobody's; doses; codes you asked for; your events' own alarms (GP5) |

- The pause is `Time::Sleep` for what comes, `Reason::Paused` for what is said: no notification but doses and the alarms you set (an event's own alarm is one), no task, no project, the Porch's codes waiting (P7). (R; an event's alarm reading as an alarm you set: O, through the notifications' rule "never during sleep or a pause unless the event falls in it")
- Free time ends by itself at the night's start (its wind-down), else at midnight: it never covers the night, and tomorrow starts as usual (O; GP10a: the night is sleep's). A nap inside it is a nap.
- Pressing Pause during Free time ends Free time, and today's end of work stays as usual: a pause never costs an evening (GP19–GP20, P7). (R)

## Free time

### Starting it
- **The person's act** (GP1): the button, never proposed by Sioul, never from behaviour, never "to work better". (R)
- **Leaving work behind** (GP3): a focus session running stops and counts, and the line on where you stopped is offered, once, dismissible. (R)
- **No timer, no length, no countdown** (GP4). (R)
- **The weather does not prompt** (GP2): nothing is said of the weather. (R; the opt-in weather line is not built)

### Who reaches you
- **Your safe senders only**, quietly, at the times you ticked for them; neutral and restricted senders wait whatever their ticks (GP5; `pause::reach_now`). Codes and links you just asked for, and what you send yourself, come at once, as in every quiet time. (R)
- **"Nothing at all"** (GP6): a setting, and a switch in the status line's menu for this free time: not even your safe senders; doses and codes still come. (R)
- **Notifications**: doses, codes you asked for and your events' own alarms; reminders before dates, sites, meal notices, the pause to move, the watch's offers and the work day's notice wait, as usual (`notify`, its Free time column; `reminders::Holds`). (R: GP5; which notices wait is G)
- **The phone** lets your starred contacts through, unless "Nothing at all" (do-not-disturb, below). (O)
- **One sentence says it**, the status line's: "Free time: only your safe senders, doses and codes reach you. Work comes back when you do." (GP5, GP16) (R)

### What is offered
- **Leisure, offered, never a list** (GP7): the Tasks page shows "Free time", its line, and "If you want:" with your open leisure items (joy and someday included), at most eight, in the order you made them, unranked; no next step, no timer, no count, no "after that". (R; eight is G)
- **Movement and exercise** are offered only while "Offer movement and exercise" is on; unsaid, it is on unless "Days kept even" is (the setting for energy-limiting illness): then no exercise is suggested (GP8; categories such as sport, walk, yoga, swim, in English and French, `pause::MOVEMENT`). (R; the word list is G)
- What you do in it counts as any item would, on its own ratings (GP8, G24). (R)

### The end of work
- **It moves later by the working time Free time took** (GP10): the room your working hours had in it, as the plan counts room (meals, naps and events aside), summed over the day's free times (`pause::worked_away`). (R, inference)
- **Never past the earliest of** (GP10):
  - the wind-down less an hour (G: an hour of low-arousal time before it);
  - your latest end of work: usual end + 2 hours unless set (G);
  - the last moment that leaves the evening's time for you its room before the night (capacity.md §9);
  - midnight.
  The evening's events, meals and admin hours are gone around (G).
- **Hours, not capacity** (GP9): the day's budgets do not grow; the plan gives the moved end's room to **light steps only** (O: "only light, non-anxious steps"; GP12 allows usual ones, the owner's word wins); they go there first; nothing heavy or worrying after the usual end (GP12–GP13). (R, O)
- **What no longer fits goes to later days**, as the plan always lays it, with no count (GP10, GP14); a dated step that can no longer be done is said as the plan always says it. (R)
- **Said once** (GP16): coming back, the status line says "Today, work runs until 19:00." and nothing more; no countdown, no "hours owed", no number of steps moved. (R)
- **One key keeps the usual end** (GP11, GP19): "Keep my usual end", beside that sentence, in the status line's menu during Free time and while the end is moved, no reason asked; that day nothing more moves. The setting "The end of work moves" off makes it the default. (R)
- **Sioul ends the moved time itself** (GP21): from the usual end to the moved end the time is work (`Reason::Extended`, no line: it was said once); at the moved end, leisure. "Done for today" ends it earlier, as any day (GP15). (R)
- While Free time goes on, the plan already lays today as if you came back now.

### Never
- Pauses are not counted, charted, compared or used to infer anything (GP17, P26, P30). (R)

## Pause

### Set up on a calm day (P1), Settings ▸ Pauses
- **Said once** (P5, O): "Sioul is not an emergency service. It does not watch you. It acts only when you press Pause."
- **What is held**: everything Sioul shows; **Dose reminders still come** (unless held: the doses' pause cell of the matrix, P7, health.md); your Always through people's calls and messages come, the phone's mode letting them through as far as Android can say it (P9, [attention.md](attention.md) §6).
- **What helps you** (P2, P15): in your words, one thing a line; a line with a link or a file's path opens it from the pause (a playlist, photos, an app's page). Nothing generic is added, no library to browse.
- **A line for the pause** (P17): one line of yours, shown on the screen; never a counted exercise.
- **Breathing guide** (P16): off unless switched on; its pace, breaths a minute (6 unless set: G).
- **Coming back** (P23): lighter, as a hazy day (unsaid); no more work today; as planned.
- **Numbers for** (P13): the country whose emergency number and crisis line show; unsaid, the phone numbers' country (Contacts), else the system's, never guessed from the language.
- **Do not disturb**: what this device can silence, said plainly, with the system's pages it needs (below).
- **Try the pause screen** (P1): the screen as it will be; nothing held.
- **Forget the last pause** (P26): see "What is kept".

### The press (P6)
- One button, **Pause**, at the end of the status line, apart from Free time, the same place every time, with its icon and an accessible name; on a phone, also on the quick-settings tile and at a long press on Sioul's icon (`dnd`; they open Sioul on the pause and never end it). It asks nothing. (R)
- It holds everything Sioul controls, on every device (P7–P8); a focus session running stops and counts; today's end of work stays as usual (P7: work never moves into the evening). (R)

### The screen (P11–P17)
- Over Sioul's whole window: Sioul's content only; on a phone the system stays free (calls, other apps, emergency dialling) (P14). (R)
- **The words** (O, P11–P12):
  - "Paused."
  - "Sioul is keeping everything on hold: mail, tasks, messages. Nothing new will show here until you come back. Sioul asks nothing of you until then."
  - « En pause. Sioul garde tout en attente : courrier, tâches, messages. Rien de nouveau ne s’affichera ici avant votre retour. D’ici là, Sioul ne vous demande rien. »
  - With doses coming: "Dose reminders still come." (P10: say what still comes.)
  - The owner's starting text said "reminders" and "Nothing needs doing now" (« Rien ne presse »): "reminders" became "messages" because dose reminders may still come, and the last sentence became a fact Sioul can keep (it asks nothing) rather than a claim about what you have to do; « Rien ne presse » is a set phrase.
  - 26 words in English; with the doses' line, the numbers and the button, under 45. (T: P31)
- **Your list**, folded under "What helps you" until opened; a line that opens something is a button. (P15) (R)
- **Your line**, if written. (P17) (R)
- **The breathing guide**, if switched on: a button; tapped, a slow, low-contrast circle grows over two fifths of a breath and shrinks over three, at your pace, no counting text, no "deep breaths"; a tap stops it (P16). Reduced motion is not read: Qt gives no such setting, so it moves only when tapped. (R; the 2:3 split is G)
- **The numbers** (P13): one quiet line, the emergency number and the crisis line of your country, one tap each (`tel:`); the others (medical emergency, in writing, urgent care) folded under "Other numbers". Never a question first. (R; the crisis line for everyone is T, research open question 9)
- **One button, "Come back"**: no PIN, no question, no hold; Escape is no way out (P14). (R)
- **Nothing moves** unless you start it; no count, no time elapsed, no countdown (P12). (R)

### Coming back (P22–P25)
- "You are back. Nothing was lost." (O, P23)
- **The rest of today** (P23, capacity criterion 36):
  - lighter (unsaid): today's room and heavy steps are no more than a hazy day's, 60 % and one heavy (`pause::today_level`, the `lighter` days in `quiet.toml`); "The rest of today is lighter." when today's work is not over;
  - no more work today: the day closed as "Done for today", without its sheet; "Work waits until tomorrow at 09:00.";
  - as planned.
  The morning's weather is not changed: the evening's review never sees the pause (P25). (R)
- **The work of the paused hours goes to later days** (P7): the day is not extended, a moved end of today is dropped. (R, O)
- **One offer, default no** (P23): "Tomorrow can be lighter too." with **Lighten tomorrow**, shown when tomorrow is a working day; taken, tomorrow is held as today is (it takes effect when tomorrow comes: looking ahead from today, the plan still shows tomorrow's usual room). (R)
- **Held things come back slowly** (P24): the Porch rests until your next admin hours (else your next working hours, else tomorrow morning), saying when it opens; "Open anyway" opens it. Codes you asked for still show. (R)
- **Undo**, ten seconds in the status line: as if the pause had not been pressed (pressed on this device in this run: as before the press; else the pause's stamps taken away, its return undone). An accidental press costs nothing. (G)
- **Doses held during the pause** (the setup's choice) are reminded once on coming back, as doses held in sleep are reminded at waking (`hours::woke_from`). (R: P7, health.md)
- No reflection prompt, no rating, no count; the evening review does not mention it (P25). (R)

### What is kept (P26)
- The last pause's two stamps, for the undo and its own reference, and the days held lighter; no history, no count. **Forget the last pause** takes the stamps away. (R; keeping the stamps at all is research open question 6: T)

## Telling them apart (GP18–GP20)
- Different names, places, icons and looks: Free time a switch with a flower, lit while on; Pause a button with the pause sign, apart, at the very end; the pause covers the window, Free time does not. (R)
- The status line says which is on: "Free time: …" or "Paused.". (R)
- From Free time, **Keep my usual end** turns it into a pause that moves nothing, so Free time pressed at a bad moment never costs an evening (GP19). (R)
- One rule for who gets through: both use the same lists and quiet rules, the pause narrowing them (GP20). (R)

## The status line's end
- Free time and Pause have a place of their own at the line's end, never pushed out: what the line holds is cut before them. On a phone they are their icons alone, each with its accessible name; both fit 412 pixels. (O)
- Free time's menu: a right click, or a long press on a touch screen: **Come back from free time**, **Keep my usual end (17:00)**, **Nothing at all**. The status line's own menu has the same items during Free time ("Work now" and "work a while" are not offered then).

## Do-not-disturb
The pauses hold Sioul's own notifications on every device (above). Each device also puts its system's do-not-disturb on for itself, where the system lets an application do it, with the exceptions set in the pause's settings; the pause's line says what was silenced and what could not be (`crates/sioul-app/src/dnd.rs`: `can`, `enter`, `leave`). While a pause lasts, only what Sioul turned on is turned off at its end; Sioul's do-not-disturb going off altogether turns the system's own off too, where Sioul can, and a system's own do-not-disturb turned on or off presses Sioul's switch ([do-not-disturb.md](do-not-disturb.md#both-ways)). Asked again unchanged, nothing is redone; what Sioul turned on is kept in its state folder (`dnd.toml`), so that the next start puts right what a crash left.

| System | What Sioul does | What still gets through |
|---|---|---|
| Android 10 and later | a mode of its own per pause, "Pause" and "Free time"; needs "Do Not Disturb access" ([android.md](android.md#pauses)) | starred contacts and repeat callers, or nobody; alarms; what you play; dose reminders, on a channel of their own |
| Plasma | its notification server's inhibition, "While Sioul is active (Pause)" | critical notifications, Sioul's among them, unless Plasma is set to hide them |
| GNOME | your own Do Not Disturb, switched on only with your yes in the pause's settings, and off after | critical notifications, Sioul's among them |
| macOS | your shortcuts "Sioul pause on" and "Sioul pause off" | what your Focus lets through |
| Windows, other Linux desktops | nothing; the line says so | everything but Sioul's own |

- **Plasma**: `Inhibit` and `UnInhibit`, Plasma's own addition to `org.freedesktop.Notifications`, found by its capability "inhibitions" (`crates/sioul-sync/src/dnd.rs`). An inhibition belongs to the connection that asked for it: a thread keeps that connection open while the pause lasts, so Sioul quitting or crashing ends it (the desktop is never left silenced), and the next start, the pause still on, asks again. Plasma's notification server started again is asked again; an inhibition you end from the notifications applet stays ended.
- **Sioul's doses on Plasma**: Sioul sends every notification at critical urgency (`crates/sioul-sync/src/notify.rs`), and Plasma shows critical ones during do-not-disturb unless its setting "Critical notifications: Show in do not disturb mode" is off (`[Notifications] CriticalInDndMode=false` in `plasmanotifyrc`). Sioul reads it, and Sioul's own "Show in do not disturb mode" among Plasma's settings for applications (`[Applications][com.aurelienpierre.Sioul] ShowPopupsInDndMode`); when neither lets the doses through, it says so, with a button to Plasma's notification settings at Sioul's page. It never writes them. Every notification names Sioul's desktop file, so that Plasma and GNOME list Sioul among their applications.
- **GNOME** has no do-not-disturb for applications: its switch is yours, the setting `org.gnome.desktop.notifications show-banners`. Sioul switches it, with `gsettings`, only with your yes in the pause's settings ("Switch GNOME's Do Not Disturb on with Sioul's, and off after", off until you tick it; one yes for both pauses, `[pause] gnome`), only from off (on already, it is yours and stays on), and switches it back as the last pause ends, only if nobody changed it meanwhile (dconf tells each change on the session bus). Why a yes rather than never: the press asks nothing (P1), and someone overwhelmed cannot be asked for a click in the top bar; the yes is given on a calm day, the switch shows in the top bar, and it is put back exactly. Never from a Flatpak: its `gsettings` would write the sandbox's own file, which silences nothing. Critical notifications, Sioul's doses among them, show during GNOME's Do Not Disturb.
- **macOS** lets no application turn a Focus on (`INFocusStatusCenter` only reads it, with your leave). Shortcuts can: make "Sioul pause on" (turning a Focus on, Do Not Disturb for one) and "Sioul pause off" (turning it off) in the Shortcuts app, and Sioul runs the first as the first pause starts and the second as the last ends (`shortcuts run`). Sioul cannot see whether a Focus was on before: the second does what you made it do.
- **Windows**: turning on do-not-disturb or a Focus session is a Limited Access Feature, which Microsoft unlocks only for applications it approves; the line says so, and where the switch is (Windows key + N).
- **Other desktops** (dunst, mako, XFCE…): the freedesktop specification has no do-not-disturb; the line names the notification server and says Sioul cannot silence it.
- **Doses during a pause**: on a computer, the critical urgency of all of Sioul's notifications lets dose reminders through the desktop's do-not-disturb, and the pauses hold every other one: nothing else needed changing. On Android, a channel of their own (above).
- **When it is applied**: at the press and the return, at each minute of the window (after the sharing's exchange: a pause pressed elsewhere), at the start and when the window comes back on a phone; Free time's mode is off while paused, the pause's own holding (`pauses::apply`, through the one shared do-not-disturb state, `everywhere::apply`, which turns the system's on and off for every reason). The phone's mode lets starred contacts through in Free time unless "Nothing at all"; the pause's, as its setup says.
- **On every device**: the two pauses are reasons of the one shared do-not-disturb ([do-not-disturb.md](do-not-disturb.md)), beside sleep, focus and its switch, which apply it through the same layer. Its switch pressed off during a pause lifts only the system's do-not-disturb: the pause still holds Sioul's own notifications.

## The numbers (P13)
`crates/sioul-core/data/crisis-lines.toml`: for each country, each number with its kind (emergency, crisis, medical, in writing, urgent care; and the number emergency services call back from, never shown to call), how to reach it, who it is for, the page it was checked on and the day. Checked on 2026-10-06 at the services' own pages: France 112, 3114, 15, 114 (by text) and the callback 0 800 112 112; the European Union 112; the United Kingdom 999, Samaritans 116 123, Shout 85258 (text SHOUT), NHS 111; the United States 911, 988; Canada 911, 988. A country not in the file shows Europe's 112. `cargo test -p sioul-core crisis_lines` checks the file is well formed; the numbers themselves must be checked again before each release, and `checked` changed (P13, 8.4: 9 % of apps gave a wrong number).

## Not built
- **Sending anything** (O): no message, call or SMS; no location. P3, P4, P18–P21.
- **A pause ending at a clock time set beforehand** (P22's second half): the pause ends when you come back.
- **Professional help suggested after repeated pauses** (P27): it needs a count of pauses, which Sioul does not keep (P26).
- **The pause holding your alarms**: alarms always ring (the do-not-disturb modes let alarms through, and Sioul's alarm at waking rings as set); silencing them was judged costlier than hearing one.
- **Reduced motion** for the breathing guide (Qt gives no setting): it moves only when tapped.
- **A weather line for Free time** (GP2, open question 11).
- **A planned free time** ("if it is dry tomorrow at 14:00", open question 14).
- **Asking whether mornings or evenings suit you** for the latest end (GP10b, 3.3): one setting for everyone.
- **The texts tried with the people concerned** (P31), before release.

## To check over time (GP, in the manner of capacity.md §12, on your own record, never shown)
- How often the moved end reaches its limit, and how often "Keep my usual end" is chosen: if the limit is often reached, the default moves too much.
- Days said "too much" and nights said "heavy" after days whose end moved.

## Tested
`cargo test --release -p sioul-core pause` (the pause above everything, doses still allowed; Free time as leisure with the safe list only; the moved end and each limit; what does not fit going to later days, budgets unchanged; "Keep my usual end"; the pause moving work to later days, never into the evening; the bad-day level coming back; the state's round trip through the sharing's file; the crisis lines' file), `quiet` (movement not offered in Free time). Seen in the window on the demo profile, desktop and phone, English and French (`SIOUL_GRAB_STEPS=pauses`).
