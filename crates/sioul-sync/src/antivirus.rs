// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

//! Checking a file before it opens: one question, three answers, one way per
//! system (docs/client.md, "Antivirus").
//!
//! - **Windows**: the Antimalware Scan Interface (AMSI) hands the file to the
//!   antivirus Windows already runs, Microsoft Defender by default. Nothing to install.
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
//! - **Without an antivirus**, nothing is refused: the window says the file
//!   will not be checked, asks before opening or saving it, and says how to
//!   install one (`install_hint`).
//! - **A phone** has no antivirus another app can call, and no ClamAV:
//!   `scan` answers `Unavailable` there without trying, and Sioul's window
//!   does not ask it (crates/sioul-app/src/attachments.rs).

use std::path::Path;
#[cfg(not(windows))]
use std::path::PathBuf;

/// What the antivirus said.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Verdict {
    Clean,
    /// A threat, by the name the antivirus gives it.
    Infected(String),
    /// Not scanned: larger than the antivirus scans (a file's size, what an
    /// archive holds, how deep it nests). Nothing is opened without asking.
    TooBig,
    /// No antivirus answered: nothing is opened.
    Unavailable(String),
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
        windows_scan(file)
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
    let daemon = daemon_config().is_some_and(|text| daemon_alerts(&text));
    let mut problem = String::new();
    for (command, options) in runs(own.is_dir().then_some(own.as_path()), daemon) {
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

/// AMSI: the file's bytes, with its name, handed to Windows' antivirus.
#[cfg(windows)]
fn windows_scan(file: &Path) -> Verdict {
    use windows::Win32::System::Antimalware::{AMSI_RESULT_DETECTED, AmsiCloseSession, AmsiInitialize, AmsiOpenSession, AmsiScanBuffer, AmsiUninitialize};
    use windows::core::HSTRING;
    let bytes = match std::fs::read(file) {
        Ok(bytes) => bytes,
        Err(e) => return Verdict::Unavailable(e.to_string()),
    };
    let Ok(length) = u32::try_from(bytes.len()) else { return Verdict::TooBig };
    let name = HSTRING::from(file.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default());
    // SAFETY: AMSI's documented sequence: a context, a session, one scan of a
    // buffer that outlives the call, then both closed, on this thread.
    unsafe {
        let context = match AmsiInitialize(&HSTRING::from("Sioul")) {
            Ok(context) => context,
            Err(e) => return Verdict::Unavailable(e.message()),
        };
        let session = AmsiOpenSession(context).ok();
        let result = AmsiScanBuffer(context, bytes.as_ptr().cast(), length, &name, session);
        if let Some(session) = session {
            AmsiCloseSession(context, session);
        }
        AmsiUninitialize(context);
        match result {
            Ok(result) if result.0 >= AMSI_RESULT_DETECTED.0 => Verdict::Infected("Microsoft Defender".into()),
            Ok(_) => Verdict::Clean,
            Err(e) => Verdict::Unavailable(e.message()),
        }
    }
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
