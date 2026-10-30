// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

// An event's own clock, chosen in its editor (EventDialog.qml), on a
// stand-in for Sioul: local time (the time zone shown) by default, UTC, or
// another zone found by its name (ZonePicker.qml); on another clock, the
// line beside the times says them here (`agenda::clock_line`, its words
// tested in Rust); an edit keeps the event's own clock; a whole day has
// none. Run by tools/qml-test.sh.

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
        property var forms: ({
            "/calendars/local/personal/board.ics": { start: "2026-11-04T10:00", end: "2026-11-04T11:00", clock: "America/New_York" },
            "/calendars/local/personal/call.ics": { start: "2026-11-04T15:00", end: "2026-11-04T16:00", clock: "UTC" }
        })

        function text(key) {
            return key
        }
        function textWith(key, name, value) {
            return key + ":" + value
        }
        function zoneName() {
            return "Europe/Paris"
        }
        // What the form hands, said back: the test reads what it was asked.
        function clockLine(start, end, clock) {
            return start + " – " + end + " on " + clock
        }
        function zoneNames(query) {
            return JSON.stringify(["America/New_York", "America/North_Dakota/New_Salem"].filter(n => n.toLowerCase().replace(/_/g, " ").indexOf(query.toLowerCase()) >= 0))
        }
        function calendars() {
            return JSON.stringify([{ id: "local/personal", name: "Personal · on this device only", color: null }])
        }
        function eventForm(key) {
            const found = mock.forms[key]
            if (!found)
                return ""
            return JSON.stringify({
                edit: { title: "Board", location: "", notes: "", start: found.start, end: found.end, all_day: false, clock: found.clock, repeat: "", margins: { before: 0, after: 0 }, demands: {}, remind: "" },
                calendar: "local/personal",
                calendar_name: "Personal · on this device only",
                here_only: "",
                stays: "",
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
        name: "EventClock"
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

        function box() {
            return find(dialog.contentItem, "eventClock")
        }

        function line() {
            return find(dialog.contentItem, "eventClockLine")
        }

        // As a press on a line does, the box's own index left to its binding.
        function choose(index) {
            box().activated(index)
            compare(box().currentIndex, index, "the box says the line chosen")
        }

        function picker() {
            return find(dialog.contentItem, "eventZone")
        }

        function init() {
            mock.saved = null
            dialog.close()
        }

        // A new event is on local time, said with the zone shown, and no line beside.
        function test_a_new_event_is_on_local_time() {
            dialog.edit("", "2026-11-04", 15)
            tryCompare(dialog, "opened", true)
            verify(box().visible, "the clock is offered")
            compare(box().currentIndex, 0)
            compare(box().currentText, "event-clock-local:Europe/Paris")
            verify(!line().visible, "nothing to say beside local times")
            dialog.save()
            compare(mock.saved.edit.clock, "", "written on the time zone shown")
        }

        // UTC chosen: the line beside says the times here, as typed; the save keeps UTC.
        function test_the_form_s_conversion_line() {
            dialog.edit("", "2026-11-04", 15)
            tryCompare(dialog, "opened", true)
            choose(1)
            compare(dialog.clock, "UTC")
            verify(line().visible)
            compare(line().text, "2026-11-04T15:00 – 2026-11-04T16:00 on UTC", "the times typed, on the clock chosen")
            dialog.fill("Call", "16:30", "17:00", "")
            compare(line().text, "2026-11-04T16:30 – 2026-11-04T17:00 on UTC", "said again as the times change")
            dialog.save()
            compare(mock.saved.edit.clock, "UTC")
            compare(mock.saved.edit.start, "2026-11-04T16:30", "the times as typed, on UTC")
        }

        // Another zone, found by typing part of its name, picked from the matches.
        function test_another_zone_is_found_by_its_name() {
            dialog.edit("", "2026-11-04", 15)
            tryCompare(dialog, "opened", true)
            choose(2)
            verify(picker().visible, "the search shows")
            compare(dialog.clock, "", "local until a zone is picked")
            const query = find(picker(), "zoneQuery")
            query.text = "new y"
            query.textEdited()
            compare(picker().matches.length, 1)
            picker().pick(picker().matches[0])
            compare(dialog.clock, "America/New_York")
            compare(box().currentIndex, 2, "still on another zone")
            compare(query.text, "America/New_York", "the zone picked, in its field")
            compare(line().text, "2026-11-04T15:00 – 2026-11-04T16:00 on America/New_York")
            dialog.save()
            compare(mock.saved.edit.clock, "America/New_York")
        }

        // An edit keeps the event's own clock, shown as it is.
        function test_an_edit_keeps_the_event_s_clock() {
            dialog.edit("/calendars/local/personal/board.ics", "")
            tryCompare(dialog, "opened", true)
            compare(box().currentIndex, 2)
            verify(picker().visible)
            compare(find(picker(), "zoneQuery").text, "America/New_York", "its zone, in the field")
            compare(line().text, "2026-11-04T10:00 – 2026-11-04T11:00 on America/New_York")
            dialog.save()
            compare(mock.saved.edit.clock, "America/New_York")
            compare(mock.saved.edit.start, "2026-11-04T10:00")

            dialog.edit("/calendars/local/personal/call.ics", "")
            tryCompare(dialog, "opened", true)
            compare(box().currentIndex, 1)
            dialog.save()
            compare(mock.saved.edit.clock, "UTC")
        }

        // Back to local time, and a whole day: no clock written.
        function test_local_time_again_and_a_whole_day() {
            dialog.edit("/calendars/local/personal/board.ics", "")
            tryCompare(dialog, "opened", true)
            choose(0)
            verify(!line().visible)
            dialog.save()
            compare(mock.saved.edit.clock, "")

            dialog.edit("/calendars/local/personal/call.ics", "")
            tryCompare(dialog, "opened", true)
            find(dialog.contentItem, "eventAllDay").checked = true
            verify(!box().visible, "a whole day has no clock")
            dialog.save()
            compare(mock.saved.edit.clock, "")
        }
    }
}
