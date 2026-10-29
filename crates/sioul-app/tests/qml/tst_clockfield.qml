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
            verify(field.inputMask.indexOf("99:99") === 0, field.inputMask)
        }
    }
}
