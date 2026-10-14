// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

// Sioul's icon in the tray (qml-desktop/Tray.qml): its menu holds Close the
// work day and Close the whole day, each opening its sheet in the window, in
// a short menu (the window, the two closings, Quit); a click on one of its
// messages goes to the window, which opens what the notice offered (on
// Windows, the end of work's and the night's notices: main.qml's
// `trayMessageClicked`). On a stand-in for the window and for Sioul, which
// record what they are asked. Run by tools/qml-test.sh.

import QtQuick
import QtTest
import "../../qml-desktop"

TestCase {
    id: root

    name: "Tray"

    QtObject {
        id: shownWindow

        property bool visible: true
        property var asked: []

        function reviewFromTray(kind) {
            shownWindow.asked = shownWindow.asked.concat(["review " + kind])
        }
        function toggleShown() {
            shownWindow.asked = shownWindow.asked.concat(["toggle"])
        }
        function quitSioul() {
            shownWindow.asked = shownWindow.asked.concat(["quit"])
        }
        function trayMessageClicked() {
            shownWindow.asked = shownWindow.asked.concat(["message"])
        }
    }

    // Sioul's words by their ids, as `sioul.text` gives them.
    QtObject {
        id: words

        function text(id) {
            return id
        }
    }

    Tray {
        id: tray

        window: shownWindow
        sioul: words
    }

    function entry(name) {
        for (const item of tray.menu.items) {
            if (item.objectName === name)
                return item
        }
        return null
    }

    function test_the_menu_closes_the_work_day_or_the_whole_day() {
        shownWindow.asked = []
        const work = root.entry("closeWorkDay")
        const night = root.entry("closeWholeDay")
        verify(work !== null && night !== null)
        compare(work.text, "review-close-work")
        compare(night.text, "review-close-night")
        // As a click in the menu does (the item's activate, a private slot, emits it).
        work.triggered()
        night.triggered()
        compare(shownWindow.asked, ["review work", "review night"])
    }

    function test_the_menu_stays_short() {
        // The window, the two closings, Quit: four entries, two separators.
        const entries = tray.menu.items.filter(item => !item.separator)
        compare(entries.map(item => item.text), ["tray-hide", "review-close-work", "review-close-night", "tray-quit"])
        compare(tray.menu.items.length, 6)
    }

    function test_a_message_clicked_goes_to_the_window() {
        shownWindow.asked = []
        tray.messageClicked()
        compare(shownWindow.asked, ["message"])
    }
}
