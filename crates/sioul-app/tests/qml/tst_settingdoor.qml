// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

// A setting opened by its key (SettingDoor.qml, main.qml's
// `showParameters`): each lands where it lives now, on a stand-in for the
// window that records what it is asked, and a stand-in for Sioul that
// says where each key lives as the core's `settings::page_of` does (its
// own test, `a_link_by_key_opens_the_page_that_holds_it`, checks the core's
// side). Run by tools/qml-test.sh.

import QtQuick
import QtTest
import "../../qml"

TestCase {
    id: root

    name: "SettingDoor"

    // What the core says of each key (`settings::page_of`): "" for Settings.
    QtObject {
        id: mock

        // A computer: a Texts page of its own.
        property bool telephony: false
        readonly property var places: ({
            "screen_colours": "sites",
            "calmer_colours": "sites",
            "sites": "sites",
            "bitwarden": "bitwarden",
            "bitwarden.email": "bitwarden",
            "bitwarden_vault": "bitwarden",
            "reminders.before_event": "agenda",
            "reminders.waits": "tasks",
            "reminders.payment_days": "money",
            "texts": "texts",
            "texts.sending": "texts",
            "case_store": "notes",
            "notes_folder": "notes"
        })

        function settingPage(key) {
            return mock.places[key] || ""
        }
    }

    // A page that records its ⚙ opened, at which setting.
    component FakePage: QtObject {
        property var asked: []

        function openSettings(open, key) {
            asked = asked.concat([[open, key]])
        }
    }

    QtObject {
        id: fakeWindow

        property int page: 0
        property var parametersPage: QtObject {
            property var asked: []

            function showSetting(key) {
                asked = asked.concat([key])
            }
        }
        property var sitesPage: FakePage {}
        property var accountsPage: FakePage {}
        property var agendaPage: FakePage {}
        property var tasksPage: FakePage {}
        property var budgetsPage: FakePage {}
        property var textsPage: FakePage {}
        property var notesPage: FakePage {}
    }

    SettingDoor {
        id: door

        window: fakeWindow
        sioul: mock
    }

    function init() {
        fakeWindow.page = 0
        fakeWindow.parametersPage.asked = []
        fakeWindow.sitesPage = Qt.createQmlObject("import QtQuick; QtObject { property var asked: []; function openSettings(open, key) { asked = asked.concat([[open, key]]) } }", root)
        fakeWindow.accountsPage = Qt.createQmlObject("import QtQuick; QtObject { property var asked: []; function openSettings(open, key) { asked = asked.concat([[open, key]]) } }", root)
    }

    // Bitwarden: Accounts, its password manager's card; the vault's own key opens it.
    function test_bitwarden_opens_the_accounts_card() {
        door.open("bitwarden")
        compare(fakeWindow.page, 11)
        compare(fakeWindow.accountsPage.asked, [[true, ""]])
        door.open("bitwarden.email")
        door.open("bitwarden_vault")
        compare(fakeWindow.accountsPage.asked.slice(1), [[true, "bitwarden.email"], [true, "bitwarden_vault"]])
        compare(fakeWindow.sitesPage.asked, [], "never the Sites page")
        compare(fakeWindow.parametersPage.asked, [], "never Settings")
    }

    // The sites' colours: the Sites page, its ⚙ open at the row.
    function test_the_sites_colours_open_the_sites_page_settings() {
        door.open("calmer_colours")
        compare(fakeWindow.page, 3)
        compare(fakeWindow.sitesPage.asked, [[true, "calmer_colours"]])
        compare(fakeWindow.parametersPage.asked, [], "not Settings")
        door.open("sites")
        compare(fakeWindow.sitesPage.asked[1], [true, ""], "the page's own name: its ⚙ at its top")
    }

    // Three reminders, each its page's: before an event in the Agenda's ⚙, a wait over
    // in the Tasks', a planned payment in the Budgets page's; the others stay in Settings.
    function test_the_reminders_open_their_pages_settings() {
        for (const [key, page, name] of [["reminders.before_event", 4, "agendaPage"], ["reminders.waits", 1, "tasksPage"], ["reminders.payment_days", 9, "budgetsPage"]]) {
            fakeWindow[name] = Qt.createQmlObject("import QtQuick; QtObject { property var asked: []; function openSettings(open, key) { asked = asked.concat([[open, key]]) } }", root)
            fakeWindow.page = 0
            door.open(key)
            compare(fakeWindow.page, page, key)
            compare(fakeWindow[name].asked, [[true, key]], key)
        }
        for (const key of ["reminders.asked_days", "reminders_closed", "reminders.gathered", "reminders"]) {
            fakeWindow.page = 0
            door.open(key)
            compare(fakeWindow.page, 12, key)
        }
        compare(fakeWindow.parametersPage.asked, ["reminders.asked_days", "reminders_closed", "reminders.gathered", "reminders"])
    }

    // The notes folder: the Notes page's ⚙, at its row (the sharing's line, the Porch,
    // older links); Settings' tab "files" holds the sharing alone, opened by its id.
    function test_the_notes_folder_opens_the_notes_page_settings() {
        fakeWindow.notesPage = Qt.createQmlObject("import QtQuick; QtObject { property var asked: []; function openSettings(open, key) { asked = asked.concat([[open, key]]) } }", root)
        door.open("case_store")
        compare(fakeWindow.page, 6)
        compare(fakeWindow.notesPage.asked, [[true, "case_store"]])
        door.open("files")
        compare(fakeWindow.page, 12, "the sharing: Settings")
        compare(fakeWindow.parametersPage.asked, ["files"])
    }

    // The multimedia messages sent from here: the Texts page's ⚙, by the setting's key
    // or by the page's name ("texts"); a device with telephony has no Texts page:
    // Settings, as for any key it does not show.
    function test_texts_open_the_texts_page_settings() {
        fakeWindow.textsPage = Qt.createQmlObject("import QtQuick; QtObject { property var asked: []; function openSettings(open, key) { asked = asked.concat([[open, key]]) } }", root)
        door.open("texts")
        compare(fakeWindow.page, 14)
        door.open("texts.sending")
        compare(fakeWindow.textsPage.asked, [[true, ""], [true, "texts.sending"]])
        compare(fakeWindow.parametersPage.asked, [])
        mock.telephony = true
        door.open("texts.sending")
        compare(fakeWindow.page, 12, "a phone: no Texts page")
        compare(fakeWindow.parametersPage.asked, ["texts.sending"])
        compare(fakeWindow.textsPage.asked.length, 2)
        mock.telephony = false
    }

    // What Settings holds: Settings, at its tab; the usual meals, naps and night
    // ("needs", every door to them) in Settings ▸ Hours.
    function test_settings_keys_open_settings() {
        for (const key of ["theme", "window", "needs", "time_off", "attention.pause", "words.languages", "files"]) {
            fakeWindow.page = 0
            door.open(key)
            compare(fakeWindow.page, 12, key)
        }
        compare(fakeWindow.parametersPage.asked, ["theme", "window", "needs", "time_off", "attention.pause", "words.languages", "files"])
        compare(fakeWindow.sitesPage.asked, [])
    }

    // A page without a ⚙ of its own on this device (Android's Sites page): the page alone.
    function test_a_page_without_its_settings_shows_alone() {
        fakeWindow.sitesPage = Qt.createQmlObject("import QtQuick; QtObject {}", root)
        door.open("screen_colours")
        compare(fakeWindow.page, 3)
    }
}
