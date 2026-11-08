// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

// Batch edits in the full lists (8 November 2026): above the task list and
// the Search tab while several tasks or events are selected (Ctrl or Shift
// with a click, a long press on a touch screen). How many, then what changes
// for all of them in one act, with Undo on the status line: project, type,
// category, day, status, list, Done, Delete. What a batch leaves out, a
// meeting with others above all, is said under it, each with why (`batch.rs`).

pragma ComponentBehavior: Bound

import QtQuick
import QtQuick.Controls.Basic
import QtQuick.Layouts

ColumnLayout {
    id: bar

    required property var sioul
    required property var theme
    // What is selected: [{thing, key, uid, title}].
    property var things: []
    // The choices: [{id, label}] for projects and kinds, names for categories,
    // [{id, label, tasks, events}] for lists and calendars.
    property var projects: []
    property var kinds: []
    property var categories: []
    property var lists: []
    // What the last act left out: [{title, why}].
    property var leftOut: []

    readonly property bool hasTasks: bar.things.some(t => t.thing === "task")
    readonly property bool hasEvents: bar.things.some(t => t.thing === "event")

    // The selection is to be emptied: after an act, or asked.
    signal cleared

    spacing: 4
    visible: bar.things.length > 0 || bar.leftOut.length > 0

    // One act on everything selected; what it left out kept to be said.
    function change(asked) {
        const text = bar.sioul.changeThings(JSON.stringify(bar.things.map(t => ({ thing: t.thing, key: t.key, uid: t.uid }))), JSON.stringify(asked))
        const answer = text ? JSON.parse(text) : {}
        if (answer.error)
            bar.sioul.status = answer.error
        else if (answer.said)
            bar.sioul.status = answer.said
        bar.leftOut = answer.left || []
        if (!answer.error)
            bar.cleared()
    }

    function counted(id, n) {
        return typeof bar.sioul.textCounted === "function" ? bar.sioul.textCounted(id, n) : id + ":" + n
    }

    Flow {
        visible: bar.things.length > 0
        Layout.fillWidth: true
        spacing: 6

        Label {
            objectName: "batchCount"
            height: clearButton.height
            verticalAlignment: Text.AlignVCenter
            text: bar.counted("batch-selected", bar.things.length)
            color: bar.theme.text
            font.weight: Font.DemiBold
        }
        Button {
            objectName: "batchProject"
            text: bar.sioul.text("batch-project") + " ▾"
            onClicked: projectMenu.popup()

            SioulMenu {
                id: projectMenu

                MenuItem {
                    text: bar.sioul.text("things-no-project")
                    onTriggered: bar.change({ project: ":none" })
                }
                Repeater {
                    model: bar.projects

                    delegate: MenuItem {
                        required property var modelData

                        text: bar.theme.plain(modelData.label)
                        onTriggered: bar.change({ project: modelData.id })
                    }
                }
            }
        }
        Button {
            objectName: "batchKind"
            visible: bar.hasTasks
            text: bar.sioul.text("batch-kind") + " ▾"
            onClicked: kindMenu.popup()

            SioulMenu {
                id: kindMenu

                MenuItem {
                    text: bar.sioul.text("things-no-kind")
                    onTriggered: bar.change({ kind: ":none" })
                }
                Repeater {
                    model: bar.kinds

                    delegate: MenuItem {
                        required property var modelData

                        text: bar.theme.plain(modelData.label)
                        onTriggered: bar.change({ kind: modelData.id })
                    }
                }
            }
        }
        Button {
            objectName: "batchCategory"
            text: bar.sioul.text("batch-category") + " ▾"
            onClicked: categoryMenu.popup()

            SioulMenu {
                id: categoryMenu

                MenuItem {
                    text: bar.sioul.text("things-no-category")
                    onTriggered: bar.change({ category: ":none" })
                }
                Repeater {
                    model: bar.categories

                    delegate: MenuItem {
                        required property string modelData

                        text: bar.theme.plain(modelData)
                        onTriggered: bar.change({ category: modelData })
                    }
                }
            }
        }
        Button {
            objectName: "batchDate"
            text: bar.sioul.text("batch-date")
            onClicked: datePopup.open()

            Popup {
                id: datePopup

                y: parent.height
                padding: 10

                ColumnLayout {
                    spacing: 6

                    DateField {
                        id: dayField

                        objectName: "batchDay"
                        Layout.preferredWidth: 180
                        theme: bar.theme
                        sioul: bar.sioul
                    }
                    RowLayout {
                        spacing: 6

                        Button {
                            objectName: "batchDateSet"
                            enabled: /^\d{4}-\d{2}-\d{2}$/.test(dayField.date)
                            highlighted: true
                            text: bar.sioul.text("ui-ok")
                            onClicked: {
                                datePopup.close()
                                bar.change({ date: dayField.date })
                            }
                        }
                        Button {
                            objectName: "batchDateNone"
                            visible: bar.hasTasks
                            flat: true
                            text: bar.sioul.text("batch-date-none")
                            onClicked: {
                                datePopup.close()
                                bar.change({ date: "" })
                            }
                        }
                    }
                }
            }
        }
        Button {
            objectName: "batchStatus"
            text: bar.sioul.text("batch-status") + " ▾"
            onClicked: statusMenu.popup()

            SioulMenu {
                id: statusMenu

                Repeater {
                    model: bar.hasTasks ? ["needs-action", "in-process", "completed", "cancelled"] : []

                    delegate: MenuItem {
                        required property string modelData

                        text: bar.sioul.text("batch-task-status-" + modelData)
                        onTriggered: bar.change({ status: modelData })
                    }
                }
                MenuSeparator {
                    visible: bar.hasTasks && bar.hasEvents
                    height: visible ? implicitHeight : 0
                }
                Repeater {
                    model: bar.hasEvents ? ["confirmed", "tentative", "cancelled"] : []

                    delegate: MenuItem {
                        required property string modelData

                        text: bar.sioul.text("things-status-" + modelData)
                        onTriggered: bar.change({ status: modelData })
                    }
                }
            }
        }
        Button {
            objectName: "batchList"
            visible: bar.lists.length > 0
            text: bar.sioul.text("batch-list") + " ▾"
            onClicked: listMenu.popup()

            SioulMenu {
                id: listMenu

                Repeater {
                    // Only where every thing selected can go.
                    model: bar.lists.filter(l => (!bar.hasTasks || l.tasks) && (!bar.hasEvents || l.events))

                    delegate: MenuItem {
                        required property var modelData

                        text: bar.theme.plain(modelData.label)
                        onTriggered: bar.change({ list: modelData.id })
                    }
                }
            }
        }
        Button {
            objectName: "batchDone"
            visible: bar.hasTasks
            text: bar.sioul.text("batch-done")
            icon.name: "checkbox"
            onClicked: bar.change({ status: "completed" })
        }
        Button {
            objectName: "batchDelete"
            text: bar.sioul.text("batch-delete")
            icon.name: "edit-delete"
            onClicked: bar.change({ delete: true })
        }
        ToolButton {
            id: clearButton

            objectName: "batchClear"
            text: "×"
            Accessible.name: bar.sioul.text("batch-clear")
            ToolTip.visible: hovered
            ToolTip.text: bar.sioul.text("batch-clear")
            onClicked: {
                bar.leftOut = []
                bar.cleared()
            }
        }
    }

    // What the last act left as it was, each with why.
    Label {
        visible: bar.leftOut.length > 0
        Layout.fillWidth: true
        text: bar.sioul.text("batch-left-title")
        color: bar.theme.muted
    }
    Repeater {
        model: bar.leftOut

        delegate: Label {
            required property var modelData

            objectName: "batchLeft"
            Layout.fillWidth: true
            text: modelData.line
            textFormat: Text.PlainText
            wrapMode: Text.Wrap
            color: bar.theme.muted
        }
    }
}
