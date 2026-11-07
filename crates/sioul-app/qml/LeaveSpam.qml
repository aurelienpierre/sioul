// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

// Leaving the lists of the mail you called spam, in Mail ▸ ⚙ ▸ Lists you
// left (crates/sioul-app/src/leave.rs, docs/client.md, "Unsubscribing"): a
// row when there are some, found off the window's thread; it opens a small
// sheet with their names and the one button. What is done then waits ten
// seconds with Undo in the status line, and a failure of the last try is said
// under its list's name.

pragma ComponentBehavior: Bound

import QtQuick
import QtQuick.Controls.Basic
import QtQuick.Layouts

ColumnLayout {
    id: leave

    objectName: "leaveSpam"

    required property var sioul
    required property var theme
    // What it does, in a sentence: the setting's own (settings.rs), said in the sheet.
    property string about: ""
    // The section's title, when the panel shows none for this row (no list left yet).
    property string title: ""
    // leave.rs's `Found`.
    property var found: ({ count: 0, row: "", button: "", lists: [], ended: false })
    // The sheet with their names.
    property alias sheet: sheet

    // A run ended: the lists you left changed.
    signal ended()

    visible: leave.found.count > 0
    spacing: 6
    Component.onCompleted: leave.sioul.leaveSpamLook()

    Connections {
        target: leave.sioul

        function onLeaveSpamFound(text) {
            leave.found = JSON.parse(text || "null") || leave.found
            if (leave.found.count === 0)
                sheet.close()
            if (leave.found.ended)
                leave.ended()
        }
    }

    Label {
        visible: leave.title !== ""
        Layout.fillWidth: true
        Layout.topMargin: 6
        text: leave.title
        textFormat: Text.PlainText
        font.pixelSize: 16
        font.weight: Font.DemiBold
        elide: Text.ElideRight
        color: leave.theme.accent
    }
    Button {
        id: openSheet

        Layout.fillWidth: true
        implicitHeight: Math.max(leave.theme.touch ? 48 : 32, rowText.implicitHeight + topPadding + bottomPadding)
        text: leave.found.row
        Accessible.name: leave.found.row
        onClicked: sheet.open()

        contentItem: Label {
            id: rowText

            text: openSheet.text
            textFormat: Text.PlainText
            wrapMode: Text.Wrap
            verticalAlignment: Text.AlignVCenter
            color: leave.theme.text
        }

        // Over the whole window, whatever holds the row (a layout would claim its place).
        SafeDialog {
            id: sheet

            parent: Overlay.overlay
            anchors.centerIn: parent
            modal: true
            title: leave.sioul.text("set-leave-spam")
            width: Math.min(460, (parent ? sheet.roomWidth : 460) - 2 * leave.theme.gap)
            padding: leave.theme.gap

            contentItem: ColumnLayout {
                spacing: 8

                Label {
                    Layout.fillWidth: true
                    text: leave.about
                    textFormat: Text.PlainText
                    wrapMode: Text.Wrap
                    font.pixelSize: 13
                    lineHeight: 1.25
                    color: leave.theme.muted
                }
                ScrollView {
                    id: names

                    Layout.fillWidth: true
                    Layout.fillHeight: true
                    Layout.preferredHeight: Math.min(rows.implicitHeight, sheet.roomHeight * 0.5)
                    contentWidth: availableWidth
                    clip: true

                    ColumnLayout {
                        id: rows

                        width: names.availableWidth
                        spacing: 6

                        Repeater {
                            model: leave.found.lists

                            delegate: ColumnLayout {
                                id: named

                                required property var modelData

                                Layout.fillWidth: true
                                spacing: 1

                                Label {
                                    Layout.fillWidth: true
                                    // Names and domains come from the messages.
                                    text: named.modelData.name
                                    textFormat: Text.PlainText
                                    wrapMode: Text.Wrap
                                    color: leave.theme.text
                                }
                                Label {
                                    Layout.fillWidth: true
                                    text: named.modelData.domain
                                    textFormat: Text.PlainText
                                    elide: Text.ElideMiddle
                                    font.pixelSize: 13
                                    color: leave.theme.muted
                                }
                                Label {
                                    visible: named.modelData.why !== ""
                                    Layout.fillWidth: true
                                    text: named.modelData.why
                                    textFormat: Text.PlainText
                                    wrapMode: Text.Wrap
                                    font.pixelSize: 13
                                    lineHeight: 1.2
                                    color: leave.theme.muted
                                }
                            }
                        }
                    }
                }
            }

            footer: DialogButtonBox {
                Button {
                    implicitHeight: leave.theme.touch ? 48 : implicitContentHeight + topPadding + bottomPadding
                    text: leave.theme.plain(leave.found.button)
                    highlighted: true
                    DialogButtonBox.buttonRole: DialogButtonBox.AcceptRole
                }
                Button {
                    implicitHeight: leave.theme.touch ? 48 : implicitContentHeight + topPadding + bottomPadding
                    text: leave.sioul.text("ui-cancel")
                    DialogButtonBox.buttonRole: DialogButtonBox.RejectRole
                }
            }

            onAccepted: leave.sioul.leaveSpamRun(JSON.stringify(leave.found.lists.map(list => list.key)))
        }
    }
}
