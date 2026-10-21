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
        // The status line's moment, its `dnd` part (everywhere.rs's `moment`).
        property var moment: ({ dnd: { on: false, state: "usual", paused: false } })

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

        property var pressed: []

        function text(id) {
            return id
        }
        function reachChoose(state, length) {
            words.pressed = words.pressed.concat([state + " " + length])
        }
        function dndToggle(on, minutes) {
            words.pressed = words.pressed.concat(["dnd " + on + " " + minutes])
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
        // The window, the three states, the two closings, Quit: seven entries, three separators.
        const entries = tray.menu.items.filter(item => !item.separator)
        compare(entries.map(item => item.text), ["tray-hide", "reach-for-60", "reach-usual", "reach-menu-dnd", "review-close-work", "review-close-night", "tray-quit"])
        compare(tray.menu.items.length, 10)
    }

    // The three states (part 5 of "What reaches you"): the one now ticked, each pressed; no Reachable in a pause.
    function test_the_menu_offers_the_three_states() {
        words.pressed = []
        verify(root.entry("trayUsual").checked && !root.entry("trayReachable").checked && !root.entry("trayDnd").checked)
        root.entry("trayReachable").triggered()
        root.entry("trayDnd").triggered()
        compare(words.pressed, ["reachable 60", "dnd true 60"])
        shownWindow.moment = { dnd: { on: true, state: "dnd", paused: true } }
        verify(root.entry("trayDnd").checked && !root.entry("trayUsual").checked)
        verify(!root.entry("trayReachable").visible, "never during a pause")
        root.entry("trayUsual").triggered()
        compare(words.pressed[words.pressed.length - 1], "dnd false 0", "as usual: do-not-disturb off")
        shownWindow.moment = { dnd: { on: false, state: "reachable", paused: false } }
        verify(root.entry("trayReachable").checked)
        root.entry("trayUsual").triggered()
        compare(words.pressed[words.pressed.length - 1], "usual ", "as usual: Reachable ended")
        shownWindow.moment = { dnd: { on: false, state: "usual", paused: false } }
    }

    function test_a_message_clicked_goes_to_the_window() {
        shownWindow.asked = []
        tray.messageClicked()
        compare(shownWindow.asked, ["message"])
    }
}
