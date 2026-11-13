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
| How long it stays | **the minutes set** (`[notices] informative_minutes`, five unless set, read between one and sixty; Settings ▸ What reaches you ▸ Sioul's own); the time to leave until its event starts instead (its popup the minutes set); a thing that ends sooner takes it away then | until its thing is settled (the code copied, read or expired; the call over) or ends (`until`) |
| Posted only when | told now, someone at the computer, and neither a pause nor a do-not-disturb holds; else it **waits**, never marked told, while it is still worth telling (below) | told now; under a pause or a do-not-disturb, only when it pierces, else it waits as what is to know does |
| Urgency | normal | normal; critical only when it pierces a do-not-disturb that holds now |
| On the server | the expire timeout of its lifetime, the hint `transient` (no trace in Plasma's history), and a `CloseNotification` from Sioul at its end (GNOME ignores timeouts); the time to leave not transient: its popup the minutes set, then in the history until its event starts | the hint `resident` (a press leaves it, and its other buttons, where the server honours it: Plasma, GNOME), the expire timeout until its `until`, else 0 (never); still listened to after its popup expired on a server that moves it to its history, until settled or twelve hours |

The time running (`time`) is no notice: a status, while a focus session runs (`notify::ongoing`), resident, changed in place. Other apps' notifications on a phone (`app-automatons`, `app-at-once`) are never Sioul's to post. Neither has a class.

An event's own alarm is to do: its row is locked through a pause and a do-not-disturb ([attention.md](attention.md#3-the-usual-matrix)), and only what is to do passes them.

**`notice::decide(notice, output, moment, minutes, now) -> Decision`** says it all at once: `Post(Shape { class, urgency, timeout_ms, transient, resident, key, close_at })`, `Wait(until)` (not now, still worth telling until then) or `Drop(why)` (`NotOurs`, `NotTold`, `Away`, `Held`, `Over`). Its inputs:

- **`output`**: what the attention model says now (`attention::Output`: told, pierce). A source that decided already (a reminder due, a batch of mail its rows let through, a gathered time, a meal's notice at its time, which sleep's column does not hold) passes `notice::told(kind)`: told, piercing as its kind does (`attention::Kind::pierces`: codes, doses, the alarm at waking, an event's alarms).
- **`moment`** (`notice::Moment`): `present`, someone at this computer (input in the last ten minutes and its session unlocked, as the devices' entries say: sioul-app's `devices::someone_here`; unknown counts as someone); `paused`, Sioul's pause; `silenced`, a do-not-disturb holding: one of Sioul's modes (the switch, a focus session, the pause, Free time, the night: `everywhere::now().on()`), or the desktop's own, which Sioul follows ([do-not-disturb.md](do-not-disturb.md#both-ways)). A process without the window (the command line) reads it from its files (`notice::moment_from_files`). `day_ends`, when the local day ends (`Moment::end_of_day`).

**Through do-not-disturb, Sioul decides alone** (decision 4): nothing to tick in Plasma's settings. What is to know never passes a pause or a do-not-disturb: the Porch keeps what matters. What is to do passes when the model says it pierces, at critical urgency while a do-not-disturb holds. Under a pause without the desktop's do-not-disturb (`[dnd] pauses` off), normal urgency is enough. Mail from someone Always through is to know: under do-not-disturb on a computer, the Porch shows it, untold.

**What waits** (the review of phase 1, findings 1 and 2): a notice that cannot come now, nobody at the computer, a pause or a do-not-disturb, is never lost nor marked told. `decide` answers `Wait(until)` while it is still worth telling (`Notice::relevance`): as its source says (`relevant`), else its thing's end (`until`: the time to leave until its event starts, a call while it rings), else its kind's: a date, a wait, a payment, a paper, a contract, new mail and the minor notices until the end of their day; the end of work until the evening's leisure is over (its source says the night's start); a meal or a nap at its time while it lasts; the sites' gathered news an hour (the next gathering names it anyway); the pause to move, a meal's heads-up and a site's message a minute, after which they go (a site's message is kept on the Porch and for the next gathering in any case). Past that end, `Drop`. The first end found for a key is kept, so that a reminder offered again each minute keeps the end of the day it first waited in. Every source's mark (a reminder's claim in its ledger, the end of work's day told, the sites' news named, the language offered) is made only as the notice is posted.

## The transport

`sioul_sync::notify::Teller` (crates/sioul-sync/src/notify.rs): the notices of a process, **one thread and one D-Bus connection for all of them** (`teller()`, started at the first word), in place of a thread and a connection per notice. Its words go in order: `post`, `settle`, `flush` (waits until the words before it are done: a process that ends right after, `outside::to_older`), `restart`, `listen`.

- **Replacement**: a map from each notice's key to the server's number, given as `replaces_id`.
- **The Notify**: "Sioul", the number it replaces, its icon, its title, its body as the server reads it (markup written as entities, `body_text`), its buttons, the hints `suppress-sound`, `desktop-entry` (Sioul's desktop file), `urgency`, `transient` for what is to know, `image-path` for a site's own icon, and its timeout.
- **Clicks**: every notice with a thing gets the `"default"` action, which opens it. A button keyed by its act (`open`, `copy`, `do`; a second of the same act by its place, `open-2`). A press never ends the listening. A notice to do is resident: on Plasma and GNOME a click on a code's notice opens its message and leaves the notice, with Copy and Open, in place (a server that closes a notice on any press would close it). Copy takes the notice away. A teller nobody listens to (the command line) posts neither buttons nor a click of its own: the desktop then brings Sioul's window forward by itself.
- **The activation token**: the specification's `ActivationToken(id, token)`, which Plasma sends just before `ActionInvoked`, is kept and handed with the press (`Press { about, act, token }`); the window gives it to Qt (`sioul_set_activation_token`, cpp/application.cpp) before it brings itself forward, so that KWin lets it come to the front.
- **Only the server is heard**: Plasma 6.8 sends its signals to everyone (they are not addressed: `server_p.cpp`); a press said by another program than the server's unique name is not heard.
- **The ends**: a notice is taken away by Sioul at its `close_at`, when its thing is settled (`Settled`, looked at every thirty seconds: new mail's messages read or dismissed on any device, a code's message read), by `settle(about)`, and after Copy. Closed by you or by the server, it is forgotten, but a close the server sent before giving its number anew (a notice dismissed just as the next took its place: the close's serial before that of the Notify's answer). Expired at the server (`NotificationClosed` reason 1), what is to know is forgotten, what is to do is still listened to (a click in the server's history), until its thing is settled or twelve hours pass.
- **The connection and the server**: the bus's `NameOwnerChanged` for `org.freedesktop.Notifications` is heard, and the server's unique name asked before each notice: once the name changes hands (plasmashell replaced, or gone), the numbers kept are never closed nor replaced there (the new server may give them to another program's notice), and what still shows is posted again, anew, on the new server, or once one comes; a notice posted while no server owns the name is kept and posted then, while its time lasts. The connection lost, the thread waits for its words and tries again a second, two, four… a minute apart at most, never spinning; connected again, it posts again what still shows, in place of its number on the same server.
- **A new run**: what shows is written to `$XDG_STATE_HOME/sioul/notices-shown.json` at each change (the window's alone): format 1, the server's unique name and the bus's own id (`GetId`, another for each bus started: a server's unique name may come again after a new login), then each notice's number, thing, hint, kind, class, end and time. Never its words: a code's body is its secret. At the window's start (`tell::start`), the previous run's notices are closed where the same server on the same bus still shows them (a notice of a run gone hears nothing: its click would open nothing), and what is still to do among them is told again by its source: a code not expired, read again from its message (`backend::retell_codes`), an event's alarm before its time is over (`remind::retell`). Checked upstream (plasma-workspace, branch Plasma/6.8, `libnotificationmanager/server_p.cpp`): `ServerPrivate::CloseNotification` closes any number, whichever connection asks; it never checks who posted it.
- **Windows and macOS**: through notify-rust, as far as each carries it, every notice sent at once on the teller's one thread, its words alone, nothing taken away. macOS: mac-notification-sys hears a button only by blocking a thread and running a half-second timer on the main loop for each notice until it is cleared from the Notification Center (notify-rust's `wait_for_action`), and notify-rust's UNUserNotificationCenter side is a preview: so no button there (the closing notices of the day lose theirs on a Mac: the status line and the tray offer the same), and a click on a notice brings Sioul forward, as macOS does by itself. Windows: the window gives a notice with a thing to Sioul's icon in the tray instead, whose message a click opens (`tray_notice`, main.qml's `onTrayNotice`); since a click says nothing of which message it was, with another told in the last ten minutes a click opens neither, never the wrong one; where the tray shows no message, the notice is posted through the teller as it was decided (`tell::plain`), never decided again. Android: none here.

**Doses keep their own path**, untouched: `remind_dose` and its tries on threads and connections of their own, the "Taken" press, the record of what became of each try, their withdrawal once marked anywhere ([health.md](health.md)), and the question on doses due while Sioul was closed (`remind`). They go at critical urgency, as before, which passes the desktop's do-not-disturb; they already behave as notices to do. They move onto the common call only if you ask.

## The window

`crates/sioul-app/src/tell.rs`: `tell(notice)` (the matrix's word for one of Sioul's own; told for mail and the minor ones), `tell_as(notice, output, settled)`, `tell_with(notice, output, settled, claim, keep)` (the source's mark made just before it is posted; `keep`, what waits kept here and told again at the window's minute, `again`; without it, its source offers it again, as the reminders do each minute), `settle(about)`, `moment()`, `start(qt)`, `plain(title, body)`; each answers its `Fate`: `Posted`, `Waits`, `Over` (no longer worth telling: a reminder is then marked told without a word), `Dropped`. A phone posts none here.

**One route**: a press reaches the window as `reminder_opened("", address, hint)`, and main.qml's `open(address, hint)` sends each scheme to the route that shows it: `sioul:move` → "Where I stopped" (`askNeed("")`); `sioul:need/<block>` → its question (`askNeed(block)`); `sioul:day/…` → the day's sheet (`reviewDay`); `sioul:site/<id>#<key>` → the site, then the kept web notice's own `click()` (the site opens the conversation), the site alone when it is gone; `mid:` and `sioul:mail/` → the message in Mail's reader; `sioul:event/…` → its card at that time; `sioul:dose/<key>` → the doses' route, as before; `sioul:mailbox`, `sioul:porch`, `sioul:settings/…`; any other `sioul:` address → where it lives (`openThing`, by the address's first word). Tested in crates/sioul-app/tests/qml/tst_noticeopen.qml.

**The sources on a computer**:

| Notice | Source | Kind, class | About | Ends |
|---|---|---|---|---|
| Time to move | health.rs `movement_tick` | move, to know | `sioul:move` | its minutes; only while someone is at the computer (away, the pause is counted again from the return) |
| A meal's heads-up and its notice at its time; the night's | health.rs `needs_tick`, reviews.rs `show_notice` | needs, to know | `sioul:need/<block>` | its minutes; the one at its time in the heads-up's place |
| Work hours are over | reviews.rs `tick` | work-over, to know | `sioul:day/work:<date>` | its minutes, or the work day closed |
| Dates, waits, payments, papers, contracts | remind.rs, `Reminder::notice` | dates, to know | its task, budget's line, paper, contract; a money watch's warning `sioul:budget/#<its key>` (two at once are two notices) | its minutes |
| The time to leave | remind.rs | before, to know | `sioul:event/<UID>#<start>` | its event's start |
| An event's own alarm | remind.rs | alarms, to do | `sioul:event/<UID>#<start>` | a quarter of an hour after its event began |
| A code | backend.rs `notify_codes`, `CodeView::notice` | codes, to do | its message | expired, copied, or its message read or dismissed |
| A site's message; a call in a site | sites.rs `notified`, SitesPage.qml | sites-live to know; site-calls to do | `sioul:site/<id>#<key>` | its minutes; a call when the site withdraws it, three minutes at most |
| Your sites have news | sites.rs `gather_tick` | sites, to know | `sioul:porch` | its minutes |
| New mail | mailnote.rs `show` | mail, to know | its one message, else `sioul:mailbox` | its minutes, or each message settled |
| A language found in your mail | languages.rs | minor, to know | `sioul:settings/words.languages` | its minutes |
| Calendar files for an older Sioul | outside.rs `to_older` | minor, to know | none (the process leaves) | its minutes |

**Sites**: every message a site notifies is kept for the Porch and the next gathering, told or not. The Sites page keeps a web notice (`QWebEngineNotification`, which Qt gives to JavaScript and would collect) only when Sioul told it (`siteNotified` answers whether it posted or waits), by its site and its key (`webNotices`): its tag, "" without one, so that the next takes the last one's place, as the same tag does; it goes once its site is opened, or after an hour. A click on Sioul's notice opens the site, then calls the kept notice's `click()`. Its `closed` (the site withdrew it) settles Sioul's notice for a call alone (answered, or over: `sites::withdrawn`); a message's stays its minutes, since some sites close their own after a few seconds. The notice carries the site's own icon from the cache of icons. **"Your sites have news"** names what is new since it last rang: each site's notification is marked told once named, as the notice is posted (`sites::Notice::told`, a key an older Sioul passes over); the Porch keeps them all until the site is seen. A call in a site pierces whenever its cell lets it come (`Kind::pierces`), as what you set or asked for does.

**The agenda leaves the notifications** (decision 3): the evening reminder of tomorrow's events is no longer made, on any device (`reminders::event_reminders`); `[reminders] events` is no longer read, its setting gone. Today's and tomorrow's events stay on the Porch and the widgets. The event's own alarm rings on the computer.

**What the window says of it**: Settings ▸ What reaches you's cards and its sentence on now say, on a computer, what is to know as shown without a notification at a pause, and under a do-not-disturb (Free time and the night when they turn it on), whatever its cell says (`reaches::known_only`). Always through's mail is still said by its row there.

**The command line**: `sioul remind --watch` and the accounts' codes post through the same transport, decided the same way, without buttons, someone counted at the computer as its session says (`sioul_sync::power`, unknown counting as someone); a reminder that waits is offered again each minute, marked told only as it is posted, the end of its relevance kept by its key.

## Tests

- crates/sioul-core/src/notice.rs: each kind in its class; informative notices for their minutes, transient, never to an empty desk, in a pause or under a do-not-disturb, waiting then while worth telling, each kind's end of relevance; the time to leave until its event; actionable ones until their end, critical only when piercing a do-not-disturb on now, held otherwise; one notice per kind and thing; the lifetime read and bounded, an older configuration without it read; `told` piercing as its kind.
- crates/sioul-sync/src/notify.rs (`notices_go_as_their_class_says_and_hear_every_press`, against a stand-in server on the test's own bus): replacement, transient, the timeout, the urgency, the close at the lifetime's end, the default action, the activation token, a press from another program not heard, listening after expiry for what is to do, a body click that leaves the other buttons alive, Copy taking it away, critical only when piercing a do-not-disturb on now, `settle`, the settled thing, a site's picture, a teller nobody listens to, and the next run closing the previous one's notices and giving back what is still to do; the file of what shows read in its own format only; the buttons keyed by their act; the doses' `remind` as before. `what_shows_is_posted_again_on_a_server_started_anew`: the server's name changing hands, what shows posted again anew on the new one, nothing closed there; a notice posted while no server owns the name kept and posted once one comes.
- crates/sioul-core/src/sites.rs: the gathered notice names each notification once. crates/sioul-core/src/reminders.rs: no evening reminder; each money watch's warning a notice of its own; a due reminder not marked until claimed. crates/sioul-core/src/attention.rs: a call in a site pierces when its cell lets it come. crates/sioul-app/src/remind.rs and backend.rs: a reminder's and a code's notice, its thing, its hint, its end, its buttons.
- crates/sioul-app/tests/qml/tst_noticeopen.qml: `NoticeOpener.qml`, main.qml's `open(address)`, sends each scheme to its route.

**Not testable here**: Plasma itself (the popup's look, its history, KWin taking the token), GNOME, macOS and Windows; a site's own `click()` and `closed` (Proton, Discord); the real session's presence.
