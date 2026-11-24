// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

// A contact's form (ContactForm.qml) on a stand-in for Sioul, at a
// computer's width and at a phone's 392 points: under its groups, one line
// says that they do not decide who may reach the person when the card's
// address book holds its groups back (`heldBook`, someone else can write
// it), as soon as a group is typed, added or already on the card, never
// for a book whose groups decide, nor for a card in no group; its "Its
// groups…" asks for the book's switch (Contacts ⚙ ▸ Address books); the
// line stays within the edge. The rule itself is the core's
// (addressbooks.rs); this is the window's side. A new card's address book:
// asked when two books can take it, the one Sioul chose first chosen, carried
// by its edit; still asked when the one chosen went meanwhile, so that
// another can be chosen; never asked of a card that exists. Fiction only. Run
// by tools/qml-test.sh.

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

    QtObject {
        id: mock

        function text(key) { return key }
        function textWith(key, name, value) { return key + ":" + name + "=" + value }
    }

    // A card of the co-workers' book, invented.
    readonly property var card: ({ name: "Sam Exemple", prefix: "", given: "", family: "", suffix: "", org: "Exemple", title: "", birthday: "", notes: "", urls: [], categories: [], emails: [{ label: "work", value: "sam@example.org" }], phones: [], addresses: [] })

    Component {
        id: formComponent

        ContactForm {
            sioul: mock
            theme: testTheme
            known: ["Clients", "Team"]
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

        name: "ContactForm"
        when: windowShown

        function make(width, held) {
            const form = test.createTemporaryObject(formComponent, root, { width: width, heldBook: held })
            form.load(root.card)
            test.waitForRendering(form)
            return form
        }

        // The field that adds a group (typed, not yet added).
        function newGroup(form) {
            return root.all(form, c => c.placeholderText === "+ contact-category-add")[0]
        }

        // A book whose groups are held back: said once a group is typed, added or on the card.
        function test_a_group_on_a_held_book_says_it_does_not_decide() {
            for (const width of [1000, 392]) {
                const form = make(width, "Team")
                const line = root.named(form, "contactFormGroupsHeld")[0]
                verify(!line.visible, "no group yet: nothing to say")
                const field = newGroup(form)
                field.text = "Clients"
                verify(line.visible, "a group typed, not yet added, counts")
                compare(line.text, "contact-groups-held:book=Team", "the book named")
                field.accepted()
                compare(form.chosen, ["Clients"])
                compare(field.text, "")
                verify(line.visible, "the group added")
                test.waitForRendering(form)
                for (const item of [line].concat(root.all(line, c => c.visible && c.width > 0))) {
                    const x = item.mapToItem(form, 0, 0).x
                    verify(x >= 0 && x + item.width <= width + 1, "within " + width + ": " + x + " + " + item.width)
                }
                // Said, with no door: the book's switch is Contacts ⚙ ▸ Address books' alone.
                compare(root.named(form, "contactFormGroupsDoor").length, 0)
                form.chosen = []
                verify(!line.visible, "the group taken off: nothing to say")
                // A card that comes with its groups says it at once.
                form.load(Object.assign({}, root.card, { categories: ["Team"] }))
                verify(line.visible, "a card already in a group")
                compare(form.edit().categories, ["Team"], "the groups saved as they are: the line only says")
            }
        }

        // A new card asks its book when two can take it, Sioul's choice chosen and carried
        // by the edit; a book chosen that went meanwhile leaves the question open, its
        // box empty, rather than another book chosen in its place; a card that exists
        // asks nothing and carries none. Within a phone's width.
        function test_a_new_card_s_book() {
            for (const width of [1000, 392]) {
                const form = make(width, "")
                const books = [{ id: "cloud/contacts", name: "Contacts" }, { id: "cloud/team", name: "Team" }]
                form.books = books
                const box = root.named(form, "contactFormBook")[0]
                verify(!box.visible, "a card that exists: nothing asked")
                compare(form.edit().book, undefined, "nor carried")
                form.loadNew(Object.assign({}, root.card, { name: "" }), "cloud/team")
                verify(box.visible, "two books: asked")
                compare(box.currentText, "Team")
                compare(form.edit().book, "cloud/team")
                test.waitForRendering(form)
                const x = box.mapToItem(form, 0, 0).x
                verify(x >= 0 && x + box.width <= width + 1, "within " + width)
                // The book chosen gone, one left: still asked, nothing chosen in its place.
                form.books = [books[0]]
                verify(box.visible, "the one chosen went: asked again")
                compare(box.currentIndex, -1)
                compare(form.edit().book, "cloud/team", "the edit says the book chosen, which the backend refuses in words")
                form.loadNew(Object.assign({}, root.card, { name: "" }), "cloud/contacts")
                verify(!box.visible, "one book, chosen: nothing asked")
                compare(form.edit().book, "cloud/contacts")
                // Then a card that exists, in the same form: no book.
                form.load(root.card)
                verify(!box.visible)
                compare(form.edit().book, undefined)
            }
        }

        // A book whose groups decide (or a new card's first book that does): nothing said.
        function test_a_book_whose_groups_decide_says_nothing() {
            const form = make(1000, "")
            form.load(Object.assign({}, root.card, { categories: ["Clients"] }))
            newGroup(form).text = "Team"
            verify(!root.named(form, "contactFormGroupsHeld")[0].visible)
            // The book changes under the form (a move, the switch turned): the line follows.
            form.heldBook = "Team"
            verify(root.named(form, "contactFormGroupsHeld")[0].visible)
            form.heldBook = ""
            verify(!root.named(form, "contactFormGroupsHeld")[0].visible)
        }
    }
}
