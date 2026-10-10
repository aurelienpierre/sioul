// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

// Words from outside drawn as words (the security review of 10 October
// 2026): a name, a subject or an event's title holding <img src="http://…">,
// as a stranger, a hostile server or a shared calendar writes it, shown by
// Sioul's buttons, menus, tooltips, dialog titles, lists and labels, is never
// fetched, and a button holding it does not crash (Qt 6.11 crashed on such a
// button, its text guessed rich). A stand-in web server, tools/qml-stand-in.py,
// which tools/qml-test.sh starts for this file, notes what is asked of it; a
// Label left to guess rich text fetches its picture there, to show that a
// fetch would be seen. Run by tools/qml-test.sh.

pragma ComponentBehavior: Bound

import QtQuick
import QtQuick.Controls.Basic
import QtQuick.Layouts
import QtTest
import "../../qml"

Item {
    id: root

    width: 900
    height: 900

    readonly property string server: "http://127.0.0.1:18731"

    // Words holding a picture on the stand-in, named after where they are shown.
    function picture(name) {
        return "<img src=\"" + root.server + "/" + name + ".png\">"
    }

    Theme {
        id: testTheme
    }

    // A stand-in for Sioul: its sentences are their keys, an argument put in
    // as Rust puts it (no isolation marks around it).
    QtObject {
        id: mock

        function text(key) {
            return key
        }
        function textWith(key, name, value) {
            return key + " " + value
        }
        // reaches.rs's `person`: "How <name> reaches you", a contact's name, a list's.
        function personSheet(key, addresses) {
            return JSON.stringify({ title: "How " + root.picture("person-sheet-title") + " reaches you", name: root.picture("person-sheet-name"), who: "stranger", said: root.picture("person-sheet-said"), choice: "x", choices: [{ value: "x", label: root.picture("person-sheet-choice") }], own: [], always: false, always_help: "", lines: [], note: "", calls_title: "", calls: [], texts_title: "", texts: [] })
        }
        function personSheetChange(key, addresses, verb, value) {
            return "null"
        }
    }

    ColumnLayout {
        anchors.fill: parent
        spacing: 4

        // The Porch's "Open <event's title>" for two events at once, a card's
        // "How <name> reaches you" (PorchPage, ContactsPage, ReachesTab).
        Button {
            text: testTheme.plain(mock.textWith("overlap-open", "title", root.picture("porch-button")))
        }
        // An action of the Reader, by its name; narrow, its name on hover (Reader.qml).
        ActionButton {
            theme: testTheme
            iconName: "mail-reply-sender"
            label: root.picture("action-button")
        }
        ActionButton {
            id: action

            theme: testTheme
            iconName: "mail-reply-all"
            label: root.picture("action-button-tooltip")
            compact: true
        }
        // A setting's sentence, a contact's name to keep (ContactDuplicates).
        WrapCheckBox {
            text: root.picture("wrap-check-box")
        }
        RadioButton {
            text: testTheme.plain(root.picture("radio-button"))
        }
        // A site's name, its tooltip (SitesPage).
        ItemDelegate {
            id: delegate

            text: testTheme.plain(root.picture("item-delegate"))
            ToolTip.visible: true
            ToolTip.text: testTheme.plain(root.picture("item-delegate-tooltip"))
        }
        Label {
            text: root.picture("label-plain")
            textFormat: Text.PlainText
        }
        // A calendar's, a folder's, a category's names to choose from.
        ComboBox {
            id: combo

            model: [root.picture("combo-first"), root.picture("combo-second")].map(name => testTheme.plain(name))
        }
        ComboBox {
            id: roles

            textRole: "label"
            valueRole: "id"
            model: [{ id: "a", label: root.picture("combo-role") }].map(c => Object.assign({}, c, { label: testTheme.plain(c.label) }))
        }
        // The control: a Label left to guess, as before the review. Its picture is fetched.
        Label {
            text: root.picture("control-label-autotext")
        }
    }

    // A menu of sites and the line that opens it, as Sites ▸ Usual sites draws them.
    SioulMenu {
        id: menu

        PresetGroupMenu {
            id: group

            sioul: mock
            group: ({ name: root.picture("menu-title"), sites: [{ name: root.picture("menu-item"), kept: false }] })
        }
    }

    ConfirmDialog {
        id: confirm

        sioul: mock
        theme: testTheme
    }

    TestCase {
        name: "PlainText"
        when: windowShown

        property var seen: null

        // What the stand-in was asked, in order.
        function asked() {
            seen = null
            const request = new XMLHttpRequest()
            request.onreadystatechange = () => {
                if (request.readyState === XMLHttpRequest.DONE)
                    seen = request.status === 200 ? request.responseText.split("\n").filter(line => line !== "") : ["(no answer: " + request.status + ")"]
            }
            request.open("GET", root.server + "/seen")
            request.send()
            tryVerify(() => seen !== null, 5000, "the stand-in answers")
            return seen
        }

        function test_outside_words_are_drawn_and_never_fetched() {
            // Everything shown: the tooltips, the lists opened, the menus, the dialogs.
            action.ToolTip.visible = true
            combo.popup.open()
            roles.popup.open()
            menu.popup(root, 10, 10)
            group.popup(root, 300, 10)
            confirm.ask(root.picture("confirm-title"), root.picture("confirm-sentence"), root.picture("confirm-action"), false)
            const component = Qt.createComponent("../../qml/PersonSheet.qml")
            verify(component.status === Component.Ready, component.errorString())
            const sheet = component.createObject(root, { sioul: mock, theme: testTheme })
            verify(sheet !== null)
            sheet.ask("", "[\"a@example.org\"]")
            tryVerify(() => sheet.opened && confirm.opened && combo.popup.opened && menu.opened && action.ToolTip.toolTip.opened, 3000, "all shown")
            // The control's picture asked: a fetch is seen when there is one.
            tryVerify(() => asked().indexOf("/control-label-autotext.png") >= 0, 5000, "the stand-in sees what Qt fetches")
            wait(500)
            compare(asked().filter(path => path !== "/control-label-autotext.png"), [], "no picture of the words shown fetched")
            // Shown as words: the picture's address is there to read.
            verify(delegate.text.indexOf("item-delegate.png") > 0)
            sheet.close()
            confirm.close()
            menu.close()
        }
    }
}
