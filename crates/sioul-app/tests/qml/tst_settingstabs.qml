// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

// Settings' tabs on a phone's width (ParametersPage.qml), on a stand-in for
// Sioul, in English and French: with Devices among them, every tab shows
// whole within the page, none past its edge nor over another, each tall
// enough for a finger, in a few rows that leave the page room under them;
// pressing one shows it. A phone also lists "This phone", which only
// Android does: the row here holds one tab fewer than a phone's. Run by
// tools/qml-test.sh. To see the tabs, set `pictures` below to a folder (a
// change of your own, not to commit).

import QtQuick
import QtQuick.Controls.Basic
import QtTest
import "../../qml"

Item {
    id: root

    // Where the pictures go; none when empty.
    readonly property string pictures: ""

    // A phone's screen, in points.
    width: 412
    height: 915

    Theme {
        id: testTheme

        compact: true
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
                "settings-tab-ai": "AI agents"
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
                "settings-tab-ai": "Agents d’IA"
            }
        })

        function text(key) {
            return mock.words[mock.language][key] || key
        }
        function settings(view) {
            return "[]"
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

    ParametersPage {
        id: page

        anchors.fill: parent
        sioul: mock
        theme: testTheme
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
        function tabs() {
            const names = page.sections.map(s => mock.text(page.tabKey(s)))
            return walk(page, i => i.visible && i.checkable === true && i.clicked !== undefined && names.indexOf(i.text) >= 0, [])
        }
        function save(name) {
            if (root.pictures === "")
                return
            wait(100)
            grabImage(root).save(root.pictures + "/" + name + ".png")
        }

        function test_every_tab_fits_a_phone_s_width_data() {
            return [{ tag: "en", language: "en" }, { tag: "fr", language: "fr" }]
        }

        function test_every_tab_fits_a_phone_s_width(data) {
            mock.language = data.language
            page.section = "look"
            wait(100)
            const shown = tabs()
            compare(shown.length, page.sections.length, "every tab shows")
            verify(page.sections.indexOf("devices") >= 0, "Devices among them")
            const edge = root.width
            const boxes = shown.map(t => {
                const at = t.mapToItem(root, 0, 0)
                return { text: t.text, x: at.x, y: at.y, w: t.width, h: t.height, full: t.implicitWidth }
            })
            for (const b of boxes) {
                verify(b.x >= 0 && b.x + b.w <= edge + 0.5, b.text + ": within the page")
                verify(b.w + 0.5 >= b.full, b.text + ": its words whole")
                verify(b.h >= 36, b.text + ": a finger's height (" + b.h + ")")
            }
            for (let i = 0; i < boxes.length; ++i)
                for (let j = i + 1; j < boxes.length; ++j) {
                    const a = boxes[i], b = boxes[j]
                    verify(a.x + a.w <= b.x + 0.5 || b.x + b.w <= a.x + 0.5 || a.y + a.h <= b.y + 0.5 || b.y + b.h <= a.y + 0.5, a.text + " and " + b.text + " apart")
                }
            // A few rows, leaving most of the screen to the tab shown.
            const rows = new Set(boxes.map(b => Math.round(b.y))).size
            const bottom = Math.max(...boxes.map(b => b.y + b.h))
            console.log("settings tabs at 412 points, " + data.language + ": " + rows + " rows, down to " + Math.round(bottom))
            verify(rows <= 5, rows + " rows")
            verify(bottom <= root.height * 0.4, "the tabs end at " + Math.round(bottom))
            save("settings-tabs-" + data.language)
            // Devices pressed: shown.
            shown.find(t => t.text === mock.text("settings-tab-devices")).clicked()
            compare(page.section, "devices")
        }
    }
}
