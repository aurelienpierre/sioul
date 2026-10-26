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
        exec: Exec,
    }

    impl Follow {
        /// This process's program, followed; None when its file cannot be told.
        pub fn start() -> Option<Follow> {
            let (resolved, running) = own_program()?;
            let path = started_as(&running).unwrap_or(resolved);
            Some(Follow { path, arguments: std::env::args_os().collect(), running, own: true, seen: None, failed: None, settle: SETTLE, exec })
        }

        /// A file followed as if it were the program, run by `exec` at once
        /// when it is replaced: for tests.
        #[cfg(test)]
        pub fn of_file(path: &Path, exec: Exec) -> Follow {
            let running = Stat::of(path).expect("a file to follow");
            Follow { path: path.to_path_buf(), arguments: vec!["sioul".into(), "mcp".into()], running, own: false, seen: None, failed: None, settle: Duration::ZERO, exec }
        }

        /// Between two messages, while nothing the client sent waits unread:
        /// when a new program is in place and has stayed as it is for a
        /// second, it runs in this one's place, and this does not return. One
        /// that cannot run is said on standard error and in the log, then left
        /// alone until the path holds another file: this program goes on serving.
        pub fn between(&mut self, server: &Server) {
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
            let error = (self.exec)(&self.path, &self.arguments, server.handover().as_deref());
            self.seen = None;
            self.failed = Some(new);
            let said = format!("its program was replaced, but the new one could not run ({error}); this one goes on serving");
            eprintln!("sioul mcp: {said}.");
            log::event(server.client(), "kept", &said);
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
