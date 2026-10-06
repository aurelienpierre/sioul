// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

// The routines: the admin window's, made from what is there now, then yours;
// each played, changed or taken out; a new one written one step a line.

pragma ComponentBehavior: Bound

import QtQuick
import QtQuick.Controls.Basic
import QtQuick.Layouts

Dialog {
    id: routines

    required property var sioul
    required property var theme
    property var list: []
    // The routine being written: its id ("" for a new one); null while listing.
    property var editing: null
    property string problem: ""

    signal play(var routine)

    function show() {
        routines.list = JSON.parse(routines.sioul.routines())
        routines.editing = null
        routines.problem = ""
        routines.open()
    }

    function edit(routine) {
        routines.editing = routine ? routine.id : ""
        titleField.text = routine ? routine.title : ""
        stepsArea.text = routine ? routine.text : ""
        autoBox.checked = routine ? routine.auto : false
        routines.problem = ""
    }

    parent: Overlay.overlay
    anchors.centerIn: parent
    modal: true
    width: Math.min(560, (parent ? parent.width : 560) - 2 * routines.theme.gap)
    // A routine being changed keeps the list's title: "A new routine" is for a new one only.
    title: routines.sioul.text(routines.editing === "" ? "routines-new" : "routines")

    contentItem: ColumnLayout {
        spacing: 8

        // Listed: what a routine is, and what it is for; written: how its steps are written.
        Label {
            Layout.fillWidth: true
            text: routines.sioul.text(routines.editing === null ? "routines-what" : "routines-help")
            wrapMode: Text.Wrap
            font.pixelSize: 13
            color: routines.theme.muted
        }

        // The list.
        Repeater {
            model: routines.editing === null ? routines.list : []

            delegate: Rectangle {
                id: row

                required property var modelData

                Layout.fillWidth: true
                implicitHeight: rowLayout.implicitHeight + 14
                color: routines.theme.surface
                border.color: routines.theme.line
                radius: 4

                RowLayout {
                    id: rowLayout

                    anchors.fill: parent
                    anchors.margins: 7
                    spacing: 8

                    ColumnLayout {
                        Layout.fillWidth: true
                        spacing: 2

                        Label {
                            Layout.fillWidth: true
                            text: row.modelData.title
                            textFormat: Text.PlainText
                            elide: Text.ElideRight
                            color: routines.theme.text
                        }
                        Label {
                            Layout.fillWidth: true
                            text: row.modelData.builtin ? routines.sioul.text("routines-builtin") : row.modelData.step.map(s => s.title).join(" → ")
                            textFormat: Text.PlainText
                            wrapMode: Text.Wrap
                            maximumLineCount: 2
                            elide: Text.ElideRight
                            font.pixelSize: 12
                            color: routines.theme.muted
                        }
                    }
                    Button {
                        highlighted: true
                        text: routines.sioul.text("routines-play")
                        onClicked: {
                            routines.close()
                            routines.play(row.modelData)
                        }
                    }
                    Button {
                        visible: !row.modelData.builtin
                        flat: true
                        text: routines.sioul.text("routines-change")
                        onClicked: routines.edit(row.modelData)
                    }
                    Button {
                        visible: !row.modelData.builtin
                        flat: true
                        text: routines.sioul.text("routines-remove")
                        onClicked: {
                            routines.problem = routines.sioul.removeRoutine(row.modelData.id)
                            routines.list = JSON.parse(routines.sioul.routines())
                        }
                    }
                }
            }
        }
        Button {
            visible: routines.editing === null
            text: routines.sioul.text("routines-new")
            icon.name: "list-add"
            onClicked: routines.edit(null)
        }

        // Writing one.
        GridLayout {
            visible: routines.editing !== null
            Layout.fillWidth: true
            columns: 2
            columnSpacing: 10
            rowSpacing: 8

            Label {
                text: routines.sioul.text("routines-title")
                color: routines.theme.muted
            }
            TextField {
                id: titleField

                Layout.fillWidth: true
            }
            Label {
                Layout.alignment: Qt.AlignTop
                text: routines.sioul.text("routines-steps")
                color: routines.theme.muted
            }
            TextArea {
                id: stepsArea

                Layout.fillWidth: true
                Layout.preferredHeight: 140
                wrapMode: TextEdit.Wrap
                placeholderText: "10 min …\n5 min …"
                background: Rectangle {
                    color: routines.theme.surface
                    border.color: stepsArea.activeFocus ? routines.theme.focus : routines.theme.line
                    radius: 3
                }
            }
            CheckBox {
                id: autoBox

                Layout.columnSpan: 2
                text: routines.sioul.text("routines-auto")
            }
        }
        Label {
            visible: routines.problem !== ""
            Layout.fillWidth: true
            text: routines.problem
            textFormat: Text.PlainText
            wrapMode: Text.Wrap
            color: routines.theme.warm
        }
        RowLayout {
            Layout.fillWidth: true
            spacing: 8

            Item {
                Layout.fillWidth: true
            }
            Button {
                text: routines.sioul.text(routines.editing === null ? "ui-close" : "ui-cancel")
                onClicked: {
                    if (routines.editing === null)
                        routines.close()
                    else
                        routines.editing = null
                }
            }
            Button {
                visible: routines.editing !== null
                highlighted: true
                text: routines.sioul.text("ui-save")
                onClicked: {
                    routines.problem = routines.sioul.saveRoutine(routines.editing, titleField.text, stepsArea.text, autoBox.checked)
                    if (routines.problem === "") {
                        routines.list = JSON.parse(routines.sioul.routines())
                        routines.editing = null
                    }
                }
            }
        }
    }
}
