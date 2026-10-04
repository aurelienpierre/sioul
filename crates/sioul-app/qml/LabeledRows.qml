// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

// Values with what each is for ("work", "home", "cell"): one row each, a way
// to take one off, and one to add another.

pragma ComponentBehavior: Bound

import QtQuick
import QtQuick.Controls.Basic
import QtQuick.Layouts

ColumnLayout {
    id: rows

    required property var sioul
    required property var theme
    required property var labels
    required property ListModel model
    required property string addText
    property bool multiline: false

    Layout.fillWidth: true
    spacing: 4

    Repeater {
        model: rows.model

        delegate: RowLayout {
            id: line

            required property int index
            required property string label
            required property string value

            Layout.fillWidth: true
            spacing: 6

            ComboBox {
                Layout.preferredWidth: 130
                Layout.alignment: Qt.AlignTop
                model: rows.labels.map(l => l === "" ? "—" : rows.sioul.text("label-" + l))
                currentIndex: Math.max(0, rows.labels.indexOf(line.label))
                onActivated: index => rows.model.setProperty(line.index, "label", rows.labels[index])
            }
            TextField {
                visible: !rows.multiline
                Layout.fillWidth: true
                text: line.value
                onTextEdited: rows.model.setProperty(line.index, "value", text)
            }
            TextArea {
                id: area

                visible: rows.multiline
                Layout.fillWidth: true
                Layout.preferredHeight: 80
                text: line.value
                wrapMode: TextArea.Wrap
                onTextChanged: if (activeFocus) rows.model.setProperty(line.index, "value", text)

                // A field shows where it is: a border, darker when it has the focus.
                background: Rectangle {
                    color: rows.theme.surface
                    radius: rows.theme.radius
                    border.color: area.activeFocus ? rows.theme.focus : rows.theme.line
                }
            }
            ToolButton {
                Layout.alignment: Qt.AlignTop
                text: "×"
                Accessible.name: rows.sioul.text("ui-remove")
                ToolTip.visible: hovered
                ToolTip.text: rows.sioul.text("ui-remove")
                onClicked: rows.model.remove(line.index)
            }
        }
    }
    Button {
        flat: true
        text: "+  " + rows.addText
        onClicked: rows.model.append({ label: "", value: "" })
    }
}
