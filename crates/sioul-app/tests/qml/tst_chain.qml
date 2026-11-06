// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

// The chain of ties (ChainPage.qml) on a stand-in for Sioul, whose chains
// are made up: the drawing's and the outline's things in one order, the
// keys of the tree view pattern (↓ ↑ → ← Home, typing), "+" asked with the
// thing pressed and nothing already shown moving, "−" giving the picture
// back, a card opened and the same picture after it. Run by tools/qml-test.sh.

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

    // A chain made up, by what is opened: the project, its task and the task's message, its note folded.
    function thing(uri, kind, family, title, column, row, parent, more, opened) {
        return { uri: uri, kind: kind, family: family, kind_words: kind, title: title, detail: "", date: "", when: null, key: uri, found: true, program: false,
                 column: column, row: row, parent: parent, how: parent === null ? "" : "in-project", how_words: parent === null ? "" : "In the project", score: 0.6,
                 more: more, more_words: more > 0 ? more + " more" : "", opened: opened, collection: kind === "project", count: 0, busy: false, set_aside: false,
                 health: false, sessions: 0, chain: "", name: kind + ": " + title }
    }
    function chainOf(opened) {
        const plus = opened.indexOf("sioul:note/plan.md") >= 0
        const things = [
            root.thing("sioul:project/flat", "project", "projects", "Flat", 0, 0, null, 2, 0),
            root.thing("sioul:task/rent", "task", "time", "Pay the rent", 1, 0, 0, 0, 0),
            root.thing("mid:rent@example.org", "mail", "messages", "Rent for November", 2, 0, 1, 0, 0),
            root.thing("sioul:note/plan.md", "note", "records", "Plan", 1, 1, 0, plus ? 0 : 2, plus ? 1 : 0)
        ]
        if (plus) {
            things.push(root.thing("sioul:note/a.md", "note", "records", "Alpha", 2, 1, 3, 0, 0))
            things.push(root.thing("sioul:note/b.md", "note", "records", "Beta", 2, 2, 3, 0, 0))
        }
        return JSON.stringify({ entry: "sioul:project/flat", things: things, cross: [], health: false, columns: 3, rows: plus ? 3 : 2 })
    }

    QtObject {
        id: mock

        signal chainReady(string uri, string opened, string json)
        signal linksChanged
        signal moneyChanged
        signal papersChanged
        signal lettersChanged
        signal personsChanged

        property var asked: []
        property var untied: []

        function askChain(uri, opened, phone) {
            mock.asked = mock.asked.concat([opened])
            Qt.callLater(() => mock.chainReady(uri, opened, root.chainOf(JSON.parse(opened))))
        }
        function text(id) {
            return id
        }
        function textWith(id, name, value) {
            return id + ":" + value
        }
        function unlinkThings(a, b) {
            mock.untied = mock.untied.concat([[a, b]])
            return ""
        }
    }

    QtObject {
        id: shell

        property var menuAt: null
        property var cards: []
        property bool closed: false

        function openThing(item, fromCard) {
            shell.cards = shell.cards.concat([item.uri])
        }
        function closeChain() {
            shell.closed = true
        }
    }

    ChainPage {
        id: page

        anchors.fill: parent
        sioul: mock
        theme: testTheme
        window: shell
    }

    TestCase {
        name: "Chain"
        when: windowShown

        function init() {
            mock.asked = []
            shell.cards = []
            page.outlineShown = false
            page.show("sioul:project/flat")
            tryVerify(() => page.view !== null && page.view.things.length === 4)
        }

        function uris() {
            const out = []
            for (let i = 0; i < page.view.things.length; i++)
                out.push(page.view.things[i].uri)
            return out
        }

        // The drawing and the outline: the same things, in one order, which the keys follow.
        function test_one_order_for_both_views() {
            compare(uris(), ["sioul:project/flat", "sioul:task/rent", "mid:rent@example.org", "sioul:note/plan.md"])
            const outline = findChild(page, "chainOutlineView")
            page.outlineShown = true
            tryCompare(outline, "count", 4)
            for (let i = 0; i < 4; i++)
                compare(outline.model.get(i).uri, uris()[i])
            page.current = 2
            page.outlineShown = false
            compare(page.current, 2, "the keys stay on the same thing")
        }

        // ↓ ↑ in the outline's order, → into a thing then "+" where something is left out,
        // ← back up, Home to the entry point, typing to a title.
        function test_keys_of_a_tree() {
            page.forceActiveFocus()
            compare(page.current, 0)
            keyClick(Qt.Key_Down)
            compare(page.current, 1)
            keyClick(Qt.Key_Right)
            compare(page.current, 2, "into the task: its message")
            keyClick(Qt.Key_Left)
            compare(page.current, 1, "back to the task")
            keyClick(Qt.Key_Left)
            compare(page.current, 0, "back to the entry point")
            keyClick(Qt.Key_End)
            compare(page.current, 3)
            keyClick(Qt.Key_Home)
            compare(page.current, 0)
            keyClick("r")
            compare(page.current, 2, "typing finds Rent for November")
            keyClick(Qt.Key_Home)
            keyClick(Qt.Key_End)
            keyClick(Qt.Key_Right)
            compare(mock.asked[mock.asked.length - 1], JSON.stringify(["sioul:note/plan.md"]), "→ on a folded thing presses its +")
            tryVerify(() => page.view.things.length === 6)
            compare(page.current, 3, "still on the note")
            keyClick(Qt.Key_Left)
            compare(mock.asked[mock.asked.length - 1], "[]", "← on an opened thing folds it")
            tryVerify(() => page.view.things.length === 4)
        }

        // "+": the thing pressed opens under itself; what was shown keeps its column, its
        // row and its order; "−" gives the earlier picture back.
        function test_plus_moves_nothing() {
            const before = page.view.things.map(t => [t.uri, t.column, t.row])
            page.plus(3)
            tryVerify(() => page.view.things.length === 6)
            const after = page.view.things.map(t => [t.uri, t.column, t.row])
            for (let i = 0; i < before.length; i++)
                compare(after[i], before[i], before[i][0])
            page.minus(3)
            tryVerify(() => page.view.things.length === 4)
            compare(page.view.things.map(t => [t.uri, t.column, t.row]), before)
        }

        // A card opened from the chain, then Back: the same picture, the keys on the same thing.
        function test_back_from_a_card_keeps_the_picture() {
            page.plus(3)
            tryVerify(() => page.view.things.length === 6)
            const shown = JSON.stringify(page.view)
            page.openAt(4)
            compare(shell.cards, ["sioul:note/a.md"])
            compare(page.current, 4)
            compare(JSON.stringify(page.view), shown, "nothing read again")
            page.back()
            verify(shell.closed, "Back: where it was opened")
        }
    }
}
