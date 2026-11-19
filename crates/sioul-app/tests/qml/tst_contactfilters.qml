// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

// The Contacts page's filter row (ContactFilters.qml, wave D) on a stand-in
// for Sioul, at a computer's width and at a phone's 412 points: what the
// core says shows (how many of how many, the group, has or lacks); each
// change gives back the whole filter, one book ticked among several, a group
// or none, has or lacks; "Show all" clears all but the search; nothing passes
// the right edge on a phone; at a phone's width (`compact`) the filters fold
// behind one Filters button that counts those set, "Show all" and the count
// beside it in one row, unfolded and folded again by it. Fiction only. Run by
// tools/qml-test.sh.

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
        function textCounted(key, n) { return key + ":" + n }
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

        function make(width, compact) {
            const row = test.createTemporaryObject(rowComponent, root, { width: width, compact: compact === true })
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
                verify(!root.named(row, "contactFiltersFold")[0].visible, "no Filters button unless compact")
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

        // A phone (`compact`, the column 392 points wide): the filters folded behind one
        // button that counts those set, "Show all" and the count beside it, one row; the
        // button unfolds them, every one within the edge, and folds them again.
        function test_a_phone_folds_them_behind_one_button() {
            const row = make(392, true)
            const fold = root.named(row, "contactFiltersFold")[0]
            const clear = root.named(row, "contactFiltersClear")[0]
            const count = root.named(row, "contactFiltersCount")[0]
            verify(fold.visible && !fold.checked && !row.unfolded)
            compare(fold.text, "task-filters:1", "one set: has an e-mail address")
            verify(!root.named(row, "contactFiltersBooks")[0].visible, "the books folded")
            verify(!root.named(row, "contactFiltersGroup")[0].visible, "the group folded")
            compare(root.named(row, "contactFiltersWant").filter(w => w.visible).length, 0, "has or lacks folded")
            verify(clear.visible && count.visible)
            compare(count.text, "132 of 840")
            compare([clear.y, count.y], [fold.y, fold.y], "one row: the button, Show all, the count")
            verify(count.x + count.width <= 392 + 1, "within the edge: " + (count.x + count.width))
            compare(row.height, fold.height, "one row high")

            mouseClick(fold)
            verify(row.unfolded && fold.checked, "unfolded")
            test.waitForRendering(row)
            verify(root.named(row, "contactFiltersBooks")[0].visible && root.named(row, "contactFiltersGroup")[0].visible)
            compare(root.named(row, "contactFiltersWant").filter(w => w.visible).length, 4)
            for (const item of root.all(row, c => c.visible && c.width > 0 && c.objectName.startsWith("contactFilters"))) {
                const x = item.mapToItem(row, 0, 0).x
                verify(x >= 0 && x + item.width <= 392 + 1, item.objectName + " at " + x + " + " + item.width)
            }
            verify(row.height > 3 * fold.height, "unfolded, the row wraps: " + row.height)
            // A change made unfolded keeps the fold as it is.
            root.named(row, "contactFiltersWant")[3].activated(2)
            compare(test.given[0].title, "lacks")
            verify(row.unfolded)

            mouseClick(fold)
            verify(!row.unfolded && !fold.checked, "folded again")
            // The count follows the filters: four set; none set, the word alone and no Show all.
            row.view = Object.assign({}, root.view, { filter: Object.assign({}, root.view.filter, { books: ["cloud/pro"], ungrouped: true, title: "lacks" }) })
            compare(fold.text, "task-filters:4")
            row.view = Object.assign({}, root.view, { filter: { books: [], group: "", ungrouped: false, phone: "any", email: "any", address: "any", title: "any", name: "" }, counted: "840 contacts" })
            compare(fold.text, "task-filters:0")
            verify(!clear.visible, "nothing set: no Show all")
            test.waitForRendering(row)
            compare(count.y, fold.y, "the count beside the button")
        }
    }
}
