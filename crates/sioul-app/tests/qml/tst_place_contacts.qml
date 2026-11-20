// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

// Contacts keeps its place (ContactsPage.qml, ListPlace.qml; docs/qt-quick.md,
// "A list keeps its place"), on a stand-in for Sioul that hands the whole list
// again after each change, as Rust does: the list scrolled to its middle, a
// card deleted from its row's menu, the top one too, one deleted on another
// device above the view, the list read again unchanged: the row at the top
// stays at its pixel, or the next one when it went. A card made with New,
// out of sight, comes into view by the smallest move; another search starts
// at the top. Made-up names. Run by tools/qml-test.sh.

import QtQuick
import QtQuick.Controls.Basic
import QtTest
import "../../qml"

Item {
    id: root

    width: 1000
    height: 700

    Theme {
        id: testTheme
    }

    // The stand-in's address book: invented people, sorted by name as Rust sorts them.
    property var people: []

    function person(n) {
        const name = "Person " + (n < 10 ? "0" : "") + n
        return { key: "/books/p" + n + ".vcf", uri: "sioul:contact/p" + n, name: name, title: "", detail: "p" + n + "@example.org", photo: "", email: "p" + n + "@example.org", read_only: false }
    }

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
        property string search: ""
        // The list, as the backend's property is (its change is `contactsChanged`): made whole again each time.
        property string contacts: "{}"

        function tell() {
            const shown = root.people.filter(p => mock.search === "" || p.name.indexOf(mock.search) >= 0).sort((a, b) => a.name.localeCompare(b.name))
            mock.contacts = JSON.stringify({ contacts: shown, sentence: "", can_add: true, categories: [], category: "", filter: { name: mock.search }, books: [], counted: "", shown: shown.length, total: root.people.length })
        }
        function text(key) { return key }
        function textWith(key, name, value) { return key + ":" + value }
        function textArgs(key, args) { return key + ":" + args }
        function textCounted(key, count) { return key + ":" + count }
        function contact(key) {
            const p = root.people.find(c => c.key === key)
            return p ? JSON.stringify({ key: p.key, uid: p.key, name: p.name, org: "", title: "", card: "", emails: [{ label: "", value: p.email }], phones: [], addresses: [], birthday: "", notes: "", urls: [], categories: [], book: "Contacts", place: "", read_only: false, photo: "", photo_web: "" }) : "null"
        }
        function deleteContact(key) {
            root.people = root.people.filter(p => p.key !== key)
            mock.tell()
        }
        function saveContact(key, json) {
            const card = JSON.parse(json)
            const made = key === "" ? "/books/new-" + root.people.length + ".vcf" : key
            root.people = root.people.filter(p => p.key !== made).concat([Object.assign(root.person(0), { key: made, uri: "sioul:contact/" + made, name: card.name })])
            mock.tell()
            return JSON.stringify({ key: made })
        }
        function searchContacts(text) {
            mock.search = text
            mock.tell()
        }
        function contactHours(key) { return "" }
        function clientProfile(ref) { return JSON.stringify({ exists: false, summary: "" }) }
        function personOf(key) { return "null" }
        function personOfCard(uid) { return "null" }
        function household() { return JSON.stringify({ persons: [], others: [], relations: [] }) }
        function addressBooks() { return "[]" }
        function placeOf(address) { return "" }
        function mapView() { return "{}" }
        function related(uri) { return "[]" }
    }

    QtObject {
        id: shell

        property bool compact: false
        property string sioulLocale: "en"
        property point menuAt: Qt.point(10, 10)

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

    // Every object under `item` that passes `test`, a scroll view's, a loader's and a popup's own included.
    function all(item, test, out) {
        out = out || []
        if (!item)
            return out
        const kids = item.children || []
        for (let i = 0; i < kids.length; ++i) {
            const kid = kids[i]
            if (test(kid) && out.indexOf(kid) < 0)
                out.push(kid)
            root.all(kid, test, out)
            if (kid.contentItem !== undefined && kid.contentItem !== null && kid.contentItem !== kid && kids.indexOf(kid.contentItem) < 0)
                root.all(kid.contentItem, test, out)
        }
        return out
    }

    TestCase {
        name: "PlaceContacts"
        when: windowShown

        property var page: null
        property var list: null

        function frame() {
            root.Window.window.update()
            waitForRendering(root)
        }

        function init() {
            mock.search = ""
            root.people = Array.from({ length: 80 }, (_, i) => root.person(i))
            mock.tell()
            page = createTemporaryObject(pageComponent, root)
            page.takeShown()
            list = root.all(page, c => c.reuseItems !== undefined && c.cacheBuffer !== undefined && c.visible)[0]
            frame()
        }

        // The rows shown, by name, with their tops in the list's view.
        function rows() {
            const out = []
            for (let i = 0; i < list.count; ++i) {
                const item = list.itemAtIndex(i)
                if (item !== null && item.visible)
                    out.push({ name: item.modelData.name, key: item.modelData.key, y: item.y - list.contentY, height: item.height, item: item })
            }
            return out.filter(r => r.y + r.height > 0 && r.y < list.height).sort((a, b) => a.y - b.y)
        }

        // Scrolled to its middle, the top row cut by 7 points.
        function toMiddle() {
            list.contentY = (list.contentHeight - list.height) / 2
            frame()
            list.contentY += rows()[0].y + 7
            frame()
            return rows()[0]
        }

        // A row's menu, opened as a mouse does, and its line `name` chosen.
        function fromMenu(row, name) {
            mouseClick(row.item, 30, 10, Qt.RightButton)
            tryVerify(() => root.all(page.Overlay.overlay, c => c.objectName === name && c.visible).length === 1, 2000, "the row's menu")
            root.all(page.Overlay.overlay, c => c.objectName === name && c.visible)[0].triggered()
        }

        function test_a_card_deleted_from_its_menu_keeps_the_place() {
            const top = toMiddle()
            verify(top.name !== "Person 00", "scrolled into the list")
            fromMenu(rows()[3], "contactRowDelete")
            wait(0)
            frame()
            compare(rows()[0].name, top.name, "the same card at the top")
            compare(rows()[0].y, top.y, "at the same pixel")
        }

        function test_the_top_card_deleted_leaves_the_next_one_in_its_place() {
            const top = toMiddle()
            const next = rows()[1]
            fromMenu(rows()[0], "contactRowDelete")
            wait(0)
            frame()
            compare(rows().map(r => r.name).indexOf(top.name), -1, "the top one gone")
            compare(rows()[0].name, next.name, "the next one at the top")
            compare(rows()[0].y, top.y, "in its place, to the pixel")
        }

        function test_another_device_and_the_same_list_again() {
            const top = toMiddle()
            // Deleted on another device, above the view; then the list read again as it is.
            mock.deleteContact("/books/p3.vcf")
            wait(0)
            frame()
            compare(rows()[0].name, top.name)
            compare(rows()[0].y, top.y)
            mock.tell()
            wait(0)
            frame()
            compare(rows()[0].name, top.name)
            compare(rows()[0].y, top.y)
        }

        // New, then Save: the card made out of sight comes into view by the smallest move, its
        // bottom at the list's bottom; the rows above it do not jump to the top.
        function test_a_card_made_here_comes_into_view() {
            toMiddle()
            page.startNew()
            const name = root.all(page, c => c.objectName === "contactFormName" && c.visible)[0]
            verify(name !== undefined, "the form")
            name.text = "Person 77b"
            const save = root.all(page, c => c.text === "ui-save" && c.visible && typeof c.clicked === "function")[0]
            mouseClick(save)
            wait(0)
            frame()
            const made = rows().find(r => r.name === "Person 77b")
            verify(made !== undefined, "in view")
            verify(Math.abs(made.y + made.height - list.height) < 1, "its bottom at the list's bottom: " + (made.y + made.height) + " / " + list.height)
        }

        // Another search: another list, from its top.
        function test_another_search_starts_at_the_top() {
            toMiddle()
            mock.searchContacts("Person 1")
            wait(0)
            frame()
            compare(list.contentY, list.originY, "at its top")
            compare(rows()[0].name, "Person 10")
        }
    }
}
