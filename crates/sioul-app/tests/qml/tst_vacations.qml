// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

// Settings ▸ Hours ▸ Vacations and holidays (SettingRow.qml's "timeoff" kind,
// VacationsCalendars.qml; 18 November 2026) on a stand-in for Sioul, at a
// computer's width and at a phone's (412): your vacations, each with its
// kind; a new one saved with its kind (a vacation said by none); each
// calendar of holidays with its switch, which says whether this device has
// read it yet; "Subscribe to a calendar of holidays…" opens the subscribe
// form here, over Settings (never the Agenda's ⚙). Fiction only. Run by
// tools/qml-test.sh.

import QtQuick
import QtQuick.Controls.Basic
import QtTest
import "../../qml"

Item {
    id: root

    width: 1344
    height: 1400

    Theme {
        id: testTheme
    }

    QtObject {
        id: mock

        signal agendasChanged

        property var calls: []
        property var calendars: [
            { id: "fr-public-metropole", name: "Jours fériés Métropole", holidays: "public", days_off: true, after_class: false, known: true, line: "Read Wed 19 Nov 08:00.", counts_line: "Jours fériés Métropole · public holidays: its days count as my days off" },
            { id: "fr-school-zone-a", name: "Zone A", holidays: "school", days_off: false, after_class: true, known: false, line: "This device has not read this calendar yet: until it does, it does not know these days.", counts_line: "Zone A · school holidays: its days count as my days off" }
        ]

        function text(key) { return key }
        function textWith(key, name, value) { return key + ":" + value }
        function textArgs(key, args) { return key }
        function holidaysCalendars() { return JSON.stringify({ calendars: mock.calendars }) }
        function setSubscriptionField(id, field, value) { mock.calls = mock.calls.concat([["field", id, field, value]]); return "" }
        function reminderOpened(kind, uri, key) { mock.calls = mock.calls.concat([["opened", kind]]) }
        function subscribeOfficials() { return JSON.stringify({ home: "", countries: [], officials: [] }) }
    }

    readonly property var setting: ({ key: "time_off", kind: "timeoff", label: "set-time-off", help: "set-time-off-help", group: "set-time-off-group", section: "hours", choices: [], min: 0, max: 0, step: 1, unit: "", value: [
        { id: "a1", from: "2030-08-01", until: "2030-08-15", label: "Summer", kind: "" },
        { id: "a2", from: "2030-03-02", until: "2030-03-06", label: "", kind: "sick" }
    ] })

    property var saved: null

    Component {
        id: rowComponent

        SettingRow {
            width: root.width
            sioul: mock
            theme: testTheme
            setting: root.setting
            onSave: (key, value, shown) => root.saved = { key: key, value: value }
        }
    }

    function all(item, test, out) {
        out = out || []
        for (let i = 0; i < item.children.length; i++) {
            const child = item.children[i]
            if (test(child))
                out.push(child)
            root.all(child, test, out)
        }
        return out
    }

    function named(item, name) {
        return root.all(item, c => c.objectName === name && c.visible)
    }

    TestCase {
        name: "Vacations"
        when: windowShown

        function init() {
            mock.calls = []
            root.saved = null
        }

        function test_vacations_and_holidays_data() {
            return [{ tag: "computer", width: 1344 }, { tag: "phone", width: 412 }]
        }

        function test_vacations_and_holidays(data) {
            root.width = data.width
            const row = createTemporaryObject(rowComponent, root)
            waitForRendering(row)
            // Each vacation with its kind.
            const lines = root.named(row, "vacationLine")
            compare(lines.length, 2)
            verify(lines[0].text.indexOf("vacations-kind-vacation") >= 0 && lines[0].text.indexOf("Summer") >= 0, lines[0].text)
            verify(lines[1].text.indexOf("vacations-kind-sick") >= 0, lines[1].text)
            // A new one with its kind.
            const fields = root.all(row, c => c.placeholderText === "2026-12-24" || c.placeholderText === "2027-01-02")
            fields[0].text = "2030-12-22"
            fields[1].text = "2030-12-31"
            root.named(row, "vacationLabel")[0].text = "Winter"
            root.named(row, "vacationKind")[0].currentIndex = 2
            mouseClick(root.named(row, "vacationAdd")[0])
            compare(root.saved.key, "time_off")
            compare(root.saved.value.length, 3)
            compare(root.saved.value[2].kind, "other")
            compare(root.saved.value[2].label, "Winter")
            // The calendars of holidays: a switch each, and what this device knows.
            const switches = root.named(row, "holidaysCounts")
            compare(switches.length, 2)
            compare(switches[0].text, "Jours fériés Métropole · public holidays: its days count as my days off", "one sentence, as Sioul says it")
            verify(switches[0].checked && !switches[1].checked, "public holidays count, school holidays not")
            const said = root.named(row, "holidaysLine")
            compare(said[1].color, testTheme.warm, "not read here yet: said")
            mouseClick(switches[1])
            compare(mock.calls[0], ["field", "fr-school-zone-a", "days_off", "true"])
            const block = root.named(row, "vacationsCalendars")[0]
            mouseClick(root.named(row, "holidaysSubscribe")[0])
            tryVerify(() => block.subscribing, 2000, "the subscribe form, over Settings")
            compare(mock.calls.filter(c => c[0] === "opened").length, 0, "never the Agenda's ⚙")
            keyClick(Qt.Key_Escape)
            tryVerify(() => !block.subscribing, 2000, "put away")
            const wide = root.all(row, c => c.visible && c.width > 0 && c.mapToItem(row, 0, 0).x + c.width > row.width + 1)
            compare(wide.length, 0, "nothing wider than the row")
        }

        // No calendar of holidays yet: one sentence, and the door to subscribe.
        function test_no_calendar_of_holidays_yet() {
            const before = mock.calendars
            mock.calendars = []
            const row = createTemporaryObject(rowComponent, root)
            verify(root.named(row, "vacationsCalendarsNone").length === 1)
            verify(root.named(row, "holidaysSubscribe").length === 1)
            mock.calendars = before
        }
    }
}
