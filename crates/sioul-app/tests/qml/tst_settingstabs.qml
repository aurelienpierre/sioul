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
// setting opens its section. The phone's page is told it runs on Android,
// so that its list holds a phone's sections, This phone among them.
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
        // A phone: its own tab, This phone (the page told it runs on Android, below).
        readonly property bool telephony: true
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
                "settings-tab-phone": "This phone",
                "settings-tab-files": "Your folder and sharing",
                "settings-tab-devices": "Devices",
                "settings-tab-invoices": "Invoices",
                "settings-tab-ai": "AI agents",
                "settings-about-look": "Language, colours, the places' names, passwords as you type.",
                "settings-about-hours": "Working hours, hours for your admin, meals and the night, time off.",
                "settings-about-attention": "Who reaches you, and when; Sioul's own notices; do not disturb on every device.",
                "settings-about-words": "The words Sioul looks for in mail, papers, calls and tasks, in your languages.",
                "settings-about-reminders": "Reminders of dates asked, with Sioul's window closed, and notifications gathered.",
                "settings-about-pauses": "Free time and the pause: when work ends, what helps you, the pause's screen.",
                "settings-about-phone": "What this device is, and what its system lets Sioul do: calls, other apps' notifications, texts, Do not disturb, alarms.",
                "settings-about-files": "Your notes folder, and sharing with your other devices.",
                "settings-about-devices": "Your devices: what each is, what it is for, what comes forward on each.",
                "settings-about-invoices": "Who issues your invoices, their numbers, VAT, bank and legal lines.",
                "settings-about-ai": "What AI agents may use, project by project."
            },
            "fr": {
                "ui-parameters": "Paramètres",
                "settings-tab-look": "Affichage",
                "settings-tab-hours": "Heures",
                "settings-tab-attention": "Ce qui vous joint",
                "settings-tab-words": "Mots",
                "settings-tab-reminders": "Rappels",
                "settings-tab-pauses": "Pauses",
                "settings-tab-phone": "Ce téléphone",
                "settings-tab-files": "Votre dossier et le partage",
                "settings-tab-devices": "Appareils",
                "settings-tab-invoices": "Factures",
                "settings-tab-ai": "Agents d’IA",
                "settings-about-look": "Langue, couleurs, nom des lieux, mots de passe pendant la saisie.",
                "settings-about-hours": "Heures de travail, heures pour vos démarches, repas et nuit, congés.",
                "settings-about-attention": "Qui vous joint, et quand ; les avis de Sioul ; « Ne pas déranger » sur tous vos appareils.",
                "settings-about-words": "Les mots que Sioul cherche dans les courriels, les papiers, les appels et les tâches, dans vos langues.",
                "settings-about-reminders": "Rappels des dates demandées, fenêtre de Sioul fermée, et notifications regroupées.",
                "settings-about-pauses": "Temps libre et pause : quand le travail finit, ce qui vous aide, l’écran de la pause.",
                "settings-about-phone": "Ce qu’est cet appareil, et ce que son système laisse faire à Sioul : appels, notifications des autres applications, SMS, « Ne pas déranger », alarmes.",
                "settings-about-files": "Votre dossier de notes, et le partage avec vos autres appareils.",
                "settings-about-devices": "Vos appareils : ce qu’est chacun, à quoi il sert, ce qui passe devant sur chacun.",
                "settings-about-invoices": "Qui émet vos factures, leurs numéros, la TVA, la banque et les mentions légales.",
                "settings-about-ai": "Ce que les agents d’IA peuvent utiliser, projet par projet."
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
        // What the sections a link opens read, stood in for: nothing to show.
        function textWith(key, name, value) {
            return mock.text(key)
        }
        function textArgs(key, args) {
            return mock.text(key)
        }
        function reachesView() {
            return ""
        }
        function dndSetup() {
            return ""
        }
        function appNotesSetup() {
            return ""
        }
        function callsSetup() {
            return ""
        }
        function phoneMessages() {
            return ""
        }
        function texts() {
            return ""
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
            android: true
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
                const section = entry.objectName.slice("settingsSection:".length)
                verify(entry.visible, entry.text + ": shows")
                verify(at.x >= 0 && at.x + entry.width <= phone.width + 0.5, entry.text + ": within the page")
                verify(entry.height >= 44, entry.text + ": a finger's height (" + entry.height + ")")
                // Its line, in words (the stand-in gives a key back when it has no words for it).
                compare(entry.Accessible.description, mock.text("settings-about-" + section), entry.text + ": says what it holds")
                verify(entry.Accessible.description !== "" && entry.Accessible.description.indexOf("settings-about-") < 0, entry.text + ": in words")
                // Its words ask no more than the row's width: they wrap (be09e855).
                verify(entry.contentItem.implicitWidth <= entry.availableWidth + 0.5, entry.text + ": its words within the row (" + Math.round(entry.contentItem.implicitWidth) + " for " + Math.round(entry.availableWidth) + ")")
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

        function test_a_link_opens_its_section_on_a_phone_data() {
            return [
                { tag: "a setting's row", key: "time_off", section: "hours" },
                { tag: "a section", key: "invoices", section: "invoices" },
                { tag: "a section of no rows", key: "ai", section: "ai" },
                { tag: "a list of words", key: "words.voicemail.operators", section: "words" },
                { tag: "an older key, calls (a phone: This phone)", key: "calls", section: "phone" },
                { tag: "an older key, apps (Android: This phone)", key: "apps", section: "phone" },
                { tag: "an older key, dnd", key: "dnd", section: "attention" },
                { tag: "What reaches you's view", key: "attention.dnd", section: "attention" }
            ]
        }

        // From elsewhere (a notice, the Porch, another page): the section open, and Back where the
        // link was, as from any page; the list again at the next visit.
        function test_a_link_opens_its_section_on_a_phone(data) {
            mock.language = "en"
            verify(narrow.listed, "the list first")
            narrow.showSetting(data.key)
            compare(narrow.section, data.section)
            verify(!narrow.listed, "its section open")
            verify(!narrow.canGoBack, "Back leaves Settings, as from any page")
            narrow.visible = false
            narrow.visible = true
            verify(narrow.listed, "the list at the next visit")
        }

        // Opened from the list: Back returns to it; a section left open, the list at the next visit.
        function test_back_and_the_next_visit_on_a_phone() {
            mock.language = "en"
            one(narrow, "settingsSection:hours").clicked()
            verify(narrow.canGoBack, "Back to the list")
            narrow.visible = false
            narrow.visible = true
            verify(narrow.listed, "the list again")
            // On a phone, nothing of a section is made behind the list.
            compare(narrow.shown.length, 0, "no rows behind the list")
            compare(one(narrow, "settingsTabs").model.length, 0, "no tabs made on a phone")
        }
    }
}
