# What reaches you, and when: one model

Sioul spends one thing four ways, your attention: what it **shows** (the Porch, the pages), what it **tells** (its own notifications), what it **holds** of the system's (other apps' notifications, calls), and what it **silences** (the system's do-not-disturb). One model decides all four: a matrix of who or what × when → a level, which every caller reads through one pipeline. The code is `sioul_core::attention` (crates/sioul-core/src/attention.rs, [§9](#9-the-code)); the window says it in words in `reaches.rs` and draws it in Settings ▸ **What reaches you** ([§8](#8-the-interface)). The guide's page for the person is [What reaches you, and when](https://aurelienpierre.github.io/sioul/guide/notifications.html).

**Why one model.** Until 7 October 2026 the rules lived in eight places: who × when (`reach`), what × when (`notify`), the time now (`quiet`, `pause`), do-not-disturb (`everywhere`), the Porch's lanes (`porch`), other apps (`appnotes`), calls (`calls`, `Calls.java`) and sites. Each was right on its own; together they overlapped, and some gave three answers for one person at one time: a safe friend's chat came at 03:00, their mail waited for waking, their call was declined. The owner asked to "flatten all possible timeframes × actions (contacts/lists, internal notifications, external notifications through "don't disturb", screened calls) into an unified decision matrix", in the docs and in the interface, before the implementation was redone. This page is the model as built; the questions it settled are in [§10](#10-what-was-decided). The design as it was first proposed, with every rule of the older code and its line at commit `2e9bbf1`, is in this page's history.

## 1. The objects

### 1.1 Sources
A source is where something comes from. **Sioul's own**: Sioul fetches, keeps or computes it, and decides everything about it. **The system's**: another program owns it; Sioul can only hold it, let it through, or silence the system.

| Source | Whose | What Sioul does | Code |
|---|---|---|---|
| Your mail addresses | Sioul's | judges, sorts, shows, tells | `porch`, `mailnote` |
| Your sites, on a computer | Sioul's (its browser) | catches their notifications | app `sites` |
| Agenda, tasks, budgets, papers, contracts, the bank | Sioul's | reminds | `reminders` |
| Health: doses, meals, naps, the night, moving, the watch, the alarm at waking | Sioul's | reminds, rings | app `health`, `wake` |
| The focus timer, the end of the day | Sioul's | shows, offers | `timenote`, app `reviews` |
| Phone calls (Android 10 and later) | the system's | lets ring, or declines | `calls`, `Calls.java` |
| Other apps' notifications (Android) | the system's | holds, or lets through | `appnotes`, `AppNotes.java` |
| The system's do-not-disturb | the system's | turns Sioul's own modes on and off | app `dnd`, `everywhere`, `PauseMode.java` |
| Your presses | yours | Pause, Free time, the do-not-disturb switch, Let every call through, Work now, Real time | `quiet.toml`, `do-not-disturb.toml` |

On a computer Sioul sees no other app and no call: the system's do-not-disturb is its only hold on them.

### 1.2 Who: the people rows
Three **channels**: mail, calls, and messages from other apps (a phone: texts, chats, a mail app, a missed call). Each has a row per who, in this order (`attention::persons`):

- **Always through** (« Passent toujours »): the people on that list (`config/dnd-people.toml`, the same on every device) and a conversation set so. Its cells say how much more than their own list they get: `●` at once whatever their list (drawn ★), `◑` shown, not told, `=` as their own list, and on a layer `☆`, the layer does not hold them.
- **Safe**, **Neutral**, **Restricted**, **Strangers**: the states of `reach::Who`, the blocked aside. Only you put someone on Safe; anyone in your address books, or let in from the screener, is neutral; strangers are in none and on no list.
- **Hidden numbers** (calls only): a call that shows no number, someone who hides it or a hospital's switchboard.
- **Groups** (messages only): a conversation where several people write, as strangers until you change their row.
- **Blocked**: never, on any channel; a fixed row.

**From the person to the row** (`porch::Senders::judge`, `judge_number`): the address's or the number's own entry; its card's (`contact:<UID>`); the card's categories; the most precise pattern or prefix; then neutral when let in or in an address book; else a stranger. At one level, blocked before restricted before neutral before safe. **Proof comes first**: mail that fails authentication is weighed as a stranger's (a blocked address stays blocked); forged mail and borrowed names are set aside. A name alone (another app's chat) counts only when it may hold more (`appnotes::named`); a contact of the phone alone is neutral.

### 1.3 What: Sioul's own and the automated sources
One row per kind, with no who (`attention::Kind`), in five groups:

| Group | Rows (their id) |
|---|---|
| What you set or asked for | codes and links you asked for (`codes`), doses (`doses`), the alarm at waking (`wake`), an event's alarms (`alarms`) |
| Reminders | Sioul's reminder before an event (`before`), events the working day before (`day-before`), dates, waits, payments, papers (`dates`) |
| Your day | meals, naps and the night (`needs`), the pause to move (`move`), "Work hours are over" (`work-over`), the time running (`time`), your watch's offers (`watch`) |
| Sites, on a computer | sites gathered (`sites`), a site in real time (`sites-live`), a call in a site (`site-calls`) |
| Other apps, on a phone | automatons (`app-automatons`), apps and browser sites set to At once (`app-at-once`) |

Thirty-seven rows in all (`Row::ALL`), with ids such as `mail.safe`, `calls.hidden`, `messages.groups`, `codes`.

### 1.4 When: seven times, two layers
One **time** holds at any moment (`quiet::mode`, read by `Now::of`), the first that applies:

1. **Pause** (« En pause »): pressed, until you come back.
2. **Sleep**: from winding down to waking, and each nap.
3. **Free time** (« Temps libre »): pressed, until you come back, the night or midnight.
4. **Meals**: from getting one ready to its end.
5. **The hours**: **Work**, **Admin** and **Leisure** as your week says; work and admin together when both are open, or when no hours are set at all.

Two **layers** hold on top of the time, never instead of it:

- **Time for you**: a slot of today's plan kept for you. The window that lays out the day writes today's slots to `state/slots.toml` (`attention::Slots`, only when they changed); every process reads them: the window, the listener of other apps, a phone's background step, `sioul remind --watch`, `sioul watch`, the calls' table.
- **Do not disturb**, from its switch or a focus session. Do-not-disturb held for a pause, Free time or sleep is no layer: their own columns apply.

Two **switch layers** have one cell each, outside the grid: **Let every call through** (every call rings but the blocked's) and **Real time** (the Porch's box: mail each minute, every site in real time).

**Reading a moment** (`Attention::level`, `Attention::person`): the least strict of the time's columns, then the stricter of that and each layer that holds. For the people's rows, a week without admin hours lends work's cell to admin time and back; the kinds are never lent. Free time's **Nothing at all** holds every state's row, never Always through.

### 1.5 Levels
One scale for every row, from the least strict to the strictest (`attention::Level`):

| Mark | Level | What it means |
|---|---|---|
| ● | **At once** | It comes now: shown, told, let through, rings. On a layer: the layer changes nothing. |
| ◑ | **Shown, not told** | It is in Sioul now if you look (the Porch); nothing tells you. It is told when a time comes in which it is told. A phone cannot show what it holds: for calls and messages it reads as ○, and is not offered. |
| ◐ | **When its event falls then** | An event's own (its alarms, Sioul's reminder before it): at once when the event begins within this time; else later. Not offered where the time's end is not known (a pause, the layers). |
| ◎ | **At the gathered times** | Told, or let through, at the next gathered time (`[reminders] gathered`). |
| ○ | **Later** | It waits out of sight, and comes when a time it may come in begins: mail off the Porch, a message held, a call sent to voicemail and listed then, a reminder told if it still makes sense. |
| – | **Not at all** | Never told. What Sioul keeps stays in its place (a code on the Porch, mail in its lane, the meal on Health); a message or a call is never let through. |
| = | **As their list** | Always through's row only: as the person's own row says. |
| ☆ | **Through, at their own times** | Always through's row, on a layer only: the layer does not hold them; their own row's times do. |

Each cell offers some levels only (`attention::choices`): mail ● ◑ ○ –; calls and messages ● ○; Always through ● ◑ (mail) and =, with ☆ on a layer; each kind its own (codes ● or –, doses ● or ○, sites ◎ or ○…).

### 1.6 Outputs
- **Shown in Sioul**: the Porch (its lanes; above them codes, doses, calls declined (on every device, the phones' logs shared: [porch.md](porch.md), "Calls declined"), sites' news, the line of held apps, letters, payments), Health, the Agenda, the pages, the status line, the phone's home card, the pause's screen. Showing is never a notification.
- **Told**: a notification Sioul sends. On a computer, critical urgency without sound for codes, reminders, doses and notices, and for new mail from someone Always through; normal urgency for other new mail; an ongoing one for the time running. On a phone, a channel each: "Doses", "Doses during a pause", "Waking", "Events", "An event's alarms during a pause", "New mail", "New mail from people always let through", "Focus timer", "Devices in step".
- **Held or let through**: another app's notification is snoozed until a time, never cancelled (`AppNotes.java`); a call is let ring, or declined plainly, the network sending it to voicemail and the Porch listing it later (`Calls.java`).
- **Silenced**: the system's do-not-disturb, through Sioul's three modes ([§6](#6-what-the-system-lets-through)).

## 2. The pipeline
`Attention::decide(&Event, &Now) -> Output` answers for one event. Each caller builds the event, asks, and keeps what is its own: which device tells, the words, the ledgers that tell each thing once. An earlier step wins over a later one:

| Step | What it decides | Answer |
|---|---|---|
| 1. **Source and kind** | what it is: `Source::Own(kind)`, `Mail`, `Call`, `Message`, `Site` | nothing yet |
| 2. **Emergency** (calls) | an emergency number or its callback; the day after you called one | rings, even blocked |
| 3. **Blocked** | the blocked row | never: mail set aside and untold, a call declined and not listed, a message held; blocked beats Always through |
| 4. **Content** (mail) | the lane (`attention::Lane`): a code takes the codes' row; what you sent yourself is shown, never told; set aside, the review queue, hostile, less important accounts, newsletters and mail read elsewhere are shown as their row says, never told | the lane's |
| 5. **Floors** (calls) | Let every call through; a second call within 15 minutes (never a hidden number's, which cannot be told apart) | rings |
| 6. **Exceptions** | Always through (its row, `=` and `☆` read against their own row); a chat site naming someone your cards know follows their Messages row when it holds more | the exception's |
| 7. **The matrix** | the time and its layers, the row | a level |
| 8. **What it is for** | the source's area (an address, an app, a site, a task) against now: what is for work waits in your evening, but your safe senders' and Always through's, and but when the row and the area never meet in your week | later |
| 9. **Holds** | the Porch resting after a pause (mail shown, not told); a meeting (no meal's notice, no "Work hours are over"); the chat limit (chat sites wait) | later, or not at all |
| 10. **Output** | `Output { level, shown, told, pierce, step }`; `pierce` when it is told through the system's do-not-disturb (codes, doses, alarms, Always through) | |

**Which device tells, once**, stays the caller's: the device you use tells mail, sites and the end of work ("notices" lease); doses and Health's notices follow the "health" lease; each device tells its own events.

### Examples
- **A safe friend at 03:00.** Their chat waits for waking (Messages × Safe × Sleep is ○); their mail is on the Porch if you look, told at waking (◑); their call goes to voicemail, and the Porch lists it at waking. On the Always through list: their chat and their call come at once, their mail is ◑.
- **A number both blocked and on the Always through list**, calling at noon: declined. Blocking someone takes them off Always through, and putting them on Always through takes them off the blocked list, said (`porch::set_standing`, `porch::unblock_person`).
- **A code from your bank at 22:40, while you wind down**: told (Codes × Sleep is ●; "On the Porch only" stays a choice there). The same code by text is never held either.
- **A neutral colleague's mail at 19:00 to your work address**: it waits off the Porch until work (Mail × Neutral × Leisure is ○).
- **A group's message during Free time**: held until Free time ends and its row lets it come (Groups × Free time is ○).
- **Under do-not-disturb from its switch**: Sioul's reminder before an event comes (●); mail from someone Always through is told at once, at critical urgency on a computer; a safe sender's who is not on the list is shown, not told (◑); a safe caller goes to voicemail, the list's people ring.

## 3. The usual matrix
What Sioul does unless you change it (`attention::usual`; the test `the_usual_matrix_as_decided` writes it out). W A L M S P F: Work, Admin, Leisure, Meals, Sleep, Pause, Free time; Y: Time for you; D: Do not disturb. **Bold**: fixed.

| Mail | W | A | L | M | S | P | F | Y | D |
|---|---|---|---|---|---|---|---|---|---|
| Always through | ★ | ★ | ★ | ★ | ◑ | ◑ | ★ | ★ | ★ |
| Safe | ● | ● | ● | ● | ◑ | ◑ | ◑ | ◑ | ◑ |
| Neutral | ● | ● | ○ | ○ | ○ | ○ | ○ | ◑ | ◑ |
| Restricted | ● | ○ | ○ | ○ | ○ | ○ | ○ | ◑ | ◑ |
| Strangers | ● | ● | ○ | ○ | ○ | ○ | ○ | ◑ | ◑ |
| Blocked | **–** | **–** | **–** | **–** | **–** | **–** | **–** | **–** | **–** |

| Calls | W | A | L | M | S | P | F | Y | D |
|---|---|---|---|---|---|---|---|---|---|
| Always through | ★ | ★ | ★ | ★ | ★ | ★ | ★ | ★ | ★ |
| Safe | ● | ● | ● | ● | ○ | ○ | ● | ● | ○ |
| Neutral | ● | ● | ○ | ○ | ○ | ○ | ○ | ● | ○ |
| Restricted | ● | ○ | ○ | ○ | ○ | ○ | ○ | ● | ○ |
| Strangers | ○ | ○ | ○ | ○ | ○ | ○ | ○ | ● | ○ |
| Hidden numbers | ● | ● | ○ | ○ | ○ | ○ | ○ | ● | ○ |
| Blocked | **–** | **–** | **–** | **–** | **–** | **–** | **–** | **–** | **–** |

| Messages | W | A | L | M | S | P | F | Y | D |
|---|---|---|---|---|---|---|---|---|---|
| Always through | ★ | ★ | ★ | ★ | ★ | ★ | ★ | ★ | ★ |
| Safe | ● | ● | ● | ● | ○ | ○ | ● | ● | ○ |
| Neutral | ● | ● | ○ | ○ | ○ | ○ | ○ | ● | ○ |
| Restricted | ● | ○ | ○ | ○ | ○ | ○ | ○ | ● | ○ |
| Strangers | ● | ● | ○ | ○ | ○ | ○ | ○ | ● | ○ |
| Groups | ● | ● | ○ | ○ | ○ | ○ | ○ | ● | ○ |
| Blocked | **–** | **–** | **–** | **–** | **–** | **–** | **–** | **–** | **–** |

A mail app's notification on a phone takes the Mail rows (◑ held, since a phone cannot show what it holds); a missed call seen in another app takes the Calls rows.

| Sioul's own | W | A | L | M | S | P | F | Y | D |
|---|---|---|---|---|---|---|---|---|---|
| Codes and links you asked for | **●** | **●** | **●** | **●** | ● | **●** | **●** | **●** | **●** |
| Doses | **●** | **●** | **●** | **●** | ● | ● | **●** | **●** | **●** |
| The alarm at waking | **●** | **●** | **●** | **●** | **●** | **●** | **●** | **●** | **●** |
| An event's alarms | ● | ● | ● | ● | ◐ | **●** | ◐ | ● | **●** |
| Reminders before an event | ● | ● | ● | ● | ◐ | ● | ◐ | ● | ● |
| Events, the working day before | ● | ● | ● | ● | ○ | ○ | ○ | ● | ● |
| Dates, waits, payments, papers | ● | ● | ● | ● | ○ | ○ | ○ | ● | ● |
| Meals, naps and the night | ● | ● | ● | ● | – | – | – | ● | ● |
| The pause to move | ● | ● | ● | ● | – | – | – | – | – |
| Work hours are over | – | ● | ● | ● | – | – | – | ● | ● |
| The time running | ● | ● | ● | ● | ● | ● | ● | ● | ● |
| Your watch's offers | ● | – | – | – | – | – | – | ● | ● |
| Sites, gathered (computer) | ◎ | ◎ | ◎ | ◎ | ○ | ○ | ○ | ○ | ○ |
| A site in real time (computer) | ● | ● | ● | ● | ○ | ○ | ○ | ○ | ○ |
| A call in a site (computer) | ● | ● | ● | ● | ○ | ○ | ○ | ● | ○ |
| Automatons (phone) | ◎ | ◎ | ◎ | ◎ | ○ | ○ | ○ | ○ | ○ |
| Apps and sites set to At once (phone) | ● | ● | ● | ● | ○ | ○ | ○ | ○ | ○ |

**Locks** (`attention::lock`, each with a sentence, `attention-lock-*`): the blocked never; a dose at once but in sleep and a pause, your choice there; a code at once but in sleep, where "On the Porch only" stays a choice; an event's alarms at once in a pause and under do-not-disturb, as both promise; the alarm at waking always. Health's night or nap notice at its own start comes whatever sleep's column says, but in a pause.

**What the cells leave alone**: what a thing is for (step 8) and the named holds (step 9), each said in its time's card; the master switches (new mail told at all, newsletters, sites gathered, each reminder's own, Health's); and each source's own choice (a site's Real time and Silenced, an app's, a conversation's and a browser site's on a phone, an event's Remind, a block's Not today).

## 4. Presets
Each says only how it differs from **As Sioul does now** (`attention::Preset`):

| Preset | Mail | Calls | Messages | Sioul's own |
|---|---|---|---|---|
| **Quieter** (« Plus calme ») | safe: ◑ in leisure and meals | neutral and hidden numbers: ○ in admin | safe: ○ in Free time | dates: ○ in leisure and meals; a site in real time: ○ in leisure and meals |
| **More reachable** (« Plus joignable ») | neutral: ● in leisure and meals; strangers: ◑ in leisure | neutral: ● in leisure and meals; hidden numbers: ● in leisure | neutral: ● in leisure and meals | automatons: ● in work and admin |

Choosing one writes every row as it says (`apply_preset`), never a lock. The tab asks first when your own changes would give way, and says "Yours: 3 changes from As Sioul does now", one press from going back.

## 5. Two answers at once
- Two times open together (work and admin): the less strict cell.
- A time and a layer: the stricter.
- Among your own choices: the most precise wins (a conversation over a person over an app; an address over a card over a category over a domain).
- Blocked and Always through: blocked, on every channel, in the pipeline and in the calls' table; one list takes them off the other, and the person's sheet says so.

## 6. What the system lets through
Silencing the system is no step of the pipeline: it follows the time. The pause, Free time (with `[dnd] pauses`, on unless set), sleep (with `[dnd] sleep`), a focus session (with `[dnd] focus`) and the switch each turn on one of Sioul's modes (`everywhere::now`).

**On Android** a mode lets through what the matrix lets through at its column, as far as Android can say it (`Attention::silence`, sent flat to `PauseMode.java` by app `dnd`): for calls nobody, starred contacts, contacts or anyone (while this phone screens calls, every contact, Sioul having declined the others already; else starred contacts, Android's nearest to Always through; anyone while Let every call through holds); for messages nobody or starred contacts, never wider, though Sioul may hold other apps' notifications: Android sounds a notification before any listener hears of it, so the mode alone keeps a held message silent; a second call within 15 minutes; conversations marked Priority when Always through's messages pass; alarms always; doses and an event's alarms on channels of their own that pass, when their rows say at once.

**On a computer**: Plasma inhibits its pop-ups but critical ones (codes, doses, reminders, mail from Always through); GNOME's switch, with your consent; macOS through two shortcuts; Windows and other desktops cannot be asked ([do-not-disturb.md](do-not-disturb.md)).

## 7. The configuration
`[attention]` in `config.toml`, one key per row, a list of column words: a bare column is ●, `column:level` another level (`now`, `quiet`, `event`, `gathered`, `later`, `never`, `as`, `through`). A row that says the usual is not written; a fixed cell keeps its value; a word not understood, or a value not offered there, is left aside (`Attention::of`).

```toml
[attention]
"mail.neutral" = ["work", "admin", "leisure", "meals:quiet", "sleep:later", "pause:later", "free:later", "slot:quiet", "dnd:quiet"]
"messages.always" = ["work", "admin", "leisure", "meals", "sleep:as", "pause:as", "free", "slot", "dnd:through"]
codes = ["work", "admin", "leisure", "meals", "sleep:never", "pause", "free", "slot", "dnd"]
```

**The older keys.** While no `[attention]` is written, `[reach]`, `[reach.calls]`, `[reach.messages]`, `[notify]`, `[reminders] doses_in_sleep`, `[pause] doses`, `[pause] people` and `[dnd] people` seed the matrix once (`attention::seeded`): read into cells, compared with what the same rules make of an empty configuration, and only what you had changed is kept; everywhere else the usual cells hold, the owner's decisions included. A mail row was the product of two grids: ticked and told now gives ●, ticked and told later ◑, unticked ○. The first row written (Settings, a preset) writes every row that is not usual and takes the older keys out (`attention::OLDER`, `config::set_table`). Nothing is written beside for an older Sioul: alpha, one user.

**Settings' keys**: `attention.<row>` with the row's words whole; the older grids' `notify.<kind>` and `reach.<row>` land there too (`settings::apply`). The tab's switches stay ordinary settings: `reminders.mail`, `reminders.mail_newsletters`, `reminders.gather`, `free_time.nothing`, `dnd.button`, `dnd.focus`, `dnd.pauses`, `dnd.sleep` (section "attention" of `settings::for_view("parameters")`).

**Shared**: `[attention]` travels with the settings to every device ([database.md](database.md)); the Always through list in the senders' part; `state/slots.toml` is each device's own.

## 8. The interface
**Settings ▸ What reaches you** (« Ce qui vous joint »; `ReachesTab.qml`, its words from `reaches.rs`, its data from the window's `reachesView`):

- **On top**, in every view: the moment now in sentences ("Now: leisure, until 22:00. Mail from your safe senders comes at once…"), and **Start from**: As Sioul does now, Quieter, More reachable; "Yours: 3 changes…" with **Back to As Sioul does now**.
- **By time**, the default: a card per time and per layer, now's first, each in sentences grouped by level (what comes at once, at the gathered times, when its event falls then, is shown without a notification, waits until when, does not come), Always through first when it gets more than their own lists; then what the phone or the computer silences then, and the named holds of that time ("After the pause, the Porch rests…"). Free time's card holds **Nothing at all**. **Change…** opens that time's rows, each value in words.
- **By person**: Mail, Calls, Messages, each with a sentence when no phone of yours screens calls or holds messages yet; mail's own switches (new mail told, newsletters) and what the spam filter and an address's area hold; the channel's grid on a computer, its rows on a phone, each opening its nine values; then who is on which list (the four lists, the people placed on a list, your contacts' categories).
- **Sioul's own**: the kinds' grid, the sites' gathering and the gathered times.
- **Exceptions**: Always through (each channel's row in a sentence, then the list); a phone's conversations, apps and browser sites; a computer's sites; an event's Remind; Health's days.
- **Do not disturb**: what turns it on, what holds while it does, what each device does.

A press on a cell opens its choices in words, the current one in bold; a fixed cell says why; a dot marks a cell changed from As Sioul does now. At 412 pixels the whole grid never shows: cards, a time's rows, a row's values; while a time or a row is open, the moment and the presets step aside. The grid is `AttentionGrid.qml`, its marks `LevelMark.qml`.

**Settings ▸ This phone** (« Ce téléphone »; `PhoneSetup.qml`), on a phone: what sets it up, nothing that decides when: call screening, other apps' notification access and holding, Do Not Disturb's access and its modes, who of your Always through people is starred there, keeping in step in the background, exact alarms and Sioul's own notifications.

**A person's sheet** (`PersonSheet.qml`), from a contact's card or a message's sender: "How Camille reaches you": their list and why, the list chosen for them, Always through, and a sentence per channel of what reaches you from them at each time (`reaches::person`). No times of one's own: a person has a list and Always through.

## 9. The code
**`sioul_core::attention`**:
- **The objects**: `Person` (`persons(channel)` the rows of each channel), `Kind`, `Row` (`People(Channel, Person)` or `Own(Kind)`), `Column`, `Level` (`mark`), `usual`, `choices`, `lock`.
- **The matrix**: `Attention` (`usual`, `of(&Config)`, `cell`, `set`, `words`, `changes_from`); `level(Row, &Now)`; `person(Channel, Person, always, &Now)`; `listed(Person, always, &Now)` (a declined call listed after the fact); `mail(..)` (the Porch's reading of a message).
- **The moment**: `Now` (`of(&Mode)`, `of_moment(..)`, `time(Column)`, `layers(slot, dnd)`; `times`, `time`, `week`, `slot`, `dnd`, `nothing`, `span`, `gathering`, `realtime`, `through`, `holds: Holds { porch_rests, meeting, chats }`).
- **The pipeline**: `decide(&Event, &Now) -> Output`; `Event { source, area, starts }` (`own`, `of`, `for_area`, `starting`); `Source`, `Lane`, `Output`, `Step` (emergency, blocked, content, floor, matrix, area, hold).
- **The system**: `silence(&[Column], nothing, Phone { screens }) -> Silence { calls, messages: Senders, repeat, conversations, alarms, doses, events }`.
- **Presets**: `Preset` (`matrix`), `apply_preset`.
- **The configuration**: `Settings` (`[attention]`), `NotifySettings` (the older `[notify]`, read to seed), `seeded`, `apply`, `OLDER`.
- **Time for you**: `Slots`, `dnd_from_files`.
- **Words**: `grid` (columns, marks, rows with their choices, each cell said in a sentence, presets, the changes), `channel_rows`, `kind_rows`, `now_sentence`, `list_choice`, `column_label`, `row_label`, `level_label`; the `attention-*` strings of both `.ftl` files.

**Who asks it**: the Porch's display (`backend::compute`, the phone's card `homecard`); new mail (`mailnote`: now, through, later or never; through at critical urgency on a computer, on its own channel on a phone); codes (`backend::notify_codes`, `sioul watch`); reminders (`reminders::Holds`: an event's own by its span, work's by their area); Health's notices, the pause to move, the watch (`health`); the end of the day (`reviews`); the time running (`timenote`); the pause's screen (`pauses`); a phone's background fetch (`steps`); sites (`sites`: gathered, live, calls, chat sites by who wrote); other apps (`appnotes::decide`); calls (`calls::frames` from the matrix and its layers, Always through as rows of their own, never a blocked number); Sioul's system modes (`everywhere`, `dnd`); `sioul remind`.

**The window's words** (`crates/sioul-app/src/reaches.rs`): `view` (the tab's JSON: the moment now, the presets, the cards, the grid, the exceptions), `set_row`, `preset`, `person`, `person_change`; each sentence built from the matrix, people grouped by what each channel does with them ("mail and messages from everyone", "calls from your safe and neutral senders"), Sioul's own named by their kinds, verbs agreeing in both languages.

**Removed**: `notify.rs`; `reach`'s matrices (`Reach`, `Matrix`, `Times`, `Row`, `Column`, `next_allowed`); `quiet::mail_in_view` and `quiet::list_choice`; `pause::reach_now`; `everywhere::mail_gate`; Accounts ▸ Who may reach you; Settings ▸ Reminders and notifications ▸ When each comes; Settings ▸ Do not disturb's tab, its list now Always through; Settings ▸ Calls and Other apps, their phone parts now This phone; a contact's "Their list" and a message's "Their mail", now the person's sheet.

**Tests**: in `attention.rs`, the usual matrix written out; locks and choices; rows read leniently from the configuration; times together, layers and lending; Always through read against their own row; an oracle against the decisions of the code it replaced, but where the owner decided otherwise, each asserted as such; messages, calls and each decided change; areas and holds; what each Android mode lets through; the older keys seeding what was changed and nothing else; the matrix written whole once, the older keys out; slots read by every process; presets never touching a lock; the grid's words in both languages (no key missing, French typography, « type », never « sorte »). In `reaches.rs`, every card, the moment now and a person's lines in both languages, every row and level said. In `settings.rs`, the tab's switches in their own section.

## 10. What was decided
The owner's answers of 7 October 2026: "Your plan seems sensible to me, and the re-interfacing too. Inconsistencies you found need a fix, obviously." Every recommendation of the design, but the 24th:

1. **One matrix**: mail's two grids (who × when, what × when) written as one, with ◑ for "shown, not told".
2. **Always through** (« Passent toujours »), a row per channel, one meaning everywhere: calls and messages at once at every time; mail at once, but shown without a notification in sleep and a pause.
3. **Blocked beats Always through**; one list takes them off the other, said.
4. **Messages at night** wait in sleep and a pause, as mail's telling does; in Free time your safe senders' still come.
5. **Mail apps on a phone** take the Mail rows, ◑ held.
6. **Codes asked for** come at once in every time, sleep and a pause included, from mail and from other apps; "On the Porch only" stays a choice for sleep.
7. **An event's alarms in a pause, on a phone**: a channel of their own that passes Sioul's modes.
8. **Always through conversations under Sioul's modes**: the modes let Priority conversations through; Sioul opens the conversation's page in Android to mark it.
9. **Safe callers in Free time**: while the phone screens calls, Sioul's modes let every contact's call ring; else starred contacts only.
10. **Always through mail on a computer** goes out at critical urgency.
11. **Calls carry the layers**: Time for you and do-not-disturb, in the calls' table.
12. **Time for you everywhere**: `state/slots.toml`.
13. **Chat sites on a computer**: one naming someone your cards know follows their Messages row when it holds more.
14. **No hours set**: mail reads work and admin together, as calls and messages do.
15. **Free time as cells**, a column in every row; Nothing at all holds the states' rows.
16. and 17. **Spam**: what your own filter flags or moves waits in the review queue, never told; the table's defaults stay, spam and uncertain flagged ([spam-filter.md](spam-filter.md)).
18. to 20. **The interface**: one tab, three presets, a phone showing times first ([§8](#8-the-interface)).
21. **No times of one's own** for a person.
22. **Groups** have a row of their own, as strangers until changed.
23. **Calls after the fact**: a declined call is listed when its Calls row lets them now; a row that never rings is listed in work and admin time.
24. **Older Sioul**: not kept in step; the older keys seed the matrix once, then go.
25. **The holds outside the matrix** stay named holds, each said in its time's card.
26. **French**: « type », never « sorte », for a kind.

## 11. What stays outside
Which device tells (leases), what each platform can show (no codes or dates told on a phone yet), the words of each notification, the ledgers that tell each thing once. The model decides when; they decide where and how.
