// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

// Time noted by hand (a meeting, a call, work done away from the timer), or
// any stretch changed once noted, the timer's too: its task, its project,
// its day, from when to when, a word on what it was.

pragma ComponentBehavior: Bound

import QtQuick
import QtQuick.Controls.Basic
import QtQuick.Layouts

Dialog {
    id: dialog

    // Sioul's backend (backend.rs): its words in your language and what it does.
    required property var sioul
    // The window's colours, sizes and fonts (Theme.qml).
    required property var theme
    // "The task's own project" (`time-own-project`), then the open projects, and
    // the one asked for even when closed: the choices.
    property var projects: []
    // "No task", then the open tasks (and the stretch's own).
    property var tasks: []
    // What went wrong when saving, in words; "" for nothing.
    property string problem: ""
    // The stretch being changed, by its key; "" for a new one.
    property string replacing: ""

    // The stretch was kept.
    signal saved

    // "09:30" → minutes of the day; -1 when it is no time.
    function minuteOf(text) {
        const parts = text.split(":")
        const h = Number(parts[0]), m = Number(parts[1])
        return parts.length === 2 && parts[0].trim() !== "" && h >= 0 && h < 24 && m >= 0 && m < 60 ? h * 60 + m : -1
    }

    // From one time to the other, past midnight when it ends before it begins.
    function lengthOf(fromText, toText) {
        const from = dialog.minuteOf(fromText), to = dialog.minuteOf(toText)
        if (from < 0 || to < 0)
            return 0
        return to > from ? to - from : to + 24 * 60 - from
    }

    // How long the stretch typed lasts, in minutes (`lengthOf`).
    function length() {
        return dialog.lengthOf(at.text, until.text)
    }

    // Minutes of the day as "09:30".
    function clock(minutes) {
        const pad = n => n < 10 ? "0" + n : String(n)
        const m = ((minutes % 1440) + 1440) % 1440
        return pad(Math.floor(m / 60)) + ":" + pad(m % 60)
    }

    // Today, as 2026-10-05.
    function today() {
        const d = new Date()
        const pad = n => n < 10 ? "0" + n : String(n)
        return d.getFullYear() + "-" + pad(d.getMonth() + 1) + "-" + pad(d.getDate())
    }

    // A stretch noted before, by the timer or by hand: its day, from, to, task, project, word.
    function change(entry) {
        dialog.begin(entry.project, entry.task)
        dialog.replacing = entry.key
        day.date = entry.day
        at.text = entry.at
        until.text = entry.until
        note.text = entry.note
        unbilled.checked = !entry.billable
    }

    // The form opened for a new stretch, in `project` and on `task` when given.
    function begin(project, task) {
        // Open projects, and the one asked for even when closed: a stretch changed
        // there must not move to another project unseen.
        const projects = JSON.parse(dialog.sioul.projectRows() || "[]").filter(p => p.status !== "closed" || p.id === project)
        dialog.projects = [{ id: "", title: dialog.sioul.text("time-own-project") }].concat(projects)
        dialog.tasks = [{ uid: "", title: dialog.sioul.text("time-no-task"), project: "" }].concat(JSON.parse(dialog.sioul.taskChoices(task || "") || "[]"))
        dialog.replacing = ""
        dialog.problem = ""
        day.date = dialog.today()
        // The half hour just gone, to change as it was.
        const now = new Date()
        const minutes = Math.floor((now.getHours() * 60 + now.getMinutes()) / 5) * 5
        at.text = dialog.clock(minutes - 30)
        until.text = dialog.clock(minutes)
        note.text = ""
        unbilled.checked = false
        which.currentIndex = Math.max(0, dialog.projects.findIndex(p => p.id === (project || "")))
        what.currentIndex = Math.max(0, dialog.tasks.findIndex(t => t.uid === (task || "")))
        dialog.open()
        at.forceActiveFocus()
    }

    // The form saved: the stretch kept by the backend, in the project and on the task
    // chosen; what went wrong said in the form, which stays open.
    function save() {
        const chosen = dialog.projects[which.currentIndex]
        const task = dialog.tasks[what.currentIndex]
        const project = chosen ? chosen.id : ""
        const minutes = dialog.length()
        if (minutes === 0) {
            dialog.problem = dialog.sioul.text("time-bad-hours")
            return
        }
        const problem = dialog.replacing !== ""
            ? dialog.sioul.changeTime(dialog.replacing, JSON.stringify({ day: day.date, from: at.text, to: until.text, project: project, task: task ? task.uid : "", note: note.text, unbilled: unbilled.checked }))
            : dialog.sioul.noteTime(JSON.stringify({ day: day.date, at: at.text, minutes: minutes, project: project, task: task ? task.uid : "", note: note.text, unbilled: unbilled.checked }))
        if (problem !== "") {
            dialog.problem = problem
            return
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
            text: dialog.sioul.text("time-field-task")
            color: dialog.theme.muted
        }
        PlainComboBox {
            id: what

            Layout.fillWidth: true
            model: dialog.tasks.map(t => dialog.theme.plain(t.title))
            // A task chosen: its project, unless one was chosen for this stretch.
            onActivated: index => {
                const task = dialog.tasks[index]
                if (task && task.project !== "" && which.currentIndex === 0)
                    which.currentIndex = Math.max(0, dialog.projects.findIndex(p => p.id === task.project))
            }
        }
        Label {
            text: dialog.sioul.text("time-field-project")
            color: dialog.theme.muted
        }
        PlainComboBox {
            id: which

            Layout.fillWidth: true
            model: dialog.projects.map(p => dialog.theme.plain(p.title))
        }
        Label {
            text: dialog.sioul.text("time-field-day")
            color: dialog.theme.muted
        }
        DateField {
            id: day

            theme: dialog.theme
            sioul: dialog.sioul
            locale: Qt.locale(dialog.sioul.text("qt-locale"))
            pickLabel: dialog.sioul.text("event-pick-day")
        }
        Label {
            text: dialog.sioul.text("time-field-from")
            color: dialog.theme.muted
        }
        // From, to, and how long that makes.
        RowLayout {
            spacing: 8

            TextField {
                id: at

                Layout.preferredWidth: 70
                inputMask: "99:99;_"
                inputMethodHints: Qt.ImhTime
                Accessible.name: dialog.sioul.text("time-field-from")
                onAccepted: dialog.save()
            }
            Label {
                text: dialog.sioul.text("time-field-to")
                color: dialog.theme.muted
            }
            TextField {
                id: until

                Layout.preferredWidth: 70
                inputMask: "99:99;_"
                inputMethodHints: Qt.ImhTime
                Accessible.name: dialog.sioul.text("time-field-to")
                onAccepted: dialog.save()
            }
            Label {
                readonly property int minutes: dialog.lengthOf(at.text, until.text)

                visible: minutes > 0
                text: Math.floor(minutes / 60) > 0 ? Math.floor(minutes / 60) + " h " + String(minutes % 60).padStart(2, "0") : minutes + " min"
                textFormat: Text.PlainText
                color: dialog.theme.muted
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

            visible: dialog.projects.length > 0 && !!dialog.projects[which.currentIndex] && dialog.projects[which.currentIndex].for_client === true
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
