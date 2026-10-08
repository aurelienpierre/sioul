// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

// A medicine's takes as rows (TakesEditor.qml), each a time picked, never
// typed (TimeField.qml), and its own amount; a row added at a usual time not
// taken yet, a row taken out. And a line holding commands (CommandText.qml):
// its words, then each command in a field that selects, never a command made
// of words from elsewhere. Run by tools/qml-test.sh.

import QtQuick
import QtQuick.Controls.Basic
import QtTest
import "../../qml" as App

Item {
    id: root
    width: 412
    height: 600

    QtObject {
        id: words
        function text(key) { return key }
        function textWith(key, name, value) { return key + ":" + value }
    }
    QtObject {
        id: theme
        property color line: "#e2dccf"
        property color muted: "#6b655c"
        property color text: "#2d2a26"
        property color warm: "#8f6330"
        property string mono: "monospace"
    }

    Column {
        width: parent.width
        spacing: 10

        App.TimeField {
            id: picker
            sioul: words
            theme: theme
            onEdited: spy.picked.push(picker.time)
        }
        App.TakesEditor {
            id: takes
            width: parent.width
            sioul: words
            theme: theme
            usual: "1 tablet"
            onEdited: spy.edits += 1
        }
        App.CommandText {
            id: said
            width: parent.width
            sioul: words
            theme: theme
        }
    }
    QtObject { id: spy; property var picked: []; property int edits: 0 }

    TestCase {
        name: "Takes"
        when: windowShown

        // Every item under `item` that `match` accepts, depth first.
        function found(item, match) {
            let out = []
            for (let i = 0; i < item.children.length; ++i) {
                const child = item.children[i]
                if (match(child))
                    out.push(child)
                out = out.concat(found(child, match))
            }
            return out
        }

        function init() {
            spy.picked = []
            spy.edits = 0
            picker.time = "08:00"
            takes.load([])
            said.commands = true
            said.text = ""
            wait(20)
        }

        function test_a_time_is_picked_by_the_keyboard() {
            const boxes = found(picker, c => c instanceof ComboBox)
            compare(boxes.length, 2, "an hour and a minute")
            compare(boxes[0].currentText, "08")
            boxes[0].forceActiveFocus()
            keyClick(Qt.Key_Down)
            compare(picker.time, "09:00")
            compare(spy.picked, ["09:00"])
            boxes[1].forceActiveFocus()
            keyClick(Qt.Key_Down)
            compare(picker.time, "09:05", "every five minutes")
        }

        function test_a_minute_between_the_steps_is_kept() {
            picker.time = "07:32"
            const minutes = found(picker, c => c instanceof ComboBox)[1]
            compare(minutes.currentText, "32")
            verify(picker.minutes.indexOf("30") >= 0 && picker.minutes.indexOf("35") >= 0)
            compare(spy.picked, [], "set from outside: nothing edited")
        }

        function test_rows_added_at_usual_times_and_taken_out() {
            compare(takes.takes(), [{ time: "08:00", amount: "" }], "one take at 08:00 to start")
            takes.add()
            takes.add()
            compare(takes.takes().map(t => t.time), ["08:00", "20:00", "12:00"], "morning, evening, noon")
            compare(spy.edits, 2)
            wait(20)
            const out = found(takes, c => c instanceof ToolButton)
            compare(out.length, 3, "each row can be taken out")
            mouseClick(out[1])
            compare(takes.takes().map(t => t.time), ["08:00", "12:00"])
            compare(spy.edits, 3)
        }

        function test_a_takes_own_amount_and_the_usual_as_its_hint() {
            takes.load([{ time: "08:00", amount: "" }, { time: "20:00", amount: "2 tablets" }])
            wait(20)
            const fields = found(takes, c => c instanceof TextField)
            compare(fields.length, 2)
            compare(fields[0].placeholderText, "1 tablet", "the dose, as a hint")
            compare(fields[1].text, "2 tablets")
            fields[0].forceActiveFocus()
            keyClick(Qt.Key_3)
            compare(takes.takes()[0], { time: "08:00", amount: "3" })
            compare(spy.edits, 1)
        }

        function test_commands_stand_apart_to_copy() {
            said.text = "On Arch Linux, these install it, then start it: `sudo pacman -S pcsclite ccid` `sudo systemctl enable --now pcscd.socket`"
            compare(said.parts.length, 3)
            compare(said.parts[0], { command: false, text: "On Arch Linux, these install it, then start it:" })
            compare(said.parts[1], { command: true, text: "sudo pacman -S pcsclite ccid" })
            compare(said.parts[2].text, "sudo systemctl enable --now pcscd.socket")
            wait(20)
            const fields = found(said, c => c instanceof TextField && c.visible)
            compare(fields.length, 2)
            verify(fields[0].readOnly && fields[0].selectByMouse, "selectable, never edited")
            // Words after a command lose what joined them to it.
            said.text = "in GnuPG, `gpg --card-edit`, then admin, then passwd."
            compare(said.parts.map(p => p.text), ["in GnuPG,", "gpg --card-edit", "then admin, then passwd."])
            // A lone backtick is words; off, backticks are words too.
            said.text = "a name with ` in it"
            compare(said.parts, [{ command: false, text: "a name with ` in it" }])
            said.commands = false
            said.text = "To have files checked: `rm -rf ~`"
            compare(said.parts, [{ command: false, text: "To have files checked: `rm -rf ~`" }])
        }
    }
}
