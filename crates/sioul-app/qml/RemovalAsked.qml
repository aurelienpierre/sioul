// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

// Before "Remove its files" (docs/database.md, "Removing a device's files"):
// what goes, with its size, and what stays, in plain words; then the press,
// or Cancel. Asked where it was pressed: on a device's card (DevicesTab.qml),
// among the devices that no longer count and in what waits to leave the
// server (SharePanel.qml). Its host says what would go (`removal`, read
// when it was pressed), which device is asked about (`removing`), and reads
// its list again once it is done (`reload`).

import QtQuick
import QtQuick.Controls.Basic
import QtQuick.Layouts

ColumnLayout {
    id: asked

    // Where it is said: the sharing panel, or the devices' tab.
    required property var host
    // The device's id.
    required property string deviceId

    spacing: 4

    Label {
        Layout.fillWidth: true
        text: asked.host.removal.text || ""
        textFormat: Text.PlainText
        wrapMode: Text.Wrap
        font.pixelSize: 13
        color: asked.host.theme.text
    }
    RowLayout {
        spacing: 6

        Button {
            text: asked.host.sioul.text("share-device-remove-confirm")
            onClicked: {
                asked.host.problem = asked.host.sioul.deviceRemoveFiles(asked.deviceId)
                asked.host.removing = ""
                asked.host.reload()
            }
        }
        Button {
            flat: true
            text: asked.host.sioul.text("ui-cancel")
            onClicked: asked.host.removing = ""
        }
    }
}
