# The end of the work day, and of the day

Two moments close a day. **Close the work day** ends work and admin, early or at the usual hour: "Done for today" opens it. **Close the whole day**, before sleep, looks back over the whole day: work, admin and leisure. Each asks how the day felt, in a few taps, every answer optional, and remembers how the day began: the morning's weather. The plan learns from the answers (the day's outcome, [research/capacity-budget.md](research/capacity-budget.md) criteria 13–14, [research/wellbeing-gain.md](research/wellbeing-gain.md) G7). Nothing is counted, scored or compared.

Code: `crates/sioul-core/src/reviews.rs` (the records, the outcome, what the status line offers, the words), `crates/sioul-app/src/reviews.rs` (the sheet's view, the notice, the Health page's line), `qml/DayReview.qml` (the sheet), `qml/DayReviewLine.qml` (under a day on the Health page).

## What is kept
One table per date, in `$XDG_DATA_HOME/sioul/reviews/YYYY-MM.toml`, readable and editable by hand:

```toml
[2026-10-06]
weather = "haze"
weather_at = "2026-10-06T08:40:00+02:00"

[2026-10-06.work]
at = "2026-10-06T17:04:12+02:00"
felt = "heavy"
mix = "about_right"
memo = """
The bank called back.
"""

[2026-10-06.night]
at = "2026-10-06T22:10:00+02:00"
felt = "usual"
```

- **The morning's weather** (`weather`): copied when you say it on the Tasks page or with `sioul tasks weather`, the first said that day; said again within half an hour, a correction. A different weather said later is `later`. Never said, none: the plan's default "clear" is not something you said.
- **Each review** (`work`, `night`): when it was given, how the day felt (`light`, `usual`, `heavy`, `gave_back`), its mix (`too_much`, `about_right`, `too_empty`), a note in Markdown. Each answer is optional; a review with none says the work day was closed without a word.
- **The day lived**: a night review given after midnight belongs to the evening before (the night's own rule, as on the Health page); without a night set, before 05:00 is the day before.
- **Read leniently**: a time written as Unix seconds or as a TOML date-time, a word a newer Sioul knows: read, never an error. Writing changes one date's table and keeps the rest of the file as written, comments and unknown fields included; a file that does not read is never written over.

**The outcome** (`Day::outcome`): the night's mix if given, else the work's. For the cost budgets, `too_much` is a bad day, `about_right` and `too_empty` went fine; for the gain's minimum, only `about_right` is a good day. A day without a mix counts for nothing. The day's balance (`crate::capacity::day_balance`) is the capacity's.

**Shared** with the sharing's "time" part (`data/reviews/`), beside the plan's other files: the weather (`today.toml`), "Done for today" (`quiet.toml`), the sessions. The outcome belongs with the plan it judges, so every device that plans learns the same. Each review travels whole: two devices answering the same one keep the later answer, never a mix of both ([database.md](database.md)).

## At any hour, from the places
- Two buttons at the bottom of the places (`qml/Places.qml`; on a phone, in their drawer), one under the other: **Close the work day** (a ticked list) and **Close the whole day** (a moon). Each opens its review at any hour, a day closed early as well as late; its tip says what closing does. The status line offers each only at its time (below).
- **The same two in the tray's menu**, on a computer: the menu of Sioul's icon in the system tray (on a Mac, in the menu bar; `qml-desktop/Tray.qml`) has **Close the work day** and **Close the whole day** between showing the window and quitting, each opening its sheet in the window, brought back if it was hidden there (`main.qml`'s `reviewFromTray`).

## Closing the work day
- **"Done for today"** (the Tasks page, the focus window) opens the sheet. **Close the work day** keeps the answers and closes the day as "Done for today" always did: today's room goes, the plan flows on, a session running stops and counts, "Undo" waits in the status line ([tasks.md](tasks.md), rule 11). The screen after it says where everything went; its first line is now "Work is put away until tomorrow at 09:00. The rest of the day is yours." **Not now** changes nothing, and the answers begun stay in the sheet for later that day.
- **At the end of the day's last work or admin hours**, the status line offers **Close the work day**, with "Work hours are over for today." when there is room for it, and one quiet notice comes, "Work hours are over", with the same button where the system carries one ([below](#the-notices-on-each-system)): once, within ten minutes of the hours' end, from the device you are at (the one that keeps notices, `sioul_sync::lease`). Never during sleep or a meeting, never once the day was closed or its review given, never repeated.
- **A day that ends without the button** closes as usual. The status line offers the review until the evening; from the evening on, closing the day covers it.
- **The sheet**, made the first time it is asked for: what was said of the morning and what the plan did with it ("This morning: haze. The plan kept a lighter day."; "This morning: clear; later, fog. The plan lightened the rest of the day."); "The day felt:" Light, Usual, Heavy, Gave back; "The mix:" Too much, About right, Too empty; a note, with a preview as Notes shows Markdown; the notes of the last fourteen days, folded. Nothing about what was not done: planned and done are never compared.

## Closing the day
- **When**: the night's own notice, as the wind-down begins, has **Close the whole day** beside **Options…** while that night's review is not given, **Options…** alone once it is, as each system carries buttons ([below](#the-notices-on-each-system)). In the evening, from three hours before the wind-down, the status line offers it ("Before sleep, the day can be closed."); without a night set, from 20:00 to 02:00. On the Health page, under the day: **Close the whole day** in the evening and during the night, and the words said for each day before. During sleep, from the wind-down, the status line offers nothing.
- **The sheet** is the same, over the whole day: the morning's weather, the end of work as said then ("At the end of work: heavy; the mix: too much."), tomorrow's first step when one was named ("Tomorrow starts with: open the form (15 min)"), then what the day asked and gave, in words only: "Thinking: heavy for you.", "Some time gave back.", and the plan's own line when the day changes tomorrow's. A scale with nothing known that day says nothing. Then the same three questions. **Close the whole day** keeps them; the status line says "The day is closed."

## The notices on each system
Both notices are decided once, for every system: the end of work's by `reviews::work_notice`, the night's by `health::needs_now`; how each is shown is the system's (`reviews::show_notice`).
- **Linux and the BSDs**: the desktop's own notifications (freedesktop, over D-Bus: `sioul_sync::notify`), with their buttons. A press opens Sioul on the sheet, or on the night's question for **Options…**.
- **macOS**: the same buttons, through the notifications Sioul already sends there (notify-rust's NSUserNotification, which Apple deprecated in macOS 11 and still shows): one button as the notice's own, two as "Options" and its list; a click on the notice is its first button. Each notice waits for its answer on a thread of its own, until it is pressed or leaves the Notification Center (`notify::remind_offering`). Not UNUserNotificationCenter: notify-rust's support for it is a preview, and turning it on would move every notice of Sioul's there, the doses' too, untried on a Mac. In a run where Qt shows its own notice once ("Sioul goes on", the first time the window hides in the tray), Qt takes the presses of every notice of that run: a press then only brings the window back.
- **Windows**: no buttons. The two notices come as messages of Sioul's icon in the tray (Qt's `showMessage`), and a click on one opens what its first button offers, within six hours (`main.qml`'s `onTrayNotice`, `trayMessageClicked`); without a tray that shows messages, the system's own notice says the words alone. Chosen over a toast's buttons: Sioul's other notices are WinRT toasts through notify-rust, which, with no AppUserModelID registered by the installer and the zip package, are shown as Windows PowerShell's; and their buttons reach Sioul only through the toast's own event, while its pop-up shows, a press from the action centre needing a COM activator; none of it could be run here. Windows may play its notification sound for these messages: Settings ▸ System ▸ Notifications ▸ Sioul turns it off.
- **Android**: Android's notifications, in the "End of the day" channel (« Fin de journée »), at low importance: no sound, no banner, yours to change; the buttons as on Linux; a press opens Sioul on the sheet or the question (`ReminderOpener`, as for an event), the notice taken away. Android freezes Sioul when it is not shown, so the times are given ahead to Android's alarm clock with the events' reminders ([android.md](android.md#events): `day:work:<date>` at the hours' end, `day:night:<date>` as the wind-down begins, `reviews::coming_work`, `health::coming_night`). At its alarm Rust decides as a computer's minute does (`reviews::decide_work`, `health::decide_night`), reading who keeps the notices without claiming it from there (`share::looked`), and asks again each minute while the end of work's ten minutes last; it shows the notice through `DayNotes.java`, or nothing: no words are kept to say in its place, as an event's are. The notice leaves the shade six hours after it came (the night's at waking at the latest), when a press would close another day. Of Health's notices, only the night's own comes on a phone. Not tried on a phone yet.

## Why, and what is guessed
- **One outcome a day, cheap to give** (CB14; G7's "too much duty / about right / too empty"): without it the plan learns habit, not capacity (CB13). "The day felt" reuses a task's own words, so a heavy day and a heavy task mean the same.
- **Closing work is a boundary to mark.** Detachment in the evening goes with less exhaustion, and the exhausted detach least (Sonnentag & Fritz 2007, 2015; Wendsche & Lohmann-Haislah 2017); a plan for what remains raises evening detachment (Smit 2016) and helps sleep come (Scullin et al. 2018), hence the first step named on both sheets; an end-of-day dialogue helped people without a ritual of their own (Williams et al. 2018, small). That the ritual itself calms is unproven: the flagship study was retracted ([research/done-for-the-day.md](research/done-for-the-day.md), section 7).
- **The sheet comes before the closing**, by choice: the few words are the ritual. The research on the button said "one click, no question" (done-for-the-day.md, recommendation 2). What keeps it close: **Close the work day** is the default and needs no answer, so a bad day costs one tap more; nothing asks whether to stop; the questions are about the day, not about you; **Not now** goes without a word. Emotional check-ins before committing were disliked by adults with ADHD (Chen, Meng & Nie 2026); this one comes after, and asks nothing.
- **Words only** (CB45–49, G20): no score, percentage, gauge, red or fuel tank; the reading back is a few days in their own words, no streak, count or calendar of moods (Silverman & Barasch 2023; Ancker et al. 2015).
- **The notices**: once, quiet, never during sleep or a meeting, as the meal notices; gain blocks will be free of notifications (G18b).
- **Guesses**: the half hour that makes a second weather a correction; the evening's three hours before the wind-down, and 20:00 to 02:00 without a night; 05:00 as the turn of the day without a night; ten minutes for the notice; fourteen days read back.

## Tested
`cargo test -p sioul-core reviews` (a review kept and read back, both reviews of one day, older and hand-written files, a day with only its weather, what the status line offers when, the words in English and French); `cargo test -p sioul-sync share::tests::the_days_reviews_travel_whole`; `cargo test -p sioul-app reviews` (the balance in words, never a number); `cargo test -p sioul-app eventalarms` (what a tapped notice may open; the two notices' words as `android/jvm-checks/DayNotesCheck.java` reads them, compared with that file); `cargo test -p sioul-sync notify` (the Linux notices' buttons as before; a press on macOS read as its button, a click as the first); `tools/qml-test.sh tst_tray.qml` (the tray's two entries, a click on its message); `android/jvm-checks/run.sh` (`DayNotesCheck`: each notice's buttons and what each opens, the presses kept apart). Seen on the demo profile at desktop and phone sizes, in English and French (`SIOUL_GRAB_STEPS=review`).

## Not built
- A next-morning "rested / not" (CB14's second signal).
- Answering by the command line or the AI agent.
- Buttons on Windows' notices: an AppUserModelID set up by the installer and the zip package, and a COM activator, so that a press from the action centre reaches Sioul.
- macOS's newer notifications (UNUserNotificationCenter), once notify-rust's support is more than a preview.
