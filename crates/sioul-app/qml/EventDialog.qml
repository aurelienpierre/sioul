// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

// An event's form: the title, whole days or hours, when, where; folded
// underneath, notes, how it repeats, the time kept before and after it
// (getting there, getting ready, coming back), when it is reminded (as
// usual, not this one, or so long before: docs/reminders.md), what it costs
// and gives back (four costs and a gain as tiles, 0 to 10, unsaid until
// said: CostTiles.qml), and the calendar when there are several. Opened from an event's details ("Edit"),
// or new.

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
    readonly property var marginMinutes: [0, 5, 10, 15, 20, 30, 45, 60, 90, 120]
    // Minutes before and after; the four costs and the gain, 0 to 10 or unsaid (null).
    property var around: ({ before: 0, after: 0 })
    property var demands: ({ cognitive: null, emotional: null, anxiety: null, body: null, gain: null })
    // Its reminder before it: "" as usual, "none", or minutes ("15"); and the choices offered.
    property string remind: ""
    property var reminds: []

    signal saved

    function pad(n) {
        return n < 10 ? "0" + n : String(n)
    }

    function minutesText(m) {
        return m < 60 ? m + " min" : Math.floor(m / 60) + " h" + (m % 60 ? " " + dialog.pad(m % 60) : "")
    }

    // A cost or the gain, 0 to 10, or null when unsaid; kept until Save.
    function rate(name, value) {
        const demands = Object.assign({}, dialog.demands)
        demands[name] = value
        dialog.demands = demands
    }

    function rating(name) {
        const value = dialog.demands[name]
        return value === undefined ? null : value
    }

    // Opens on an event, or on a new one on `day` ("2026-10-05"), at `hour` or the next full hour.
    function edit(key, day, hour) {
        dialog.key = key
        dialog.problem = ""
        dialog.links = []
        dialog.calendars = JSON.parse(dialog.sioul.calendars() || "[]")
        if (key) {
            const found = JSON.parse(dialog.sioul.eventForm(key) || "null")
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
            dialog.around = Object.assign({ before: 0, after: 0 }, e.margins)
            dialog.demands = Object.assign({ cognitive: null, emotional: null, anxiety: null, body: null, gain: null }, e.demands)
            dialog.remind = e.remind || ""
            dialog.reminds = JSON.parse(dialog.sioul.reminderChoices(dialog.remind) || "[]")
            dialog.moreShown = e.notes !== "" || e.repeat !== "" || dialog.around.before > 0 || dialog.around.after > 0 || dialog.remind !== "" || ["cognitive", "emotional", "anxiety", "body", "gain"].some(n => dialog.rating(n) !== null)
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
            dialog.around = { before: 0, after: 0 }
            dialog.demands = { cognitive: null, emotional: null, anxiety: null, body: null, gain: null }
            dialog.remind = ""
            dialog.reminds = JSON.parse(dialog.sioul.reminderChoices("") || "[]")
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

    // For the window's pictures: the costs and the gain in view.
    function showRatings() {
        dialog.moreShown = true
        Qt.callLater(() => {
            const top = tiles.mapToItem(scroll.contentItem.contentItem, 0, 0).y
            scroll.contentItem.contentY = Math.max(0, Math.min(top - 12, scroll.contentItem.contentHeight - scroll.height))
        })
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
            margins: dialog.around,
            demands: dialog.demands,
            remind: dialog.remind,
            links: dialog.links
        }
    }

    // Over the whole window, whatever item made it (a Later's is 0 by 0).
    parent: Overlay.overlay
    anchors.centerIn: Overlay.overlay
    modal: true
    // As wide as what it holds, without a cap of its own (docs/qt-quick.md, "An
    // editing card's width"): its fields at about 70 characters of the body
    // font, a readable line, or its tiles side by side, whichever is wider;
    // never past the window less the usual margins, all of a phone's.
    width: Math.min(Math.ceil(Math.max(70 * body.averageCharacterWidth, tiles.naturalWidth)) + dialog.leftPadding + dialog.rightPadding + 12, (parent ? parent.width : 800) - 2 * dialog.theme.gap)
    // Taller than the window, every detail unfolded: the form scrolls, its buttons stay.
    height: Math.min(implicitHeight, (Overlay.overlay ? Overlay.overlay.height : 800) - 32)
    title: dialog.key ? dialog.sioul.text("ui-edit-event") : dialog.sioul.text("ui-new-event")

    FontMetrics {
        id: body

        font: dialog.font
    }

    ScrollView {
        id: scroll

        anchors.fill: parent
        contentWidth: availableWidth
        clip: true

        ColumnLayout {
            width: scroll.availableWidth
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
                    sioul: dialog.sioul
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
                    sioul: dialog.sioul
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
                // Getting there and back, getting ready: kept free around it, never a pause.
                Label {
                    Layout.maximumWidth: 160
                    text: dialog.sioul.text("task-field-before")
                    wrapMode: Text.Wrap
                    color: dialog.theme.muted
                }
                ComboBox {
                    Layout.fillWidth: true
                    model: dialog.marginMinutes.map(m => m === 0 ? dialog.sioul.text("task-rating-unsaid") : dialog.minutesText(m))
                    currentIndex: Math.max(0, dialog.marginMinutes.indexOf(dialog.around.before))
                    onActivated: index => dialog.around = Object.assign({}, dialog.around, { before: dialog.marginMinutes[index] })
                }
                Label {
                    Layout.maximumWidth: 160
                    text: dialog.sioul.text("task-field-after")
                    wrapMode: Text.Wrap
                    color: dialog.theme.muted
                }
                ComboBox {
                    Layout.fillWidth: true
                    model: dialog.marginMinutes.map(m => m === 0 ? dialog.sioul.text("task-rating-unsaid") : dialog.minutesText(m))
                    currentIndex: Math.max(0, dialog.marginMinutes.indexOf(dialog.around.after))
                    onActivated: index => dialog.around = Object.assign({}, dialog.around, { after: dialog.marginMinutes[index] })
                }
                // Its reminder: as usual (Settings ▸ Reminders and notifications),
                // not this one, or so long before it and its time to get there.
                Label {
                    Layout.maximumWidth: 160
                    text: dialog.sioul.text("event-remind")
                    wrapMode: Text.Wrap
                    color: dialog.theme.muted
                }
                ComboBox {
                    Layout.fillWidth: true
                    textRole: "label"
                    model: dialog.reminds
                    currentIndex: Math.max(0, dialog.reminds.findIndex(c => c.value === dialog.remind))
                    onActivated: index => dialog.remind = dialog.reminds[index].value
                }
                // What it costs, and what it gives back: four tiles and a row, 0 to 10
                // each, unsaid until said (CostTiles.qml); kept until Save.
                CostTiles {
                    id: tiles

                    Layout.columnSpan: 2
                    Layout.fillWidth: true
                    Layout.topMargin: 4
                    Layout.bottomMargin: 4
                    sioul: dialog.sioul
                    theme: dialog.theme
                    values: dialog.demands
                    onEdited: (name, value) => dialog.rate(name, value)
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
        }
    }

    footer: ColumnLayout {
        spacing: 6

        Label {
            visible: dialog.problem !== ""
            Layout.fillWidth: true
            Layout.leftMargin: 12
            Layout.rightMargin: 12
            text: dialog.problem
            textFormat: Text.PlainText
            wrapMode: Text.Wrap
            color: dialog.theme.warm
        }
        RowLayout {
            Layout.fillWidth: true
            Layout.leftMargin: 12
            Layout.rightMargin: 12
            Layout.bottomMargin: 12

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
