// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

// A door to a row of a page's ⚙ (SettingDoor.qml, then the page's
// `openSettings(true, key)`): the panel (SettingsPanel.qml, made by
// SettingsButton.qml the first time) opens at that row, the first time as
// the next ones, its rows laid out after it was asked; a move of the hand
// leaves it there. Forty made-up rows, on a stand-in for Sioul. Run by
// tools/qml-test.sh.

import QtQuick
import QtQuick.Controls.Basic
import QtTest
import "../../qml"

Item {
    id: root

    width: 1200
    height: 700

    Theme {
        id: testTheme
    }

    QtObject {
        id: mock

        function text(key) {
            return key
        }
        function textWith(key, name, value) {
            return key + " " + value
        }
        function viewFlag(name) {
            return false
        }
        function settings(view) {
            const rows = []
            for (let i = 0; i < 40; i++)
                rows.push({ key: "row." + i, kind: "bool", label: "Row " + i, help: "What row " + i + " changes, in a sentence long enough to wrap on a narrow panel.", value: false, choices: [], group: "Group " + (i - i % 6), section: "", min: 0, max: 0, step: 1, unit: "" })
            return JSON.stringify(rows)
        }
        function setSetting(key, value) {
            return ""
        }
        function changeSetting(key, shown, value) {
            return ""
        }
    }

    Component {
        id: buttonKind

        SettingsButton {
            sioul: mock
            theme: testTheme
            view: "tasks"

            // As the pages' own (TasksPage.qml, BudgetsPage.qml, TextsPage.qml, SitesPage.qml, AgendaPage.qml).
            function openSettings(open, key) {
                show(open)
                if (open && key)
                    Qt.callLater(() => scrollTo(key))
            }
        }
    }

    TestCase {
        name: "SettingsScroll"
        when: windowShown

        function panelOf(button) {
            for (let i = 0; i < button.children.length; i++) {
                if (button.children[i].item !== undefined && button.children[i].item !== null)
                    return button.children[i].item
            }
            return null
        }
        // The row's label, as far from the panel's top as it shows.
        function rowTop(panel, label) {
            const found = []
            const walk = item => {
                if (item.visible && item.text === label)
                    found.push(item)
                for (let i = 0; i < item.children.length; i++)
                    walk(item.children[i])
            }
            walk(panel.contentItem)
            return found.length === 0 ? -1 : found[0].mapToItem(panel.contentItem, 0, 0).y
        }

        function test_the_first_open_lands_on_the_row() {
            const button = createTemporaryObject(buttonKind, root)
            button.openSettings(true, "row.30")
            tryVerify(() => panelOf(button) !== null)
            tryVerify(() => panelOf(button).opened)
            const flick = panelOf(button).contentItem.contentItem
            tryVerify(() => flick.contentY > 0, 2000, "scrolled, the first time")
            wait(500)
            const top = rowTop(panelOf(button), "Row 30")
            verify(top >= 0 && top < panelOf(button).height / 3, "Row 30 near the top: " + top)
            // Closed, then opened again at another row.
            button.show(false)
            tryVerify(() => !panelOf(button).visible)
            button.openSettings(true, "row.12")
            wait(500)
            const again = rowTop(panelOf(button), "Row 12")
            verify(again >= 0 && again < panelOf(button).height / 3, "Row 12 near the top: " + again)
        }

        function test_the_hand_takes_over() {
            const button = createTemporaryObject(buttonKind, root)
            button.openSettings(true, "row.30")
            tryVerify(() => panelOf(button) !== null && panelOf(button).opened)
            const flick = panelOf(button).contentItem.contentItem
            flick.movementStarted()
            compare(panelOf(button).wanted, "", "followed no more")
            button.show(false)
        }
    }
}
