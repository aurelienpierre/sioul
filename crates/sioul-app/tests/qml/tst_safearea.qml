// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

// Android's navigation buttons stood in for (Qt's SafeArea, its additional
// margins on the window's root item): a 48-point bar below, then beside, as
// on a phone held sideways (review of 4c77d0c, L9). The event form unfolded,
// a calendar file's dialog with many events and a long menu each lie inside
// the safe rectangle, their buttons too. Run by tools/qml-test.sh.

import QtQuick
import QtQuick.Controls.Basic
import QtTest
import "../../qml"

Item {
    id: root

    width: 412
    height: 891

    Theme {
        id: testTheme
    }

    QtObject {
        id: mock

        // Made up: no real calendar, no real person.
        function text(key) {
            return key
        }
        function textWith(key, name, value) {
            return key + ":" + value
        }
        function textArgs(key, args) {
            return key
        }
        function zoneName() {
            return "Europe/Paris"
        }
        function clockLine(start, end, clock) {
            return ""
        }
        function zoneNames(query) {
            return "[]"
        }
        function calendars() {
            return JSON.stringify([{ id: "local/personal", name: "Personal", color: null }])
        }
        function eventForm(key) {
            return ""
        }
        function reminderChoices(current) {
            return "[]"
        }
        function legSteps(key) {
            return "{}"
        }
        function calendarFileKnown(calendar) {
            return "[]"
        }
        function closeCalendarFile() {
        }
    }

    EventDialog {
        id: eventForm

        sioul: mock
        theme: testTheme
    }

    CalendarFileDialog {
        id: calendarFile

        sioul: mock
        theme: testTheme
    }

    SioulMenu {
        id: menu

        Repeater {
            model: 30

            MenuItem {
                required property int index

                text: "Choice " + index
            }
        }
    }

    TestCase {
        name: "SafeArea"
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

        // The window's safe rectangle with Android's bar stood in for.
        function bars(top, right, bottom, left) {
            const margins = root.Window.window.contentItem.SafeArea.additionalMargins
            margins.top = top
            margins.right = right
            margins.bottom = bottom
            margins.left = left
        }

        // `item` within the safe rectangle: what it covers, in the window's points.
        function inside(what, item, top, right, bottom, left) {
            const at = item.mapToItem(root, 0, 0)
            verify(at.y >= top - 0.5 && at.y + item.height <= root.height - bottom + 0.5, what + ": from " + Math.round(at.y) + " to " + Math.round(at.y + item.height) + ", the bar from " + (root.height - bottom))
            verify(at.x >= left - 0.5 && at.x + item.width <= root.width - right + 0.5, what + ": from " + Math.round(at.x) + " to " + Math.round(at.x + item.width) + ", the bar from " + (root.width - right))
        }

        function cleanup() {
            eventForm.close()
            calendarFile.close()
            menu.close()
            bars(0, 0, 0, 0)
        }

        function test_the_event_form_and_its_buttons_stay_above_a_bar_below() {
            bars(0, 0, 48, 0)
            compare(root.Overlay.overlay.SafeArea.margins.bottom, 48, "the overlay knows the bar")
            eventForm.edit("", "2026-11-04")
            eventForm.fill("Piano lesson", "17:00", "18:00", "weekly")
            tryCompare(eventForm, "opened", true)
            // Laid out, every field unfolded: taller than the room left.
            tryVerify(() => eventForm.implicitHeight > root.height - 48, 2000, "the form unfolded is tall: " + eventForm.implicitHeight)
            inside("the event form", eventForm.contentItem.parent, 0, 0, 48, 0)
            inside("Cancel", find(eventForm.footer, "eventCancel"), 0, 0, 48, 0)
        }

        function test_the_event_form_stays_clear_of_a_bar_beside() {
            bars(0, 48, 0, 0)
            eventForm.edit("", "2026-11-04")
            tryCompare(eventForm, "opened", true)
            waitForRendering(eventForm.contentItem)
            inside("the event form, sideways", eventForm.contentItem.parent, 0, 48, 0, 0)
            inside("Cancel, sideways", find(eventForm.footer, "eventCancel"), 0, 48, 0, 0)
        }

        function test_a_long_dialog_keeps_its_buttons_in_view() {
            bars(24, 0, 48, 0)
            const events = []
            for (let i = 0; i < 30; ++i)
                events.push({ uid: "e" + i + "@example.org", title: "Event " + i, when: "Wednesday 4 November, 16:00 – 16:15", there: "", place: "Somewhere", organizer: "", notes: "" })
            calendarFile.show(JSON.stringify({ problem: "", events: events, calendars: [{ id: "local/personal", name: "Personal" }], categories: [] }))
            tryCompare(calendarFile, "opened", true)
            tryVerify(() => calendarFile.implicitHeight > root.height, 2000, "thirty events: taller than the window")
            inside("the calendar file's dialog", calendarFile.contentItem.parent, 24, 0, 48, 0)
            inside("Add to the agenda", find(calendarFile.footer, "calendarFileAdd"), 24, 0, 48, 0)
        }

        function test_a_long_menu_stays_above_the_bar() {
            bars(0, 0, 48, 0)
            menu.popup(root, 20, root.height - 60)
            tryCompare(menu, "opened", true)
            inside("the menu", menu.contentItem.parent, 0, 0, 48, 0)
        }
    }
}
