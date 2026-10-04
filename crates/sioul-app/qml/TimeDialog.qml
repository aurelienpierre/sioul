// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

// Time noted by hand: a meeting, a call, work done away from the timer. For
// a project (or a task), on a day, how long, a word on what it was.

pragma ComponentBehavior: Bound

import QtQuick
import QtQuick.Controls.Basic
import QtQuick.Layouts

Dialog {
    id: dialog

    required property var sioul
    required property var theme
    property var projects: []
    property string problem: ""
    // The stretch being changed, by its key; "" for a new one.
    property string replacing: ""

    signal saved

    // How long, as typed: "1h30", "1 h 30", "90", "45m", "45 min".
    function minutesOf(text) {
        const t = text.trim().toLowerCase().replace(/\s+/g, "")
        const hours = t.match(/^(\d+)h(\d+)?(min|m)?$/)
        if (hours)
            return Number(hours[1]) * 60 + Number(hours[2] || 0)
        const minutes = t.match(/^(\d+)(min|m)?$/)
        return minutes ? Number(minutes[1]) : 0
    }

    function today() {
        const d = new Date()
        const pad = n => n < 10 ? "0" + n : String(n)
        return d.getFullYear() + "-" + pad(d.getMonth() + 1) + "-" + pad(d.getDate())
    }

    // A stretch noted before, to change: its day, time, length, project, word.
    function change(entry) {
        dialog.begin(entry.project)
        dialog.replacing = entry.key
        day.date = entry.day
        at.text = entry.at
        length.text = String(entry.minutes)
        note.text = entry.note !== "" ? entry.note : entry.title
        unbilled.checked = !entry.billable
    }

    function begin(project) {
        // Open projects, and the one asked for even when closed: a stretch changed
        // there must not move to another project unseen.
        dialog.projects = JSON.parse(dialog.sioul.projectRows() || "[]").filter(p => p.status !== "closed" || p.id === project)
        dialog.replacing = ""
        dialog.problem = ""
        day.date = dialog.today()
        at.text = ""
        length.text = ""
        note.text = ""
        unbilled.checked = false
        which.currentIndex = Math.max(0, dialog.projects.findIndex(p => p.id === project))
        dialog.open()
        length.forceActiveFocus()
    }

    function save() {
        const chosen = dialog.projects[which.currentIndex]
        const edit = {
            day: day.date,
            at: at.text,
            minutes: dialog.minutesOf(length.text),
            project: chosen ? chosen.id : "",
            note: note.text,
            unbilled: unbilled.checked
        }
        const problem = dialog.sioul.noteTime(JSON.stringify(edit))
        if (problem !== "") {
            dialog.problem = problem
            return
        }
        // Changed: the new stretch is noted first, then the old one taken out.
        if (dialog.replacing !== "") {
            const left = dialog.sioul.removeTime(dialog.replacing)
            if (left !== "")
                dialog.sioul.status = left
        }
        dialog.close()
        dialog.saved()
    }

    parent: Overlay.overlay
    anchors.centerIn: parent
    modal: true
    width: Math.min(460, (parent ? parent.width : 460) - 2 * dialog.theme.gap)
    title: dialog.replacing === "" ? dialog.sioul.text("time-note") : dialog.sioul.text("time-change")

    contentItem: GridLayout {
        columns: 2
        columnSpacing: 10
        rowSpacing: 8

        Label {
            text: dialog.sioul.text("time-field-project")
            color: dialog.theme.muted
        }
        ComboBox {
            id: which

            Layout.fillWidth: true
            model: dialog.projects.map(p => dialog.theme.plain(p.title))
        }
        Label {
            text: dialog.sioul.text("time-field-length")
            color: dialog.theme.muted
        }
        TextField {
            id: length

            Layout.fillWidth: true
            placeholderText: dialog.sioul.text("time-field-length-hint")
            onAccepted: dialog.save()
        }
        Label {
            text: dialog.sioul.text("time-field-day")
            color: dialog.theme.muted
        }
        RowLayout {
            spacing: 8

            DateField {
                id: day

                theme: dialog.theme
                locale: Qt.locale(dialog.sioul.text("qt-locale"))
                pickLabel: dialog.sioul.text("event-pick-day")
            }
            TextField {
                id: at

                Layout.preferredWidth: 70
                placeholderText: "14:00"
                inputMask: "99:99;_"
            }
        }
        Label {
            text: dialog.sioul.text("time-field-note")
            color: dialog.theme.muted
        }
        TextField {
            id: note

            Layout.fillWidth: true
            placeholderText: dialog.sioul.text("time-field-note-hint")
            onAccepted: dialog.save()
        }
        Item {
            Layout.preferredHeight: 1
        }
        CheckBox {
            id: unbilled

            visible: dialog.projects.length > 0 && dialog.projects[which.currentIndex] && dialog.projects[which.currentIndex].is_project
            text: dialog.sioul.text("time-field-unbilled")
        }
        Label {
            visible: dialog.problem !== ""
            Layout.columnSpan: 2
            Layout.fillWidth: true
            text: dialog.problem
            textFormat: Text.PlainText
            wrapMode: Text.Wrap
            color: dialog.theme.warm
        }
    }

    // The buttons inside an Item: a DialogButtonBox as the footer itself closes
    // the dialog on "Save" even when saving fails, and what went wrong is never read.
    footer: Item {
        implicitWidth: buttons.implicitWidth
        implicitHeight: buttons.implicitHeight

        DialogButtonBox {
            id: buttons

            anchors.fill: parent

            Button {
                text: dialog.sioul.text("ui-save")
                highlighted: true
                DialogButtonBox.buttonRole: DialogButtonBox.AcceptRole
            }
            Button {
                text: dialog.sioul.text("ui-cancel")
                DialogButtonBox.buttonRole: DialogButtonBox.RejectRole
            }
            onAccepted: dialog.save()
            onRejected: dialog.close()
        }
    }
}
