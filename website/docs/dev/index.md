---
title: For developers
description: Sioul's design notes, architecture, and how to build, test and document it.
---

# For developers

Sioul is free software, under the GPL-3.0-or-later licence, written in Rust, with a Qt 6 window in QML. Its sources are at [github.com/aurelienpierre/sioul](https://github.com/aurelienpierre/sioul).

This section holds the design notes of the repository's `docs/` folder, as they are: what each part does, why, from which research, and where it stands. The [user guide](../index.md) is for using Sioul; these notes are for working on it.

**New here?** If you have not worked with Rust, Qt Quick, CXX-Qt, Fluent or Android, start with [a map of the code](start-here.md), then the guide to each tool as Sioul uses it ([Rust](rust.md), [Qt Quick and QML](qt-quick.md), [CXX-Qt](cxx-qt.md), [Fluent](fluent.md), [Android](android-basics.md)), and [a feature followed end to end](end-to-end.md), with the checklist for adding one.

## How it is designed

Sioul adapts the demands of the world to the person: it starts from the person's needs and from what a day can hold, keeps that time, and plans the obligations in what remains; it shows what belongs in the person's attention now; it keeps the ties between things so that the person does not have to. Each rule comes from a chain: an observation in the research, its mechanism, the rule it gives, what Sioul does, and what it refuses to do ([design](design.md), [what the research says](research.md), [the research notes](research/README.md)). Three tests for anything added: it takes admin work off the person rather than moving it elsewhere; its complexity stays on Sioul's side; it is something software may do to a person.

A few words in the notes are older than the window: a *case* is what the window calls a *project*; *admin windows* became working hours and hours for your admin (free time became every other time, leisure); *Parameters* is the Settings page.

## Architecture, in brief

| Part | What it does |
|---|---|
| `crates/sioul-core` | The library at the centre: reading and judging mail, the Porch, projects and their routes, areas and hours, tasks and the plan, notes, links between everything, time, budgets, the bank, papers, letters, health, reminders, translations. Everything the window shows is decided and worded here, so that the command line, the window and AI agents see the same thing. |
| `crates/sioul-sync` | What talks to the world: finding servers, the keyring, IMAP sync and the actions on messages, sending, the IDLE watchers, notifications, CalDAV and CardDAV, Google, GitHub, Bitwarden, the antivirus, reading scans, sharing between devices. |
| `crates/sioul-cli` | `sioul`, the command line, first because agents and scripts use it too; `sioul mcp` serves agents. |
| `crates/sioul-app` | The window: Qt Quick (QML) through CXX-Qt. Its Rust side in `src/`, its pages in `qml/`, a little C++ in `cpp/` (Qt WebEngine's set-up, the PDF writer, line spacing), the Breeze icons it bundles in `icons/`. The window holds no logic. |
| `crates/sioul-learn` | The spam filter's training, on computers only: it learns from your own mail and writes the table every device reads. Still being built. |
| `android/` | The phone's app: Qt for Android builds the window into it; Java classes run what Android starts while the window is away, and talk to Rust through `main.cpp` ([Android, the basics](android-basics.md)). |

Storage is plain files wherever possible: Maildir for mail; one `.ics` or `.vcf` file per event, task or contact (vdir), tasks tied together as RFC 9253 says; Markdown for notes; TOML for the configuration, projects and budgets; sealed JSON lines for sharing. The whole picture, with each library and its licence: [Architecture](architecture.md).

Every sentence goes through Fluent, in `crates/sioul-core/locales/<language>/sioul.ftl`, English and French; a test checks that every language has every message ([Languages](i18n.md)).

## Building and testing

```
cargo build --release               # core, sync, command line (no Qt)
cargo test                          # their tests
cargo build --release -p sioul-app  # the window (Qt 6.9 or newer)
tools/lint-qml.sh                   # the QML, after the window
tools/check-messages.py             # every message, every language
```

- **What it needs**, system by system: [Install](../guide/install.md) in the user guide, and [Building and running](building.md).
- **Images of each page**: `SIOUL_GRAB=<folder> sioul-app` shows each page in turn, saves it, and quits. With `XDG_CONFIG_HOME`, `XDG_DATA_HOME` and `XDG_STATE_HOME` pointed at a test folder, it shows invented data instead of yours; without a screen, add `QT_QPA_PLATFORM=offscreen QT_QUICK_BACKEND=software`.
- **A demonstration, off the network**: `SIOUL_DEMO=1` keeps Sioul from fetching or sending anything by itself. `tools/demo/make-demo.py --into <folder>` writes an invented profile for it, and `tools/demo/screenshots.sh` takes this website's pictures from it.
- **Nothing is tested against real accounts.** Writing to a mail server is tested against GreenMail, contacts and calendars against Radicale, Google Tasks and GitHub against stand-ins of their APIs (`tools/`), OpenPGP against GnuPG. How: [Building and running](building.md).
- **Three systems**: `.github/workflows/build.yml` builds and tests on Linux, Windows and macOS on each push to `main` that touches code; started by hand, it also makes a Windows folder and a macOS `.dmg` with Qt beside the program.

The rules of the code ([Architecture](architecture.md#code-style)): one task per function; comments give the reason and the reference (the RFC, the paper, the document); every rule the window applies has a test, on invented mail using reserved example domains (RFC 2606), and no personal data.

## Where things are

| Path | What |
|---|---|
| `crates/sioul-core/src/` | the core: `porch.rs`, `areas.rs`, `quiet.rs`, `plan.rs`, `tasks.rs`, `view.rs`… |
| `crates/sioul-core/locales/` | the translations |
| `crates/sioul-core/tests/fixtures/` | invented mail, for the tests and the demonstration |
| `crates/sioul-sync/src/` | the network, the keyring, sync, sharing |
| `crates/sioul-cli/src/` | the command line; `mcp/`, the server for agents |
| `crates/sioul-app/` | the window: `src/`, `qml/`, `cpp/`, `icons/` |
| `crates/sioul-learn/` | the spam filter's training, on computers |
| `android/` | the phone's app: `CMakeLists.txt`, `main.cpp`, the manifest and the Java classes in `package/` |
| `docs/` | the design notes, shown in this section; `docs/research/`, the research behind them |
| `examples/` | a configuration, projects and budgets, and `demo.toml` for trying the command line on invented mail |
| `presets/sites.json` | the usual sites; `tools/check-presets.py` checks every address |
| `tools/` | the QML linter, the messages' and the presets' checkers, the icon bundler, the API stand-ins, and `demo/`, the invented profile and this website's pictures |
| `packaging/` | the Flatpak manifest, the Windows installer's script, the macOS files, used by `.github/workflows/release.yml` for each version |
| `data/` | the desktop entry (`com.aurelienpierre.Sioul.desktop`), the AppStream file, the icon's drawings |
| `website/` | this website |

## The notes

- **Design**: [the design](design.md) and its rule; [what the research says](research.md), each finding with the rule it gives; [the roadmap](roadmap.md).
- **Research**: [the research notes](research/README.md), in detail: tasks, "Done for today", life admin, wearables, Google and security keys, the licences of what Sioul bundles.
- **Building**: [building and running](building.md), [architecture](architecture.md), [languages](i18n.md).
- **Mail**: [the Porch](porch.md), [mail, contacts and calendars](client.md), [the case store](case-store.md), [Virtual Secretary](virtual-secretary.md).
- **Tasks and time**: [tasks, notes, links and focus](tasks.md), [what a day holds](capacity.md), [areas and hours](areas.md), [reminders](reminders.md), [projects, time and invoices](projects.md), [sounds](sounds.md).
- **Money and papers**: [accounting](accounting.md), [papers](papers.md).
- **Elsewhere**: [Google](google.md), [GitHub](github.md), [sites](sites.md), [health](health.md), [several devices](database.md), [AI](ai.md), [AI agents through MCP](mcp.md).

## How this website is built

- **[Zensical](https://zensical.org) 0.0.67.** The configuration is `website/zensical.toml`. The user guide is written by hand in `website/docs/` (`index.md`, `guide/`, `privacy.md`), and so is this page (`website/docs/dev/index.md`).
- **The notes are never copied by hand.** `website/build.sh` copies `docs/`, with `docs/research/`, into `website/docs/dev/` at each build (that folder is ignored by git, but for this page and `api.md`). On the copies only, links that leave `docs/` (to `../examples/`, `../packaging/`) become their address on GitHub. It then says which notes are missing from the navigation, and builds.
- **A new note** in `docs/` is built with the rest; to show it in this section's sidebar, add it to the "For developers" part of `nav` in `website/zensical.toml`.
- **The [API reference](api.md)** is made from the code's comments. `build.sh` writes [the QML page](qml.md) at each build, with `tools/qml-docs.py`; `build.sh --api` also runs rustdoc and javadoc, and copies their pages into the site, under `api/rust/` and `api/java/`.
- **Locally**:

    ```
    python3 -m venv <a folder outside the repository>
    <that folder>/bin/pip install zensical==0.0.67
    PATH=<that folder>/bin:$PATH website/build.sh          # into website/site/
    PATH=<that folder>/bin:$PATH website/build.sh serve    # at http://localhost:8000/sioul/
    PATH=<that folder>/bin:$PATH website/build.sh --api    # with the Rust and Java references (Rust, Qt 6, a JDK)
    ```

- **Published** by `.github/workflows/pages.yml`, on each push to `main` that changes `website/`, `docs/`, the code the reference is made from (`crates/`, `android/package/`, `Cargo.toml`, `Cargo.lock`, `tools/qml-docs.py`) or the workflow, or by hand: the same `build.sh --strict`, then `build.sh --strict --api-only` for the API reference, then GitHub Pages. When the reference fails (a download, a warning of rustdoc or javadoc), the guide and the notes are published without it, and the run says so.
- **Screenshots** are in `website/docs/assets/screens/`, taken by `tools/demo/screenshots.sh` from an invented demonstration profile.
- Page addresses end in `.html`, because `docs/research.md` and `docs/research/README.md` would otherwise both become `research/index.html`.

## Talking about it

Questions, reports and ideas: [GitHub issues](https://github.com/aurelienpierre/sioul/issues). Sioul is made by one person, in the open: no support is promised.
