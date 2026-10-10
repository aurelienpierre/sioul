# Start here: a map of the code

This page is for developers who know how to program but have not worked with Rust, Qt Quick (QML), CXX-Qt, Fluent or Android. It says what Sioul is made of, how a click in the window becomes a change on disk and a sentence on the screen, what runs in the background, where to start reading, and what Sioul's own words mean.

The guides after it go further, one subject each:
- **[Rust, as Sioul uses it](rust.md)**: the workspace, the crates, errors, files, time, threads and tests.
- **[Qt Quick and QML, as Sioul uses them](qt-quick.md)**: the pages, their properties and bindings, how they are made, desktop and phone.
- **[The bridge: CXX-Qt](cxx-qt.md)**: how QML calls Rust, and how Rust's results come back.
- **[Strings: Fluent](fluent.md)**: how every sentence is written, in English and French.
- **[Android, as Sioul uses it](android-basics.md)**: the app, its processes, its services, and Java talking to Rust.
- **[A feature, end to end](end-to-end.md)**: one small feature followed through every layer, and the checklist for adding one.

How to build and run Sioul: [Building and running](building.md). Why each part is as it is: the design notes beside these guides, starting with [the design](design.md) and [architecture](architecture.md).

## What Sioul is made of
Sioul is one Cargo *workspace* (Cargo is Rust's build tool; a workspace is a set of packages built together, declared in the root `Cargo.toml`). It holds five *crates* (Rust packages), and the files they build in or read.

| Path | What it is |
|---|---|
| `crates/sioul-core/` | A Rust library: the rules and the words. It reads the plain files (mail, tasks, notes, settings), decides what to show and writes it in your language. It also holds the part of the spam filter every device runs (`src/spam/`: the tokenizer, the messages' features, the table that scores them). It has no network code and no window. |
| `crates/sioul-sync/` | A Rust library: what talks to the world. Finding servers, the system keyring, IMAP and SMTP, CalDAV and CardDAV, Google, GitHub, Bitwarden, the antivirus, reading scans, notifications, sharing between your devices. |
| `crates/sioul-cli/` | The `sioul` program, the command line, with `sioul mcp`, the server for AI agents ([mcp.md](mcp.md)). |
| `crates/sioul-learn/` | A Rust library, for computers only: the spam filter's training on your own mail, which writes the table every device reads. Still being built. |
| `crates/sioul-app/` | The window. `src/`: its Rust side, the object QML talks to and the work it starts. `qml/`: the pages. `cpp/`: a little C++ (Qt WebEngine's set-up, the PDF writer, line spacing, the window's icon, the warm-up at the start, quitting cleanly on SIGTERM, and the application the system tray needs). `icons/` and `fonts/`: bundled through Qt resource files. `build.rs`: the list of QML files. It is a library too, so that Android's program can run it. |
| `crates/sioul-app/qml/android/` | The two pages Android replaces, `SitesPage.qml` and `PdfView.qml`: Qt WebEngine and Qt PDF do not exist there. |
| `crates/sioul-app/qml-desktop/` | `Tray.qml`, the icon in the system tray, on computers only. |
| `crates/sioul-app/tests/qml/` | The QML tests (`tst_*.qml`): components on a stand-in for Sioul; `tools/qml-test.sh` runs them ([building.md](building.md#the-checks-before-a-commit)). |
| `crates/sioul-core/locales/` | Every sentence: `en/sioul.ftl` and `fr/sioul.ftl`, Fluent files built into the programs. |
| `crates/sioul-core/data/` | `crisis-lines.toml`, the emergency numbers and crisis lines, by country: the pause's screen shows them ([pauses.md](pauses.md)), and calls from them always ring ([android.md](android.md#calls)). |
| `crates/sioul-core/tests/` | The core's integration test (`porch.rs`), and invented mail and notes in `fixtures/`, read by the tests and by `examples/demo.toml`. |
| `android/` | The phone's app: `CMakeLists.txt` builds the APK around the window's Rust library; `main.cpp` is the program Android starts, and the glue between Java and Rust; `package/` holds the manifest, the Java classes (`src/com/aurelienpierre/sioul/`) and Android's resources (`res/`); `jvm-checks/`, the Java's pure parts checked on a computer's JVM against Rust's ([android.md](android.md#building-it)). |
| `presets/sites.json` | The sites Sioul offers to add (banks, offices, chats), built into the core (`sioul_core::presets`); `presets/research/` holds the lists they come from. |
| `data/` | The desktop file, the AppStream file and the icon's drawings. |
| `examples/` | A commented `config.toml`, a notes folder's projects' and budgets' files, and `demo.toml`, the command line on invented mail. |
| `tools/` | Checks and helpers: `final-pass.sh` (every check before a commit), `check-messages.py` (the strings), `lint-qml.sh`, `check-plain-text.py` and `qml-test.sh` (the QML; `qml-stand-in.py`, the web server one QML test asks), `check-presets.py` and `check-sites.py` (the sites), the makers of icons and of the symbols font, stand-ins of Google's and GitHub's APIs for tests, `measure-load.py`, `demo/` (an invented profile, `run.sh`, which starts the window on it for a check, and the website's pictures) and `e2e/notifications/` (a notification end to end). |
| `packaging/` | The Flatpak, AppImage, Windows and macOS files, used by `.github/workflows/release.yml`. |
| `.github/workflows/` | `build.yml` (Linux, Windows, macOS), `android.yml` (the APK), `release.yml` (the packages), `pages.yml` (this website). |
| `docs/` | The design notes and these guides; `docs/research/`, the research behind them. |
| `website/` | The website: the user guide in English (`website/docs/`) and French (`website/fr/docs/`). Its "For developers" section is `docs/`, copied at each build by `website/build.sh`. |

How the crates depend on each other:
- **`sioul-core`** uses no other crate of Sioul's.
- **`sioul-sync`** uses `sioul-core`.
- **`sioul-learn`** uses `sioul-core` and `sioul-sync`.
- **`sioul-cli` and `sioul-app`**, the two programs, use the core and sync, and `sioul-learn` on computers only (Android's build never compiles it); the window also uses Qt, through CXX-Qt.

`cargo build` at the root builds the core, sync, the command line and `sioul-learn`; the window is built only when asked for (`cargo build -p sioul-app`), because it needs Qt's development files ([building.md](building.md)).

## The rule of the layers
**Rust decides and words; QML lays out.** The rules, and the sentences, are written in Rust: in `sioul-core` above all (`view.rs`, `taskview.rs` and their neighbours), so that the command line, the window and AI agents see the same thing ([architecture.md](architecture.md)). The window's own Rust side (`crates/sioul-app/src/`) gathers what the pages need, starts the slow work, and words a few lines of its own with the same translator. The QML shows the text it is given, chooses what goes where on the screen, and calls Rust when you act. When you look for where something is decided, look in `crates/sioul-core/src/` first.

**Plain files hold the data.** Mail is in Maildir folders; events, tasks and contacts are one `.ics` or `.vcf` file each; notes are Markdown; settings and Sioul's own state are TOML ([architecture.md](architecture.md), "Storage"). There is no database: each module reads the files it needs, and writes them back whole, or edits them in place, keeping what it does not know (your comments, other programs' fields).

## How a click becomes an action and a sentence
The Porch's **Done for now** button closes the Porch on what it showed. Followed through each layer:

1. **QML** (`crates/sioul-app/qml/PorchPage.qml`): the button's label is a sentence asked of the core by its name, and its click calls the bridge object, which every page receives as `sioul`:

    ```qml
    Button {
        visible: page.view.open && page.view.lanes.length > 0
        Layout.topMargin: page.theme.gap
        text: page.sioul.text("ui-done")
        onClicked: {
            page.openKey = ""
            page.sioul.done()
        }
    }
    ```

2. **The bridge** (`crates/sioul-app/src/backend.rs`): `done` is a Rust method that QML can call (an *invokable*, [cxx-qt.md](cxx-qt.md)). It runs on the window's thread. It loads the Porch's memory, marks the newest message shown for each account, saves it, and puts a sentence in the status line:

    ```rust
    fn done(mut self: Pin<&mut Self>) {
        let shared = self.shared();
        let newest = shared.shown.lock().map(|s| s.clone()).unwrap_or_default();
        let path = PorchState::default_path();
        let mut state = PorchState::load(&path);
        state.close(&newest);
        let line = match state.save(&path) {
            Ok(()) => tr().text(if newest.is_empty() { "done-nothing" } else { "done-closed" }, None),
            Err(e) => e,
        };
        shared.opened_anyway.store(false, Ordering::Relaxed);
        self.as_mut().set_status(QString::from(&line));
        show(&self.qt_thread(), &shared);
    }
    ```

3. **The core** (`crates/sioul-core/src/state.rs`): `PorchState` is a plain Rust struct that serde turns into TOML. `save` writes `porch.toml` in Sioul's state folder (`sioul_core::config::state_dir`: `$XDG_STATE_HOME/sioul`, on Linux `~/.local/state/sioul` when that variable is not set): beside it first, then renamed over it, so that a crash never leaves half a file.
4. **The sentence** (`crates/sioul-core/locales/en/sioul.ftl`): `tr().text("done-closed", None)` looks the message up in the language of the session, through Fluent ([fluent.md](fluent.md)): "Done. What comes next waits for the next window." in English, « C’est fait. Ce qui arrive maintenant attend la prochaine fenêtre. » in French. Setting the `status` property tells QML, and the status line shows it.
5. **Back to the screen**: `show` computes the Porch again on another thread (`compute`, which calls `sioul_core::porch::gather` and `sioul_core::view::porch`), turns the result into JSON, and hands it back to the window's thread, which sets the `porch` property. `PorchPage.qml` hears that the property changed (`onPorchChanged`), reads it, parses it with `JSON.parse`, and every binding that reads it draws again: what was shown has left the lanes, and only what came after it stays.

Every feature follows this path: QML calls an invokable; the invokable asks the core (and, for the network, sync); the core reads and writes plain files and returns facts already in words; the result comes back as a property (usually a JSON text) or as the invokable's return value; QML lays it out. [A feature, end to end](end-to-end.md) follows a whole feature this way, with its test and its documentation.

## What runs in the background
- **The window's thread** draws, runs QML and runs every invokable. Nothing slow may run there: the window would freeze.
- **Threads for the pages**: each set of pages (the Porch and budgets, mail, the agenda and contacts, tasks and notes, health) is computed on a thread of its own, one computation at a time, then handed back to the window's thread (`backend::coalesced`, [cxx-qt.md](cxx-qt.md)).
- **One watcher per mail account**: a thread that keeps the inbox open with IMAP IDLE and fetches what arrives (`backend::start_watcher`, `sioul_sync::watch`). Calendars and contacts have their own (`pim::start_dav_watchers`).
- **The window's minute**: `qml/Clock.qml` turns at each minute, and `main.qml` then calls `sioul.refreshMode()` (`refresh_mode` in `backend.rs`), which starts the minute's work: doses and errands to remind, the pauses, the exchange with your other devices, reminders before dates, new mail's notification at its time, the end of the work day, the weather.
- **Without the window, on a computer**: the window can hide in the system tray and go on. `sioul remind --watch` tells reminders with the window closed (started with the session when asked in Settings), and `sioul watch` keeps the inboxes open from a terminal ([building.md](building.md), "First steps").
- **Without the window, on a phone**: Android freezes an app it does not show. Alarms given ahead to Android start receivers that load Sioul's Rust library without its window; a service in a process of its own keeps the phone in step with your other devices; calls and other apps' notifications are screened in processes of their own ([android-basics.md](android-basics.md), [android.md](android.md)).
- **async only in the network code**: IMAP, SMTP and DNS are asynchronous (tokio) inside `sioul-sync`, but its functions that reach a server make their own small runtime and wait for it, so their callers see ordinary blocking functions ([rust.md](rust.md#threads-and-async)).

## Where to start reading
1. **`crates/sioul-core/src/lib.rs`**: the list of the core's modules. Each module begins with a comment (`//!`) that says what it does, often with the design note it follows.
2. **`crates/sioul-core/src/porch.rs` and `view.rs`**: how mail is judged and sorted, and how the Porch becomes words. The test `crates/sioul-core/tests/porch.rs` sorts a morning of invented mail and checks every lane.
3. **`crates/sioul-app/src/lib.rs`**: how the window starts (`run`).
4. **The top of `crates/sioul-app/src/backend.rs`**: the bridge, the list of what QML can read and call.
5. **`crates/sioul-app/qml/main.qml`**: the window, the places on the left, the pages made as they are first shown; then any page, such as `PorchPage.qml`.
6. **`crates/sioul-core/locales/en/sioul.ftl`**: a message's name in QML or Rust leads to its words here.
7. **For the phone**: `android/main.cpp`, then `android/package/AndroidManifest.xml`, then the Java class the manifest names for what you look at.

To find where something on the screen is made, search its words in `en/sioul.ftl`, take the message's name, and search that name in `crates/` (`grep -rn '"done-closed"' crates/`).

## Sioul's words
Sioul has its own names for its parts. The design notes use them without explaining them each time.

| Word | What it is | More |
|---|---|---|
| **the Porch** | Where new mail waits, checked and sorted, until you look; also the codes you asked for, today's doses, where you stopped. The first place of the window. | [porch.md](porch.md) |
| **lanes** | The Porch's groups: Right now, Set aside, one per project, Screener, Filed, People, less important accounts, and two for a shielded public address (`sioul_core::porch::Lane`). | [porch.md](porch.md) |
| **Right now** | The lane of short-lived secrets (one-time codes, password resets, sign-in links): shown at once, whatever the time. | [porch.md](porch.md) |
| **the screener** | The lane of senders you do not know yet: they wait until you let them in. | [porch.md](porch.md) |
| **a project** | Any matter you follow, for a client or your own: a record in your own Markdown notes, its routes (which mail belongs to it), its tasks and events (`sioul_core::projects`, `sioul-projects.toml`). Older notes and files call it **a case** (`sioul-cases.toml`, `[[case]]`, `sioul:case/`): the same thing, still read. | [notes-folder.md](notes-folder.md) |
| **the notes folder** | Your folder of Markdown files, which holds the projects, budgets, papers and notes as plain files; older notes call it **the case store**, and the configuration's key is still `case_store`. | [notes-folder.md](notes-folder.md) |
| **admin windows** | The older word for working hours and hours for your admin. "Free time" in older notes meant every other time, now **leisure**; **Free time** is now one of the two pauses. | [areas.md](areas.md) |
| **areas** | What a thing is for: work, your own admin, leisure, as three switches (`sioul_core::areas::Area`). | [areas.md](areas.md) |
| **the five times** | What now is for: work, admin, meals, sleep, leisure (`sioul_core::areas::Time`, decided by `sioul_core::quiet::mode`). | [areas.md](areas.md) |
| **reach** | Who may reach you, on which channel (mail, calls, other apps' messages), at which time: safe, neutral, restricted, strangers, blocked, and a matrix per channel (`sioul_core::reach`). | [porch.md](porch.md), "Who may reach you, and when" |
| **the pauses** | **Free time** (« Temps libre »), leisure whatever the hour, and **Pause** (« En pause »), which holds everything on every device (`sioul_core::pause`). | [pauses.md](pauses.md) |
| **do-not-disturb** | Sioul's own, shared by your devices: its switch, a focus session, sleep, the pauses (`sioul_core::everywhere`). | [do-not-disturb.md](do-not-disturb.md) |
| **the plan** | When each task could happen, computed from the tasks each time, never stored (`sioul_core::plan`). | [tasks.md](tasks.md) |
| **Now** | The one next step the plan picks, with its reason (Tasks ▸ Now). | [tasks.md](tasks.md) |
| **the day's weather** | How you say today is, clear, haze or fog: it sets how much the plan puts in the day (`sioul_core::today`). | [tasks.md](tasks.md) |
| **costs, gain and margins** | What a task or an event asks of you (four costs, 0 to 10), what it gives back, and the time kept before and after it (`sioul_core::demands`). | [capacity.md](capacity.md) |
| **needs** | Meals, naps and the night, each a *block* of the day, set first and kept free in the plan (`sioul_core::needs`). Not `sioul_core::blocks`, which is tasks pinned to a time. | [health.md](health.md) |
| **where you stopped** | One line left when something interrupts you, shown again until you say it is done (`sioul_core::stopped`). | [end-to-end.md](end-to-end.md) |
| **Done for today** | Closing the work day, or the whole day, at any hour. | [reviews.md](reviews.md) |
| **the status line** | One sentence about what happened last, with "Undo"; the title bar on a computer, the bottom of the screen on a phone. | [design.md](design.md) |
| **the places** | The column of icons on the left of the window, one per page. | [design.md](design.md) |
| **sites** (older: **portals**) | Web-only mailboxes, chats and other sites, kept logged in on a computer, each in its own browser profile; on a phone, opened in the browser. | [sites.md](sites.md) |
| **the sharing** | Your devices exchanging what only Sioul keeps, through a folder your sync app carries, sealed with your passphrase (`sioul_sync::share`). | [database.md](database.md) |
| **parts** | What the sharing carries, switched on part by part: settings, senders, health, time… | [database.md](database.md) |
| **leases** | One device at a time for some work (reminding medicines, numbering invoices), claimed through the sharing folder (`sioul_sync::lease`). | [database.md](database.md) |
| **links**, **ties** | Links between everything: mail, tasks, events, contacts, notes, drafts, budget lines (`sioul_core::links`). | [tasks.md](tasks.md) |
| **the shield** | Reading the tone and topic of mail to a public address before you see it (`sioul_core::shield`). | [porch.md](porch.md) |
| **papers**, **letters** | The papers wallet (documents with their dates of renewal), and paper letters read from scans. | [papers.md](papers.md) |
| **the card on the home screen** | Sioul's Android widget, written by Rust, drawn by Java. | [android.md](android.md) |
