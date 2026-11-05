// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

//! "What Sioul keeps about…" written as the window writes it (src/persons.rs,
//! cpp/pdfwriter.cpp), with Qt itself: a person's page (made up here, the
//! shape `persons::page` gives) becomes a PDF file; the file is there, is a
//! PDF, and its text names the person and each of their papers, agendas and
//! letters; a word that looks like markup stays a word. The text is read back
//! by Poppler's `pdftotext`, as Sioul reads scans; without it, the file's
//! being there and a PDF is all that is checked, and the test says so.
//!
//! Its own `main` (harness = false): Qt wants its application on the
//! program's first thread. Run it alone, with stand-in folders (the
//! translator reads the configuration): `cargo test --release -p sioul-app --test pdf`.

fn main() {
    sioul_core::config::stand_ins_only();
    // SAFETY: nothing else runs yet: no thread of this program reads the environment.
    unsafe {
        std::env::set_var("QT_QPA_PLATFORM", "offscreen");
    }
    let application = cxx_qt_lib::QGuiApplication::new();
    assert!(!application.is_null(), "Qt's application, offscreen");
    let folder = sioul_core::config::data_dir().join(format!("pdf-test-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&folder);
    let path = folder.join("Wren").join("What Sioul keeps.pdf");
    let shown = serde_json::json!({
        "name": "Wren",
        "line": "Child · 12 · lives with you",
        "papers": [
            { "title": "Quillmere passport", "kind_label": "Passport", "line": "valid until Friday 3 March 2028", "file_name": "Passport.pdf" },
            { "title": "School insurance <b>Thornwick</b>", "kind_label": "Insurance certificate", "line": "", "file_name": "" }
        ],
        "agendas": [{ "name": "Wren, school and activities" }],
        "letters": [{ "sender": "Collège Ambrefell", "kind_label": "A request for documents", "received": "Monday 2 November" }]
    });
    let problem = sioul_app::keeps_pdf(&shown.to_string(), &path);
    assert_eq!(problem, "", "written");
    let bytes = std::fs::read(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
    assert!(bytes.starts_with(b"%PDF-"), "a PDF: {:?}", &bytes[..bytes.len().min(16)]);
    match std::process::Command::new("pdftotext").args(["-layout"]).arg(&path).arg("-").output() {
        Ok(read) if read.status.success() => {
            let text = String::from_utf8_lossy(&read.stdout).split_whitespace().collect::<Vec<_>>().join(" ");
            for wanted in ["Wren", "Quillmere passport", "School insurance <b>Thornwick</b>", "Wren, school and activities", "Collège Ambrefell"] {
                assert!(text.contains(wanted), "the PDF names “{wanted}”: {text}");
            }
            println!("pdf: written, a PDF, naming the person, their papers, agendas and letters ({} bytes)", bytes.len());
        }
        _ => println!("pdf: written and a PDF ({} bytes); its text not read: no pdftotext here", bytes.len()),
    }
    let _ = std::fs::remove_dir_all(&folder);
}
