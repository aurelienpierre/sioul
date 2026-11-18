// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

// The repeat part of the event form (EventDialog.qml; 18 November 2026) on
// a stand-in for Sioul, at a computer's width and at a phone's (412): a
// series ends never, on a day or after a number of times, read back from
// its file and saved as the form says; it pauses during my vacations,
// public holidays and each school-holidays calendar by its name, ticked
// together; pauses are not offered on a meeting with others, nor on what
// does not repeat. Fiction only. Run by tools/qml-test.sh.

import QtQuick
import QtQuick.Controls.Basic
import QtTest
import "../../qml"

Item {
    id: root

    width: 1344
    height: 1000

    Theme {
        id: testTheme
    }

    QtObject {
        id: mock

        property var saved: null
        property var forms: ({
            "/calendars/local/personal/swimming.ics": { edit: { repeat: "weekly", every: 1, ends: "until", until: "2031-06-25", count: 0, pauses: ["school/zone-a", "vacations"] } },
            "/calendars/local/personal/market.ics": { edit: { repeat: "weekly", every: 1, ends: "count", until: "", count: 12, pauses: [] } },
            "/calendars/murena/personal/review.ics": { edit: { repeat: "weekly", every: 1, ends: "", until: "", count: 0, pauses: [] }, with_others: true }
        })

        function text(key) { return key }
        function zoneName() { return "Europe/Paris" }
        function clockLine(start, end, clock) { return "" }
        function textWith(key, name, value) { return key + ":" + value }
        function textArgs(key, args) { return key }
        function calendars() { return JSON.stringify([{ id: "local/personal", name: "Personal · on this device only", color: null }, { id: "murena/personal", name: "Personal · example.org", color: null }]) }
        function reminderChoices(current) { return "[]" }
        function pauseSources() {
            return JSON.stringify({ sources: [{ id: "vacations", name: "my vacations" }, { id: "public", name: "public holidays" }, { id: "school/zone-a", name: "Zone A" }] })
        }
        function eventForm(key) {
            const found = mock.forms[key]
            if (!found)
                return ""
            return JSON.stringify({
                edit: Object.assign({ title: "Swimming", location: "", notes: "", start: "2030-10-02T17:00", end: "2030-10-02T18:00", all_day: false, margins: { before: 0, after: 0 }, demands: {}, remind: "" }, found.edit),
                calendar: key.indexOf("murena") >= 0 ? "murena/personal" : "local/personal",
                calendar_name: "Personal",
                here_only: "",
                stays: "",
                with_others: found.with_others === true,
                read_only: false
            })
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
        name: "EventRepeat"
        when: windowShown

        function all(item, test, out) {
            out = out || []
            if (!item || out.seen === undefined && false)
                return out
            if (out.indexOf(item) >= 0 || (out.walked || []).indexOf(item) >= 0)
                return out
            out.walked = (out.walked || []).concat([item])
            if (test(item))
                out.push(item)
            for (let i = 0; i < (item.children || []).length; ++i)
                all(item.children[i], test, out)
            if (item.contentItem && item.contentItem !== item)
                all(item.contentItem, test, out)
            return out
        }

        function find(name) {
            const found = all(dialog.contentItem, c => c.objectName === name)
            return found.length > 0 ? found[0] : null
        }

        function pauses() {
            return all(dialog.contentItem, c => c.objectName === "eventPause")
        }

        function init() {
            mock.saved = null
            dialog.close()
        }

        function test_a_series_ends_and_pauses_data() {
            return [{ tag: "computer", width: 1344 }, { tag: "phone", width: 412 }]
        }

        // A series that ends on a day and pauses for the zone's holidays and my vacations.
        function test_a_series_ends_and_pauses(data) {
            root.width = data.width
            // At a phone's width, the form as narrow as the phone's window would make it, from its opening.
            if (data.width < 600)
                dialog.width = data.width - 2 * testTheme.gap
            dialog.edit("/calendars/local/personal/swimming.ics", "")
            tryCompare(dialog, "opened", true)
            verify(dialog.moreShown, "a series unfolds the details")
            const ends = find("eventRepeatEnds")
            verify(ends.visible && ends.enabled)
            compare(ends.currentIndex, 1, "on a day")
            verify(find("eventRepeatUntil").visible && !find("eventRepeatCount").visible)
            compare(find("eventRepeatUntil").date, "2031-06-25")
            const boxes = pauses()
            compare(boxes.map(b => b.text), ["my vacations", "public holidays", "Zone A"])
            verify(boxes[0].checked && !boxes[1].checked && boxes[2].checked)
            // Public holidays ticked too: three together.
            // Ticked as a press does (on a phone it lies below the fold of the form).
            boxes[1].toggle()
            boxes[1].toggled()
            dialog.save()
            compare(mock.saved.edit.ends, "until")
            compare(mock.saved.edit.until, "2031-06-25")
            compare(mock.saved.edit.count, 0)
            compare(mock.saved.edit.pauses.slice().sort(), ["public", "school/zone-a", "vacations"])
            // Its new rows never wider than the form's other rows (the details' grid; that grid's
            // own minimum, from rows of before, is the form's at this width).
            const grid = find("eventRepeat").parent
            for (const item of [find("eventRepeatEnds"), find("eventRepeatUntil")].concat(pauses()))
                tryVerify(() => item.mapToItem(grid, 0, 0).x + item.width <= grid.width + 1, 2000, item.objectName + " fits: " + (item.mapToItem(grid, 0, 0).x + item.width) + " of " + grid.width)
        }

        // After a number of times; then never; then no repeat at all: no end, no pause said.
        function test_a_series_ends_after_a_number_of_times_or_never() {
            dialog.edit("/calendars/local/personal/market.ics", "")
            tryCompare(dialog, "opened", true)
            const ends = find("eventRepeatEnds")
            compare(ends.currentIndex, 2)
            verify(find("eventRepeatCount").visible)
            compare(find("eventRepeatCount").value, 12)
            dialog.save()
            compare([mock.saved.edit.ends, mock.saved.edit.count, mock.saved.edit.until], ["count", 12, ""])
            dialog.edit("/calendars/local/personal/market.ics", "")
            tryCompare(dialog, "opened", true)
            find("eventRepeatEnds").currentIndex = 0
            dialog.save()
            compare([mock.saved.edit.ends, mock.saved.edit.count], ["", 0])
            dialog.edit("/calendars/local/personal/market.ics", "")
            tryCompare(dialog, "opened", true)
            find("eventRepeat").currentIndex = 0
            verify(!find("eventRepeatEnds").visible && pauses().every(b => !b.visible), "nothing of a series on what does not repeat")
            dialog.save()
            compare([mock.saved.edit.repeat, mock.saved.edit.ends, mock.saved.edit.pauses.length], ["", "", 0])
        }

        // A meeting with others: its repeat is its organizer's, and no pause is offered.
        function test_no_pause_on_a_meeting_with_others() {
            dialog.edit("/calendars/murena/personal/review.ics", "")
            tryCompare(dialog, "opened", true)
            verify(!find("eventRepeatEnds").enabled, "its end is its organizer's")
            verify(pauses().every(b => !b.visible), "no pause offered")
            dialog.save()
            compare(mock.saved.edit.pauses.length, 0)
        }

        // A new event: no end, no pause ticked; weekly chosen, the choices offered.
        function test_a_new_series_offers_its_end_and_pauses() {
            dialog.edit("", "2030-10-02", 17)
            tryCompare(dialog, "opened", true)
            dialog.moreShown = true
            compare(find("eventRepeatEnds").currentIndex, 0)
            find("eventRepeat").currentIndex = 2
            verify(pauses().length === 3 && pauses().every(b => b.visible && !b.checked))
        }
    }
}
