// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

//! Builds the QML module: the Rust object behind the window, and the pages.
//! Needs Qt 6's development files, Qt Declarative included (docs/building.md).

use cxx_qt_build::{CxxQtBuilder, QmlModule};

fn main() {
    windows_details();
    // Android has no Qt WebEngine and no Qt PDF: the pages made of them have
    // Android versions of the same names in qml/android/ (the module's qmldir
    // finds them there). Elsewhere Qt WebEngine is not linked either: the
    // Sites page's import loads it (cpp/webengine.cpp).
    let android = std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("android");
    let pages = [
        "qml/main.qml",
        "qml/PorchPage.qml",
        "qml/MailPage.qml",
        "qml/Reader.qml",
        "qml/ComposeWindow.qml",
        "qml/BudgetsPage.qml",
        "qml/AccountsPage.qml",
        "qml/Panel.qml",
        "qml/Icon.qml",
        "qml/ActionButton.qml",
        "qml/AddressField.qml",
        "qml/AgendaPage.qml",
        "qml/ContactsPage.qml",
        "qml/ContactForm.qml",
        "qml/LabeledRows.qml",
        "qml/DateField.qml",
        "qml/EventDialog.qml",
        "qml/EventRow.qml",
        "qml/WeekPlanning.qml",
        "qml/TasksPage.qml",
        "qml/TaskRow.qml",
        "qml/TaskPanel.qml",
        "qml/TaskTimeline.qml",
        "qml/CaptureField.qml",
        "qml/RelatedList.qml",
        "qml/FocusWindow.qml",
        "qml/NotesPage.qml",
        "qml/Theme.qml",
        "qml/SioulWindow.qml",
        "qml/SettingsPanel.qml",
        "qml/SettingsButton.qml",
        "qml/SettingRow.qml",
        "qml/MoveDialog.qml",
        "qml/AddMenu.qml",
        "qml/ThingActions.qml",
        "qml/LinkPicker.qml",
        "qml/ProjectsPage.qml",
        "qml/ProjectDialog.qml",
        "qml/TimePage.qml",
        "qml/TimeDialog.qml",
        "qml/BudgetDetail.qml",
        "qml/BalanceChart.qml",
        "qml/MovementDialog.qml",
        "qml/ContactMap.qml",
        "qml/SitesPage.qml",
        "qml/DoneDialog.qml",
        "qml/PdfView.qml",
        "qml/AudioPlayer.qml",
        "qml/MemoRecorder.qml",
        "qml/BudgetDialog.qml",
        "qml/ConfirmDialog.qml",
        "qml/HealthPage.qml",
        "qml/WeatherApplet.qml",
        "qml/SoundsApplet.qml",
        "qml/NoisePlayer.qml",
        "qml/WebAuthDialog.qml",
        "qml/NewMenu.qml",
        "qml/ParametersPage.qml",
        "qml/Avatar.qml",
        "qml/LineDialog.qml",
        "qml/TimeExport.qml",
        "qml/WatchPanel.qml",
        "qml/SharePanel.qml",
        "qml/PapersPage.qml",
        "qml/PaperDialog.qml",
        "qml/DayView.qml",
        "qml/RoutinesDialog.qml",
        "qml/RoutinePlayer.qml",
        "qml/ContractDialog.qml",
        "qml/ContractsSection.qml",
        "qml/PasswordField.qml",
        "qml/SioulMenu.qml",
        "qml/SitePopup.qml", "qml/PresetPlaceMenu.qml", "qml/PresetGroupMenu.qml", "qml/BankAccountDialog.qml", "qml/BankRulesDialog.qml", "qml/ReserveDialog.qml",
        "qml/LoginChooser.qml",
        "qml/VaultUnlock.qml",
        "qml/AccountPassword.qml",
        "qml/FolderBrowser.qml",
        "qml/RestCover.qml",
        "qml/DoseTaken.qml",
        "qml/BankSection.qml",
        "qml/LettersSection.qml",
    ];
    let desktop_only = ["qml/SitesPage.qml", "qml/SitePopup.qml", "qml/WebAuthDialog.qml", "qml/PdfView.qml"];
    let pages: Vec<&str> = if android {
        pages.into_iter().filter(|page| !desktop_only.contains(page)).chain(["qml/android/SitesPage.qml", "qml/android/PdfView.qml"]).collect()
    } else {
        pages.to_vec()
    };
    // Line spacing for editable text, which Qt Quick does not offer; PDFs written; the window's
    // icon; text and images made ready at the start.
    let mut cpp = vec!["cpp/textspacing.h", "cpp/textspacing.cpp", "cpp/pdfwriter.h", "cpp/pdfwriter.cpp", "cpp/appicon.cpp", "cpp/warmup.cpp"];
    if !android {
        cpp.push("cpp/webengine.cpp");
    }
    CxxQtBuilder::new_qml_module(QmlModule::new("com.aurelienpierre.sioul").depend("QtQuick").qml_files(pages))
        .files(["src/backend.rs", "src/desktop.rs"])
        .cpp_files(cpp)
        .qt_module("Quick")
        // The Breeze icons Sioul uses, for systems without them (tools/bundle-icons.py).
        .qrc("icons/icons.qrc")
        // Sioul's own icon, for its windows (tools/make-icons.py).
        .qrc("app.qrc")
        // The symbols the system's fonts may lack (tools/make-symbols-font.py).
        .qrc("fonts/fonts.qrc")
        .build();
}

/// On Windows, the program's own icon and its details, as Explorer shows them
/// in its properties (tools/make-icons.py makes the icon). Nothing elsewhere.
fn windows_details() {
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("windows") {
        return;
    }
    let mut resource = winresource::WindowsResource::new();
    resource
        .set_icon("../../packaging/windows/sioul.ico")
        .set("ProductName", "Sioul")
        .set("FileDescription", "Sioul, a calm place for mail, tasks and admin")
        .set("CompanyName", "Aurélien Pierre")
        .set("LegalCopyright", "Copyright © 2026 Aurélien Pierre. GPL-3.0-or-later.");
    resource.compile().expect("the program's Windows resources (icon, details)");
    println!("cargo::rerun-if-changed=../../packaging/windows/sioul.ico");
}
