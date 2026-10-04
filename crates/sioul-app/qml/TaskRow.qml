// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

// One task: a circle to tick, its title, and below, quietly, what matters
// about it: the date asked as time left, how long it takes, what it waits
// for. Nothing is red; a date within a week is in a warmer tone. The same
// task looks the same everywhere: in the list, on the board, in a panel.

pragma ComponentBehavior: Bound

import QtQuick
import QtQuick.Controls.Basic
import QtQuick.Layouts

ItemDelegate {
    id: row

    required property var task
    required property var theme
    // For the circle's name, said by screen readers in words.
    property var sioul: null
    property bool selected: false
    property bool compact: false
    // Indent for steps, in levels.
    property int depth: 0

    signal open(string uid)
    signal tick(string uid)
    signal menu(var task)

    readonly property bool done: row.task.status === "completed" || row.task.status === "cancelled"
    readonly property var details: [row.task.due, row.task.estimate, row.task.steps, row.task.waits, row.task.stopped].filter(t => t !== "")

    leftPadding: 8 + row.depth * 22
    padding: row.compact ? 4 : 8
    highlighted: row.selected
    onClicked: row.open(row.task.uid)
    Keys.onReturnPressed: row.open(row.task.uid)
    Accessible.name: row.task.title

    background: Rectangle {
        color: row.highlighted || row.hovered ? row.theme.surface : "transparent"
        radius: row.theme.radius
        border.color: row.visualFocus ? row.theme.focus : row.highlighted ? row.theme.line : "transparent"
    }

    TapHandler {
        acceptedButtons: Qt.RightButton
        onTapped: row.menu(row.task)
    }

    contentItem: RowLayout {
        spacing: 10

        // Ticked, it is done; a step started shows half full.
        AbstractButton {
            id: circle

            Layout.alignment: Qt.AlignTop
            Layout.topMargin: 2
            implicitWidth: 20
            implicitHeight: 20
            enabled: !row.task.read_only && !row.task.has_steps
            Accessible.name: row.sioul ? row.sioul.text(row.done ? "task-open-again" : "task-done") : ""
            onClicked: row.tick(row.task.uid)

            contentItem: Item {}
            background: Rectangle {
                radius: 10
                color: row.done ? row.theme.accent : "transparent"
                border.width: circle.hovered || circle.visualFocus ? 2 : 1.5
                border.color: row.task.has_steps ? row.theme.line : circle.hovered ? row.theme.accent : row.theme.muted

                // Started: the left half filled.
                Rectangle {
                    visible: row.task.status === "in-process"
                    x: 3
                    y: 3
                    width: 7
                    height: 14
                    radius: 2
                    color: row.theme.accent
                }
                Label {
                    anchors.centerIn: parent
                    visible: row.done
                    text: row.task.status === "cancelled" ? "×" : "✓"
                    textFormat: Text.PlainText
                    font.pixelSize: 13
                    color: row.theme.accentText
                }
            }
        }

        ColumnLayout {
            Layout.fillWidth: true
            spacing: 1

            RowLayout {
                Layout.fillWidth: true
                spacing: 6

                // Its kind, quietly: a call, a form, going out…
                Icon {
                    visible: !!row.task.kind
                    iconName: row.task.kind ? row.theme.taskKindIcons[row.task.kind] || "" : ""
                    size: 14
                    opacity: 0.7
                }
                Label {
                    Layout.fillWidth: true
                    text: row.task.title
                    textFormat: Text.PlainText
                    elide: Text.ElideRight
                    wrapMode: Text.Wrap
                    maximumLineCount: 2
                    font.pixelSize: row.compact ? 14 : 15
                    font.weight: row.task.has_steps ? Font.DemiBold : Font.Normal
                    font.strikeout: row.task.status === "cancelled"
                    color: row.done ? row.theme.muted : row.theme.text
                }
                Rectangle {
                    visible: row.task.list !== ""
                    Layout.preferredWidth: 8
                    Layout.preferredHeight: 8
                    radius: 4
                    color: row.task.color ? row.task.color : row.theme.accent
                }
            }
            Label {
                visible: row.details.length > 0 && !row.done
                Layout.fillWidth: true
                text: row.details.join("  ·  ")
                textFormat: Text.PlainText
                elide: Text.ElideRight
                wrapMode: row.compact ? Text.NoWrap : Text.Wrap
                maximumLineCount: row.compact ? 1 : 3
                font.pixelSize: 12
                color: row.task.due_soon ? row.theme.warm : row.theme.muted
            }
            Label {
                visible: row.done && !!row.task.done_on
                text: row.task.done_on || ""
                textFormat: Text.PlainText
                font.pixelSize: 12
                color: row.theme.muted
            }
        }
    }
}
