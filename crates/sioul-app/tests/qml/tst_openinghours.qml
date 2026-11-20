// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

// An office's opening hours in the window (OpeningHours.qml,
// OpeningHoursForm.qml, OpeningHoursLookup.qml) on a stand-in for Sioul
// that answers through calls, as the backend does, and as late, empty or
// odd as it can: the section shows nothing before the card is read, then
// what is true now and the hours; a person's card offers no lookup, a card
// that can only be read no change; the lookup sends only the words shown,
// once, ignores an answer for another card, says when nothing is found,
// and offers a number after a pick, never writing it by itself; the form
// fills its week from the short form and makes the short form again from
// its days. Everything within the edge at 412 points. The rules themselves
// are the core's (opening.rs, hourslookup.rs). Fiction only. Run by
// tools/qml-test.sh.

import QtQuick
import QtQuick.Controls.Basic
import QtTest
import "../../qml"

Item {
    id: root

    width: 1200
    height: 1600

    Theme {
        id: testTheme
    }

    // An invented town hall's section, as `contactHours` writes it.
    readonly property var hall: ({
        name: "Example town hall", card: "", read_only: false,
        open: { value: "Mo-Fr 09:00-12:00,14:00-17:00; PH off", lines: ["Monday to Friday: 9:00–12:00, 14:00–17:00"], holidays_off: true, unread: false, rest: [], source: "Typed by you on 20 November 2026", stale: false, now: "Open until 12:00" },
        phone: null, note: "Closed in August", numbers: ["01 23 45 67 89"],
        asked: { name: "Example town hall", place: "99999 Exempleville", france: true },
        sources: "the directory and OpenStreetMap", offline: false
    })

    QtObject {
        id: mock

        signal contactsChanged
        signal hoursFound(string key, string found)

        // What the next call of `contactHours` answers: "" while the card is not read yet.
        property string section: ""
        property var asked: []
        property var picked: []
        property var added: []
        property var saved: []
        property int calls: 0

        function text(key) { return key }
        function textWith(key, name, value) { return key + ":" + name + "=" + value }
        function contactHours(key) { return mock.section }
        function callBack(key) { mock.calls++; return JSON.stringify({ uid: "call-1" }) }
        function lookUpHours(key, asked) { mock.asked = mock.asked.concat([asked]) }
        function pickHours(key, found) { mock.picked = mock.picked.concat([found]); return JSON.stringify({ ok: true, offers: { phone: "+33 1 98 76 54 32", address: "" } }) }
        function addToCard(key, what, value) { mock.added = mock.added.concat([what + "=" + value]); return JSON.stringify({ ok: true }) }
        function saveContactHours(key, typed) { mock.saved = mock.saved.concat([typed]); return JSON.stringify({ ok: true }) }
        // As the program reads them: what it counts by day, the rest as written.
        function readHours(text) {
            const known = {
                "Mo-Fr 09:00-12:00,14:00-17:00; PH off": ["09:00-12:00, 14:00-17:00", "09:00-12:00, 14:00-17:00", "09:00-12:00, 14:00-17:00", "09:00-12:00, 14:00-17:00", "09:00-12:00, 14:00-17:00", "", ""],
                "mo 09:00-12:00, 14:00-17:00; tu 09:00-12:00, 14:00-17:00; we 09:00-12:00, 14:00-17:00; th 09:00-12:00, 14:00-17:00; fr 09:00-12:00, 14:00-17:00; sa 09:00-12:00": ["09:00-12:00, 14:00-17:00", "09:00-12:00, 14:00-17:00", "09:00-12:00, 14:00-17:00", "09:00-12:00, 14:00-17:00", "09:00-12:00, 14:00-17:00", "09:00-12:00", ""]
            }
            const days = known[text]
            if (days === undefined)
                return JSON.stringify({ written: "", lines: [], days: ["", "", "", "", "", "", ""], rest: text.trim() === "" ? [] : [text], holidays_off: true })
            return JSON.stringify({ written: text, lines: ["read"], days: days, rest: [], holidays_off: true })
        }
    }

    Component {
        id: sectionComponent

        OpeningHours {
            sioul: mock
            theme: testTheme
            window: root
            key: "/books/hall.vcf"
        }
    }

    Component {
        id: lookupComponent

        OpeningHoursLookup {
            sioul: mock
            theme: testTheme
        }
    }

    Component {
        id: formComponent

        OpeningHoursForm {
            sioul: mock
            theme: testTheme
        }
    }

    // Every item under `item` that passes `test`, a popup's content and a scroll view's included.
    function all(item, test, out) {
        out = out || []
        if (!item)
            return out
        for (let i = 0; i < (item.children || []).length; i++) {
            const child = item.children[i]
            if (test(child))
                out.push(child)
            root.all(child, test, out)
        }
        if (item.contentItem && item.contentItem !== item && (item.children || []).indexOf(item.contentItem) < 0) {
            if (test(item.contentItem))
                out.push(item.contentItem)
            root.all(item.contentItem, test, out)
        }
        return out
    }

    function named(item, name) {
        return root.all(item, c => c.objectName === name)[0]
    }

    // Every visible item within `width` of `within`.
    function inside(within, width) {
        for (const item of root.all(within, c => c.visible && c.width > 0 && c.text !== undefined)) {
            const x = item.mapToItem(within, 0, 0).x
            if (x < -1 || x + item.width > width + 1)
                return item.text + " at " + x + " + " + item.width
        }
        return ""
    }

    TestCase {
        id: test

        name: "OpeningHours"
        when: windowShown

        function init() {
            mock.section = ""
            mock.asked = []
            mock.picked = []
            mock.added = []
            mock.saved = []
            mock.calls = 0
        }

        // Before the card is read, nothing; once the contacts change, its hours and what is true now.
        function test_the_section_reads_the_card_when_it_changes() {
            for (const width of [1000, 412]) {
                init()
                const section = createTemporaryObject(sectionComponent, root, { width: width })
                waitForRendering(section)
                compare(section.shown, null, "the backend has not answered")
                verify(!root.named(section, "openingHoursChange").visible)
                mock.section = JSON.stringify(root.hall)
                mock.contactsChanged()
                waitForRendering(section)
                compare(root.named(section, "openingHoursNow").text, "Open until 12:00")
                compare(root.named(section, "openingHoursNote").text, "Closed in August")
                verify(root.named(section, "openingHoursLookUp").visible && root.named(section, "openingHoursChange").visible && root.named(section, "openingHoursCallBack").visible)
                compare(root.inside(section, width), "", "within " + width)
                // Call back: one press, the task made by the backend.
                let answer = null
                section.calledBack.connect(a => answer = a)
                mouseClick(root.named(section, "openingHoursCallBack"))
                compare(mock.calls, 1)
                compare(answer.uid, "call-1")
            }
        }

        // A person's card: never looked up. A card only read: neither changed nor looked up.
        function test_what_a_card_does_not_offer() {
            const section = createTemporaryObject(sectionComponent, root, { width: 600 })
            mock.section = JSON.stringify(Object.assign({}, root.hall, { card: "person", asked: null }))
            section.reload()
            verify(!root.named(section, "openingHoursLookUp").visible, "a person's name is never sent")
            verify(root.named(section, "openingHoursChange").visible)
            mock.section = JSON.stringify(Object.assign({}, root.hall, { read_only: true }))
            section.reload()
            verify(!root.named(section, "openingHoursLookUp").visible && !root.named(section, "openingHoursChange").visible)
            // A card with no hours yet says so, and offers to add them.
            mock.section = JSON.stringify(Object.assign({}, root.hall, { open: null, note: "" }))
            section.reload()
            verify(root.named(section, "openingHoursNone").visible)
            compare(root.named(section, "openingHoursChange").text, "hours-add")
            // The card gone: nothing.
            mock.section = ""
            section.reload()
            compare(section.shown, null)
        }

        // The words shown are what is sent, once; an answer for another card says nothing; nothing found is said.
        function test_a_lookup_sends_what_it_shows_and_waits() {
            for (const width of [1000, 412]) {
                init()
                const holder = createTemporaryObject(lookupComponent, root, {})
                holder.begin("/books/hall.vcf", root.hall)
                holder.width = width
                tryVerify(() => holder.opened && holder.contentItem.width <= width, 2000)
                waitForRendering(holder.contentItem)
                compare(root.named(holder.contentItem, "openingHoursLookupWords").text, "Example town hall")
                compare(mock.asked.length, 0, "nothing sent before the press")
                root.named(holder.contentItem, "openingHoursLookupWords").text = "Mairie de Exempleville"
                holder.ask()
                compare(mock.asked.length, 1)
                compare(JSON.parse(mock.asked[0]), { name: "Mairie de Exempleville", place: "99999 Exempleville", france: true })
                // Another card's answer, late: ignored.
                mock.hoursFound("/books/other.vcf", JSON.stringify({ found: [{ name: "Elsewhere" }], failed: [] }))
                compare(holder.step, "asking")
                // Nothing found, one source away: both said, nothing to pick.
                mock.hoursFound("/books/hall.vcf", JSON.stringify({ found: [], failed: ["OpenStreetMap did not answer"] }))
                compare(holder.step, "answered")
                verify(root.named(holder.contentItem, "openingHoursLookupNone").visible)
                compare(root.named(holder.contentItem, "openingHoursLookupPick0"), undefined)
                // A second answer after it: ignored, the list stays.
                mock.hoursFound("/books/hall.vcf", JSON.stringify({ found: [{ name: "Late" }], failed: [] }))
                compare(holder.found.length, 0)
                holder.close()
            }
        }

        // Picked: the backend writes the hours; the number the card lacks is offered, written only on its press.
        function test_a_pick_offers_what_the_card_lacks() {
            const holder = createTemporaryObject(lookupComponent, root, {})
            holder.begin("/books/hall.vcf", root.hall)
            holder.width = 412
            tryVerify(() => holder.opened, 2000)
            holder.ask()
            const office = { source: "service-public.gouv.fr", name: "Mairie - Exempleville", address: "Place de l'Exemple\n99999 Exempleville", phone: "+33 1 98 76 54 32", hours: "Mo-Fr 09:00-17:00; PH off", note: "", lines: ["Monday to Friday: 9:00–17:00"], source_name: "the directory" }
            const bare = { source: "openstreetmap.org", name: "Example post office", address: "", phone: "", hours: "", note: "", lines: [], source_name: "OpenStreetMap" }
            mock.hoursFound("/books/hall.vcf", JSON.stringify({ found: [office, bare], failed: [] }))
            waitForRendering(holder.contentItem)
            verify(root.named(holder.contentItem, "openingHoursLookupPick0").visible)
            verify(!root.named(holder.contentItem, "openingHoursLookupPick1").visible, "no hours there: nothing to take")
            compare(root.inside(holder.contentItem, holder.contentItem.width), "")
            compare(mock.added.length, 0)
            holder.take(office)
            compare(mock.picked.length, 1)
            compare(holder.step, "picked")
            const add = root.named(holder.contentItem, "openingHoursLookupAddPhone")
            verify(add.visible)
            compare(mock.added.length, 0, "never written by itself")
            mouseClick(add)
            compare(mock.added, ["phone=+33 1 98 76 54 32"])
            verify(!add.visible)
            holder.close()
        }

        // The week fills from the short form; a day changed makes the short form again.
        function test_the_form_fills_its_week_and_back() {
            const form = createTemporaryObject(formComponent, root, {})
            form.edit("/books/hall.vcf", root.hall)
            form.width = 412
            tryVerify(() => form.opened && form.contentItem.width <= 412, 2000)
            waitForRendering(form.contentItem)
            compare(root.named(form.contentItem, "openingHoursFormDay0").text, "09:00-12:00, 14:00-17:00")
            compare(root.named(form.contentItem, "openingHoursFormDay5").text, "", "Saturday closed")
            root.named(form.contentItem, "openingHoursFormDay5").text = "09:00-12:00"
            form.fromDays()
            compare(root.named(form.contentItem, "openingHoursFormShort").text, "mo 09:00-12:00, 14:00-17:00; tu 09:00-12:00, 14:00-17:00; we 09:00-12:00, 14:00-17:00; th 09:00-12:00, 14:00-17:00; fr 09:00-12:00, 14:00-17:00; sa 09:00-12:00")
            compare(root.inside(form.contentItem, form.contentItem.width), "")
            // Typed so that Sioul cannot count it: said, and kept for the free line.
            root.named(form.contentItem, "openingHoursFormShort").text = "by appointment"
            form.readShort()
            compare(root.named(form.contentItem, "openingHoursFormReads").text, "hours-form-unread")
            form.save()
            compare(JSON.parse(mock.saved[0]), { open: "by appointment", phone: "", note: "Closed in August" })
        }
    }
}
