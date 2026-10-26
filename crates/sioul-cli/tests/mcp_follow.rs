// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

//! `sioul mcp` follows its program (docs/mcp.md, "A new program, the same
//! connection"): the real program, started from a link to it in a folder of
//! the test's own, its file replaced between two requests as an install
//! replaces it. A file that is no program leaves the server serving, which
//! says so; a new program takes the connection over, the handshake with it,
//! and says that the tools changed; requests sent all along, two in one
//! write, are each answered once. A home made in the system's temporary
//! folder, its XDG folders and HOME there: the person's own files are never read.

#![cfg(unix)]

use serde_json::{Value, json};
use std::io::{BufRead, BufReader, Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdin, Command, Stdio};
use std::sync::mpsc::{Receiver, channel};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

const FOLDERS: [(&str, &str); 5] = [("XDG_CONFIG_HOME", "config"), ("XDG_DATA_HOME", "data"), ("XDG_STATE_HOME", "state"), ("XDG_CACHE_HOME", "cache"), ("HOME", "home")];

/// The program built for these tests.
const BUILT: &str = env!("CARGO_BIN_EXE_sioul");

/// How long the server may take to see a new file and run it: a second to
/// see it, a second for it to stay as it is, and room for a loaded machine.
const WITHIN: Duration = Duration::from_secs(30);

fn home() -> PathBuf {
    let root = std::env::temp_dir().join(format!("sioul-mcp-follow-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    for (_, folder) in FOLDERS {
        std::fs::create_dir_all(root.join(folder)).unwrap();
    }
    std::fs::create_dir_all(root.join("config/sioul")).unwrap();
    std::fs::write(root.join("config/sioul/config.toml"), "language = \"en\"\n").unwrap();
    std::fs::create_dir_all(root.join("bin")).unwrap();
    root
}

/// What an install puts in place.
enum Made {
    /// The program built, linked rather than copied when the system allows it.
    Linked,
    /// A copy of it: another file, the same program.
    Copied,
    /// A file that is no program, though it may be run.
    NoProgram,
}

/// A new file moved over `program` whole, as an install moves it.
fn install(program: &Path, made: Made) {
    let staged = program.with_extension("new");
    let _ = std::fs::remove_file(&staged);
    match made {
        Made::Linked => {
            if std::fs::hard_link(BUILT, &staged).is_err() {
                std::fs::copy(BUILT, &staged).unwrap();
            }
        }
        Made::Copied => {
            std::fs::copy(BUILT, &staged).unwrap();
        }
        Made::NoProgram => {
            use std::os::unix::fs::PermissionsExt;
            std::fs::write(&staged, "no program\n").unwrap();
            std::fs::set_permissions(&staged, std::fs::Permissions::from_mode(0o755)).unwrap();
        }
    }
    std::fs::rename(&staged, program).unwrap();
}

/// The file the server runs, as Linux tells it: its path, and its inode.
#[cfg(target_os = "linux")]
fn running(child: &Child) -> (PathBuf, u64) {
    use std::os::unix::fs::MetadataExt;
    let exe = format!("/proc/{}/exe", child.id());
    (std::fs::read_link(&exe).unwrap(), std::fs::metadata(&exe).unwrap().ino())
}

/// The file at `path`, as `running` tells it.
#[cfg(target_os = "linux")]
fn at(path: &Path) -> (PathBuf, u64) {
    use std::os::unix::fs::MetadataExt;
    (std::fs::canonicalize(path).unwrap(), std::fs::metadata(path).unwrap().ino())
}

/// Every line of the log, in its order.
fn logged(root: &Path) -> Vec<Value> {
    let folder = root.join("state/sioul/mcp");
    let mut files: Vec<PathBuf> = std::fs::read_dir(&folder).into_iter().flatten().filter_map(|e| e.ok().map(|e| e.path())).collect();
    files.sort();
    files.iter().flat_map(|f| std::fs::read_to_string(f).unwrap_or_default().lines().filter_map(|l| serde_json::from_str(l).ok()).collect::<Vec<Value>>()).collect()
}

/// `sioul mcp`, served as a client serves it: what it writes on standard
/// output read line by line apart, what it says on standard error kept.
struct Served {
    child: Child,
    stdin: Option<ChildStdin>,
    lines: Receiver<String>,
    said: Arc<Mutex<String>>,
    id: u64,
}

impl Served {
    fn start(root: &Path, program: &Path) -> Served {
        let mut command = Command::new(program);
        command.arg("--config").arg(root.join("config/sioul/config.toml")).args(["--language", "en", "mcp"]);
        for (variable, folder) in FOLDERS {
            command.env(variable, root.join(folder));
        }
        command.env_remove("SIOUL_MCP_HANDOVER");
        let mut child = command.stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::piped()).spawn().unwrap();
        let stdin = child.stdin.take();
        let stdout = child.stdout.take().unwrap();
        let (sender, lines) = channel();
        std::thread::spawn(move || {
            for line in BufReader::new(stdout).lines().map_while(Result::ok) {
                if sender.send(line).is_err() {
                    break;
                }
            }
        });
        let said = Arc::new(Mutex::new(String::new()));
        let mut stderr = child.stderr.take().unwrap();
        let kept = Arc::clone(&said);
        std::thread::spawn(move || {
            let mut chunk = [0u8; 4096];
            while let Ok(read) = stderr.read(&mut chunk) {
                if read == 0 {
                    break;
                }
                kept.lock().unwrap().push_str(&String::from_utf8_lossy(&chunk[..read]));
            }
        });
        Served { child, stdin, lines, said, id: 0 }
    }

    /// A request, numbered.
    fn request(&mut self, method: &str, params: Value) -> (u64, Value) {
        self.id += 1;
        (self.id, json!({ "jsonrpc": "2.0", "id": self.id, "method": method, "params": params }))
    }

    /// Messages written in one write, as a client that sends two at once writes them.
    fn send(&mut self, messages: &[Value]) {
        let text: String = messages.iter().map(|m| format!("{m}\n")).collect();
        let stdin = self.stdin.as_mut().unwrap();
        stdin.write_all(text.as_bytes()).unwrap();
        stdin.flush().unwrap();
    }

    /// The next message on standard output, if one comes within `within`.
    fn next(&self, within: Duration) -> Option<Value> {
        let line = self.lines.recv_timeout(within).ok()?;
        Some(serde_json::from_str(&line).unwrap_or_else(|e| panic!("{e}: {line}")))
    }

    /// A request, and its answer: nothing else may come first.
    fn ask(&mut self, method: &str, params: Value) -> Value {
        let (id, request) = self.request(method, params);
        self.send(&[request]);
        let answer = self.next(WITHIN).expect("an answer");
        assert_eq!(answer["id"], id, "{answer}");
        answer
    }

    /// What a program that took the connection over sends first.
    fn tools_changed(&self) {
        let first = self.next(WITHIN).expect("the new program's notification");
        assert_eq!(first, json!({ "jsonrpc": "2.0", "method": "notifications/tools/list_changed" }));
    }

    fn said(&self) -> String {
        self.said.lock().unwrap().clone()
    }
}

impl Drop for Served {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

#[test]
fn a_new_program_takes_the_connection_over() {
    let root = home();
    let program = root.join("bin/sioul");
    install(&program, Made::Linked);
    let mut served = Served::start(&root, &program);
    let hello = served.ask("initialize", json!({ "protocolVersion": "2025-06-18", "capabilities": { "roots": { "listChanged": true } }, "clientInfo": { "name": "follow", "version": "1" } }));
    assert_eq!(hello["result"]["capabilities"]["tools"]["listChanged"], true, "{hello}");
    served.send(&[json!({ "jsonrpc": "2.0", "method": "notifications/initialized" })]);
    assert_eq!(served.ask("ping", json!({}))["result"], json!({}));
    #[cfg(target_os = "linux")]
    let first = running(&served.child).1;

    // A file that is no program, moved in place: the server says so, and goes on serving.
    install(&program, Made::NoProgram);
    let deadline = Instant::now() + WITHIN;
    while !served.said().contains("could not run") {
        assert!(Instant::now() < deadline, "nothing said: {}", served.said());
        std::thread::sleep(Duration::from_millis(100));
    }
    assert_eq!(served.ask("ping", json!({}))["result"], json!({}));
    #[cfg(target_os = "linux")]
    assert_eq!(running(&served.child).1, first, "the same program serves");
    assert!(logged(&root).iter().any(|l| l["event"] == "kept" && l["client"] == "follow"), "{:?}", logged(&root));

    // A new program, between two requests: the second is answered by it, on the connection handed over.
    install(&program, Made::Copied);
    served.tools_changed();
    #[cfg(target_os = "linux")]
    assert_eq!(running(&served.child), at(&program), "the new program serves");
    // A home with no notes folder: the tool says so, answered all the same.
    let projects = served.ask("tools/call", json!({ "name": "list_projects", "arguments": {} }));
    assert!(projects["result"]["content"][0]["text"].is_string(), "{projects}");
    let log = logged(&root);
    let followed = log.iter().position(|l| l["event"] == "followed" && l["client"] == "follow").unwrap_or_else(|| panic!("no line of the new program: {log:?}"));
    assert!(log[followed]["detail"].as_str().unwrap().contains(env!("CARGO_PKG_VERSION")), "{}", log[followed]);
    assert!(log[followed..].iter().any(|l| l["tool"] == "list_projects" && l["client"] == "follow"), "the client known to the new program: {log:?}");

    // Requests sent two in one write, all along, while another program comes: each answered once.
    install(&program, Made::Linked);
    let (mut asked, mut came) = (Vec::new(), Vec::new());
    let deadline = Instant::now() + WITHIN;
    while !came.iter().any(|m: &Value| m["method"] == "notifications/tools/list_changed") {
        assert!(Instant::now() < deadline, "no new program after {} requests", asked.len());
        let (a, one) = served.request("ping", json!({}));
        let (b, two) = served.request("ping", json!({}));
        served.send(&[one, two]);
        asked.extend([a, b]);
        while let Some(message) = served.next(Duration::from_millis(20)) {
            came.push(message);
        }
    }
    let (last, request) = served.request("ping", json!({}));
    served.send(&[request]);
    asked.push(last);
    while came.last().is_none_or(|m| m["id"] != last) {
        came.push(served.next(WITHIN).expect("every answer"));
    }
    let mut answered: Vec<u64> = came.iter().filter_map(|m| m["id"].as_u64()).collect();
    answered.sort_unstable();
    assert_eq!(answered, asked, "each request answered once");
    assert_eq!(came.iter().filter(|m| m["method"] == "notifications/tools/list_changed").count(), 1);
    #[cfg(target_os = "linux")]
    assert_eq!(running(&served.child), at(&program));

    // The client goes: the server ends, as it would have.
    drop(served.stdin.take());
    let deadline = Instant::now() + WITHIN;
    let status = loop {
        if let Some(status) = served.child.try_wait().unwrap() {
            break status;
        }
        assert!(Instant::now() < deadline, "still serving");
        std::thread::sleep(Duration::from_millis(50));
    };
    assert!(status.success(), "{status}: {}", served.said());
    drop(served);
    let _ = std::fs::remove_dir_all(&root);
}
