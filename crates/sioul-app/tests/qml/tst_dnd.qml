// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

// The status line's button of what reaches you and its card (ReachButton.qml,
// ReachCard.qml; do-not-disturb's switch until part 5), and do-not-disturb's
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
        property var chosen: []
        property int letThrough: 0
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
        function reachChoose(state, length) { mock.chosen = mock.chosen.concat([[state, length]]) }
        function letPhoneThrough() { mock.letThrough += 1 }
        property bool realtime: false
        property var through: []
        property var calls: ({ shown: false, through: false, line: "" })
        function setRealtimeMode(on) { mock.realtime = on }
        function callsThrough(on, minutes) { mock.through = mock.through.concat([[on, minutes]]) }
        function reachCard() { return JSON.stringify({ now: ["Work and admin until 17:00.", "The people you know reach you at once."], held: "", realtime: mock.realtime, screening: mock.calls.shown ? "Numbers nobody knows go to your voicemail, at all hours." : "", calls: mock.calls }) }
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
        id: buttonComponent

        ReachButton {
            sioul: mock
            theme: testTheme
            moment: ({ dnd: { on: false, line: "", why_line: "", details: [], end_time: "17:00", state: "usual", reachable: null, paused: false, reach_hours: 61200, reach_hours_time: "17:00", reach_tonight: 81000, reach_tonight_time: "22:30", reach_capped: [false, false, true] } })
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
        name: "ReachButton"
        when: windowShown

        readonly property var usualDnd: ({ on: false, line: "", why_line: "", details: [], end_time: "17:00", state: "usual", reachable: null, paused: false, reach_hours: 61200, reach_hours_time: "17:00", reach_tonight: 81000, reach_tonight_time: "22:30", reach_capped: [false, false, true] })

        function cardOf(button) {
            button.openCard()
            const card = button.cardLoader.item
            tryVerify(() => card.opened)
            // Laid out: its parts placed after the frame that shows them.
            waitForRendering(card.contentItem)
            wait(50)
            return card
        }

        function find(item, name) {
            if (item.objectName === name)
                return item
            const kids = item.children || []
            for (let i = 0; i < kids.length; ++i) {
                const found = find(kids[i], name)
                if (found)
                    return found
            }
            if (item.contentItem && item.contentItem !== item) {
                const found = find(item.contentItem, name)
                if (found)
                    return found
            }
            return null
        }

        function menuTexts(menu) {
            const texts = []
            for (let i = 0; i < menu.count; ++i) {
                const item = menu.itemAt(i)
                if (item && item.visible && item.text)
                    texts.push(item.objectName)
            }
            return texts
        }

        // Its icon says the state; its name for screen readers, the state and its end; a click opens the card.
        function test_the_button_says_the_state() {
            const usual = createTemporaryObject(buttonComponent, root)
            verify(usual.visible)
            compare(usual.icon.name, "notification-inactive")
            compare(usual.display, AbstractButton.IconOnly, "as usual: its icon alone")
            compare(usual.Accessible.name, "reach-button-name(reach-usual)")
            mouseClick(usual)
            tryVerify(() => usual.cardLoader.item !== null && usual.cardLoader.item.opened, 2000, "a click opens the card")
            usual.cardLoader.item.close()
            const dnd = createTemporaryObject(buttonComponent, root, { moment: { dnd: Object.assign({}, usualDnd, { on: true, state: "dnd", line: "Do not disturb, on every device, until 15:00." }) } })
            compare(dnd.icon.name, "notification-disabled")
            compare(dnd.display, AbstractButton.TextBesideIcon)
            compare(dnd.text, "Do not disturb, on every device, until 15:00.")
            verify(dnd.switchedOn, "the accent")
            const reachable = createTemporaryObject(buttonComponent, root, { moment: { dnd: Object.assign({}, usualDnd, { state: "reachable", reachable: { line: "Reachable until 17:00.", until: "17:00" } }) } })
            compare(reachable.icon.name, "notification-active")
            compare(reachable.text, "Reachable until 17:00.")
            compare(reachable.Accessible.name, "reach-button-name(Reachable until 17:00.)")
        }

        // The three states side by side; Reachable and Do not disturb show their lengths, As usual applies at once.
        function test_the_card_presses_each_state() {
            mock.chosen = []
            mock.toggles = []
            const button = createTemporaryObject(buttonComponent, root)
            const card = cardOf(button)
            verify(find(card.contentItem, "state-usual").highlighted, "as usual now")
            mouseClick(find(card.contentItem, "state-reachable"))
            wait(50)
            const labels = card.lengths("reachable").map(l => l.label)
            verify(labels.indexOf("reach-len-until(17:00)") >= 0 && labels.indexOf("reach-len-tonight") >= 0 && labels.indexOf("reach-len-off") >= 0, labels)
            compare(card.lengths("reachable")[2].name, "reach-for-120-latest(22:30)", "two hours past the night's start: said so")
            tryVerify(() => find(card.contentItem, "length-reachable-60") !== null)
            mouseClick(find(card.contentItem, "length-reachable-60"))
            compare(mock.chosen[mock.chosen.length - 1][0], "reachable")
            compare(mock.chosen[mock.chosen.length - 1][1], "60")
            mouseClick(find(card.contentItem, "state-dnd"))
            wait(50)
            tryVerify(() => find(card.contentItem, "length-dnd-30") !== null)
            mouseClick(find(card.contentItem, "length-dnd-30"))
            compare(mock.toggles[mock.toggles.length - 1][0], true)
            compare(mock.toggles[mock.toggles.length - 1][1], 30)
            card.close()
            const reachable = createTemporaryObject(buttonComponent, root, { moment: { dnd: Object.assign({}, usualDnd, { state: "reachable", reachable: { line: "Reachable until 17:00.", until: "17:00" } }) } })
            mouseClick(find(cardOf(reachable).contentItem, "state-usual"))
            compare(mock.chosen[mock.chosen.length - 1][0], "usual", "Reachable ended")
            reachable.cardLoader.item.close()
            const on = createTemporaryObject(buttonComponent, root, { moment: { dnd: Object.assign({}, usualDnd, { on: true, state: "dnd", line: "Do not disturb, here only." }) } })
            mouseClick(find(cardOf(on).contentItem, "state-usual"))
            compare(mock.toggles[mock.toggles.length - 1][0], false, "do-not-disturb off")
            on.cardLoader.item.close()
            // At night, until tonight and until turned off both end at waking: one choice.
            const night = createTemporaryObject(buttonComponent, root, { moment: { dnd: Object.assign({}, usualDnd, { reach_hours: 0, reach_tonight_time: "07:00", reach_at_night: true }) } })
            const asleep = cardOf(night).lengths("reachable").map(l => l.label)
            verify(asleep.indexOf("reach-len-waking") >= 0 && asleep.indexOf("reach-len-off") < 0 && asleep.indexOf("reach-len-tonight") < 0, asleep)
            night.cardLoader.item.close()
        }

        // During a pause: no Reachable, nothing let through.
        function test_a_pause_offers_no_reachable() {
            const button = createTemporaryObject(buttonComponent, root, { moment: { dnd: Object.assign({}, usualDnd, { on: true, state: "dnd", paused: true, line: "Do not disturb, here only." }) } })
            const card = cardOf(button)
            verify(!find(card.contentItem, "state-reachable").enabled)
            verify(!find(card.contentItem, "let-through").visible)
            card.close()
            button.openMenu()
            const menu = button.menuLoader.item
            tryVerify(() => menu.opened)
            const shown = menuTexts(menu)
            verify(shown.indexOf("menu-reachable") < 0 && shown.indexOf("menu-dnd") >= 0 && shown.indexOf("menu-usual") >= 0, shown)
            menu.close()
        }

        // What the phone holds let through (a computer: without saying what), Mail every minute,
        // the call screening apart: Expecting a call, Screen calls again.
        function test_the_card_s_other_parts() {
            mock.letThrough = 0
            mock.through = []
            mock.realtime = false
            mock.calls = { shown: true, through: false, line: "" }
            const button = createTemporaryObject(buttonComponent, root)
            const card = cardOf(button)
            compare(card.view.screening, "Numbers nobody knows go to your voicemail, at all hours.")
            mouseClick(find(card.contentItem, "let-through"))
            compare(mock.letThrough, 1)
            const every = find(card.contentItem, "mail-every-minute")
            mouseClick(every)
            compare(mock.realtime, true, "Mail every minute on")
            const expecting = find(card.contentItem, "expecting")
            verify(expecting.visible)
            mouseClick(expecting)
            compare(mock.through[mock.through.length - 1][0], true)
            compare(mock.through[mock.through.length - 1][1], 60)
            card.close()
            mock.calls = { shown: true, through: true, line: "Expecting a call until 15:00: numbers nobody knows ring." }
            const again = find(cardOf(button).contentItem, "expecting")
            compare(again.text, "calls-note-again")
            mouseClick(again)
            compare(mock.through[mock.through.length - 1][0], false, "screened again")
            button.cardLoader.item.close()
            mock.calls = { shown: false, through: false, line: "" }
        }

        // Every control named for screen readers; nothing past the card's edge, however narrow.
        function test_every_control_is_named_and_fits() {
            mock.calls = { shown: true, through: false, line: "" }
            const button = createTemporaryObject(buttonComponent, root)
            const card = cardOf(button)
            mouseClick(find(card.contentItem, "state-reachable"))
            card.width = 300
            wait(50)
            const names = ["state-reachable", "state-usual", "state-dnd", "length-reachable-30", "length-reachable-hours", "length-reachable-tonight", "length-reachable-off", "let-through", "mail-every-minute", "expecting"]
            const column = card.contentItem.contentItem.children[0]
            for (const name of names) {
                const item = find(card.contentItem, name)
                verify(item !== null, name)
                verify(item.Accessible.name !== "", name + ": named")
                const at = item.mapToItem(column, 0, 0)
                verify(at.x >= -0.5 && at.x + item.width <= column.width + 0.5, name + ": " + at.x + " + " + item.width + " in " + column.width)
            }
            card.close()
            mock.calls = { shown: false, through: false, line: "" }
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
