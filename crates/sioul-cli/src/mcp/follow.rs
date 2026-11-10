// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

//! The program `sioul mcp` runs from, followed (docs/mcp.md, "A new program,
//! the same connection"). An MCP client keeps its server for as long as it
//! runs itself, days at times, while an install puts a new program where the
//! old one was: the agent would go on with the old program's tools. So,
//! between two messages, while nothing the client sent waits unread in this
//! process, the server looks at its program's file; once a new one has stayed
//! as it is for a second, the server runs it in its own place (execve), by the
//! same path, with the same arguments, on the same standard input and output.
//! The handshake goes with it, in `SIOUL_MCP_HANDOVER`: the new program
//! answers the next message without a second `initialize`, and first tells
//! the client that the tools may have changed (mcp.rs).
//!
//! A program is new, on Linux, when /proc/self/exe reads "(deleted)" or the
//! path holds another file than the one that runs; on macOS and the other
//! Unix systems, when the path holds another file or its modification time
//! moved. On Windows nothing is done: a program's file cannot be deleted nor
//! written over while it runs, so an install waits for the client to stop or
//! replaces it at the next start, and Windows has no exec, to run another
//! program in the same process with the same input and output. The client
//! starts the new one the next time it starts `sioul mcp`.
//!
//! `sioul remind --watch` follows its program the same way (remind.rs), with
//! no connection to hand over: an install that leaves it running the old
//! program would leave reminders to the old code until the session ends.

use super::protocol::Server;
use std::time::Duration;

/// The environment variable that hands the connection over to the program
/// that takes over: the revision agreed, the client's name and capabilities.
pub const HANDOVER: &str = "SIOUL_MCP_HANDOVER";

/// How often the program's file is looked at while the client sends nothing.
pub const EVERY: Duration = Duration::from_secs(1);

#[cfg(unix)]
pub use unix::Follow;

/// Windows, and any system without exec: nothing to follow (above).
#[cfg(not(unix))]
pub struct Follow;

#[cfg(not(unix))]
impl Follow {
    /// None: a program that runs is not replaced here, and none can take its place.
    pub fn start() -> Option<Follow> {
        None
    }

    pub fn between(&mut self, _server: &Server) {}

    pub fn step(&mut self, _handover: Option<&str>, _kept: impl FnOnce(&str)) {}
}

#[cfg(unix)]
mod unix {
    use super::super::log;
    use super::{HANDOVER, Server};
    use std::ffi::{CString, OsString};
    use std::os::unix::ffi::OsStrExt;
    use std::path::{Path, PathBuf};
    use std::time::{Duration, Instant};

    /// How long a new program stays as it is before it is run: an install
    /// that writes it in place, rather than moving it there whole, has finished.
    const SETTLE: Duration = Duration::from_secs(1);

    /// How a new program is run: execve(2), or a test's stand-in. It returns
    /// only when the program could not run, with why.
    pub type Exec = fn(&Path, &[OsString], Option<&str>) -> std::io::Error;

    /// How a new program is tried before it takes the connection (`try_it`),
    /// with the arguments it would be given, or a test's stand-in: why it
    /// cannot serve, when it cannot.
    pub type Probe = fn(&Path, &[OsString]) -> Result<(), String>;

    /// The longest a new program's `--version` may take.
    const PROBE: Duration = Duration::from_secs(5);

    /// A file as the system describes it: which one it is, and what was last done to it.
    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    struct Stat {
        device: u64,
        inode: u64,
        length: u64,
        modified: (i64, i64),
        changed: (i64, i64),
        mode: u32,
    }

    impl Stat {
        /// The file at `path`, its links followed; None when there is none, or no file.
        fn of(path: &Path) -> Option<Stat> {
            use std::os::unix::fs::MetadataExt;
            let m = std::fs::metadata(path).ok().filter(std::fs::Metadata::is_file)?;
            Some(Stat {
                device: m.dev(),
                inode: m.ino(),
                length: m.size(),
                modified: (m.mtime(), m.mtime_nsec()),
                changed: (m.ctime(), m.ctime_nsec()),
                mode: m.mode(),
            })
        }

        fn same_file(&self, other: &Stat) -> bool {
            (self.device, self.inode) == (other.device, other.inode)
        }
    }

    /// The program followed: where it is, how it was started, and what was seen of its file.
    pub struct Follow {
        /// The path it was started by.
        path: PathBuf,
        /// Its arguments, its own name first: the new program gets the same.
        arguments: Vec<OsString>,
        /// The file that runs, as it was at the start.
        running: Stat,
        /// Whether that file is this process's own program, which Linux's
        /// /proc/self/exe describes (a test's file is not).
        own: bool,
        /// A new file seen, and since when: it runs once it stayed as it is for `settle`.
        seen: Option<(Stat, Instant)>,
        /// A new file that could not run: left alone until the path holds another.
        failed: Option<Stat>,
        settle: Duration,
        probe: Probe,
        exec: Exec,
    }

    impl Follow {
        /// This process's program, followed; None when its file cannot be told.
        pub fn start() -> Option<Follow> {
            let (resolved, running) = own_program()?;
            let path = started_as(&running).unwrap_or(resolved);
            Some(Follow { path, arguments: std::env::args_os().collect(), running, own: true, seen: None, failed: None, settle: SETTLE, probe: try_it, exec })
        }

        /// A file followed as if it were the program, tried by `probe` and
        /// run by `exec` at once when it is replaced: for tests.
        #[cfg(test)]
        pub fn of_file(path: &Path, probe: Probe, exec: Exec) -> Follow {
            let running = Stat::of(path).expect("a file to follow");
            Follow { path: path.to_path_buf(), arguments: vec!["sioul".into(), "mcp".into()], running, own: false, seen: None, failed: None, settle: Duration::ZERO, probe, exec }
        }

        /// Between two messages, while nothing the client sent waits unread:
        /// when a new program is in place and has stayed as it is for a
        /// second, it runs in this one's place, and this does not return. One
        /// that cannot run is said on standard error and in the log, then left
        /// alone until the path holds another file: this program goes on serving.
        pub fn between(&mut self, server: &Server) {
            self.step(server.handover().as_deref(), |said| {
                eprintln!("sioul mcp: {said}.");
                log::event(server.client(), "kept", said);
            });
        }

        /// `between`, for any program that follows its own: `handover` goes
        /// to the new program in its environment when there is one; `kept`
        /// says why a new program could not run, this one going on.
        pub fn step(&mut self, handover: Option<&str>, kept: impl FnOnce(&str)) {
            let Some(new) = self.replacement() else {
                self.seen = None;
                return;
            };
            if self.failed == Some(new) {
                return;
            }
            match self.seen {
                Some((seen, since)) if seen == new => {
                    if since.elapsed() < self.settle {
                        return;
                    }
                }
                _ => {
                    self.seen = Some((new, Instant::now()));
                    if !self.settle.is_zero() {
                        return;
                    }
                }
            }
            // A file anyone else could have put or changed there is never run as you.
            // Tried first: a program that cannot start (its arguments refused, a library
            // missing, a file cut short, an older version) never takes the connection,
            // which would end.
            let error = match yours_alone(&self.path).and_then(|()| (self.probe)(&self.path, &self.arguments)) {
                Ok(()) => (self.exec)(&self.path, &self.arguments, handover).to_string(),
                Err(why) => why,
            };
            self.seen = None;
            self.failed = Some(new);
            kept(&format!("its program was replaced, but the new one could not run ({error}); this one goes on serving"));
        }

        /// The file now at the program's path, when it is another than the one that runs.
        fn replacement(&self) -> Option<Stat> {
            let now = Stat::of(&self.path)?;
            let replaced = !now.same_file(&self.running) || (self.own && deleted()) || (WRITTEN_OVER && now.modified != self.running.modified);
            replaced.then_some(now)
        }
    }

    /// Whether a program's file can be written over while it runs, so that
    /// its time tells it: not on Linux, which refuses (ETXTBSY).
    const WRITTEN_OVER: bool = !cfg!(any(target_os = "linux", target_os = "android"));

    /// What /proc/self/exe adds to the name of a program whose file was deleted.
    #[cfg(any(target_os = "linux", target_os = "android"))]
    const DELETED: &[u8] = b" (deleted)";

    /// The program that runs: its path, links resolved, and its file, even deleted.
    #[cfg(any(target_os = "linux", target_os = "android"))]
    fn own_program() -> Option<(PathBuf, Stat)> {
        let link = std::fs::read_link("/proc/self/exe").ok()?;
        let name = link.as_os_str().as_bytes();
        let path = PathBuf::from(std::ffi::OsStr::from_bytes(name.strip_suffix(DELETED).unwrap_or(name)));
        Some((path, Stat::of(Path::new("/proc/self/exe"))?))
    }

    /// Whether /proc/self/exe reads "(deleted)": the program's file is gone from its folder.
    #[cfg(any(target_os = "linux", target_os = "android"))]
    fn deleted() -> bool {
        std::fs::read_link("/proc/self/exe").is_ok_and(|link| link.as_os_str().as_bytes().ends_with(DELETED))
    }

    /// No /proc to ask: the path's file and time tell it.
    #[cfg(not(any(target_os = "linux", target_os = "android")))]
    fn deleted() -> bool {
        false
    }

    #[cfg(not(any(target_os = "linux", target_os = "android")))]
    fn own_program() -> Option<(PathBuf, Stat)> {
        let path = std::path::absolute(std::env::current_exe().ok()?).ok()?;
        let running = Stat::of(&path)?;
        Some((path, running))
    }

    /// The path the client started the program by, when it still names the
    /// file that runs: its own name when that holds a folder (a full path, as
    /// Claude Desktop is given), else the first folder of the `PATH` holding
    /// it. Through a link, the link: an install that moves the link to a new
    /// program is followed too.
    fn started_as(running: &Stat) -> Option<PathBuf> {
        let first = PathBuf::from(std::env::args_os().next()?);
        let candidates: Vec<PathBuf> = if first.components().count() > 1 {
            vec![std::path::absolute(&first).ok()?]
        } else {
            std::env::split_paths(&std::env::var_os("PATH")?).map(|folder| folder.join(&first)).filter(|path| path.is_absolute()).collect()
        };
        candidates.into_iter().find(|path| Stat::of(path).is_some_and(|stat| stat.same_file(running)))
    }

    /// Whether the file at `path`, and the folders that hold it (its link's
    /// and its own), are yours or the system's, and nobody else can write
    /// them: a file someone else put there is never run as you (review of
    /// wt/mail, F8). A folder others may write is accepted when it keeps
    /// what each one put there (the sticky bit, as /tmp).
    pub(super) fn yours_alone(path: &Path) -> Result<(), String> {
        use std::os::unix::fs::MetadataExt;
        // SAFETY: getuid cannot fail.
        let me = unsafe { libc::getuid() };
        let owned = |m: &std::fs::Metadata| m.uid() == me || m.uid() == 0;
        let real = std::fs::canonicalize(path).map_err(|e| e.to_string())?;
        let file = std::fs::metadata(&real).map_err(|e| e.to_string())?;
        if !owned(&file) || file.mode() & 0o022 != 0 {
            return Err(format!("{} belongs to another account, or another account may change it", real.display()));
        }
        for folder in [path.parent(), real.parent()].into_iter().flatten() {
            let held = std::fs::metadata(folder).map_err(|e| e.to_string())?;
            if !owned(&held) || (held.mode() & 0o022 != 0 && held.mode() & 0o1000 == 0) {
                return Err(format!("{} belongs to another account, or another account may write in it", folder.display()));
            }
        }
        Ok(())
    }

    /// The new program tried before it takes the connection, standard input
    /// from /dev/null, its output captured, five seconds at most each
    /// (`PROBE`): `--version`, never older than this program's; then the
    /// arguments it would be given, with `--help`, which an older program
    /// refusing one of them (`remind --watch --session`) fails (review of
    /// wt/mail, F3). Why it cannot serve, when it cannot.
    fn try_it(path: &Path, arguments: &[OsString]) -> Result<(), String> {
        let said = run(path, &[OsString::from("--version")], true)?;
        if let Some(theirs) = version_in(&said)
            && theirs < version_in(&format!("sioul {}", env!("CARGO_PKG_VERSION"))).unwrap_or_default()
        {
            return Err(format!("it is an older version ({})", said.trim()));
        }
        // After `--`, `--help` would be a word: the arguments are not tried then.
        let given = arguments.get(1..).unwrap_or_default();
        if given.iter().any(|a| a == "--") {
            return Ok(());
        }
        let mut asked: Vec<OsString> = given.to_vec();
        asked.push("--help".into());
        run(path, &asked, false).map(drop).map_err(|why| format!("it refuses the arguments it would be given ({why})"))
    }

    /// `path` run with `arguments`: what it printed (with `kept`; a short
    /// answer alone, which the pipe holds), when it ended well.
    fn run(path: &Path, arguments: &[OsString], kept: bool) -> Result<String, String> {
        use std::process::{Command, Stdio};
        let out = if kept { Stdio::piped() } else { Stdio::null() };
        let mut child = Command::new(path).args(arguments).env_remove(HANDOVER).stdin(Stdio::null()).stdout(out).stderr(Stdio::null()).spawn().map_err(|e| e.to_string())?;
        let deadline = Instant::now() + PROBE;
        loop {
            match child.try_wait().map_err(|e| e.to_string())? {
                Some(status) if status.success() => {
                    let mut out = String::new();
                    if let Some(mut stdout) = child.stdout.take() {
                        let _ = std::io::Read::read_to_string(&mut stdout, &mut out);
                    }
                    return Ok(out);
                }
                Some(status) => return Err(format!("{} ended with {status}", arguments.first().map_or(String::new(), |a| a.to_string_lossy().into_owned()))),
                None if Instant::now() >= deadline => {
                    let _ = child.kill();
                    let _ = child.wait();
                    return Err("it did not answer within five seconds".into());
                }
                None => std::thread::sleep(Duration::from_millis(20)),
            }
        }
    }

    /// The version a `--version` line says ("sioul 0.0.9 …" → [0, 0, 9]); none when it says none.
    pub(super) fn version_in(said: &str) -> Option<Vec<u64>> {
        let word = said.split_whitespace().nth(1)?;
        let numbers: Vec<u64> = word.split(['-', '+', ' ']).next()?.split('.').map(|n| n.parse().ok()).collect::<Option<_>>()?;
        (!numbers.is_empty()).then_some(numbers)
    }

    /// Runs `path` in this process's place with `arguments`, the environment
    /// as it is and the handover in it: execve(2) itself rather than
    /// `Command::exec`, whose execvp would hand a file that is no program to
    /// /bin/sh. Returns only when it could not.
    fn exec(path: &Path, arguments: &[OsString], handover: Option<&str>) -> std::io::Error {
        let text = |bytes: &[u8]| CString::new(bytes).map_err(|_| std::io::Error::new(std::io::ErrorKind::InvalidInput, "a NUL byte in an argument"));
        let made = (|| {
            let program = text(path.as_os_str().as_bytes())?;
            let arguments = arguments.iter().map(|a| text(a.as_bytes())).collect::<Result<Vec<_>, _>>()?;
            let mut environment = Vec::new();
            for (name, value) in std::env::vars_os().filter(|(name, _)| name != HANDOVER) {
                environment.push(text(&[name.as_bytes(), b"=", value.as_bytes()].concat())?);
            }
            if let Some(handover) = handover {
                environment.push(text(format!("{HANDOVER}={handover}").as_bytes())?);
            }
            Ok::<_, std::io::Error>((program, arguments, environment))
        })();
        let (program, arguments, environment) = match made {
            Ok(made) => made,
            Err(e) => return e,
        };
        let pointers = |strings: &[CString]| strings.iter().map(|s| s.as_ptr()).chain(std::iter::once(std::ptr::null())).collect::<Vec<_>>();
        let (argv, envp) = (pointers(&arguments), pointers(&environment));
        // SAFETY: every pointer points into a CString that lives until the
        // call returns, and both lists end with a null pointer, as execve(2)
        // asks. Standard output was flushed with each message; descriptors 0,
        // 1 and 2 go on to the new program, the others close (CLOEXEC).
        unsafe { libc::execve(program.as_ptr(), argv.as_ptr(), envp.as_ptr()) };
        std::io::Error::last_os_error()
    }
}

#[cfg(test)]
mod tests {
    /// Review of wt/mail, F3 and F8: versions read from `--version`; a
    /// program others may change, or in a folder others may write, is
    /// never followed.
    #[cfg(unix)]
    #[test]
    fn a_new_program_is_followed_only_when_yours_alone_and_not_older() {
        use super::unix::{version_in, yours_alone};
        assert_eq!(version_in("sioul 0.0.9 (a1b2c3d, 2026-11-10)"), Some(vec![0, 0, 9]));
        assert_eq!(version_in("sioul 0.0.10-3-gabcdef"), Some(vec![0, 0, 10]));
        assert!(version_in("sioul 0.0.9").unwrap() < version_in("sioul 0.0.10").unwrap());
        assert_eq!(version_in("nonsense"), None);
        use std::os::unix::fs::PermissionsExt;
        let dir = std::env::temp_dir().join(format!("sioul-follow-owner-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(0o755)).unwrap();
        let program = dir.join("sioul");
        std::fs::write(&program, "a program").unwrap();
        std::fs::set_permissions(&program, std::fs::Permissions::from_mode(0o755)).unwrap();
        assert!(yours_alone(&program).is_ok());
        std::fs::set_permissions(&program, std::fs::Permissions::from_mode(0o775)).unwrap();
        assert!(yours_alone(&program).is_err(), "a group may change it");
        std::fs::set_permissions(&program, std::fs::Permissions::from_mode(0o755)).unwrap();
        std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(0o777)).unwrap();
        assert!(yours_alone(&program).is_err(), "anyone may put another there");
        std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(0o1777)).unwrap();
        assert!(yours_alone(&program).is_ok(), "sticky: what you put there stays yours");
        std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(0o755)).unwrap();
        let _ = std::fs::remove_dir_all(&dir);
    }
}
