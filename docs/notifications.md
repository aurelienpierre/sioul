# Notifications: one model, one call

Every notification Sioul posts on a computer is a **notice about one thing**: a message, a code, a task's date, an event, a meal, the pause to move, the end of the day, a site's message. Each source makes one call about it; one module decides how it reaches you, how long it stays, and what a click on it opens. The attention model ([attention.md](attention.md)) decides *when* something may reach you; this page is *how*. Decided on 13 November 2026, with its seven recommendations; this is its first phase, the model and the computer. The phone's notices (Java's, [android.md](android.md#notifications)) follow in the third phase; until then they are as before.

## Why

Before this, about fifteen files each built their own notifications, each with its own urgency, its own lifetime or none, its own click or none. Every reminder went at critical urgency, so that it passed the desktop's do-not-disturb; Plasma 6.8 turns a critical notification's timeout into "never expires", and nothing replaced or withdrew them: a day of running left about thirteen "Time to move" on the screen, two notices per meal, and each one still listened to kept a thread and a D-Bus connection awake. A site's message went as a code, with no click of its own: the desktop raised Sioul's window and Sioul never heard of it. A click on a code's notice ended its buttons. A click after a popup closed, or after Sioul restarted, was heard by nobody, and the window was raised without the token KWin asks for.

## The model

`sioul_core::notice` (crates/sioul-core/src/notice.rs).

**`Notice { about, hint, kind, title, body, actions, until, icon }`**:

- **`about`**: the thing, by its address, the same addresses the links and the MCP tools use. **A click on the notice opens it.**
- **`hint`**: where this device last saw it, for the window to find it at once: a message's file, an event's file and the time it is for (`…/yoga.ics#1793520000`).
- **`kind`**: `notice::Kind::Own(attention::Kind)` for Sioul's own (codes, doses, an event's alarms, the time to leave, dates, needs, the pause to move, the end of work, sites), `Mail` for new mail (whose rows are the people's: it has no kind of its own), `Minor` for a word of Sioul's own outside the matrix (the offer of a language found in your mail, calendar files handed to an older Sioul).
- **`actions`**: buttons, each a verb on the thing (`Act`): `Open { address, hint }`, `Copy(text)` (a code: the notice has done its work then, and goes), `Verb(name)` (one of the window's own: "read-language:de").
- **`until`**: the thing's own end, when it has one: a code's expiry, an event's start, a call's last ring.
- **`icon`**: a theme's name, or a file (a site's own icon, sent as `image-path`); "" the kind's own.

`Notice::key()`, its kind and its thing (`move|sioul:move`), is what a new notice replaces: one "Time to move" at most; a meal's notice at its time takes its heads-up's place; a chat's next message in the same conversation takes the last one's.

### The addresses

| Thing | Address |
|---|---|
| A message | `mid:<Message-ID>`, or `sioul:mail/<account>/<digest>` without one (`links::mail_uri`, `digest_uri`) |
| A code | its message's address |
| A task, a paper, a contract, a budget's line, a letter | `sioul:task/<UID>`, `sioul:paper/…`, `sioul:contract/…`, `sioul:budget/<budget>/<line>`, `sioul:letter/…` (`links`) |
| An event, at one time | `sioul:event/<UID>#<start>` (`notice::address::event`) |
| A dose | `sioul:dose/<key>` |
| A meal, a nap, the night | `sioul:need/<block>`, the block's key |
| The pause to move | `sioul:move` |
| The end of the work day, of the whole day | `sioul:day/work:<date>`, `sioul:day/night:<date>` |
| A site's notice | `sioul:site/<id>#<key>`: the site, and the web notice's own key (its tag: one per conversation on a chat site; one the Sites page makes where the site gave none) |
| The sites' news gathered | `sioul:porch` |
| New mail of several messages | `sioul:mailbox`, the mailbox at its top, until each message has its own opener (phase 2) |
| A setting | `sioul:settings/<key>` (the language offer: `sioul:settings/words.languages`) |

Ids and tags inside an address are percent-encoded (`/`, `#`, `%`, `?`, spaces…), as the window's `decodeURIComponent` reads them back.

## Two classes

`notice::Class`, from the kind (`attention::Kind::class`):

| | **Informative**: something to know | **Actionable**: something to do |
|---|---|---|
| Kinds | the pause to move (`move`), meals, naps and the night (`needs`), "Work hours are over" (`work-over`), dates, waits, payments, papers, contracts and the money watch (`dates`), the time to leave for an event (`before`), the sites gathered (`sites`), a site's message (`sites-live`), new mail, the minor notices | codes and temporary passwords (`codes`), doses (`doses`), an event's own alarms (`alarms`), a call in a site (`site-calls`), the alarm at waking (`wake`) |
| How long it stays | **the minutes set** (`[notices] informative_minutes`, five unless set, read between one and sixty; Settings ▸ What reaches you ▸ Sioul's own); the time to leave until its event starts instead; a thing that ends sooner takes it away then | until its thing is settled (the code copied, read or expired; the call over) or ends (`until`) |
| Posted only when | told now, someone at the computer, and neither a pause nor a do-not-disturb holds | told now; under a pause or a do-not-disturb, only when it pierces |
| Urgency | normal | normal; critical only when it pierces a do-not-disturb that holds now |
| On the server | the expire timeout of its lifetime, the hint `transient` (no trace in Plasma's history), and a `CloseNotification` from Sioul at its end (GNOME ignores timeouts) | the expire timeout until its `until`, else 0 (never); still listened to after its popup expired, twelve hours at most |

The time running (`time`) is no notice: a status, while a focus session runs (`notify::ongoing`), resident, changed in place. Other apps' notifications on a phone (`app-automatons`, `app-at-once`) are never Sioul's to post. Neither has a class.

An event's own alarm is to do: its row is locked through a pause and a do-not-disturb ([attention.md](attention.md#3-the-usual-matrix)), and only what is to do passes them.

**`notice::decide(notice, output, moment, minutes, now) -> Decision`** says it all at once: `Drop(why)` (`NotOurs`, `NotTold`, `Away`, `Held`, `Over`) or `Post(Shape { class, urgency, timeout_ms, transient, key, close_at })`. Its inputs:

- **`output`**: what the attention model says now (`attention::Output`: told, pierce). A source that decided already (a reminder due, a batch of mail its rows let through, a gathered time, a meal's notice at its time, which sleep's column does not hold) passes `notice::told(kind)`: told, piercing as its kind does (`attention::Kind::pierces`: codes, doses, the alarm at waking, an event's alarms).
- **`moment`** (`notice::Moment`): `present`, someone at this computer (input in the last ten minutes and its session unlocked, as the devices' entries say: sioul-app's `devices::someone_here`; unknown counts as someone); `paused`, Sioul's pause; `silenced`, a do-not-disturb holding: one of Sioul's modes (the switch, a focus session, the pause, Free time, the night: `everywhere::now().on()`), or the desktop's own, which Sioul follows ([do-not-disturb.md](do-not-disturb.md#both-ways)). A process without the window (the command line) reads it from its files (`notice::moment_from_files`).

**Through do-not-disturb, Sioul decides alone** (decision 4): nothing to tick in Plasma's settings. What is to know never passes a pause or a do-not-disturb: the Porch keeps what matters. What is to do passes when the model says it pierces, at critical urgency while a do-not-disturb holds. Under a pause without the desktop's do-not-disturb (`[dnd] pauses` off), normal urgency is enough. Mail from someone Always through is to know: under do-not-disturb on a computer, the Porch shows it, untold.

## The transport

`sioul_sync::notify::Teller` (crates/sioul-sync/src/notify.rs): the notices of a process, **one thread and one D-Bus connection for all of them** (`teller()`, started at the first word), in place of a thread and a connection per notice. Its words go in order: `post`, `settle`, `flush` (waits until the words before it are done: a process that ends right after, `outside::to_older`), `restart`, `listen`.

- **Replacement**: a map from each notice's key to the server's number, given as `replaces_id`.
- **The Notify**: "Sioul", the number it replaces, its icon, its title, its body as the server reads it (markup written as entities, `body_text`), its buttons, the hints `suppress-sound`, `desktop-entry` (Sioul's desktop file), `urgency`, `transient` for what is to know, `image-path` for a site's own icon, and its timeout.
- **Clicks**: every notice with a thing gets the `"default"` action, which opens it. A button keyed by its act (`open`, `copy`, `do`; a second of the same act by its place, `open-2`). A press never ends the listening: a click on a code's notice opens its message and leaves Copy and Open alive (before, the first press of any key ended it). Copy takes the notice away. A teller nobody listens to (the command line) posts neither buttons nor a click of its own: the desktop then brings Sioul's window forward by itself.
- **The activation token**: the specification's `ActivationToken(id, token)`, which Plasma sends just before `ActionInvoked`, is kept and handed with the press (`Press { about, act, token }`); the window gives it to Qt (`sioul_set_activation_token`, cpp/application.cpp) before it brings itself forward, so that KWin lets it come to the front.
- **Only the server is heard**: Plasma 6.8 sends its signals to everyone (they are not addressed: `server_p.cpp`); a press said by another program than the server's unique name is not heard.
- **The ends**: a notice is taken away by Sioul at its `close_at`, when its thing is settled (`Settled`, looked at every thirty seconds: new mail's messages read or dismissed on any device, a code's message read), by `settle(about)`, and after Copy. Closed by you or by the server, it is forgotten; expired at the server (`NotificationClosed` reason 1), what is to know is forgotten, what is to do is still listened to (a click in Plasma's history), until its thing is settled or twelve hours pass.
- **The server started again**: its unique name is asked before each notice; another one than before, the numbers kept are forgotten (the new server could give them to another program's notice).
- **A new run**: what shows is written to `$XDG_STATE_HOME/sioul/notices-shown.json` at each change (the window's alone): format 1, the server's unique name and the bus's own id (`GetId`, another for each bus started: a server's unique name may come again after a new login), then each notice's number, thing, hint, kind, class, end and time. Never its words: a code's body is its secret. At the window's start (`tell::start`), the previous run's notices are closed where the same server on the same bus still shows them (a notice of a run gone hears nothing: its click would open nothing), and what is still to do among them is told again by its source: a code not expired, read again from its message (`backend::retell_codes`), an event's alarm before its time is over (`remind::retell`). Checked upstream (plasma-workspace, branch Plasma/6.8, `libnotificationmanager/server_p.cpp`): `ServerPrivate::CloseNotification` closes any number, whichever connection asks; it never checks who posted it.
- **Windows and macOS**: through notify-rust, as far as each carries it. macOS: the buttons and a click of its own, each notice waited for on a thread of its own (its call blocks until a press); nothing taken away. Windows: the words alone; the window gives a notice with a thing to Sioul's icon in the tray instead, whose message a click opens (`tray_notice`, main.qml's `onTrayNotice`). Android: none here.

**Doses keep their own path**, untouched: `remind_dose` and its tries on threads and connections of their own, the "Taken" press, the record of what became of each try, their withdrawal once marked anywhere ([health.md](health.md)), and the question on doses due while Sioul was closed (`remind`). They go at critical urgency, as before, which passes the desktop's do-not-disturb; they already behave as notices to do. They move onto the common call only if you ask.

## The window

The computer's sources move onto `tell` in the next step; until then they post as before.

## Tests

- crates/sioul-core/src/notice.rs: each kind in its class; informative notices for their minutes, transient, never to an empty desk, in a pause or under a do-not-disturb; the time to leave until its event; actionable ones until their end, critical only when piercing a do-not-disturb on now, held otherwise; one notice per kind and thing; the lifetime read and bounded, an older configuration without it read; `told` piercing as its kind.
- crates/sioul-sync/src/notify.rs (`notices_go_as_their_class_says_and_hear_every_press`, against a stand-in server on the test's own bus): replacement, transient, the timeout, the urgency, the close at the lifetime's end, the default action, the activation token, a press from another program not heard, listening after expiry for what is to do, a body click that leaves the other buttons alive, Copy taking it away, critical only when piercing a do-not-disturb on now, `settle`, the settled thing, a site's picture, a teller nobody listens to, and the next run closing the previous one's notices and giving back what is still to do; the file of what shows read in its own format only; the buttons keyed by their act; the doses' `remind` as before.

**Not testable here**: Plasma itself (the popup's look, its history, KWin taking the token), GNOME, macOS and Windows; a site's own `click()` and `closed` (Proton, Discord); the real session's presence.
