// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

//! Reading a scan's text (docs/porch.md, "Paper letters"): a PDF's own text
//! when it has one (Poppler's `pdftotext`), else its pages as images
//! (`pdftoppm`) read by Tesseract, as photos and scanned images are. Both are
//! the system's own programs, as the antivirus is: optional, said when missing,
//! with the command that installs them. Nothing leaves this computer.

use std::path::{Path, PathBuf};

/// Why a scan could not be read.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Unread {
    /// A program is missing; how to install it.
    Missing(String),
    Failed(String),
}

/// A program, as the system has it: on its path, else where Homebrew puts it
/// (a macOS application does not get the shell's path).
fn program(name: &str) -> PathBuf {
    ["/opt/homebrew/bin", "/usr/local/bin"].iter().map(|dir| Path::new(dir).join(name)).find(|p| p.is_file()).filter(|_| cfg!(target_os = "macos")).unwrap_or_else(|| PathBuf::from(name))
}

fn runs(name: &str) -> bool {
    crate::command(program(name)).arg("--version").output().is_ok()
}

/// How to install what reads scans here, in one line, in your language:
/// Tesseract, its models of the languages `wanted` ("fra", English being
/// in every Tesseract), and Poppler.
pub fn install_hint(wanted: &[String]) -> String {
    // SUSE names its models by the language's English name.
    #[cfg(all(unix, not(target_os = "macos")))]
    let models = |prefix: &str| {
        let names: Vec<String> = wanted.iter().map(|l| l.trim()).filter(|l| !l.is_empty() && *l != "eng").map(|l| match prefix {
            "tesseract-ocr-traineddata-" => format!("{prefix}{}", match l { "fra" => "french", "deu" => "german", "spa" => "spanish", "ita" => "italian", other => other }),
            _ => format!("{prefix}{l}"),
        }).collect();
        if names.is_empty() { String::new() } else { format!(" {}", names.join(" ")) }
    };
    #[cfg(not(all(unix, not(target_os = "macos"))))]
    let _ = wanted;
    #[cfg(windows)]
    {
        crate::translator().text("ocr-hint-windows", None)
    }
    #[cfg(target_os = "macos")]
    {
        "brew install tesseract tesseract-lang poppler".to_string()
    }
    #[cfg(all(unix, not(target_os = "macos")))]
    {
        let release = std::fs::read_to_string("/etc/os-release").unwrap_or_default().to_ascii_lowercase();
        let like = |name: &str| release.lines().any(|l| (l.starts_with("id=") || l.starts_with("id_like=")) && l.contains(name));
        if like("fedora") || like("rhel") {
            format!("sudo dnf install tesseract{} poppler-utils", models("tesseract-langpack-"))
        } else if like("debian") || like("ubuntu") {
            format!("sudo apt install tesseract-ocr{} poppler-utils", models("tesseract-ocr-"))
        } else if like("arch") {
            format!("sudo pacman -S tesseract{} poppler", models("tesseract-data-"))
        } else if like("suse") {
            format!("sudo zypper install tesseract-ocr{} poppler-tools", models("tesseract-ocr-traineddata-"))
        } else {
            crate::translator().text("ocr-hint-packages", None)
        }
    }
}

/// The languages Tesseract has among those wanted ("fra+eng": the models of
/// the languages you read, `words::OcrWords::tesseract`), else English.
fn languages(wanted: &[String]) -> String {
    let out = crate::command(program("tesseract")).arg("--list-langs").output().map(|o| String::from_utf8_lossy(&o.stdout).to_string()).unwrap_or_default();
    let have: Vec<&str> = out.lines().skip(1).map(str::trim).collect();
    let wanted: Vec<&str> = wanted.iter().map(|l| l.trim()).filter(|l| have.contains(l)).collect();
    if wanted.is_empty() { "eng".into() } else { wanted.join("+") }
}

fn tesseract(image: &Path, wanted: &[String]) -> Result<String, Unread> {
    if !runs("tesseract") {
        return Err(Unread::Missing(install_hint(wanted)));
    }
    let out = crate::command(program("tesseract")).arg(image).arg("stdout").args(["-l", &languages(wanted)]).output().map_err(|e| Unread::Failed(e.to_string()))?;
    if !out.status.success() {
        return Err(Unread::Failed(String::from_utf8_lossy(&out.stderr).lines().last().unwrap_or("").to_string()));
    }
    Ok(String::from_utf8_lossy(&out.stdout).to_string())
}

/// A scan's text: a PDF's own, else read from its pages; an image's, read.
/// `work` is a folder for the pages while they are read (emptied after);
/// `wanted`, Tesseract's models to read them with ("fra", "eng").
pub fn text_of(file: &Path, work: &Path, wanted: &[String]) -> Result<String, Unread> {
    let extension = file.extension().map(|e| e.to_string_lossy().to_ascii_lowercase()).unwrap_or_default();
    if extension != "pdf" {
        return tesseract(file, wanted);
    }
    if !runs("pdftotext") {
        return Err(Unread::Missing(install_hint(wanted)));
    }
    let own = crate::command(program("pdftotext")).args(["-layout"]).arg(file).arg("-").output().map_err(|e| Unread::Failed(e.to_string()))?;
    let text = String::from_utf8_lossy(&own.stdout).to_string();
    if text.chars().filter(|c| c.is_alphanumeric()).count() > 80 {
        return Ok(text);
    }
    // A scanned PDF: its pages as images, at 300 dpi, read one by one. The
    // pictures of your letter go afterwards, whether they could be read or not.
    let _ = std::fs::remove_dir_all(work);
    std::fs::create_dir_all(work).map_err(|e| Unread::Failed(e.to_string()))?;
    let read = pages_read(file, work, wanted);
    let _ = std::fs::remove_dir_all(work);
    read
}

/// A PDF's pages made images in `work`, then read in their order.
fn pages_read(file: &Path, work: &Path, wanted: &[String]) -> Result<String, Unread> {
    let made = crate::command(program("pdftoppm")).args(["-r", "300", "-png"]).arg(file).arg(work.join("page")).output().map_err(|e| Unread::Failed(e.to_string()))?;
    if !made.status.success() {
        return Err(Unread::Failed(String::from_utf8_lossy(&made.stderr).lines().last().unwrap_or("").to_string()));
    }
    let mut pages: Vec<PathBuf> = std::fs::read_dir(work).into_iter().flatten().filter_map(Result::ok).map(|e| e.path()).filter(|p| p.extension().is_some_and(|e| e == "png")).collect();
    pages.sort();
    let mut out = String::new();
    for page in pages {
        out.push_str(&tesseract(&page, wanted)?);
        out.push('\n');
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A French letter written as an image, read back. Needs Tesseract with
    /// French and Python's Pillow, which make no part of Sioul: run by hand.
    #[test]
    #[ignore]
    fn a_letter_as_an_image() {
        let dir = std::env::temp_dir().join(format!("sioul-ocr-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let image = dir.join("letter.png");
        let script = format!(
            "from PIL import Image, ImageDraw, ImageFont\nimport glob\nfont = ImageFont.truetype(sorted(glob.glob('/usr/share/fonts/**/DejaVuSans.ttf', recursive=True))[0], 34)\nim = Image.new('L', (1700, 900), 255)\nd = ImageDraw.Draw(im)\nlines = ['CAISSE D ALLOCATIONS FAMILIALES', 'Exempleville, le 28 septembre 2026', 'Objet : notification de décision', 'Voies et délais de recours : vous pouvez contester', 'cette décision dans un délai de deux mois.']\nfor i, l in enumerate(lines):\n    d.text((80, 80 + i * 70), l, fill=0, font=font)\nim.save('{}')\n",
            image.display()
        );
        let made = std::process::Command::new("python3").arg("-c").arg(script).status().unwrap();
        assert!(made.success());
        let words = sioul_core::words::Words::builtin();
        let text = text_of(&image, &dir.join("pages"), &words.ocr.tesseract).unwrap();
        let reading = sioul_core::letters::read(&words, &text, "2026-10-02".parse().unwrap());
        assert_eq!(reading.sender, "CAF", "{text}");
        assert_eq!(reading.deadline, Some("2026-12-02".parse().unwrap()), "{text}");
        let _ = std::fs::remove_dir_all(&dir);
    }
}
