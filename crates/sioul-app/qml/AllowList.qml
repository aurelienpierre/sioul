// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

// "What Android must allow", in this phone's card in Settings ▸ Devices
// (docs/android.md, "What Android withholds"): one line per access, in the
// order the core gives (the role for calls, contacts, the call log,
// notification access, the texts' permissions, do-not-disturb access, exact
// alarms, no battery restriction; the permission to modify contacts while
// starred people both ways is on), each with what it is for in a few words,
// its state in a word (given, not given, cannot tell) and one button, which
// asks Android or opens its page (`phonecard::checklist`, `phone_allow`).
// Never counted, never red: a state not given is said in the warm colour.

pragma ComponentBehavior: Bound

import QtQuick
import QtQuick.Controls.Basic
import QtQuick.Layouts

ColumnLayout {
    id: list

    // Sioul's backend (backend.rs): its words in your language.
    required property var sioul
    // The window's colours, sizes and fonts (Theme.qml).
    required property var theme
    // The lines: [{what, label, for, state, said, button}].
    property var lines: []
    // A phone's width: each line's state and button under its words.
    property bool narrow: false

    // A line's button pressed: its access's name.
    signal ask(string what)

    spacing: 10

    Repeater {
        model: list.lines

        delegate: GridLayout {
            id: access

            required property var modelData

            objectName: "allow-" + access.modelData.what
            Layout.fillWidth: true
            columns: list.narrow ? 2 : 3
            columnSpacing: 10
            rowSpacing: 2

            ColumnLayout {
                Layout.fillWidth: true
                Layout.columnSpan: list.narrow ? 2 : 1
                spacing: 0

                Label {
                    Layout.fillWidth: true
                    text: access.modelData.label
                    textFormat: Text.PlainText
                    wrapMode: Text.Wrap
                    font.weight: Font.DemiBold
                    color: list.theme.text
                }
                Label {
                    Layout.fillWidth: true
                    text: access.modelData.for
                    textFormat: Text.PlainText
                    wrapMode: Text.Wrap
                    font.pixelSize: 13
                    color: list.theme.muted
                }
            }
            Label {
                objectName: "allowState"
                Layout.preferredWidth: list.narrow ? -1 : 120
                Layout.fillWidth: list.narrow
                text: access.modelData.said
                textFormat: Text.PlainText
                wrapMode: Text.Wrap
                color: access.modelData.state === "not-given" ? list.theme.warm : access.modelData.state === "given" ? list.theme.accent : list.theme.muted
            }
            Button {
                objectName: "allowButton"
                Layout.alignment: Qt.AlignRight | Qt.AlignVCenter
                flat: access.modelData.state === "given"
                text: list.theme.plain(access.modelData.button)
                Accessible.name: access.modelData.label + ": " + access.modelData.button
                onClicked: list.ask(access.modelData.what)
            }
        }
    }
    // Sioul's own notifications: Android's page for them.
    Button {
        objectName: "allowNotices"
        flat: true
        text: list.sioul.text("attention-phone-open-notifications")
        onClicked: list.sioul.wakeSettings("notifications")
    }
}
