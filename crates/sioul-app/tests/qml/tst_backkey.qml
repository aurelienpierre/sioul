// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

// Back closes what is open on top first (24 November 2026: with a page's ⚙
// open, Android's Back sent Sioul to the home screen). Qt's popups close on
// Escape, and on Android's Back, only while they hold the keyboard's focus
// (QQuickPopup::keyPressEvent: Android's build compares the key with
// Qt::Key_Back beside QKeySequence::Cancel), and while one that closes on
// Escape is open, the window's own Back shortcut is held back
// (QQuickShortcutContext). So each kind of popup must take the focus as it
// opens: a page's ⚙ (SettingsPanel.qml through SettingsButton.qml), a
// SafePopup (the Porch's "?"), a SafeDialog, a menu (SioulMenu.qml). Here,
// on a computer's Qt, Escape stands for Back: the same branch of Qt's code,
// and the page under the popup must never hear it. Run by tools/qml-test.sh.

import QtQuick
import QtQuick.Controls.Basic
import QtTest
import "../../qml"

Item {
    id: root

    width: 412
    height: 891

    // What reached the page under the popups: the keys a popup left to it.
    property var heard: []

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
            return JSON.stringify([{ key: "row.0", kind: "bool", label: "Row 0", help: "What it changes.", value: false, choices: [], group: "Group", section: "", min: 0, max: 0, step: 1, unit: "" }])
        }
        function setSetting(key, value) {
            return ""
        }
        function changeSetting(key, shown, value) {
            return ""
        }
    }

    // The page: it holds the keyboard's focus until a popup takes it, and says what reached it.
    Rectangle {
        id: page

        anchors.fill: parent
        focus: true
        color: "white"
        Keys.onPressed: event => root.heard = root.heard.concat([event.key])
    }

    Component {
        id: buttonKind

        SettingsButton {
            sioul: mock
            theme: testTheme
            view: "tasks"
        }
    }

    Component {
        id: popupKind

        // As the Porch's "?" (PorchPage.qml): modal, over the window, nothing set for the focus.
        SafePopup {
            parent: Overlay.overlay
            modal: true
            width: 300
            height: 200

            Label {
                text: "Help"
            }
        }
    }

    Component {
        id: dialogKind

        // As most forms (ConfirmDialog.qml, BudgetDialog.qml…): nothing set for the focus.
        SafeDialog {
            property bool cancelled: false

            parent: Overlay.overlay
            modal: true
            standardButtons: Dialog.Cancel | Dialog.Ok
            onRejected: cancelled = true

            Label {
                text: "A question"
            }
        }
    }

    Component {
        id: menuKind

        SioulMenu {
            MenuItem {
                text: "One"
            }
            MenuItem {
                text: "Two"
            }
        }
    }

    TestCase {
        name: "BackKey"
        when: windowShown

        function init() {
            root.heard = []
            page.forceActiveFocus()
        }

        function panelOf(button) {
            for (let i = 0; i < button.children.length; i++) {
                if (button.children[i].item !== undefined && button.children[i].item !== null)
                    return button.children[i].item
            }
            return null
        }

        // A page's ⚙: the panel made in its Loader, over the overlay, not modal.
        function test_a_page_s_settings_take_the_focus_and_close_first() {
            const button = createTemporaryObject(buttonKind, page)
            button.show(true)
            tryVerify(() => panelOf(button) !== null && panelOf(button).opened)
            const panel = panelOf(button)
            tryVerify(() => panel.activeFocus, 2000, "the panel holds the focus")
            keyClick(Qt.Key_Escape)
            tryVerify(() => !panel.visible, 2000, "closed first")
            compare(root.heard.length, 0, "the page under it heard nothing")
        }

        function test_a_popup_takes_the_focus_and_closes_first() {
            const popup = createTemporaryObject(popupKind, root)
            popup.open()
            tryVerify(() => popup.opened)
            verify(popup.activeFocus, "the popup holds the focus")
            keyClick(Qt.Key_Escape)
            tryVerify(() => !popup.visible, 2000, "closed first")
            compare(root.heard.length, 0, "the page under it heard nothing")
        }

        function test_a_dialog_takes_the_focus_and_is_cancelled_first() {
            const dialog = createTemporaryObject(dialogKind, root)
            dialog.open()
            tryVerify(() => dialog.opened)
            verify(dialog.activeFocus, "the dialog holds the focus")
            keyClick(Qt.Key_Escape)
            tryVerify(() => !dialog.visible, 2000, "closed first")
            verify(dialog.cancelled, "as Cancel: nothing done")
            compare(root.heard.length, 0, "the page under it heard nothing")
        }

        function test_a_menu_takes_the_focus_and_closes_first() {
            const menu = createTemporaryObject(menuKind, page)
            menu.popup(10, 10)
            tryVerify(() => menu.opened)
            verify(menu.activeFocus, "the menu holds the focus")
            keyClick(Qt.Key_Escape)
            tryVerify(() => !menu.visible, 2000, "closed first")
            compare(root.heard.length, 0, "the page under it heard nothing")
        }

        // Two at once (a dialog over a page's ⚙): the top one first, then the one under it.
        function test_the_top_most_closes_first() {
            const button = createTemporaryObject(buttonKind, page)
            button.show(true)
            tryVerify(() => panelOf(button) !== null && panelOf(button).opened)
            const panel = panelOf(button)
            const dialog = createTemporaryObject(dialogKind, root)
            dialog.open()
            tryVerify(() => dialog.opened && dialog.activeFocus)
            keyClick(Qt.Key_Escape)
            tryVerify(() => !dialog.visible, 2000, "the dialog first")
            verify(panel.visible, "the panel stays")
            tryVerify(() => panel.activeFocus, 2000, "the focus back on the panel")
            keyClick(Qt.Key_Escape)
            tryVerify(() => !panel.visible, 2000, "then the panel")
            compare(root.heard.length, 0, "the page under them heard nothing")
        }
    }
}
