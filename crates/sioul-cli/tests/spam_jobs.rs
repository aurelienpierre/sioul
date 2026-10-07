// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

//! The spam filter's jobs, run apart by the real program (docs/mcp.md, "The
//! spam filter's tools"): `sioul mcp` served on standard input as an MCP
//! client serves it, `spam_train` started and followed with `spam_job` to
//! its end, in a process of its own; `spam_fetch` with no account that
//! fetches mail ending on what it could not do; `sioul spam job` listing
//! both. A home made in the system's temporary folder, its XDG folders and
//! HOME there, a corpus of invented mail on reserved domains (RFC 2606):
//! the person's own files are never read, no server is reached, no keyring opened.

use serde_json::{Value, json};
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};
use std::time::{Duration, Instant};

const FOLDERS: [(&str, &str); 5] = [("XDG_CONFIG_HOME", "config"), ("XDG_DATA_HOME", "data"), ("XDG_STATE_HOME", "state"), ("XDG_CACHE_HOME", "cache"), ("HOME", "home")];

/// The home: an account that keeps mail here but fetches none (no server), and a corpus of invented mail.
fn home() -> PathBuf {
    let root = std::env::temp_dir().join(format!("sioul-spam-jobs-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    for (_, folder) in FOLDERS {
        std::fs::create_dir_all(root.join(folder)).unwrap();
    }
    let config = root.join("config/sioul/config.toml");
    std::fs::create_dir_all(config.parent().unwrap()).unwrap();
    let maildir = root.join("mail/home");
    for sub in ["cur", "new", "tmp"] {
        std::fs::create_dir_all(maildir.join(sub)).unwrap();
    }
    let text = format!("language = \"en\"\n\n[[account]]\nid = \"home\"\nkind = \"imap\"\naddress = \"owner@example.org\"\nmaildir = \"{}\"\n", maildir.display().to_string().replace('\\', "\\\\"));
    std::fs::write(&config, text).unwrap();
    let dirs = sioul_learn::Dirs { data: root.join("data/sioul/spam"), state: root.join("state/sioul/spam"), cache: root.join("cache/sioul/spam") };
    let mail = sioul_learn::synthetic::mailbox(9, 300, 200);
    let records: Vec<_> = mail.iter().enumerate().map(|(i, m)| sioul_learn::synthetic::record_of(m, 1, i as u32 + 1)).collect();
    sioul_learn::corpus::store(&dirs, &records).unwrap();
    root
}

/// The program, in the home.
fn sioul(root: &Path) -> Command {
    let mut command = Command::new(env!("CARGO_BIN_EXE_sioul"));
    command.arg("--config").arg(root.join("config/sioul/config.toml")).args(["--language", "en"]);
    for (variable, folder) in FOLDERS {
        command.env(variable, root.join(folder));
    }
    command
}

/// `sioul mcp`, served line by line.
struct Served {
    child: Child,
    stdin: ChildStdin,
    stdout: BufReader<ChildStdout>,
    id: u64,
}

impl Served {
    fn start(root: &Path) -> Served {
        let mut child = sioul(root).arg("mcp").stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::null()).spawn().unwrap();
        let stdin = child.stdin.take().unwrap();
        let stdout = BufReader::new(child.stdout.take().unwrap());
        Served { child, stdin, stdout, id: 0 }
    }

    fn ask(&mut self, method: &str, params: Value) -> Value {
        self.id += 1;
        writeln!(self.stdin, "{}", json!({ "jsonrpc": "2.0", "id": self.id, "method": method, "params": params })).unwrap();
        self.stdin.flush().unwrap();
        let mut line = String::new();
        self.stdout.read_line(&mut line).unwrap();
        serde_json::from_str(&line).unwrap_or_else(|e| panic!("{e}: {line}"))
    }

    fn call(&mut self, tool: &str, arguments: Value) -> Value {
        self.ask("tools/call", json!({ "name": tool, "arguments": arguments }))["result"].clone()
    }

    /// A job followed until it ends: its last answer.
    fn until_ended(&mut self, id: &str) -> Value {
        let deadline = Instant::now() + Duration::from_secs(300);
        loop {
            let job = self.call("spam_job", json!({ "id": id }));
            let state = job["structuredContent"]["state"].as_str().unwrap_or_default().to_string();
            if !["starting", "running"].contains(&state.as_str()) {
                return job;
            }
            assert!(Instant::now() < deadline, "still {state}: {job}");
            std::thread::sleep(Duration::from_millis(250));
        }
    }
}

impl Drop for Served {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

fn text(result: &Value) -> String {
    result["content"][0]["text"].as_str().unwrap_or_default().to_string()
}

#[test]
fn jobs_run_apart_and_say_their_end() {
    let root = home();
    let mut served = Served::start(&root);
    served.ask("initialize", json!({ "protocolVersion": "2025-06-18", "capabilities": {}, "clientInfo": { "name": "jobs", "version": "1" } }));
    // A trial training, started apart: answered at once, with its id.
    let started = served.call("spam_train", json!({ "no_fetch": true, "dim": 8, "epochs": 2, "bucket": 2000, "threads": 2, "errors": 3 }));
    assert_eq!(started["isError"], false, "{started}");
    let id = started["structuredContent"]["id"].as_str().unwrap().to_string();
    assert!(text(&started).starts_with("Started apart") && id.contains("-train-"), "{started}");
    let args: Vec<&str> = started["structuredContent"]["args"].as_array().unwrap().iter().map(|a| a.as_str().unwrap()).collect();
    assert!(args.contains(&"--no-replace") && args.contains(&"--no-fetch") && args.windows(2).any(|w| w == ["--dim", "8"]), "{args:?}");
    // Followed to its end, in its own process.
    let job = served.until_ended(&id);
    let content = &job["structuredContent"];
    assert_eq!(content["state"], "done", "{job}");
    assert!(content["pid"].as_u64().is_some_and(|pid| pid != u64::from(served.child.id())), "{job}");
    let result = &content["result"];
    assert_eq!((result["trial"].clone(), result["summary"]["replaced"].clone(), result["summary"]["model"]["dim"].clone()), (json!(true), json!(false), json!(8)), "{result}");
    assert!(result["errors"]["ham_called_spam"].as_array().is_some_and(|e| e.len() <= 3) && result["grid"].as_array().is_some_and(|g| g.len() == 10), "{result}");
    assert!(text(&job).contains("A trial:"), "{}", text(&job));
    assert!(!root.join("data/sioul/spam/table.bin").exists() && !root.join("state/sioul/spam/trained.toml").exists(), "a trial writes no table");
    let folder = root.join("state/sioul/spam/jobs");
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        assert_eq!(std::fs::metadata(folder.join(format!("{id}.json"))).unwrap().permissions().mode() & 0o777, 0o600);
    }
    // A download with no account that fetches mail: started, then failed, said.
    let fetch = served.call("spam_fetch", json!({}));
    assert_eq!(fetch["isError"], false, "{fetch}");
    let fetch_id = fetch["structuredContent"]["id"].as_str().unwrap().to_string();
    let ended = served.until_ended(&fetch_id);
    assert_eq!(ended["structuredContent"]["state"], "failed", "{ended}");
    assert!(ended["structuredContent"]["error"].as_str().unwrap().contains("Nothing to fetch"), "{ended}");
    // Both kept, the newest first, from the command line too.
    let listed = sioul(&root).args(["spam", "job", "--json"]).output().unwrap();
    let listed: Value = serde_json::from_slice(&listed.stdout).unwrap();
    let ids: Vec<&str> = listed["jobs"].as_array().unwrap().iter().map(|j| j["id"].as_str().unwrap()).collect();
    assert_eq!(ids.len(), 2, "{listed}");
    assert!(ids.contains(&id.as_str()) && ids.contains(&fetch_id.as_str()), "{listed}");
    drop(served);
    let _ = std::fs::remove_dir_all(&root);
}
