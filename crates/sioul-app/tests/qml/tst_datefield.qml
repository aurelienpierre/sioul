// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

// The date picker (DateField.qml): its month arrows have a name a screen
// reader says and a tip shows, "Previous month: September 2026", in
// Sioul's words; the month alone where no words are given. It behaves as the
// hour's panel does (ClockField.qml): the button or Alt+Down opens it, a click
// outside keeps, Esc gives back, Tab goes past the button; a day picked moves on
// to the hour beside it. Run by tools/qml-test.sh.

import QtQuick
import QtQuick.Controls.Basic
import QtTest
import "../../qml"

Item {
    id: root

    width: 600
    height: 500

    Theme {
        id: testTheme
    }

    QtObject {
        id: mock

        readonly property var words: ({
            "date-month-previous": "Previous month: {month}",
            "date-month-next": "Next month: {month}"
        })

        function text(key) {
            return mock.words[key] || key
        }
        function textWith(key, name, value) {
            return mock.text(key).replace("{" + name + "}", value)
        }
    }

    DateField {
        id: named

        theme: testTheme
        sioul: mock
        locale: Qt.locale("en_GB")
        shownYear: 2026
        shownMonth: 9
    }

    DateField {
        id: bare

        theme: testTheme
        locale: Qt.locale("en_GB")
        shownYear: 2026
        shownMonth: 0
    }

    Component {
        id: dateComponent

        DateField {
            x: 20
            y: 20
            theme: testTheme
            sioul: mock
            locale: Qt.locale("en_GB")
            date: "2026-11-05"
        }
    }

    Component {
        id: hourComponent

        ClockField {
            x: 240
            y: 20
            theme: testTheme
        }
    }

    Component {
        id: nextComponent

        TextField {
            x: 400
            y: 20
            width: 100
        }
    }

    TestCase {
        name: "DateField"
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
            return null
        }

        // A day's cell of the month, by its number in the month shown.
        function dayCell(month, day) {
            const kids = month.contentItem.children
            for (let i = 0; i < kids.length; ++i)
                if (kids[i].model && kids[i].model.day === day && kids[i].model.month === month.month)
                    return kids[i]
            return null
        }

        function pickDay(date, day) {
            const month = find(date.monthPopup.contentItem, "dateMonth")
            tryVerify(() => dayCell(month, day) !== null)
            mouseClick(dayCell(month, day))
        }

        function test_the_arrows_are_named() {
            compare(named.arrowName(-1), "Previous month: September 2026")
            compare(named.arrowName(1), "Next month: November 2026")
        }

        function test_across_a_year() {
            compare(bare.arrowName(-1), "December 2025", "the month alone, without words")
            compare(bare.arrowName(1), "February 2026")
        }

        // The button opens the month on the date's own, a day applies and closes it.
        function test_a_day_applies_and_closes() {
            const date = createTemporaryObject(dateComponent, root)
            let edited = 0
            date.edited.connect(() => edited++)
            mouseClick(find(date, "dateButton"))
            verify(date.monthOpen)
            compare(date.shownYear, 2026)
            compare(date.shownMonth, 10)
            pickDay(date, 10)
            compare(date.date, "2026-11-10")
            compare(edited, 1)
            verify(!date.monthOpen)
        }

        function test_the_button_closes_what_it_opened() {
            const date = createTemporaryObject(dateComponent, root)
            mouseClick(find(date, "dateButton"))
            mouseClick(find(date, "dateButton"))
            verify(!date.monthOpen, "closed, not reopened")
        }

        function test_a_click_outside_keeps() {
            const date = createTemporaryObject(dateComponent, root)
            mouseClick(find(date, "dateButton"))
            mouseClick(root, root.width - 10, root.height - 10)
            verify(!date.monthOpen)
            compare(date.date, "2026-11-05")
        }

        // Alt+Down opens it with the focus left in the field; Esc gives back a date
        // typed beside it and closes the month alone.
        function test_alt_down_and_esc() {
            const date = createTemporaryObject(dateComponent, root)
            const field = find(date, "dateText")
            field.forceActiveFocus(Qt.TabFocusReason)
            keyClick(Qt.Key_Down, Qt.AltModifier)
            verify(date.monthOpen, "Alt+Down opens it")
            verify(field.activeFocus, "the focus stays in the field")
            for (const key of "20261225")
                keyClick(key)
            compare(date.date, "2026-12-25", "typed beside it")
            keyClick(Qt.Key_Escape)
            verify(!date.monthOpen)
            compare(date.date, "2026-11-05", "the earlier one")
        }

        // The arrows turn the months with the focus left in the field: Esc still works there,
        // and Tab away closes the month; hidden too.
        function test_the_arrows_tab_away_and_hidden() {
            const date = createTemporaryObject(dateComponent, root)
            const field = find(date, "dateText")
            const after = createTemporaryObject(nextComponent, root)
            field.KeyNavigation.tab = after
            mouseClick(find(date, "dateButton"))
            verify(field.activeFocus)
            compare(date.shownMonth, 10)
            const arrow = find(date.monthPopup.contentItem, "dateNextMonth")
            mouseClick(arrow)
            compare(date.shownMonth, 11)
            verify(date.monthOpen, "still open")
            verify(field.activeFocus, "the focus stays in the field")
            keyClick(Qt.Key_Tab)
            verify(after.activeFocus)
            verify(!date.monthOpen, "Tab away closes it")
            mouseClick(find(date, "dateButton"))
            verify(date.monthOpen)
            date.visible = false
            verify(!date.monthOpen, "hidden, it closes")
        }

        // Date, then hour, by Tab and digits alone: Tab goes past the button, and into a
        // field the whole text is selected, so that the digits are heard.
        function test_keyboard_alone_date_then_hour() {
            const before = createTemporaryObject(nextComponent, root, { y: 80 })
            const date = createTemporaryObject(dateComponent, root)
            const hour = createTemporaryObject(hourComponent, root, { text: "08:15" })
            const after = createTemporaryObject(nextComponent, root)
            before.KeyNavigation.tab = find(date, "dateText")
            find(date, "dateText").KeyNavigation.tab = hour
            before.forceActiveFocus()
            keyClick(Qt.Key_Tab)
            verify(find(date, "dateText").activeFocus)
            for (const key of "20261225")
                keyClick(key)
            compare(date.date, "2026-12-25")
            compare(find(date, "dateButton").activeFocusOnTab, false, "no Tab stop on the button")
            keyClick(Qt.Key_Tab)
            verify(hour.activeFocus)
            for (const key of "930")
                keyClick(key)
            hour.KeyNavigation.tab = after
            keyClick(Qt.Key_Tab)
            verify(after.activeFocus)
            compare(hour.text, "09:30")
        }

        // Pointer alone, a whole date and time: a day picked gives the hour the focus, and
        // the panel opens there, typing still possible. Calendar, day, hour, minute: four clicks.
        function test_a_day_moves_on_to_the_hour() {
            const hour = createTemporaryObject(hourComponent, root, { text: "08:15" })
            const date = createTemporaryObject(dateComponent, root, { hourField: hour })
            mouseClick(find(date, "dateButton"))
            pickDay(date, 10)
            verify(!date.monthOpen, "the month gone")
            verify(hour.activeFocus, "the focus on the hour")
            verify(hour.panelOpen, "its panel open")
            mouseClick(find(hour.panel.contentItem, "clockHour14"))
            mouseClick(find(hour.panel.contentItem, "clockMinute30"))
            compare(date.date, "2026-11-10")
            compare(hour.text, "14:30")
            keyClick(Qt.Key_Return)
            verify(!hour.panelOpen, "Enter closes it")
        }

        // A day alone is a whole answer beside an empty hour (a task's "No hour"): no panel,
        // no hour made up. A hidden hour (a whole day) is left alone.
        function test_an_empty_or_hidden_hour_stays_closed() {
            const empty = createTemporaryObject(hourComponent, root)
            const date = createTemporaryObject(dateComponent, root, { hourField: empty })
            mouseClick(find(date, "dateButton"))
            pickDay(date, 10)
            verify(empty.activeFocus, "the focus comes")
            verify(!empty.panelOpen, "the panel does not")
            compare(empty.clock, "")

            const hidden = createTemporaryObject(hourComponent, root, { text: "08:15", visible: false })
            const other = createTemporaryObject(dateComponent, root, { hourField: hidden })
            mouseClick(find(other, "dateButton"))
            pickDay(other, 10)
            verify(!hidden.activeFocus)
            verify(!hidden.panelOpen)
        }
    }
}
