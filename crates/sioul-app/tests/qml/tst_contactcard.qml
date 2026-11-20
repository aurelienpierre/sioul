// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

// A contact's card on a phone (ContactsPage.qml), on a stand-in for Sioul:
// an office's card with its opening hours, tied to one thing or to twelve,
// opens with its hours in sight, whole; what it is tied to scrolls with the
// card and never takes its room; Close stays in sight. Delete beside Edit,
// pressed, does what the row menu's Delete does and closes the card, and a
// card of a book you only read offers none. Run by
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
    // An invented person with a postal address (its details under More), and a card of a book read only.
    readonly property var jeanne: ({
        key: "/books/jeanne.vcf", uid: "jeanne-1", name: "Jeanne Exemple", org: "", title: "", card: "",
        emails: [{ label: "home", value: "jeanne@example.org" }],
        phones: [{ label: "cell", value: "+33 6 12 34 56 78" }],
        addresses: [{ label: "home", value: "1 rue de l'Exemple\n99999 Exempleville" }], birthday: "", notes: "", urls: [], categories: [], book: "Contacts", place: "cloud/contacts", read_only: false, photo: "", photo_web: "",
        hours: { open: null, phone: null, note: "" }
    })
    readonly property var shared: ({
        key: "/books/shared.vcf", uid: "shared-1", name: "Sam Exemple", org: "", title: "", card: "",
        emails: [{ label: "work", value: "sam@example.org" }], phones: [],
        addresses: [], birthday: "", notes: "", urls: [], categories: [], book: "Shared with you", place: "cloud/shared", read_only: true, photo: "", photo_web: "",
        hours: { open: null, phone: null, note: "" }
    })
    readonly property var cards: [root.office, root.jeanne, root.shared]
    // The address books, as the view lists them (`ContactsView::books`).
    readonly property var contactsBook: ({ place: "cloud/contacts", name: "Contacts", read_only: false, groups: true })
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
        // The address books, read only ones too, as the view lists them.
        property var books: [root.contactsBook]
        // What was asked: the cards deleted.
        property var deleted: []
        // The list, as the backend's property is (its change is `contactsChanged`).
        property string contacts: JSON.stringify({ contacts: root.cards.map(c => ({ key: c.key, name: c.name, detail: "", title: "", photo: "", place: c.place, read_only: c.read_only })), sentence: "", can_add: true, categories: [], category: "", filter: {}, books: mock.books, counted: "", shown: root.cards.length, total: root.cards.length })

        function text(key) { return key }
        function textWith(key, name, value) { return key + ":" + value }
        function textArgs(key, args) { return key + ":" + args }
        function textCounted(key, count) { return key + ":" + count }
        function contact(key) {
            const card = root.cards.find(c => c.key === key)
            if (!card)
                return "null"
            return JSON.stringify(card)
        }
        function contactHours(key) { return key === root.office.key ? JSON.stringify(root.hours) : "" }
        function clientProfile(ref) { return JSON.stringify({ exists: false, summary: "" }) }
        function personOf(key) { return "null" }
        function personOfCard(uid) { return "null" }
        function household() { return JSON.stringify({ persons: [], others: [], relations: [] }) }
        function addressBooks() { return "[]" }
        // Scheduled with Undo by the backend; the stand-in keeps what was asked.
        function deleteContact(key) { mock.deleted = mock.deleted.concat([key]) }
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

        // Each test from the same start: nothing asked yet.
        function init() {
            mock.ties = 1
            mock.books = [root.contactsBook]
            mock.deleted = []
        }

        // The page with a card open, laid out.
        function opened(card) {
            const page = createTemporaryObject(pageComponent, root)
            page.open(card.key)
            waitForRendering(page)
            return page
        }

        // What shows by its objectName, or by its words: visible ones only.
        function shown(page, name) {
            return root.all(page, c => c.visible && c.objectName === name)[0]
        }
        function says(page, words) {
            return root.all(page, c => c.visible && c.text === words)[0]
        }

        // Within the card's width on a phone: never past the screen's edge.
        function within(item) {
            const x = item.mapToItem(root, 0, 0).x
            verify(x >= 0 && x + item.width <= root.width + 0.5, item.objectName + ": " + x + " + " + item.width + " within " + root.width)
        }

        // Delete beside Edit, pressed: the card closed, its deletion asked of the
        // backend (which waits ten seconds with Undo in the status line, as the row
        // menu's Delete does). A card of a book you only read offers neither.
        function test_delete_on_the_card() {
            const page = opened(root.jeanne)
            const remove = shown(page, "contactCardDelete")
            verify(remove, "Delete under the card")
            verify(says(page, "ui-edit"), "beside Edit")
            compare(remove.text, "ui-delete")
            verify(!remove.highlighted, "a colour that does not shout")
            within(remove)
            mouseClick(remove)
            compare(mock.deleted, [root.jeanne.key], "deleted as the row menu deletes")
            compare(page.person, null, "the card closed")
            compare(page.openKey, "")
            const readOnly = opened(root.shared)
            verify(readOnly.person !== null)
            verify(!shown(readOnly, "contactCardDelete"), "no Delete on a card of a book read only")
            verify(!says(readOnly, "ui-edit"), "nor Edit")
            verify(says(readOnly, "ui-close"), "Close stays")
        }

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
