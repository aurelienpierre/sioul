// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

//! `sioul topics seeds` and `sioul topics eval`, run by the real program on
//! a home made in the system's temporary folder (its XDG folders and HOME
//! there): invented topical mail on reserved domains (RFC 2606) kept in the
//! account's Maildir, Sent included, and in the spam filter's corpus for its
//! labels; a project routed by the tax office's domain, which seeds nothing;
//! a contact's card with a category. The person's own files are never read,
//! no server is reached.

use serde_json::Value;
use std::path::{Path, PathBuf};
use std::process::Command;

const FOLDERS: [(&str, &str); 5] = [("XDG_CONFIG_HOME", "config"), ("XDG_DATA_HOME", "data"), ("XDG_STATE_HOME", "state"), ("XDG_CACHE_HOME", "cache"), ("HOME", "home")];

fn home() -> PathBuf {
    let root = std::env::temp_dir().join(format!("sioul-topics-cli-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    for (_, folder) in FOLDERS {
        std::fs::create_dir_all(root.join(folder)).unwrap();
    }
    let notes = root.join("notes");
    std::fs::create_dir_all(&notes).unwrap();
    std::fs::write(notes.join("sioul-projects.toml"), "[[project]]\nid = \"impots-2025\"\ntitle = \"Impôts 2025\"\n\n[[project.route]]\nfrom_domains = [\"impots.example\"]\n").unwrap();
    let config = root.join("config/sioul/config.toml");
    std::fs::create_dir_all(config.parent().unwrap()).unwrap();
    let maildir = root.join("mail/home");
    for sub in ["cur", "new", "tmp"] {
        std::fs::create_dir_all(maildir.join(sub)).unwrap();
    }
    let text = format!(
        "language = \"en\"\ncase_store = \"{}\"\n\n[[account]]\nid = \"home\"\nkind = \"imap\"\naddress = \"owner@example.org\"\nmaildir = \"{}\"\ntrusted_authserv_ids = [\"mx.example.org\"]\n",
        notes.display().to_string().replace('\\', "\\\\"),
        maildir.display().to_string().replace('\\', "\\\\")
    );
    std::fs::write(&config, text).unwrap();
    // One card of the choir's, in the category "Chorale".
    let book = root.join("data/sioul/contacts/local/friends");
    std::fs::create_dir_all(&book).unwrap();
    std::fs::write(book.join("choir.vcf"), "BEGIN:VCARD\r\nVERSION:4.0\r\nUID:choir-1\r\nFN:Choir office\r\nEMAIL:contact@chorale.example\r\nCATEGORIES:Chorale\r\nEND:VCARD\r\n").unwrap();
    let dirs = sioul_learn::Dirs { data: root.join("data/sioul/spam"), state: root.join("state/sioul/spam"), cache: root.join("cache/sioul/spam") };
    let mail = sioul_learn::synthetic::topical(11, 900);
    let records: Vec<_> = mail.iter().enumerate().map(|(i, m)| sioul_learn::synthetic::record_of(m, 1, i as u32 + 1)).collect();
    sioul_learn::corpus::store(&dirs, &records).unwrap();
    let mut kept = mail.clone();
    kept.extend(sioul_learn::synthetic::replies(&mail, 12));
    sioul_learn::synthetic::store_maildir(&maildir, &kept).unwrap();
    root
}

fn sioul(root: &Path, language: &str) -> Command {
    let mut command = Command::new(env!("CARGO_BIN_EXE_sioul"));
    command.arg("--config").arg(root.join("config/sioul/config.toml")).args(["--language", language]);
    for (variable, folder) in FOLDERS {
        command.env(variable, root.join(folder));
    }
    command
}

fn run(root: &Path, language: &str, args: &[&str]) -> String {
    let out = sioul(root, language).args(args).output().unwrap();
    assert!(out.status.success(), "{args:?}: {}", String::from_utf8_lossy(&out.stderr));
    String::from_utf8(out.stdout).unwrap()
}

/// The seeds: the folders, the project joined with its folder, the category
/// with its own, in lines and as JSON, in English and in French.
#[test]
fn seeds_and_eval_on_stand_in_mail() {
    let root = home();
    let data: Value = serde_json::from_str(&run(&root, "en", &["topics", "seeds", "--json"])).unwrap();
    let topics = data["proposal"]["topics"].as_array().unwrap();
    let names: Vec<&str> = topics.iter().map(|t| t["name"].as_str().unwrap()).collect();
    assert_eq!(names.len(), 3, "{data:#}");
    for name in ["Chorale", "Impôts", "Santé"] {
        assert!(names.contains(&name), "{names:?}");
    }
    let kinds = |name: &str| -> Vec<String> { topics.iter().find(|t| t["name"] == name).unwrap()["seeds"].as_array().unwrap().iter().map(|s| s["kind"].as_str().unwrap().to_string()).collect() };
    // The project seeds nothing: a project says nothing of a topic.
    assert_eq!(kinds("Impôts"), vec!["folder"]);
    assert_eq!(kinds("Chorale"), vec!["folder", "category"]);
    assert_eq!(kinds("Santé"), vec!["folder"]);
    assert_eq!(data["left_out"]["small"], 1);
    assert_eq!(data["reading"]["yours"], 12);
    assert_eq!(data["reading"]["category_yours"], 12);
    assert_eq!(data["reading"]["folders_left_out"], 1, "Junk");
    let lines = run(&root, "en", &["topics", "seeds"]);
    assert!(lines.contains("Topics proposed (3):") && lines.contains("12 of them yours (in Sent"), "{lines}");
    let lignes = run(&root, "fr", &["topics", "seeds"]);
    assert!(lignes.contains("Thèmes proposés (3)\u{202f}:") && lignes.contains("le dossier Santé"), "{lignes}");

    // The groups, four ways; without a Model2Vec folder, no k-means.
    let groups: Value = serde_json::from_str(&run(&root, "en", &["topics", "groups", "--json", "--k", "4", "--iterations", "60"])).unwrap();
    let ways = groups["ways"].as_array().unwrap();
    assert_eq!(ways.iter().map(|w| w["name"].as_str().unwrap()).collect::<Vec<_>>(), vec!["senders", "model2vec", "lda", "nmf"], "{groups:#}");
    assert_eq!(ways[1]["skipped"], "no-model");
    assert!(ways[2]["groups"].as_array().unwrap().len() >= 3);
    let lines = run(&root, "fr", &["topics", "groups", "--k", "4", "--iterations", "60"]);
    assert!(lines.contains("lda\u{202f}: ") && lines.contains("par exemple\u{202f}: «\u{202f}"), "{lines}");
    println!("{lines}");

    // The measures, small and quick: every way, the decision, the fold, churn.
    let args = ["topics", "eval", "--json", "--threads", "2", "--dim", "16", "--epochs", "20", "--buckets", "2000,4000", "--none-weights", "0.2,0.5"];
    let eval: Value = serde_json::from_str(&run(&root, "en", &args)).unwrap();
    assert_eq!(eval["ways"].as_array().unwrap().len(), 4, "{eval:#}");
    assert!(eval["decided"]["sure"].as_object().unwrap().len() == 3);
    assert_eq!(eval["fold"]["broken"], false);
    assert!(eval["fold"]["bytes_f16"].as_u64().unwrap() < eval["fold"]["bytes_f32"].as_u64().unwrap());
    let lines = run(&root, "en", &["topics", "eval", "--quick", "--threads", "2", "--dim", "16", "--epochs", "20", "--buckets", "2000"]);
    assert!(lines.contains("Kept: threshold-2000") && lines.contains("The table:"), "{lines}");
    // Shown with --nocapture: what a person reads.
    println!("{lines}");
    println!("{}", run(&root, "fr", &["topics", "eval", "--quick", "--threads", "2", "--dim", "16", "--epochs", "20", "--buckets", "2000"]));

    // Your topics, then the model, learned from the corpus: the first table written here, the mail kept here judged by it.
    let config = root.join("config/sioul/config.toml");
    let mut text = std::fs::read_to_string(&config).unwrap();
    for (id, name, folder) in [("choir", "Chorale", "Chorale"), ("tax", "Impôts", "Impôts"), ("health", "Santé", "Santé"), ("club", "Club de lecture", "Lecture")] {
        text.push_str(&format!("\n[[topic]]\nid = \"{id}\"\nname = \"{name}\"\nseeds = {{ folders = [\"{folder}\"] }}\n"));
    }
    std::fs::write(&config, text).unwrap();
    let trained: Value = serde_json::from_str(&run(&root, "en", &["topics", "train", "--json"])).unwrap();
    assert_eq!((trained["outcome"].as_str(), trained["written"].as_bool()), (Some("first"), Some(true)), "{trained:#}");
    let club = trained["topics"].as_array().unwrap().iter().find(|t| t["id"] == "club").unwrap();
    assert_eq!((club["examples"].as_u64(), club["learned"].as_bool()), (Some(0), Some(false)), "no mail, no class");
    let table = std::fs::read(root.join("data/sioul/topics/table.bin")).unwrap();
    assert_eq!(sioul_core::topics::table::Table::from_bytes(&table).unwrap().meta.topics, ["choir", "tax", "health"]);
    let judged = std::fs::read_dir(root.join("cache/sioul/topics/judged")).unwrap().filter_map(Result::ok).filter(|e| e.path().extension().is_some_and(|x| x == "jsonl")).count();
    assert_eq!(judged, 1, "what this table judged, its own file");
    println!("{}", run(&root, "fr", &["topics", "train", "--no-replace"]));
    let _ = std::fs::remove_dir_all(root);
}
