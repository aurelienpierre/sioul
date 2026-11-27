// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

//! `sioul store` and `sioul settings` (docs/store.md, "Commands"): Sioul's
//! sealed files read and written through the seal, so that a tool or an AI
//! agent the person runs can set Sioul up from outside. By default only the
//! configuration (`config/`) is given; any other file with `--any`; a
//! client's file only with `--clients`: a refusal of Sioul's own programs,
//! not a cryptographic wall (the keyring gives any program of the session
//! what it holds). `ls`, `check` and `unseal` neither name nor write out a
//! client's file without `--clients`, nor health or the person's own keys
//! without `--any`. `put` and `edit` refuse a file that does not read as its
//! kind, and one changed meanwhile; each write keeps the version it replaced.
//!
//! The key comes from the keyring, never prompting (`storekey`); on a
//! system without a keyring service, the store's passphrase is asked.

#[allow(unused_imports)]
use crate::files::Probe as _;
use crate::Session;
use clap::Subcommand;
use sioul_core::sealed::{self, Error, Expect, Folders, How, LogicalPath, Options, Store};
use sioul_sync::storekey::{self, KeyError};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};

#[derive(Subcommand)]
pub(crate) enum StoreCommand {
    /// The sealed files' folders, or one folder's files: sizes, versions, copies.
    Ls {
        /// A folder (config, data/mail/…) or a file; every folder when none.
        path: Option<String>,
        /// Health and your own keys named too.
        #[arg(long)]
        any: bool,
        /// Clients' files named too.
        #[arg(long)]
        clients: bool,
    },
    /// A configuration file unsealed to the standard output.
    Cat {
        path: String,
        /// Any of Sioul's files, not only its configuration.
        #[arg(long)]
        any: bool,
        /// A client's file.
        #[arg(long)]
        clients: bool,
        /// An earlier version (its number in `sioul store versions`).
        #[arg(long)]
        version: Option<usize>,
    },
    /// A configuration file sealed from the standard input; the version it replaces is kept.
    Put {
        path: String,
        #[arg(long)]
        any: bool,
        #[arg(long)]
        clients: bool,
    },
    /// A configuration file opened in $EDITOR through the memory-backed folder (Linux), sealed again when it is closed.
    Edit {
        path: String,
        #[arg(long)]
        any: bool,
        #[arg(long)]
        clients: bool,
    },
    /// A file's earlier versions, the newest first.
    Versions {
        path: String,
        #[arg(long)]
        any: bool,
        #[arg(long)]
        clients: bool,
    },
    /// An earlier version put back; the one it replaces becomes version 1, so the same command undoes it.
    Restore {
        path: String,
        /// Its number in `sioul store versions`.
        #[arg(long)]
        version: usize,
        #[arg(long)]
        any: bool,
        #[arg(long)]
        clients: bool,
    },
    /// Every copy of every file opened and compared with its folder's list.
    Check {
        /// Mends what can be mended (writers wait meanwhile).
        #[arg(long)]
        repair: bool,
        /// Health and your own keys named too.
        #[arg(long)]
        any: bool,
        /// Clients' files named too.
        #[arg(long)]
        clients: bool,
    },
    /// Every file Sioul keeps written in clear, each in its standard format, for any other program.
    Export {
        /// An empty folder of your own, outside Sioul's folders.
        #[arg(long)]
        to: PathBuf,
        /// The clients' files too.
        #[arg(long)]
        clients: bool,
    },
    /// A file set aside at a start (written beside what Sioul keeps by another program or an older Sioul) taken in on purpose: sealed at its own path, what it replaces kept as an earlier version.
    BringIn {
        /// Its path under data/set-aside/ (`sioul store ls data/set-aside --any` lists them).
        path: String,
        /// Any of Sioul's files, not only its configuration.
        #[arg(long)]
        any: bool,
        /// A client's file.
        #[arg(long)]
        clients: bool,
    },
    /// What "Start again" set aside, opened again with one of its ways back and put back as what Sioul keeps here (what it kept until now set aside in its turn, never removed); with no name, the ones set aside. Close Sioul's window first.
    Reopen {
        /// Its name (`sioul store reopen` lists them).
        when: Option<String>,
        /// With its recovery code (asked).
        #[arg(long)]
        code: bool,
        /// With your security key (its PIN asked).
        #[arg(long)]
        card: bool,
    },
    /// The store's files written plain in today's layout, for an older Sioul.
    Unseal {
        /// An empty folder of your own, outside Sioul's folders.
        #[arg(long)]
        to: PathBuf,
        /// Health and your own keys too.
        #[arg(long)]
        any: bool,
        /// The clients' files too.
        #[arg(long)]
        clients: bool,
    },
}

#[derive(Subcommand)]
pub(crate) enum SettingsCommand {
    /// One setting's value, by its name in the configuration (mail.history_weeks).
    Get { key: String },
    /// One setting changed as the window changes it, its value in JSON: true, 3, "text", [1, 2].
    Set { key: String, value: String },
}

/// What the store's errors say, in the session's language.
fn said(s: &Session, path: &str, e: &Error) -> String {
    match e {
        Error::NotOpen => s.say("store-key-locked", &[]),
        Error::NotCreated => s.say("store-not-created", &[]),
        Error::Exists => s.say("store-exists", &[]),
        Error::WrongKey => s.say("store-wrong-key", &[]),
        Error::Absent => s.say("store-absent", &[("path", path.to_string())]),
        Error::Damaged { path: what_path, .. } => s.say("store-damaged", &[("path", what_path.clone())]),
        Error::Changed { .. } => s.say("store-changed", &[("path", path.to_string())]),
        Error::Locked(why) => s.say("store-locked", &[("why", why.clone())]),
        Error::NoRoom => s.say("store-no-room", &[]),
        Error::Synced(folder) => s.say("store-synced", &[("folder", folder.display().to_string())]),
        Error::Refused(why) | Error::Change(why) | Error::Io(why) => s.say("store-refused", &[("why", why.clone())]),
        Error::NotText => s.say("store-not-text", &[("path", path.to_string())]),
    }
}

fn key_said(s: &Session, e: &KeyError) -> String {
    match e {
        KeyError::NoStore => s.say("store-not-created", &[]),
        KeyError::Locked => s.say("store-key-locked", &[]),
        KeyError::NoService => s.say("store-key-no-service", &[]),
        KeyError::Missing => s.say("store-key-missing", &[]),
        KeyError::WrongPassphrase => s.say("store-wrong-passphrase", &[]),
        KeyError::WrongCode => s.say("store-wrong-code", &[]),
        KeyError::NoCard(_) => s.say("store-no-card", &[]),
        KeyError::Other(why) => s.say("store-refused", &[("why", why.clone())]),
    }
}

/// The profile's store, opened with its key: from the keyring, else the passphrase where there is no keyring.
fn open(s: &Session) -> Result<Store, String> {
    let folders = Folders::of_profile();
    let key = match storekey::root_key(&folders) {
        Ok(key) => key,
        Err(KeyError::NoService) => {
            let passphrase = rpassword::prompt_password(s.say("store-passphrase", &[])).map_err(|e| e.to_string())?;
            storekey::from_passphrase(&folders, &passphrase).map_err(|e| key_said(s, &e))?
        }
        Err(e) => return Err(key_said(s, &e)),
    };
    // n is the store's own (`Store::set_versions`), the same for every process.
    Store::open(&folders, &key, Options::for_profile("cli")).map_err(|e| said(s, "", &e))
}

/// What a path needs to be named or written out: a client's file `--clients`, health and the person's own keys `--any`.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Private {
    Client,
    Personal,
}

fn private(path: &LogicalPath) -> Option<Private> {
    let clients = LogicalPath::new("data/clients").ok()?;
    if path.is_within(&clients) {
        return Some(Private::Client);
    }
    let personal = path.as_str().split('/').any(|part| part.starts_with("health")) || path.as_str().contains("/pgp/own");
    personal.then_some(Private::Personal)
}

fn shown(path: &LogicalPath, any: bool, clients: bool) -> bool {
    match private(path) {
        Some(Private::Client) => clients,
        Some(Private::Personal) => any,
        None => true,
    }
}

/// The path, if the flags given reach it: the configuration by default, a client's file with `--clients`, any other with `--any`.
fn reached(s: &Session, path: &str, any: bool, clients: bool) -> Result<LogicalPath, String> {
    let logical = LogicalPath::new(path).map_err(|_| s.say("store-bad-path", &[("path", path.to_string())]))?;
    let client = logical.is_within(&LogicalPath::new("data/clients").map_err(|e| e.to_string())?);
    if client && !clients {
        return Err(s.say("store-client-file", &[("path", path.to_string())]));
    }
    if !client && !any && logical.root_name() != "config" {
        return Err(s.say("store-not-config", &[("path", path.to_string())]));
    }
    Ok(logical)
}

/// Whether `bytes` read as the kind their name says (TOML, JSON, lines of JSON); a configuration as Sioul reads it.
fn parses(path: &LogicalPath, bytes: &[u8]) -> Result<(), String> {
    let name = path.file_name();
    let text = || std::str::from_utf8(bytes).map_err(|_| "not UTF-8 text".to_string());
    if path.as_str() == "config/config.toml" {
        toml::from_str::<sioul_core::config::Config>(text()?).map(|_| ()).map_err(|e| e.message().to_string())
    } else if name.ends_with(".toml") {
        text()?.parse::<toml::Table>().map(|_| ()).map_err(|e| e.message().to_string())
    } else if name.ends_with(".json") {
        serde_json::from_slice::<serde_json::Value>(bytes).map(|_| ()).map_err(|e| e.to_string())
    } else if name.ends_with(".jsonl") {
        text()?.lines().filter(|l| !l.trim().is_empty()).try_for_each(|l| serde_json::from_str::<serde_json::Value>(l).map(|_| ())).map_err(|e| e.to_string())
    } else {
        Ok(())
    }
}

fn when(ms: i64) -> String {
    jiff::Timestamp::from_millisecond(ms).map_or_else(|_| ms.to_string(), |t| t.to_zoned(jiff::tz::TimeZone::system()).strftime("%Y-%m-%d %H:%M").to_string())
}

pub(crate) fn run(s: &Session, command: StoreCommand) -> Result<(), String> {
    match command {
        StoreCommand::Ls { path, any, clients } => ls(s, path.as_deref(), any, clients),
        StoreCommand::Cat { path, any, clients, version } => {
            let logical = reached(s, &path, any, clients)?;
            let store = open(s)?;
            let bytes = match version {
                Some(n) => {
                    let versions = store.versions(&logical).map_err(|e| said(s, &path, &e))?;
                    let earlier: Vec<_> = versions.iter().filter(|v| !v.current).collect();
                    let chosen = earlier.get(n.wrapping_sub(1)).ok_or_else(|| s.say("store-no-such-version", &[("path", path.clone()), ("version", n.to_string())]))?;
                    store.read_version(&logical, chosen.generation).map_err(|e| said(s, &path, &e))?.bytes
                }
                None => match store.read(&logical) {
                    Ok(Some(opened)) => {
                        // Its current copy did not open: said, so that an earlier version is never taken for the file.
                        match opened.from {
                            sealed::Source::Earlier { written_ms, .. } => eprintln!("{}", s.say("store-cat-earlier", &[("path", path.clone()), ("when", when(written_ms))])),
                            sealed::Source::Twin | sealed::Source::Current => {}
                        }
                        opened.bytes
                    }
                    Ok(None) => return Err(said(s, &path, &Error::Absent)),
                    Err(Error::Refused(_)) => match store.read_lines(&logical).map_err(|e| said(s, &path, &e))? {
                        Some(lines) => {
                            if !lines.damaged.is_empty() {
                                eprintln!("{}", s.say("store-cat-lines-lost", &[("path", path.clone()), ("count", lines.damaged.len().to_string())]));
                            }
                            lines.lines.into_iter().map(|(_, l)| l + "\n").collect::<String>().into_bytes()
                        }
                        None => return Err(said(s, &path, &Error::Absent)),
                    },
                    Err(e) => return Err(said(s, &path, &e)),
                },
            };
            std::io::stdout().write_all(&bytes).map_err(|e| e.to_string())
        }
        StoreCommand::Put { path, any, clients } => {
            let logical = reached(s, &path, any, clients)?;
            let store = open(s)?;
            // The generation it replaces, taken before the new bytes are read: a change meanwhile is refused.
            let expect = match store.stat(&logical).map_err(|e| said(s, &path, &e))? {
                Some(entry) => Expect::Generation(entry.generation),
                None => Expect::Absent,
            };
            let mut bytes = Vec::new();
            std::io::stdin().read_to_end(&mut bytes).map_err(|e| e.to_string())?;
            parses(&logical, &bytes).map_err(|why| s.say("store-not-parsed", &[("path", path.clone()), ("why", why)]))?;
            let written = store.write(&logical, &bytes, How { expect, ..How::default() }).map_err(|e| said(s, &path, &e))?;
            eprintln!("{}", s.say("store-written", &[("path", path), ("version", written.generation.to_string())]));
            Ok(())
        }
        StoreCommand::Edit { path, any, clients } => edit(s, &path, any, clients),
        StoreCommand::Versions { path, any, clients } => {
            let logical = reached(s, &path, any, clients)?;
            let store = open(s)?;
            let versions = store.versions(&logical).map_err(|e| said(s, &path, &e))?;
            let mut n = 0;
            for v in versions {
                let number = if v.current {
                    s.say("store-version-current", &[])
                } else {
                    n += 1;
                    n.to_string()
                };
                println!("{number}\t{}\t{} B\t{}", when(v.written_ms), v.size, crate::one_line(&v.origin));
            }
            Ok(())
        }
        StoreCommand::Restore { path, version, any, clients } => {
            let logical = reached(s, &path, any, clients)?;
            let store = open(s)?;
            let versions = store.versions(&logical).map_err(|e| said(s, &path, &e))?;
            let earlier: Vec<_> = versions.iter().filter(|v| !v.current).collect();
            let chosen = earlier.get(version.wrapping_sub(1)).ok_or_else(|| s.say("store-no-such-version", &[("path", path.clone()), ("version", version.to_string())]))?;
            store.restore(&logical, chosen.generation).map_err(|e| said(s, &path, &e))?;
            eprintln!("{}", s.say("store-restored", &[("path", path.clone()), ("version", version.to_string())]));
            Ok(())
        }
        StoreCommand::Check { repair, any, clients } => {
            let store = open(s)?;
            let report = store.check(repair).map_err(|e| said(s, "", &e))?;
            print_report(s, &report, any, clients);
            let lost = report.folders.iter().any(|f| !f.lost.is_empty() || f.list_damaged);
            if lost { Err(s.say("store-check-lost", &[])) } else { Ok(()) }
        }
        StoreCommand::Unseal { to, any, clients } => unseal(s, &to, any, clients),
        StoreCommand::Reopen { when, code, card } => reopen(s, when.as_deref(), code, card),
        StoreCommand::BringIn { path, any, clients } => bring_in(s, &path, any, clients),
        StoreCommand::Export { to, clients } => export(s, &to, clients),
    }
}

/// A file set aside at a start (`data/set-aside/<day>/<its path>`) taken in on
/// purpose: sealed at its own path, what it replaces kept as an earlier
/// version, the set-aside copy then removed.
fn bring_in(s: &Session, path: &str, any: bool, clients: bool) -> Result<(), String> {
    let aside = LogicalPath::new(path).map_err(|_| s.say("store-bad-path", &[("path", path.to_string())]))?;
    let not_aside = || s.say("store-bring-in-not-aside", &[("path", path.to_string())]);
    let target = aside.as_str().strip_prefix("data/set-aside/").and_then(|rest| rest.split_once('/')).map(|(_, target)| target.to_string()).ok_or_else(not_aside)?;
    let target = reached(s, &target, any, clients)?;
    let store = open(s)?;
    let bytes = store.read(&aside).map_err(|e| said(s, path, &e))?.ok_or_else(not_aside)?.bytes;
    store.write(&target, &bytes, How::default()).map_err(|e| said(s, target.as_str(), &e))?;
    store.remove(&aside, Expect::Any).map_err(|e| said(s, path, &e))?;
    eprintln!("{}", s.say("store-bring-in-done", &[("path", path.to_string()), ("target", target.to_string())]));
    Ok(())
}

/// A store "Start again" set aside, opened again with one of its own ways
/// back (its passphrase by default, else `--code` or `--card`) and put back
/// as what Sioul keeps here; with no name, the ones set aside and their ways.
fn reopen(s: &Session, when: Option<&str>, code: bool, card: bool) -> Result<(), String> {
    let folders = Folders::of_profile();
    let aside = storekey::set_aside_stores(&folders);
    let Some(when) = when else {
        if aside.is_empty() {
            println!("{}", s.say("store-reopen-none", &[]));
        }
        for when in &aside {
            let ways: Vec<String> = storekey::set_aside_ways(&folders, when).into_iter().map(|w| w.kind).collect();
            println!("{}", s.say("store-reopen-list", &[("when", when.clone()), ("ways", ways.join(", "))]));
        }
        if let Some(first) = aside.first() {
            eprintln!("{}", s.say("store-reopen-which", &[("when", first.clone())]));
        }
        return Ok(());
    };
    if !aside.iter().any(|w| w == when) {
        return Err(s.say("store-reopen-none", &[]));
    }
    if storekey::set_aside_ways(&folders, when).is_empty() {
        return Err(s.say("store-reopen-no-way", &[]));
    }
    let key = if card {
        let pin = zeroize::Zeroizing::new(rpassword::prompt_password(s.say("store-reopen-pin", &[])).map_err(|e| e.to_string())?);
        storekey::set_aside_key(&folders, when, storekey::Given::Card { pin: &pin, readers: sioul_sync::securitykey::readers() })
    } else if code {
        let typed = zeroize::Zeroizing::new(rpassword::prompt_password(s.say("store-reopen-code", &[])).map_err(|e| e.to_string())?);
        storekey::set_aside_key(&folders, when, storekey::Given::Code(&typed))
    } else {
        let passphrase = zeroize::Zeroizing::new(rpassword::prompt_password(s.say("store-passphrase", &[])).map_err(|e| e.to_string())?);
        storekey::set_aside_key(&folders, when, storekey::Given::Passphrase(&passphrase))
    }
    .map_err(|e| key_said(s, &e))?;
    let before: Vec<String> = storekey::set_aside_stores(&folders);
    storekey::reopen_set_aside(&folders, when, &key).map_err(|e| key_said(s, &e))?;
    let now = storekey::set_aside_stores(&folders);
    let then = now.iter().find(|w| !before.contains(w)).map(|w| format!(" ({w})")).unwrap_or_default();
    eprintln!("{}", s.say("store-reopen-done", &[("when", when.to_string()), ("then", then)]));
    Ok(())
}

/// Every file Sioul keeps written in clear under `to`, in the layout Sioul has
/// always used (config/, data/, state/, cache/), each in its standard format:
/// TOML, iCalendar, vCard, Maildir, JSON, PDF. Health and the person's own
/// keys go too (it is their copy); a client's file only with `--clients`.
fn export(s: &Session, to: &Path, clients: bool) -> Result<(), String> {
    let folders = Folders::of_profile();
    if [&folders.config, &folders.data, &folders.state, &folders.cache].iter().any(|f| to.starts_with(f) || f.starts_with(to)) {
        return Err(s.say("store-unseal-inside", &[]));
    }
    if crate::files::read_dir(to).is_ok_and(|mut d| d.next().is_some()) {
        return Err(s.say("store-unseal-not-empty", &[("folder", to.display().to_string())]));
    }
    let store = open(s)?;
    let left = std::cell::Cell::new(0usize);
    let keep = |path: &LogicalPath| {
        let kept = clients || private(path) != Some(Private::Client);
        if !kept {
            left.set(left.get() + 1);
        }
        kept
    };
    let (written, damaged) = sealed::files::export(&store, to, &keep).map_err(|e| format!("{}: {e}", to.display()))?;
    eprintln!("{}", s.say("store-exported", &[("count", written.to_string()), ("folder", to.display().to_string())]));
    if left.get() > 0 {
        eprintln!("{}", s.say("store-export-clients-left", &[("count", left.get().to_string())]));
    }
    if !damaged.is_empty() {
        return Err(s.say("store-export-damaged", &[("count", damaged.len().to_string())]));
    }
    Ok(())
}

fn ls(s: &Session, path: Option<&str>, any: bool, clients: bool) -> Result<(), String> {
    let store = open(s)?;
    let mut hidden = 0usize;
    let Some(path) = path else {
        for root in sealed::Root::ALL {
            let root = LogicalPath::new(root.name()).map_err(|e| e.to_string())?;
            let mut all: std::collections::BTreeSet<LogicalPath> = store.folders(&root).map_err(|e| said(s, root.as_str(), &e))?.into_iter().collect();
            all.insert(root.clone());
            for folder in all {
                let files = store.list(&folder).map_err(|e| said(s, folder.as_str(), &e))?;
                if files.is_empty() {
                    continue;
                }
                if !shown(&folder, any, clients) || files.iter().any(|f| !shown(&f.path, any, clients)) && !files.iter().any(|f| shown(&f.path, any, clients)) {
                    hidden += files.len();
                    continue;
                }
                println!("{}/\t{}", crate::one_line(folder.as_str()), s.say("store-ls-files", &[("count", files.len().to_string())]));
            }
        }
        if hidden > 0 {
            eprintln!("{}", s.say("store-ls-hidden", &[("count", hidden.to_string())]));
        }
        return Ok(());
    };
    let logical = LogicalPath::new(path).map_err(|_| s.say("store-bad-path", &[("path", path.to_string())]))?;
    let entries = match (!logical.is_root()).then(|| store.stat(&logical)).transpose().map_err(|e| said(s, path, &e))?.flatten() {
        Some(entry) => vec![entry],
        None => store.list(&logical).map_err(|e| said(s, path, &e))?,
    };
    for entry in entries {
        if !shown(&entry.path, any, clients) {
            hidden += 1;
            continue;
        }
        let copies = 1 + usize::from(entry.twin) + entry.versions;
        let damaged = if entry.damaged { format!("\t{}", s.say("store-ls-damaged", &[])) } else { String::new() };
        let attrs: Vec<String> = entry.attrs.iter().map(|(k, v)| format!("{k}={v}")).collect();
        println!("{}\t{} B\t{}\t{}\t{}{damaged}\t{}", crate::one_line(entry.path.as_str()), entry.size, entry.generation, s.say("store-ls-copies", &[("count", copies.to_string())]), when(entry.written_ms), crate::one_line(&attrs.join(" ")));
    }
    for folder in store.folders(&logical).map_err(|e| said(s, path, &e))? {
        if folder.folder() == logical && shown(&folder, any, clients) {
            println!("{}/", crate::one_line(folder.as_str()));
        }
    }
    if hidden > 0 {
        eprintln!("{}", s.say("store-ls-hidden", &[("count", hidden.to_string())]));
    }
    Ok(())
}

/// The memory-backed folder where a file is opened for $EDITOR: Linux's runtime folder, nowhere else.
fn memory_folder() -> Option<PathBuf> {
    if !cfg!(target_os = "linux") {
        return None;
    }
    let dir = PathBuf::from(std::env::var_os("XDG_RUNTIME_DIR")?);
    dir.present_dir().then_some(dir)
}

fn edit(s: &Session, path: &str, any: bool, clients: bool) -> Result<(), String> {
    let logical = reached(s, path, any, clients)?;
    let Some(runtime) = memory_folder() else { return Err(s.say("store-edit-elsewhere", &[])) };
    let store = open(s)?;
    let (bytes, generation) = match store.read(&logical) {
        Ok(Some(opened)) if opened.from == sealed::Source::Current || opened.from == sealed::Source::Twin => (opened.bytes, Some(opened.generation)),
        // An earlier version is never edited as the file: put it back first.
        Ok(Some(_)) => return Err(said(s, path, &Error::Damaged { path: path.to_string(), what: String::new() })),
        Ok(None) => (Vec::new(), None),
        Err(e) => return Err(said(s, path, &e)),
    };
    // A folder of its own, yours alone, its file named as the file is (its kind for the editor).
    let dir = runtime.join(format!("sioul-edit-{}", std::process::id()));
    let mut builder = crate::files::DirBuilder::new();
    #[cfg(unix)]
    builder.mode(0o700);
    builder.create(&dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    let file = dir.join(logical.file_name().replace(['/', '\\'], "_"));
    let mut keep_copy = false;
    let done = (|| {
        let mut options = crate::files::OpenOptions::new();
        options.write(true).create_new(true);
        #[cfg(unix)]
        options.mode(0o600);
        options.open(&file).and_then(|mut f| f.write_all(&bytes)).map_err(|e| format!("{}: {e}", file.display()))?;
        let editor = std::env::var("VISUAL").or_else(|_| std::env::var("EDITOR")).unwrap_or_else(|_| "vi".to_string());
        let mut words = editor.split_whitespace();
        let program = words.next().unwrap_or("vi");
        let status = std::process::Command::new(program).args(words).arg(&file).status().map_err(|e| format!("{program}: {e}"))?;
        if !status.success() {
            return Err(s.say("store-edit-cancelled", &[]));
        }
        let edited = crate::files::read(&file).map_err(|e| format!("{}: {e}", file.display()))?;
        if edited == bytes {
            eprintln!("{}", s.say("store-unchanged", &[("path", path.to_string())]));
            return Ok(());
        }
        // From here on, what was edited is kept when it cannot be written: nothing typed is lost.
        keep_copy = true;
        parses(&logical, &edited).map_err(|why| s.say("store-not-parsed", &[("path", path.to_string()), ("why", why)]))?;
        let how = How { expect: generation.map_or(Expect::Absent, Expect::Generation), ..How::default() };
        let written = store.write(&logical, &edited, how).map_err(|e| said(s, path, &e))?;
        keep_copy = false;
        eprintln!("{}", s.say("store-written", &[("path", path.to_string()), ("version", written.generation.to_string())]));
        Ok(())
    })();
    if done.is_err() && keep_copy {
        eprintln!("{}", s.say("store-edit-kept", &[("file", file.display().to_string())]));
    } else {
        let _ = crate::files::remove_dir_all(&dir);
    }
    done
}

/// Every file of the store written plain under `to`, in today's layout; clients' files with `--clients`, health and keys with `--any`.
fn unseal(s: &Session, to: &Path, any: bool, clients: bool) -> Result<(), String> {
    let folders = Folders::of_profile();
    if [&folders.config, &folders.data, &folders.state, &folders.cache].iter().any(|f| to.starts_with(f) || f.starts_with(to)) {
        return Err(s.say("store-unseal-inside", &[]));
    }
    if crate::files::read_dir(to).is_ok_and(|mut d| d.next().is_some()) {
        return Err(s.say("store-unseal-not-empty", &[("folder", to.display().to_string())]));
    }
    let store = open(s)?;
    let (mut written, mut left) = (0usize, 0usize);
    for root in sealed::Root::ALL {
        let root = LogicalPath::new(root.name()).map_err(|e| e.to_string())?;
        let mut all: std::collections::BTreeSet<LogicalPath> = store.folders(&root).map_err(|e| said(s, root.as_str(), &e))?.into_iter().collect();
        all.insert(root.clone());
        for folder in all {
            for entry in store.list(&folder).map_err(|e| said(s, folder.as_str(), &e))? {
                if !shown(&entry.path, any, clients) {
                    left += 1;
                    continue;
                }
                let bytes = match entry.kind {
                    sealed::Kind::File => store.read(&entry.path).map_err(|e| said(s, entry.path.as_str(), &e))?.map(|o| o.bytes),
                    sealed::Kind::Lines => store.read_lines(&entry.path).map_err(|e| said(s, entry.path.as_str(), &e))?.map(|l| l.lines.into_iter().map(|(_, l)| l + "\n").collect::<String>().into_bytes()),
                };
                let Some(bytes) = bytes else { continue };
                let target = entry.path.as_str().split('/').fold(to.to_path_buf(), |path, part| path.join(part));
                if let Some(parent) = target.parent() {
                    crate::files::create_dir_all(parent).map_err(|e| format!("{}: {e}", parent.display()))?;
                }
                let mut options = crate::files::OpenOptions::new();
                options.write(true).create_new(true);
                #[cfg(unix)]
                options.mode(0o600);
                options.open(&target).and_then(|mut f| f.write_all(&bytes)).map_err(|e| format!("{}: {e}", target.display()))?;
                written += 1;
            }
        }
    }
    eprintln!("{}", s.say("store-unsealed", &[("count", written.to_string()), ("folder", to.display().to_string())]));
    if left > 0 {
        eprintln!("{}", s.say("store-unseal-left", &[("count", left.to_string())]));
    }
    Ok(())
}

fn print_report(s: &Session, report: &sealed::Report, any: bool, clients: bool) {
    let name_of = |folder: &str, name: &str| -> String {
        let shown_here = LogicalPath::new(&format!("{folder}/{name}")).is_ok_and(|p| shown(&p, any, clients));
        if shown_here { crate::one_line(name) } else { s.say("store-check-private", &[]) }
    };
    // A client's folder is named only with --clients (its name may be the client's): counted under one word otherwise.
    let folder_of = |folder: &str| -> String {
        let client = LogicalPath::new(folder).is_ok_and(|p| private(&p) == Some(Private::Client));
        if client && !clients { s.say("store-check-private", &[]) } else { crate::one_line(folder) }
    };
    for folder in &report.folders {
        let copies: Vec<String> = folder.copies.iter().map(|(good, files)| s.say("store-check-copies", &[("files", files.to_string()), ("good", good.to_string())])).collect();
        println!("{}", s.say("store-check-folder", &[("folder", folder_of(&folder.folder)), ("files", folder.files.to_string()), ("whole", folder.whole.to_string()), ("copies", copies.join(", "))]));
        if folder.list_damaged {
            println!("  {}", s.say("store-check-list-damaged", &[]));
        }
        for (name, good, kept) in &folder.thin {
            println!("  {}", s.say("store-check-thin", &[("name", name_of(&folder.folder, name)), ("good", good.to_string()), ("kept", kept.to_string())]));
        }
        for (name, from) in &folder.mended {
            let from = match from {
                sealed::MendedFrom::SecondCopy => s.say("store-check-from-twin", &[]),
                sealed::MendedFrom::Earlier { written_ms } => s.say("store-check-from-earlier", &[("when", when(*written_ms))]),
            };
            println!("  {}", s.say("store-check-mended", &[("name", name_of(&folder.folder, name)), ("from", from)]));
        }
        for name in &folder.lost {
            println!("  {}", s.say("store-check-lost-file", &[("name", name_of(&folder.folder, name))]));
        }
        if folder.lost_lines > 0 {
            println!("  {}", s.say("store-check-lines", &[("count", folder.lost_lines.to_string())]));
        }
    }
    for path in &report.newer {
        let shown_here = LogicalPath::new(path).is_ok_and(|p| shown(&p, any, clients));
        let name = if shown_here { crate::one_line(path) } else { s.say("store-check-private", &[]) };
        println!("{}", s.say(if report.repaired { "store-check-newer-taken" } else { "store-check-newer" }, &[("name", name)]));
    }
    if report.unlisted > 0 {
        println!("{}", s.say(if report.repaired { "store-check-unlisted-taken" } else { "store-check-unlisted" }, &[("count", report.unlisted.to_string())]));
    }
}

/// Settings the window changes through more than the configuration (a keyring,
/// files moved, a process started): refused here, changed in the window.
const WINDOW_ONLY: &[&str] = &["ai_key", "github_token", "task_categories", "reminders_closed", "papers.folder", "letters.folder", "case_store", "passwords_shown", "home_card_details", sioul_core::mapsearch::KEY];

/// `sioul settings get|set`: one setting, as the window reads and changes it
/// (`settings::change`: its lock, the values fresh from the file), in the
/// configuration where it lives now; a value that is not JSON, or a change
/// after which the configuration no longer reads, is refused and nothing stays changed.
pub(crate) fn settings(s: &Session, command: SettingsCommand) -> Result<(), String> {
    match command {
        SettingsCommand::Get { key } => {
            let text = crate::files::read_to_string(&s.config_path).map_err(|e| format!("{}: {e}", s.config_path.display()))?;
            let table: toml::Table = text.parse().map_err(|e: toml::de::Error| e.message().to_string())?;
            let mut value = toml::Value::Table(table);
            for part in key.split('.') {
                value = value.get(part).cloned().ok_or_else(|| s.say("settings-unset", &[("key", key.clone())]))?;
            }
            match value {
                toml::Value::String(text) => println!("{text}"),
                other => println!("{other}"),
            }
            Ok(())
        }
        SettingsCommand::Set { key, value } => {
            if WINDOW_ONLY.contains(&key.as_str()) {
                return Err(s.say("settings-in-window", &[("key", key)]));
            }
            let parsed: sioul_core::config::SettingValue = serde_json::from_str(&value).map_err(|_| s.say("settings-not-json", &[("value", value.clone())]))?;
            let before = crate::files::read(&s.config_path).ok();
            sioul_core::settings::change(&s.config_path, &key, None, &parsed)?;
            if let Err(why) = sioul_core::config::Config::load(&s.config_path) {
                // Put back as it was: a configuration that no longer reads is never left behind.
                if let Some(before) = before {
                    sioul_core::filelock::replace(&s.config_path, &before).map_err(|e| e.to_string())?;
                }
                return Err(s.say("settings-broke", &[("key", key), ("why", why)]));
            }
            eprintln!("{}", s.say("settings-set", &[("key", key), ("value", value)]));
            Ok(())
        }
    }
}
