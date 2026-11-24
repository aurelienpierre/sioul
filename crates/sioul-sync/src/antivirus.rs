// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

//! Checking a file before it opens: one question, four answers, one way per
//! system (docs/client.md, "Antivirus").
//!
//! - **Windows**: the antivirus Windows runs (Microsoft Defender, or the one
//!   installed), asked by every door the system has, newest first
//!   (`WINDOWS_DOORS`): the Antimalware Scan Interface (AMSI: Windows 10 and
//!   Windows Server 2016 onwards), then Windows' attachment service
//!   (`IAttachmentExecute::Save`: Windows XP SP2 onwards). Neither is linked
//!   when Sioul starts: each is looked up when asked, so that a Windows
//!   without it answers "closed" (`Answer::Closed`) and never keeps Sioul
//!   from starting. AMSI gives a verdict; the attachment service gives none
//!   (Microsoft documents that it *may* run the antivirus, and that by
//!   default Windows does not call it for an attachment): a file it removes,
//!   changes or refuses is refused, and one it lets through unchanged is
//!   "not checked", never clean. Whether an antivirus is there, and its
//!   name, is what Windows Security Center says (`presence`): its list of
//!   products from Windows 8, the health of its antivirus category on
//!   Windows Vista and 7; a Windows server has neither, and then Sioul does
//!   not know and asks the doors anyway. What decides (the order of the
//!   doors, each one's answer read from plain numbers, the Security Center's
//!   word) compiles and is tested on every system; only the calls to Windows
//!   live in `antivirus_windows.rs`. By Windows:
//!   - Windows 10 and 11: AMSI gives the verdict when an antivirus is
//!     enrolled with it; the Security Center names the products on.
//!   - Windows 8 and 8.1: no AMSI; the attachment service, so a file it lets
//!     through is "not checked" and asked about; the Security Center names
//!     the products on.
//!   - Windows Vista and 7: the same door; the Security Center gives only its
//!     health (good: an antivirus is there, unnamed).
//!   - Windows Server 2016 and later: AMSI; no Security Center ("unknown").
//!
//!   Sioul's release is built for Windows 10 and 11 alone (Qt 6.11, Rust's
//!   `x86_64-pc-windows-msvc`): the older ones are answered here so that
//!   nothing in this module keeps a program from starting on them.
//! - **Linux and macOS**: ClamAV, when the system has it (its packages, or
//!   Homebrew): its daemon when it runs (fast) and says when a file passes its
//!   limits, else its scanner with the system's signatures, else with Sioul's
//!   own signatures, which Sioul keeps current with the system's `freshclam`.
//!   Nothing of ClamAV ships with Sioul.
//! - **A file larger than the antivirus scans** (ClamAV's limits on a file's
//!   size, on what an archive holds, on its depth; AMSI's 4 GB) is "not
//!   scanned: too big", never clean: ClamAV left alone finds such a file clean
//!   without reading it, so its scanner is asked to say so
//!   (`--alert-exceeds-max`), and its daemon is asked only when its own
//!   configuration says so (`AlertExceedsMax yes`).
//! - **Without an antivirus, or when none answers**, nothing is refused: the
//!   window says the file was not checked and may be dangerous, asks once
//!   before opening or saving it, and says how to install or turn one on
//!   (`install_hint`). What the window does for each answer is one table, in
//!   the window's crate (`attachments::decide`).
//! - **A phone** has no antivirus another app can call, and no ClamAV:
//!   `scan` answers `Unavailable` there without trying, and Sioul's window
//!   does not ask it (crates/sioul-app/src/attachments.rs).

use std::path::Path;
#[cfg(not(windows))]
use std::path::PathBuf;

/// The calls to Windows, thin: what they answer is read here.
#[cfg(windows)]
#[path = "antivirus_windows.rs"]
mod on_windows;

/// What the antivirus said.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Verdict {
    /// Read by an antivirus, which found nothing.
    Clean,
    /// Refused: a threat, by the name the antivirus gives it (ClamAV); on
    /// Windows, what refused it and how (`AMSI 32768`, the attachment
    /// service's code). Never opened.
    Infected(String),
    /// Not scanned: larger than the antivirus scans (a file's size, what an
    /// archive holds, how deep it nests). Nothing is opened without asking.
    TooBig,
    /// Not checked: no antivirus was found, or none answered, or the only
    /// door that answered gives no verdict; why, in a few words. Never clean:
    /// nothing is opened without asking.
    Unavailable(String),
}

/// Whether an antivirus is there, known before a scan, as the system says.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Presence {
    /// One is there and on: by its name, when the system gives it
    /// ("Microsoft Defender", "ClamAV").
    Here(Option<String>),
    /// The system says there is none, or none is on.
    Absent,
    /// The system cannot say (a Windows server; the Security Center not
    /// answering): Sioul does not know, and asks the antivirus anyway.
    Unknown,
}

/// Checks a file.
pub fn scan(file: &Path) -> Verdict {
    // A computer without an antivirus, in tests only (never in Sioul's builds).
    #[cfg(feature = "insecure-test-tls")]
    if std::env::var_os("SIOUL_TEST_NO_ANTIVIRUS").is_some() {
        return Verdict::Unavailable("no antivirus (test)".into());
    }
    #[cfg(windows)]
    {
        on_windows::scan(file)
    }
    // A phone: no antivirus another app can call, and no ClamAV to start.
    #[cfg(target_os = "android")]
    {
        let _ = file;
        Verdict::Unavailable("no antivirus on a phone".into())
    }
    #[cfg(not(any(windows, target_os = "android")))]
    {
        clamav(file)
    }
}

/// What a rule that keeps a file as a paper is told of it
/// (`sioul_core::wiring::engine::Scanned`, docs/rules.md, "Attachments kept
/// as papers"), as "Keep in papers" decides it by hand: on a phone no
/// antivirus can be asked, and the file is kept unchecked, as by hand there;
/// on a computer, the antivirus's verdict; one that could not read it (none
/// there, none answering, too large) keeps it marked unchecked, read again
/// by a device that can ask one.
/// `phone`: Sioul runs the phone's way (the window's `attachments::phone`).
pub fn for_rules(file: &Path, phone: bool) -> sioul_core::wiring::engine::Scanned {
    use sioul_core::wiring::engine::Scanned;
    if phone {
        return Scanned::NoAntivirus;
    }
    match scan(file) {
        Verdict::Clean => Scanned::Clean,
        Verdict::Infected(what) => Scanned::Infected(what),
        Verdict::TooBig => Scanned::Unanswered("too big".into()),
        Verdict::Unavailable(detail) => Scanned::Unanswered(detail),
    }
}

/// Whether there is an antivirus here, and its name, known without asking
/// it. On Windows, what Windows Security Center says (kept a minute); on a
/// phone, none; on Linux and macOS, ClamAV when one of the programs `scan`
/// runs is installed. `Absent` promises that `scan` gives no clean verdict;
/// `Here` promises no verdict at all.
pub fn presence() -> Presence {
    #[cfg(feature = "insecure-test-tls")]
    if std::env::var_os("SIOUL_TEST_NO_ANTIVIRUS").is_some() {
        return Presence::Absent;
    }
    #[cfg(windows)]
    {
        on_windows::presence()
    }
    #[cfg(target_os = "android")]
    {
        Presence::Absent
    }
    #[cfg(not(any(windows, target_os = "android")))]
    {
        if runs(None, daemon_here()).iter().any(|(command, _)| installed(command)) { Presence::Here(Some("ClamAV".into())) } else { Presence::Absent }
    }
}

/// What `presence` says, at once, for the window's thread: on Windows what
/// the Security Center said last ("unknown" until it first answers, asked
/// again off the thread when a minute old); elsewhere `presence` itself,
/// which asks nothing slow.
pub fn presence_now() -> Presence {
    #[cfg(windows)]
    {
        #[cfg(feature = "insecure-test-tls")]
        if std::env::var_os("SIOUL_TEST_NO_ANTIVIRUS").is_some() {
            return Presence::Absent;
        }
        on_windows::presence_now()
    }
    #[cfg(not(windows))]
    {
        presence()
    }
}

/// Whether there is an antivirus to ask here (`presence` is not `Absent`):
/// "Checking…" is said only then. False: `scan` gives no clean verdict.
pub fn present() -> bool {
    presence() != Presence::Absent
}

/// The doors Windows has to its antivirus, newest first: each is asked in
/// turn until one gives a verdict (`through`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Door {
    /// The Antimalware Scan Interface (`amsi.dll`: Windows 10 and Windows
    /// Server 2016 onwards): the file's bytes, with its name, handed to the
    /// antivirus registered with it; its verdict comes back.
    Amsi,
    /// Attachment Execution Services (`IAttachmentExecute::Save`, in
    /// `shdocvw.dll`: Windows XP SP2 and Windows Server 2003 onwards), which
    /// mail programs and browsers use: it may hand the file to the antivirus
    /// registered for it and records where the file came from; it gives no
    /// verdict.
    AttachmentService,
}

/// The order Windows' doors are asked in: AMSI, then the attachment service.
pub const WINDOWS_DOORS: [Door; 2] = [Door::Amsi, Door::AttachmentService];

/// What one door said of a file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Answer {
    /// Not there on this Windows, or it failed before reading the file: why.
    /// The next door is asked.
    Closed(String),
    /// Read by an antivirus, which found nothing.
    NothingFound,
    /// Handed over, and no verdict given back: why. The next door is asked;
    /// alone, the file was not checked.
    NoVerdict(String),
    /// Refused, removed or changed: by what, and how.
    Refused(String),
    /// Larger than the door reads. The next door is asked.
    TooBig,
}

/// The verdict of the doors' answers, asked in order and only as far as
/// needed: the first refusal refuses; the first "nothing found" is clean,
/// unless Windows said before that no antivirus is on (`Presence::Absent`):
/// then nothing proves that an antivirus read it, and the next door is
/// asked. Every door closed, too big or without a verdict: not checked
/// (too big when one door said so).
pub fn through(presence: &Presence, answers: impl IntoIterator<Item = Answer>) -> Verdict {
    let mut why: Vec<String> = Vec::new();
    let mut too_big = false;
    for answer in answers {
        match answer {
            Answer::Refused(how) => return Verdict::Infected(how),
            Answer::NothingFound if *presence != Presence::Absent => return Verdict::Clean,
            Answer::NothingFound => why.push("nothing found, but Windows says no antivirus is on".into()),
            Answer::TooBig => too_big = true,
            Answer::Closed(reason) | Answer::NoVerdict(reason) => why.push(reason),
        }
    }
    if too_big { Verdict::TooBig } else { Verdict::Unavailable(why.join("; ")) }
}

/// A file Sioul has just written that cannot be read back, before any door
/// is asked: gone, it was taken away meanwhile (an antivirus watching the
/// disk does that to a threat), and is refused, never offered unchecked;
/// otherwise, not checked, why said.
pub fn unread(error: &std::io::Error) -> Verdict {
    if error.kind() == std::io::ErrorKind::NotFound { Verdict::Infected("removed before it could be read".into()) } else { Verdict::Unavailable(error.to_string()) }
}

/// The smallest AMSI result that is malware (`AMSI_RESULT_DETECTED`).
pub const AMSI_DETECTED: i32 = 32768;
/// AMSI's results that say an administrator's policy blocked the content
/// (`AMSI_RESULT_BLOCKED_BY_ADMIN_START` to `…_END`).
pub const AMSI_BLOCKED_BY_ADMIN: std::ops::RangeInclusive<i32> = 0x4000..=0x4FFF;

/// AMSI's answer, from what `AmsiScanBuffer` returned, as plain numbers: its
/// HRESULT (negative: it failed, and the next door is asked) and its result
/// (32768 and above: malware; 0x4000 to 0x4FFF: blocked by an
/// administrator's policy; below, a risk level that is not malware, 0 and 1
/// meaning nothing found).
pub fn amsi_answer(hresult: i32, result: i32) -> Answer {
    if hresult < 0 {
        return Answer::Closed(format!("AMSI 0x{:08X}", hresult.cast_unsigned()));
    }
    if result >= AMSI_DETECTED || AMSI_BLOCKED_BY_ADMIN.contains(&result) { Answer::Refused(format!("AMSI {result}")) } else { Answer::NothingFound }
}

/// What Windows' attachment service said, as plain numbers: `Save`'s HRESULT
/// (negative: refused, as an antivirus's failure blocks an attachment) and
/// whether the file is still there with the same bytes (an antivirus may
/// delete or clean it). Saved and unchanged, it gives no verdict: Microsoft
/// documents that `Save` *may* run the antivirus, not that it did.
pub fn service_answer(saved: i32, unchanged: bool) -> Answer {
    if saved < 0 {
        Answer::Refused(format!("attachment service 0x{:08X}", saved.cast_unsigned()))
    } else if !unchanged {
        Answer::Refused("attachment service: removed or changed".into())
    } else {
        Answer::NoVerdict("Windows' attachment service gives no verdict".into())
    }
}

/// `WSC_SECURITY_PRODUCT_STATE_ON`: the product is on and protecting.
pub const WSC_PRODUCT_ON: i32 = 0;
/// `WSC_SECURITY_PROVIDER_HEALTH_GOOD`: the antivirus category is good.
pub const WSC_HEALTH_GOOD: i32 = 0;

/// What Windows Security Center says before any scan, as plain values: the
/// antivirus products it lists, each with its state (`IWSCProductList`,
/// Windows 8 onwards; none when the list cannot be read), else the health
/// of its antivirus category (`WscGetSecurityProviderHealth`, Windows Vista
/// onwards; none when it cannot be read or the service does not run). The
/// products on, by name; a list with none on, absent; good health, one is
/// there unnamed; anything else, Windows does not say.
pub fn security_center(products: Option<&[(String, i32)]>, health: Option<i32>) -> Presence {
    if let Some(products) = products {
        let on: Vec<&(String, i32)> = products.iter().filter(|(_, state)| *state == WSC_PRODUCT_ON).collect();
        if on.is_empty() {
            return Presence::Absent;
        }
        // A name from the system, shown as plain text: no backticks (the dialog shows those as commands), not too long.
        let names: Vec<String> = on.iter().map(|(name, _)| name.replace('`', "'").trim().chars().take(60).collect::<String>()).filter(|name| !name.is_empty()).collect();
        return Presence::Here((!names.is_empty()).then(|| names.join(", ")));
    }
    match health {
        Some(WSC_HEALTH_GOOD) => Presence::Here(None),
        _ => Presence::Unknown,
    }
}

/// One of ClamAV's programs, as the system has it: on its path, else where
/// Homebrew puts it (a macOS application does not get the shell's path).
#[cfg(not(windows))]
fn program(name: &str) -> PathBuf {
    ["/opt/homebrew/bin", "/usr/local/bin"].iter().map(|dir| Path::new(dir).join(name)).find(|p| p.is_file()).filter(|_| cfg!(target_os = "macos")).unwrap_or_else(|| PathBuf::from(name))
}

/// How to install an antivirus here, in one line: the command for this
/// system's packages, in backticks (the window offers it to copy), or what to
/// turn on, in your language.
pub fn install_hint() -> String {
    #[cfg(windows)]
    {
        crate::translator().text("antivirus-hint-windows", None)
    }
    #[cfg(target_os = "macos")]
    {
        "`brew install clamav`".to_string()
    }
    // A phone has none to install that Sioul could call: a computer with one, then.
    #[cfg(target_os = "android")]
    {
        crate::translator().text("antivirus-hint-phone", None)
    }
    #[cfg(all(unix, not(any(target_os = "macos", target_os = "android"))))]
    {
        let release = std::fs::read_to_string("/etc/os-release").unwrap_or_default().to_ascii_lowercase();
        let like = |name: &str| release.lines().any(|l| (l.starts_with("id=") || l.starts_with("id_like=")) && l.contains(name));
        if like("fedora") || like("rhel") {
            "`sudo dnf install clamav clamav-update && sudo freshclam`".to_string()
        } else if like("debian") || like("ubuntu") {
            "`sudo apt install clamav && sudo freshclam`".to_string()
        } else if like("arch") {
            "`sudo pacman -S clamav && sudo freshclam`".to_string()
        } else if like("suse") {
            "`sudo zypper install clamav && sudo freshclam`".to_string()
        } else {
            crate::translator().text("antivirus-hint-packages", None)
        }
    }
}

/// Whether one of ClamAV's programs is installed: on the path, or where Homebrew puts it.
#[cfg(not(windows))]
fn installed(name: &str) -> bool {
    let found = program(name);
    (found.is_absolute() && found.is_file()) || std::env::var_os("PATH").is_some_and(|path| std::env::split_paths(&path).any(|dir| dir.join(name).is_file()))
}

/// Where Sioul keeps its own ClamAV signatures (not on Windows, which has its own antivirus).
#[cfg(not(windows))]
fn own_signatures() -> PathBuf {
    sioul_core::config::data_dir().join("clamav")
}

/// Whether the system keeps ClamAV signatures Sioul's scanner can read.
#[cfg(not(windows))]
fn system_signatures() -> bool {
    ["/var/lib/clamav", "/usr/local/share/clamav", "/opt/homebrew/var/lib/clamav", "/usr/local/var/lib/clamav"]
        .iter()
        .any(|dir| std::fs::read_dir(dir).into_iter().flatten().filter_map(Result::ok).any(|e| e.path().extension().is_some_and(|x| x == "cvd" || x == "cld")))
}

#[cfg(not(any(windows, target_os = "android")))]
fn clamav(file: &Path) -> Verdict {
    let own = own_signatures();
    let mut problem = String::new();
    for (command, options) in runs(own.is_dir().then_some(own.as_path()), daemon_here()) {
        match std::process::Command::new(program(command)).args(&options).arg(file).output() {
            Ok(out) => match clamav_verdict(out.status.code(), &String::from_utf8_lossy(&out.stdout)) {
                Some(verdict) => return verdict,
                None => problem = String::from_utf8_lossy(&out.stderr).lines().find(|l| !l.contains("LibClamAV Warning")).unwrap_or("").to_string(),
            },
            Err(e) if problem.is_empty() => problem = e.to_string(),
            Err(_) => {}
        }
    }
    Verdict::Unavailable(problem)
}

/// The scanners to ask, in order, with their options: ClamAV's daemon when
/// it says when a file passes its limits (`daemon_alerts`), else none of it;
/// its scanner with the system's signatures, then with Sioul's own (`own`),
/// each asked to say so (`--alert-exceeds-max`).
#[cfg(not(any(windows, target_os = "android")))]
fn runs(own: Option<&Path>, daemon_alerts: bool) -> Vec<(&'static str, Vec<String>)> {
    let scanner = || vec!["--no-summary".to_string(), "--alert-exceeds-max=yes".to_string()];
    let mut runs: Vec<(&'static str, Vec<String>)> = Vec::new();
    if daemon_alerts {
        runs.push(("clamdscan", vec!["--no-summary".into(), "--fdpass".into()]));
    }
    runs.push(("clamscan", scanner()));
    if let Some(own) = own {
        runs.push(("clamscan", [scanner(), vec![format!("--database={}", own.display())]].concat()));
    }
    runs
}

/// What ClamAV's exit code and output say: 0, nothing found; 1, a threat
/// (`<file>: Eicar-Test-Signature FOUND`), or a limit passed, which it names
/// `Heuristics.Limits.Exceeded…` when asked to say so: not scanned, too big;
/// anything else, none: it could not scan.
#[cfg(not(any(windows, target_os = "android")))]
fn clamav_verdict(code: Option<i32>, stdout: &str) -> Option<Verdict> {
    match code {
        Some(0) => Some(Verdict::Clean),
        Some(1) => {
            let threat = stdout.lines().find_map(|l| l.trim_end().strip_suffix(" FOUND").and_then(|l| l.rsplit_once(": ")).map(|(_, t)| t.to_string())).unwrap_or_default();
            Some(if threat.starts_with("Heuristics.Limits.Exceeded") { Verdict::TooBig } else { Verdict::Infected(threat) })
        }
        _ => None,
    }
}

/// The configuration ClamAV's daemon and `clamdscan` read, where each
/// system's packages put it; none when there is none to read.
#[cfg(not(any(windows, target_os = "android")))]
fn daemon_config() -> Option<String> {
    ["/etc/clamd.d/scan.conf", "/etc/clamav/clamd.conf", "/etc/clamd.conf", "/usr/local/etc/clamd.conf", "/opt/homebrew/etc/clamav/clamd.conf", "/usr/local/etc/clamav/clamd.conf"].iter().find_map(|path| std::fs::read_to_string(path).ok())
}

/// Whether this system's ClamAV daemon says when a file passes its limits (`daemon_alerts`).
#[cfg(not(any(windows, target_os = "android")))]
fn daemon_here() -> bool {
    daemon_config().is_some_and(|text| daemon_alerts(&text))
}

/// Whether ClamAV's daemon says when a file passes its limits: its
/// configuration's `AlertExceedsMax yes`. Without it, the daemon finds such a
/// file clean without reading it, and there is no option to ask it otherwise
/// for one scan: the scanner is asked instead.
#[cfg(not(any(windows, target_os = "android")))]
fn daemon_alerts(config: &str) -> bool {
    config.lines().map(str::trim).filter(|l| !l.starts_with('#')).filter_map(|l| l.split_once(char::is_whitespace)).any(|(key, value)| key == "AlertExceedsMax" && matches!(value.trim().to_ascii_lowercase().as_str(), "yes" | "true" | "1"))
}

/// Keeps Sioul's own ClamAV signatures current, once a day, when the system
/// has ClamAV's programs but no signatures of its own. Returns whether it ran.
#[cfg(not(windows))]
pub fn refresh_signatures() -> Result<bool, String> {
    // Without ClamAV (Android has none), nothing to keep current.
    if system_signatures() || !installed("freshclam") {
        return Ok(false);
    }
    let own = own_signatures();
    let fresh = std::fs::metadata(own.join("daily.cld"))
        .or_else(|_| std::fs::metadata(own.join("daily.cvd")))
        .and_then(|m| m.modified())
        .ok()
        .and_then(|t| t.elapsed().ok())
        .is_some_and(|age| age.as_secs() < 24 * 3600);
    if fresh {
        return Ok(false);
    }
    std::fs::create_dir_all(&own).map_err(|e| e.to_string())?;
    // freshclam reads its settings from a file: the smallest one, written here.
    let settings = own.join("freshclam.conf");
    std::fs::write(&settings, format!("DatabaseDirectory {}\nDatabaseMirror database.clamav.net\nNotifyClamd no\n", own.display())).map_err(|e| e.to_string())?;
    let out = std::process::Command::new(program("freshclam")).arg(format!("--config-file={}", settings.display())).arg("--quiet").output().map_err(|e| e.to_string())?;
    if out.status.success() { Ok(true) } else { Err(String::from_utf8_lossy(&out.stderr).lines().next().unwrap_or("freshclam").to_string()) }
}

/// Windows keeps its antivirus current itself.
#[cfg(windows)]
pub fn refresh_signatures() -> Result<bool, String> {
    Ok(false)
}

#[cfg(all(test, not(any(windows, target_os = "android"))))]
mod tests {
    use super::*;

    /// A file larger than ClamAV scans is "too big", never clean: its scanner
    /// is always asked to say so, its daemon only when it is set to.
    #[test]
    fn too_big_is_not_clean() {
        // As ClamAV 1.4 writes it, for a file over --max-filesize.
        assert_eq!(clamav_verdict(Some(1), "/tmp/plain.txt: Heuristics.Limits.Exceeded.MaxFileSize FOUND\n"), Some(Verdict::TooBig));
        assert_eq!(clamav_verdict(Some(1), "/tmp/a.zip: Heuristics.Limits.Exceeded.MaxScanSize FOUND\n"), Some(Verdict::TooBig));
        assert_eq!(clamav_verdict(Some(1), "/tmp/eicar.com: Eicar-Test-Signature FOUND\n"), Some(Verdict::Infected("Eicar-Test-Signature".into())));
        assert_eq!(clamav_verdict(Some(0), "/tmp/plain.txt: OK\n"), Some(Verdict::Clean));
        assert_eq!(clamav_verdict(Some(2), ""), None);
        // Every scanner run asks for the alert; the daemon is asked only when its configuration alerts.
        let own = Path::new("/data/sioul/clamav");
        for daemon in [false, true] {
            let all = runs(Some(own), daemon);
            assert!(all.iter().filter(|(c, _)| *c == "clamscan").all(|(_, o)| o.iter().any(|o| o == "--alert-exceeds-max=yes")), "{all:?}");
            assert_eq!(all.iter().any(|(c, _)| *c == "clamdscan"), daemon);
        }
        assert_eq!(runs(None, false).len(), 1);
        // Fedora's file as it comes: the line commented out.
        assert!(!daemon_alerts("# When AlertExceedsMax is set...\n#AlertExceedsMax yes\nMaxFileSize 400M\n"));
        assert!(daemon_alerts("LocalSocket /run/clamd.scan/clamd.sock\nAlertExceedsMax yes\n"));
        assert!(!daemon_alerts("AlertExceedsMax no\n"));
    }
}

/// Windows' doors, read on every system: what each answer means, in order.
#[cfg(test)]
mod windows_tests {
    use super::*;

    /// AMSI first, the attachment service after: newest first.
    #[test]
    fn the_doors_go_newest_first() {
        assert_eq!(WINDOWS_DOORS, [Door::Amsi, Door::AttachmentService]);
    }

    /// AMSI's numbers: malware and an administrator's block refuse; below
    /// 32768 outside that block, nothing found; a failed call is a closed door.
    #[test]
    fn amsi_numbers_read() {
        assert_eq!(amsi_answer(0, 0), Answer::NothingFound, "AMSI_RESULT_CLEAN");
        assert_eq!(amsi_answer(0, 1), Answer::NothingFound, "AMSI_RESULT_NOT_DETECTED");
        assert_eq!(amsi_answer(1, 1), Answer::NothingFound, "S_FALSE is a success");
        assert_eq!(amsi_answer(0, 32767), Answer::NothingFound, "a risk level, not malware");
        assert_eq!(amsi_answer(0, 32768), Answer::Refused("AMSI 32768".into()), "AMSI_RESULT_DETECTED");
        assert_eq!(amsi_answer(0, 40000), Answer::Refused("AMSI 40000".into()));
        assert_eq!(amsi_answer(0, 0x4000), Answer::Refused("AMSI 16384".into()), "blocked by an administrator");
        assert_eq!(amsi_answer(0, 0x4FFF), Answer::Refused("AMSI 20479".into()));
        assert_eq!(amsi_answer(0, 0x3FFF), Answer::NothingFound);
        assert_eq!(amsi_answer(0, 0x5000), Answer::NothingFound);
        // E_INVALIDARG, as Windows writes it: the door is closed, never clean.
        assert_eq!(amsi_answer(0x8007_0057_u32.cast_signed(), 0), Answer::Closed("AMSI 0x80070057".into()));
    }

    /// The attachment service: refused when `Save` fails or the file went or
    /// changed; otherwise no verdict, never "nothing found".
    #[test]
    fn the_attachment_service_never_says_clean() {
        assert_eq!(service_answer(0, true), Answer::NoVerdict("Windows' attachment service gives no verdict".into()));
        assert_eq!(service_answer(1, true), Answer::NoVerdict("Windows' attachment service gives no verdict".into()));
        assert_eq!(service_answer(0, false), Answer::Refused("attachment service: removed or changed".into()));
        assert_eq!(service_answer(0x8000_4005_u32.cast_signed(), true), Answer::Refused("attachment service 0x80004005".into()));
    }

    /// The doors' answers in order, with what Windows said before.
    #[test]
    fn through_the_doors() {
        let here = Presence::Here(Some("Microsoft Defender".into()));
        let closed = || Answer::Closed("no AMSI on this Windows".into());
        let no_verdict = || service_answer(0, true);
        // Windows 10 and 11: AMSI reads it.
        assert_eq!(through(&here, [Answer::NothingFound, no_verdict()]), Verdict::Clean);
        assert_eq!(through(&Presence::Unknown, [Answer::NothingFound]), Verdict::Clean, "a server: the antivirus registered with AMSI answered");
        assert_eq!(through(&here, [amsi_answer(0, 32768), no_verdict()]), Verdict::Infected("AMSI 32768".into()));
        // Windows 7 and 8: no AMSI; the attachment service lets it through without a verdict, or refuses it.
        let not_checked = through(&Presence::Unknown, [closed(), no_verdict()]);
        assert!(matches!(&not_checked, Verdict::Unavailable(why) if why.contains("no AMSI") && why.contains("no verdict")), "{not_checked:?}");
        assert_eq!(through(&Presence::Unknown, [closed(), service_answer(0, false)]), Verdict::Infected("attachment service: removed or changed".into()));
        // No door at all: not checked.
        assert!(matches!(through(&here, [closed(), Answer::Closed("no attachment service".into())]), Verdict::Unavailable(_)));
        // Windows said no antivirus is on: "nothing found" proves no reading, and the next door is asked.
        assert!(matches!(through(&Presence::Absent, [Answer::NothingFound, no_verdict()]), Verdict::Unavailable(_)));
        assert_eq!(through(&Presence::Absent, [Answer::NothingFound, service_answer(-1, true)]), Verdict::Infected("attachment service 0xFFFFFFFF".into()));
        // Larger than AMSI reads, and nobody else read it: too big; refused by the next door: refused.
        assert_eq!(through(&here, [Answer::TooBig, no_verdict()]), Verdict::TooBig);
        assert_eq!(through(&here, [Answer::TooBig, service_answer(0, false)]), Verdict::Infected("attachment service: removed or changed".into()));
    }

    /// A file gone before it was read was taken away: refused, never offered
    /// unchecked; one that cannot be read otherwise is not checked.
    #[test]
    fn a_file_gone_before_the_scan_is_refused() {
        assert_eq!(unread(&std::io::Error::from(std::io::ErrorKind::NotFound)), Verdict::Infected("removed before it could be read".into()));
        assert!(matches!(unread(&std::io::Error::from(std::io::ErrorKind::PermissionDenied)), Verdict::Unavailable(_)));
    }

    /// A door after a verdict is not asked.
    #[test]
    fn no_door_after_a_verdict() {
        let mut asked = Vec::new();
        let answers = WINDOWS_DOORS.iter().map(|door| {
            asked.push(*door);
            match door {
                Door::Amsi => Answer::NothingFound,
                Door::AttachmentService => service_answer(0, true),
            }
        });
        assert_eq!(through(&Presence::Here(None), answers), Verdict::Clean);
        assert_eq!(asked, [Door::Amsi]);
    }

    /// What Windows Security Center says: the products on by name, none on
    /// as absent, the health of Windows 7, and nothing on a server.
    #[test]
    fn the_security_center_says() {
        let defender = [("Microsoft Defender".to_string(), WSC_PRODUCT_ON)];
        assert_eq!(security_center(Some(&defender), None), Presence::Here(Some("Microsoft Defender".into())));
        // A product installed beside Defender, which is then off.
        let two = [("Microsoft Defender".to_string(), 1), ("Example Antivirus".to_string(), WSC_PRODUCT_ON)];
        assert_eq!(security_center(Some(&two), None), Presence::Here(Some("Example Antivirus".into())));
        // Off, snoozed, expired: none is on.
        let off = [("Microsoft Defender".to_string(), 1), ("Example Antivirus".to_string(), 2), ("Other".to_string(), 3)];
        assert_eq!(security_center(Some(&off), None), Presence::Absent);
        assert_eq!(security_center(Some(&[]), None), Presence::Absent);
        // A name made plain: no backticks, a length kept.
        let odd = [("`rm -rf`".to_string(), WSC_PRODUCT_ON), ("  ".to_string(), WSC_PRODUCT_ON)];
        assert_eq!(security_center(Some(&odd), None), Presence::Here(Some("'rm -rf'".into())));
        assert_eq!(security_center(Some(&[("x".repeat(100), WSC_PRODUCT_ON)]), None), Presence::Here(Some("x".repeat(60))));
        // Windows Vista and 7: the health alone; good, there unnamed; poor, snoozed, not monitored: unknown.
        assert_eq!(security_center(None, Some(WSC_HEALTH_GOOD)), Presence::Here(None));
        for health in [1, 2, 3] {
            assert_eq!(security_center(None, Some(health)), Presence::Unknown);
        }
        // A server: neither answers.
        assert_eq!(security_center(None, None), Presence::Unknown);
    }
}
