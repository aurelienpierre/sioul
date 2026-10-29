// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

// Every form's one time field (ClockField.qml, the unified agenda, part 5):
// "HH:MM" behind its mask; Up and Down by five minutes to the next whole
// five, around midnight, said as typed; what does not read left as it is.
// Run by tools/qml-test.sh.

import QtQuick
import QtQuick.Controls.Basic
import QtTest
import "../../qml"

Item {
    id: root

    width: 300
    height: 100

    Component {
        id: fieldComponent

        ClockField {}
    }

    // A field bound to its model, as the settings' rows are (`text: …`).
    QtObject {
        id: model

        property string value: "09:00"
    }

    Component {
        id: boundComponent

        ClockField {
            text: model.value
        }
    }

    TestCase {
        name: "ClockField"
        when: windowShown

        function test_up_and_down_by_five_minutes() {
            const field = createTemporaryObject(fieldComponent, root, { text: "09:03" })
            let edited = 0
            field.textEdited.connect(() => edited++)
            field.forceActiveFocus()
            keyClick(Qt.Key_Up)
            compare(field.text, "09:05", "to the next whole five")
            keyClick(Qt.Key_Up)
            compare(field.text, "09:10")
            keyClick(Qt.Key_Down)
            keyClick(Qt.Key_Down)
            keyClick(Qt.Key_Down)
            compare(field.text, "08:55")
            compare(edited, 5, "said as typed")
            field.text = "23:58"
            keyClick(Qt.Key_Up)
            compare(field.text, "00:00", "around midnight")
            keyClick(Qt.Key_Down)
            compare(field.text, "23:55")
            field.text = "25:00"
            keyClick(Qt.Key_Up)
            compare(field.text, "25:00", "what does not read stays as typed")
            field.text = "9:"
            keyClick(Qt.Key_Up)
            compare(field.text.replace(/ /g, ""), "9:", "half typed stays as typed")
            verify(field.inputMask.indexOf("99:99") === 0, field.inputMask)
        }
    

        // An end field stops at 23:55, never wrapping to the day's start (review of
        // 21436bf, later 10).
        function test_an_end_stops_at_the_day_s_end() {
            const field = createTemporaryObject(fieldComponent, root, { text: "23:50", end: true })
            field.forceActiveFocus()
            keyClick(Qt.Key_Up)
            keyClick(Qt.Key_Up)
            compare(field.text, "23:55")
            field.text = "00:05"
            keyClick(Qt.Key_Down)
            keyClick(Qt.Key_Down)
            compare(field.text, "00:00")
        }

        // A field without a mask is stepped whole, never added to (review of 679f96a, later 11).
        function test_a_field_without_a_mask_steps_whole() {
            const field = createTemporaryObject(fieldComponent, root, { inputMask: "", text: "09:00" })
            field.forceActiveFocus()
            keyClick(Qt.Key_Up)
            compare(field.text, "09:05")
        }

        // A step keeps the field's binding: its model changed after, it shows the model's.
        function test_a_step_keeps_the_binding() {
            const field = createTemporaryObject(boundComponent, root)
            field.forceActiveFocus()
            keyClick(Qt.Key_Up)
            compare(field.text, "09:05")
            model.value = "14:30"
            compare(field.text, "14:30", "the model's, after a step")
            model.value = "09:00"
        }
    }
}
