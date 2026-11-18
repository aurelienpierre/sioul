// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

// Contacts ⚙ ▸ Address books (AddressBooksSetup.qml, wave D) on a stand-in
// for Sioul, at a computer's width and at a phone's 412 points: each book's
// line says where it is kept, how many, whose it is, whether it can be
// written; yours opens to its sharing (asked of its server then), is renamed,
// merged into another, removed after the question of where its contacts go;
// someone else's offers only "Stop showing"; Google's says what it allows
// and offers nothing else; "New address book…" sends its account and name.
// Fiction only. Run by tools/qml-test.sh.

import QtQuick
import QtQuick.Controls.Basic
import QtTest
import "../../qml"

Item {
    id: root

    width: 1200
    height: 2400

    Theme {
        id: testTheme
    }

    QtObject {
        id: mock

        signal bookShared(string place, string answer)
        signal contactsChanged

        property var calls: []

        function text(key) { return key }
        function textArgs(key, args) { return key + ":" + args }
        function addressBooksSetup() { return JSON.stringify(root.view) }
        function readBookSharing(place) { mock.calls = mock.calls.concat([["sharing", place]]) }
        function shareBook(place, who, write) { mock.calls = mock.calls.concat([["share", place, who, write]]) }
        function unshareBook(place, who) { mock.calls = mock.calls.concat([["unshare", place, who]]) }
        function renameAddressBook(place, name) { mock.calls = mock.calls.concat([["rename", place, name]]); return "" }
        function makeAddressBook(text) { mock.calls = mock.calls.concat([["make", JSON.parse(text)]]); return "" }
        function addressBookRemoval(place) {
            mock.calls = mock.calls.concat([["removal", place]])
            return JSON.stringify(place === "cloud/contacts"
                ? { act: "cards", sentence: "“Contacts” holds 840 contacts. Where do they go?", count: 840, into: [["cloud/pro", "Professional · example.org"], ["local/home", "Home · this device alone"]], note: "Deleted with it, they are copied on this device first." }
                : { act: "leave", sentence: "Stop showing “Team”? It stays its owner's.", count: 12, into: [], note: "" })
        }
        function removeAddressBook(place, how) { mock.calls = mock.calls.concat([["remove", place, how]]); return "" }
    }

    readonly property var line: ({ count: 0, counted: "0 contacts", waiting: "", fixed: "", can_rename: true, can_remove: true, can_share: true, can_leave: false, writable: true, access: "Can be written", whose: "Yours" })
    readonly property var view: ({
        books: [
            Object.assign({}, root.line, { place: "cloud/contacts", account: "cloud", name: "Contacts", kept: "example.org", count: 840, counted: "840 contacts" }),
            Object.assign({}, root.line, { place: "cloud/pro", account: "cloud", name: "Professional", kept: "example.org" }),
            Object.assign({}, root.line, { place: "cloud/team_shared_by_sam", account: "cloud", name: "Team", kept: "example.org", whose: "Shared with you by sam", access: "Read only", writable: false, can_rename: false, can_remove: false, can_share: false, can_leave: true }),
            Object.assign({}, root.line, { place: "google/default", account: "google", name: "Address book", kept: "Google", can_rename: false, can_remove: false, can_share: false, fixed: "Google keeps one address book." })
        ],
        homes: [{ id: "cloud", name: "you@example.org" }, { id: "local", name: "on this device alone" }]
    })

    Component {
        id: setupComponent

        AddressBooksSetup {
            sioul: mock
            theme: testTheme
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

        name: "AddressBooksSetup"
        when: windowShown

        function init() {
            mock.calls = []
        }

        function make(width) {
            const setup = test.createTemporaryObject(setupComponent, root, { width: width })
            test.waitForRendering(setup)
            return setup
        }

        function book(setup, index) {
            return root.named(setup, "addressBook")[index]
        }

        // Each line: where, how many, whose, written or read only; nothing past the edge on a phone.
        function test_each_line_says_whose_and_what() {
            for (const width of [1000, 412]) {
                const setup = make(width)
                compare(root.named(setup, "addressBook").length, 4)
                const lines = root.all(book(setup, 2), c => c.text === "example.org · 0 contacts · Shared with you by sam · Read only")
                compare(lines.length, 1, "someone else's, read only, said on its line")
                for (const item of root.named(setup, "addressBookLine")) {
                    const x = item.mapToItem(setup, 0, 0).x
                    verify(x >= 0 && x + item.width <= width + 1, "within " + width + ": " + x + " + " + item.width)
                }
            }
        }

        // Yours: its sharing asked when it opens, renamed, shared, merged, removed after the question.
        function test_yours_renamed_shared_merged_removed() {
            const setup = make(1000)
            setup.toggle("cloud/contacts")
            wait(30)
            compare(JSON.stringify(mock.calls[0]), JSON.stringify(["sharing", "cloud/contacts"]))
            const contacts = book(setup, 0)
            const name = root.named(contacts, "addressBookName")[0]
            name.text = "Personal"
            mouseClick(root.named(contacts, "addressBookRename")[0])
            compare(JSON.stringify(mock.calls[1]), JSON.stringify(["rename", "cloud/contacts", "Personal"]))
            mock.bookShared("cloud/contacts", JSON.stringify({ line: "Only you.", sharer: "nextcloud", people: [], problem: "" }))
            wait(30)
            root.named(contacts, "addressBookShareWho")[0].text = "office"
            mouseClick(root.named(contacts, "addressBookShare")[0])
            compare(JSON.stringify(mock.calls[2]), JSON.stringify(["share", "cloud/contacts", "office", false]))
            // Merged into another: its contacts moved there, then it removed.
            mouseClick(root.named(contacts, "addressBookMerge")[0])
            wait(30)
            const into = root.named(contacts, "addressBookMergeInto")[0]
            verify(into.visible)
            compare(into.count, 2, "the other books that can be written")
            mouseClick(root.named(contacts, "addressBookMergeGo")[0])
            compare(JSON.stringify(mock.calls[3]), JSON.stringify(["remove", "cloud/contacts", "move:cloud/pro"]))
            // Removed: where its contacts go, asked first.
            mouseClick(root.named(contacts, "addressBookRemove")[0])
            wait(30)
            compare(root.named(contacts, "addressBookRemoval")[0].text, "“Contacts” holds 840 contacts. Where do they go?")
            compare(mock.calls.length, 5, "nothing removed yet")
            const moveTo = root.named(contacts, "addressBookRemoveInto")[0]
            moveTo.currentIndex = 1
            mouseClick(root.named(contacts, "addressBookRemoveMove")[0])
            compare(JSON.stringify(mock.calls[5]), JSON.stringify(["remove", "cloud/contacts", "move:local/home"]))
        }

        // Someone else's: Stop showing alone; Google's: what it allows, nothing else.
        function test_someone_elses_and_google_s() {
            const setup = make(412)
            setup.toggle("cloud/team_shared_by_sam")
            wait(30)
            compare(mock.calls.length, 0, "not yours to share: no server asked")
            const team = book(setup, 2)
            verify(!root.named(team, "addressBookName")[0].visible && !root.named(team, "addressBookMerge")[0].visible)
            const leave = root.named(team, "addressBookRemove")[0]
            compare(leave.text, "books-leave")
            mouseClick(leave)
            wait(30)
            mouseClick(root.named(team, "addressBookRemoveGo")[0])
            compare(JSON.stringify(mock.calls[1]), JSON.stringify(["remove", "cloud/team_shared_by_sam", "leave"]))
            setup.toggle("google/default")
            wait(30)
            const google = book(setup, 3)
            verify(root.all(google, c => c.text === "Google keeps one address book." && c.visible).length === 1)
            verify(!root.named(google, "addressBookName")[0].visible && !root.named(google, "addressBookRemove")[0].visible)
        }

        // A new book: its account and its name sent.
        function test_a_new_book() {
            const setup = make(1000)
            mouseClick(root.named(setup, "addressBookNew")[0])
            wait(30)
            root.named(setup, "addressBookNewHome")[0].currentIndex = 0
            root.named(setup, "addressBookNewName")[0].text = "Professional"
            mouseClick(root.named(setup, "addressBookMake")[0])
            compare(JSON.stringify(mock.calls[0]), JSON.stringify(["make", { account: "cloud", name: "Professional" }]))
        }
    }
}
