// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

//! What kind of device this is, for what follows the person: a phone, a
//! tablet or a computer (docs/android.md, "Tablets").
//!
//! Sioul asks three questions where it once asked "is it Android?", and this
//! answers the third only:
//!
//! 1. **What can this system do?** Calls, texts, alarms, the background
//!    service: the system itself says (`cfg(target_os = "android")` for its
//!    APIs, Android's own features for telephony).
//! 2. **How much room is there?** The window's size, on every system
//!    (`window.compact` in the window's QML).
//! 3. **Is it carried?** Which device is "the phone" for what follows you
//!    (reminders that come with you, the devices' names): this kind.
//!
//! **Its default**: Android with a smallest screen width under 600 dp is a
//! phone (Android's own line for tablets, `sw600dp`); Android at 600 dp or
//! more is a tablet; every other system is a computer. An Android whose width
//! is not known (Java did not answer) stays a phone, as every Android was.
//! **Its override**: Settings ▸ This device (`device_kind` in the
//! configuration, this device's own, never shared: sioul-sync's
//! `CONFIG_LOCAL`), for a tablet carried as a phone, or a Linux phone.
//!
//! A tablet acts as a computer for what follows the person. Android's
//! background rules still apply to it (the service, its wakes, the battery):
//! they are the system's, not the kind's. Nor do the doses read this kind:
//! they read the system's (`sioul_sync::devices::Entry::kind`).

/// Android's smallest screen width, in density-independent pixels, from
/// which a device is a tablet (Android's `sw600dp` resources).
pub const TABLET_SMALLEST_DP: u32 = 600;

/// A phone, a tablet or a computer: what the person carries, or sits at.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeviceKind {
    Phone,
    Tablet,
    Computer,
}

impl DeviceKind {
    /// Every kind, as Settings ▸ This device offers them.
    pub const ALL: [DeviceKind; 3] = [DeviceKind::Phone, DeviceKind::Tablet, DeviceKind::Computer];

    /// Its word in the configuration, the devices' entries and the window: "phone", "tablet", "computer".
    pub fn word(self) -> &'static str {
        match self {
            DeviceKind::Phone => "phone",
            DeviceKind::Tablet => "tablet",
            DeviceKind::Computer => "computer",
        }
    }

    /// A kind from its word, whatever its case and spaces; none for another word or "".
    pub fn read(word: &str) -> Option<DeviceKind> {
        DeviceKind::ALL.into_iter().find(|kind| kind.word().eq_ignore_ascii_case(word.trim()))
    }

    /// What the system says by itself: Android under `TABLET_SMALLEST_DP`
    /// (or its width unknown) a phone, Android at it or above a tablet,
    /// anything else a computer.
    pub fn guessed(android: bool, smallest_dp: Option<u32>) -> DeviceKind {
        match (android, smallest_dp) {
            (false, _) => DeviceKind::Computer,
            (true, Some(dp)) if dp >= TABLET_SMALLEST_DP => DeviceKind::Tablet,
            (true, _) => DeviceKind::Phone,
        }
    }

    /// This device's kind: your word in Settings ▸ This device (`said`, the
    /// configuration's `device_kind`) when it names one, else the guess.
    pub fn of(said: Option<&str>, android: bool, smallest_dp: Option<u32>) -> DeviceKind {
        said.and_then(DeviceKind::read).unwrap_or_else(|| DeviceKind::guessed(android, smallest_dp))
    }

    /// Whether it is carried, for what follows the person: a phone only. A
    /// tablet stays where a computer would.
    pub fn carried(self) -> bool {
        self == DeviceKind::Phone
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Android's own line decides when nothing is said: a phone under 600 dp
    /// (or unknown), a tablet from 600 dp; every other system a computer,
    /// whatever width it reports.
    #[test]
    fn the_default_follows_android_s_line_for_tablets() {
        assert_eq!(DeviceKind::guessed(true, Some(360)), DeviceKind::Phone);
        assert_eq!(DeviceKind::guessed(true, Some(599)), DeviceKind::Phone);
        assert_eq!(DeviceKind::guessed(true, Some(600)), DeviceKind::Tablet);
        assert_eq!(DeviceKind::guessed(true, Some(800)), DeviceKind::Tablet);
        assert_eq!(DeviceKind::guessed(true, None), DeviceKind::Phone, "Java silent: a phone, as every Android was");
        assert_eq!(DeviceKind::guessed(false, None), DeviceKind::Computer);
        assert_eq!(DeviceKind::guessed(false, Some(360)), DeviceKind::Computer, "a narrow computer is still a computer");
    }

    /// Your word wins over the guess, either way; a word that names no kind
    /// (an empty one: "as this device says") leaves the guess.
    #[test]
    fn your_word_wins_over_the_guess() {
        assert_eq!(DeviceKind::of(Some("phone"), true, Some(800)), DeviceKind::Phone, "a tablet carried as a phone");
        assert_eq!(DeviceKind::of(Some("tablet"), true, Some(360)), DeviceKind::Tablet);
        assert_eq!(DeviceKind::of(Some("computer"), true, Some(800)), DeviceKind::Computer);
        assert_eq!(DeviceKind::of(Some(" Phone "), false, None), DeviceKind::Phone, "a Linux phone");
        assert_eq!(DeviceKind::of(Some(""), true, Some(800)), DeviceKind::Tablet);
        assert_eq!(DeviceKind::of(Some("watch"), false, None), DeviceKind::Computer);
        assert_eq!(DeviceKind::of(None, true, Some(400)), DeviceKind::Phone);
        for kind in DeviceKind::ALL {
            assert_eq!(DeviceKind::read(kind.word()), Some(kind));
        }
        assert!(DeviceKind::Phone.carried() && !DeviceKind::Tablet.carried() && !DeviceKind::Computer.carried(), "a tablet stays where a computer would");
    }

    /// The configuration keeps it as `device_kind`, read back as written; an
    /// older file has none.
    #[test]
    fn the_configuration_keeps_your_word() {
        let config: crate::config::Config = toml::from_str("device_kind = \"tablet\"\n").unwrap();
        assert_eq!(config.device_kind(), Some(DeviceKind::Tablet));
        let older: crate::config::Config = toml::from_str("theme = \"dark\"\n").unwrap();
        assert_eq!(older.device_kind(), None);
        // Settings ▸ This device writes it, and "" (found by itself) takes it out.
        let dir = std::env::temp_dir().join(format!("sioul-device-kind-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("config.toml");
        std::fs::write(&path, "theme = \"dark\"\n").unwrap();
        crate::settings::change(&path, "device_kind", None, &crate::config::SettingValue::Text("tablet".into())).unwrap();
        assert_eq!(crate::config::Config::load(&path).unwrap().device_kind(), Some(DeviceKind::Tablet));
        crate::settings::change(&path, "device_kind", None, &crate::config::SettingValue::Text(String::new())).unwrap();
        let text = std::fs::read_to_string(&path).unwrap();
        assert!(!text.contains("device_kind") && text.contains("theme"), "{text}");
        // Its row: the guess named first, then the three kinds.
        let tr = crate::i18n::Translator::new("en");
        let rows = crate::settings::for_view("parameters", &crate::config::Config::default(), &tr, &[], None);
        let row = rows.iter().find(|r| r.key == "device_kind").unwrap();
        let values: Vec<String> = row.choices.iter().map(|c| match &c.value { crate::config::SettingValue::Text(t) => t.clone(), _ => "?".into() }).collect();
        assert_eq!(values, ["", "phone", "tablet", "computer"]);
        assert_eq!(row.choices[0].label, "Found by itself (a computer)");
        assert!(!row.label.starts_with("set-") && !row.help.starts_with("set-"));
        let _ = std::fs::remove_dir_all(&dir);
    }
}
