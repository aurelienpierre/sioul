// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

//! The engine on small modules written for each case (WebAssembly's text
//! form), and on stand-in services: what an extension is given, each limit,
//! each calm end, the log's hiding. No network, no keyring, no window.

use super::engine::{self, Limits, Services, Stop, WebAnswer, WebRequest};
use super::*;
use sioul_core::extensions::manifest::Grants;
use std::collections::BTreeSet;

/// Stand-in services: the web answers 200 with what was asked, the secret
/// in memory, the sign-in page returns what the test says; removed once
/// `removed_after` asks were looked at, when it says so.
#[derive(Default)]
struct Stand {
    asked: Vec<WebRequest>,
    secret: Option<String>,
    returned: Option<String>,
    pages: Vec<String>,
    removed_after: Option<usize>,
    looked: usize,
}

impl Services for Stand {
    fn web(&mut self, request: &WebRequest, _: Duration, _: usize) -> Result<WebAnswer, &'static str> {
        self.asked.push(request.clone());
        Ok(WebAnswer { status: 200, headers: vec![("Content-Type".into(), "text/plain".into())], body: format!("you asked {}", request.url) })
    }
    fn secret(&mut self) -> Result<Option<String>, &'static str> {
        Ok(self.secret.clone())
    }
    fn keep_secret(&mut self, secret: &str) -> Result<(), &'static str> {
        self.secret = Some(secret.to_string());
        Ok(())
    }
    fn forget_secret(&mut self) -> Result<(), &'static str> {
        self.secret = None;
        Ok(())
    }
    fn sign_in(&mut self, open: &str, _: &str, host: &str, _: Duration) -> Result<Option<String>, &'static str> {
        self.pages.push(format!("{host} {open}"));
        Ok(self.returned.clone())
    }
    fn now(&self) -> i64 {
        1_794_996_000
    }
    fn removed(&mut self) -> bool {
        self.looked += 1;
        self.removed_after.is_some_and(|after| self.looked > after)
    }
}

fn grants(hosts: &[&str], sign_in: Option<&str>) -> Grants {
    Grants { hosts: hosts.iter().map(|h| h.to_string()).collect::<BTreeSet<_>>(), sign_in: sign_in.map(str::to_string) }
}

/// A WebAssembly text's string of bytes.
fn data(text: &str) -> String {
    text.bytes().map(|b| format!("\\{b:02x}")).collect()
}

/// A module of the interface: its imports (by name, each `(param i32 i32) (result i32)` but `answer` and `reply`),
/// its asks written at 1024 one after another, and `body`, the code of `sioul_call`.
/// `$at_N` and `$len_N` stand for the Nth ask's place and length.
fn module(imports: &[&str], asks: &[&str], body: &str) -> Vec<u8> {
    let mut text = String::from("(module\n");
    for name in imports {
        let kind = match *name {
            "answer" => "(param i32)",
            "reply" => "(param i32 i32)",
            _ => "(param i32 i32) (result i32)",
        };
        let (module, field) = name.split_once('.').unwrap_or(("sioul", name));
        text.push_str(&format!("  (import \"{module}\" \"{field}\" (func ${} {kind}))\n", field.replace('.', "_")));
    }
    text.push_str("  (memory (export \"memory\") 8)\n");
    let mut body = body.to_string();
    let mut at = 1024;
    for (i, ask) in asks.iter().enumerate() {
        text.push_str(&format!("  (data (i32.const {at}) \"{}\")\n", data(ask)));
        body = body.replace(&format!("$at_{i}"), &at.to_string()).replace(&format!("$len_{i}"), &ask.len().to_string());
        at += ask.len() + 16;
    }
    text.push_str("  (func (export \"sioul_interface\") (result i32) i32.const 1)\n");
    text.push_str("  (func (export \"sioul_alloc\") (param i32) (result i32) i32.const 32768)\n");
    text.push_str(&format!("  (func (export \"sioul_call\") (param i32 i32) (result i32) (local $n i32)\n{body}\n  )\n)\n"));
    wat::parse_str(&text).unwrap_or_else(|e| panic!("{e}\n{text}"))
}

/// The answer to ask 0 given back as the reply.
const ECHO: &str = "(local.set $n (call $ASK (i32.const $at_0) (i32.const $len_0)))
    (call $answer (i32.const 49152))
    (call $reply (i32.const 49152) (local.get $n))
    i32.const 0";

fn echo(import: &str) -> String {
    ECHO.replace("$ASK", &format!("${import}"))
}

fn small() -> Limits {
    Limits { memory: 2 << 20, steps: 200_000_000, slice: 10_000_000, ..Limits::default() }
}

fn run_it(file: &[u8], grants: &Grants, services: Stand, may_sign_in: bool) -> (engine::Outcome, Stand) {
    engine::call(file, grants, &call_text(Call::Refresh, false), services, &small(), may_sign_in)
}

#[test]
fn a_module_replies_through_its_memory() {
    let file = module(&["now", "answer", "reply"], &["{}"], &echo("now"));
    let (outcome, _) = run_it(&file, &grants(&[], None), Stand::default(), false);
    let reply: serde_json::Value = serde_json::from_str(&outcome.reply.unwrap()).unwrap();
    assert_eq!(reply, serde_json::json!({ "now": "2026-11-18T10:00:00Z", "unix": 1_794_996_000 }));
}

/// E1: randomness for every extension (a sign-in's `state`, PKCE), never the
/// same twice, 1 to 64 bytes.
#[test]
fn an_extension_is_given_randomness() {
    let asks = [r#"{"bytes":32}"#, r#"{}"#, r#"{"bytes":65}"#];
    let body = "(local.set $n (call $random (i32.const $at_0) (i32.const $len_0)))
    (call $answer (i32.const 49152))
    (drop (call $random (i32.const $at_1) (i32.const $len_1)))
    (call $answer (i32.const 50176))
    (drop (call $random (i32.const $at_2) (i32.const $len_2)))
    (call $answer (i32.const 51200))
    (call $reply (i32.const 49152) (i32.const 3072))
    i32.const 0";
    let file = module(&["random", "answer", "reply"], &asks, body);
    let (outcome, _) = run_it(&file, &grants(&[], None), Stand::default(), false);
    let reply = outcome.reply.unwrap();
    let answers: Vec<serde_json::Value> = reply.split('\0').filter(|a| !a.is_empty()).map(|a| serde_json::from_str(a).unwrap()).collect();
    let (first, second) = (answers[0]["random"].as_str().unwrap(), answers[1]["random"].as_str().unwrap());
    assert_eq!((first.len(), second.len()), (43, 43), "32 bytes each, in URL-safe base64");
    assert_ne!(first, second);
    assert!(first.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_'));
    assert_eq!(answers[2], serde_json::json!({ "refused": "form" }));
}

#[test]
fn it_is_given_only_what_its_manifest_grants() {
    // A web request where no host is named: the import is not given, the module not started.
    let file = module(&["web", "answer", "reply"], &[r#"{"method":"GET","url":"https://api.example.org/"}"#], &echo("web"));
    let (outcome, stand) = run_it(&file, &grants(&[], None), Stand::default(), false);
    assert_eq!(outcome.reply, Err(Stop::AsksMore("sioul.web".into())));
    assert!(stand.asked.is_empty());
    // Anything not of Sioul's.
    let file = module(&["env.clock", "answer", "reply"], &[], "i32.const 0");
    assert_eq!(run_it(&file, &grants(&["api.example.org"], None), Stand::default(), false).0.reply, Err(Stop::AsksMore("env.clock".into())));
    // The secret, without a sign-in in the manifest.
    let file = module(&["secret", "answer", "reply"], &[r#"{"get":true}"#], &echo("secret"));
    assert_eq!(run_it(&file, &grants(&["api.example.org"], None), Stand::default(), false).0.reply, Err(Stop::AsksMore("sioul.secret".into())));
}

#[test]
fn a_web_request_reaches_only_the_hosts_named() {
    let named = grants(&["api.example.org"], None);
    let ask = |url: &str| module(&["web", "answer", "reply"], &[&format!(r#"{{"method":"GET","url":"{url}","headers":[["Accept","text/plain"]]}}"#)], &echo("web"));
    let (outcome, stand) = run_it(&ask("https://API.example.org/a?b=c"), &named, Stand::default(), false);
    let reply: serde_json::Value = serde_json::from_str(&outcome.reply.unwrap()).unwrap();
    assert_eq!((reply["status"].as_u64(), reply["headers"][0][0].as_str()), (Some(200), Some("content-type")));
    assert_eq!(stand.asked[0].host, "api.example.org");
    // W3: Sioul's line holds the host and the status, never the path nor the query.
    assert!(outcome.log.iter().any(|l| l.contains("sioul: GET api.example.org: 200")) && !outcome.log.iter().any(|l| l.contains("/a")), "{:?}", outcome.log);
    for (url, why) in [
        ("https://evil.example.net/", "host"),
        ("https://sub.api.example.org/", "host"),
        ("http://api.example.org/", "scheme"),
        ("https://api.example.org:8443/", "address"),
        ("https://user@api.example.org/", "address"),
        ("file:///etc/passwd", "scheme"),
    ] {
        let (outcome, stand) = run_it(&ask(url), &named, Stand::default(), false);
        assert_eq!(outcome.reply.unwrap(), format!(r#"{{"refused":"{why}"}}"#), "{url}");
        assert!(stand.asked.is_empty(), "{url}");
    }
    // A header Sioul sets, a body over 256 KiB, a method other than GET and POST.
    for ask in [r#"{"method":"GET","url":"https://api.example.org/","headers":[["Host","evil.example.net"]]}"#.to_string(), format!(r#"{{"method":"POST","url":"https://api.example.org/","body":"{}"}}"#, "x".repeat(300_000)), r#"{"method":"DELETE","url":"https://api.example.org/"}"#.to_string()] {
        let file = module(&["web", "answer", "reply"], &[&ask], &echo("web"));
        let (outcome, stand) = run_it(&file, &named, Stand::default(), false);
        assert!(outcome.reply.unwrap().starts_with(r#"{"refused""#));
        assert!(stand.asked.is_empty());
    }
}

#[test]
fn a_sign_in_is_shown_only_on_a_sign_in_call_and_hidden_from_the_log() {
    let given = grants(&["sso.example.org"], Some("example-app://cb"));
    let imports = ["sign_in", "log", "answer", "reply"];
    let asks = [r#"{"open":"https://sso.example.org/authorize?x=1","watch":"example-app://cb"}"#, r#"{"line":"got the code OC-secret-1234 and the rest"}"#];
    let body = "(local.set $n (call $sign_in (i32.const $at_0) (i32.const $len_0)))
    (call $answer (i32.const 49152))
    (drop (call $log (i32.const $at_1) (i32.const $len_1)))
    (call $reply (i32.const 49152) (local.get $n))
    i32.const 0";
    let file = module(&imports, &asks, body);
    let stand = Stand { returned: Some("example-app://cb?code=OC-secret-1234&state=s".into()), ..Stand::default() };
    // K4: a fragment's values too, with no query before it.
    {
        let fragment = module(&imports, &[asks[0], r#"{"line":"token AT-fragment-5678 seen"}"#], body);
        let stand = Stand { returned: Some("example-app://cb#access_token=AT-fragment-5678&state=s".into()), ..Stand::default() };
        let (outcome, _) = engine::call(&fragment, &given, &call_text(Call::SignIn(None), false), stand, &small(), true);
        assert!(outcome.log.iter().any(|l| l.contains("token [hidden] seen")), "{:?}", outcome.log);
    }
    // In a refresh: refused, nothing shown.
    let (outcome, stand) = run_it(&file, &given, stand, false);
    assert_eq!(outcome.reply.unwrap(), r#"{"refused":"not-now"}"#);
    assert!(stand.pages.is_empty());
    // In a sign-in: the page shown with its site's name, the return given back, the code hidden from the log.
    let (outcome, stand) = engine::call(&file, &given, &call_text(Call::SignIn(None), false), stand, &small(), true);
    assert_eq!(outcome.reply.unwrap(), r#"{"returned":"example-app://cb?code=OC-secret-1234&state=s"}"#);
    assert_eq!(stand.pages, vec!["sso.example.org https://sso.example.org/authorize?x=1".to_string()]);
    let log = outcome.log.join("\n");
    assert!(log.contains("got the code [hidden]") && !log.contains("OC-secret-1234"), "{log}");
    // D3: an address that begins alike but names another host is no return: the page counts as closed.
    let stand = Stand { returned: Some("example-app://cb.other.example/?code=OC-other".into()), ..Stand::default() };
    let (outcome, _) = engine::call(&file, &given, &call_text(Call::SignIn(None), false), stand, &small(), true);
    assert_eq!(outcome.reply.unwrap(), r#"{"cancelled":true}"#);
    // A page on a host not named, or a return not the one declared: refused.
    for ask in [r#"{"open":"https://evil.example.net/","watch":"example-app://cb"}"#, r#"{"open":"https://sso.example.org/","watch":"other-app://cb"}"#] {
        let file = module(&["sign_in", "answer", "reply"], &[ask], &echo("sign_in"));
        let (outcome, _) = engine::call(&file, &given, &call_text(Call::SignIn(None), false), Stand::default(), &small(), true);
        assert!(outcome.reply.unwrap().starts_with(r#"{"refused""#), "{ask}");
    }
}

#[test]
fn the_secret_kept_never_reaches_the_log() {
    let given = grants(&["sso.example.org"], Some("example-app://cb"));
    let asks = [r#"{"keep":"{\"refresh\":\"RT-kept-0000\"}"}"#, r#"{"line":"renewed with RT-kept-0000"}"#];
    let body = "(local.set $n (call $secret (i32.const $at_0) (i32.const $len_0)))
    (call $answer (i32.const 49152))
    (drop (call $log (i32.const $at_1) (i32.const $len_1)))
    (call $reply (i32.const 49152) (local.get $n))
    i32.const 0";
    let file = module(&["secret", "log", "answer", "reply"], &asks, body);
    let (outcome, stand) = run_it(&file, &given, Stand::default(), false);
    assert_eq!(outcome.reply.unwrap(), r#"{"kept":true}"#);
    assert_eq!(stand.secret.as_deref(), Some(r#"{"refresh":"RT-kept-0000"}"#));
    assert!(outcome.log.iter().any(|l| l.ends_with("renewed with [hidden]")), "{:?}", outcome.log);
    // K3: too long for every system's keyring: refused whole, never cut, and said.
    let long = format!(r#"{{"keep":"{}"}}"#, "k".repeat(2401));
    let file = module(&["secret", "answer", "reply"], &[&long], &echo("secret"));
    let (outcome, stand) = run_it(&file, &given, Stand::default(), false);
    assert_eq!(outcome.reply.unwrap(), r#"{"refused":"size"}"#);
    assert_eq!(stand.secret, None);
    assert!(outcome.log.iter().any(|l| l.contains("refused a secret of 2401 bytes")), "{:?}", outcome.log);
}

/// K1: an extension removed while a call runs (a refresh renewing its
/// token): the call ends at its next ask, and the secret is never written
/// back after Remove forgot it.
#[test]
fn a_call_removed_meanwhile_keeps_nothing() {
    let given = grants(&["sso.example.org"], Some("example-app://cb"));
    let asks = [r#"{"keep":"RT-first-0000"}"#, r#"{"keep":"RT-renewed-0000"}"#];
    let body = "(drop (call $secret (i32.const $at_0) (i32.const $len_0)))
    (drop (call $secret (i32.const $at_1) (i32.const $len_1)))
    (call $reply (i32.const 0) (i32.const 2))
    i32.const 0";
    let file = module(&["secret", "answer", "reply"], &asks, body);
    let (outcome, stand) = run_it(&file, &given, Stand { removed_after: Some(1), ..Stand::default() }, false);
    assert_eq!(outcome.reply, Err(Stop::Removed));
    assert_eq!(stand.secret.as_deref(), Some("RT-first-0000"), "the renewed token is never kept");
    assert!(outcome.log.iter().any(|l| l.contains("stopped: it was removed meanwhile")), "{:?}", outcome.log);
    // A module that only computes ends at its next slice.
    let looping = module(&["answer", "reply"], &[], "(loop $forever (br $forever)) i32.const 0");
    let (outcome, _) = run_it(&looping, &grants(&[], None), Stand { removed_after: Some(0), ..Stand::default() }, false);
    assert_eq!(outcome.reply, Err(Stop::Removed));
}

#[test]
fn a_module_that_loops_grows_traps_or_says_nonsense_ends_calmly() {
    let none = grants(&[], None);
    let looping = module(&["answer", "reply"], &[], "(loop $forever (br $forever)) i32.const 0");
    let started = std::time::Instant::now();
    assert_eq!(run_it(&looping, &none, Stand::default(), false).0.reply, Err(Stop::TooLong));
    assert!(started.elapsed() < Duration::from_secs(60));
    // Its time, rather than its steps.
    let short = Limits { time: Duration::from_millis(200), steps: u64::MAX, ..small() };
    assert_eq!(engine::call(&looping, &none, "{}", Stand::default(), &short, false).0.reply, Err(Stop::TooLong));
    let growing = module(&["answer", "reply"], &[], "(loop $more (br_if $more (i32.ne (memory.grow (i32.const 16)) (i32.const -1)))) i32.const 0");
    assert_eq!(run_it(&growing, &none, Stand::default(), false).0.reply, Err(Stop::Memory));
    let trapping = module(&["answer", "reply"], &[], "unreachable");
    assert!(matches!(run_it(&trapping, &none, Stand::default(), false).0.reply, Err(Stop::Trapped(_))));
    let silent = module(&["answer", "reply"], &[], "i32.const 0");
    assert!(matches!(run_it(&silent, &none, Stand::default(), false).0.reply, Err(Stop::Nonsense(_))));
    let outside = module(&["now", "answer", "reply"], &["{}"], "(drop (call $now (i32.const 1000000) (i32.const 10))) i32.const 0");
    assert!(matches!(run_it(&outside, &none, Stand::default(), false).0.reply, Err(Stop::Trapped(_))));
    let twice = module(&["answer", "reply"], &[], "(call $reply (i32.const 0) (i32.const 1)) (call $reply (i32.const 0) (i32.const 1)) i32.const 0");
    assert!(matches!(run_it(&twice, &none, Stand::default(), false).0.reply, Err(Stop::Nonsense(_))));
    assert!(matches!(run_it(b"not wasm", &none, Stand::default(), false).0.reply, Err(Stop::NotRead(_))));
    let other = wat::parse_str("(module (memory (export \"memory\") 1) (func (export \"sioul_interface\") (result i32) i32.const 2) (func (export \"sioul_alloc\") (param i32) (result i32) i32.const 64) (func (export \"sioul_call\") (param i32 i32) (result i32) i32.const 0))").unwrap();
    assert_eq!(run_it(&other, &none, Stand::default(), false).0.reply, Err(Stop::Interface(2)));
    // Each end said in the log, in one line of Sioul's.
    let (outcome, _) = run_it(&trapping, &none, Stand::default(), false);
    assert!(outcome.log.iter().any(|l| l.contains("sioul: stopped: it ended in an error of its own")), "{:?}", outcome.log);
}

#[test]
fn a_file_runs_only_with_the_hash_the_manifest_signed() {
    let folder = std::env::temp_dir().join(format!("sioul-extensions-store-{}", std::process::id()));
    std::fs::create_dir_all(&folder).unwrap();
    let file = module(&["now", "answer", "reply"], &["{}"], &echo("now"));
    std::fs::write(folder.join("timetable-0.1.0.wasm"), &file).unwrap();
    let manifest_text = format!(
        "format = 1\nserial = 3\n\n[[extension]]\nid = \"timetable\"\nversion = \"0.1.0\"\ninterface = 1\nname = {{ en = \"A timetable\", fr = \"Un emploi du temps\" }}\nabout = {{ en = \"Lessons.\", fr = \"Des cours.\" }}\nbrings = [\"agenda\"]\nhosts = []\nfile = \"timetable-0.1.0.wasm\"\nsha256 = \"{}\"\n",
        sha256_hex(&file)
    );
    let manifest_path = folder.join("manifest.toml");
    std::fs::write(&manifest_path, &manifest_text).unwrap();
    let now = SystemTime::UNIX_EPOCH + Duration::from_secs(1_794_996_000);
    let address = manifest_path.display().to_string();
    let tests_key = &signature::TEST_SIGNER;
    // Unsigned, or signed by another key: refused.
    assert!(read_manifest_with(&address, now, tests_key).is_err());
    let other = signature::test_signature_by(&[9; 32], manifest_text.as_bytes(), sequoia_openpgp::types::SignatureType::Binary, now - Duration::from_secs(60));
    std::fs::write(folder.join("manifest.toml.sig"), &other).unwrap();
    assert!(read_manifest_with(&address, now, tests_key).unwrap_err().contains("signed by another key"));
    // Signed with the tests' key: read, from a folder or a file:// address alike.
    let signed = signature::test_signature(manifest_text.as_bytes(), sequoia_openpgp::types::SignatureType::Binary, now - Duration::from_secs(60));
    std::fs::write(folder.join("manifest.toml.sig"), &signed).unwrap();
    // S3: the product's own path trusts the publisher's key alone, in a test build too.
    assert!(read_manifest(&address, now).unwrap_err().contains("signed by another key"));
    let manifest = read_manifest_with(&format!("file://{address}"), now, tests_key).unwrap();
    assert_eq!(manifest.serial, 3);
    let offer = manifest.offers[0].clone();
    // Installed and run (the data folder is the tests' own: cargo-apart.sh's XDG folders).
    let kept = install(&address, &offer).unwrap();
    assert_eq!(std::fs::read(&kept).unwrap(), file);
    let (outcome, _) = run(&offer, Call::Refresh, false, Real::new(&offer.id, None));
    assert!(outcome.reply.unwrap().contains("\"unix\""));
    // A file changed after it was checked is not run.
    std::fs::write(&kept, b"(changed)").unwrap();
    assert!(matches!(run(&offer, Call::Refresh, false, Real::new(&offer.id, None)).0.reply, Err(Stop::NotRead(_))));
    // A file whose hash is not the manifest's is not kept.
    let mut wrong = offer.clone();
    wrong.sha256 = "0".repeat(64);
    assert!(install(&address, &wrong).is_err());
    discard_file(&offer);
    let _ = std::fs::remove_dir_all(&folder);
}

/// X1: a module that stays inside every stated limit (asks of 1 MiB at
/// most, 5,000 asks, 50 lines, its memory) tries to make Sioul hold its
/// log and its hidden values without end: a host of 100,000 letters asked
/// forty times, forty values of 100,000 letters each hidden, a long line.
/// What Sioul holds for the call stays small, and nothing of it reaches the
/// log uncut or unhidden.
#[test]
fn what_sioul_holds_for_a_call_is_bounded_in_bytes() {
    const N: usize = 100_000;
    let far = format!(r#"{{"method":"GET","url":"https://{}.example.org/"}}"#, "a".repeat(N));
    let hidden = format!(r#"{{"values":["{}"]}}"#, "b".repeat(N));
    let line = format!(r#"{{"line":"{}"}}"#, "b".repeat(N));
    let (at_far, at_hidden, at_line, at_reply, answers) = (65_536, 196_608, 327_680, 458_752, 917_504);
    let changed = at_hidden + hidden.find('b').unwrap();
    let text = format!(
        r#"(module
  (import "sioul" "web" (func $web (param i32 i32) (result i32)))
  (import "sioul" "hide" (func $hide (param i32 i32) (result i32)))
  (import "sioul" "log" (func $log (param i32 i32) (result i32)))
  (import "sioul" "answer" (func $answer (param i32)))
  (import "sioul" "reply" (func $reply (param i32 i32)))
  (memory (export "memory") 16)
  (data (i32.const {at_far}) "{far}")
  (data (i32.const {at_hidden}) "{hidden}")
  (data (i32.const {at_line}) "{line}")
  (data (i32.const {at_reply}) "{{}}")
  (func (export "sioul_interface") (result i32) i32.const 1)
  (func (export "sioul_alloc") (param i32) (result i32) i32.const 1024)
  (func (export "sioul_call") (param i32 i32) (result i32) (local $i i32)
    (loop $more
      (drop (call $web (i32.const {at_far}) (i32.const {far_len})))
      (call $answer (i32.const {answers}))
      (i32.store8 (i32.const {changed}) (i32.add (i32.const 65) (local.get $i)))
      (drop (call $hide (i32.const {at_hidden}) (i32.const {hidden_len})))
      (local.set $i (i32.add (local.get $i) (i32.const 1)))
      (br_if $more (i32.lt_u (local.get $i) (i32.const 40))))
    (drop (call $log (i32.const {at_line}) (i32.const {line_len})))
    (call $reply (i32.const {at_reply}) (i32.const 2))
    i32.const 0))"#,
        far = data(&far),
        hidden = data(&hidden),
        line = data(&line),
        far_len = far.len(),
        hidden_len = hidden.len(),
        line_len = line.len(),
    );
    let file = wat::parse_str(&text).unwrap();
    let limits = Limits { memory: 2 << 20, ..Limits::default() };
    let (outcome, stand) = engine::call(&file, &grants(&["api.example.org"], None), &call_text(Call::Refresh, false), Stand::default(), &limits, false);
    assert_eq!(outcome.reply.as_deref(), Ok("{}"));
    assert!(stand.asked.is_empty(), "no request left");
    // 40 requests of 100 kB and 40 values of 100 kB: about 8 MB if each were kept.
    assert!(outcome.held < 256 * 1024, "Sioul held {} bytes for one call", outcome.held);
    let log = outcome.log.join("\n");
    assert!(outcome.log.iter().all(|l| l.chars().count() <= 301), "{log}");
    assert!(!log.contains(&"b".repeat(40)), "a line holding what could not be hidden: {log}");
    let refused: Vec<&String> = outcome.log.iter().filter(|l| l.contains("does not name")).collect();
    assert_eq!(refused.len(), 1, "a refused host said once a call: {log}");
    assert!(refused[0].contains("40") && !refused[0].contains(&"a".repeat(100)), "with its count, its name cut short: {log}");
}

#[test]
fn calls_are_written_as_the_interface_says() {
    assert_eq!(call_text(Call::Refresh, true), r#"{"call":"refresh","interface":1,"lang":"fr"}"#);
    assert_eq!(call_text(Call::SignIn(Some("example")), false), r#"{"call":"sign-in","choice":"example","interface":1,"lang":"en"}"#);
    assert_eq!(call_text(Call::SignIn(None), false), r#"{"call":"sign-in","choice":null,"interface":1,"lang":"en"}"#);
}

/// The real Skolengo extension's file, built in the extensions' repository
/// (`tools/build.sh`), through this engine against stand-in answers: the
/// interface end to end. Run on demand, with the file's path in
/// `SIOUL_EXTENSIONS_SKOLENGO`: `cargo test -p sioul-sync skolengo_s_file -- --ignored`.
#[test]
#[ignore = "needs the built skolengo.wasm (SIOUL_EXTENSIONS_SKOLENGO)"]
fn skolengo_s_file_through_the_engine() {
    let path = std::env::var("SIOUL_EXTENSIONS_SKOLENGO").expect("SIOUL_EXTENSIONS_SKOLENGO names the built file");
    let file = std::fs::read(path).unwrap();
    // Neither call below makes a request: the hosts are the tests' own.
    let given = grants(&["api.example.org", "sso.example.org"], Some("skoapp-prod://sign-in-callback"));
    // The platforms first, each with its id and its name.
    let (outcome, _) = engine::call(&file, &given, &call_text(Call::SignIn(None), true), Stand::default(), &Limits::default(), true);
    let reply = sioul_core::extensions::reply::sign_in_reply(&outcome.reply.unwrap()).unwrap();
    let sioul_core::extensions::reply::SignInReply::Choose(choices) = reply else { panic!("{reply:?}") };
    assert!(choices.len() > 1 && choices.iter().all(|(id, name)| !id.is_empty() && !name.is_empty()), "{choices:?}");
    // No sign-in kept: a refresh says so, nothing asked.
    let (outcome, stand) = engine::call(&file, &given, &call_text(Call::Refresh, false), Stand::default(), &Limits::default(), false);
    let reply = sioul_core::extensions::reply::refresh_reply(&outcome.reply.unwrap()).unwrap();
    assert_eq!(reply.problem.map(|p| p.kind), Some(sioul_core::extensions::reply::ProblemKind::SignIn));
    assert!(stand.asked.is_empty());
}
