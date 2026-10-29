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

    // The task as the core's views give it (`taskview`): its uid, title, status and
    // the words of its details.
    required property var task
    // The window's colours, sizes and fonts (Theme.qml).
    required property var theme
    // For the circle's name, said by screen readers in words.
    property var sioul: null
    // Its details are open in the panel: the row is highlighted.
    property bool selected: false
    // Tighter, its words on one line: on the board, on a phone.
    property bool compact: false
    // Indent for steps, in levels.
    property int depth: 0
    // The view folds a task with steps into one row (List, Board: composed tasks §7.2):
    // its arrow opens its steps under it, or folds them again.
    property bool folds: false
    // A step in its task's form (Edit): a handle drawn, by which it is dragged into its place.
    property bool handle: false

    // A click: its details asked, by its UID.
    signal open(string uid)
    // Its circle ticked: done, or open again.
    signal tick(string uid)
    // A right click or a long press: its menu asked.
    signal menu(var task)
    // Its arrow pressed: its steps shown (`open`), or folded again.
    signal fold(string uid, bool open)

    // Done or dropped: its circle filled, its title quieter.
    readonly property bool done: row.task.status === "completed" || row.task.status === "cancelled"
    // The words under its title that say something: the date asked, the length, the
    // steps, what it waits for, where you stopped.
    // Folded, its one line: its appointment's time and its next step, never a count.
    readonly property var details: row.task.folded ? [row.task.fold_line || ""].filter(t => t !== "") : [row.task.due, row.task.estimate, row.task.steps, row.task.waits, row.task.stopped].filter(t => t !== "")

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

    // On a touch screen, the menu comes at a long press; letting go then opens nothing.
    onPressAndHold: {
        row.Window.window.menuAt = row.mapToItem(null, row.pressX, row.pressY)
        row.menu(row.task)
    }
    TapHandler {
        acceptedButtons: Qt.RightButton
        // A touch has no buttons: on a touch screen, the row's long press.
        acceptedDevices: PointerDevice.Mouse | PointerDevice.TouchPad
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

                // One place before the title, made only where it is needed: in the form, a
                // step's handle (it moves by a drag on its row); where the view folds, a task
                // with steps' arrow (composed tasks §7.2).
                Loader {
                    active: row.handle || (row.folds && !!row.task.has_steps)
                    visible: active
                    sourceComponent: ToolButton {
                        objectName: row.handle ? "taskRowHandle" : "taskRowFold"
                        implicitWidth: 24
                        implicitHeight: 24
                        padding: 0
                        // A handle is drawn only: the drag is its row's.
                        enabled: !row.handle
                        text: row.handle ? "⠿" : row.task.folded ? "▸" : "▾"
                        Accessible.name: row.handle || !row.sioul ? "" : row.sioul.text(row.task.folded ? "task-fold-open" : "task-fold-close")
                        onClicked: row.fold(row.task.uid, !!row.task.folded)
                    }
                }
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
                // Pinned to a time (its block in your calendar, docs/tasks.md): a small pin and when, quietly.
                Icon {
                    visible: !!row.task.pinned_time && !row.done
                    iconName: "pin"
                    size: 12
                    color: row.theme.accent
                    tip: row.task.pinned || ""
                }
                Label {
                    visible: !!row.task.pinned_time && !row.done
                    text: row.task.pinned_time || ""
                    textFormat: Text.PlainText
                    font.pixelSize: 12
                    font.features: { "tnum": 1 }
                    color: row.theme.accent
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
            // Set aside by an appointment over (the done tasks list it, however old):
            // said so, with Bring back, which needs no card (review of 4d7bab4, later 8).
            // Made only then: every other row stays as light as before.
            Loader {
                active: !!row.task.set_aside
                visible: active
                Layout.fillWidth: true
                sourceComponent: RowLayout {
                    spacing: 8
                    Label {
                        Layout.fillWidth: true
                        text: row.task.set_aside || ""
                        textFormat: Text.PlainText
                        wrapMode: Text.Wrap
                        font.pixelSize: 12
                        color: row.theme.muted
                    }
                    Button {
                        objectName: "taskRowBringBack"
                        visible: !row.task.read_only && row.sioul !== null
                        flat: true
                        text: row.sioul ? row.sioul.text("appointment-bring-back") : ""
                        onClicked: {
                            const answer = JSON.parse(row.sioul.bringBack(row.task.uid) || "{}")
                            if (answer.error)
                                row.sioul.status = answer.error
                        }
                    }
                }
            }
        }
    }
}
