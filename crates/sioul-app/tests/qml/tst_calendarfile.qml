// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

// A calendar file opened in Sioul (CalendarFileDialog.qml), on a stand-in
// for Sioul: its event shown, both times when on another clock; the agenda
// chosen says when it holds the event already; the categories chosen go
// with Add; Cancel writes nothing; a file that does not open says why, with
// Close only. The reading and the writing are tested in Rust
// (agenda::tests::a_calendar_file_*). Run by tools/qml-test.sh.

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

        property var added: null
        property int closed: 0
        // Made up: no real calendar, no real person.
        readonly property string view: JSON.stringify({
            problem: "",
            events: [{ uid: "visio-42@booking.example.org", title: "Visio de suivi", when: "Wednesday 4 November, 16:00 – 16:15", there: "15:00 – 15:15 Paris · 16:00 – 16:15 here", place: "En ligne", organizer: "By Camille Exemple", notes: "Bonjour" }],
            calendars: [{ id: "local/personal", name: "Personal" }, { id: "dav/work", name: "Work" }],
            categories: ["Travail", "Santé"]
        })

        function text(key) {
            return key
        }
        function textWith(key, name, value) {
            return key + ":" + value
        }
        function calendarFileKnown(calendar) {
            return calendar === "dav/work" ? JSON.stringify(["visio-42@booking.example.org"]) : "[]"
        }
        function addCalendarFile(calendar, categories) {
            mock.added = { calendar: calendar, categories: JSON.parse(categories) }
            return ""
        }
        function closeCalendarFile() {
            mock.closed += 1
        }
    }

    CalendarFileDialog {
        id: dialog

        sioul: mock
        theme: testTheme
    }

    TestCase {
        name: "CalendarFile"
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

        function init() {
            mock.added = null
            mock.closed = 0
        }

        function test_an_event_is_added_with_its_agenda_and_tags() {
            dialog.show(mock.view)
            tryCompare(dialog, "opened", true)
            verify(dialog.width <= root.width, "within a phone's width")
            compare(find(dialog.contentItem, "calendarFileKnown").visible, false, "not in Personal")
            // The work agenda holds it already: said before it is written over.
            const agenda = find(dialog.contentItem, "calendarFileAgenda")
            agenda.currentIndex = 1
            agenda.activated(1)
            verify(find(dialog.contentItem, "calendarFileKnown").visible)
            dialog.addTag("#Travail")
            dialog.addTag("travail")
            dialog.addTag("Rendez-vous")
            compare(dialog.tags, ["Travail", "Rendez-vous"])
            dialog.add()
            compare(mock.added.calendar, "dav/work")
            compare(mock.added.categories, ["Travail", "Rendez-vous"])
            tryCompare(dialog, "opened", false)
            compare(mock.closed, 0, "added: nothing set aside")
        }

        function test_cancel_writes_nothing() {
            dialog.show(mock.view)
            tryCompare(dialog, "opened", true)
            find(dialog.footer, "calendarFileCancel").clicked()
            tryCompare(dialog, "opened", false)
            compare(mock.added, null)
            compare(mock.closed, 1)
        }

        function test_a_file_that_does_not_open_says_why() {
            dialog.show(JSON.stringify({ problem: "calendar-file-unreadable", events: [], calendars: [], categories: [] }))
            tryCompare(dialog, "opened", true)
            compare(find(dialog.contentItem, "calendarFileProblem").text, "calendar-file-unreadable")
            compare(find(dialog.footer, "calendarFileAdd").visible, false)
            compare(find(dialog.footer, "calendarFileCancel").text, "ui-close")
            dialog.close()
            tryCompare(dialog, "opened", false)
            compare(mock.closed, 1)
        }
    }
}
