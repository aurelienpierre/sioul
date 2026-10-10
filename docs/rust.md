# Rust, as Sioul uses it

This page is for a developer who has not written Rust. It does not teach the language: [The Rust Programming Language](https://doc.rust-lang.org/book/) ("the Book") does, and each section below names the chapter to read. It shows the parts of Rust that Sioul's code relies on, with short excerpts from the code itself, so that the first files you open read more easily. [Start here](start-here.md) has the map of the code.

## The workspace and its crates
A *crate* is Rust's unit of compilation: a library or a program. A *package* is a folder with a `Cargo.toml` that builds one or more crates; Sioul has one per folder of `crates/`, and calls each a crate. Cargo, Rust's build tool, builds them, fetches their *dependencies* (other crates, from [crates.io](https://crates.io)) and runs their tests. A *workspace* builds several packages together, with one `Cargo.lock` (the exact version of every dependency) and one `target/` folder.

The root `Cargo.toml` declares it:

```toml
[workspace]
resolver = "3"
members = ["crates/sioul-core", "crates/sioul-cli", "crates/sioul-sync", "crates/sioul-app", "crates/sioul-learn"]
# The window needs Qt's development files (docs/building.md); everything else builds without them.
# sioul-learn (the spam filter's training) is for desktops: the phone never builds it.
default-members = ["crates/sioul-core", "crates/sioul-cli", "crates/sioul-sync", "crates/sioul-learn"]

[workspace.package]
version = "0.0.5"
edition = "2024"
rust-version = "1.89"
```

- **`default-members`**: what `cargo build` and `cargo test` take when no package is named. The window is left out, so that a computer without Qt builds the rest. `-p` names one: `cargo build -p sioul-app`.
- **`version`**: Sioul's version, once for every crate (each says `version.workspace = true`); Android's build reads it there too ([android-basics.md](android-basics.md#building-the-apk)).
- **`edition = "2024"`**: the edition of the language the code is written in. A few things in the code are new in it: `unsafe extern "C"` blocks, `#[unsafe(no_mangle)]`, and `if let … && let …` chains (below). The [Edition Guide](https://doc.rust-lang.org/edition-guide/rust-2024/index.html) lists the changes.
- **`rust-version = "1.89"`**: the oldest compiler that builds Sioul.
- **`[workspace.dependencies]`**: each dependency's version is set once, at the root; a crate says `serde.workspace = true` in its own `Cargo.toml` to take it.
- **Profiles**: `[profile.dev]` keeps debug builds small (line tables only, no debug information for dependencies), and builds the cryptography crates optimised even in debug builds, since a passphrase's key derivation (Argon2) takes seconds without it.

The Book: [chapter 7, packages, crates and modules](https://doc.rust-lang.org/book/ch07-00-managing-growing-projects-with-packages-crates-and-modules.html), and [chapter 14, Cargo workspaces](https://doc.rust-lang.org/book/ch14-03-cargo-workspaces.html).

## Features and platforms
A *feature* is a switch a crate offers at build time. Sioul has one, `insecure-test-tls`, for tests against a local mail server only ([building.md](building.md), "Testing against a local mail server"). `crates/sioul-sync/Cargo.toml` declares it under `[features]` (`insecure-test-tls = []`), and each crate that uses sync passes it on, under a feature of the same name (`insecure-test-tls = ["sioul-sync/insecure-test-tls", …]` in `crates/sioul-cli/Cargo.toml`, `crates/sioul-app/Cargo.toml` and `crates/sioul-learn/Cargo.toml`).

Code reads a feature with `#[cfg(feature = "…")]` or `cfg!(feature = "…")`. `crates/sioul-sync/src/lib.rs` refuses to build a release with it:

```rust
#[cfg(all(feature = "insecure-test-tls", not(debug_assertions)))]
compile_error!("insecure-test-tls accepts any certificate: for test builds only, never with --release");
```

The same attributes choose code per system. `#[cfg(target_os = "android")]` keeps an item for Android only; `cfg!(target_os = "android")` is a `true` or `false` the code can test. Dependencies can be per system too: `crates/sioul-sync/Cargo.toml` takes a different keyring backend for Linux, Windows, macOS and Android under `[target.'cfg(…)'.dependencies]`, and the two programs take `sioul-learn` on computers only (`[target.'cfg(not(target_os = "android"))'.dependencies]`).

The Cargo Book: [features](https://doc.rust-lang.org/cargo/reference/features.html), [platform-specific dependencies](https://doc.rust-lang.org/cargo/reference/specifying-dependencies.html#platform-specific-dependencies).

## Modules and visibility
A crate is a tree of *modules*. `crates/sioul-core/src/lib.rs` is the root of the core; each `pub mod porch;` there is the file `src/porch.rs`. A module with modules of its own is a file and a folder of the same name: `crates/sioul-cli/src/mcp.rs` declares `mod protocol;`, `mod read;` and others, which are `src/mcp/protocol.rs`, `src/mcp/read.rs`. It can also be a folder alone, whose `mod.rs` is the module: the core's spam filter, `crates/sioul-core/src/spam/mod.rs`, with `spam/tokenize.rs` and its other parts beside it.

What a module may use from another is set by *visibility*:
- **`pub`**: seen from other crates. The core's modules and most of their functions are `pub`, since the command line, the window and sync all use them: `sioul_core::stopped::Stopped::load`.
- **`pub(crate)`**: seen anywhere in the same crate, not outside. The window's modules use it for what they share with each other: `pub(crate) fn tr()` in `backend.rs`.
- **nothing**: seen only in the module and its children. The window's modules are declared `mod alarms;` in `crates/sioul-app/src/lib.rs`: no other crate depends on them.

`use` brings names into a file: at the top of `crates/sioul-app/src/backend.rs`, `use sioul_core::porch::{self, KnownSenders, SenderList, Triaged};` makes `porch::gather`, `KnownSenders`, `SenderList` and `Triaged` short. Inside a crate, `crate::` starts from the crate's root and `super::` from the module above.

## Ownership and borrowing
Every value in Rust has one *owner*; when the owner goes, the value is freed. Other code can *borrow* it: `&T` reads it, `&mut T` changes it, and the compiler checks that no borrow outlives the value and that nothing reads a value while something else changes it. This replaces a garbage collector and prevents data races between threads. The Book: [chapter 4](https://doc.rust-lang.org/book/ch04-00-understanding-ownership.html).

What it looks like in Sioul:
- **Functions borrow; structs own.** `Stopped::keep` (`crates/sioul-core/src/stopped.rs`) takes borrowed text, `&str` and `&Path`, and makes owned values (`String`) only for what it keeps. It begins:

    ```rust
    pub fn keep(path: &Path, text: &str, task: &str, at: i64) -> Result<(), String> {
        let stopped = Stopped { text: text.trim().to_string(), at, task: task.to_string() };
    ```

    `String` and `PathBuf` own their text; `&str` and `&Path` borrow it. Most functions take the borrowed kind, so that a caller can pass what it has without copying.

- **`clone()`** makes a copy the code then owns. It is used where a value must outlive the place it came from, such as a value moved into another thread.
- **`Arc`**, a counted reference, lets several threads own one value. The window's state that threads share is `Arc<Shared>` (`crates/sioul-app/src/backend.rs`); `Arc::clone` adds an owner, and a `move` closure takes it into the thread:

    ```rust
    let (qt, shared) = (qt.clone(), Arc::clone(&shared));
    std::thread::spawn(move || {
        let checked: usize = load_config().mail_sources().iter().map(|s| sioul_sync::verify::verify_folder(&s.folder, false)).sum();
        if checked > 0 {
            show(&qt, &shared);
        }
    });
    ```

    This is the end of the `start` invokable (`backend.rs`): the mail fetched before Sioul checked senders itself is checked once, on a thread, and the Porch is shown again if any was.

- **`Mutex` and atomics** let threads change what they share. A `Mutex` holds a value that one thread at a time may reach: `shared.watchers.lock()` waits for it and returns a guard, which lets go when it goes out of scope (inside a `Result`: a thread that panicked while holding it leaves it *poisoned*). `AtomicBool` and `AtomicU64` hold one flag or number that any thread can read and change without waiting. The Book: [chapter 16](https://doc.rust-lang.org/book/ch16-00-concurrency.html).
- **`Pin<&mut Sioul>`**, in the bridge, is a mutable borrow of an object that must stay at one address in memory, as Qt's objects must: CXX-Qt asks for it wherever a method changes the object ([cxx-qt.md](cxx-qt.md)).
- **`'static`** in a type, as in `&'static Translator`, says that the borrowed value lives as long as the program. `backend::tr()` makes the translator once, in a `OnceLock`, and lends it to anyone.

## Results, options and `?`
Rust has no exceptions. A function that can fail returns `Result<T, E>`: `Ok(value)` or `Err(error)`. A value that may be missing is `Option<T>`: `Some(value)` or `None`. The `?` operator, after a `Result` or an `Option`, returns the error (or `None`) at once and otherwise gives the value. The Book: [chapter 9](https://doc.rust-lang.org/book/ch09-02-recoverable-errors-with-result.html).

From `Stopped::keep`: `map_err` turns the system's error into Sioul's, and `?` returns it if there is one:

```rust
let fail = |e: std::io::Error| format!("{}: {e}", path.display());
if let Some(parent) = path.parent() {
    std::fs::create_dir_all(parent).map_err(fail)?;
}
```

The same with an `Option`, in `crates/sioul-app/src/lib.rs`, where any step that finds nothing ends the function with `None`:

```rust
let stat = std::fs::read_to_string("/proc/self/stat").ok()?;
// The fields after the program's name, which ends at the last ")": the
// 22nd, when the process started, in hundredths of a second since boot.
let started: f64 = stat.rsplit_once(')')?.1.split_whitespace().nth(19)?.parse().ok()?;
```

Two forms the code uses often, which older Rust did not have:
- **`let … else`**: take the value or leave. From `crates/sioul-app/src/work.rs`:

    ```rust
    let Some(stopped) = sioul_core::stopped::Stopped::load(&sioul_core::stopped::Stopped::default_path()) else { return "null".into() };
    ```

- **`if let … && let …`** (*let chains*, edition 2024): several conditions, each of which may unpack a value. From `backend::start_watcher`:

    ```rust
    if let Ok(mut watchers) = shared.watchers.lock()
        && watchers.get(&account.id).is_some_and(|c| Arc::ptr_eq(c, &control))
    {
        watchers.remove(&account.id);
    }
    ```

## The error types
- **The core returns `Result<T, String>`**: the error is a readable line, usually the file's path and what went wrong (`…/stopped.toml: Permission denied (os error 13)`). The window shows it in the status line as it is.
- **Sync has one error type, `SyncError`** (`crates/sioul-sync/src/lib.rs`): an `enum` with one case per kind of failure (`Network`, `Tls`, `Login`, `AppPassword`, `Disk`…), each with its detail. It knows the Fluent message that says it (`message_id`, then `sentence(tr, account)`), whether the failure lasts, so that trying again later cannot help and may lock the account (`is_lasting`: a refused login, a certificate that does not check), and whether a new password can help (`wants_password`). It implements `Display` and `std::error::Error`, the two traits Rust's errors share.
- **No panics across the edges.** A *panic* is Rust's crash of a thread, for bugs, never for expected failures. Where bad data could make one, the code catches it so that the window or Android's caller goes on: `std::panic::catch_unwind` in `backend::coalesced` (a page's computation) and in the functions Java calls through C (`sioul_alarm_decide` in `crates/sioul-app/src/alarms.rs`, and their kin), since a panic must never cross into C or Java.
- **`unwrap()`**, which panics on an error, belongs in tests.

## Traits that matter here
A *trait* is a set of methods a type promises to have, as an interface in other languages. The Book: [chapter 10](https://doc.rust-lang.org/book/ch10-02-traits.html).
- **Derived traits**: `#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]` above a struct writes those traits for it. `Debug` prints it in tests, `Clone` copies it, `Default` makes an empty one (`PorchState::default()`, and `unwrap_or_default()` falls back to it), `PartialEq` compares it in `assert_eq!`, `Serialize` and `Deserialize` turn it into and out of TOML or JSON.
- **`Default` for the bridge**: CXX-Qt makes the Rust side of the `Sioul` object with `Default::default()`, hence `#[derive(Default)]` on `SioulRust` ([cxx-qt.md](cxx-qt.md)).
- **Closures as arguments**: `Fn`, `FnMut` and `FnOnce` are the traits of functions passed as values. `backend::coalesced` takes `work: impl Fn(&Arc<Shared>) + Send + 'static`: a function that may be called several times, sent to another thread (`Send`), and borrowing nothing that could go away (`'static`). `sioul_sync::watch` takes `report: impl FnMut(Result<Report, SyncError>)`, called after each fetch.
- **CXX-Qt's traits**: `Threading` gives the bridge object `qt_thread()`, and `CxxQtType` gives `rust()`, its Rust side.

## Plain files: serde, toml and toml_edit
[serde](https://serde.rs/) turns Rust values into text and back; `toml` and `serde_json` are its TOML and JSON formats. A struct that derives `Serialize` and `Deserialize` is read and written in one line each. `crates/sioul-core/src/stopped.rs`:

```rust
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Stopped {
    pub text: String,
    /// Unix seconds.
    pub at: i64,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub task: String,
}
```

`toml::from_str::<Stopped>(&text)` reads one, `toml::to_string(&stopped)` writes one. `#[serde(default)]` lets a file without `task` be read (the field is then empty), and `skip_serializing_if` leaves it out of the file when it is empty.

**The configuration is edited in place**, with [toml_edit](https://docs.rs/toml_edit), never written again from a struct: the file is yours, and its comments and order stay. `crates/sioul-core/src/config.rs` reads it into a `DocumentMut`, changes one value (`set_account_item`), checks that the result still reads as a `Config`, writes it beside and renames it over the old one (`write_document`).

**JSON for the window**: the bridge hands QML most of what it shows as JSON text, made with `serde_json` from structs of `crates/sioul-core/src/view.rs` (`backend::json`) or with the `serde_json::json!` macro for small objects.

## Time: jiff
Sioul's dates and times use [jiff](https://docs.rs/jiff): `Zoned::now()` is now in the system's time zone, `Timestamp` an instant (often stored as Unix seconds, `Timestamp::now().as_second()`), `jiff::civil::Date` a day without a zone, `TimeZone::system()` the system's zone. `chrono` and `chrono-tz` appear only where `calcard`, the iCalendar and vCard parser, needs them: `crates/sioul-core/src/agenda.rs` turns a jiff time zone into calcard's.

## Threads and async
- **The window and the command line are synchronous.** Slow work goes to a plain thread, `std::thread::spawn`, and its result comes back to the window's thread through CXX-Qt ([cxx-qt.md](cxx-qt.md)).
- **The network code is asynchronous inside `sioul-sync`.** IMAP, SMTP and DNS libraries are written for [tokio](https://tokio.rs/tokio/tutorial), Rust's most used async runtime: an `async fn` returns a *future*, a task that a runtime runs, and `.await` waits for another without holding a thread. Sync keeps this inside: the functions the window and the command line call make a small runtime on the current thread and wait for the result (`crates/sioul-sync/src/fetch.rs`):

    ```rust
    fn runtime() -> Result<tokio::runtime::Runtime, SyncError> {
        tokio::runtime::Builder::new_current_thread().enable_all().build().map_err(|e| SyncError::Network(e.to_string()))
    }

    pub(crate) fn block_on<T>(future: impl Future<Output = Result<T, SyncError>>) -> Result<T, SyncError> {
        runtime()?.block_on(future)
    }
    ```

    So `sioul_sync::sync(&account, &password)` returns when the fetch is over, and a caller runs it on a thread of its own.

- **A watcher is stopped or woken from outside** through a `Control` (`crates/sioul-sync/src/fetch.rs`): flags any thread can set (stop, fetch now, real time), and a way to wake the watcher's waits at once.
- **Between processes**, a file two of them change is locked while it is read, changed and written (`sioul_core::filelock::with_lock`: an advisory lock on a hidden `.<name>.lock` beside it, through `std::fs::File::lock`): the window, the command line and a phone's background service may write the same file.

The Book: [chapter 16, concurrency](https://doc.rust-lang.org/book/ch16-00-concurrency.html), and [chapter 17, async](https://doc.rust-lang.org/book/ch17-00-async-await.html).

## Calling C, and being called from it
Rust calls C (and C++ written to look like C) through `unsafe extern "C"` blocks, and makes functions C can call with `#[unsafe(no_mangle)] pub extern "C" fn`. Sioul does both: the window calls its C++ helpers (`crates/sioul-app/src/lib.rs`, `sioul_start_web_engine`), and Android's C++ calls Rust ([android-basics.md](android-basics.md)). *Unsafe* code is code whose safety the compiler cannot check, so the code says why it is sound: a function marked `unsafe` has a `# Safety` section in its comment, which says what its caller must promise (`sioul_alarm_decide` in `crates/sioul-app/src/alarms.rs`), and an `unsafe { … }` block has a `// SAFETY:` comment above it (`crates/sioul-app/src/lib.rs` has one at each). Some of the Android calls lack theirs: write one with any new block. The Book: [chapter 20, unsafe Rust](https://doc.rust-lang.org/book/ch20-01-unsafe-rust.html).

## Tests
Tests sit beside the code, in a module compiled only for tests. The end of `crates/sioul-core/src/stopped.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_line_left_then_done() {
        let dir = std::env::temp_dir().join(format!("sioul-stopped-{}", std::process::id()));
        let path = dir.join("stopped.toml");
        let _ = std::fs::remove_dir_all(&dir);
        assert_eq!(Stopped::load(&path), None);
        Stopped::keep(&path, "  the second paragraph, after the quote ", "t1", 100).unwrap();
        assert_eq!(Stopped::load(&path), Some(Stopped { text: "the second paragraph, after the quote".into(), at: 100, task: "t1".into() }));
        Stopped::keep(&path, "", "", 200).unwrap();
        assert_eq!(Stopped::load(&path), None, "done: taken away");
        let _ = std::fs::remove_dir_all(&dir);
    }
}
```

- **`#[cfg(test)]`** builds the module for `cargo test` only; **`use super::*`** brings in the module above; each **`#[test]`** function is one test, which fails if it panics (`assert_eq!` panics when its two values differ).
- **Integration tests** are files in a crate's `tests/` folder, which use the crate as another crate would: `crates/sioul-core/tests/porch.rs`, with its invented mail in `tests/fixtures/`.
- **`#[ignore]`** marks tests that need something outside (a local server, a stand-in of an API, a program such as Tesseract): they run only when asked.
- **Invented data only**: reserved domains (`example.org`, RFC 2606), fictional numbers, temporary folders; a test never reads the configuration of whoever runs it (`sioul_sync::translator` reads none under `cfg!(test)`).

Running them:

| Command | What runs |
|---|---|
| `cargo test` | the default members' tests: the core's, sync's, the command line's and `sioul-learn`'s |
| `cargo test -p sioul-core` | one crate's |
| `cargo test -p sioul-core stopped` | the tests whose full name holds `stopped`, as `stopped::tests::a_line_left_then_done` |
| `cargo test -p sioul-core --lib i18n::tests` | one module's, in the library alone |
| `cargo test -p sioul-core --test porch` | one integration test file |
| `cargo test -p sioul-sync --features insecure-test-tls -- --ignored stand_in` | the ignored test against the stand-in of Google Tasks, which must be running ([building.md](building.md)) |
| `cargo test -p sioul-app homecard` | the window's tests: they need Qt to build |

CI (`.github/workflows/build.yml`) runs `cargo test --release --locked -p sioul-core -p sioul-sync -p sioul-cli` on Linux, Windows and macOS; the window's own tests run only where someone runs them.

The Book: [chapter 11](https://doc.rust-lang.org/book/ch11-00-testing.html).

## Formatting and lints
- **rustfmt**: the repository has no rustfmt configuration, and its code is not laid out as rustfmt's defaults would lay it out (many lines are longer than its 100 columns). `cargo fmt` would rewrite most of each file it touches: match the layout of the file you edit instead.
- **clippy**, Rust's linter: not run by CI, but the code answers it; the few functions with many arguments say so (`#[allow(clippy::too_many_arguments)]`). `cargo clippy -p sioul-core` runs it on one crate ([clippy's documentation](https://doc.rust-lang.org/clippy/)).
- **The house rules** are in [architecture.md](architecture.md#code-style): one task per function, comments that give the reason and the reference, a test for every rule the window applies. Each file starts with its licence (`// SPDX-License-Identifier: GPL-3.0-or-later`) and a `//!` comment saying what it is for.

## Documentation comments
`//!` at the top of a file describes the module; `///` above an item describes the item. Both are Markdown, and `cargo doc` turns them into the API reference ([the API reference](https://aurelienpierre.github.io/sioul/dev/api.html) on this site). Name the design note a module follows in its `//!` comment (`(docs/tasks.md, "Where you stopped")`): the notes and the code then lead to each other.
