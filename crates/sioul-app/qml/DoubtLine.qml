// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

// Under a dose not answered here: why Sioul cannot tell whether it was taken
// (never "not taken"), or the answers your devices gave that differ; and for
// each device the doubt names, "This device is off" (docs/health.md,
// "Knowing"): said off, it is not counted until it says anything newer. It
// never answers the dose for you. Warm, never red.

pragma ComponentBehavior: Bound

import QtQuick
import QtQuick.Controls.Basic
import QtQuick.Layouts

ColumnLayout {
    id: line

    required property var sioul
    required property var theme
    // The sentence; "" when the dose is known.
    property string doubt: ""
    // [{id, label}]: a button each.
    property var offs: []

    // A device said off: the page reads the doses again.
    signal saidOff()

    visible: doubt !== ""
    spacing: 2

    Label {
        Layout.fillWidth: true
        Layout.preferredWidth: 1
        text: line.doubt
        textFormat: Text.PlainText
        wrapMode: Text.Wrap
        font.pixelSize: 13
        color: line.theme.warm
    }
    Flow {
        visible: line.offs.length > 0
        Layout.fillWidth: true
        spacing: 6

        Repeater {
            model: line.offs

            Button {
                id: offButton

                required property var modelData

                flat: true
                text: offButton.modelData.label
                font.pixelSize: 13
                onClicked: {
                    line.sioul.deviceOff(offButton.modelData.id, true)
                    line.saidOff()
                }
            }
        }
    }
}
