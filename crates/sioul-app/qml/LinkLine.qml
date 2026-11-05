// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

// A share's link (an agenda's read-only link, your availability's): the
// address in a read-only field that selects, from its start, and Copy beside
// it, which puts it on the clipboard (`copyText`) and then says "Copied." a
// moment. Where the clipboard did not take it, the address is selected, to
// copy by hand.

pragma ComponentBehavior: Bound

import QtQuick
import QtQuick.Controls.Basic
import QtQuick.Layouts

RowLayout {
    id: line

    required property var sioul
    required property var theme
    property string link: ""
    // Copied, and said so a moment.
    readonly property bool copied: said.running

    spacing: 6
    onLinkChanged: said.stop()

    TextField {
        id: field

        objectName: "linkField"
        Layout.fillWidth: true
        Layout.preferredWidth: 1
        Layout.minimumWidth: 80
        readOnly: true
        selectByMouse: true
        text: line.link
        color: line.theme.text
        Accessible.name: line.link
        // Read from its start: the server's name first.
        onActiveFocusChanged: {
            if (!field.activeFocus)
                field.cursorPosition = 0
        }
        onTextChanged: {
            if (!field.activeFocus)
                field.cursorPosition = 0
        }
        Component.onCompleted: field.cursorPosition = 0
    }
    Button {
        objectName: "linkCopy"
        flat: true
        implicitWidth: implicitContentWidth + leftPadding + rightPadding
        icon.name: "edit-copy"
        icon.color: line.theme.text
        text: line.sioul.text(said.running ? "ui-copied" : "ui-copy")
        Accessible.description: line.link
        onClicked: {
            if (line.sioul.copyText(line.link)) {
                said.restart()
            } else {
                field.forceActiveFocus()
                field.selectAll()
            }
        }
    }
    Timer {
        id: said

        interval: 2000
    }
}
