// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

// An event's form: the title, whole days or hours, when, where; folded
// underneath, notes, how it repeats, and the calendar when there are several.

pragma ComponentBehavior: Bound

import QtQuick
import QtQuick.Controls.Basic
import QtQuick.Layouts

Dialog {
    id: dialog

    required property var sioul
    required property var theme
    // The event's file; empty for a new one.
    property string key: ""
    property var calendars: []
    property bool moreShown: false
    property string problem: ""
    // What a new event is made from: links it carries ("mid:…", "sioul:task/…").
    property var links: []
    readonly property var repeats: ["", "daily", "weekly", "monthly", "yearly"]

    signal saved

    function pad(n) {
        return n < 10 ? "0" + n : String(n)
    }

    // Opens on an event, or on a new one on `day` ("2026-10-05"), at `hour` or the next full hour.
    function edit(key, day, hour) {
        dialog.key = key
        dialog.problem = ""
        dialog.links = []
        dialog.calendars = JSON.parse(dialog.sioul.calendars() || "[]")
        if (key) {
            const found = JSON.parse(dialog.sioul.event(key) || "null")
            if (!found)
                return
            const e = found.edit
            title.text = e.title
            place.text = e.location
            notes.text = e.notes
            allDay.checked = e.all_day
            startDay.date = e.start.slice(0, 10)
            startTime.text = e.all_day ? "09:00" : e.start.slice(11, 16)
            endDay.date = e.end.slice(0, 10)
            endTime.text = e.all_day ? "10:00" : e.end.slice(11, 16)
            repeat.currentIndex = Math.max(0, dialog.repeats.indexOf(e.repeat))
            calendar.currentIndex = Math.max(0, dialog.calendars.findIndex(c => c.id === found.calendar))
            dialog.moreShown = e.notes !== "" || e.repeat !== ""
        } else {
            const now = new Date()
            hour = hour === undefined || hour < 0 ? Math.min(now.getHours() + 1, 23) : hour
            title.text = ""
            place.text = ""
            notes.text = ""
            allDay.checked = false
            startDay.date = day
            endDay.date = day
            startTime.text = dialog.pad(hour) + ":00"
            endTime.text = dialog.pad(Math.min(hour + 1, 23)) + ":" + (hour === 23 ? "59" : "00")
            repeat.currentIndex = 0
            calendar.currentIndex = 0
            dialog.moreShown = false
        }
        dialog.open()
        title.forceActiveFocus()
    }

    // A new event made from something: its title and notes given, the link kept.
    function makeFrom(text, note, link, day) {
        dialog.edit("", day)
        title.text = text
        notes.text = note
        dialog.links = link ? [link] : []
        dialog.moreShown = note !== ""
    }

    // For the window's tests: as if typed, then saved.
    function fill(text, from, to, how) {
        title.text = text
        startTime.text = from
        endTime.text = to
        repeat.currentIndex = Math.max(0, dialog.repeats.indexOf(how))
        dialog.moreShown = true
    }

    function save() {
        const where = dialog.calendars.length > 0 ? dialog.calendars[Math.max(0, calendar.currentIndex)].id : ""
        const problem = dialog.sioul.saveEvent(dialog.key, JSON.stringify(dialog.form()), where)
        if (problem) {
            dialog.problem = problem
            return
        }
        dialog.close()
        dialog.saved()
    }

    onAccepted: dialog.save()

    function form() {
        return {
            title: title.text,
            location: place.text,
            notes: notes.text,
            start: allDay.checked ? startDay.date : startDay.date + "T" + startTime.text,
            end: allDay.checked ? endDay.date : endDay.date + "T" + endTime.text,
            all_day: allDay.checked,
            repeat: dialog.repeats[repeat.currentIndex],
            links: dialog.links
        }
    }

    anchors.centerIn: Overlay.overlay
    modal: true
    width: Math.min(560, (parent ? parent.width : 560) - 32)
    title: dialog.key ? dialog.sioul.text("ui-edit-event") : dialog.sioul.text("ui-new-event")

    ColumnLayout {
        width: parent.width
        spacing: 10

        TextField {
            id: title

            Layout.fillWidth: true
            placeholderText: dialog.sioul.text("event-title")
        }
        CheckBox {
            id: allDay

            text: dialog.sioul.text("agenda-all-day")
        }
        GridLayout {
            columns: 3
            columnSpacing: 8
            rowSpacing: 6

            Label {
                text: dialog.sioul.text("event-starts")
                color: dialog.theme.muted
            }
            DateField {
                id: startDay

                theme: dialog.theme
                locale: Qt.locale(dialog.sioul.text("qt-locale"))
                pickLabel: dialog.sioul.text("event-pick-day")
                // The end follows the start when it would come before it.
                onEdited: if (endDay.date < startDay.date) endDay.date = startDay.date
            }
            TextField {
                id: startTime

                visible: !allDay.checked
                Layout.preferredWidth: 80
                inputMask: "99:99"
            }
            Label {
                text: dialog.sioul.text("event-ends")
                color: dialog.theme.muted
            }
            DateField {
                id: endDay

                theme: dialog.theme
                locale: Qt.locale(dialog.sioul.text("qt-locale"))
                pickLabel: dialog.sioul.text("event-pick-day")
            }
            TextField {
                id: endTime

                visible: !allDay.checked
                Layout.preferredWidth: 80
                inputMask: "99:99"
            }
        }
        TextField {
            id: place

            Layout.fillWidth: true
            placeholderText: dialog.sioul.text("event-where")
        }

        Button {
            flat: true
            text: (dialog.moreShown ? "▾  " : "▸  ") + dialog.sioul.text("ui-more-details")
            onClicked: dialog.moreShown = !dialog.moreShown
        }
        GridLayout {
            visible: dialog.moreShown
            Layout.fillWidth: true
            columns: 2
            columnSpacing: 8
            rowSpacing: 6

            Label {
                Layout.alignment: Qt.AlignTop
                text: dialog.sioul.text("contact-notes")
                color: dialog.theme.muted
            }
            TextArea {
                id: notes

                // A field shows where it is: a border, darker when it has the focus.
                background: Rectangle {
                    color: dialog.theme.surface
                    radius: dialog.theme.radius
                    border.color: notes.activeFocus ? dialog.theme.focus : dialog.theme.line
                }
                Layout.fillWidth: true
                Layout.preferredHeight: 80
                wrapMode: TextArea.Wrap
            }
            Label {
                text: dialog.sioul.text("event-repeat")
                color: dialog.theme.muted
            }
            ComboBox {
                id: repeat

                Layout.fillWidth: true
                model: dialog.repeats.map(r => dialog.sioul.text("repeat-" + (r || "none")))
            }
            Label {
                visible: dialog.calendars.length > 1 && dialog.key === ""
                text: dialog.sioul.text("event-calendar")
                color: dialog.theme.muted
            }
            ComboBox {
                id: calendar

                visible: dialog.calendars.length > 1 && dialog.key === ""
                Layout.fillWidth: true
                model: dialog.calendars.map(c => dialog.theme.plain(c.name))
            }
        }
        Label {
            visible: dialog.problem !== ""
            Layout.fillWidth: true
            text: dialog.problem
            textFormat: Text.PlainText
            wrapMode: Text.Wrap
            color: dialog.theme.warm
        }
        RowLayout {
            Layout.fillWidth: true

            Item {
                Layout.fillWidth: true
            }
            Button {
                text: dialog.sioul.text("ui-cancel")
                onClicked: dialog.close()
            }
            Button {
                text: dialog.sioul.text("ui-save")
                highlighted: true
                onClicked: dialog.save()
            }
        }
    }
}
