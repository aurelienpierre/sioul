// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

// Do-not-disturb's switch in the status line (DndApplet.qml), and its
// settings (DndSetup.qml), each part where the window shows it: "device"
// (What reaches you ▸ Do not disturb), "list" (What reaches you ▸ Exceptions,
// the Always through list) and "phone" (This phone). On a stand-in for Sioul:
// no Sioul started, no profile read. Run by tools/qml-test.sh. The numbers
// are in ARCEP's ranges for fiction.

import QtQuick
import QtQuick.Controls.Basic
import QtTest
import "../../qml"

Item {
    id: root

    width: 1000
    height: 900

    Theme {
        id: testTheme
    }

    QtObject {
        id: mock

        property var toggles: []
        property var changes: []
        property bool android: false
        property var people: [
            { id: "a", name: "Alice", phones: ["+33 1 99 00 12 34"], emails: ["alice@example.org"], notes: [] },
            { id: "b", name: "", phones: [], emails: ["bob@example.org"], notes: ["dnd-setup-no-number"] },
        ]
        function text(key) { return key }
        function textWith(key, name, value) { return key + "(" + value + ")" }
        function textArgs(key, args) { return key + JSON.stringify(JSON.parse(args)) }
        function dndToggle(on, minutes) { mock.toggles = mock.toggles.concat([[on, minutes]]) }
        function setup() {
            const s = { here: { on: true, line: "Sioul can silence this desktop's notifications, with Plasma's do-not-disturb.", lines: ["dnd-setup-plasma-mail"], consent: "", offers: [{ key: "plasma", label: "Plasma's notification settings" }] }, gnome: false, people: mock.people, problem: "", said: "", android: mock.android }
            if (mock.android) {
                s.stars = { permission: true, summary: "Two people on your list are not starred on this phone.", missing: 2, people: [{ id: "a", name: "Alice", state: "not-starred", contact: "content://com.android.contacts/contacts/lookup/x/1" }, { id: "b", name: "bob@example.org", state: "no-number", contact: "" }] }
                s.steps = { on: true, running: true, sharing: true, battery: false, line: "dnd-steps-on", battery_line: "dnd-steps-battery" }
            }
            return JSON.stringify(s)
        }
        function dndSetup() { return mock.setup() }
        function dndChange(verb, json) {
            mock.changes = mock.changes.concat([[verb, JSON.parse(json)]])
            if (verb === "search")
                return JSON.stringify({ found: [{ uid: "u1", name: "Carol", detail: "+33 2 61 91 00 03" }], none: "" })
            return mock.setup()
        }
    }

    Component {
        id: appletComponent

        DndApplet {
            sioul: mock
            theme: testTheme
            moment: ({ dnd: { on: false, button: true, line: "", why_line: "", details: [], manual: false, end_time: "17:00" } })
        }
    }
    Component {
        id: setupComponent

        DndSetup {
            width: 412
            sioul: mock
            theme: testTheme
        }
    }

    TestCase {
        name: "DndApplet"
        when: windowShown

        function test_off_turns_on_until_turned_off() {
            mock.toggles = []
            const applet = createTemporaryObject(appletComponent, root)
            verify(applet.visible)
            compare(applet.icon.name, "notification-inactive")
            compare(applet.display, AbstractButton.IconOnly, "off: its icon alone")
            mouseClick(applet)
            compare(mock.toggles.length, 1)
            compare(mock.toggles[0][0], true)
            compare(mock.toggles[0][1], 0)
        }

        function test_on_says_where_and_turns_off() {
            mock.toggles = []
            const applet = createTemporaryObject(appletComponent, root, { moment: { dnd: { on: true, button: true, line: "Do not disturb, on every device, until 15:00.", why_line: "Turned on from your phone.", details: ["Here: silenced.", "On your phone: silenced."], manual: true, end_time: "17:00" } } })
            compare(applet.icon.name, "notification-disabled")
            compare(applet.display, AbstractButton.TextBesideIcon)
            compare(applet.text, "Do not disturb, on every device, until 15:00.")
            verify(applet.tip.indexOf("Turned on from your phone.") >= 0, applet.tip)
            mouseClick(applet)
            compare(mock.toggles[0][0], false, "a click on: off, on every device")
        }

        function test_hidden_switch_shows_while_on() {
            const hidden = createTemporaryObject(appletComponent, root, { moment: { dnd: { on: false, button: false, line: "", why_line: "", details: [], end_time: "" } } })
            verify(!hidden.visible)
            const held = createTemporaryObject(appletComponent, root, { moment: { dnd: { on: true, button: false, line: "Do not disturb, here only.", why_line: "While you focus on a task.", details: [], end_time: "" } } })
            verify(held.visible, "held on by focus: shown")
        }

        function test_its_menu_offers_how_long() {
            mock.toggles = []
            const applet = createTemporaryObject(appletComponent, root)
            applet.openMenu()
            let menu = null
            for (let i = 0; i < applet.children.length; ++i)
                if (applet.children[i].item)
                    menu = applet.children[i].item
            verify(menu !== null, "the menu made")
            tryVerify(() => menu.opened)
            const texts = []
            for (let i = 0; i < menu.count; ++i) {
                const item = menu.itemAt(i)
                if (item && item.visible && item.text)
                    texts.push(item.text)
            }
            verify(texts.indexOf("dnd-for-60") >= 0, texts)
            verify(texts.indexOf("dnd-until-time(17:00)") >= 0, texts)
            verify(texts.indexOf("dnd-until-off") >= 0, texts)
            verify(texts.indexOf("dnd-turn-off") < 0, "off: no 'turn it off'")
            for (let i = 0; i < menu.count; ++i) {
                const item = menu.itemAt(i)
                if (item && item.text === "dnd-for-60")
                    item.triggered()
            }
            compare(mock.toggles[mock.toggles.length - 1][0], true)
            compare(mock.toggles[mock.toggles.length - 1][1], 60)
            menu.close()
        }
    }

    TestCase {
        name: "DndSetup"
        when: windowShown

        function texts(item, out) {
            if (item.text !== undefined && typeof item.text === "string" && item.visible && item.text !== "")
                out.push(item.text)
            for (let i = 0; i < item.children.length; ++i)
                texts(item.children[i], out)
            return out
        }

        function overflow(item, right, out) {
            if (!item.visible)
                return out
            const at = item.mapToItem(root, 0, 0)
            if (item.width > 0 && at.x + item.width > right + 1)
                out.push((item.text || item.objectName || item.toString()) + " " + Math.round(at.x + item.width))
            for (let i = 0; i < item.children.length; ++i)
                overflow(item.children[i], right, out)
            return out
        }

        function cleanup() {
            mock.android = false
        }

        // What reaches you ▸ Do not disturb: this desktop's system alone.
        function test_a_computer_says_what_plasma_hides() {
            const setup = createTemporaryObject(setupComponent, root, { part: "device" })
            tryVerify(() => setup.shown.people.length === 2)
            const seen = texts(setup, [])
            verify(seen.indexOf("dnd-setup-here") >= 0, seen)
            verify(seen.indexOf("dnd-setup-plasma-mail") >= 0, seen)
            verify(seen.indexOf("Plasma's notification settings") >= 0, seen)
            verify(seen.indexOf("Alice") < 0, "the list is under Exceptions")
            verify(seen.indexOf("dnd-stars-title") < 0, "no stars on a computer")
            verify(seen.indexOf("dnd-steps-title") < 0, "no service on a computer")
        }

        // What reaches you ▸ Exceptions: the list alone, the same on every device.
        function test_the_list_names_each_person() {
            const setup = createTemporaryObject(setupComponent, root, { part: "list" })
            tryVerify(() => setup.shown.people.length === 2)
            const seen = texts(setup, [])
            verify(seen.indexOf("dnd-setup-list-help") >= 0, seen)
            verify(seen.indexOf("Alice") >= 0, seen)
            verify(seen.indexOf("+33 1 99 00 12 34 · alice@example.org") >= 0, seen)
            verify(seen.indexOf("bob@example.org") >= 0, "a person without a name shows by their address")
            verify(seen.indexOf("dnd-setup-no-number") >= 0, seen)
            verify(seen.indexOf("dnd-setup-plasma-mail") < 0, "this desktop's system is under Do not disturb")
            verify(seen.indexOf("dnd-stars-title") < 0, seen)
        }

        function test_the_form_is_filled_anew_each_time() {
            mock.changes = []
            const setup = createTemporaryObject(setupComponent, root, { part: "list" })
            setup.edit({ id: "a", name: "Alice", phones: "+33 1 99 00 12 34", emails: "alice@example.org" })
            const seen = texts(setup, [])
            verify(seen.indexOf("+33 1 99 00 12 34") >= 0, seen)
            setup.edit({ id: "", name: "", phones: "", emails: "" })
            verify(texts(setup, []).indexOf("+33 1 99 00 12 34") < 0, "a new person's form is empty")
            setup.act("add", { id: "", name: "Dan", phones: "+33 3 53 01 00 04", emails: "" })
            compare(mock.changes[mock.changes.length - 1][0], "add")
            compare(mock.changes[mock.changes.length - 1][1].name, "Dan")
        }

        // This phone: its system, who on it is starred, Sioul kept in step; within 412 px.
        function test_a_phone_says_who_is_not_starred_and_fits_its_width() {
            mock.android = true
            const setup = createTemporaryObject(setupComponent, root, { part: "phone" })
            tryVerify(() => setup.shown.stars !== undefined)
            const seen = texts(setup, [])
            verify(seen.indexOf("dnd-setup-here") >= 0, seen)
            verify(seen.indexOf("Two people on your list are not starred on this phone.") >= 0, seen)
            verify(seen.indexOf("dnd-stars-open") >= 0, "Alice's contact to open")
            verify(seen.indexOf("dnd-steps-allow") >= 0, "the battery's question offered")
            verify(seen.indexOf("Alice") < 0, "the list itself is under Exceptions")
            wait(50)
            const wide = overflow(setup, 412, [])
            compare(wide.length, 0, "nothing past 412 px: " + wide.join(", "))
        }

        // Exceptions on a phone: the same list, no stars (they are under This phone).
        function test_the_list_on_a_phone_shows_no_stars() {
            mock.android = true
            const setup = createTemporaryObject(setupComponent, root, { part: "list" })
            tryVerify(() => setup.shown.stars !== undefined)
            const seen = texts(setup, [])
            verify(seen.indexOf("Alice") >= 0, seen)
            verify(seen.indexOf("dnd-stars-title") < 0, seen)
            verify(seen.indexOf("dnd-steps-title") < 0, seen)
            wait(50)
            compare(overflow(setup, 412, []).length, 0)
        }
    }
}
