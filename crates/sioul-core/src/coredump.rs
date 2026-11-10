// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

//! No copy of Sioul's memory on the disk when it crashes (docs/client.md,
//! "What a disk keeps"). A core file holds what the program held: the
//! messages shown, the session keys of encrypted ones, a security key's PIN.
//! Fedora's systemd-coredump keeps one for every crash, and Ubuntu's Apport
//! puts one in its crash reports.
//!
//! - **Linux and Android**: the core file's limit (`RLIMIT_CORE`) set to one
//!   byte for Sioul and what it starts. The kernel then hands nothing to a
//!   crash collector named in `kernel.core_pattern` (fs/coredump.c: "RLIMIT_CORE
//!   is set to 1, aborting core", the guard of collectors that crash
//!   themselves), and writes no core file, whose smallest is a page. A limit
//!   of 0 would still pass the memory to systemd-coredump, which drops it,
//!   and to Apport, which keeps it. `PR_SET_DUMPABLE` is left alone: with
//!   Fedora's `fs.suid_dumpable = 2` the kernel still hands a process that
//!   is not dumpable to the collector, and the Web view's sandbox writes its
//!   own `/proc/self/uid_map` in the child it clones, which a process that is
//!   not dumpable can no longer do.
//! - **macOS and other Unix**: the limit at 0; macOS writes no core file by
//!   default, and its crash reports hold no memory.
//! - **Windows**: Windows Error Reporting leaves Sioul out (an exclusion for
//!   the program's name, yours alone), and a report made all the same holds
//!   no heap (`WerSetFlags`).
//!
//! `SIOUL_CORE_DUMP=1` in the environment leaves all of this as the system
//! has it, to debug a crash.

/// The environment's switch that lets a crash leave a core file, to debug it.
pub const ALLOW: &str = "SIOUL_CORE_DUMP";

/// No core file of a crash, unless the environment allows one ([`ALLOW`]):
/// called first thing by each program. What could not be set is returned, said.
pub fn no_core_file() -> Result<(), String> {
    keep_memory_off_disk(std::env::var_os(ALLOW).is_some_and(|v| v == "1"))
}

fn keep_memory_off_disk(allowed: bool) -> Result<(), String> {
    if allowed {
        #[cfg(windows)]
        windows::include();
        return Ok(());
    }
    #[cfg(unix)]
    {
        // SAFETY: getrlimit and setrlimit read and write the struct given, nothing else.
        unsafe {
            let mut limit = libc::rlimit { rlim_cur: 0, rlim_max: 0 };
            if libc::getrlimit(libc::RLIMIT_CORE, &mut limit) != 0 {
                return Err(std::io::Error::last_os_error().to_string());
            }
            // Kept, for the programs Sioul starts (`as_before`).
            let _ = BEFORE.set((limit.rlim_cur, limit.rlim_max));
            // The soft limit alone: a program Sioul starts may raise it again.
            limit.rlim_cur = SOFT.min(limit.rlim_max);
            if libc::setrlimit(libc::RLIMIT_CORE, &limit) != 0 {
                return Err(std::io::Error::last_os_error().to_string());
            }
            // A hard limit of 0 leaves 0, which a collector a pipe reaches ignores (review of wt/mail, F6).
            if limit.rlim_cur < SOFT {
                return Err(format!("the system's hard limit of a core file is {}: a crash collector still receives the memory", limit.rlim_max));
            }
        }
        #[cfg(any(target_os = "linux", target_os = "android"))]
        if std::fs::read_to_string("/proc/sys/kernel/core_pattern").is_ok_and(|pattern| pattern.starts_with('@')) {
            return Err("the system hands core files to a socket, whatever their limit: a crash collector still receives the memory".into());
        }
    }
    #[cfg(windows)]
    windows::exclude()?;
    Ok(())
}

/// The core file limit Sioul had before it lowered its own: soft, hard.
#[cfg(unix)]
static BEFORE: std::sync::OnceLock<(libc::rlim_t, libc::rlim_t)> = std::sync::OnceLock::new();

/// `command`, another program Sioul starts (a scanner, the system's
/// settings), given back the core file limit Sioul had before it lowered
/// its own, rather than inheriting it (review of wt/mail, F6). Programs the
/// window opens through Qt (a file's viewer) inherit Sioul's own.
pub fn as_before(command: &mut std::process::Command) {
    #[cfg(unix)]
    if let Some(&(soft, hard)) = BEFORE.get() {
        use std::os::unix::process::CommandExt;
        // SAFETY: the closure runs in the child between fork and exec, and
        // calls setrlimit alone, which is async-signal-safe.
        unsafe {
            command.pre_exec(move || {
                let limit = libc::rlimit { rlim_cur: soft, rlim_max: hard };
                libc::setrlimit(libc::RLIMIT_CORE, &limit);
                Ok(())
            });
        }
    }
    #[cfg(not(unix))]
    let _ = command;
}

/// One byte on Linux and Android, where the kernel then hands nothing to a
/// crash collector; none elsewhere.
#[cfg(unix)]
const SOFT: libc::rlim_t = if cfg!(any(target_os = "linux", target_os = "android")) { 1 } else { 0 };

/// The soft limit of a core file now, in bytes.
#[cfg(unix)]
pub fn core_limit() -> Option<u64> {
    let mut limit = libc::rlimit { rlim_cur: 0, rlim_max: 0 };
    // SAFETY: getrlimit writes the struct given, nothing else.
    (unsafe { libc::getrlimit(libc::RLIMIT_CORE, &mut limit) } == 0).then_some(limit.rlim_cur as u64)
}

#[cfg(windows)]
mod windows {
    #[link(name = "wer")]
    unsafe extern "system" {
        fn WerAddExcludedApplication(exe_name: *const u16, all_users: i32) -> i32;
        fn WerRemoveExcludedApplication(exe_name: *const u16, all_users: i32) -> i32;
        fn WerSetFlags(flags: u32) -> i32;
    }

    /// WER_FAULT_REPORTING_FLAG_NOHEAP: a report holds no heap.
    const NO_HEAP: u32 = 1;

    fn program() -> Option<Vec<u16>> {
        use std::os::windows::ffi::OsStrExt;
        let exe = std::env::current_exe().ok()?;
        Some(exe.file_name()?.encode_wide().chain(std::iter::once(0)).collect())
    }

    pub(super) fn exclude() -> Result<(), String> {
        // SAFETY: the flags are a plain number.
        unsafe { WerSetFlags(NO_HEAP) };
        let Some(name) = program() else { return Err("no program name".into()) };
        // SAFETY: `name` is a zero-terminated UTF-16 string that outlives the call.
        let result = unsafe { WerAddExcludedApplication(name.as_ptr(), 0) };
        if result < 0 { Err(format!("Windows Error Reporting: {result:#x}")) } else { Ok(()) }
    }

    pub(super) fn include() {
        if let Some(name) = program() {
            // SAFETY: as in `exclude`.
            unsafe { WerRemoveExcludedApplication(name.as_ptr(), 0) };
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// TODO §4 (10 November 2026): a crash leaves no core file by default,
    /// and the switch leaves the limit as it was.
    #[cfg(unix)]
    #[test]
    fn a_crash_leaves_no_core_file_unless_allowed() {
        let before = core_limit().unwrap();
        keep_memory_off_disk(true).unwrap();
        assert_eq!(core_limit(), Some(before), "allowed: as the system has it");
        // Off a socket pattern, and with a hard limit above a byte, said all right.
        let socket = std::fs::read_to_string("/proc/sys/kernel/core_pattern").is_ok_and(|p| p.starts_with('@'));
        let set = keep_memory_off_disk(false);
        assert!(set.is_ok() || socket || before == 0, "{set:?}");
        let most = if cfg!(any(target_os = "linux", target_os = "android")) { 1 } else { 0 };
        assert!(core_limit().is_some_and(|limit| limit <= most), "{:?}", core_limit());
        // A program Sioul starts gets the limit back (F6).
        let mut command = std::process::Command::new("sh");
        command.args(["-c", "ulimit -c"]);
        as_before(&mut command);
        let said = String::from_utf8(command.output().unwrap().stdout).unwrap();
        let expected = match BEFORE.get().unwrap().0 { libc::RLIM_INFINITY => "unlimited".to_string(), blocks => (blocks / 512).to_string() };
        assert_eq!(said.trim(), expected, "the limit before, in the shell's 512-byte blocks");
        // Each program asks it first thing.
        for (crate_dir, file) in [("sioul-app", "src/lib.rs"), ("sioul-cli", "src/main.rs")] {
            let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("..").join(crate_dir).join(file);
            let source = std::fs::read_to_string(&path).unwrap();
            assert!(source.contains("coredump::no_core_file()"), "{}", path.display());
        }
    }
}
