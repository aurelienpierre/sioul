// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

// A task's lines of supplies (docs/supplies.md): a supply and how many, a
// line each, and "Add a supply". In the task's form, what it plans to use;
// on a task done, and in the status line's "Change…", what it really used.
// Each change gives every line back at once (`edited`), as [{id, quantity}];
// the names are the person's own words, drawn plain. Fits a phone's width:
// the supply takes what the quantity and × leave.

pragma ComponentBehavior: Bound

import QtQuick
import QtQuick.Controls.Basic
import QtQuick.Layouts

ColumnLayout {
    id: lines

    // Sioul's backend (backend.rs): its words in your language.
    required property var sioul
    // The window's colours, sizes and fonts (Theme.qml).
    required property var theme
    // The supplies to choose from: [{id, name, unit}].
    property var choices: []
    // The lines: [{id, quantity}].
    property var value: []
    // Lines can be changed (a task you only read: shown, never changed).
    property bool editable: true

    // A line changed, added or taken off: every line as it is now.
    signal edited(var value)

    spacing: 4

    // The choices, a supply a line names that is no longer kept among them too (archived, gone).
    readonly property var offered: {
        const out = lines.choices.slice()
        for (const line of lines.value)
            if (!out.some(c => c.id === line.id))
                out.push({ id: line.id, name: line.name || line.id, unit: line.unit || "" })
        return out
    }

    // A quantity as typed, "1,5" or "2": its number, or NaN.
    function number(text) {
        const typed = String(text).trim().replace(/\s/g, "").replace(",", ".")
        return typed === "" || !/^[0-9]*\.?[0-9]*$/.test(typed) ? NaN : Number(typed)
    }

    function shown(quantity) {
        const point = Qt.locale(lines.sioul.text("qt-locale")).decimalPoint
        return String(quantity).replace(".", point)
    }

    function changed(index, line) {
        const value = lines.value.map(l => ({ id: l.id, quantity: l.quantity }))
        if (line === null)
            value.splice(index, 1)
        else
            value[index] = line
        lines.edited(value)
    }

    Repeater {
        model: lines.value

        delegate: RowLayout {
            id: line

            required property var modelData
            required property int index
            readonly property int at: lines.offered.findIndex(c => c.id === line.modelData.id)

            Layout.fillWidth: true
            spacing: 6

            PlainComboBox {
                objectName: "supplyPick:" + line.index
                Layout.fillWidth: true
                Layout.minimumWidth: 90
                enabled: lines.editable
                model: lines.offered.map(c => lines.theme.plain(c.name))
                currentIndex: line.at
                Accessible.name: lines.sioul.text("supplies-field-name")
                onActivated: index => lines.changed(line.index, { id: lines.offered[index].id, quantity: line.modelData.quantity })
            }
            TextField {
                id: quantity

                objectName: "supplyQuantity:" + line.index
                Layout.preferredWidth: 72
                enabled: lines.editable
                horizontalAlignment: Text.AlignRight
                inputMethodHints: Qt.ImhFormattedNumbersOnly
                text: lines.shown(line.modelData.quantity)
                Accessible.name: lines.sioul.text("supplies-quantity")
                onEditingFinished: {
                    const n = lines.number(quantity.text)
                    if (Number.isNaN(n) || n < 0) {
                        quantity.text = lines.shown(line.modelData.quantity)
                        return
                    }
                    if (n !== line.modelData.quantity)
                        lines.changed(line.index, { id: line.modelData.id, quantity: n })
                }
            }
            Label {
                visible: line.at >= 0 && (lines.offered[line.at].unit || "") !== ""
                text: line.at >= 0 ? lines.offered[line.at].unit || "" : ""
                textFormat: Text.PlainText
                color: lines.theme.muted
            }
            Button {
                objectName: "supplyRemove:" + line.index
                visible: lines.editable
                flat: true
                icon.name: "window-close"
                icon.color: lines.theme.text
                implicitWidth: implicitHeight
                Accessible.name: lines.sioul.text("supplies-line-remove")
                ToolTip.visible: hovered
                ToolTip.text: lines.sioul.text("supplies-line-remove")
                onClicked: lines.changed(line.index, null)
            }
        }
    }
    Button {
        objectName: "supplyAdd"
        visible: lines.editable && lines.choices.length > 0
        flat: true
        text: lines.sioul.text("supplies-add")
        icon.name: "list-add"
        icon.color: lines.theme.text
        onClicked: {
            // The first supply not on a line yet, one of it.
            const free = lines.choices.find(c => !lines.value.some(l => l.id === c.id)) || lines.choices[0]
            lines.edited(lines.value.map(l => ({ id: l.id, quantity: l.quantity })).concat([{ id: free.id, quantity: 1 }]))
        }
    }
}
