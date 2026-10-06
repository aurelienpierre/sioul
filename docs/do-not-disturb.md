# Do-not-disturb on every device

One do-not-disturb for all your devices: turned on or off on any of them, it holds on all, each silencing its own system as far as the system lets an application do it; one list of the people who may reach you then, kept in the sharing itself; and the phone kept in step in the background. Code: `crates/sioul-core/src/everywhere.rs` (the rules), `crates/sioul-app/src/everywhere.rs` (applying and saying it), `crates/sioul-app/src/steps.rs` and `android/package/src/com/aurelienpierre/sioul/StepService.java` (the phone in the background); the systems' own do-not-disturb is the layer of [pauses.md](pauses.md#do-not-disturb) (`crates/sioul-app/src/dnd.rs`).

The owner's words: "a global, cross-device do-not-disturb mode that communicates through the shared vaults between all instances of the app, and allows for example to turn the phone's DnD on or off from the desktop when in a task, with unified contacts management for the people allowed to go through the DnD mode across devices […] handled through phone calls, SMS and email."

## Why it holds
Do-not-disturb holds while one of its reasons does. Each reason is read from the one place that already says it, and that place is shared as it is: every device reads the same files and comes to the same answer, and nothing is said twice.

| Reason | Read from (part of the sharing) | Setting (`[dnd]`) | Ends |
|---|---|---|---|
| the pause | `quiet.toml`, `paused_since`/`paused_ended` (time) | `pauses`, on | when you come back |
| Free time | `quiet.toml`, `free_since`/`free_ended` (time); never with the pause | `pauses`, on | when you come back, at the night's start |
| sleep | Health's night and naps (health), on each device's clock | `sleep`, off | at waking |
| focus | `time/running.toml` (time): a session counting, not paused | `focus`, off | 30 minutes past the time chosen, three hours without one |
| the switch | `state/do-not-disturb.toml` (settings) | `button`, on: the switch shows | when you turn it off, or at the end you chose |

- **The latest change wins**, the switch against the reasons too: the switch pressed off after a reason began holds that reason off until it ends; a reason that begins after the press holds again (the next focus session, the next night). A press "on" never holds a reason off. Pressed off during a pause, the switch lifts only the system's do-not-disturb: the pause still holds Sioul's own notifications.
- **A focus session forgotten** never keeps you unreachable for long: it holds do-not-disturb until 30 minutes past the time chosen, three hours for a session without one (`FOCUS_GRACE`, `FOCUS_OPEN`).
- **Said first**: the pause, Free time, sleep, focus, the switch. The end said is that of every reason that holds, when each has one; a focus session's limit is never said (it is no promise).
- **Its modes**: the pause and Free time keep their own, with their own exceptions ([pauses.md](pauses.md)); sleep, focus and the switch share a third, "Do not disturb (Sioul)" (`dnd::Which::Global`): the people on your list (or nobody, `[dnd] people`), repeat callers, alarms and dose reminders get through.

## The switch, shared
`state/do-not-disturb.toml`, in the settings part: the one part every device shares, where the triggers' own settings travel (`[dnd]` in `config.toml`). One table per device, written by that device alone and shared whole (`whole = ["device.*"]`): two devices never fight over one entry in the sharing's later-wins, and the merge is Sioul's own, in code.

```toml
[device."<the device's id in the sharing>"]
kind = "phone"            # or "computer"
name = ""                 # a computer's host name
pressed = 1759760000000   # its last press of the switch (milliseconds, a hybrid clock)
on = true                 # what that press asked
until = 1759765400000     # the end chosen with it; 0: until turned off
seen = 1759754000000      # the latest press of another device known here then
why = "manual"            # what holds here now, as this device reads the reasons
silenced = true           # its system's do-not-disturb is on for it
line = "…"                # its system's answer, in its words
at = 1759760000000        # when this table was written
```

- **Latest wins, whatever the clocks**: a press is stamped after every press already known on that device (a hybrid clock: the later of now and the latest stamp seen, plus one), so a press made after seeing another comes after it even when the clocks disagree.
- **A margin for clocks that disagree**: two presses on two devices that did not know of each other (`seen` before the other's stamp), less than a minute apart (`MARGIN_MS`): "on" wins, the quieter choice; one more press, now seeing the other, settles it. Further apart, the later stamp wins.
- **The end** passes on each device by its own clock: no record needed.
- **Never written over a file that does not read** (half written by hand, damaged): the press fails and says why; the other devices' tables are never written from here.

## Applying it
`everywhere::apply()` is the one caller of the do-not-disturb layer: for each of Sioul's three modes, on when its reason holds, off otherwise; the layer does nothing when nothing changed. Called at the window's start and each minute (the pauses' tick), at once on a press, and after an exchange that wrote the switch, `quiet.toml`, `running.toml` or the settings. Then this device's table says what holds here and whether its system is silenced, the phone's card is told (`homecard::dnd_seen`), and on a phone an alarm comes back at the next end (below).

- **On Android**, only in Sioul's own process, where the layer's modes are kept (`PauseMode`'s memory): the background service, in a process of its own, asks it through `DndReceiver`; so does the alarm of the next end (the switch's end, waking, the next night when sleep counts, Free time's end, a focus session's limit), set exact through Doze after each apply, Sioul closed or not (`steps::next`).
- **Where it holds**, in the status line beside the sounds (`qml/DndApplet.qml`): "Do not disturb, on every device, until 15:00." when every other device heard from in the last seven days (`LIVE_DAYS`) holds it with its system silenced, and this one too; "here only" when none of the others does; "not on every device" in between (a device counts only by a table it wrote after do-not-disturb began, a minute's margin for clocks that disagree: one silent since says "not yet", never what held there before); "on your other devices; this one keeps only Sioul's own notifications back" when this device's system cannot be silenced (Windows, a desktop without a way, a phone without the access). The menu (a right click, a long press) lists each device in its own words: "On your phone: silenced.", "On laptop: This computer's notifications cannot be silenced by Sioul: Windows…", "On another device: not yet." No colour but the accent while it is on.
- **The switch's menu**: for 30 minutes, an hour, two hours; until what now is for ends (the end of working hours, say); until you turn it off; off, on every device; its settings.

## Who may reach you
`config/dnd-people.toml`, in the senders part, beside your lists of who may write to you: the people, each with a name, numbers and addresses, kept in the sharing itself, since the address books of two devices may not be in step. One entry per person (`[[person]]`, keyed by its `id`): two devices adding two people keep both; one changed on two devices keeps the later.

- **Edited in Settings ▸ Do not disturb** (`qml/DndSetup.qml`), with the triggers it goes with: **Add your safe senders** (each address on the Safe list, with its card's name and numbers when a card has it; everyone whose card is in a category on it; patterns such as `@example.org` name nobody, said and left out), **Add a contact…** (from your address books), **Add someone by hand**, **Change**, **Take off the list**. Someone added again with one of their addresses, numbers or their card is merged into the one already there. A person without a number, or without an address, is said so under their name.
- **Mail**: while the switch or a focus session holds, Sioul's new-mail notifications come only for senders on the list (`everywhere::mail_gate`, read by `mailnote`); the others wait, and are told when it ends ("The Porch opens…"). Sleep and the pauses keep their own rules, under which nothing is told. Nothing is hidden from the Porch itself: do-not-disturb is about being interrupted.
- **Sioul's own other notifications**: your sites' (each site's, and their gathered notification) and the suggestions to move wait while the switch or a focus session holds, as in Free time (`hours::may_notify`); dose reminders, the codes you asked a site for, your events' reminders and Health's notices of a meal, a nap or the night come (`hours::may_notify_need`: a meal forgotten during a long focus costs more than a quiet notice), silenced by the system's do-not-disturb where it holds.
- **On a computer**, no calls nor SMS: the list's mail comes as Sioul's notifications always do, at critical urgency, which GNOME's Do Not Disturb and Plasma's show. Plasma hides even critical ones when `[Notifications] CriticalInDndMode=false` (the owner's setting), unless Sioul may "Show in do not disturb mode" (`[Applications][com.aurelienpierre.Sioul] ShowPopupsInDndMode`): Settings ▸ Do not disturb reads both, says that the list's mail cannot show then, and offers Plasma's notification settings at Sioul's page. Sioul never writes Plasma's configuration.
- **On a phone, calls and messages**: Android lets through a mode only the contacts starred in your Contacts app, and a second call from the same number within 15 minutes; it has no list of its own (research: emergency-pause.md 7.1). Sioul never writes a star nor a contact (the coordinator's decision of 6 October 2026, pending the owner's: stars are your favourites everywhere, may travel with your contacts' sync, and a crash or an uninstall could leave Sioul's behind). It reads (READ_CONTACTS, asked in that tab with its reason) who on the list is starred on this phone, number by number (`ContactsContract.PhoneLookup`, an interface unchanged since Android 2, the same on 12 to 16), and says "Two people on your list are not starred on this phone", with each one's **Open their contact** (the Contacts app, where the star is) or, when no contact has their number, **Add to contacts** (the Contacts app's own form, filled; you save it). Repeat callers stay let through: emergency services may call back from a number you do not know.

## The phone in the background
"Sioul keeps your devices in step": a foreground service in a process of its own, with one quiet notification, follows your other devices while Sioul is closed: do-not-disturb within a few minutes, mail at its times. See [android.md](android.md#in-the-background): how it wakes through Doze, what it costs, what "working" means for it (it never says the phone is in use: its exports only refresh when it last shared).

## Older Sioul
- A Sioul that does not know `state/do-not-disturb.toml` nor `config/dnd-people.toml` keeps their records waiting for a version that knows them (the sharing's `known_part`): it writes neither, takes nothing of them out, and shares the rest. It ignores `[dnd]` in `config.toml` and keeps it as it is.
- It writes no table of its own: the status line says "not yet" for it, honestly, since its system is not silenced.
- Its pauses still silence it, as the pauses always did on each device.

## Tested
- `cargo test --release -p sioul-core everywhere`: the latest press wins whatever device made it; a press after seeing another comes after it whatever the clocks; two presses that did not see each other within a minute turn it on, further apart the later; the end by each device's clock; each trigger a setting, the pause first, Free time never with it; an "off" pressed after a reason began holds it off until it ends; a forgotten focus session lets you be reached again; mail and Sioul's own notifications gated by the switch and focus only; where it holds, in words, English and French; who counts as another device; only this device's table written and a broken file left alone; the list admits its people by address and number, merges, and travels a person at a time, never read empty; the Safe list brings its people with their cards; the settings' defaults; every word in both languages.
- `cargo test --release -p sioul-sync do_not_disturb`: two devices sharing one folder, each device's table travelling whole and never written by the other, the list a person at a time (two added apart both kept, one taken off gone everywhere), and a Sioul that knows neither file writing neither and taking nothing out.
- `dbus-run-session -- cargo test --release -p sioul-app everywhere`, then `… steps`: what each mode asks; who on the list is not starred, in words; the background service's rhythm; mail at its rhythm, never asleep nor paused, nor without accounts or the setting; nothing reaches Java on a computer; no permission to write contacts in the manifest, and no write in `DndContacts.java`. The do-not-disturb layer's own (`… dnd::`) cover the third mode beside the pauses.
- The record's fields a later version adds are kept and written back as they were (`settings_have_their_defaults_and_old_files_read`): this device never sends another version's table or person as changed.

## Not built
- **Sioul starring the list's people itself** (WRITE_CONTACTS, with your yes, its own stars only, taken back when someone leaves the list): held until the owner decides; it would build on the guided version.
- **A device's words in the others' language**: each device writes its system's answer in its own language.
- **Measured on a phone**: estimated so far ([android.md](android.md#in-the-background)).
