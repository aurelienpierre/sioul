# What reaches you, and when: one model

Sioul spends one thing four ways, your attention: what it **shows** (the Porch, the pages), what it **tells** (its own notifications), what it **holds** of the system's (other apps' notifications, calls), and what it **silences** (the system's do-not-disturb). One model decides all four: a matrix of who or what × when → a level, which every caller reads through one pipeline. The code is `sioul_core::attention` (crates/sioul-core/src/attention.rs, [§9](#9-the-code)); the window says it in words in `reaches.rs` and draws it in Settings ▸ **What reaches you** ([§8](#8-the-interface)). The guide's page for the person is [What reaches you, and when](https://aurelienpierre.github.io/sioul/guide/notifications.html).

**Why one model.** Until 7 October 2026 the rules lived in eight places: who × when (`reach`), what × when (`notify`), the time now (`quiet`, `pause`), do-not-disturb (`everywhere`), the Porch's lanes (`porch`), other apps (`appnotes`), calls (`calls`, `Calls.java`) and sites. Each was right on its own; together they overlapped, and some gave three answers for one person at one time: a safe friend's chat came at 03:00, their mail waited for waking, their call was declined. The owner asked to "flatten all possible timeframes × actions (contacts/lists, internal notifications, external notifications through "don't disturb", screened calls) into an unified decision matrix", in the docs and in the interface, before the implementation was redone. This page is the model as built; the questions it settled are in [§10](#10-what-was-decided). The design as first proposed was never committed; its record is [the appendix](#appendix-before-the-model): each setting and code path of the older code with its place in the model, the 24 inconsistencies found in that code at commit `2e9bbf1` with what became of each, and the recommendation each decision answered.

## 1. The objects

### 1.1 Sources
A source is where something comes from. **Sioul's own**: Sioul fetches, keeps or computes it, and decides everything about it. **The system's**: another program owns it; Sioul can only hold it, let it through, or silence the system.

| Source | Whose | What Sioul does | Code |
|---|---|---|---|
| Your mail addresses | Sioul's | judges, sorts, shows, tells | `porch`, `mailnote` |
| Your sites, on a computer | Sioul's (its browser) | catches their notifications | app `sites` |
| Agenda, tasks, budgets, papers, contracts, the bank | Sioul's | reminds | `reminders` |
| Health: doses, meals, naps, the night, moving, the alarm at waking | Sioul's | reminds, rings | app `health`, `wake` |
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
| Your day | meals, naps and the night (`needs`), the pause to move (`move`), "Work hours are over" (`work-over`), the time running (`time`) |
| Sites, on a computer | sites gathered (`sites`), a site in real time (`sites-live`), a call in a site (`site-calls`) |
| Other apps, on a phone | automatons (`app-automatons`), apps and browser sites set to At once (`app-at-once`) |

Thirty-seven rows in all (`Row::ALL`), with ids such as `mail.safe`, `calls.hidden`, `messages.groups`, `codes`.

**Other apps' own rows, on a phone** (the owner's words, 8 October 2026: "we need to list the apps sending notifications, because for example, I need to let Discord through during work hours […] at least we need a mapping between app and timeframes"). Besides the 36, a row per app (`app.<package>`) and per conversation in one (`conversation.<key>`, the key `appnotes::talk_key` makes of its app and its id), with the same nine columns. Each cell takes **At once** (●), **At the gathered times** (◎), **Held** (○) or **As usual** (=), and says as usual until set: an app nobody chose for has no row, and nothing changes for it. As usual reads, column by column, the row the notification takes otherwise: its person's Messages row (Mail's for a mail app, Calls' for a missed call), the groups', the automatons', the apps set to At once; as Always through's = reads their own row. So "Discord: through during Work, held otherwise" is ● under Work and ○ under every other column; ● under Work alone lets it through at work and leaves the rest as usual. Read as any row (`Attention::level`'s rule): the least strict of the time's columns, then the stricter of that and each layer that holds; a layer's ● lets it through that layer (do-not-disturb's cell ● for Discord: through while do-not-disturb holds, at the times its row lets it through), its = holds it as the layer holds everyone else's messages.

- **A conversation's** cell, where set, wins over its app's; where it says as usual, the app's does (`attention::source_rows`: the conversation's, then the app's).
- **What stays**: the blocked never come; codes and approvals, calls, alarms and what runs are never held; someone Always through stays so whatever an app's row says (only a conversation's own row changes them); a conversation set to Never stays held for good. A source's cell set for a time is your word for it: no area holds it then.
- **Where Android cannot follow**: a notification's first sound plays before the listener sees it (the existing guide to make an app silent covers it, [android.md](android.md#notifications-from-other-apps)); during Sioul's own modes Android's policy decides what rings, so a cell ● during a pause, Free time or do-not-disturb shows the notification without a sound, unless it is a priority conversation or a starred contact's message.

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
- Among your own choices: the most precise wins (a conversation over a person over an app; an address over a card over a category over a domain). Other apps' own rows (§1.3): a conversation's cell over its app's, and an app's cell over the person's row, but for someone Always through, whom only a conversation's row changes.
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
"app.com.discord" = ["work", "admin:later", "leisure:later", "meals:later", "sleep:later", "pause:later", "free:later", "slot:later", "dnd:later", "name=Discord"]
"conversation.8c1f0e5a2b7d4c69" = ["leisure", "dnd"]
```

Other apps' own rows (§1.3) write only the cells set (a column unsaid is as usual: `leisure:as` is taken out), and an app's with its name (`name=…`), for your other devices, which never saw the app; a conversation's never with its title, which names a person. A row as usual everywhere is taken out. An older Sioul reads no row of these (`Row::read` knows none) and leaves them as they are. The first such row written does what the first row always does: every seeded row written, the older keys out.

**The older keys.** While no `[attention]` is written, `[reach]`, `[reach.calls]`, `[reach.messages]`, `[notify]`, `[reminders] doses_in_sleep`, `[pause] doses`, `[pause] people` and `[dnd] people` seed the matrix once (`attention::seeded`): read into cells, compared with what the same rules make of an empty configuration, and only what you had changed is kept; everywhere else the usual cells hold, the owner's decisions included. A mail row was the product of two grids: ticked and told now gives ●, ticked and told later ◑, unticked ○. The first row written (Settings, a preset) writes every row that is not usual and takes the older keys out (`attention::OLDER`, `config::set_table`). Nothing is written beside for an older Sioul: alpha, one user.

**Settings' keys**: `attention.<row>` with the row's words whole (`attention.app.<package>` and `attention.conversation.<key>` too); the older grids' `notify.<kind>` and `reach.<row>` land there too (`settings::apply`). The tab's switches stay ordinary settings: `reminders.mail`, `reminders.mail_newsletters`, `reminders.gather`, `free_time.nothing`, `dnd.button`, `dnd.focus`, `dnd.pauses`, `dnd.sleep` (section "attention" of `settings::for_view("parameters")`).

**Shared**: `[attention]` travels with the settings to every device ([database.md](database.md)), other apps' own rows with it; the Always through list in the senders' part; `state/slots.toml` is each device's own.

## 8. The interface
**Settings ▸ What reaches you** (« Ce qui vous joint »; `ReachesTab.qml`, its words from `reaches.rs`, its data from the window's `reachesView`):

- **On top**, in every view: the moment now in sentences ("Now: leisure, until 22:00. Mail from your safe senders comes at once…"), and **Start from**: As Sioul does now, Quieter, More reachable; "Yours: 3 changes…" with **Back to As Sioul does now**.
- **By time**, the default: a card per time and per layer, now's first, each in sentences grouped by level (what comes at once, at the gathered times, when its event falls then, is shown without a notification, waits until when, does not come), Always through first when it gets more than their own lists; then what the phone or the computer silences then, and the named holds of that time ("After the pause, the Porch rests…"). Free time's card holds **Nothing at all**. **Change…** opens that time's rows, each value in words.
- **By person**: Mail, Calls, Messages, each with a sentence when no phone of yours screens calls or holds messages yet; mail's own switches (new mail told, newsletters) and what the spam filter and an address's area hold; the channel's grid on a computer, its rows on a phone, each opening its nine values; then who is on which list (the four lists, the people placed on a list, your contacts' categories).
- **Sioul's own**: the kinds' grid, the sites' gathering and the gathered times.
- **Exceptions**: Always through (each channel's row in a sentence, then the list); on a phone, **Other apps, by time**: a table of each app seen in the last month, then the conversations chosen for and the ten seen last (never a wall of them), the nine columns across, each cell as usual until pressed (`AttentionGrid.qml`, `appnotes::by_time`, `attention::source_grid`), with what Android cannot follow said under it; then each app's line with its row in a sentence ("At once: Work; held: Admin, Leisure…", `attention::source_sentence`), its kind and area, each conversation's way through, each browser site; on a computer, the apps' rows set on the phone, by the name written with them, changed there as here; a computer's sites; an event's Remind; Health's days.
- **Do not disturb**: what turns it on, what holds while it does, what each device does.

A press on a cell opens its choices in words, the current one in bold; a fixed cell says why; a dot marks a cell changed from As Sioul does now. At 412 pixels the whole grid never shows: cards, a time's rows, a row's values; while a time or a row is open, the moment and the presets step aside. The grid is `AttentionGrid.qml`, its marks `LevelMark.qml`.

**Its principles**, from the interface's design of 7 October 2026: one place and one vocabulary for every rule of when something reaches you; read before you change (a sentence for now, then a card per time in words, a time's rows on demand, the whole grid last), so that a person in a bad moment never has to read a grid to know what happens; no wall of marks on a phone; a fixed cell says why; changes from the preset are said and marked, with the way back one press away; what a device cannot do is said where it shows (the Calls rows say when no phone of yours screens calls; the Messages rows, on a computer, when every phone of yours says it does not give Sioul notification access, a phone that says nothing (an older Sioul) counting as one that does: `reaches::Phones`, from each phone's entry in the sharing, [database.md](database.md#devices)); no count, no red, no badge, and marks always with words: a screen reader reads each cell as a sentence. **Not built**: the grid by keyboard as designed, the arrow keys moving from cell to cell, Enter opening a cell's choices and Escape closing them.

**Settings ▸ This phone** (« Ce téléphone »; `PhoneSetup.qml`), on a phone: what sets it up, nothing that decides when: call screening, other apps' notification access and holding, Do Not Disturb's access and its modes, who of your Always through people is starred there, keeping in step in the background, exact alarms and Sioul's own notifications.

**A person's sheet** (`PersonSheet.qml`), from a contact's card or a message's sender: "How Camille reaches you": their list and why, the list chosen for them, Always through, and a sentence per channel of what reaches you from them at each time (`reaches::person`). No times of one's own: a person has a list and Always through.

## 9. The code
**`sioul_core::attention`**:
- **The objects**: `Person` (`persons(channel)` the rows of each channel), `Kind`, `Row` (`People(Channel, Person)` or `Own(Kind)`), `Column`, `Level` (`mark`), `usual`, `choices`, `lock`.
- **The matrix**: `Attention` (`usual`, `of(&Config)`, `cell`, `set`, `words`, `changes_from`); `level(Row, &Now)`; `person(Channel, Person, always, &Now)`; `listed(Person, always, &Now)` (a declined call listed after the fact); `mail(..)` (the Porch's reading of a message).
- **Other apps' own rows**: `APP_ROW`, `CONVERSATION_ROW`, `NAME`, `SOURCE_CHOICES`, `is_source`, `source_rows(package, conversation)`; on `Attention`, `source`, `sources`, `source_name`, `set_source`, `name_source`, `source_words`, `has_sources`, read by `decide` through `Event::from_sources` (its step `Step::Chosen` when a cell set decided); `Settings::sources`; `source_grid`, `SourceLine`, `source_sentence`. Their timing on a phone: `appnotes::first_time` and `by_matrix` (at once now or where things change, gathered at a gathered time), `Ask::sources`, `Why::Times`.
- **The moment**: `Now` (`of(&Mode)`, `of_moment(..)`, `time(Column)`, `layers(slot, dnd)`; `times`, `time`, `week`, `slot`, `dnd`, `nothing`, `span`, `gathering`, `realtime`, `through`, `holds: Holds { porch_rests, meeting, chats }`).
- **The pipeline**: `decide(&Event, &Now) -> Output`; `Event { source, area, starts }` (`own`, `of`, `for_area`, `starting`); `Source`, `Lane`, `Output`, `Step` (emergency, blocked, content, floor, matrix, area, hold).
- **The system**: `silence(&[Column], nothing, Phone { screens }) -> Silence { calls, messages: Senders, repeat, conversations, alarms, doses, events }`.
- **Presets**: `Preset` (`matrix`), `apply_preset`.
- **The configuration**: `Settings` (`[attention]`), `NotifySettings` (the older `[notify]`, read to seed), `seeded`, `apply`, `OLDER`.
- **Time for you**: `Slots`, `dnd_from_files`.
- **Words**: `grid` (columns, marks, rows with their choices, each cell said in a sentence, presets, the changes), `channel_rows`, `kind_rows`, `list_choice`, `column_label`, `row_label`, `level_label`; the `attention-*` strings of both `.ftl` files. The moment now is said in one place, the window's (`crates/sioul-app/src/reaches.rs`, `now_sentences`), which knows this device's channels and the hours' end.

**Who asks it**: the Porch's display (`backend::compute`, the phone's card `homecard`); new mail (`mailnote`: now, through, later or never; through at critical urgency on a computer, on its own channel on a phone); codes (`backend::notify_codes`, `sioul watch`); reminders (`reminders::Holds`: an event's own by its span, work's by their area); Health's notices, the pause to move (`health`); the end of the day (`reviews`); the time running (`timenote`); the pause's screen (`pauses`); a phone's background fetch (`steps`); sites (`sites`: gathered, live, calls, chat sites by who wrote); other apps (`appnotes::decide`); calls (`calls::frames` from the matrix and its layers, Always through as rows of their own, never a blocked number); Sioul's system modes (`everywhere`, `dnd`); `sioul remind`.

**The window's words** (`crates/sioul-app/src/reaches.rs`): `view` (the tab's JSON: the moment now, the presets, the cards, the grid, the exceptions), `set_row`, `preset`, `person`, `person_change`; each sentence built from the matrix, people grouped by what each channel does with them ("mail and messages from everyone", "calls from your safe and neutral senders"), Sioul's own named by their kinds, verbs agreeing in both languages.

**Removed**: `notify.rs`; `reach`'s matrices (`Reach`, `Matrix`, `Times`, `Row`, `Column`, `next_allowed`); `quiet::mail_in_view` and `quiet::list_choice`; `pause::reach_now`; `everywhere::mail_gate`; Accounts ▸ Who may reach you; Settings ▸ Reminders and notifications ▸ When each comes; Settings ▸ Do not disturb's tab, its list now Always through; Settings ▸ Calls and Other apps, their phone parts now This phone; a contact's "Their list" and a message's "Their mail", now the person's sheet.

**Tests**: in `attention.rs`, other apps' own rows read over their usual row (nothing changes without one, at every time and layer; through during work and held otherwise; as usual reading the person's row; a layer's cell; a conversation's over its app's; Always through kept; the blocked never; no area over a cell set; "Nothing at all"; the grid and the row's sentence in both languages) and written with the settings (only the cells set and an app's name, the first write seeding, taken out when as usual); in `appnotes.rs`, an app's own row deciding when (every fixture unchanged without one, through at work, held in the evening until work, a conversation's row over its app's, gathered by day, under do-not-disturb, worked out again, a code and the blocked as before, an automaton the matrix lets through at once coming at once); the usual matrix written out; locks and choices; rows read leniently from the configuration; times together, layers and lending; Always through read against their own row; an oracle against the decisions of the code it replaced, but where the owner decided otherwise, each asserted as such; messages, calls and each decided change; areas and holds; what each Android mode lets through; the older keys seeding what was changed and nothing else; the matrix written whole once, the older keys out; slots read by every process; presets never touching a lock; the grid's words in both languages (no key missing, French typography, « type », never « sorte »). In `reaches.rs`, every card, the moment now and a person's lines in both languages, every row and level said. In `settings.rs`, the tab's switches in their own section.

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


## Appendix: before the model
The design as first proposed on 7 October 2026, before it was built, kept for the record of why: what each older setting and code path became ([A.1](#a1-each-older-setting-and-code-path-and-its-place-in-the-model)), the inconsistencies of the older code and what became of each ([A.2](#a2-the-inconsistencies-of-the-older-code)), and the recommendation each decision of [§10](#10-what-was-decided) answered ([A.3](#a3-the-questions-what-was-recommended-and-what-was-decided)). Files and lines are those of commit `2e9bbf1`, the code before the model; most of those files are gone ([§9](#9-the-code), "Removed").

### A.1 Each older setting and code path, and its place in the model
| Before the model | Where, at `2e9bbf1` | In the model |
|---|---|---|
| Accounts ▸ Who may reach you ▸ Mail, `[reach]` `safe`, `neutral`, `restricted`, `stranger` | reach.rs:439-468; settings.rs:316-327 | the Mail rows' time columns: ticked and told → ●, ticked and not told → ◑, unticked → ○ |
| … ▸ Calls, `[reach.calls]`, with `hidden` | the same | the Calls rows: ● or ○ |
| … ▸ Messages, `[reach.messages]` | the same | the Messages rows, with the cells of the older "Messages from people" row |
| … the four lists, the people on a list, the categories | porch.rs:764-1083 | the people rows ([§1.2](#12-who-the-people-rows)), their files unchanged |
| Settings ▸ Reminders and notifications ▸ When each comes, `[notify]` | notify.rs | the kinds' rows: 17 unchanged; "New mail" and "Messages from people" melted into the Mail and Messages rows; their do-not-disturb values went to the Always through rows |
| New mail: told at the times it may come (`[reminders] mail`), newsletters included or not | settings.rs:514-515 | the Mail rows' "told" switch; the rule of the Filed lane |
| Sites' notifications gathered (`gather`), the gathered times (`gathered`) | settings.rs:517-518 | the Sites row's switch; one list of gathered times, for sites and apps |
| Settings ▸ Do not disturb: the switch; while I focus, during the pauses, while I sleep | everywhere.rs:50-77 | the Do not disturb layer: what turns it on, and what each column silences |
| The people on my list get through, and the list | everywhere.rs:528-718; the window's calls.rs:216-217 | the Always through rows, one per channel, and a person's own "Always through" |
| Keep this phone in step in the background | `[dnd] background`, this phone only (share.rs:123) | the phone's own setup |
| Settings ▸ Pauses ▸ Nothing at all (`[free_time] nothing`) | pause.rs:59-74; reach.rs:402-404 | Free time's choice: every people row ○ in this Free time |
| Settings ▸ Pauses ▸ Starred contacts get through (`[pause] people`) | pause.rs:101; the window's everywhere.rs:96 | the Pause column's "who the system lets through" |
| `[pause] doses`, `[reminders] doses_in_sleep` | notify.rs:423-424, 564-567 | the Doses row × Pause and Sleep (already cells) |
| Settings ▸ Calls (the role, contacts, forwarding, texts) | the window's calls.rs; `CallsSetup.qml` | the phone's own setup; the grid is the Calls rows |
| Let every call through | calls.rs:398-404 | a layer of calls, a floor after the blocked |
| Settings ▸ Other apps: access, holding, the apps that rang | the window's appnotes.rs | the phone's own setup |
| … each app, conversation and browser site | appnotes.rs:599-644 | exceptions |
| A site's Real time, Silenced and For; the Porch's Real time box | `[[site]]`; the window's sites.rs:339-345 | exceptions; the Real time layer |
| An address's For, priority and shield | `[[account]]` | the pipeline's step on areas; the Porch's lanes |
| An event's Remind; a block's Not today | reminders, `needs::Days` | exceptions |
| Tasks ⚙ ▸ Time for you | the window's capacity.rs | the Time for you layer |
| Mail ⚙ ▸ Your own spam filter | settings.rs:278-288 | the spam filter's classes |
| Health ⚙: the chat limit, moving, each block's notices | | named holds and master switches |
| Android: Sioul's channels, its modes, stars | Java | the outputs |

### A.2 The inconsistencies of the older code
Each, as found at `2e9bbf1`, then what became of it as built on 7 October 2026 (the sections above say how it works now). "Q" numbers are the questions of A.3.

1. **Two grids for mail.** Who × when (reach.rs:348-354) said when mail was shown; what × when (notify.rs:353-357) said when it was told; a cell of one multiplied the other (`Cell::admits`, notify.rs:296-303; the window's mailnote.rs:131-139). The safe row ticked sleep, yet nothing was told in sleep: you read two grids to know one answer. *Became*: one grid, with ◑ "shown, not told" (Q1).
2. **One person, three answers at night.** A safe person's chat came at 03:00 (notify.rs:361 gave the people of other apps ● in sleep; messages copied mail's safe row, reach.rs:466); their mail waited for waking (notify.rs:353-357); their call was declined (reach.rs:360-361). In a pause, the same three answers; in Free time, their chat and their call came while their mail was shown, not told. *Became*: one answer per row and time (Q4).
3. **A mail app on the phone and Sioul's own mail disagreed.** A mail app's notification used the Mail rows' ticks but the cells of other apps' people (appnotes.rs:476-482, 937): another mail app's notification at 03:00 from a safe sender came, while Sioul's notification of the same message waited. *Became*: mail apps take the Mail rows (Q5).
4. **The do-not-disturb list had three meanings.** Calls: it always rang, sleep and pauses included (calls.rs:384-386; the window's calls.rs:216-217). Mail and messages: only while the switch or a focus session held (everywhere.rs:352-354), and, as usual, only when Who may reach you let them through too (`list`, notify.rs:299). The system: starred contacts, not the list (PauseMode.java:252-258). Its name, "Who may reach you during do-not-disturb", said none of the first. *Became*: Always through, one meaning everywhere (Q2).
5. **Blocked, yet let through.** A number blocked and on the list rang (calls.rs:384-386, before 394-396). A blocked person writing in a conversation set to "always through" was let through (appnotes.rs:959-963, before 933). Mail from the blocked was set aside first (porch.rs:1287-1289). *Became*: the blocked first, everywhere (Q3).
6. **An event's alarms, "fixed: at once in a pause"** (notify.rs:331), arrived silent on a phone: the "Events" channel was low and never passed do-not-disturb (EventAlarms.java:292-293), and Sioul's own pause mode was on. *Became*: an event's alarms have a channel of their own that passes Sioul's modes (`Silence::events`, and its Java side) (Q7).
7. **Always-through conversations** were let through at any time (appnotes.rs:961), then silenced by Sioul's own modes, which let no conversation through (PauseMode.java:272) and only starred people's messages (256-257). *Became*: Sioul's modes let important conversations through (`Silence::conversations`; Java marks them) (Q8).
8. **Safe callers in Free time.** The Calls rows let safe senders ring in Free time (reach.rs:402-404), but Free time's mode let only starred contacts ring (the window's everywhere.rs:97; PauseMode.java:254-256): a safe contact who was not starred was silenced. *Became*: every contact rings through Sioul's modes while the phone screens calls (`Silence::calls`) (Q9).
9. **The list's mail on a computer** was told during do-not-disturb at normal urgency (sioul-sync's notify.rs:68-90), which Plasma's inhibition, Sioul's own mode, kept out of sight; [do-not-disturb.md](do-not-disturb.md) said it came at critical urgency, which it did not. *Became*: Always through mail at critical urgency on a computer (`Output::pierce`) (Q10).
10. **Time for you was known by the window only.** The slots were a static of the window's process (the window's capacity.rs:70-74, 95). The listener of other apps (the window's appnotes.rs:149), the background service, `sioul remind --watch` and `sioul watch` (reminders.rs:244; notify.rs:538-544) never saw one, and calls ignored it. *Became*: today's slots in `state/slots.toml`, read by every process (Q12).
11. **Sites on a computer, apps on a phone.** One list of gathered times (`[reminders] gathered`), two rows and two ways: one gathered notification from the computer you were at (the window's sites.rs:288-293), or each notification held until then. A chat site on a computer was judged by its site and its area (the window's sites.rs:336-351), the same chat's app on a phone by who wrote (appnotes.rs:483-516). *Became*: a chat site naming someone your cards know follows their Messages row when that holds more (Q13).
12. **Spam left in its lane was told.** With "say", the default then, probably spam kept its lane and was told as its row (mailnote.rs:88-95 did not list it), though the author had decided it was never told. *Became*: the review queue, never told (Q16).
13. **Calls ignored do-not-disturb's switch and focus.** Their frames carried the times only (calls.rs:203-214); the switch reached calls through the system's mode alone. Mail, messages and Sioul's own followed the Do not disturb column. *Became*: calls carry the layers (Q11).
14. **No hours set.** Mail came whatever its row (quiet.rs:536); calls and messages read work and admin (reach.rs:406). *Became*: mail reads work and admin too (Q14).
15. **Codes in sleep.** Sioul's mail codes waited on the Porch in sleep and in a pause (notify.rs:345); another app's codes were never held (appnotes.rs:954); on a phone, mail codes were never told at all (sioul-sync's notify.rs:148-153). Winding down is sleep, so a code asked for at 22:40 was not told. *Became*: codes at once everywhere, "On the Porch only" a choice for sleep (Q6).
16. **Free time's rule was code, not cells.** Who may reach you had no Free time column; Free time narrowed every channel to the safe (reach.rs:402-404; pause.rs:242-245), while When each comes had a Free time column. "Nothing at all" lived in Settings ▸ Pauses (settings.rs:529) and acted on every channel. *Became*: a Free time column in every people row (Q15).
17. **Calls after the fact, two rules.** A missed call seen by the listener followed the Calls row with the cells of other apps' people (appnotes.rs:476-477); a call Sioul declined was listed when the Calls or the Mail row let them through now (the window's calls.rs:330-333). *Became*: both by the Calls row (Q23).
18. **Doses in a pause, four places**: the matrix's cell, the older `[pause] doses` kept beside it (notify.rs:564-567), the pause mode's dose channel (the window's everywhere.rs:94), and the pause's screen (the window's pauses.rs:306). *Became*: one cell, the doses' Pause cell, read by the reminder, the pause's mode and its screen.
19. **Holds outside the matrix.** The Porch resting after a pause (pause.rs:198-200), a meeting (health.rs:2339-2342), the chat limit (the window's sites.rs:340), what a thing is for: each its own rule, none said in the grids. *Became*: named holds (`attention::Holds`, `Step::Hold`), each said in its time's card (Q25).
20. **`list-any` told mail the Porch hid.** During do-not-disturb, mail from someone on the list whose row said ○ then was told (`ListAny`, notify.rs:300) but not shown (backend.rs:2146). *Became*: ☆ both shows and tells.
21. **Groups had no row.** A group conversation was a stranger (appnotes.rs:969-972), which no grid said. *Became*: a Groups row (Q22).
22. **A restricted domain landed in the screener.** A sender named only by a pattern was not "known" (porch.rs:1147): restricted by domain, their mail waited in the screener's lane among strangers', though at the restricted row's times. *Kept as it was, on purpose*: the screener holds the senders a domain alone names, each address unknown, while their row (restricted, say) gives their times. Whether such a sender should skip the screener, as a contact does, is not decided yet ([porch.md](porch.md)).
23. **The words.** French said « sorte » for a kind of notification (« Chaque sorte de notification », « sorte par sorte »), where the application's word for a kind is « type ». *Became*: « type » in every French string that names a kind (Q26).
24. **Your own do-not-disturb, two answers.** On Linux, Sioul's notifications passed it at critical urgency, "what it sends always gets through" (sioul-sync's notify.rs:375-376), but new mail (74-80). On a phone, on Windows and on macOS, your own do-not-disturb held Sioul's notifications like any app's, but, on a phone, the alarm at waking and doses during a pause. *Became*, by platform: a computer's notifications from Sioul pass its do-not-disturb at critical urgency, except new mail, which goes at normal urgency unless it comes from someone Always through; a phone's go through Sioul's own channels, and those that pass are named by `Silence`.

### A.3 The questions, what was recommended, and what was decided
Each question was put with its recommendation on 7 October 2026. Every recommendation was adopted but the 24th ([§10](#10-what-was-decided)).

| # | The question | Recommended | Decided |
|---|---|---|---|
| 1 | One matrix, with ◑ "Shown, not told", for mail? | Yes: ◑ is the older sleep of safe senders, named. | As recommended. |
| 2 | The do-not-disturb list becomes "Always through" (« Passent toujours »), a row per channel? | Yes, with one meaning everywhere: calls and messages at once at every time, sleep and pauses included; mail told at once under do-not-disturb, Free time and Time for you, and shown, not told, in sleep and a pause. | As recommended. |
| 3 | Does blocked beat Always through? | Yes; putting someone on one list takes them off the other, said. | As recommended. |
| 4 | Messages at night (A.2, 2)? | Messages from people are held in sleep and a pause, as mail's telling is (Always through aside); in Free time, the safe's messages still come at once, as their calls do. | As recommended. |
| 5 | Mail apps on the phone (A.2, 3)? | They take the Mail rows' cells, ◑ read as ○: a phone cannot show what it holds. | As recommended. |
| 6 | Codes asked for (A.2, 15)? | At once in every time, sleep and a pause included, for mail and other apps alike; "On the Porch only" stays a choice for sleep. | As recommended. |
| 7 | An event's alarms in a pause, on a phone (A.2, 6)? | A channel of their own that passes Sioul's modes, as doses have; Sioul's reminders before the event stay quiet. | As recommended. |
| 8 | Always-through conversations under Sioul's modes (A.2, 7)? | Sioul's modes let important conversations through, and Sioul opens the conversation's Android settings to mark it so. | As recommended. |
| 9 | Safe callers in Free time (A.2, 8)? | While the phone screens calls, Sioul's modes let every contact's call ring, since the screening has already declined the others; without screening, starred contacts only. The cost: a call the screening cannot answer in time rings, as Android would, and rings through the mode too if it is a contact's. | As recommended. |
| 10 | The list's mail on a computer (A.2, 9)? | Always through mail at critical urgency under do-not-disturb. | As recommended. |
| 11 | Calls under do-not-disturb's switch and focus (A.2, 13)? | The calls' table carries the layers; Sioul declines what the matrix holds, and lists it after. | As recommended. |
| 12 | Time for you everywhere (A.2, 10)? | Today's slots written to `state/slots.toml` on each device, read by every process. | As recommended. |
| 13 | Chat sites on a computer (A.2, 11)? | A chat site's notification that names a person on a card follows the Messages rows, since names may only hold more; the rest stays a site's. | As recommended. |
| 14 | No hours set (A.2, 14)? | Mail reads work and admin, as calls and messages do; the usual rows let everyone through then anyway. | As recommended. |
| 15 | Free time as cells (A.2, 16)? | Yes, a Free time column in every people row; "Nothing at all" stays the choice made when pressing Free time, holding every people row. | As recommended. |
| 16 | "Maybe spam", flagged: told or not? | Keep it untold: a stranger's mail is told in work and admin time at most, and the review queue shows it then. | As recommended. |
| 17 | The spam filter's defaults? | Keep them, spam and uncertain flagged, ham left alone; "move" only by the person's choice, since it changes the server. | As recommended. |
| 18 | Where it lives? | One Settings tab, "What reaches you", and Settings ▸ This phone for a phone's setup. | As recommended. |
| 19 | Presets? | "As Sioul does now", "Quieter", "More reachable"; no preset ever touches a lock. | As recommended. |
| 20 | A phone? | Times first, a time's rows second, never the whole grid at 412 pixels. | As recommended. |
| 21 | A person's own times, a row for one person? | Not now: a person has a list and Always through, enough for every case found so far. | As recommended. |
| 22 | Groups (A.2, 21)? | A row of their own in Messages, as strangers until changed. | As recommended. |
| 23 | Calls after the fact (A.2, 17)? | Both by the Calls row. | As recommended. |
| 24 | An older Sioul? | Write the older keys beside the new ones for two versions, then drop them. | **Not adopted**: no older key is written beside the new ones; the older keys seed the matrix once, then go, and an older Sioul is not kept in step. |
| 25 | The holds outside the matrix (A.2, 19)? | Kept as named holds, each said in its time's card ("after a pause, the Porch rests until 14:00"). | As recommended. |
| 26 | French (A.2, 23)? | « type », never « sorte », in every string that names a kind. | As recommended. |
