# A feature, end to end

This page follows one small, real feature of Sioul through every layer of the code, with each file and function it touches: "Where you stopped". Then it gives the checklist for adding a feature of your own. Read [Start here](start-here.md) first for the map; the guides on [Rust](rust.md), [Qt Quick and QML](qt-quick.md), [the bridge](cxx-qt.md) and [Fluent](fluent.md) explain the words used below.

## The feature
When something interrupts you (a pause to move, a meal, the night, a timer stopped), you can leave one line on where you were. The line shows again at the top of the Porch and of the Tasks page, and in the focus window, until you press **Done**; your other devices show it too. **New ▾ ▸ Where I stopped…** leaves one at any time. The user guide describes it in [Tasks, "Starting, and stopping"](https://aurelienpierre.github.io/sioul/guide/tasks.html#starting-and-stopping).

The path, in one table:

| Layer | File | What it holds |
|---|---|---|
| The rule and the file | `crates/sioul-core/src/stopped.rs` | `Stopped`: `load`, `keep`, `default_path`; the test `a_line_left_then_done` |
| The window's Rust | `crates/sioul-app/src/work.rs` | `stopped` (the line as JSON, in words), `set_stopped` (leaves it, then shows the pages again) |
| The bridge | `crates/sioul-app/src/backend.rs` | the invokables `stopped` and `set_stopped` |
| The pages | `crates/sioul-app/qml/StoppedCard.qml`, `Interruption.qml`, `NewMenu.qml`, `FocusWindow.qml`, `main.qml` | the card, the dialog, the menu's entry, the focus window's line |
| The words | `crates/sioul-core/locales/en/sioul.ftl`, `fr/sioul.ftl` | `stopped-at`, `stopped-done`, `stopped-title`, `stopped-hint`, `new-stopped`, `stopped-note`, `task-stopped` |
| Your other devices | `crates/sioul-sync/src/share.rs` | `state/stopped.toml` in the part "time" |
| The design note | `docs/tasks.md` | "Places", "Where you stopped" |
| The user guide | `website/docs/guide/tasks.md`, `website/fr/docs/guide/tasks.md` | "Starting, and stopping"; « Commencer, et s’arrêter » |

## 1. The core: the rule and the file
`crates/sioul-core/src/stopped.rs` holds everything the feature decides: what a line is, where it is kept, and that an empty line means done. The module is declared in `crates/sioul-core/src/lib.rs` (`pub mod stopped;`), and its `//!` comment says what it is for and why, with the research behind it (a cue to resume cuts the time to get back into a task: Trafton et al. 2003; Leroy & Glomb 2018).

```rust
/// The line, when it was left, and the task it was about ("" for none).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Stopped {
    pub text: String,
    /// Unix seconds.
    pub at: i64,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub task: String,
}
```

- **`Stopped::default_path()`** is `stopped.toml` in Sioul's state folder (`sioul_core::config::state_dir()`: `$XDG_STATE_HOME/sioul`, else `~/.local/state/sioul`).
- **`Stopped::load(path)`** returns the line, or `None` when there is none, the file is missing, or it does not read.
- **`Stopped::keep(path, text, task, at)`** writes the line, or removes the file when the line is empty:

    ```rust
    /// A line left (an empty one takes it away: done).
    pub fn keep(path: &Path, text: &str, task: &str, at: i64) -> Result<(), String> {
        let stopped = Stopped { text: text.trim().to_string(), at, task: task.to_string() };
        let fail = |e: std::io::Error| format!("{}: {e}", path.display());
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(fail)?;
        }
        if stopped.text.is_empty() {
            return match std::fs::remove_file(path) {
                Err(e) if e.kind() != std::io::ErrorKind::NotFound => Err(fail(e)),
                _ => Ok(()),
            };
        }
        crate::filelock::with_lock(path, || std::fs::write(path, toml::to_string(&stopped).map_err(|e| e.to_string())?).map_err(fail))
    }
    ```

    The write is locked (`filelock::with_lock`) because the window is not the only writer: the sharing writes the same file when another device changed it.

- **Paths are arguments**, never read from inside: the tests give a temporary folder, the window gives `default_path()`.

**The test** sits at the end of the same file, in `mod tests`. `a_line_left_then_done` keeps a line with spaces around it in a temporary folder, reads it back trimmed with its task, then keeps an empty line and checks that nothing is left ([rust.md](rust.md#tests) shows it whole). Run it with `cargo test -p sioul-core stopped`. The window's crate tests one more use of the line: a timer stopped from its notification keeps the line left at its pause as its session's note (`its_buttons_pause_go_on_and_stop_once` in `crates/sioul-app/src/timenote.rs`, `cargo test -p sioul-app timenote`, which needs Qt to build).

## 2. The window's Rust: what the page needs
Two functions of `crates/sioul-app/src/work.rs` (the tasks' and notes' module of the window) gather what the pages need around the core's rule:

```rust
/// Where you stopped, as JSON: {text, when, task, title}; "null" when no line is left.
pub(crate) fn stopped(shared: &Shared) -> String {
    let Some(stopped) = sioul_core::stopped::Stopped::load(&sioul_core::stopped::Stopped::default_path()) else { return "null".into() };
    let title = loaded(shared).tasks.iter().find(|t| t.uid == stopped.task).map(|t| t.title.clone()).unwrap_or_default();
    let when = Timestamp::from_second(stopped.at).map(|t| tr().when(&t.to_zoned(TimeZone::system()))).unwrap_or_default();
    serde_json::json!({ "text": stopped.text, "when": when, "task": stopped.task, "title": title }).to_string()
}

/// A line left on where you stopped, about the task the timer runs for, if
/// any; an empty line: done. Returns what went wrong, else "".
pub(crate) fn set_stopped(qt: &QtThread, shared: &Arc<Shared>, text: &str) -> String {
    let task = timelog::running().map(|r| r.task).unwrap_or_default();
    let problem = sioul_core::stopped::Stopped::keep(&sioul_core::stopped::Stopped::default_path(), text, &task, Timestamp::now().as_second()).err().unwrap_or_default();
    show_work(qt, shared);
    crate::backend::show(qt, shared);
    problem
}
```

- **`stopped`** adds what the file does not hold: the task's title, from the tasks as last read (`work::loaded`), and the time in words, in your language (`Translator::when`). QML receives a sentence's parts, never a timestamp to format.
- **`set_stopped`** ties the line to the task the timer runs for, if one runs (`sioul_core::timelog::running`), keeps it, then asks for the tasks' page and the Porch to be computed again, off the window's thread (`show_work`, `backend::show`, [cxx-qt.md](cxx-qt.md#which-thread-does-what)). When they come back, the properties `tasks` and `porch` change, which the card hears (below).
- **What went wrong** comes back as a sentence, empty when all went well: here the core's own line, the file's path and the system's error.

Other parts of the window read the same file: the focus window's session (`focus_json` in `work.rs` gives it the line left at a pause of this session, else the last session's note), the end of a session (`focus_stop` keeps the line as the session's note when no other was given), and the time running's notification (`timenote.rs`).

## 3. The bridge
`crates/sioul-app/src/backend.rs` declares the two functions in the `Sioul` object's bridge, each with a `///` line saying what it returns ([cxx-qt.md](cxx-qt.md#how-it-is-declared)):

```rust
/// Where you stopped, as JSON: {text, when, task, title}; "null" for none.
#[qinvokable]
fn stopped(self: &Sioul) -> QString;

/// A line left on where you stopped (an empty one: done); returns what went wrong, else "".
#[qinvokable]
fn set_stopped(self: Pin<&mut Sioul>, text: &QString) -> QString;
```

and implements them further down, in `impl qobject::Sioul`, by converting the text and calling `work.rs`:

```rust
fn stopped(&self) -> QString {
    QString::from(&crate::work::stopped(&self.shared()))
}

fn set_stopped(self: Pin<&mut Self>, text: &QString) -> QString {
    QString::from(&crate::work::set_stopped(&self.qt_thread(), &self.shared(), &text.to_string()))
}
```

`stopped` only reads, so it takes `&Sioul`; `set_stopped` starts work that will set properties, so it takes `Pin<&mut Sioul>` and hands that work the handle back to the window's thread (`qt_thread()`). With `#[auto_cxx_name]`, QML calls them `stopped()` and `setStopped(text)`.

## 4. The pages
**`crates/sioul-app/qml/StoppedCard.qml`** shows the line. Its top comment says what it is and names the design note. It asks the bridge for the line, keeps it parsed, and is visible only while there is one ([qt-quick.md](qt-quick.md#a-qml-file-is-a-type) shows its start). Its words are asked by name, and **Done** keeps an empty line:

```qml
Label {
    Layout.fillWidth: true
    text: card.stopped === null ? "" : card.sioul.textWith("stopped-at", "when", card.stopped.when)
    textFormat: Text.PlainText
    wrapMode: Text.Wrap
    font.weight: Font.DemiBold
    color: card.theme.text
}
…
Button {
    text: card.sioul.text("stopped-done")
    onClicked: {
        card.sioul.setStopped("")
        card.reload()
    }
}
```

- **It reads itself again** when the bridge's `tasks` or `porch` property changes (a `Connections` on `card.sioul`, [qt-quick.md](qt-quick.md#signals-and-handlers)): the line may have been left or cleared elsewhere, in the focus window, on another page, on another device.
- **The task's button** opens the task the line was about, through the main window: `card.window.openTask(card.stopped.task)`.
- **Two pages make it**, `PorchPage.qml` and `TasksPage.qml`, each giving it `sioul`, `theme` and `window`, its required properties.

**`crates/sioul-app/qml/Interruption.qml`** is the dialog that leaves a line, alone or with a meal's, a nap's or the night's question (later, at another time, not today). It writes only when the line changed, so that closing it untouched never clears a line:

```qml
// The line kept when it changed; an empty one is "done" only when typed so.
function keepLine() {
    const stopped = JSON.parse(dialog.sioul.stopped() || "null")
    if (line.text.trim() !== (stopped ? stopped.text : ""))
        dialog.sioul.setStopped(line.text)
}
```

**How the dialog is reached**: `main.qml` makes it the first time it is asked (`askNeed`, which gives the file to a `Loader` with `setSource`, [qt-quick.md](qt-quick.md#made-when-first-needed)); `askNeed("")` asks for the line alone. Three ways lead there:
- **New ▾ ▸ Where I stopped…**: `NewMenu.qml` lists `["stopped", "new-stopped", "bookmarks-organize"]` (the kind, its message, its icon) among its `kinds`, and `main.qml`'s `newThing("stopped")` calls `window.askNeed("")`;
- **the pause to move's notification**, on Linux: its button, worded by `stopped-note` (`crates/sioul-app/src/health.rs`), brings the window up on the same dialog (the kind `"stopped"` in `main.qml`);
- **a meal's, a nap's or the night's question**, which opens the same dialog with its block (`askNeed(key)`).

**`crates/sioul-app/qml/FocusWindow.qml`** has its own field for the line when you take a pause during a session (`takeBreak`, then `setStopped` in `backFromMoving`), and shows the session's line under the task (`task-stopped`).

**`crates/sioul-app/build.rs`** lists `qml/StoppedCard.qml` and `qml/Interruption.qml` among the module's files: a QML file missing from that list is not in the window ([cxx-qt.md](cxx-qt.md#buildrs-what-the-module-holds)).

## 5. The words
Each sentence is a message in both Fluent files, at the same place in each ([fluent.md](fluent.md)). In `crates/sioul-core/locales/en/sioul.ftl`:

```ftl
stopped-hint = Where I stopped, in one line, for when I'm back
stopped-title = Where you stopped
stopped-done = Done
stopped-at = Where you stopped, { $when }
```

and in `crates/sioul-core/locales/fr/sioul.ftl`:

```ftl
stopped-hint = Où j’en suis, en une ligne, pour quand je reviens
stopped-title = Où vous en étiez
stopped-done = C’est fait
stopped-at = Où vous en étiez, { $when }
```

- **`stopped-at`** is the card's heading; `$when` is given by the card, which got it from Rust already in words.
- **`stopped-done`** is the card's button; **`stopped-title`** the dialog's title when it asks for the line alone; **`stopped-hint`** the words above its field, and the focus window's placeholder.
- **Elsewhere in the files**: `new-stopped` (the entry of **New ▾**), `stopped-note` (the notification's button), `task-stopped` (the focus window's line, with `$text`).
- **The French typography** holds: the apostrophe `’`, and the narrow no-break space before the colon of `task-stopped`. `tools/check-messages.py` checks both, and that every name the code asks for exists in each language.

## 6. On your other devices
The line is one of the files the sharing carries ([database.md](database.md)): `stores_of` in `crates/sioul-sync/src/share.rs` lists it in the part "time", with the time noted and the day's choices:

```rust
file("time", "state/stopped.toml", s.join("stopped.toml"), Shape::Toml(&PLAIN_RULES)),
```

When another device leaves or clears the line, the exchange writes `stopped.toml` here (under the lock the core takes too), the pages are computed again, and the card reads itself again. The user guide's [sharing page](https://aurelienpierre.github.io/sioul/guide/sharing.html) lists it under "Time".

## 7. The documentation
- **The design note**: `docs/tasks.md`, in "Places", the item "Where you stopped": what it does, where you meet it, the research behind it, and where it lives in the code (`sioul_core::stopped`, `StoppedCard.qml`). Design notes say why; their names lead from the code to them and back.
- **The user guide, in English**: `website/docs/guide/tasks.md`, "Starting, and stopping", the paragraph **Where you stopped.**; and a line each in `porch.md` (what the Porch shows), `first-steps.md` (the **New ▾** menu) and `sharing.md` (what "Time" carries).
- **The user guide, in French**: `website/fr/docs/guide/tasks.md`, « Commencer, et s’arrêter », the paragraph **Où vous en étiez.**, and the same lines in the same pages. Each French heading keeps the English page's anchor (`## Commencer, et s’arrêter {#starting-and-stopping}`), so that a link to `tasks.md#starting-and-stopping` works in both languages.
- **The comments**: the module's `//!`, the functions' `///`, the QML file's top comment: they make the [API reference](https://aurelienpierre.github.io/sioul/dev/api.html).

## Adding a feature: the checklist
1. **Say why first.** The design note it belongs to, in `docs/`: what it does, what it never does, the research or the reason behind it ([design.md](design.md)). A new note is added to the "For developers" navigation in `website/zensical.toml`.
2. **The core**: the rule, in `crates/sioul-core/src/` when it decides or words anything, so that the command line, the window and AI agents share it ([architecture.md](architecture.md)). A `//!` comment for a new module, a `///` line for each public item, paths and times as arguments.
3. **Its test**, in the same file: invented data only (reserved domains such as `example.org`, fictional numbers, temporary folders), never the configuration of whoever runs it ([rust.md](rust.md#tests)).
4. **The window's Rust**: a function in the window's module it belongs to (`work.rs`, `pim.rs`, `health.rs`…). Anything slow runs on a thread and comes back through `qt.queue`; a page computed again goes through its job (`coalesced`, [cxx-qt.md](cxx-qt.md#which-thread-does-what)).
5. **The bridge**: the declaration in `backend.rs`, with a `///` line saying what it returns (for JSON, its fields), and the implementation in `impl qobject::Sioul` ([cxx-qt.md](cxx-qt.md#adding-an-invokable-end-to-end)).
6. **The QML**: a new file is listed in `crates/sioul-app/build.rs`; it starts with its licence and a comment saying what it is; it takes what it needs of `sioul`, `theme` and `window` as required properties, and asks every word by its name. Forms and dialogs are made when first needed (`Later.qml`). Check it narrow (below 720 pixels) and by touch: a long press wherever a right click opens a menu ([qt-quick.md](qt-quick.md#a-computer-and-a-phone)).
7. **The words**: every sentence in both `.ftl` files, at the same place, with French typography ([fluent.md](fluent.md#adding-a-message-in-both-languages)).
8. **Your other devices**: state that must travel is a line in `stores_of` (`crates/sioul-sync/src/share.rs`), in the part it belongs to ([database.md](database.md)).
9. **A phone**: if it must work while the window is away, Android starts it ([android-basics.md](android-basics.md#running-without-the-window)); say in [android.md](android.md) what was tried on a phone.
10. **The user guide**, in English and in French, at the same place in each, with the same anchors; new pictures of the window with `tools/demo/screenshots.sh`.
11. **The checks**:

    | Command | What it checks |
    |---|---|
    | `cargo test -p sioul-core` (and each crate you changed) | the rules |
    | `cargo build -p sioul-app`, then `cargo test -p sioul-app <name>` | the window builds, the bridge matches its declaration, the window's tests (CI does not run these) |
    | `python3 tools/check-messages.py` | every message the code asks for, in every language, and French typography (CI runs it) |
    | `tools/lint-qml.sh` | the QML, with Sioul's own types (after `cargo build -p sioul-app`) |
    | `cargo clippy -p sioul-core` | Rust's lints, on the crate you changed |
    | `SIOUL_GRAB=<folder> sioul-app`, with test folders in `XDG_CONFIG_HOME`, `XDG_DATA_HOME` and `XDG_STATE_HOME` | every page, saved as an image, on invented data ([building.md](building.md#working-on-the-window)) |
    | `website/build.sh --strict` | the design notes and the user guide build, without a broken link ([how this website is built](https://aurelienpierre.github.io/sioul/dev/index.html#how-this-website-is-built)) |
