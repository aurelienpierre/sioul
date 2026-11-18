// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

// The usual meals, naps and night in Settings ▸ Hours (ParametersPage.qml,
// SettingRow.qml's "needs" kind, NeedsSection.qml), on a stand-in for Sioul
// whose rows are the hours' as the core gives them (`settings::for_view`):
// the door by their key ("needs", where every door to them leads: the
// Porch's "Set my night", a meal's card, What reaches you, Health's ⚙)
// shows the tab, the block under its own heading after the working hours
// and the hours for your admin, before the days off; Health's ⚙ keeps a
// line and a button that open them there. Made-up rows. Run by
// tools/qml-test.sh.

import QtQuick
import QtQuick.Controls.Basic
import QtTest
import "../../qml"

Item {
    id: root

    width: 900
    height: 900

    Theme {
        id: testTheme
    }

    QtObject {
        id: mock

        readonly property bool telephony: false
        readonly property string deviceKind: "computer"
        // What Health's ⚙ asked the window to open.
        property var opened: []

        function text(key) {
            return key
        }
        function textWith(key, name, value) {
            return key + " " + value
        }
        function viewFlag(name) {
            return false
        }
        // The hours' rows, in the core's order and groups; the other tabs' left out.
        function settings(view) {
            const row = (key, kind, group, label) => ({ key: key, kind: kind, label: label, help: label + "-help", value: false, choices: [], group: group, section: "hours", min: 0, max: 0, step: 1, unit: "" })
            return JSON.stringify(view !== "parameters" ? [] : [
                row("language", "bool", "set-look-group", "set-language"),
                row("window", "bool", "set-hours-group", "set-windows"),
                row("window.admin", "bool", "set-hours-group", "set-windows-admin"),
                row("needs", "needs", "needs-title", "needs-title"),
                row("time_off", "bool", "set-time-off-group", "set-time-off")
            ].map(r => r.key === "language" ? Object.assign(r, { section: "look" }) : r))
        }
        function setSetting(key, value) {
            return ""
        }
        function changeSetting(key, shown, value) {
            return ""
        }
        // Made up: a breakfast and the night.
        function needs() {
            return JSON.stringify({
                needs: { meals_on: true, meals: [{ name: "", at: "08:00", minutes: 20, before: 10, after: 15, days: [true, true, true, true, true, true, true], on: true, notices: true }], naps_on: false, naps: [], sleep_on: true, sleep: { bed: "23:00", wake: "07:00", wind_down: 60, notices: true, alarm: [false, false, false, false, false, false, false] }, heads_up: 15, later: 15 },
                usual: { meals: ["Breakfast"], nap: "", sleep: "" },
                gaps: [],
                wake: { phone: false, names: [], short: [], next: "", exact: true, screen: true, notifications: true }
            })
        }
        function saveNeeds(edit, shown) {
            return ""
        }
        function healthSettings() {
            return JSON.stringify({ movement: { enabled: true, minutes: 45 }, chats: { enabled: false, minutes: 60, locked_minutes: 30 }, errands_list: "", lists: [] })
        }
        function reminderOpened(kind, uri, key) {
            mock.opened = mock.opened.concat([kind])
        }
    }

    ParametersPage {
        id: page

        anchors.fill: parent
        sioul: mock
        theme: testTheme
    }

    Component {
        id: healthSettingsComponent

        HealthSettings {
            sioul: mock
            theme: testTheme
        }
    }

    TestCase {
        name: "SettingHours"
        when: windowShown

        function walk(item, test, found) {
            if (!item)
                return found
            if (test(item))
                found.push(item)
            const children = item.children || []
            for (let i = 0; i < children.length; ++i)
                walk(children[i], test, found)
            if (item.contentItem && item.contentItem !== item && children.indexOf(item.contentItem) < 0)
                walk(item.contentItem, test, found)
            return found
        }
        function yOf(item) {
            return item.mapToItem(root, 0, 0).y
        }
        function label(text) {
            return walk(page, i => i.visible && i.text === text, [])[0] || null
        }

        function init() {
            page.reload()
            page.section = "look"
        }

        // Their door: the tab, the block under its heading, between the two weeks and the days off.
        function test_the_door_shows_them_under_their_heading_in_hours() {
            page.showSetting("needs")
            compare(page.section, "hours")
            const sections = walk(page, i => i instanceof NeedsSection, [])
            compare(sections.length, 1, "one block")
            tryVerify(() => sections[0].visible && sections[0].height > 0)
            // Laid out, and scrolled to it (ParametersPage.qml's `toSetting`, 150 ms).
            wait(400)
            compare(sections[0].about, "needs-title-help", "its sentence said first, the row's own")
            const heading = label("needs-title")
            verify(heading !== null, "under their own heading")
            const admin = label("set-windows-admin")
            const off = label("set-time-off-group")
            verify(admin !== null && off !== null)
            verify(yOf(admin) < yOf(heading), "after the hours for your admin")
            verify(yOf(heading) < yOf(sections[0]), "the heading above the block")
            verify(yOf(heading) >= 0 && yOf(heading) < root.height / 2, "their heading in view")
            verify(yOf(sections[0]) + sections[0].height <= yOf(off) + 0.5, "before the days off")
            compare(walk(page, i => i.visible && i.text === "set-hours-group", []).length, 1, "the hours' heading once")
            compare(walk(page, i => i.visible && i.text === "needs-title-help", []).length, 1, "their sentence said once, by the block")
        }

        // Health's ⚙: a line, and a button that closes it and opens them where they are.
        function test_health_settings_lead_there() {
            mock.opened = []
            const panel = createTemporaryObject(healthSettingsComponent, root)
            panel.open()
            tryCompare(panel, "opened", true)
            compare(walk(panel.contentItem, i => i instanceof NeedsSection, []).length, 0, "not set there")
            const button = walk(panel.contentItem, i => i.objectName === "healthNeedsLink", [])[0]
            verify(button && button.visible)
            button.clicked()
            compare(mock.opened, ["needs"])
            tryCompare(panel, "visible", false)
        }
    }
}
