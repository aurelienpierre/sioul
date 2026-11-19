// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

// The Contacts page's filter row (ContactFilters.qml, wave D) on a stand-in
// for Sioul, at a computer's width and at a phone's 412 points: what the
// core says shows (how many of how many, the group, has or lacks); each
// change gives back the whole filter, one book ticked among several, a group
// or none, has or lacks; "Show all" clears all but the search; nothing passes
// the right edge on a phone. Fiction only. Run by tools/qml-test.sh.

import QtQuick
import QtQuick.Controls.Basic
import QtTest
import "../../qml"

Item {
    id: root

    width: 1200
    height: 800

    Theme {
        id: testTheme
    }

    QtObject {
        id: mock

        function text(key) { return key }
        function textArgs(key, args) { return key + ":" + args }
    }

    readonly property var view: ({
        filter: { books: [], group: "", ungrouped: false, phone: "any", email: "has", address: "any", title: "any", name: "mart" },
        books: [{ place: "cloud/contacts", name: "Contacts · example.org", read_only: false }, { place: "cloud/pro", name: "Professional · example.org", read_only: false }, { place: "local/home", name: "Home · this device alone", read_only: false }],
        categories: ["Clients", "Fournisseurs"],
        counted: "132 of 840",
        shown: 132,
        total: 840
    })

    Component {
        id: rowComponent

        ContactFilters {
            sioul: mock
            theme: testTheme
            view: root.view
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

        name: "ContactFilters"
        when: windowShown

        property var given: []

        function make(width) {
            const row = test.createTemporaryObject(rowComponent, root, { width: width })
            test.given = []
            row.asked.connect(f => test.given = test.given.concat([JSON.parse(JSON.stringify(f))]))
            test.waitForRendering(row)
            return row
        }

        function test_what_the_core_says_shows() {
            for (const width of [1000, 412]) {
                const row = make(width)
                compare(root.named(row, "contactFiltersCount")[0].text, "132 of 840")
                const wants = root.named(row, "contactFiltersWant")
                compare(wants.length, 4, "phone, e-mail, postal address, title")
                compare(wants[1].currentIndex, 1, "has an e-mail address")
                compare(wants[0].currentText, "contacts-filter-phone-any")
                verify(root.named(row, "contactFiltersClear")[0].visible, "narrowed: Show all offered")
                compare(root.named(row, "contactFiltersBooks")[0].text, "contacts-filter-books-all")
            }
        }

        // Each change gives back the whole filter, the search kept.
        function test_each_change_gives_the_whole_filter() {
            const row = make(1000)
            const group = root.named(row, "contactFiltersGroup")[0]
            group.activated(1)
            compare(test.given[0].ungrouped, true, "in no group")
            compare(test.given[0].group, "")
            compare(test.given[0].email, "has", "the others kept")
            compare(test.given[0].name, "mart", "the search kept")
            group.activated(3)
            compare([test.given[1].group, test.given[1].ungrouped], ["Fournisseurs", false])
            const title = root.named(row, "contactFiltersWant")[3]
            title.activated(1)
            compare(test.given[2].title, "has")
            root.named(row, "contactFiltersWant")[0].activated(2)
            compare(test.given[3].phone, "lacks")
            row.turnBook("cloud/pro", true)
            compare(test.given[4].books, ["cloud/pro"], "one book among several")
            mouseClick(root.named(row, "contactFiltersClear")[0])
            const cleared = test.given[5]
            compare([cleared.books.length, cleared.group, cleared.ungrouped, cleared.phone, cleared.email, cleared.address, cleared.title, cleared.name], [0, "", false, "any", "any", "any", "any", "mart"])
        }

        // Several books ticked: the button says how many; one book alone shows no button.
        function test_books_one_several_or_all() {
            const row = make(1000)
            row.view = Object.assign({}, root.view, { filter: Object.assign({}, root.view.filter, { books: ["cloud/pro", "local/home"] }) })
            wait(20)
            compare(root.named(row, "contactFiltersBooks")[0].text, "contacts-filter-books-some:{\"n\":2}")
            row.turnBook("local/home", false)
            compare(test.given[0].books, ["cloud/pro"])
            row.view = Object.assign({}, root.view, { books: [root.view.books[0]] })
            wait(20)
            verify(!root.named(row, "contactFiltersBooks")[0].visible, "one book: nothing to choose")
        }

        // On a phone: the row wraps, nothing past the right edge.
        function test_a_phone_s_width() {
            const row = make(412)
            const shown = root.all(row, c => c.visible && c.width > 0 && (c.objectName.startsWith("contactFilters")))
            verify(shown.length >= 6)
            for (const item of shown) {
                const x = item.mapToItem(row, 0, 0).x
                verify(x >= 0 && x + item.width <= 412 + 1, item.objectName + " at " + x + " + " + item.width)
            }
            verify(row.height > 60, "wrapped on several lines: " + row.height)
        }
    }
}
