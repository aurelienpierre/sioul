// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

// The two buttons every card ends with: something new tied to it, or a tie
// to something that exists. Right click in a list offers the same.

import QtQuick
import QtQuick.Controls.Basic
import QtQuick.Layouts

RowLayout {
    id: actions

    required property var sioul
    required property var theme
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
        onClicked: addMenu.popup(addButton, 0, addButton.height)
    }
    Button {
        enabled: actions.source !== null && actions.source.uri !== ""
        flat: true
        text: actions.sioul.text("ui-link-existing")
        icon.name: "insert-link"
        icon.color: actions.theme.text
        onClicked: actions.window.linkFrom(actions.source)
    }

    AddMenu {
        id: addMenu

        sioul: actions.sioul
        window: actions.window
        source: actions.source
    }
}
