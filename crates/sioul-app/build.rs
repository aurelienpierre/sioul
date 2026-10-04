// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

//! Builds the QML module: the Rust object behind the window, and the pages.
//! Needs Qt 6's development files, Qt Declarative included (docs/building.md).

use cxx_qt_build::{CxxQtBuilder, QmlModule};

fn main() {
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
        "qml/BankSection.qml",
        "qml/LettersSection.qml",
    ];
    // The Breeze icons Sioul uses, for systems without them (tools/bundle-icons.py).
    CxxQtBuilder::new_qml_module(QmlModule::new("com.aurelienpierre.sioul").depend("QtQuick").qml_files(pages))
        .files(["src/backend.rs", "src/desktop.rs"])
        // Line spacing for editable text, which Qt Quick does not offer.
        .cpp_files(["cpp/textspacing.h", "cpp/textspacing.cpp", "cpp/pdfwriter.h", "cpp/pdfwriter.cpp", "cpp/webengine.cpp", "cpp/appicon.cpp"])
        .qt_module("Quick")
        .qt_module("WebEngineQuick")
        .qrc("icons/icons.qrc")
        // Sioul's own icon, for its windows (tools/make-icons.py).
        .qrc("app.qrc")
        .build();
}
