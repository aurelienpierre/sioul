// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

// An event's calendar, chosen in its editor (EventDialog.qml), on a stand-in
// for Sioul: an event of a server's "Personnel" could not be moved
// to a calendar of the computer's own, nor back: the
// chooser was folded under "More details", out of sight. It shows now with
// the details folded, on an event of a calendar it offers, says where the event
// is, can be changed, and the save hands the chosen calendar to `saveEvent`;
// an event of a calendar not offered (read only) stays where it is. Run by
// tools/qml-test.sh.

import QtQuick
import QtQuick.Controls.Basic
import QtTest
import "../../qml"

Item {
    id: root

    width: 900
    height: 800

    Theme {
        id: testTheme
    }

    QtObject {
        id: mock

        property string status: ""
        property var saved: null
        // Made up: no real calendar, no real person.
        property var offered: [
            { id: "murena/personnel", name: "Personnel · example.org · events and tasks", color: null },
            { id: "local/personal", name: "Personal · on this device only", color: null },
            { id: "local/family", name: "Family · on this device only", color: null }
        ]
        property var forms: ({
            "/calendars/murena/personnel/dentist.ics": { calendar: "murena/personnel", calendar_name: "Personnel · example.org · events and tasks" },
            "/calendars/local/personal/walk.ics": { calendar: "local/personal", calendar_name: "Personal · on this device only" },
            "/calendars/murena/birthdays/ana.ics": { calendar: "murena/birthdays", calendar_name: "Birthdays · example.org" },
            "/calendars/murena/personnel/kept.ics": { calendar: "murena/personnel", calendar_name: "Personnel · example.org · events and tasks", margins: { before: 25, after: 60 } }
        })

        function text(key) {
            return key
        }
        function textWith(key, name, value) {
            return key + ":" + value
        }
        function calendars() {
            return JSON.stringify(mock.offered)
        }
        function eventForm(key) {
            const found = mock.forms[key]
            if (!found)
                return ""
            return JSON.stringify({
                edit: { title: "Dentist", location: "", notes: "", start: "2026-10-23T13:00", end: "2026-10-23T13:40", all_day: false, repeat: "", margins: found.margins || { before: 0, after: 0 }, demands: {}, remind: "" },
                calendar: found.calendar,
                calendar_name: found.calendar_name,
                here_only: "",
                read_only: false
            })
        }
        function reminderChoices(current) {
            return "[]"
        }
        function saveEvent(key, edit, calendar) {
            mock.saved = { key: key, edit: JSON.parse(edit), calendar: calendar }
            return ""
        }
    }

    EventDialog {
        id: dialog

        sioul: mock
        theme: testTheme
    }

    TestCase {
        name: "EventCalendar"
        when: windowShown

        function find(item, name) {
            if (!item)
                return null
            if (item.objectName === name)
                return item
            const kids = item.children || []
            for (let i = 0; i < kids.length; ++i) {
                const found = find(kids[i], name)
                if (found)
                    return found
            }
            if (item.contentItem && item.contentItem !== item) {
                const inside = find(item.contentItem, name)
                if (inside)
                    return inside
            }
            return null
        }

        function chooser() {
            return find(dialog.contentItem, "event-calendar")
        }

        function init() {
            mock.saved = null
            dialog.close()
        }

        // An event of the server's "Personnel": shown in its calendar, moved
        // to one of this device's own, and back.
        function test_an_event_moves_to_another_calendar() {
            dialog.edit("/calendars/murena/personnel/dentist.ics", "")
            tryCompare(dialog, "opened", true)
            const box = chooser()
            verify(box !== null, "the chooser is there")
            verify(!dialog.moreShown, "the details folded, as the editor opens")
            verify(box.visible && box.enabled, "shown, in sight, and can be changed")
            compare(box.currentIndex, 0, "it says where the event is")
            box.currentIndex = 1
            box.activated(1)
            dialog.save()
            verify(mock.saved !== null, "saved")
            compare(mock.saved.key, "/calendars/murena/personnel/dentist.ics")
            compare(mock.saved.calendar, "local/personal", "the calendar chosen goes with the save")

            mock.saved = null
            dialog.edit("/calendars/local/personal/walk.ics", "")
            tryCompare(dialog, "opened", true)
            compare(chooser().currentIndex, 1, "the event's own calendar, not the first")
            chooser().currentIndex = 0
            chooser().activated(0)
            dialog.save()
            compare(mock.saved.calendar, "murena/personnel", "back to the server's")
        }

        // Opened twice, the second event's calendar is the one said: the box
        // never keeps the first one's choice, nor falls back to the first line.
        function test_the_calendar_said_is_the_event_s_own_each_time() {
            dialog.edit("/calendars/local/personal/walk.ics", "")
            tryCompare(dialog, "opened", true)
            chooser().currentIndex = 2
            dialog.close()
            dialog.edit("/calendars/murena/personnel/dentist.ics", "")
            tryCompare(dialog, "opened", true)
            compare(chooser().currentIndex, 0)
            dialog.save()
            compare(mock.saved.calendar, "murena/personnel", "unchanged: it stays")
        }

        // A calendar not offered (read only): no chooser, the event stays.
        function test_an_event_of_a_calendar_read_only_stays() {
            dialog.edit("/calendars/murena/birthdays/ana.ics", "")
            tryCompare(dialog, "opened", true)
            verify(!chooser().visible)
            dialog.save()
            compare(mock.saved.calendar, "murena/birthdays")
        }

        // The time an event keeps before and after it, its own, always shows
        // in its form, whatever another program wrote (25 min is not among the
        // usual choices), and is saved as it is: never hidden as "unsaid".
        function test_the_time_an_event_keeps_shows() {
            dialog.edit("/calendars/murena/personnel/kept.ics", "")
            tryCompare(dialog, "opened", true)
            verify(dialog.moreShown, "unfolded: it keeps time")
            const before = find(dialog.contentItem, "event-before")
            const after = find(dialog.contentItem, "event-after")
            compare(before.currentText, "25 min")
            compare(after.currentText, "1 h")
            dialog.save()
            compare(mock.saved.edit.margins.before, 25)
            compare(mock.saved.edit.margins.after, 60)
        }

        // A new event goes into the calendar chosen, the first by default.
        function test_a_new_event_goes_where_it_is_put() {
            dialog.edit("", "2026-10-23")
            tryCompare(dialog, "opened", true)
            verify(chooser().visible)
            compare(chooser().currentIndex, 0)
            chooser().currentIndex = 2
            dialog.fill("Walk", "14:00", "15:00", "")
            dialog.save()
            compare(mock.saved.key, "")
            compare(mock.saved.calendar, "local/family")
        }
    }
}
