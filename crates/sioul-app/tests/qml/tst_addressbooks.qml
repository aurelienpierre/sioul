// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

// Contacts ⚙ ▸ Address books (AddressBooksSetup.qml, wave D) on a stand-in
// for Sioul, at a computer's width and at a phone's 412 points: each book's
// line says where it is kept, how many, whose it is, whether it can be
// written; yours opens to its sharing (asked of its server then), is renamed,
// merged into another after its question (count, target, who sees it),
// removed after the question of where its contacts go; the book chosen and
// the name typed survive the lines drawn again at a sync (review F4);
// someone else's offers only "Stop showing"; Google's says what it allows
// and offers nothing else; "New address book…" sends its account and name;
// the batches of the last thirty days are listed, each undone after its
// question (review F2); "Its groups decide who may reach me" on for a book
// only you write, off with its sentence for one someone else can write,
// and sent when switched; "Merge into…", "Export…" and "Remove…" in one row,
// within the edge on a phone, the merge's choices in a row under it.
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
        property string contactsBatches: ""

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
        function mergeQuestion(from, into) {
            mock.calls = mock.calls.concat([["question", from, into]])
            return JSON.stringify({ sentence: "Move the 840 contacts of “Contacts” into “" + into + "”, then remove “Contacts”?", losses: "", shared: "“Professional” is shared with office: they will see what goes into it.", reach: ["Who may reach you stays as its groups say."], cannot: [] })
        }
        function readContactsBatches() { mock.calls = mock.calls.concat([["batches"]]) }
        function undoContactsBatch(id) { mock.calls = mock.calls.concat([["undo", id]]); return "" }
        function setBookGroups(place, on) { mock.calls = mock.calls.concat([["groups", place, on]]); return "" }
    }

    readonly property var line: ({ count: 0, counted: "0 contacts", waiting: "", fixed: "", groups: true, groups_note: "", can_rename: true, can_remove: true, can_share: true, can_leave: false, writable: true, access: "Can be written", whose: "Yours" })
    readonly property var view: ({
        books: [
            Object.assign({}, root.line, { place: "cloud/contacts", account: "cloud", name: "Contacts", kept: "example.org", count: 840, counted: "840 contacts" }),
            Object.assign({}, root.line, { place: "cloud/pro", account: "cloud", name: "Professional", kept: "example.org", waiting: "Kept: its server holds contacts that were added there since; they are here again." }),
            Object.assign({}, root.line, { place: "cloud/team_shared_by_sam", account: "cloud", name: "Team", kept: "example.org", whose: "Shared with you by sam", access: "Read only", writable: false, can_rename: false, can_remove: false, can_share: false, can_leave: true, groups: false, groups_note: "Sam can write this address book, so its groups no longer decide who may reach you." }),
            Object.assign({}, root.line, { place: "google/default", account: "google", name: "Address book", kept: "Google", can_rename: false, can_remove: false, can_share: false, fixed: "Google keeps one address book." })
        ],
        homes: [{ id: "cloud", name: "you@example.org" }, { id: "local", name: "on this device alone" }]
    })

    Component {
        id: batchesComponent

        ContactBatches {
            sioul: mock
            theme: testTheme
        }
    }

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

    // The calls of one kind, in order.
    function calls(kind) {
        return mock.calls.filter(c => c[0] === kind).map(c => JSON.stringify(c))
    }

    TestCase {
        id: test

        name: "AddressBooksSetup"
        when: windowShown

        function init() {
            mock.calls = []
            mock.contactsBatches = ""
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
                const kept = root.named(book(setup, 1), "addressBookWaiting")
                verify(kept.length === 1 && kept[0].visible && kept[0].text.startsWith("Kept:"), "a book kept by its server says why, closed")
                compare(root.named(book(setup, 0), "addressBookWaiting").length, 0)
                const held = root.named(book(setup, 2), "addressBookWaiting")
                verify(held.length === 1 && held[0].visible && held[0].text.startsWith("Sam can write"), "its groups no longer counting, said closed")
                for (const item of root.named(setup, "addressBookLine")) {
                    const x = item.mapToItem(setup, 0, 0).x
                    verify(x >= 0 && x + item.width <= width + 1, "within " + width + ": " + x + " + " + item.width)
                }
            }
        }

        // Yours: its sharing asked when it opens, renamed, shared, merged, removed after the question.
        function test_yours_renamed_shared_merged_removed() {
            const setup = make(1000)
            mock.calls = []
            setup.toggle("cloud/contacts")
            wait(30)
            compare(JSON.stringify(mock.calls[0]), JSON.stringify(["sharing", "cloud/contacts"]))
            const contacts = book(setup, 0)
            const name = root.named(contacts, "addressBookName")[0]
            name.text = "Personal"
            mouseClick(root.named(contacts, "addressBookRename")[0])
            compare(JSON.stringify(mock.calls[1]), JSON.stringify(["rename", "cloud/contacts", "Personal"]))
            // Its contacts as one vCard file; an empty book has nothing to export.
            verify(root.named(contacts, "addressBookExport")[0].visible)
            setup.toggle("cloud/pro")
            wait(30)
            verify(!root.named(book(setup, 1), "addressBookExport")[0].visible)
            mock.calls = mock.calls.filter(c => c[1] !== "cloud/pro")
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
            wait(30)
            // Asked first: the count and the target, who sees it (its sharing asked of its server).
            compare(root.calls("question"), [JSON.stringify(["question", "cloud/contacts", "cloud/pro"])])
            compare(root.calls("remove"), [], "nothing merged before the question is answered")
            verify(root.calls("sharing").indexOf(JSON.stringify(["sharing", "cloud/pro"])) >= 0, "who sees the target asked of its server")
            const asked = root.named(contacts, "addressBookMergeQuestion").map(l => l.text)
            compare(asked.length, 3)
            verify(asked[1].indexOf("shared with office") >= 0, asked[1])
            compare(asked[2], "Who may reach you stays as its groups say.", "what the merge does to who may reach you")
            mouseClick(root.named(contacts, "addressBookMergeConfirm")[0])
            compare(root.calls("remove"), [JSON.stringify(["remove", "cloud/contacts", "move:cloud/pro"])])
            // Removed: where its contacts go, asked first, then the same question.
            mouseClick(root.named(contacts, "addressBookRemove")[0])
            wait(30)
            compare(root.named(contacts, "addressBookRemoval")[0].text, "“Contacts” holds 840 contacts. Where do they go?")
            compare(root.calls("remove").length, 1, "nothing removed yet")
            const moveTo = root.named(contacts, "addressBookRemoveInto")[0]
            moveTo.activated(1)
            wait(30)
            mouseClick(root.named(contacts, "addressBookRemoveMove")[0])
            wait(30)
            compare(root.calls("question")[1], JSON.stringify(["question", "cloud/contacts", "local/home"]))
            mouseClick(root.named(contacts, "addressBookMergeConfirm")[0])
            compare(root.calls("remove")[1], JSON.stringify(["remove", "cloud/contacts", "move:local/home"]))
        }

        // A book's actions in one row, at a computer's width and a phone's (the
        // Settings column, 392 points), each within the edge; merging, its choices
        // in a row under them; the line closed and opened again, the row is back.
        function test_the_actions_in_one_row() {
            for (const width of [1000, 392]) {
                const setup = make(width)
                setup.toggle("cloud/contacts")
                wait(30)
                const contacts = book(setup, 0)
                const actions = ["addressBookMerge", "addressBookExport", "addressBookRemove"].map(n => root.named(contacts, n)[0])
                verify(actions.every(a => a.visible), "the three offered")
                const ys = actions.map(a => a.mapToItem(setup, 0, 0).y)
                compare([ys[1], ys[2]], [ys[0], ys[0]], "one row at " + width)
                for (const item of actions) {
                    const x = item.mapToItem(setup, 0, 0).x
                    verify(x >= 0 && x + item.width <= width + 1, item.objectName + " within " + width + ": " + x + " + " + item.width)
                }
                mouseClick(actions[0])
                wait(30)
                verify(!actions[0].visible && actions[1].visible && actions[2].visible, "merging: Merge into… gives way to its choices")
                const into = root.named(contacts, "addressBookMergeInto")[0]
                verify(into.visible && into.mapToItem(setup, 0, 0).y > actions[1].mapToItem(setup, 0, 0).y, "the choices under the row")
                setup.merging = ""
                setup.toggle("cloud/contacts")
                wait(30)
                setup.toggle("cloud/contacts")
                wait(30)
                const again = book(setup, 0)
                verify(root.named(again, "addressBookActions")[0].visible && root.named(again, "addressBookMerge")[0].visible, "closed and opened again: the row is back")
            }
        }

        // The book chosen in "Merge into" and a name being typed stay when a
        // sync draws the lines again (review F4).
        function test_choices_survive_a_redraw() {
            const setup = make(1000)
            setup.toggle("cloud/contacts")
            wait(30)
            mouseClick(root.named(book(setup, 0), "addressBookMerge")[0])
            wait(30)
            root.named(book(setup, 0), "addressBookMergeInto")[0].activated(1)
            const name = root.named(book(setup, 0), "addressBookName")[0]
            name.forceActiveFocus()
            name.selectAll()
            keyClick(Qt.Key_P)
            keyClick(Qt.Key_R)
            keyClick(Qt.Key_O)
            mock.contactsChanged()
            wait(50)
            const redrawn = book(setup, 0)
            compare(root.named(redrawn, "addressBookMergeInto")[0].currentIndex, 1, "the second book still chosen")
            compare(root.named(redrawn, "addressBookName")[0].text, "pro", "the name typed still there")
            mouseClick(root.named(redrawn, "addressBookMergeGo")[0])
            compare(root.calls("question"), [JSON.stringify(["question", "cloud/contacts", "google/default"])])
        }

        // The batches of the last thirty days: each line, why one cannot be undone, Undo after its question.
        function test_batches_listed_and_undone() {
            mock.contactsBatches = JSON.stringify([
                { id: "1790000000-move", line: "18 November 2026 · 118 contacts moved.", partly: "", why_not: "", ask: "Put back the 118 contacts?" },
                { id: "1789000000-merge", line: "17 November 2026 · 40 contacts moved.", partly: "Stopped after 40 of 50.", why_not: "Cannot be undone: its address book is gone.", ask: "" }
            ])
            for (const width of [1000, 412]) {
                const setup = test.createTemporaryObject(batchesComponent, root, { width: width })
                test.waitForRendering(setup)
                verify(root.calls("batches").length >= 1, "read when shown")
                const rows = root.named(setup, "contactsBatch")
                compare(rows.length, 2)
                const undo = root.named(rows[0], "contactsBatchUndo")[0]
                verify(undo.visible && !root.named(rows[1], "contactsBatchUndo")[0].visible, "the second cannot be undone, said")
                verify(root.all(rows[1], c => c.text === "Cannot be undone: its address book is gone." && c.visible).length === 1)
                mouseClick(undo)
                wait(30)
                compare(root.calls("undo"), [], "asked first")
                mouseClick(root.named(rows[0], "contactsBatchUndoGo")[0])
                compare(root.calls("undo"), [JSON.stringify(["undo", "1790000000-move"])])
                mock.calls = []
            }
        }

        // Someone else's: Stop showing alone; Google's: what it allows, nothing else.
        function test_someone_elses_and_google_s() {
            const setup = make(412)
            mock.calls = []
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
            compare(root.calls("remove"), [JSON.stringify(["remove", "cloud/team_shared_by_sam", "leave"])])
            setup.toggle("google/default")
            wait(30)
            const google = book(setup, 3)
            verify(root.all(google, c => c.text === "Google keeps one address book." && c.visible).length === 1)
            verify(!root.named(google, "addressBookName")[0].visible && !root.named(google, "addressBookRemove")[0].visible)
        }

        // "Its groups decide who may reach me": on for yours, off for one
        // someone else can write; switched, sent with the book's place.
        function test_groups_switch() {
            const setup = make(412)
            setup.toggle("cloud/contacts")
            setup.toggle("cloud/team_shared_by_sam")
            wait(30)
            const yours = root.named(book(setup, 0), "addressBookGroups")[0]
            const theirs = root.named(book(setup, 2), "addressBookGroups")[0]
            verify(yours.visible && yours.checked && theirs.visible && !theirs.checked)
            mouseClick(theirs)
            compare(root.calls("groups"), [JSON.stringify(["groups", "cloud/team_shared_by_sam", true])])
        }

        // A new book: its account and its name sent.
        function test_a_new_book() {
            const setup = make(1000)
            mouseClick(root.named(setup, "addressBookNew")[0])
            wait(30)
            root.named(setup, "addressBookNewHome")[0].currentIndex = 0
            root.named(setup, "addressBookNewName")[0].text = "Professional"
            mouseClick(root.named(setup, "addressBookMake")[0])
            compare(root.calls("make"), [JSON.stringify(["make", { account: "cloud", name: "Professional" }])])
        }
    }
}
