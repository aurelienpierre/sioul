// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

// A contact's card on a phone (ContactsPage.qml), on a stand-in for Sioul:
// an office's card with its opening hours, tied to one thing or to twelve,
// opens with its hours in sight, whole; what it is tied to scrolls with the
// card and never takes its room; Close stays in sight. Run by
// tools/qml-test.sh. Fiction only.

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

        compact: true
        touch: true
    }

    // An invented office: two ways to reach it, its hours, its ties.
    readonly property var office: ({
        key: "/books/office.vcf", uid: "office-1", name: "Health cover office", org: "Health cover office", title: "", card: "org",
        emails: [{ label: "work", value: "members@healthcover.example.org" }],
        phones: [{ label: "work", value: "+33 1 23 45 67 89" }],
        addresses: [], birthday: "", notes: "", urls: [], categories: [], book: "Contacts", place: "", read_only: false, photo: "", photo_web: "",
        hours: { open: { value: "Mo-Fr 09:00-12:00,14:00-16:30; PH off" }, phone: null, note: "" }
    })
    readonly property var hours: ({
        name: "Health cover office", card: "org", read_only: false, mark: false,
        open: { value: "Mo-Fr 09:00-12:00,14:00-16:30; PH off", short: "", lines: ["Monday to Friday: 9:00–12:00, 14:00–16:30"], holidays_off: true, unread: false, doubt: "", rest: [], source: "Typed by you on 14 September 2026", stale: false, now: "Open until 16:30" },
        phone: { value: "Mo-Th 09:00-12:00; PH off", short: "", lines: ["Monday to Thursday: 9:00–12:00"], holidays_off: true, unread: false, doubt: "", rest: [], source: "Typed by you on 14 September 2026", stale: false, now: "Closed: opens Monday at 9:00" },
        note: "Closed between Christmas and New Year.", numbers: ["+33 1 23 45 67 89"],
        asked: { name: "Health cover office", place: "", france: true },
        sources: "the directory and OpenStreetMap", map: "", map_name: "OpenStreetMap", holidays_known: true, offline: false
    })

    QtObject {
        id: mock

        signal linksChanged
        signal tasksChanged
        signal notesChanged
        signal eventsChanged
        signal householdChanged
        signal personsChanged
        signal placesChanged
        signal hoursFound(string key, string found)

        property string status: ""
        // How many things the card is tied to.
        property int ties: 1
        // The list, as the backend's property is (its change is `contactsChanged`).
        property string contacts: JSON.stringify({ contacts: [{ key: root.office.key, name: root.office.name, detail: "", title: "", photo: "" }], sentence: "", can_add: true, categories: [], category: "", filter: {}, books: [], counted: "", shown: 1, total: 1 })

        function text(key) { return key }
        function textWith(key, name, value) { return key + ":" + value }
        function textArgs(key, args) { return key + ":" + args }
        function textCounted(key, count) { return key + ":" + count }
        function contact(key) { return key === root.office.key ? JSON.stringify(root.office) : "null" }
        function contactHours(key) { return key === root.office.key ? JSON.stringify(root.hours) : "" }
        function clientProfile(ref) { return JSON.stringify({ exists: false, summary: "" }) }
        function personOf(key) { return "null" }
        function personOfCard(uid) { return "null" }
        function household() { return JSON.stringify({ persons: [], others: [], relations: [] }) }
        function addressBooks() { return "[]" }
        function placeOf(address) { return "" }
        function related(uri) {
            const out = []
            for (let i = 0; i < mock.ties; i++)
                out.push({ uri: "sioul:task/call-" + i, how: "Involves them", title: "Call the office about refund " + (i + 1), detail: "", when: "", kind: "task", found: true })
            return JSON.stringify(out)
        }
    }

    QtObject {
        id: shell

        property bool compact: true
        property string sioulLocale: "en"

        function openThing(item) {}
        function writeTo(address) {}
        function openPersonSheet(key, which) {}
        function openInvoiceDraft(draft) {}
        function linkFrom(source) {}
        function cardRef(person) { return "" }
    }

    Component {
        id: pageComponent

        ContactsPage {
            width: root.width
            height: root.height
            sioul: mock
            theme: testTheme
            window: shell
        }
    }

    // Every object under `item` that passes `test`, a scroll view's and a loader's own included.
    function all(item, test, out) {
        out = out || []
        if (!item)
            return out
        const kids = (item.children || [])
        for (let i = 0; i < kids.length; ++i) {
            const kid = kids[i]
            if (test(kid))
                out.push(kid)
            root.all(kid, test, out)
            if (kid.contentItem !== undefined && kid.contentItem !== null && kid.contentItem !== kid && kids.indexOf(kid.contentItem) < 0)
                root.all(kid.contentItem, test, out)
        }
        return out
    }

    function named(item, name) {
        return root.all(item, c => c.objectName === name)[0]
    }

    TestCase {
        name: "ContactCard"
        when: windowShown

        // The card's scroll view, the one that holds the opening hours.
        function scrollOf(item) {
            for (let p = item; p; p = p.parent) {
                if (p.contentY !== undefined && p.contentHeight !== undefined)
                    return p
            }
            return null
        }

        // On a phone, an office's card opens with its opening hours in sight, whole, however many
        // things it is tied to: what is tied to it scrolls with the card, never takes its room
        // (with twelve ties kept under the card, its scroll view was left 219 points of 891, the
        // opening hours' buttons ending at 458); Close stays in sight.
        function test_an_office_s_hours_in_sight_on_a_phone() {
            let room = 0
            for (const ties of [1, 12]) {
                mock.ties = ties
                const page = createTemporaryObject(pageComponent, root)
                page.open(root.office.key)
                tryVerify(() => root.named(page, "openingHoursCallBack") !== undefined && root.named(page, "openingHoursCallBack").visible, 2000)
                waitForRendering(page)
                const call = root.named(page, "openingHoursCallBack")
                const flick = scrollOf(call)
                verify(flick, "the card scrolls")
                compare(flick.contentY, 0, "opened at its top")
                const bottom = call.mapToItem(flick, 0, call.height).y
                verify(bottom <= flick.height, ties + " ties: Call back… ends at " + bottom + ", the card's scroll view at " + flick.height)
                // The ties in the card's own scroll view, which they leave as tall as with one.
                const tie = root.all(page, c => c.visible && c.text === "Call the office about refund " + ties)[0]
                verify(tie, "the last tie")
                verify(scrollOf(tie) === flick, "the ties scroll with the card")
                if (room === 0)
                    room = flick.height
                compare(flick.height, room, ties + " ties: the card keeps its room")
                const close = root.all(page, c => c.visible && c.text === "ui-close")[0]
                verify(close, "Close")
                verify(close.mapToItem(root, 0, close.height).y <= root.height + 0.5, "Close in sight")
            }
        }
    }
}
