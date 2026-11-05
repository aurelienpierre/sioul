// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

// Agenda ⚙ ▸ Your availability (AvailabilitySetup.qml, AvailabilityChooser.qml)
// on a stand-in for Sioul: each Nextcloud's line and the working hours'
// switch; one card per availability, its agendas with their words, each
// share's line, who keeps it, its link with Copy, Stop; "Share with…" asks
// who, names the way and shares it, another way one press away, a booking
// page's link pasted back; "New availability…" with no agenda ticked; read
// again when the agendas change. At a phone's width. Fiction only. Run by
// tools/qml-test.sh.

import QtQuick
import QtQuick.Controls.Basic
import QtTest
import "../../qml"

Item {
    id: root

    width: 412
    height: 2400

    Theme {
        id: testTheme
    }

    QtObject {
        id: mock

        signal agendaShared(string id, string answer)
        signal agendasChanged

        property var calls: []
        property var copied: []
        property int reads: 0
        property var shown: root.view

        function text(key) { return key }
        function textWith(key, name, value) { return key + ":" + value }
        function availability() { mock.reads++; return JSON.stringify(mock.shown) }
        function copyText(text) { mock.copied = mock.copied.concat([text]); return true }
        function availabilityWays(text) {
            const asked = JSON.parse(text)
            mock.calls = mock.calls.concat([["ways", asked]])
            const ways = {
                person: [{ way: "mirror-share", account: "cloud", who: asked.who, sentence: "availability-way-mirror-share:" + asked.who }, { way: "mirror-link", account: "cloud", who: "", sentence: "availability-way-mirror-link-person" }],
                group: [{ way: "mirror-share", account: "cloud", who: "group:" + asked.who, sentence: "availability-way-mirror-group" }],
                link: [{ way: "mirror-link", account: "cloud", who: "", sentence: "availability-way-mirror-link" }],
                booking: [{ way: "appointments", account: "cloud", who: "", sentence: "availability-way-appointments" }, { way: "mirror-link", account: "cloud", who: "", sentence: "availability-way-mirror-link-booking" }]
            }
            return JSON.stringify({ choices: ways[asked.audience] || [], problem: "" })
        }
        function shareAvailability(text) { mock.calls = mock.calls.concat([["share", JSON.parse(text)]]); return "" }
        function stopAvailability(text) { mock.calls = mock.calls.concat([["stop", JSON.parse(text)]]); return "" }
        function saveAvailability(text) {
            mock.calls = mock.calls.concat([["save", JSON.parse(text)]])
            mock.shown = Object.assign({}, root.view, { items: root.view.items.concat([{ id: "new1", name: "Public", agendas: [], words: {}, shares: [], mirrors: [] }]) })
            return JSON.stringify({ id: "new1", problem: "" })
        }
        function availabilityHours(account, on) { mock.calls = mock.calls.concat([["hours", account, on]]); return "" }
    }

    readonly property string link: "https://cloud.example.org/remote.php/dav/public-calendars/b1?export"
    readonly property var view: ({
        head: [{ account: "cloud", server: "cloud.example.org", line: "availability-server-sees:cloud.example.org", hours: false, propose: true }],
        items: [{
            id: "a1", name: "For Sam", rounding: 0, horizon: 60, closing: false, mirrors: ["cloud/busy-sam"],
            agendas: [{ id: "cloud/personal", name: "Personal" }, { id: "cloud/work", name: "Work" }],
            words: { "cloud/work": "Work" },
            shares: [
                { audience: "person", who: "sam", way: "mirror-share", account: "cloud", link: "", made: "2026-11-05", line: "availability-share-person", keeper: "availability-kept-by-devices" },
                { audience: "link", who: "", way: "mirror-link", account: "cloud", link: root.link, made: "2026-11-05", line: "availability-share-link", keeper: "availability-kept-while-open" }
            ]
        }],
        agendas: [{ id: "cloud/personal", name: "Personal", account: "cloud" }, { id: "cloud/work", name: "Work", account: "cloud" }],
        accounts: [{ id: "cloud", name: "you@cloud.example.org", google: false }]
    })

    Component {
        id: setupComponent

        AvailabilitySetup {
            width: root.width
            sioul: mock
            theme: testTheme
            about: "set-availability-help"
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
        return root.all(item, c => c.objectName === name)
    }

    TestCase {
        id: test

        name: "AvailabilitySetup"
        when: windowShown

        function init() {
            mock.calls = []
            mock.copied = []
            mock.shown = root.view
        }

        function make() {
            const setup = test.createTemporaryObject(setupComponent, root)
            test.waitForRendering(setup)
            return setup
        }

        function calls(kind) {
            return mock.calls.filter(c => c[0] === kind)
        }

        // A card: its name, its agendas with their words, each share's line,
        // who keeps it, the link with Copy, Stop; nothing wider than a phone.
        function test_a_card_says_who_sees_what_and_who_keeps_it() {
            const setup = make()
            compare(root.named(setup, "availabilityServerLine")[0].text, "availability-server-sees:cloud.example.org")
            compare(root.named(setup, "availabilityName")[0].text, "For Sam")
            compare(root.named(setup, "availabilityChip").map(c => c.text), ["Personal", "Work"], "a word that is its agenda's name, said once")
            compare(root.named(setup, "availabilityShareLine").map(l => l.text), ["availability-share-person", "availability-share-link"])
            compare(root.named(setup, "availabilityKeeper").map(l => l.text), ["availability-kept-by-devices", "availability-kept-while-open"])
            const links = root.named(setup, "availabilityLink")
            verify(!links[0].visible && links[1].visible, "a link only where the share has one")
            mouseClick(root.named(links[1], "linkCopy")[0])
            compare(mock.copied, [root.link])
            mouseClick(root.named(setup, "availabilityStop")[1])
            compare(calls("stop")[0][1], { id: "a1", way: "mirror-link", account: "cloud", who: "" })
            verify(setup.implicitWidth <= 412, "fits a phone upright: " + setup.implicitWidth)
        }

        // Share with…: who is asked; the way named in one sentence, another
        // one press away; Share sends the way chosen and closes.
        function test_share_with_names_the_way_and_shares_it() {
            const setup = make()
            mouseClick(root.named(setup, "availabilityShareWith")[0])
            const chooser = root.named(setup, "availabilityChooser")[0]
            verify(chooser !== undefined && chooser.visible)
            compare(calls("ways").length, 0, "a person: nothing asked before a name")
            const who = root.named(chooser, "availabilityWho")[0]
            who.forceActiveFocus()
            for (const key of [Qt.Key_S, Qt.Key_A, Qt.Key_M])
                keyClick(key)
            tryCompare(root.named(chooser, "availabilityWay")[0], "text", "availability-way-mirror-share:sam", 2000)
            compare(calls("ways").slice(-1)[0][1], { id: "a1", audience: "person", who: "sam" })
            mouseClick(root.named(chooser, "availabilityAnotherWay")[0])
            compare(root.named(chooser, "availabilityWay")[0].text, "availability-way-mirror-link-person")
            mouseClick(root.named(chooser, "availabilityAnotherWay")[0])
            mouseClick(root.named(chooser, "availabilityShare")[0])
            compare(calls("share")[0][1], { id: "a1", audience: "person", who: "sam", way: "mirror-share", account: "cloud", link: "" })
            tryCompare(setup, "asking", "")
        }

        // A booking tool: your server's own page first; its link pasted
        // back before Share; a link alone needs no name.
        function test_a_booking_page_takes_its_link_pasted_back() {
            const setup = make()
            setup.asking = "a1"
            wait(30)
            const chooser = root.named(setup, "availabilityChooser")[0]
            const booking = root.named(chooser, "availabilityAudience").find(b => b.text === "availability-audience-booking")
            mouseClick(booking)
            compare(root.named(chooser, "availabilityWay")[0].text, "availability-way-appointments")
            verify(root.named(chooser, "availabilityOpenPage")[0].visible)
            verify(root.named(chooser, "availabilityBookingDirect")[0].visible, "the tool reading your server itself, said")
            const share = root.named(chooser, "availabilityShare")[0]
            verify(!share.enabled, "nothing to share before its link")
            root.named(chooser, "availabilityPasted")[0].text = "https://cloud.example.org/index.php/apps/calendar/appointment/abc"
            verify(share.enabled)
            mouseClick(share)
            compare(calls("share")[0][1], { id: "a1", audience: "booking", who: "", way: "appointments", account: "cloud", link: "https://cloud.example.org/index.php/apps/calendar/appointment/abc" })
            compare(setup.pageOf("appointments", "cloud"), "https://cloud.example.org/index.php/apps/calendar/")
        }

        // New availability…: a name and the agendas, none ticked; saved,
        // its "Share with…" opened.
        function test_a_new_availability_starts_with_no_agenda_ticked() {
            const setup = make()
            mouseClick(root.named(setup, "availabilityNew")[0])
            test.waitForRendering(setup)
            const boxes = root.named(setup, "availabilityAgenda")
            compare(boxes.map(b => b.checked), [false, false], "none ticked")
            mouseClick(boxes[0])
            root.named(setup, "availabilityNewName")[0].text = "Public"
            mouseClick(root.named(setup, "availabilitySave")[0])
            const saved = calls("save")[0][1]
            compare(saved.name, "Public")
            compare(saved.agendas, ["cloud/personal"], JSON.stringify(saved))
            compare(JSON.stringify(saved.words), "{}")
            compare(setup.asking, "new1", "its Share with… open")
            verify(!setup.making)
        }

        // The working hours' switch, per Nextcloud.
        function test_working_hours_go_to_a_server_by_its_switch() {
            const setup = make()
            const box = root.named(setup, "availabilityHours")[0]
            compare(box.text, "availability-hours:cloud.example.org")
            mouseClick(box)
            compare(calls("hours")[0], ["hours", "cloud", true])
        }

        // Changed elsewhere (a sync, another device, a server's answer): read again.
        function test_a_change_elsewhere_is_read_again() {
            make()
            const before = mock.reads
            mock.agendasChanged()
            mock.agendaShared("availability", "{}")
            mock.agendaShared("cloud/personal", "{}")
            compare(mock.reads, before + 2, "the agendas and its own answers, not another agenda's")
        }
    }
}
