// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

// A medicine's takes at set times each day, as rows (MedicineDialog.qml,
// PrescriptionDialog.qml): each its time, picked (TimeField.qml), and its own
// amount, only when it differs from the medicine's dose ("2 tablets" at
// 20:00, the dose "1 tablet" at the others); Add a take adds a row, each row
// can be taken out. The amounts and the times are read when the form saves
// (`takes()`): no take, two at one time, are said then (`health::apply_medicine`).

pragma ComponentBehavior: Bound

import QtQuick
import QtQuick.Controls.Basic
import QtQuick.Layouts

ColumnLayout {
    id: editor

    required property var sioul
    required property var theme
    // The medicine's dose, what a take with no amount of its own is given: its field's hint.
    property string usual: ""

    // A take changed, added or taken out by hand (not when `load` fills the rows).
    signal edited

    // The takes now, [{time, amount}], in the rows' order.
    function takes() {
        const out = []
        for (let i = 0; i < rows.count; i++)
            out.push({ time: rows.get(i).time, amount: rows.get(i).amount })
        return out
    }

    // Rows for `takes` ([{time, amount}]); one at 08:00 when there are none.
    function load(takes) {
        rows.clear()
        for (const take of (takes && takes.length > 0 ? takes : [{ time: "08:00", amount: "" }]))
            rows.append({ time: take.time, amount: take.amount || "" })
    }

    // A new take at a usual time not taken yet: morning, evening, noon, then the hours between.
    function add() {
        const taken = editor.takes().map(t => t.time)
        const usual = ["08:00", "20:00", "12:00", "18:00", "22:00", "06:00", "10:00", "14:00", "16:00"]
        rows.append({ time: usual.find(t => taken.indexOf(t) < 0) || "12:00", amount: "" })
        editor.edited()
    }

    spacing: 4

    ListModel {
        id: rows
    }

    Repeater {
        model: rows

        delegate: RowLayout {
            id: row

            required property int index
            required property string time
            required property string amount

            Layout.fillWidth: true
            spacing: 6

            TimeField {
                id: picked

                sioul: editor.sioul
                theme: editor.theme
                time: row.time
                onEdited: {
                    rows.setProperty(row.index, "time", picked.time)
                    editor.edited()
                }
            }
            TextField {
                id: own

                // What is left of the row: never widening the form on a phone.
                Layout.fillWidth: true
                Layout.preferredWidth: 1
                Layout.minimumWidth: 60
                text: row.amount
                placeholderText: editor.theme.plain(editor.usual !== "" ? editor.usual : editor.sioul.text("health-take-amount-hint"))
                Accessible.name: editor.sioul.text("health-take-amount-hint")
                onTextEdited: {
                    rows.setProperty(row.index, "amount", own.text)
                    editor.edited()
                }
            }
            ToolButton {
                // An ×: the medicine's own row has the bin.
                icon.name: "window-close"
                icon.color: editor.theme.text
                Accessible.name: editor.sioul.text("health-take-remove")
                ToolTip.delay: 400
                ToolTip.visible: hovered
                ToolTip.text: editor.sioul.text("health-take-remove")
                onClicked: {
                    rows.remove(row.index)
                    editor.edited()
                }
            }
        }
    }
    Button {
        flat: true
        implicitWidth: implicitContentWidth + leftPadding + rightPadding
        icon.name: "list-add"
        icon.color: editor.theme.text
        text: editor.sioul.text("health-add-take")
        onClicked: editor.add()
    }
}
