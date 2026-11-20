// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

// The buttons every card ends with: something new tied to it, a tie to
// something that exists, and its ties as a chain (ChainPage.qml; Ctrl+G).
// Right click in a list offers the same. As wide as the layout it sits in,
// they go on to a second line when that is narrower than the three side by
// side (Notes' column beside the note, 260 points): a row of them would set
// the column's least width, and everything in it would pass its edge.

pragma ComponentBehavior: Bound

import QtQuick
import QtQuick.Controls.Basic
import QtQuick.Layouts

Flow {
    id: actions

    Layout.fillWidth: true

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

    // The thing shown with these buttons: the one Ctrl+G opens the ties of.
    function told() {
        if (actions.visible && actions.source !== null && (actions.source.uri || "") !== "" && actions.window.thingShown !== undefined)
            actions.window.thingShown = actions.source
    }

    onSourceChanged: actions.told()
    onVisibleChanged: actions.told()

    Button {
        id: addButton

        objectName: "thingAdd"
        enabled: actions.source !== null
        flat: true
        text: actions.sioul.text("ui-add-new") + "  ▾"
        icon.name: "list-add"
        icon.color: actions.theme.text
        onClicked: addMenu.now().popup(addButton, 0, addButton.height)
    }
    Button {
        objectName: "thingLink"
        enabled: actions.source !== null && actions.source.uri !== ""
        flat: true
        text: actions.sioul.text("ui-link-existing")
        icon.name: "insert-link"
        icon.color: actions.theme.text
        onClicked: actions.window.linkFrom(actions.source)
    }
    // Its ties as a chain: what it is tied to, and what those are tied to.
    Button {
        objectName: "thingTies"
        enabled: actions.source !== null && (actions.source.uri || "") !== ""
        flat: true
        text: actions.sioul.text("ui-ties")
        icon.name: "view-list-tree"
        icon.color: actions.theme.text
        onClicked: actions.window.openChain(actions.source.uri)
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
