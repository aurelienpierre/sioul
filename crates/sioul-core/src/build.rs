// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

//! The build behind this Sioul (docs/building.md, "Which build"): its version
//! and the commit it was built from, said in `sioul --version`, in Settings,
//! in this device's entry among your devices and in its records, so that what
//! each device wrote can be traced to the code that wrote it.

/// The version, as `Cargo.toml` says it: "0.0.3".
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// The commit, as the build found it (`build.rs`): twelve characters of its
/// hash ("eff8661abcde"), "-dirty" after them when the tree held changes not
/// committed; `SIOUL_COMMIT`'s value where the repository was not there; "unknown".
pub const COMMIT: &str = env!("SIOUL_BUILD_COMMIT");

/// Both, as said everywhere: "0.0.3 (eff8661abcde)".
pub const DESCRIBED: &str = concat!(env!("CARGO_PKG_VERSION"), " (", env!("SIOUL_BUILD_COMMIT"), ")");

/// A build as an entry or a record says it, in words: "0.0.3 (eff8661abcde)";
/// the version alone when the commit is unsaid (an older Sioul); "" when neither is.
pub fn described(version: &str, commit: &str) -> String {
    match (version.trim(), commit.trim()) {
        ("", _) => String::new(),
        (version, "") => version.to_string(),
        (version, commit) => format!("{version} ({commit})"),
    }
}

/// Whether `version` comes before `than`, number by number ("0.0.2" before
/// "0.0.10"); neither when either does not read as numbers (nothing said).
pub fn older(version: &str, than: &str) -> bool {
    let numbers = |v: &str| -> Option<Vec<u64>> { v.trim().split(['.', '-', '+']).take(3).map(|n| n.parse().ok()).collect() };
    match (numbers(version), numbers(than)) {
        (Some(a), Some(b)) if !a.is_empty() && !b.is_empty() => a < b,
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_build_is_said_and_compared() {
        assert!(!VERSION.is_empty() && !COMMIT.is_empty());
        assert!(COMMIT == "unknown" || COMMIT.trim_end_matches("-dirty").chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '.'), "{COMMIT}");
        assert_eq!(DESCRIBED, format!("{VERSION} ({COMMIT})"));
        assert_eq!(described("0.0.3", "eff8661abcde"), "0.0.3 (eff8661abcde)");
        assert_eq!(described("0.0.2", ""), "0.0.2", "an older Sioul says its version alone");
        assert_eq!(described("", ""), "");
        assert!(older("0.0.2", "0.0.3") && older("0.0.9", "0.0.10") && older("0.9.0", "1.0.0"));
        assert!(!older("0.0.3", "0.0.3") && !older("0.0.4", "0.0.3"));
        assert!(!older("", "0.0.3") && !older("0.0.3", "") && !older("next", "0.0.3"), "what does not read is never said older");
    }
}
