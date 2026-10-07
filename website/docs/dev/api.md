---
title: API reference
description: "The reference of Sioul's code, made from the code's own comments: the Rust crates, the Android code in Java, and the window's QML files."
---

# API reference

The reference of Sioul's code is made from the code's own comments, each time this website is published. It has three parts, one for each language of the code.

| Reference | What it covers | Made by |
|---|---|---|
| [Rust](../api/rust/sioul_core/index.html) | The five crates: the core, sync, the command line, the window's Rust side, and the spam filter's training | rustdoc |
| [Java](../api/java/com/aurelienpierre/sioul/package-summary.html) | The Android side: alarms, receivers, services, the home-screen card | javadoc |
| [QML](qml.md) | The window's pages, dialogs and controls | `tools/qml-docs.py` |

A comment that is wrong or missing is corrected in the code, never in the reference. How the parts fit together: [Architecture](architecture.md).

## The Rust reference

One reference per crate. A crate is a Rust library or program; the five of them are in `crates/`.

- **[sioul-core](../api/rust/sioul_core/index.html)** (`crates/sioul-core`): the library at the centre. Everything the window shows is decided and worded here: mail and the Porch, tasks and the plan, notes, links, time, money, health, translations. Start here.
- **[sioul-sync](../api/rust/sioul_sync/index.html)** (`crates/sioul-sync`): what talks to the world: mail servers, calendars and contacts, the keyring, Google, GitHub, Bitwarden, sharing between devices.
- **[sioul](../api/rust/sioul/index.html)** (`crates/sioul-cli`): the command line, a program named `sioul`, and `sioul mcp`, the server for AI agents.
- **[sioul-app](../api/rust/sioul_app/index.html)** (`crates/sioul-app`): the window's Rust side. Its module `backend` holds `Sioul`, the object the QML pages call; the other modules hold the window's side of each area (mail, tasks, contacts, health…), and run its slow work off the window's thread.
- **[sioul-learn](../api/rust/sioul_learn/index.html)** (`crates/sioul-learn`): the spam filter's training, on computers only: the corpus, the labels, fastText, the SVM, the calibration, the evaluation, and the table every device reads ([Your own spam filter](spam-filter.md)). What every device runs is in the core, in its module `spam`.

Each crate's page says what the crate is for, where to start reading, and lists its modules by theme, one line each. The reference shows private items too: every module and function, not only what a crate offers to other crates. The window's modules are all private, and a contributor needs to read them.

### Reading rustdoc

rustdoc makes the same kind of pages for every Rust project, so what you learn here works for any crate on [docs.rs](https://docs.rs).

- **A crate's page** starts with what the crate is for: the `//!` comment at the top of its `lib.rs` (or `main.rs`, for a program). Then rustdoc lists the crate's modules, by name.
- **A module** is one file: `porch.rs` is the module `porch`. Its page starts with the file's own `//!` comment, then lists what the module holds, by kind: modules inside it, structs and enums (kinds of data), functions, traits (what several types can do), constants.
- **An item's page** shows its declaration, then its comment (the `///` lines above it in the code). A function's declaration says what it takes and what it gives back: `fn is_gmail(address: &str) -> bool` (in `sioul-sync`) takes a text and answers yes or no. A struct's page lists its fields, then its functions (`impl` blocks) and the traits it has.
- **The search box**, at the top of every page, finds any item of the five crates by name: type a few letters (`porch`, `lane`, `plan`). A kind before a colon keeps that kind only: `fn:plan`, `struct:task`, `mod:porch`. Press `S` or `/` to reach the box from anywhere, `?` for the other keys.
- **"Source"**, at the right of each item's title, opens the code it was made from, at its line. The code is there too, from the start of the file.
- **Links in the comments** lead to the item they name: the code writes `[porch::Lane]` and rustdoc makes it a link.
- **Colours**: light or dark, in the settings (the gear at the top right).

### Where to start

- **How the window reaches Rust**: [`sioul_app::backend`](../api/rust/sioul_app/backend/index.html). Each function marked `#[qinvokable]` is one a QML page can call (`sioul.text("…")`); each `#[qproperty]` is a value the pages read, and are told about when it changes. Most values are JSON made by the core's views.
- **What the window shows**: the core's [`view`](../api/rust/sioul_core/view/index.html) (the Porch, mail, budgets, contacts, the agenda) and [`taskview`](../api/rust/sioul_core/taskview/index.html) (tasks and the plan), then the module each one reads.
- **What the command line does**: the crate `sioul`, its [`Command`](../api/rust/sioul/enum.Command.html) enum: one variant per command, its comments the help the program prints.

## The Java reference

[The Java reference](../api/java/com/aurelienpierre/sioul/package-summary.html) covers `android/package/src/com/aurelienpierre/sioul/`: what Android starts on Sioul's behalf, often while Sioul's window is closed. Each class says what it is for at the top of its page.

- **Android's own classes** (`Activity`, `BroadcastReceiver`, `Service`) link to [Android's reference](https://developer.android.com/reference/), and Java's own (`String`, `List`) to Oracle's. An Android class is started by the system for what the app's manifest (`android/package/AndroidManifest.xml`) declares: a receiver for an alarm, a service for the steps, an activity for a shortcut.
- **A class's page** lists its fields, constructors and methods, private ones included, each with its comment. The name of the class, or of a member, opens its source at its line.
- **How Java and Rust talk to each other**, and how Qt for Android runs Sioul: [Android](android.md).

## The QML reference

[The QML reference](qml.md) is one page: each `.qml` file of `crates/sioul-app/qml/`, grouped by what it is (the window, the status line, pages, dialogs, controls…). Each one shows what the file says about itself, the type it is based on, what whoever makes it must give it (its required properties), and its own properties, signals and functions, each with the comment above it. That page starts with how to read a QML file.

## How the reference is made

- **Rust**: `cargo doc --no-deps --document-private-items`, one crate at a time, into `target/doc/`. On your computer, `cargo doc --no-deps --document-private-items -p sioul-core --open` makes one crate's reference and opens it; `sioul-app`'s needs Qt 6's development files, as building the window does ([Building and running](building.md)). With `RUSTDOCFLAGS="-D warnings"` before it, the same command stops at the first warning, as the publication does.
- **Java**: javadoc, its private members included, with Android's `android.jar` (from the Android SDK, `platforms;android-36`) so that Android's classes are known and linked. The Android code uses no class of Qt's (it names `QtActivity` in a string only), so Qt's own jar is not needed. javadoc checks each comment's HTML and references; a member without a comment is allowed.
- **QML**: `tools/qml-docs.py` reads each file's comments and writes the QML page. No standard tool fits: Qt's own, qdoc, needs its own markup in the comments.
- **The website**: `website/build.sh` writes the QML page at every build. `website/build.sh --api` also runs rustdoc and javadoc, then copies their pages into the site, under `api/rust/` and `api/java/`; `--api-only` makes them and copies them into the site built before, touching nothing else. Both need Rust, Qt 6's development files and a JDK besides what the website needs.
- **Published** by `.github/workflows/pages.yml`, on each push to `main` that changes the code, the website or the notes: first the guide and the notes (`build.sh --strict`: a warning from Zensical stops the publication), then the reference (`build.sh --strict --api-only`). A warning from rustdoc or javadoc, or a download that fails (Qt, Android's platform), leaves the reference out of that publication, never the guide: the site is published without it, and the run's page says so. A link to nothing never reaches the site.

## Writing comments that read well here

- **Rust**: a `//!` block at the top of each file says what the module is for; `///` lines above an item say what it is or does. The first sentence shows in lists, so it says the most. Code goes in backticks. Another item is named by its path in brackets (`` [`porch::Lane`] ``), which becomes a link; a web address goes in angle brackets (`<https://…>`). Outside backticks, rustdoc reads `<folder>` as an HTML tag (it hides it, and warns) and `[x]` as a link: write `` `<folder>` `` and `` `[x]` ``.
- **A new module** gets its line on its crate's page, under its theme: the `//!` block at the top of `lib.rs` (`main.rs` for the command line). In `sioul-app`, whose modules are private, that page links a module by its page, `` [`backend`](backend/index.html) ``: from the crate's public page, a link to a private item by its path warns.
- **The command line's help**: in `crates/sioul-cli`, the `///` comments of the commands and their options are also what `sioul --help` prints. Their words are read in a terminal too: plain text, backticks for what is typed.
- **Java**: a `/** … */` comment above a class, a field or a method. javadoc reads it as HTML: `&` is written `&amp;`, and anything with `<` or `>` goes in `{@code …}`.
- **QML**: a `//` block after the licence lines says what the file is; `//` lines just above a property, a signal or a function say what it is. A blank line between a comment and what it describes detaches them.
