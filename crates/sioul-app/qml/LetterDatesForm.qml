// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

// A letter's dates, put right (the unified agenda, part 4; decision 19):
// its date asked and its appointment's day and time, read from its scan,
// while it has neither its task nor its event. Saved through
// `letterDates(id, edit)`: the letters' file under its lock, its other
// fields as they are then. A date read stays as read until you change it here.

import QtQuick
import QtQuick.Controls.Basic
import QtQuick.Layouts

Dialog {
    id: form

    required property var sioul
    required property var theme
    property string letterId: ""
    property string problem: ""

    signal saved

    // `item`: {deadline, appointment, time}, each "" for none.
    function edit(id, item) {
        form.letterId = id
        form.problem = ""
        deadlineField.text = item.deadline || ""
        appointmentField.text = item.appointment || ""
        timeField.text = item.time || ""
        form.open()
        deadlineField.forceActiveFocus()
    }

    function save() {
        const problem = form.sioul.letterDates(form.letterId, JSON.stringify({ deadline: deadlineField.text, appointment: appointmentField.text, time: timeField.text }))
        if (problem !== "") {
            form.problem = problem
            return
        }
        form.close()
        form.saved()
    }

    parent: Overlay.overlay
    anchors.centerIn: parent
    modal: true
    width: Math.min(440, (parent ? parent.width : 440) - 2 * form.theme.gap)
    title: form.sioul.text("dated-letter-dates")

    contentItem: GridLayout {
        columns: 2
        columnSpacing: 12
        rowSpacing: 8

        Label {
            text: form.sioul.text("dated-letter-deadline")
            textFormat: Text.PlainText
            color: form.theme.text
        }
        TextField {
            id: deadlineField

            objectName: "letterDeadline"
            Layout.fillWidth: true
            placeholderText: "2026-11-30"
            inputMethodHints: Qt.ImhDate
        }
        Label {
            text: form.sioul.text("dated-letter-appointment")
            textFormat: Text.PlainText
            color: form.theme.text
        }
        TextField {
            id: appointmentField

            objectName: "letterAppointment"
            Layout.fillWidth: true
            placeholderText: "2026-11-05"
            inputMethodHints: Qt.ImhDate
        }
        Label {
            text: form.sioul.text("dated-letter-time")
            textFormat: Text.PlainText
            color: form.theme.text
        }
        TextField {
            id: timeField

            objectName: "letterTime"
            Layout.fillWidth: true
            placeholderText: "14:30"
            inputMethodHints: Qt.ImhTime
        }
        Label {
            visible: form.problem !== ""
            Layout.columnSpan: 2
            Layout.fillWidth: true
            text: form.problem
            textFormat: Text.PlainText
            wrapMode: Text.Wrap
            color: form.theme.text
        }
    }
    // Saved by its own button: a date that does not read keeps the form open, said.
    footer: RowLayout {
        spacing: 8

        Item {
            Layout.fillWidth: true
        }
        Button {
            text: form.sioul.text("ui-cancel")
            flat: true
            onClicked: form.close()
        }
        Button {
            objectName: "letterDatesSave"
            text: form.sioul.text("ui-save")
            highlighted: true
            onClicked: form.save()
        }
    }
}
