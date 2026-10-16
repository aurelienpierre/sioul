// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

// The Texts page on a computer (TextsPage.qml), on a stand-in for Sioul: new
// texts that an exchange brought (the sharing's word, `sharedIn`) shown at
// once while the page stays open, with the conversation open, the draft
// being typed, the row chosen and the place read back to all kept (its top
// between two messages, or its message's copy now the other phone's, too:
// never the conversation's first message); at the thread's end, the new text
// in view; at the list's top, the conversation that just got a text in view;
// another part's word, or the page out of sight, asks nothing. What the page lists is tested in sioul-core
// (texts.rs) and the app (texts.rs). Run by tools/qml-test.sh.

import QtQuick
import QtQuick.Controls.Basic
import QtTest
import "../../qml"

Item {
    id: root

    width: 1100
    height: 700

    Theme {
        id: testTheme
    }

    QtObject {
        id: mock

        // What the page asked, in order: [verb, args].
        property var asked: []
        // Made up: fiction numbers only.
        property var conversations: [{ id: "+33199001234", with: ["+33199001234"], title: "01 99 00 12 34", last: "Message 40", when: "09:40", at: 40, group: false, drafts: 0 }]
        property var thread: []

        signal porchChanged()
        signal sharedIn(string stores)

        function text(key) {
            return key
        }
        function textWith(key, name, value) {
            return key + ":" + value
        }
        function texts(verb, json) {
            const args = JSON.parse(json)
            mock.asked = mock.asked.concat([[verb, args]])
            if (verb === "view")
                return JSON.stringify({ can: true, phone: { id: "p", name: "", shared: "09:58", said: "" }, conversations: mock.conversations, said: "", never: "", empty: "" })
            if (verb === "conversation")
                return JSON.stringify({ id: args.id, title: "01 99 00 12 34", messages: mock.thread, drafts: [], can_write: true, group: false })
            if (verb === "parts")
                return JSON.stringify({ parts: 1, said: "" })
            return "{}"
        }
        function message(n) {
            return { id: "phone/sms-" + n, at: n, when: "09:" + n, day: "Today", time: "09:" + n, direction: n % 2 ? "in" : "out", from: "", body: "Message " + n + "\nwith a second line to make it taller.", picture: false, parts: [], deleted: false, key: "", said: null, to: "" }
        }
        function grow(from, to) {
            const list = []
            for (let n = from; n <= to; ++n)
                list.push(mock.message(n))
            return list
        }
        // A conversation of the list, made up (fiction numbers).
        function conversation(n) {
            const number = "+331990010" + String(n).padStart(2, "0")
            return { id: number, with: [number], title: "01 99 00 10 " + String(n).padStart(2, "0"), last: "Message " + n, when: "09:" + n, at: n, group: false, drafts: 0 }
        }
    }

    QtObject {
        id: stand

        property bool compact: false
    }

    TextsPage {
        id: page

        anchors.fill: parent
        sioul: mock
        theme: testTheme
        window: stand
    }

    TestCase {
        name: "TextsRefresh"
        when: windowShown

        function draftBox() {
            return findChild(page, "textsDraft")
        }
        function threadView() {
            return findChild(page, "textsThread")
        }
        function count(verb) {
            return mock.asked.filter(a => a[0] === verb).length
        }

        function init() {
            mock.conversations = [{ id: "+33199001234", with: ["+33199001234"], title: "01 99 00 12 34", last: "Message 40", when: "09:40", at: 40, group: false, drafts: 0 }]
            mock.thread = mock.grow(1, 40)
            page.visible = true
            page.close()
            page.reload()
            page.select("+33199001234")
            wait(100)
            mock.asked = []
        }

        // At its end: the new text comes into view, the draft stays.
        function test_a_text_that_comes_shows_at_once() {
            const box = draftBox()
            verify(box !== undefined)
            box.text = "Half typed"
            mock.thread = mock.grow(1, 41)
            mock.conversations = [{ id: "+33199001234", with: ["+33199001234"], title: "01 99 00 12 34", last: "Message 41", when: "09:41", at: 41, group: false, drafts: 0 },
                                  { id: "+33465710042", with: ["+33465710042"], title: "04 65 71 00 42", last: "Votre colis", when: "09:41", at: 41, group: false, drafts: 0 }]
            mock.sharedIn("data/texts/log/\ndata/time/")
            wait(100)
            compare(page.rows.length, 2, "the new conversation listed")
            compare(page.messages.length, 41, "the new text in the thread")
            compare(page.openId, "+33199001234", "the conversation open stays")
            compare(box.text, "Half typed", "the draft stays")
            const view = threadView()
            tryVerify(() => view.atYEnd, 1000, "at its end, the new text in view")
        }

        // Read back: the message at the top of the view stays there.
        function test_reading_back_keeps_its_place() {
            page.scrollBack()
            page.scrollBack()
            wait(100)
            const view = threadView()
            const before = view.indexAt(1, view.contentY + 1)
            verify(before >= 0 && before < 39, "read back: " + before)
            const id = page.messages[before].id
            mock.thread = mock.grow(1, 42)
            mock.sharedIn("data/texts/log/")
            wait(150)
            compare(page.messages.length, 42)
            verify(!page.atEnd, "still read back")
            compare(page.messages[view.indexAt(1, view.contentY + 1)].id, id, "the same message at the top")
        }

        // Read back with the top of the view between two messages (the
        // thread's spacing): the same place, never the first message.
        function test_reading_back_from_between_two_messages_keeps_its_place() {
            const view = threadView()
            page.atEnd = false
            view.positionViewAtIndex(20, ListView.Beginning)
            wait(50)
            const item = view.itemAtIndex(20)
            view.contentY = item.y + item.height + 2
            wait(50)
            verify(view.indexAt(1, view.contentY + 1) < 0, "the top between two messages")
            // The message just under the top, and how far under it (the view's
            // own coordinates change as a new model is laid out: never compared).
            const under = (view) => {
                const i = view.indexAt(1, view.contentY + 8)
                return { id: page.messages[i].id, offset: view.itemAtIndex(i).y - view.contentY }
            }
            const before = under(view)
            mock.thread = mock.grow(1, 42)
            mock.sharedIn("data/texts/log/")
            wait(150)
            verify(!page.atEnd, "still read back")
            const after = under(view)
            compare(after.id, before.id, "the same message under the top")
            verify(Math.abs(after.offset - before.offset) < 2, "the same place: " + after.offset + " px under the top for " + before.offset)
        }

        // The copy shown of each message now the other phone's (a restore's
        // copies came): the message at the top found by its time and words.
        function test_the_top_message_keeps_its_place_when_its_copy_changes() {
            const view = threadView()
            page.atEnd = false
            view.positionViewAtIndex(20, ListView.Beginning)
            wait(50)
            const at = page.messages[view.indexAt(1, view.contentY + 1)].at
            mock.thread = mock.grow(1, 42).map(m => Object.assign({}, m, { id: "other/" + m.id.split("/")[1] }))
            mock.sharedIn("data/texts/log/")
            wait(150)
            compare(page.messages[view.indexAt(1, view.contentY + 1)].at, at, "the same message at the top")
            verify(view.indexAt(1, view.contentY + 1) > 0, "never the first message")
        }

        // At its top, the list stays at its top: the conversation that just
        // got a text shows first; scrolled down, its top row stays.
        function test_the_list_at_its_top_shows_the_conversation_that_got_a_text() {
            page.close()
            const many = []
            for (let n = 0; n < 30; ++n)
                many.push(mock.conversation(n))
            mock.conversations = many
            page.reload()
            wait(100)
            const list = findChild(page, "textsList")
            list.positionViewAtBeginning()
            wait(50)
            mock.conversations = [many[12]].concat(many.slice(0, 12), many.slice(13))
            mock.sharedIn("data/texts/log/")
            wait(150)
            compare(page.rows[list.indexAt(1, list.contentY + 1)].id, many[12].id, "the conversation that got a text, in view")
            list.positionViewAtIndex(10, ListView.Beginning)
            wait(50)
            const kept = page.rows[list.indexAt(1, list.contentY + 1)].id
            mock.conversations = [many[25]].concat(mock.conversations.filter(c => c.id !== many[25].id))
            mock.sharedIn("data/texts/log/")
            wait(150)
            compare(page.rows[list.indexAt(1, list.contentY + 1)].id, kept, "scrolled down: its top row kept")
        }

        // Another part's word, or the page out of sight: nothing asked.
        function test_other_words_ask_nothing() {
            mock.sharedIn("data/time/\nfiles/notes/")
            wait(50)
            compare(count("view") + count("conversation"), 0)
            page.visible = false
            mock.sharedIn("data/texts/log/")
            wait(50)
            compare(count("view") + count("conversation"), 0, "out of sight")
            page.visible = true
            wait(50)
            verify(count("view") > 0, "shown again: read")
        }
    }
}
