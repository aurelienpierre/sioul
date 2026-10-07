# The bridge: CXX-Qt

QML runs JavaScript in Qt's C++ world; Sioul's logic is Rust. [CXX-Qt](https://kdab.github.io/cxx-qt/book/), by KDAB (version 0.10 here), joins them: Rust declares a Qt object, and CXX-Qt writes the C++ and Rust code that lets QML make it, read its properties, call its methods and hear its signals. This page shows how Sioul's window uses it, from `crates/sioul-app/src/backend.rs`, and how to add a method end to end. Read [Qt Quick and QML](qt-quick.md) first for the QML side, and [Rust, as Sioul uses it](rust.md) for the Rust words used here.

## One object for the whole window
Sioul's window talks to Rust through one object, of the type `Sioul`, made once in `main.qml` (`Sioul { id: sioul }`) and handed to every page ([qt-quick.md](qt-quick.md#required-properties-and-how-a-page-gets-sioul)). A second, small one, `Desktop` (`crates/sioul-app/src/desktop.rs`), says where the system puts a window's buttons, for the title bar Sioul draws itself.

The object offers QML three things:
- **Properties**: values QML reads, and is told about when they change: the Porch, the tasks, the status line… Most hold JSON text.
- **Invokables**: methods QML calls, which run Rust and may return a value.
- **Signals**: what Rust tells QML happened: a code's "Copy" pressed in a notification, a task done, a draft to open.

## How it is declared
A *bridge* is a Rust module marked `#[cxx_qt::bridge]`. CXX-Qt reads it at build time. The top of `backend.rs`, shortened:

```rust
#[cxx_qt::bridge]
pub mod qobject {
    unsafe extern "C++" {
        include!("cxx-qt-lib/qstring.h");
        type QString = cxx_qt_lib::QString;
    }

    #[auto_cxx_name]
    extern "RustQt" {
        #[qobject]
        #[qml_element]
        #[qproperty(QString, porch)]
        #[qproperty(QString, status)]
        #[qproperty(bool, busy)]
        …
        type Sioul = super::SioulRust;

        /// A sentence of the interface, in your language.
        #[qinvokable]
        fn text(self: &Sioul, id: &QString) -> QString;

        /// Shows what is on disk, then starts a watcher per mail account.
        #[qinvokable]
        fn start(self: Pin<&mut Sioul>);
        …
    }

    #[auto_cxx_name]
    unsafe extern "RustQt" {
        /// The notification's "copy" button was pressed.
        #[qsignal]
        fn copy_requested(self: Pin<&mut Sioul>, text: QString);
        …
    }

    impl cxx_qt::Threading for Sioul {}
}
```

- **`unsafe extern "C++"`** names the C++ types the bridge uses. `QString` is Qt's text; `cxx-qt-lib` wraps it for Rust (`QString::from(&text)` and `to_string()` convert).
- **`#[qobject] type Sioul = super::SioulRust;`** declares a Qt object (a `QObject`) named `Sioul`, whose data is the Rust struct `SioulRust`. **`#[qml_element]`** registers it in Sioul's QML module, so that QML can write `Sioul { }` after `import com.aurelienpierre.sioul`.
- **`#[qproperty(QString, porch)]`** makes a property. CXX-Qt writes a getter (`porch()`), a setter (`set_porch(…)`) and a signal emitted when it changes. The value lives in the field of the same name of `SioulRust`.
- **`#[qinvokable]`** makes a method QML can call. **`self: &Sioul`** only reads; **`self: Pin<&mut Sioul>`** may change the object (set properties, emit signals). *Pin* means the object stays at one address in memory, as a Qt object must.
- **`#[qsignal]`** declares a signal. Rust emits it by calling it (`sioul.copy_requested(text)`); QML handles it as `onCopyRequested`. In this code, signals are declared in an `unsafe extern "RustQt"` block of their own, invokables in a plain one.
- **`#[auto_cxx_name]`** gives everything its Qt name in camel case. Rust's `text_with`, `set_stopped`, `form_error` and `copy_requested` are `textWith`, `setStopped`, `formError` and `copyRequested` in QML; a property `tasks` is heard as `onTasksChanged`.
- **`impl cxx_qt::Threading for Sioul {}`** lets other threads send work back to the object (below).
- **`///` comments** above each item say what it does, and for a JSON text, what it holds: they are the bridge's documentation for whoever writes QML, also shown in [the API reference](https://aurelienpierre.github.io/sioul/dev/api.html), under `sioul_app::backend`.

The Rust side of the object is a plain struct, made with `Default` (hence the `derive`): one field per property, and what the window's threads share.

```rust
#[derive(Default)]
pub struct SioulRust {
    porch: QString,
    budgets: QString,
    …
    places_found: QString,
    shared: Arc<Shared>,
}
```

The methods are written in one `impl qobject::Sioul` block further down. They are short: each passes its arguments to a module of the crate (`work.rs`, `pim.rs`, `health.rs`…) or of the core, and returns what it gets back. For example:

```rust
fn stopped(&self) -> QString {
    QString::from(&crate::work::stopped(&self.shared()))
}

fn set_stopped(self: Pin<&mut Self>, text: &QString) -> QString {
    QString::from(&crate::work::set_stopped(&self.qt_thread(), &self.shared(), &text.to_string()))
}
```

`self.shared()` is the `Arc<Shared>` the threads share; `self.qt_thread()` is the handle a thread uses to come back (below).

## JSON across the bridge
The types that cross are few: `QString`, `bool`, `i32` and `f64`. Everything with a shape goes as JSON text in a `QString`, in both directions:
- **Rust to QML**: the views of `crates/sioul-core/src/view.rs` (`PorchView`, its lanes, its cards…) are serialized with serde (`backend::json`) and set as properties; small answers are built with `serde_json::json!`. QML parses them: `JSON.parse(page.sioul.overlaps || "[]")`.
- **QML to Rust**: a form's values, or a sentence's arguments, as one JSON object: `sioul.textArgs("overlap-today", JSON.stringify({ first: …, second: … }))`; `note_time(edit)` takes "JSON: day, at, minutes, project, task, note, unbilled".
- **Why**: the window reads the same views the command line prints and AI agents receive (`view.rs`: "an agent or a script can read the very same"), and JavaScript reads JSON directly, into objects and arrays that bindings and `Repeater`s use. No Qt list model has to be written for each list.
- **Answers that can fail** return the problem as text, empty when all went well: `set_stopped` returns "what went wrong, else """. QML shows it, or goes on when it is empty.

## Which thread does what
Qt runs the window on one thread, *the window's thread* (Qt calls it the GUI thread), in its event loop (`app.exec()` in `crates/sioul-app/src/lib.rs`). QML, every invokable, and every change of a property or signal of `Sioul` happen there. While an invokable runs, the window cannot draw: an invokable must return in a few milliseconds.

So anything slow (reading many files, the network, the keyring) runs on another thread, and comes back:

1. **The invokable starts the work** and returns at once. It hands the thread two things: `self.qt_thread()`, a handle (`QtThread`, an alias of `cxx_qt::CxxQtThread<Sioul>`) that may be sent to other threads, and `self.shared()`, the state threads share. The object itself never leaves the window's thread.
2. **The thread does the work** with the core and sync, which are ordinary blocking Rust ([rust.md](rust.md#threads-and-async)).
3. **The thread queues the result.** `qt.queue(closure)` asks Qt to run `closure` on the window's thread, with the object; only there may it set properties and emit signals.

`backend::show`, which computes the Porch and the budgets:

```rust
pub(crate) fn show(qt: &QtThread, shared: &Arc<Shared>) {
    mail::show_mail(qt, shared);
    let qt = qt.clone();
    coalesced(shared, |s| &s.views_job, move |shared| {
        let generation = shared.generation.fetch_add(1, Ordering::Relaxed) + 1;
        let views = compute(shared);
        let _ = qt.queue(move |mut sioul| {
            let shared = Arc::clone(&sioul.rust().shared);
            // Only the newest: results queued while the window was frozen (a phone
            // with Sioul in the back) are not applied one after the other at its return.
            if shared.generation.load(Ordering::Relaxed) != generation || shared.shown_generation.fetch_max(generation, Ordering::Relaxed) > generation {
                return;
            }
            …
            sioul.as_mut().set_porch(QString::from(&views.porch));
            sioul.as_mut().set_budgets(QString::from(&views.budgets));
            …
        });
    });
}
```

- **`coalesced`** runs `work` on a thread for one *job* (here `views_job`), one computation at a time: asked again while one runs, it runs once more after it, with everything changed meanwhile, rather than piling up threads on a slow disk. A page's computation that panics on bad data leaves the next ones free to run. Each set of pages has its job: the Porch and budgets (`views_job`), mail (`mail_job`), the agenda and contacts (`pim_job`), tasks and notes (`work_job`), health (`health_job`).
- **The generation** numbers each computation; only the newest result is shown, even when results come back out of order.
- **`sioul.rust()`** reaches `SioulRust` (CXX-Qt's `CxxQtType` trait); **`sioul.as_mut()`** reborrows the pinned object for each setter.
- **`let _ =`**: `queue` fails only when the object is gone (the window closed); there is nothing left to show then.
- **On a phone put away**, `coalesced` does not compute pages for nobody: the job is marked, and runs once Sioul is back (`backend::AWAY`).

Long-lived threads follow the same rule. Each mail account's watcher (`backend::start_watcher`) runs `sioul_sync::watch` on a thread of its own, and reports each fetch through `qt.queue`; an `Arc<Control>` kept in `Shared` stops it or wakes it.

Signals are emitted the same way. When a code's notification has its "Copy" button pressed, the press is heard on a thread that waits for the notification's answer, and `backend.rs` queues the signal from there:

```rust
let _ = qt.queue(move |sioul| sioul.copy_requested(QString::from(&text)));
```

and `main.qml` hears it:

```qml
Connections {
    target: sioul
    function onCopyRequested(text) {
        window.copy(text)
    }
    …
}
```

Rust never calls a QML function. It sets a property, emits a signal, or returns a value from an invokable.

## C++ beside the bridge
- **Qt's own C++, called from Rust**: a bridge can declare C++ functions too. `desktop.rs` declares `QIcon`'s static functions, to set the icon theme from Rust:

    ```rust
    unsafe extern "C++" {
        …
        include!(<QtGui/QIcon>);
        type QIcon;

        #[Self = "QIcon"]
        #[rust_name = "theme_name"]
        fn themeName() -> QString;
        …
    }
    ```

- **Sioul's own C++**, in `crates/sioul-app/cpp/`: two Qt objects QML uses (`TextSpacing`, line spacing for editable text; `PdfWriter`, a page of HTML printed into a PDF, as an invoice), registered in the same module with Qt's `QML_ELEMENT`; and plain C functions Rust calls through `unsafe extern "C"` in `lib.rs`, such as `sioul_start_web_engine` (`cpp/webengine.cpp`) and `sioul_warm_up` (`cpp/warmup.cpp`).

## build.rs: what the module holds
`crates/sioul-app/build.rs` runs before the crate is compiled. It tells CXX-Qt's builder (`cxx-qt-build`) what makes up Sioul's QML module:

```rust
let builder = CxxQtBuilder::new_qml_module(QmlModule::new("com.aurelienpierre.sioul").depend("QtQuick").qml_files(pages))
    .files(["src/backend.rs", "src/desktop.rs"])
    .cpp_files(cpp)
    .qt_module("Quick");
```

- **`QmlModule::new("com.aurelienpierre.sioul")`**: the module's name, which QML imports.
- **`.qml_files(pages)`**: every QML file of the window, listed by hand above it. **A file not in the list is not in the module**: the window cannot load it. On Android, the pages Android lacks are swapped for `qml/android/`'s, and the tray (`qml-desktop/Tray.qml`) is left out.
- **`.files([…])`**: the Rust files that hold a `#[cxx_qt::bridge]`.
- **`.cpp_files(cpp)`**: the C++ of `cpp/`; Qt WebEngine's set-up, the signals' handler and the widgets application only on computers.
- **`.qt_module("Quick")`**, and `"Widgets"` on computers: the Qt libraries to link.
- **`.qrc(…)`**: the resource files: the bundled icons, Sioul's own icon, the symbols font ([qt-quick.md](qt-quick.md#icons-fonts-and-resources)).

The build writes the generated C++, the module's `qmldir` and its type description (`plugin.qmltypes`, which `tools/lint-qml.sh` gives qmllint) under `target/…/build/sioul-app-*/out/`. At run time, `lib.rs` calls `cxx_qt_init_crate_sioul_app()` to register the module before loading `main.qml`. Android's build does the same steps by hand in CMake ([android-basics.md](android-basics.md#building-the-apk)).

## Adding an invokable, end to end
The pair `stopped` and `set_stopped` above is the model. To let QML ask Rust something new:

1. **The logic**, in the core if it decides or words anything, with a test ([rust.md](rust.md#tests)); a function of the window's crate gathers what it needs (`crate::work::stopped`).
2. **The declaration**, in the `extern "RustQt"` block of `backend.rs`, with a `///` line saying what it returns:

    ```rust
    /// Where you stopped, as JSON: {text, when, task, title}; "null" for none.
    #[qinvokable]
    fn stopped(self: &Sioul) -> QString;
    ```

    `&Sioul` if it only reads; `Pin<&mut Sioul>` if it sets a property or starts work that will.

3. **The implementation**, in the `impl qobject::Sioul` block: convert the `QString`s (`to_string()`, `QString::from`), call the function, return. Anything slow goes to a thread, and comes back through `qt.queue` (above).
4. **The call**, in QML, by its camel-case name, through the page's `sioul`: `JSON.parse(card.sioul.stopped() || "null")`.
5. **The words**: every sentence it returns, and every label around it, as messages in both `.ftl` files ([fluent.md](fluent.md)).
6. **The checks**: `cargo build -p sioul-app` (CXX-Qt checks the declaration against the implementation), `tools/lint-qml.sh`, `tools/check-messages.py`.

A new property is the same: a `#[qproperty(T, name)]` line, a field `name: T` in `SioulRust`, `set_name(…)` called on the window's thread, and QML reading it by its camel-case name (`focus_session` is `sioul.focusSession`). [A feature, end to end](end-to-end.md) follows a whole feature through these steps.

## Further reading
- [The CXX-Qt book](https://kdab.github.io/cxx-qt/book/): the bridge's attributes, threading, the build.
- [CXX](https://cxx.rs/), the library CXX-Qt is built on, for Rust and C++ calling each other.
- Qt's [QObject](https://doc.qt.io/qt-6/qobject.html) and [threads and QObjects](https://doc.qt.io/qt-6/threads-qobject.html), for why an object belongs to one thread.
