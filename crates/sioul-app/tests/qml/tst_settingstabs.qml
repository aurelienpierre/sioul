// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

// Settings' sections (ParametersPage.qml), on a stand-in for Sioul, in
// English and French. On a computer's width, the sections are tabs across the
// page (Tabs.qml): every one shows, on one row, within the page, none over
// another, each with its words whole; the section's rows sit in a centred
// column of legible width, under their headings in frames (Section.qml), and
// Devices takes the whole width. On a phone's width (412 points), the page
// opens on the list of its sections, each with its name and a line saying
// what it holds; a tap opens one, Back returns to the list, and a link to a
// setting opens its section. A phone lists "This phone", which only Android
// does: the stand-in names the tab a computer lists in its place (Texts)
// with the phone's words, so that the row holds as many words as on a phone.
// Run by tools/qml-test.sh. To see the pages, set `pictures` below to a
// folder (a change of your own, not to commit).

import QtQuick
import QtQuick.Controls.Basic
import QtTest
import "../../qml"

Item {
    id: root

    // Where the pictures go; none when empty.
    readonly property string pictures: ""

    width: 1756
    height: 915

    Theme {
        id: computerTheme
    }
    Theme {
        id: phoneTheme

        compact: true
        touch: true
    }

    QtObject {
        id: mock

        property string language: "en"
        // Without telephony, the tab a computer lists stands for the phone's own.
        readonly property bool telephony: false
        readonly property string deviceKind: "phone"
        readonly property var words: ({
            "en": {
                "ui-parameters": "Settings",
                "settings-tab-look": "Display",
                "settings-tab-hours": "Hours",
                "settings-tab-attention": "What reaches you",
                "settings-tab-words": "Words",
                "settings-tab-reminders": "Reminders",
                "settings-tab-pauses": "Pauses",
                "settings-tab-texts": "This phone",
                "settings-tab-files": "Your folder and sharing",
                "settings-tab-devices": "Devices",
                "settings-tab-invoices": "Invoices",
                "settings-tab-ai": "AI agents",
                "settings-about-look": "Language, colours, the places' names, passwords as you type, what this device is.",
                "settings-about-hours": "Working hours, hours for your admin, meals and the night, time off."
            },
            "fr": {
                "ui-parameters": "Paramètres",
                "settings-tab-look": "Affichage",
                "settings-tab-hours": "Heures",
                "settings-tab-attention": "Ce qui vous joint",
                "settings-tab-words": "Mots",
                "settings-tab-reminders": "Rappels",
                "settings-tab-pauses": "Pauses",
                "settings-tab-texts": "Ce téléphone",
                "settings-tab-files": "Votre dossier et le partage",
                "settings-tab-devices": "Appareils",
                "settings-tab-invoices": "Factures",
                "settings-tab-ai": "Agents d’IA",
                "settings-about-look": "Langue, couleurs, nom des lieux, mots de passe pendant la saisie, ce qu’est cet appareil.",
                "settings-about-hours": "Heures de travail, heures pour vos démarches, repas et nuit, congés."
            }
        })

        function text(key) {
            return mock.words[mock.language][key] || key
        }
        // Three rows of Display under two headings, one of Hours.
        function settings(view) {
            return JSON.stringify([
                { key: "places_named", section: "look", group: "Language and appearance", kind: "bool", label: "Show the places' names beside their icons", help: "", value: false },
                { key: "passwords_shown", section: "look", group: "Language and appearance", kind: "bool", label: "Show passwords as you type", help: "", value: false },
                { key: "device_kind", section: "look", group: "This device", kind: "bool", label: "What this device is", help: "", value: false },
                { key: "time_off", section: "hours", group: "Working hours and quiet time", kind: "bool", label: "Time off", help: "", value: false }
            ])
        }
        function setSetting(key, value) {
            return ""
        }
        function changeSetting(key, shown, value) {
            return ""
        }
        function devices() {
            return JSON.stringify({ on: false, why: "", stays: "", rows: [] })
        }
    }

    // A computer's window, the places beside the page left out (1400 less 56).
    Item {
        id: computer

        width: 1344
        height: root.height

        ParametersPage {
            id: wide

            anchors.fill: parent
            sioul: mock
            theme: computerTheme
        }
    }

    // A phone held upright, beside it.
    Item {
        id: phone

        x: computer.width
        width: 412
        height: root.height

        ParametersPage {
            id: narrow

            anchors.fill: parent
            sioul: mock
            theme: phoneTheme
        }
    }

    TestCase {
        name: "SettingsTabs"
        when: windowShown

        function walk(item, test, found) {
            if (!item)
                return found
            if (test(item))
                found.push(item)
            const children = item.children || []
            for (let i = 0; i < children.length; ++i)
                walk(children[i], test, found)
            if (item.contentItem && item.contentItem !== item && children.indexOf(item.contentItem) < 0)
                walk(item.contentItem, test, found)
            return found
        }
        function named(item, name) {
            return walk(item, i => i.objectName === name, [])
        }
        function one(item, name) {
            const found = named(item, name)
            verify(found.length === 1, name + ": " + found.length + " found")
            return found[0]
        }
        function columnOf(page) {
            return walk(page, i => i.room !== undefined && i.wide !== undefined, [])[0]
        }
        function save(item, name) {
            if (root.pictures === "")
                return
            wait(100)
            grabImage(item).save(root.pictures + "/" + name + ".png")
        }

        function init() {
            wide.reload()
            narrow.reload()
            wide.section = "look"
            narrow.back()
            wait(50)
        }

        function test_tabs_span_a_computer_s_page_data() {
            return [{ tag: "en", language: "en" }, { tag: "fr", language: "fr" }]
        }

        function test_tabs_span_a_computer_s_page(data) {
            mock.language = data.language
            wait(100)
            const row = one(wide, "settingsTabs")
            verify(row.visible, "the tabs show")
            // The row spans the page, less its margins.
            const rowAt = row.mapToItem(computer, 0, 0)
            fuzzyCompare(rowAt.x, computerTheme.gap, 0.5, "from the page's left margin")
            fuzzyCompare(row.width, computer.width - 2 * computerTheme.gap, 0.5, "to its right margin")
            const tabs = wide.sections.map(s => one(wide, "settingsTabs:" + s))
            verify(wide.sections.indexOf("devices") >= 0, "Devices among them")
            const boxes = tabs.map(t => {
                const at = t.mapToItem(computer, 0, 0)
                return { text: t.text, x: at.x, y: at.y, w: t.width, h: t.height, words: t.contentItem.implicitWidth }
            })
            for (const b of boxes) {
                verify(b.x >= rowAt.x - 0.5 && b.x + b.w <= rowAt.x + row.width + 0.5, b.text + ": within the row")
                verify(b.w >= b.words, b.text + ": its words whole")
                verify(Math.abs(b.y - boxes[0].y) < 0.5, b.text + ": on the one row")
            }
            for (let i = 1; i < boxes.length; ++i)
                verify(boxes[i - 1].x + boxes[i - 1].w <= boxes[i].x + 0.5, boxes[i - 1].text + " before " + boxes[i].text)
            // Every name fits: the row does not scroll, and the tabs fill it.
            verify(!row.scrolls, "no sideways scrolling at 1344 points")
            fuzzyCompare(boxes[boxes.length - 1].x + boxes[boxes.length - 1].w - boxes[0].x, row.width, 1, "the tabs fill the row")
            save(computer, "settings-computer-" + data.language)
            // Display's rows: in a column of legible width, centred, under their two headings.
            const column = columnOf(wide)
            compare(column.width, computerTheme.readable, "a legible width")
            fuzzyCompare(column.x, (column.room - column.width) / 2, 1, "centred")
            const headings = walk(column, i => i.visible && i.Accessible.role === Accessible.Heading, [])
            compare(headings.map(h => h.text).join(" | "), "Language and appearance | This device", "a frame per heading")
            // Devices pressed: shown, the whole width.
            tabs[wide.sections.indexOf("devices")].clicked()
            compare(wide.section, "devices")
            wait(50)
            compare(columnOf(wide).width, columnOf(wide).room, "Devices: the whole width")
        }

        function test_a_phone_lists_the_sections_data() {
            return [{ tag: "en", language: "en" }, { tag: "fr", language: "fr" }]
        }

        function test_a_phone_lists_the_sections(data) {
            mock.language = data.language
            wait(100)
            verify(!one(narrow, "settingsTabs").visible, "no tabs on a phone")
            verify(narrow.listed, "the list first")
            compare(narrow.canGoBack, false, "nothing to go back to")
            const entries = narrow.sections.map(s => one(narrow, "settingsSection:" + s))
            for (const entry of entries) {
                const at = entry.mapToItem(phone, 0, 0)
                verify(entry.visible, entry.text + ": shows")
                verify(at.x >= 0 && at.x + entry.width <= phone.width + 0.5, entry.text + ": within the page")
                verify(entry.height >= 44, entry.text + ": a finger's height (" + entry.height + ")")
                verify(entry.Accessible.description !== "", entry.text + ": says what it holds")
            }
            compare(entries[0].text, mock.text("settings-tab-look"))
            save(phone, "settings-phone-list-" + data.language)
            // Hours tapped: its rows, Back to the list.
            entries[narrow.sections.indexOf("hours")].clicked()
            compare(narrow.section, "hours")
            verify(!narrow.listed, "the section shows")
            verify(narrow.canGoBack, "Back returns to the list")
            verify(named(narrow, "setting:time_off").length === 1, "its row")
            save(phone, "settings-phone-hours-" + data.language)
            narrow.back()
            verify(narrow.listed, "the list again")
            // Display tapped while it is the section already shown: it opens all the same.
            narrow.section = "look"
            narrow.back()
            one(narrow, "settingsSection:look").clicked()
            verify(!narrow.listed, "Display opens")
            narrow.back()
        }

        function test_a_link_opens_its_section_on_a_phone() {
            mock.language = "en"
            verify(narrow.listed, "the list first")
            narrow.showSetting("time_off")
            compare(narrow.section, "hours")
            verify(!narrow.listed, "a setting's row: its section open")
            narrow.back()
            narrow.showSetting("invoices")
            compare(narrow.section, "invoices")
            verify(!narrow.listed, "a section's key: the section open")
            narrow.back()
            narrow.showSetting("ai")
            compare(narrow.section, "ai")
            verify(!narrow.listed, "a section of no rows: open")
        }
    }
}
