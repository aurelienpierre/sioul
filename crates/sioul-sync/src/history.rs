// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

//! What a shared file held before another device's change was written into it
//! (docs/database.md, "Earlier versions"), kept on this device only and never
//! shared: `<state>/share/history/<part>/<file>/<time>`. Of each file, the last
//! 20 versions are kept, and all those of the last 30 days; of a file no longer
//! here, those of the last 30 days only; never more than a cap in all, the
//! oldest going first. A note or a paper whose content a record named is kept
//! by reference (`<time>.sealed`), 12 days: its content is sealed in the
//! sharing folder already, and stays there longer than that. Any version can be put back
//! (`share::put_back`): the file as it is then is kept first, and the next
//! exchange sends what was put back as a change made here.

use std::path::{Path, PathBuf};

/// Versions kept of each file, however old.
pub const VERSIONS: usize = 20;
/// Days during which every version is kept.
pub const DAYS: i64 = 30;
/// Days a version kept by reference stays: its content stays in the sharing
/// folder two weeks after its last use, less a day at most (`blobs::UNUSED_DAYS`,
/// `blobs::sweep`), counted from when its writer last knew it in use: when
/// the change replacing it was made there, or read there if later. A version
/// kept here within a day of that change therefore always opens. A change received later (a
/// device away for days) may keep a reference whose content went from the
/// folder meanwhile: putting it back then says it is missing, and writes
/// nothing. 60 days until 15 October 2026, when the folder kept contents 90.
/// Kept by reference rather than copied: a copy on every device receiving a
/// change would cost each one's disk up to the history's cap (a GB, on a
/// phone too) and hold a change back where the disk is short.
pub const SEALED_DAYS: i64 = 12;
const _: () = assert!(SEALED_DAYS < crate::blobs::UNUSED_DAYS - 1);
/// A version kept by reference: its name ends so.
const SEALED: &str = ".sealed";
const DAY: i64 = 86_400_000;

/// A version kept: its name (when it was kept), that moment (milliseconds),
/// its size; kept by reference, its content's hash (sealed in the sharing folder).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Version {
    pub stamp: String,
    pub at: i64,
    pub size: u64,
    pub sealed: Option<String>,
}

/// Where a version is: a copy here, or a content sealed in the sharing folder.
#[derive(Debug, PartialEq, Eq)]
pub enum Kept {
    Copy(PathBuf),
    Sealed { hash: String, size: u64 },
}

/// Where versions are kept: beside the sharing's memory (`<state>/share/history`).
pub fn root(memory: &Path) -> PathBuf {
    memory.parent().map_or_else(|| PathBuf::from("history"), |share| share.join("history"))
}

/// A version's name: when it was kept, in UTC, to the millisecond ("2026-10-05T14-05-33.120Z").
fn stamp(ms: i64) -> String {
    let Ok(at) = jiff::Timestamp::from_millisecond(ms) else { return ms.to_string() };
    format!("{}.{:03}Z", at.strftime("%Y-%m-%dT%H-%M-%S"), ms.rem_euclid(1000))
}

/// When a version was kept (milliseconds), from its name.
pub fn kept_at(stamp: &str) -> Option<i64> {
    let (seconds, ms) = stamp.strip_suffix('Z')?.split_once('.')?;
    let at = jiff::civil::DateTime::strptime("%Y-%m-%dT%H-%M-%S", seconds).ok()?.to_zoned(jiff::tz::TimeZone::UTC).ok()?;
    Some(at.timestamp().as_millisecond() + ms.parse::<i64>().ok()?)
}

/// A part's folder in the history: one plain name, never outside it.
fn part_dir(root: &Path, part: &str) -> Option<PathBuf> {
    plain(part).then(|| root.join(part))
}

fn plain(piece: &str) -> bool {
    !piece.is_empty() && piece != "." && piece != ".." && !piece.contains(['/', '\\', ':'])
}

/// A file's folder of versions: its name in the records, as a path below its
/// part's folder, never outside it, on Windows too.
fn below(root: &Path, part: &str, file: &str) -> Option<PathBuf> {
    let dir = part_dir(root, part)?;
    file.split('/').all(plain).then(|| file.split('/').fold(dir, |path, piece| path.join(piece)))
}

/// A name for a new version in `dir`, none taken.
fn new_stamp(dir: &Path, now_ms: i64) -> String {
    let mut ms = now_ms;
    while dir.join(stamp(ms)).exists() || dir.join(format!("{}{SEALED}", stamp(ms))).exists() {
        ms += 1;
    }
    stamp(ms)
}

/// A file as it is now, copied before another device's change is written over
/// it, or takes it out; then the oldest versions past what is kept go. Refused
/// when the copy would leave the disk short (`disk::kept_free`): nothing is
/// written over a file whose copy could not be kept.
pub fn keep(root: &Path, part: &str, file: &str, path: &Path, now_ms: i64) -> Result<(), String> {
    let dir = below(root, part, file).ok_or_else(|| format!("{file}: not a file's name"))?;
    let size = std::fs::metadata(path).map_or(0, |m| m.len());
    if !crate::disk::fits(root, size) {
        return Err(format!("share-no-room:{file}"));
    }
    crate::share::private_dirs(&dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    let target = dir.join(new_stamp(&dir, now_ms));
    // Copied under a hidden name, yours alone, then renamed: never half a version.
    let temporary = crate::share::temporary(&target);
    slow_copy();
    crate::share::copy_private(path, &temporary).and_then(|()| std::fs::rename(&temporary, &target)).map_err(|e| {
        let _ = std::fs::remove_file(&temporary);
        format!("{}: {e}", target.display())
    })?;
    prune_file(&dir, now_ms, false);
    Ok(())
}

/// A file as it stands, to tell whether it changed: its size, its time and,
/// on Unix, its inode (a file written anew and renamed into place is another).
fn standing(path: &Path) -> Option<(u64, Option<std::time::SystemTime>, u64)> {
    let meta = std::fs::metadata(path).ok()?;
    #[cfg(unix)]
    let inode = std::os::unix::fs::MetadataExt::ino(&meta);
    #[cfg(not(unix))]
    let inode = 0;
    Some((meta.len(), meta.modified().ok(), inode))
}

/// A copy of a file as it is now, made ahead of the exchanges' lock (Earlier
/// versions ▸ Put back, `share::put_back`): under a hidden name in its folder
/// of versions, yours alone, dated now while it waits (an exchange's sweep
/// leaves a leftover alone its first hour), its writing handle closed there
/// (on Windows, where its close has the antivirus read it whole: job A).
/// Kept as a version under the lock by a rename alone, once the file is
/// seen as it was copied (`Prepared::keep`); taken away otherwise.
pub struct Prepared {
    dir: PathBuf,
    temporary: PathBuf,
    path: PathBuf,
    copied: Option<(u64, Option<std::time::SystemTime>, u64)>,
}

/// The copy `Prepared` holds, of `path` as it is now. Refused as `keep`
/// refuses: no room for it.
pub fn prepare(root: &Path, part: &str, file: &str, path: &Path) -> Result<Prepared, String> {
    let dir = below(root, part, file).ok_or_else(|| format!("{file}: not a file's name"))?;
    let size = std::fs::metadata(path).map_or(0, |m| m.len());
    if !crate::disk::fits(root, size) {
        return Err(format!("share-no-room:{file}"));
    }
    crate::share::private_dirs(&dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    let copied = standing(path);
    let temporary = crate::share::temporary(&dir.join("prepared"));
    slow_copy();
    if let Err(e) = crate::share::copy_private(path, &temporary) {
        let _ = std::fs::remove_file(&temporary);
        return Err(format!("{}: {e}", temporary.display()));
    }
    let _ = crate::share::set_time(&temporary, std::time::SystemTime::now());
    Ok(Prepared { dir, temporary, path: path.to_path_buf(), copied })
}

impl Prepared {
    /// Kept as the file's version at `now_ms` by a rename, when the file is
    /// still as it was copied: whether it was (else the caller keeps the file
    /// as it is now, `keep`). Under the exchanges' lock.
    pub fn keep(self, now_ms: i64) -> bool {
        if self.copied.is_none() || standing(&self.path) != self.copied {
            return false;
        }
        let target = self.dir.join(new_stamp(&self.dir, now_ms));
        if std::fs::rename(&self.temporary, &target).is_err() {
            return false;
        }
        prune_file(&self.dir, now_ms, false);
        true
    }
}

impl Drop for Prepared {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.temporary);
    }
}

#[cfg(test)]
thread_local! {
    /// Tests only: how long each copy into the history takes more on this
    /// thread (`keep`, `prepare`), for slow storage. Never in a build of the programs.
    pub(crate) static SLOW_COPY: std::cell::Cell<std::time::Duration> = const { std::cell::Cell::new(std::time::Duration::ZERO) };
}

fn slow_copy() {
    #[cfg(test)]
    std::thread::sleep(SLOW_COPY.with(std::cell::Cell::get));
}

/// A file another device's change takes out, moved here as it is: nothing
/// copied (it was going anyway), and no reference to its content, which
/// leaves the sharing folder as soon as every device that counts read that it
/// was taken out (`share::removed_due`): this version stays its 30 days.
/// Copied (`keep`) when it cannot be moved (another disk: on a phone, the
/// notes are in its shared storage, the history in Sioul's own), and when
/// the file has other names (hard links): moved, the history would hold the
/// one file they share, and make it yours alone under each. The bytes it
/// copied (0 when moved), for the exchange's bound on copies
/// (`share::TAKE_OUT_COPIES`).
pub fn keep_moved(root: &Path, part: &str, file: &str, path: &Path, now_ms: i64) -> Result<u64, String> {
    let dir = below(root, part, file).ok_or_else(|| format!("{file}: not a file's name"))?;
    crate::share::private_dirs(&dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    let size = std::fs::metadata(path).map_or(0, |m| m.len());
    #[cfg(unix)]
    let linked = std::fs::metadata(path).is_ok_and(|m| std::os::unix::fs::MetadataExt::nlink(&m) > 1);
    #[cfg(not(unix))]
    let linked = false;
    let target = dir.join(new_stamp(&dir, now_ms));
    if linked || std::fs::rename(path, &target).is_err() {
        return keep(root, part, file, path, now_ms).map(|()| size);
    }
    // Yours alone, as a copy is.
    crate::share::make_private(&target);
    prune_file(&dir, now_ms, false);
    Ok(0)
}

/// The part of the history that holds your calendars' and contacts' versions
/// set aside: an item changed both here and on its server, the server's
/// version kept, yours kept here (`dav`), to be put back from the window, on
/// a phone too, whose state folder no file manager reaches. Its files are
/// named by their place in the data folder (`data/calendars/<account>/…`).
pub const ACCOUNTS: &str = "accounts";

/// Bytes kept as a version of `file` (a calendar's item set aside, `ACCOUNTS`),
/// as `keep` keeps a file: under a hidden name, then renamed. Returns where.
pub fn keep_bytes(root: &Path, part: &str, file: &str, bytes: &[u8], now_ms: i64) -> Result<PathBuf, String> {
    let dir = below(root, part, file).ok_or_else(|| format!("{file}: not a file's name"))?;
    crate::share::private_dirs(&dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    let target = dir.join(new_stamp(&dir, now_ms));
    crate::share::write_atomically(&target, bytes)?;
    prune_file(&dir, now_ms, false);
    Ok(target)
}

/// A file's content kept by reference: a record named it, so it is sealed in
/// the sharing folder already; nothing copied.
pub fn keep_sealed(root: &Path, part: &str, file: &str, hash: &str, size: u64, now_ms: i64) -> Result<(), String> {
    let dir = below(root, part, file).ok_or_else(|| format!("{file}: not a file's name"))?;
    crate::share::private_dirs(&dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    let target = dir.join(format!("{}{SEALED}", new_stamp(&dir, now_ms)));
    crate::share::write_atomically(&target, format!("{hash} {size}\n").as_bytes())?;
    prune_file(&dir, now_ms, false);
    Ok(())
}

/// A file's versions past what is kept taken out: beyond the newest 20, those
/// older than 30 days (all of those when the file is no longer here), and
/// those kept by reference past `SEALED_DAYS`.
fn prune_file(dir: &Path, now_ms: i64, gone: bool) {
    for (i, version) in versions_in(dir).into_iter().enumerate() {
        let old = now_ms - version.at > DAYS * DAY;
        if (old && (i >= VERSIONS || gone)) || (version.sealed.is_some() && now_ms - version.at > SEALED_DAYS * DAY) {
            let _ = std::fs::remove_file(dir.join(name_of(&version)));
        }
    }
}

fn name_of(version: &Version) -> String {
    if version.sealed.is_some() { format!("{}{SEALED}", version.stamp) } else { version.stamp.clone() }
}

/// The versions in a file's folder, the newest first.
fn versions_in(dir: &Path) -> Vec<Version> {
    let mut out: Vec<Version> = std::fs::read_dir(dir)
        .into_iter()
        .flatten()
        .filter_map(Result::ok)
        .filter(|e| e.file_type().is_ok_and(|t| t.is_file()))
        .filter_map(|e| {
            let name = e.file_name().to_string_lossy().to_string();
            match name.strip_suffix(SEALED) {
                Some(stamp) => {
                    // "<hash> <size>": a few bytes, read as such.
                    let text = crate::share::read_small(&e.path())?;
                    let (hash, size) = text.trim().split_once(' ')?;
                    Some(Version { at: kept_at(stamp)?, size: size.parse().ok()?, sealed: Some(hash.to_string()), stamp: stamp.to_string() })
                }
                None => Some(Version { at: kept_at(&name)?, size: e.metadata().map_or(0, |m| m.len()), sealed: None, stamp: name }),
            }
        })
        .collect();
    out.sort_by(|a, b| b.at.cmp(&a.at).then_with(|| b.stamp.cmp(&a.stamp)));
    out
}

/// A file's versions, the newest first.
pub fn versions(root: &Path, part: &str, file: &str) -> Vec<Version> {
    below(root, part, file).map(|dir| versions_in(&dir)).unwrap_or_default()
}

/// The files of a part with versions kept, the one changed last first: each
/// with its versions, the newest first.
pub fn files(root: &Path, part: &str) -> Vec<(String, Vec<Version>)> {
    let mut out = Vec::new();
    if let Some(dir) = part_dir(root, part) {
        walk(&dir, &dir, &mut out);
    }
    out.sort_by(|a, b| b.1.first().map(|v| v.at).cmp(&a.1.first().map(|v| v.at)).then_with(|| a.0.cmp(&b.0)));
    out
}

/// A folder holding versions is a file's; the others hold files' folders.
fn walk(top: &Path, dir: &Path, out: &mut Vec<(String, Vec<Version>)>) {
    let versions = versions_in(dir);
    if !versions.is_empty()
        && let Ok(relative) = dir.strip_prefix(top)
    {
        out.push((relative.components().map(|c| c.as_os_str().to_string_lossy()).collect::<Vec<_>>().join("/"), versions));
    }
    for entry in std::fs::read_dir(dir).into_iter().flatten().filter_map(Result::ok) {
        if entry.file_type().is_ok_and(|t| t.is_dir()) {
            walk(top, &entry.path(), out);
        }
    }
}

/// The versions kept here of the copies beside drafts of the messages they
/// answer or forward (`data/drafts/<id>.eml`), as files: until 19 October
/// 2026 one could hold an encrypted message's text in clear
/// (`sioul_core::draftcopies::seal_versions` encrypts those again).
pub fn draft_copies(root: &Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    for (file, versions) in files(root, "drafts") {
        let Some(name) = file.strip_prefix("data/drafts/") else { continue };
        if name.contains('/') || !name.ends_with(".eml") {
            continue;
        }
        for kept in versions.iter().filter(|v| v.sealed.is_none()) {
            if let Some(Kept::Copy(path)) = version(root, "drafts", &file, &kept.stamp) {
                out.push(path);
            }
        }
    }
    out
}

/// Where a version kept is, if it is there: never outside the history.
pub fn version(root: &Path, part: &str, file: &str, stamp: &str) -> Option<Kept> {
    kept_at(stamp)?;
    let dir = below(root, part, file)?;
    if dir.join(stamp).is_file() {
        return Some(Kept::Copy(dir.join(stamp)));
    }
    versions_in(&dir).into_iter().find(|v| v.stamp == stamp).and_then(|v| v.sealed.map(|hash| Kept::Sealed { hash, size: v.size }))
}

/// The history held within bounds, now and then: each file's versions as
/// `keep` keeps them, those of a file no longer here (`here` says which are)
/// past 30 days, then the oldest copies past `cap` bytes in all. Your
/// versions of calendars' and contacts' items (`ACCOUNTS`) are kept as those
/// of a file still here: the last 20, and all those of the last 30 days.
pub fn prune(root: &Path, now_ms: i64, cap: u64, here: &dyn Fn(&str, &str) -> bool) {
    let mut copies: Vec<(i64, PathBuf, u64)> = Vec::new();
    for entry in std::fs::read_dir(root).into_iter().flatten().filter_map(Result::ok).filter(|e| e.file_type().is_ok_and(|t| t.is_dir())) {
        let part = entry.file_name().to_string_lossy().to_string();
        let mut listed = Vec::new();
        walk(&entry.path(), &entry.path(), &mut listed);
        for (file, _) in listed {
            let Some(dir) = below(root, &part, &file) else { continue };
            crate::share::clean_leftovers(&dir);
            // Your versions of your calendars' and contacts' items are kept as
            // a file still here's, the item on its server or not: no shared
            // store holds them (review of 10 October 2026, R4).
            prune_file(&dir, now_ms, part != ACCOUNTS && !here(&part, &file));
            copies.extend(versions_in(&dir).into_iter().filter(|v| v.sealed.is_none()).map(|v| (v.at, dir.join(&v.stamp), v.size)));
        }
    }
    let mut total: u64 = copies.iter().map(|(_, _, size)| size).sum();
    copies.sort_by_key(|(at, _, _)| *at);
    for (_, path, size) in copies {
        if total <= cap {
            break;
        }
        if std::fs::remove_file(&path).is_ok() {
            total -= size;
        }
    }
}

/// What the history may hold in all: a GB, or a twentieth of the disk if less.
pub fn cap(root: &Path) -> u64 {
    crate::disk::space(root).map_or(1 << 30, |(_, total)| (1u64 << 30).min(total / 20))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scratch(name: &str) -> PathBuf {
        let base = std::env::temp_dir().join(format!("sioul-history-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&base);
        std::fs::create_dir_all(&base).unwrap();
        base
    }

    #[test]
    fn twenty_versions_and_thirty_days() {
        let base = scratch("kept");
        let root = base.join("history");
        let file = base.join("lease.md");
        let start = 1_790_000_000_000;
        // One version a day for 40 days: all of the last 30 days stay, and never fewer than 20.
        for n in 0..40 {
            std::fs::write(&file, format!("version {n}")).unwrap();
            keep(&root, "notes", "files/notes/admin/lease.md", &file, start + n * DAY).unwrap();
        }
        let kept = versions(&root, "notes", "files/notes/admin/lease.md");
        assert_eq!(kept.len(), 31, "{kept:?}");
        assert_eq!(std::fs::read_to_string(root.join("notes/files/notes/admin/lease.md").join(&kept[0].stamp)).unwrap(), "version 39");
        // Rarely changed: twenty kept, however old.
        for n in 0..25 {
            keep(&root, "settings", "config/config.toml", &file, start + n * 100 * DAY).unwrap();
        }
        assert_eq!(versions(&root, "settings", "config/config.toml").len(), VERSIONS);
        let listed = files(&root, "notes");
        assert_eq!(listed.len(), 1);
        assert_eq!(listed[0].0, "files/notes/admin/lease.md");
        assert!(matches!(version(&root, "notes", "files/notes/admin/lease.md", &kept[3].stamp), Some(Kept::Copy(_))));
        // Never outside the history.
        assert!(version(&root, "notes", "files/notes/../../x", &kept[3].stamp).is_none());
        assert!(version(&root, "../notes", "files/notes/admin/lease.md", &kept[3].stamp).is_none());
        assert!(version(&root, "notes", "files/notes/admin/lease.md", "../../config.toml").is_none());
        assert_eq!(kept_at(&stamp(start + 123)), Some(start + 123));
        let _ = std::fs::remove_dir_all(&base);
    }

    #[test]
    fn by_reference_capped_and_gone_files_expire() {
        let base = scratch("bounds");
        let root = base.join("history");
        let file = base.join("scan.pdf");
        std::fs::write(&file, vec![7u8; 10_000]).unwrap();
        let start = 1_790_000_000_000;
        // A content a record named: kept by reference, a few bytes here.
        keep_sealed(&root, "papers", "files/papers/scan.pdf", &"a".repeat(64), 10_000, start).unwrap();
        let kept = versions(&root, "papers", "files/papers/scan.pdf");
        assert_eq!(kept.len(), 1);
        assert_eq!((kept[0].sealed.as_deref(), kept[0].size), (Some("a".repeat(64).as_str()), 10_000));
        assert_eq!(version(&root, "papers", "files/papers/scan.pdf", &kept[0].stamp), Some(Kept::Sealed { hash: "a".repeat(64), size: 10_000 }));
        // References go after `SEALED_DAYS` (12 days; 60 until 15 October
        // 2026, when this test pruned at 40 and 61 days), versions of a file
        // no longer here after 30, whatever their number.
        keep(&root, "notes", "files/notes/gone.md", &file, start).unwrap();
        keep(&root, "notes", "files/notes/here.md", &file, start).unwrap();
        prune(&root, start + SEALED_DAYS * DAY, u64::MAX, &|_, _| true);
        assert_eq!(versions(&root, "papers", "files/papers/scan.pdf").len(), 1);
        prune(&root, start + SEALED_DAYS * DAY + 1, u64::MAX, &|_, _| true);
        assert!(versions(&root, "papers", "files/papers/scan.pdf").is_empty());
        prune(&root, start + 40 * DAY, u64::MAX, &|_, file| file != "files/notes/gone.md");
        assert!(versions(&root, "notes", "files/notes/gone.md").is_empty());
        assert_eq!(versions(&root, "notes", "files/notes/here.md").len(), 1);
        // Past the cap, the oldest copies go first.
        for n in 0..5 {
            keep(&root, "notes", "files/notes/big.md", &file, start + 70 * DAY + n).unwrap();
        }
        prune(&root, start + 70 * DAY + 10, 25_000, &|_, _| true);
        let left: u64 = files(&root, "notes").iter().flat_map(|(_, v)| v.iter().map(|v| v.size)).sum();
        assert!(left <= 25_000, "{left}");
        assert_eq!(versions(&root, "notes", "files/notes/big.md").first().map(|v| v.at), Some(start + 70 * DAY + 4), "the newest stays");
        let _ = std::fs::remove_dir_all(&base);
    }

    /// The window's help and the guides say what holds: copies, the last 20
    /// and those of 30 days; a note or a paper kept as a pointer to the
    /// sharing folder, 12 days whatever its rank. Until the review of 15
    /// October 2026 (M1) they promised the 20 and the 30 days to every version.
    #[test]
    fn the_help_and_the_guides_say_how_long_versions_stay() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let read = |relative: &str| std::fs::read_to_string(root.join(relative)).unwrap();
        let help = |relative: &str| read(relative).lines().find(|l| l.starts_with("share-history-help =")).unwrap().to_string();
        let en = help("crates/sioul-core/locales/en/sioul.ftl");
        for said in [format!("last {VERSIONS} versions"), format!("last {DAYS} days"), format!("for {SEALED_DAYS} days")] {
            assert!(en.contains(&said), "{said}: {en}");
        }
        let fr = help("crates/sioul-core/locales/fr/sioul.ftl");
        for said in [format!("{VERSIONS} dernières versions"), format!("{DAYS} derniers jours"), format!("pendant {SEALED_DAYS} jours")] {
            assert!(fr.contains(&said), "{said}: {fr}");
        }
        // Every line of the guides giving the copies' 20 gives the pointers' 12 days too.
        for (guide, copies, pointers) in [
            ("website/docs/guide/notes.md", format!("last {VERSIONS}"), format!("{SEALED_DAYS} days")),
            ("website/fr/docs/guide/notes.md", format!("{VERSIONS} dernières"), format!("{SEALED_DAYS} jours")),
            ("website/docs/guide/sharing.md", format!("last {VERSIONS}"), format!("for {SEALED_DAYS} days")),
            ("website/fr/docs/guide/sharing.md", format!("{VERSIONS} dernières"), format!("pendant {SEALED_DAYS} jours")),
        ] {
            let text = read(guide);
            let lines: Vec<&str> = text.lines().filter(|l| l.contains(&copies)).collect();
            assert!(!lines.is_empty(), "{guide}");
            for line in lines {
                assert!(line.contains(&pointers), "{guide}: {line}");
            }
        }
    }

    /// What a bulk's take-outs cost when they are copied (review of 17
    /// October 2026, L9): 7,000 notes of 500 bytes kept as copies (each
    /// with another name, so never moved), then moved.
    /// `cargo test --release -p sioul-sync history::tests::what_a_bulk_s_copies_cost -- --ignored --nocapture`
    #[test]
    #[ignore]
    #[cfg(unix)]
    fn what_a_bulk_s_copies_cost() {
        let base = scratch("bulk-cost");
        let (notes, root, links) = (base.join("notes"), base.join("history"), base.join("links"));
        for dir in [&notes, &links] {
            std::fs::create_dir_all(dir).unwrap();
        }
        let note = "a note of some five hundred bytes, as an agent's run writes it. ".repeat(8);
        for n in 0..7_000 {
            std::fs::write(notes.join(format!("{n}.md")), &note[..500]).unwrap();
            std::fs::hard_link(notes.join(format!("{n}.md")), links.join(format!("{n}.md"))).unwrap();
        }
        let started = std::time::Instant::now();
        let mut copied = 0;
        for n in 0..7_000 {
            let path = notes.join(format!("{n}.md"));
            copied += keep_moved(&root, "notes", &format!("files/notes/agents/{n}.md"), &path, 1_800_000_000_000).unwrap();
            std::fs::remove_file(&path).unwrap();
        }
        let copying = started.elapsed();
        for n in 0..7_000 {
            std::fs::rename(links.join(format!("{n}.md")), notes.join(format!("{n}.md"))).unwrap();
        }
        let started = std::time::Instant::now();
        for n in 0..7_000 {
            keep_moved(&root, "notes", &format!("files/notes/agents/{n}.md"), &notes.join(format!("{n}.md")), 1_800_000_100_000).unwrap();
        }
        let moving = started.elapsed();
        println!("7000 take-outs of 500 bytes: copied ({copied} bytes) in {copying:?}; moved in {moving:?}");
        assert_eq!(copied, 7_000 * 500);
        let _ = std::fs::remove_dir_all(&base);
    }
}
