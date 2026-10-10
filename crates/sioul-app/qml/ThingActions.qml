// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

// The two buttons every card ends with: something new tied to it, or a tie
// to something that exists. Right click in a list offers the same.

pragma ComponentBehavior: Bound

import QtQuick
import QtQuick.Controls.Basic
import QtQuick.Layouts

RowLayout {
    id: actions

    // Sioul's backend (backend.rs): its words in your language and what it does.
    required property var sioul
    // The window's colours, sizes and fonts (Theme.qml).
    required property var theme
    // The main window (main.qml), which opens what is asked of it. Its "Link to…"
    // chooser ties the thing open to another.
    required property var window
    // The thing open: {uri, kind, key, title, start, name, address}.
    property var source: null

    spacing: 6

    Button {
        id: addButton

        enabled: actions.source !== null
        flat: true
        text: actions.sioul.text("ui-add-new") + "  ▾"
        icon.name: "list-add"
        icon.color: actions.theme.text
        onClicked: addMenu.now().popup(addButton, 0, addButton.height)
    }
    Button {
        enabled: actions.source !== null && actions.source.uri !== ""
        flat: true
        text: actions.sioul.text("ui-link-existing")
        icon.name: "insert-link"
        icon.color: actions.theme.text
        onClicked: actions.window.linkFrom(actions.source)
    }

    Later {
        id: addMenu

        sourceComponent: Component {
            AddMenu {
                id: addMenuForm

                sioul: actions.sioul
                window: actions.window
                source: actions.source
            }
        }
    }
}
