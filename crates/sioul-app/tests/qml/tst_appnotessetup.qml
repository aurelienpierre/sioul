// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

// Other apps' notifications on a phone (AppNotesSetup.qml), this phone's
// part, on a stand-in for Sioul: under "Hold other apps' notifications until
// their time", the switch "Hold people's messages too" (held apps phase 1,
// A2), shown while holding is on and set from there; holding off, it goes.
// Under it, the line in the phone's shade (B1), on unless turned off, set
// from there, gone with holding.
// What the switch decides is tested in the app (appnotes.rs). Run by
// tools/qml-test.sh.

import QtQuick
import QtQuick.Controls.Basic
import QtTest
import "../../qml"

Item {
    id: root

    width: 412
    height: 1600

    Theme {
        id: testTheme
    }

    QtObject {
        id: mock

        // What the tab asked, in order: "verb key=value".
        property var asked: []
        property bool hold: true
        property bool people: false
        property bool shade: true

        function text(key) {
            return key
        }
        function textWith(key, name, value) {
            return key
        }
        function setup() {
            return JSON.stringify({
                android: true, access: true, restricted: false, contacts: true,
                hold: { key: "hold", kind: "bool", label: "Hold other apps' notifications until their time", help: "", value: mock.hold, choices: [] },
                people: { key: "people", kind: "bool", label: "Hold people's messages too, by who writes and when", help: "Off, they come at once.", value: mock.people, choices: [] },
                shade: { key: "shade", kind: "bool", label: "Say in the phone's notifications what Sioul holds", help: "One line without a sound.", value: mock.shade, choices: [] },
                times: "", heard: "", rang: [],
                link: { available: false, linked: false, on: false, giving_back: false, given_back: false },
                silenced: [], apps: [], conversations: [], sites: [], grid: { columns: [], marks: [], rows: [] }, more: false
            })
        }
        function appNotesSetup() {
            return mock.setup()
        }
        function appNotesChange(verb, json) {
            const args = JSON.parse(json)
            mock.asked = mock.asked.concat([verb + " " + args.key + "=" + args.value])
            if (verb === "set" && args.key === "hold")
                mock.hold = args.value
            if (verb === "set" && args.key === "people")
                mock.people = args.value
            if (verb === "set" && args.key === "shade")
                mock.shade = args.value
            return mock.setup()
        }
        function reminderOpened(door, a, b) {
        }
    }

    AppNotesSetup {
        id: tab

        width: root.width - 32
        x: 16
        part: "phone"
        sioul: mock
        theme: testTheme
    }

    TestCase {
        name: "AppNotesSetup"
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
        function seen(item) {
            for (let at = item; at && at !== tab; at = at.parent) {
                if (!at.visible)
                    return false
            }
            return tab.visible
        }
        // A switch of the tab, by the label its row reads to a screen reader.
        function switchOf(label) {
            const found = walk(tab, i => i.checked !== undefined && i.toggled !== undefined && i.Accessible.name === label, [])
            return found.length > 0 ? found[0] : null
        }
        function peopleRow() {
            return walk(tab, i => i.objectName === "appNotesPeople", [])[0]
        }
        function shadeRow() {
            return walk(tab, i => i.objectName === "appNotesShade", [])[0]
        }

        function init() {
            mock.asked = []
            mock.hold = true
            mock.people = false
            mock.shade = true
            tab.reload()
        }

        // Under holding, while it is on: people's messages held too, off as the phone worked it out.
        function test_people_s_messages_switch_sits_under_holding() {
            const row = peopleRow()
            verify(row, "the row is there")
            verify(seen(row), "shown while holding is on")
            const people = switchOf("Hold people's messages too, by who writes and when")
            verify(people && seen(people))
            verify(!people.checked, "off, as worked out on a phone where nothing was chosen")
            const hold = switchOf("Hold other apps' notifications until their time")
            verify(hold && hold.checked)
            verify(hold.mapToItem(tab, 0, 0).y < people.mapToItem(tab, 0, 0).y, "under the master switch")
        }

        // Turned on from the tab: said to the backend, and shown as it came back.
        function test_people_s_messages_set_from_the_tab() {
            const people = switchOf("Hold people's messages too, by who writes and when")
            people.toggle()
            people.toggled()
            compare(mock.asked, ["set people=true"])
            verify(switchOf("Hold people's messages too, by who writes and when").checked, "on, as the backend says")
        }

        // Holding off: nothing is held, so the switch for people goes.
        function test_holding_off_takes_people_s_switch_away() {
            const hold = switchOf("Hold other apps' notifications until their time")
            hold.toggle()
            hold.toggled()
            compare(mock.asked, ["set hold=false"])
            verify(!seen(peopleRow()), "gone while nothing is held")
            verify(!seen(shadeRow()), "and the line's switch with it")
        }

        // The line in the shade (B1): under people's, on as Sioul ships it, turned off from the tab.
        function test_the_line_in_the_shade_is_on_unless_turned_off() {
            const row = shadeRow()
            verify(row && seen(row), "shown while holding is on")
            const shade = switchOf("Say in the phone's notifications what Sioul holds")
            verify(shade && shade.checked, "on unless turned off")
            const people = switchOf("Hold people's messages too, by who writes and when")
            verify(people.mapToItem(tab, 0, 0).y < shade.mapToItem(tab, 0, 0).y, "under people's")
            shade.toggle()
            shade.toggled()
            compare(mock.asked, ["set shade=false"])
            verify(!switchOf("Say in the phone's notifications what Sioul holds").checked, "off, as the backend says")
        }
    }
}
