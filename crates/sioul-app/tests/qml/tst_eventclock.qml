// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

// An event's own clock, chosen in its editor (EventDialog.qml), on a
// stand-in for Sioul: local time (the time zone shown) by default, UTC, or
// another zone found by its name (ZonePicker.qml); on another clock, the
// line beside the times says them here (`agenda::clock_line`, its words
// tested in Rust); an edit keeps the event's own clock; a whole day has
// none. Its times, by keys or by pointer alone: a moved start keeps the event's
// length, a day picked moves on to the hour beside it, Esc in a panel leaves the
// form open. Run by tools/qml-test.sh.

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
        function legSteps(key) {
            return "{}"
        }
        // A form's time on a clock, as Rust reads it (`agenda::instant_on`): UTC here.
        function wallInstant(wall, clock) {
            const [day, time] = wall.split("T")
            const [y, m, d] = day.split("-").map(Number)
            const [h, min] = time.split(":").map(Number)
            return clock === "UTC" ? Date.UTC(y, m - 1, d, h, min) / 1000 : NaN
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
            compare(line().text, "2026-11-04T15:00 – 2026-11-04T15:30 on UTC", "the times typed, on the clock chosen")
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
            compare(line().text, "2026-11-04T15:00 – 2026-11-04T15:30 on America/New_York")
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

        function startTime() {
            return find(dialog.contentItem, "eventStartTime")
        }

        function endTime() {
            return find(dialog.contentItem, "eventEndTime")
        }

        function startDay() {
            return find(dialog.contentItem, "eventStartDay")
        }

        function endDay() {
            return find(dialog.contentItem, "eventEndDay")
        }

        // A new event of an hour and a half, at 10:00 on a day.
        function openNew() {
            dialog.edit("", "2026-11-04", 10)
            tryCompare(dialog, "opened", true)
            dialog.fill("Board", "10:00", "11:30", "")
        }

        function type(keys) {
            for (const key of keys)
                keyClick(key)
        }

        function dayCell(month, day) {
            const kids = month.contentItem.children
            for (let i = 0; i < kids.length; ++i)
                if (kids[i].model && kids[i].model.day === day && kids[i].model.month === month.month)
                    return kids[i]
            return null
        }

        // Moved by typing, Up, the panel or Esc: the end follows, the event as long.
        function test_a_moved_start_keeps_the_length() {
            openNew()
            startTime().forceActiveFocus(Qt.TabFocusReason)
            type("1400")
            compare(endTime().text, "15:30", "typed")
            keyClick(Qt.Key_Up)
            compare(startTime().text, "14:05")
            compare(endTime().text, "15:35", "stepped")
            keyClick(Qt.Key_PageDown)
            compare(endTime().text, "14:35", "by an hour")
            mouseClick(find(startTime(), "clockButton"))
            mouseClick(find(startTime().panel.contentItem, "clockHour9"))
            compare(startTime().text, "09:05")
            compare(endTime().text, "10:35", "by a click")
            keyClick(Qt.Key_Escape)
            compare(startTime().text, "13:05", "Esc gives the start back")
            compare(endTime().text, "14:35", "and the end with it")
        }

        // Half typed, the length waits; `14` then Tab says the hour, and the end follows.
        function test_half_typed_keeps_the_end_until_it_reads() {
            openNew()
            startTime().forceActiveFocus(Qt.TabFocusReason)
            type("14")
            compare(endTime().text, "11:30", "not yet")
            keyClick(Qt.Key_Tab)
            compare(startTime().text, "14:00")
            compare(endTime().text, "15:30")
        }

        // Past midnight, the end's day follows; picked on the end, the length changes.
        function test_the_length_across_days_and_a_new_end() {
            openNew()
            startTime().forceActiveFocus(Qt.TabFocusReason)
            type("2345")
            compare(endTime().text, "01:15")
            compare(endDay().date, "2026-11-05", "the next day")
            startTime().forceActiveFocus(Qt.TabFocusReason)
            type("1000")
            compare(endDay().date, "2026-11-04", "and back")
            endTime().forceActiveFocus(Qt.TabFocusReason)
            type("1200")
            startTime().forceActiveFocus(Qt.TabFocusReason)
            type("0900")
            compare(endTime().text, "11:00", "two hours now")
        }

        // A later day keeps a long event's length; an edit's own length is kept from what it opened with.
        function test_a_moved_day_keeps_the_length() {
            dialog.edit("/calendars/local/personal/board.ics", "")
            tryCompare(dialog, "opened", true)
            mouseClick(find(startDay(), "dateButton"))
            const month = find(startDay().monthPopup.contentItem, "dateMonth")
            tryVerify(() => dayCell(month, 10) !== null)
            mouseClick(dayCell(month, 10))
            compare(startDay().date, "2026-11-10")
            compare(endDay().date, "2026-11-10", "the end's day with it")
            compare(endTime().text, "11:00")
            // A day picked moves on to the hour beside it, with its panel, in the form.
            verify(startTime().activeFocus, "the focus on the hour")
            verify(startTime().panelOpen, "its panel open")
            keyClick(Qt.Key_Escape)
            verify(dialog.opened, "Esc closes the panel, not the form")
            verify(!startTime().panelOpen)
            keyClick(Qt.Key_Escape)
            tryCompare(dialog, "opened", false)
        }

        // A whole day of several days keeps its span; its hours are out of the way.
        function test_a_whole_day_keeps_its_span() {
            openNew()
            find(dialog.contentItem, "eventAllDay").checked = true
            endDay().date = "2026-11-06"
            dialog.measureLength()
            startDay().date = "2026-11-10"
            dialog.startMoved()
            compare(endDay().date, "2026-11-12")
            verify(!startTime().visible)
            find(dialog.contentItem, "eventAllDay").checked = false
        }

        // The panel under the field, in a modal form: a click elsewhere in the form closes
        // it, the value kept, and reaches what it was aimed at.
        function test_a_click_elsewhere_in_the_form() {
            openNew()
            mouseClick(find(startTime(), "clockButton"))
            verify(startTime().panelOpen)
            mouseClick(find(startTime().panel.contentItem, "clockHour14"))
            compare(startTime().text, "14:00")
            const title = find(dialog.contentItem, "eventTitle")
            mouseClick(title)
            verify(!startTime().panelOpen, "closed")
            verify(title.activeFocus, "the click reached the title")
            verify(dialog.opened, "the form stays")
            compare(startTime().text, "14:00", "kept")
            compare(endTime().text, "15:30")
            dialog.save()
            compare(mock.saved.edit.start, "2026-11-04T14:00")
            compare(mock.saved.edit.end, "2026-11-04T15:30")
        }

        // From one panel to the next: the second button opens its own, the first closes.
        function test_from_one_panel_to_the_next() {
            openNew()
            mouseClick(find(startTime(), "clockButton"))
            mouseClick(find(endTime(), "clockButton"))
            verify(!startTime().panelOpen)
            verify(endTime().panelOpen)
            verify(endTime().activeFocus)
            mouseClick(find(endTime().panel.contentItem, "clockHour17"))
            compare(endTime().text, "17:30")
            compare(startTime().text, "10:00", "the end changes alone")
            keyClick(Qt.Key_Escape)
            compare(endTime().text, "11:30")
            verify(dialog.opened)
        }

        // Date and hour next to each other, in the same row.
        function test_date_and_hour_side_by_side() {
            openNew()
            verify(startTime().x > startDay().x)
            compare(startTime().y, startDay().y)
            verify(startTime().x - (startDay().x + startDay().width) < 20, "next to each other")
            compare(startTime().Accessible.name, "event-starts")
            compare(endTime().Accessible.name, "event-ends")
            verify(endTime().end, "the end's limits")
        }

        // A leg's form on another clock reads its start on that clock: its
        // side (before or after the appointment) follows (review of 4e065b1, L1).
        function test_a_leg_s_start_is_read_on_its_clock() {
            const until = Date.UTC(2026, 10, 4, 15, 0) / 1000
            dialog.makeLeg("/calendars/local/personal/board.ics", "Board", until - 3600, until, "2026-11-04", "15:30", "15:55")
            if (!dialog.opened)
                dialog.open()
            tryCompare(dialog, "opened", true)
            // On the time zone shown (Europe/Paris, an hour ahead): 14:30 UTC, before it.
            compare(dialog.legSide, "before")
            // 15:30 in UTC, after the appointment's end at 15:00 UTC.
            dialog.clock = "UTC"
            compare(dialog.legStart(), Date.UTC(2026, 10, 4, 15, 30) / 1000)
            compare(dialog.legSide, "after")
            dialog.close()
        }
    }
}
