// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

//! The desktop's icon theme. Plasma, GNOME and others name one and say where
//! icons live; elsewhere (a bare session, the offscreen platform, Windows,
//! macOS) Qt knows of neither, and icons would be blank. The Breeze icons
//! Sioul uses ship inside it (`tools/bundle-icons.py`), light and dark, and
//! stand in for whatever the desktop lacks.
//!
//! And where the desktop puts a window's buttons, for the title bar Sioul
//! draws itself on a computer (qml/TitleBar.qml): KDE Plasma says it in
//! kwinrc, GNOME and its kin in their `button-layout` setting, Xfce in
//! xfwm4's; macOS puts them on the left, Windows on the right.

#[cxx_qt::bridge]
pub mod ffi {
    #[auto_cxx_name]
    extern "RustQt" {
        /// What the desktop says of windows, to QML.
        #[qobject]
        #[qml_element]
        type Desktop = super::DesktopRust;

        /// The window's buttons on each side of its title bar, in the
        /// system's order, as JSON: `{"left": [...], "right": [...]}`, each
        /// "minimize", "maximize" or "close". Read once, as Sioul starts.
        #[qinvokable]
        fn window_buttons(self: &Desktop) -> QString;
    }

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
    // Each folder once: Qt looks for every icon in each, and has ":/icons" already.
    let mut paths: QList<QString> = QList::default();
    let qt = ffi::QIcon::theme_search_paths();
    let ours = std::iter::once(":/icons".to_owned()).chain(xdg_icon_folders().into_iter().map(|folder| folder.display().to_string()));
    for path in qt.iter().cloned().chain(ours.map(|path| QString::from(&path))) {
        if !paths.contains(&path) {
            paths.append(path);
        }
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

/// The object behind QML's `Desktop`: nothing of its own.
#[derive(Default)]
pub struct DesktopRust;

impl ffi::Desktop {
    fn window_buttons(&self) -> QString {
        QString::from(&window_buttons().json())
    }
}

/// One of the window's buttons that Sioul's title bar draws.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WindowButton {
    Minimize,
    Maximize,
    Close,
}

impl WindowButton {
    /// Its name, as QML reads it and GNOME writes it.
    fn id(self) -> &'static str {
        match self {
            WindowButton::Minimize => "minimize",
            WindowButton::Maximize => "maximize",
            WindowButton::Close => "close",
        }
    }
}

/// The window's buttons on the left of its title bar and on its right, each
/// side in its order.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ButtonLayout {
    pub left: Vec<WindowButton>,
    pub right: Vec<WindowButton>,
}

impl ButtonLayout {
    /// Minimize, maximize and close on the right: Windows's, KDE Plasma's as
    /// it comes (its help button aside), and Sioul's where the system says nothing.
    fn on_the_right() -> Self {
        ButtonLayout { left: Vec::new(), right: vec![WindowButton::Minimize, WindowButton::Maximize, WindowButton::Close] }
    }

    /// Each button once, where it first comes.
    fn once(self) -> Self {
        let mut seen = Vec::new();
        let mut keep = |side: Vec<WindowButton>| -> Vec<WindowButton> {
            side.into_iter()
                .filter(|button| {
                    let new = !seen.contains(button);
                    seen.push(*button);
                    new
                })
                .collect()
        };
        let left = keep(self.left);
        let right = keep(self.right);
        ButtonLayout { left, right }
    }

    /// As QML reads it: `{"left": ["close", …], "right": […]}`.
    fn json(&self) -> String {
        let ids = |side: &[WindowButton]| side.iter().map(|button| button.id()).collect::<Vec<_>>();
        serde_json::json!({ "left": ids(&self.left), "right": ids(&self.right) }).to_string()
    }
}

/// Where the system puts the window's buttons, read once: on a thread of its
/// own as Sioul starts (`warm_up`), GNOME's setting being a program to ask.
pub fn window_buttons() -> &'static ButtonLayout {
    static LAYOUT: std::sync::OnceLock<ButtonLayout> = std::sync::OnceLock::new();
    LAYOUT.get_or_init(|| system_layout().once())
}

/// Starts reading where the system puts the window's buttons, for the title
/// bar to find it read when it is made. A phone has no title bar.
#[cfg(not(target_os = "android"))]
pub fn warm_up() {
    std::thread::spawn(|| {
        window_buttons();
    });
}

/// What the system says. SIOUL_BUTTON_LAYOUT, written as GNOME writes it
/// ("close,minimize,maximize:" puts them on the left), tries a layout the
/// desktop does not have.
fn system_layout() -> ButtonLayout {
    use WindowButton::{Close, Maximize, Minimize};
    if let Some(tried) = std::env::var("SIOUL_BUTTON_LAYOUT").ok().and_then(|value| gnome_layout(&value)) {
        return tried;
    }
    // macOS: close, minimize and zoom, on the left.
    if cfg!(target_os = "macos") {
        return ButtonLayout { left: vec![Close, Minimize, Maximize], right: Vec::new() };
    }
    if cfg!(any(windows, target_os = "android")) {
        return ButtonLayout::on_the_right();
    }
    // Linux and the BSDs: the desktop's own setting; "ubuntu:GNOME", "X-Cinnamon".
    let current = std::env::var("XDG_CURRENT_DESKTOP").unwrap_or_default().to_lowercase();
    let desktops: Vec<&str> = current.split(':').map(str::trim).collect();
    let on = |names: &[&str]| desktops.iter().any(|desktop| names.contains(desktop));
    if on(&["kde"]) {
        return kde_layout();
    }
    if on(&["xfce"]) {
        // Xfce's own, as it comes: its menu on the left, then minimize, maximize and close.
        return command("xfconf-query", &["-c", "xfwm4", "-p", "/general/button_layout"]).and_then(|value| xfwm_layout(&value)).unwrap_or_else(|| ButtonLayout::on_the_right());
    }
    if on(&["gnome", "unity", "budgie", "pantheon", "x-cinnamon", "cinnamon", "mate"]) {
        // GNOME's setting, which its kin read too; Cinnamon and MATE keep their own first.
        let schemas: &[&str] = if on(&["x-cinnamon", "cinnamon"]) {
            &["org.cinnamon.desktop.wm.preferences", "org.gnome.desktop.wm.preferences"]
        } else if on(&["mate"]) {
            &["org.mate.Marco.general", "org.gnome.desktop.wm.preferences"]
        } else {
            &["org.gnome.desktop.wm.preferences"]
        };
        if let Some(layout) = schemas.iter().find_map(|schema| command("gsettings", &["get", schema, "button-layout"]).and_then(|value| gnome_layout(&value))) {
            return layout;
        }
        // GNOME's own, as it comes: close alone, on the right.
        if on(&["gnome"]) {
            return ButtonLayout { left: Vec::new(), right: vec![Close] };
        }
    }
    ButtonLayout::on_the_right()
}

/// What a program prints, trimmed; None when it is missing or fails.
fn command(program: &str, args: &[&str]) -> Option<String> {
    let output = std::process::Command::new(program)
        .args(args)
        .stdin(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .output()
        .ok()?;
    output.status.success().then(|| String::from_utf8_lossy(&output.stdout).trim().to_string())
}

/// Plasma's buttons (kwinrc, `[org.kde.kdecoration2]`): yours, else your
/// global theme's, else the system's, else Plasma's own, "MS" on the left
/// (the window's menu, on all desktops) and "HIAX" on the right.
fn kde_layout() -> ButtonLayout {
    let files: Vec<String> = kwinrc_files().iter().filter_map(|path| std::fs::read_to_string(path).ok()).collect();
    let read = |key: &str, plasma: &str| files.iter().find_map(|text| kconfig_value(text, "org.kde.kdecoration2", key)).unwrap_or_else(|| plasma.to_string());
    ButtonLayout { left: kde_buttons(&read("ButtonsOnLeft", "MS")), right: kde_buttons(&read("ButtonsOnRight", "HIAX")) }
}

/// The kwinrc files, the strongest first: in your configuration folder (and
/// the host's, from a Flatpak, whose own is apart), your global theme's
/// defaults beside it (kdedefaults), then the system's.
fn kwinrc_files() -> Vec<PathBuf> {
    let mut homes: Vec<PathBuf> = Vec::new();
    let named = [std::env::var_os("XDG_CONFIG_HOME"), std::env::var_os("HOST_XDG_CONFIG_HOME")].into_iter().flatten().filter(|v| !v.is_empty()).map(PathBuf::from);
    for home in named.chain([sioul_core::config::expand_home("~/.config")]) {
        if !homes.contains(&home) {
            homes.push(home);
        }
    }
    let dirs = std::env::var_os("XDG_CONFIG_DIRS").filter(|v| !v.is_empty()).unwrap_or_else(|| "/etc/xdg".into());
    homes
        .iter()
        .map(|home| home.join("kwinrc"))
        .chain(homes.iter().map(|home| home.join("kdedefaults").join("kwinrc")))
        .chain(std::env::split_paths(&dirs).map(|dir| dir.join("kwinrc")))
        .collect()
}

/// The value of `key` in `[group]` of a KConfig file (kwinrc), the last one
/// written there. A key marked immutable or to expand (`Key[$i]`, `Key[$e]`)
/// counts, a translation (`Key[fr]`) does not, one marked deleted
/// (`Key[$d]`) is not set. None when it is not there.
pub fn kconfig_value(text: &str, group: &str, key: &str) -> Option<String> {
    let header = format!("[{group}]");
    let mut inside = false;
    let mut value = None;
    for line in text.lines().map(str::trim) {
        if line.starts_with('[') {
            // "[group]", or "[group][$i]", immutable.
            inside = line.strip_prefix(header.as_str()).is_some_and(|rest| rest.is_empty() || rest.starts_with("[$"));
            continue;
        }
        if !inside || line.starts_with('#') {
            continue;
        }
        let Some((name, written)) = line.split_once('=') else { continue };
        let name = name.trim();
        let (bare, marks) = name.split_once('[').map_or((name, ""), |(bare, marks)| (bare.trim_end(), marks));
        if bare != key || !(marks.is_empty() || marks.starts_with('$')) {
            continue;
        }
        value = (!marks.contains('d')).then(|| written.trim().to_string());
    }
    value
}

/// Plasma's letters for the window's buttons (kwinrc's ButtonsOnLeft and
/// ButtonsOnRight), those Sioul's bar draws: I minimize, A maximize, X close.
/// The others have no place there: M the window's menu, S on all desktops, H
/// help, F keep above, B keep below, L shade, N the application's menu, _ a space.
pub fn kde_buttons(letters: &str) -> Vec<WindowButton> {
    letters
        .chars()
        .filter_map(|letter| match letter {
            'I' => Some(WindowButton::Minimize),
            'A' => Some(WindowButton::Maximize),
            'X' => Some(WindowButton::Close),
            _ => None,
        })
        .collect()
}

/// A `button-layout` as GNOME writes it, bare or as gsettings prints it
/// ('appmenu:minimize,maximize,close'): the left side's buttons before the
/// colon, the right's after it; without a colon, all on the left, as GNOME
/// reads it. "appmenu", "menu", "icon" and "spacer" have no place in Sioul's
/// bar. None for an empty value.
pub fn gnome_layout(value: &str) -> Option<ButtonLayout> {
    let value = value.trim();
    let value = value.strip_prefix('\'').and_then(|v| v.strip_suffix('\'')).unwrap_or(value).trim();
    if value.is_empty() {
        return None;
    }
    let (left, right) = value.split_once(':').unwrap_or((value, ""));
    let names = |side: &str| -> Vec<WindowButton> {
        side.split(',')
            .filter_map(|name| match name.trim() {
                "minimize" => Some(WindowButton::Minimize),
                "maximize" => Some(WindowButton::Maximize),
                "close" => Some(WindowButton::Close),
                _ => None,
            })
            .collect()
    };
    Some(ButtonLayout { left: names(left), right: names(right) })
}

/// xfwm4's `button_layout` (Xfce), "O|HMC": the left side's letters before
/// the bar, which stands for the title, the right's after it. H minimize (it
/// hides the window), M maximize, C close; O the window's menu, S shade and T
/// on all workspaces have no place in Sioul's bar. None without the bar.
pub fn xfwm_layout(value: &str) -> Option<ButtonLayout> {
    let (left, right) = value.trim().split_once('|')?;
    let letters = |side: &str| -> Vec<WindowButton> {
        side.chars()
            .filter_map(|letter| match letter {
                'H' => Some(WindowButton::Minimize),
                'M' => Some(WindowButton::Maximize),
                'C' => Some(WindowButton::Close),
                _ => None,
            })
            .collect()
    };
    Some(ButtonLayout { left: letters(left), right: letters(right) })
}

#[cfg(test)]
mod tests {
    use super::*;
    use WindowButton::{Close, Maximize, Minimize};

    fn sides(left: &[WindowButton], right: &[WindowButton]) -> ButtonLayout {
        ButtonLayout { left: left.to_vec(), right: right.to_vec() }
    }

    #[test]
    fn plasma_letters() {
        // Plasma's own: the menu and on all desktops on the left, help, minimize, maximize, close on the right.
        assert_eq!(kde_buttons("MS"), Vec::<WindowButton>::new());
        assert_eq!(kde_buttons("HIAX"), vec![Minimize, Maximize, Close]);
        assert_eq!(kde_buttons("XIA"), vec![Close, Minimize, Maximize]);
        assert_eq!(kde_buttons("M_SFBLNX"), vec![Close]);
        assert_eq!(kde_buttons(""), Vec::<WindowButton>::new());
    }

    #[test]
    fn kwinrc_values() {
        let kwinrc = "[Desktops]\nNumber=2\n\n[org.kde.kdecoration2]\nBorderSize=NoSides\nButtonsOnLeft=XIA\nButtonsOnRight=\nlibrary=org.kde.breeze\n\n[Windows]\nButtonsOnLeft=MS\n";
        assert_eq!(kconfig_value(kwinrc, "org.kde.kdecoration2", "ButtonsOnLeft").as_deref(), Some("XIA"));
        assert_eq!(kconfig_value(kwinrc, "org.kde.kdecoration2", "ButtonsOnRight").as_deref(), Some(""));
        assert_eq!(kconfig_value(kwinrc, "org.kde.kdecoration2", "Missing"), None);
        assert_eq!(kconfig_value(kwinrc, "Desktops", "ButtonsOnLeft"), None);
        assert_eq!(kconfig_value("", "org.kde.kdecoration2", "ButtonsOnLeft"), None);
        // Immutable counts, a translation does not, deleted unsets; the last one written wins.
        let marked = "[org.kde.kdecoration2][$i]\nButtonsOnLeft[$i]=IAX\nButtonsOnRight[fr]=X\n# ButtonsOnRight=IX\n[org.kde.kdecoration2]\nButtonsOnLeft = MS \nButtonsOnRight[$d]=\n";
        assert_eq!(kconfig_value(marked, "org.kde.kdecoration2", "ButtonsOnLeft").as_deref(), Some("MS"));
        assert_eq!(kconfig_value(marked, "org.kde.kdecoration2", "ButtonsOnRight"), None);
        // A group whose name only begins the same is another group.
        assert_eq!(kconfig_value("[org.kde.kdecoration2.other]\nButtonsOnLeft=X\n", "org.kde.kdecoration2", "ButtonsOnLeft"), None);
    }

    #[test]
    fn gnome_values() {
        assert_eq!(gnome_layout("'appmenu:close'"), Some(sides(&[], &[Close])));
        assert_eq!(gnome_layout("appmenu:minimize,maximize,close"), Some(sides(&[], &[Minimize, Maximize, Close])));
        assert_eq!(gnome_layout("'close,minimize,maximize:'\n"), Some(sides(&[Close, Minimize, Maximize], &[])));
        assert_eq!(gnome_layout("close:spacer,menu"), Some(sides(&[Close], &[])));
        assert_eq!(gnome_layout(" icon , close : maximize "), Some(sides(&[Close], &[Maximize])));
        // Without a colon, all on the left; nothing named, no buttons.
        assert_eq!(gnome_layout("close,minimize"), Some(sides(&[Close, Minimize], &[])));
        assert_eq!(gnome_layout("appmenu:"), Some(sides(&[], &[])));
        assert_eq!(gnome_layout("''"), None);
        assert_eq!(gnome_layout(""), None);
    }

    #[test]
    fn xfwm_values() {
        assert_eq!(xfwm_layout("O|HMC"), Some(sides(&[], &[Minimize, Maximize, Close])));
        assert_eq!(xfwm_layout("CHM|\n"), Some(sides(&[Close, Minimize, Maximize], &[])));
        assert_eq!(xfwm_layout("OTS|HC"), Some(sides(&[], &[Minimize, Close])));
        assert_eq!(xfwm_layout("HMC"), None);
    }

    #[test]
    fn each_button_once_as_qml_reads_them() {
        let layout = sides(&[Close, Minimize], &[Minimize, Maximize, Close]).once();
        assert_eq!(layout, sides(&[Close, Minimize], &[Maximize]));
        assert_eq!(layout.json(), r#"{"left":["close","minimize"],"right":["maximize"]}"#);
        assert_eq!(ButtonLayout::on_the_right().json(), r#"{"left":[],"right":["minimize","maximize","close"]}"#);
        assert_eq!(sides(&[], &[]).json(), r#"{"left":[],"right":[]}"#);
    }
}
