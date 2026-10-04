// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

//! The desktop's icon theme. Plasma, GNOME and others name one and say where
//! icons live; elsewhere (a bare session, the offscreen platform, Windows,
//! macOS) Qt knows of neither, and icons would be blank. The Breeze icons
//! Sioul uses ship inside it (`tools/bundle-icons.py`), light and dark, and
//! stand in for whatever the desktop lacks.

#[cxx_qt::bridge]
pub mod ffi {
    unsafe extern "C++" {
        include!("cxx-qt-lib/qstring.h");
        type QString = cxx_qt_lib::QString;

        include!("cxx-qt-lib/qstringlist.h");
        type QStringList = cxx_qt_lib::QStringList;

        include!(<QtGui/QIcon>);
        type QIcon;

        #[Self = "QIcon"]
        #[rust_name = "theme_name"]
        fn themeName() -> QString;

        #[Self = "QIcon"]
        #[rust_name = "set_theme_name"]
        fn setThemeName(name: &QString);

        #[Self = "QIcon"]
        #[rust_name = "set_fallback_theme_name"]
        fn setFallbackThemeName(name: &QString);

        #[Self = "QIcon"]
        #[rust_name = "theme_search_paths"]
        fn themeSearchPaths() -> QStringList;

        #[Self = "QIcon"]
        #[rust_name = "set_theme_search_paths"]
        fn setThemeSearchPaths(paths: &QStringList);
    }
}

use cxx_qt_lib::{QList, QString, QStringList};
use std::path::PathBuf;

/// The bundled icons stand in for those the desktop's theme lacks, and for the
/// theme itself when the desktop names none; the folders of the XDG base
/// directory specification are searched too.
pub fn icons() {
    // Colours chosen in Sioul (or by SIOUL_THEME) take the bundled icons drawn for them.
    let chosen = std::env::var("SIOUL_THEME").ok().filter(|t| !t.is_empty()).or_else(|| crate::backend::load_config().theme).filter(|t| t == "dark" || t == "light");
    let bundled = QString::from(if chosen.as_ref().map_or_else(dark, |t| t == "dark") { "sioul-dark" } else { "sioul" });
    let mut paths: QList<QString> = QList::default();
    for path in ffi::QIcon::theme_search_paths().iter() {
        paths.append(path.clone());
    }
    paths.append(QString::from(":/icons"));
    for folder in xdg_icon_folders() {
        paths.append(QString::from(&folder.display().to_string()));
    }
    ffi::QIcon::set_theme_search_paths(&QStringList::from(&paths));
    ffi::QIcon::set_fallback_theme_name(&bundled);
    if ffi::QIcon::theme_name().is_empty() || chosen.is_some() {
        ffi::QIcon::set_theme_name(&bundled);
    }
}

/// Whether the system shows its windows dark, where Qt has no icon theme to follow it.
fn dark() -> bool {
    #[cfg(target_os = "macos")]
    {
        std::process::Command::new("defaults").args(["read", "-g", "AppleInterfaceStyle"]).output().is_ok_and(|o| String::from_utf8_lossy(&o.stdout).trim() == "Dark")
    }
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        // HKCU\Software\Microsoft\Windows\CurrentVersion\Themes\Personalize, AppsUseLightTheme = 0;
        // asked without a console window flashing up (CREATE_NO_WINDOW).
        std::process::Command::new("reg")
            .args(["query", r"HKCU\Software\Microsoft\Windows\CurrentVersion\Themes\Personalize", "/v", "AppsUseLightTheme"])
            .creation_flags(0x0800_0000)
            .output()
            .is_ok_and(|o| String::from_utf8_lossy(&o.stdout).contains("0x0"))
    }
    #[cfg(not(any(windows, target_os = "macos")))]
    {
        false
    }
}

/// `$XDG_DATA_HOME/icons`, then `<dir>/icons` for each of `$XDG_DATA_DIRS`.
fn xdg_icon_folders() -> Vec<PathBuf> {
    let home = std::env::var_os("XDG_DATA_HOME")
        .filter(|v| !v.is_empty())
        .map_or_else(|| sioul_core::config::expand_home("~/.local/share"), PathBuf::from);
    let dirs = std::env::var_os("XDG_DATA_DIRS").filter(|v| !v.is_empty()).unwrap_or_else(|| "/usr/local/share:/usr/share".into());
    // ":" between folders, ";" on Windows, where ":" follows a drive letter.
    std::iter::once(home).chain(std::env::split_paths(&dirs)).map(|d| d.join("icons")).filter(|d| d.is_dir()).collect()
}
