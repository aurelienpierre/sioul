// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

// The Texts page on a computer (TextsPage.qml), on a stand-in for Sioul: new
// texts that an exchange brought (the sharing's word, `sharedIn`) shown at
// once while the page stays open, with the conversation open, the draft
// being typed, the row chosen and the place read back to all kept (its top
// between two messages, or its message's copy now the other phone's, too:
// never the conversation's first message); at the thread's end, the new text
// in view; at the list's top, the conversation that just got a text in view;
// another part's word, or the page out of sight, asks nothing. Attaching:
// offered once the phone said what its carrier takes; the files chosen made
// fit off the window's thread, the round last asked alone taken, each shown
// with its words, why the others did not go, the counter asked with their
// sizes, Send as one multimedia message, one removed making the others again,
// all let go with another conversation, made again once for a changed limit.
// What the page lists is tested in sioul-core (texts.rs) and the app
// (texts.rs, attach.rs, tests/attach.rs). Run by tools/qml-test.sh.

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
        // Attaching: offered or not (the phone said what its carrier takes), what was asked to be made fit, the counter's word.
        property bool attach: true
        property var attachAsked: []
        property bool over: false
        property real max: 614400

        signal porchChanged()
        signal sharedIn(string stores)
        signal textsAttached(string attached)

        function textsAttach(json) {
            mock.attachAsked = mock.attachAsked.concat([JSON.parse(json)])
        }

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
                return JSON.stringify({ id: args.id, title: "01 99 00 12 34", messages: mock.thread, drafts: [], can_write: true, group: false, attach: mock.attach, attach_said: mock.attach ? "" : "texts-attach-gate" })
            if (verb === "parts" && args.media)
                return JSON.stringify({ parts: 1, said: "texts-parts-mms", over: mock.over, max: mock.max })
            if (verb === "parts")
                return JSON.stringify({ parts: 1, said: "" })
            if (verb === "send-mms")
                return JSON.stringify({ said: "", shared: true })
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

        // Read back (the review of 16 October 2026, LOW 10): the top message's
        // copy now another phone's, timed to another millisecond within its
        // second, keeps the place; the top message replaced by another (a
        // request's bubble by its text), the next one keeps its place; two
        // messages alike (the same second and words), the second at the top,
        // the second stays there, never the first.
        function test_reading_back_keeps_its_place_through_a_copy_a_replacement_and_twins() {
            const view = threadView()
            const top = () => view.indexAt(1, view.contentY + 1)
            const timed = list => list.map(m => Object.assign({}, m, { at: m.at * 60000 }))
            const show = list => {
                mock.thread = list
                mock.sharedIn("data/texts/log/")
                wait(150)
            }
            const readBack = () => {
                page.atEnd = false
                view.positionViewAtIndex(20, ListView.Beginning)
                wait(50)
                return top()
            }
            show(timed(mock.grow(1, 40)))
            let i = readBack()
            const body = page.messages[i].body
            show(timed(mock.grow(1, 41)).map((m, k) => k === i ? Object.assign({}, m, { id: "other/sms-" + k, at: m.at + 300 }) : m))
            verify(!page.atEnd, "still read back")
            compare(page.messages[top()].body, body, "the same message at the top, timed otherwise within its second")
            i = readBack()
            const next = page.messages[i + 1].body
            const nextOffset = view.itemAtIndex(i + 1).y - view.contentY
            show(timed(mock.grow(1, 42)).map((m, k) => k === i ? Object.assign({}, m, { id: "phone/sms-900", at: m.at + 7000, body: "Message envoyé." }) : m))
            const k = page.messages.findIndex(m => m.body === next)
            verify(k > 0)
            verify(Math.abs((view.itemAtIndex(k).y - view.contentY) - nextOffset) < 2, "the next message keeps its place under the top")
            const twins = timed(mock.grow(1, 42))
            twins[20] = Object.assign({}, twins[19], { id: "phone/sms-twin" })
            show(twins)
            compare(readBack(), 20)
            show(twins.concat([Object.assign({}, mock.message(43), { at: 43 * 60000 })]))
            compare(top(), 20, "the second of the twins, never the first")
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

    TestCase {
        name: "TextsAttach"
        when: windowShown

        function count(verb) {
            return mock.asked.filter(a => a[0] === verb).length
        }
        function lastAsked(verb) {
            const all = mock.asked.filter(a => a[0] === verb)
            return all.length > 0 ? all[all.length - 1][1] : null
        }
        // What texts.rs's `attach` answers for a round, made up: no file of anyone's.
        function made(round, media, said, max) {
            mock.textsAttached(JSON.stringify({ round: round, media: media, said: said || [], limit: "600 kB", max: max || 614400 }))
        }
        function medium(n, kind) {
            return { source: "/stand-in/chosen-" + n, kind: kind || "video", ct: kind === "picture" ? "image/jpeg" : "video/mp4", path: "/stand-in/made-" + n, url: "", size: 1000 * n, said: "texts-attached-" + (kind || "video") }
        }

        function init() {
            mock.conversations = [{ id: "+33199001234", with: ["+33199001234"], title: "01 99 00 12 34", last: "Message 40", when: "09:40", at: 40, group: false, drafts: 0 }]
            mock.thread = mock.grow(1, 3)
            mock.attach = true
            mock.over = false
            mock.max = 614400
            page.visible = true
            page.close()
            page.reload()
            page.select("+33199001234")
            wait(50)
            mock.asked = []
            mock.attachAsked = []
        }

        // Offered only once the phone said what its carrier takes; before, why not, in words.
        function test_attach_is_offered_once_the_phone_said() {
            const button = findChild(page, "textsAttach")
            verify(button.visible, "offered")
            mock.attach = false
            page.reload()
            wait(50)
            verify(!button.visible, "not offered before the phone said")
            page.attachFiles(["file:///stand-in/a.jpg"])
            compare(mock.attachAsked.length, 0, "nothing made")
            compare(page.said, "texts-attach-gate")
        }

        // Made fit off the window's thread: while they are, Send waits; the round
        // last asked alone is taken; each shown with its words, the others said,
        // the counter asked with their sizes.
        function test_files_chosen_are_made_fit_and_shown() {
            const send = findChild(page, "textsSend")
            page.attachFiles(["file:///stand-in/a.jpg"])
            page.attachFiles(["file:///stand-in/a.jpg", "file:///stand-in/b.pdf"])
            compare(mock.attachAsked.length, 2)
            const asked = mock.attachAsked[1]
            compare(asked.id, "+33199001234")
            compare(asked.sources, ["file:///stand-in/a.jpg", "file:///stand-in/b.pdf"], "each file once, in its order")
            verify(page.attaching)
            verify(!send.enabled, "Send waits while they are made")
            made(mock.attachAsked[0].round, [medium(9)], [])
            compare(page.attached.length, 0, "an older round's answer let go")
            made(asked.round, [medium(1, "picture")], ["b.pdf: texts-attach-left-file"])
            verify(!page.attaching)
            compare(page.attached.length, 1)
            compare(page.attachSaid, ["b.pdf: texts-attach-left-file"])
            compare(page.attachSources, ["/stand-in/chosen-1"], "the one refused is not made again")
            const parts = lastAsked("parts")
            compare(parts.media, [{ ct: "image/jpeg", size: 1000 }], "the counter by their sizes")
            compare(parts.id, "+33199001234")
            compare(page.counted.said, "texts-parts-mms")
            verify(send.enabled, "a picture alone goes, no words needed")
            verify(findChild(page, "textsAttached").visible)
        }

        // Send: one multimedia message with the words; once shared, all let go.
        function test_send_goes_as_one_multimedia_message() {
            page.attachFiles(["file:///stand-in/a.mp4"])
            made(mock.attachAsked[0].round, [medium(1), medium(2)])
            const box = findChild(page, "textsDraft")
            box.text = "Voici."
            page.send()
            const sent = lastAsked("send-mms")
            compare(sent.to, "+33199001234")
            compare(sent.words, "Voici.")
            compare(sent.media, [{ ct: "video/mp4", path: "/stand-in/made-1" }, { ct: "video/mp4", path: "/stand-in/made-2" }])
            compare(count("send"), 0, "never a text beside it")
            compare(page.attached.length, 0)
            compare(box.text, "")
            compare(count("unattach"), 0, "their files deleted by texts.rs once shared")
        }

        // One removed: the others made again, sharing the message anew; the last removed, all let go.
        function test_removing_one_makes_the_others_again() {
            page.attachFiles(["file:///stand-in/a.mp4", "file:///stand-in/b.mp4"])
            made(mock.attachAsked[0].round, [medium(1), medium(2)])
            page.unattach(0)
            compare(mock.attachAsked.length, 2)
            compare(mock.attachAsked[1].sources, ["/stand-in/chosen-2"])
            made(mock.attachAsked[1].round, [medium(2)])
            page.unattach(0)
            compare(mock.attachAsked.length, 2, "none left: nothing to make")
            compare(count("unattach"), 1, "their files deleted")
            compare(page.attached.length, 0)
        }

        // Another conversation: all let go, and a late answer of theirs too.
        function test_another_conversation_lets_them_go() {
            page.attachFiles(["file:///stand-in/a.mp4"])
            const round = mock.attachAsked[0].round
            page.select("+33465710042")
            compare(count("unattach"), 1)
            made(round, [medium(1)])
            compare(page.attached.length, 0, "a late answer for the other conversation let go")
        }

        // A limit changed since they were made (Settings ▸ Texts): made again, once.
        function test_a_changed_limit_makes_them_again_once() {
            page.attachFiles(["file:///stand-in/a.jpg"])
            made(mock.attachAsked[0].round, [medium(1, "picture")], [], 614400)
            mock.over = true
            mock.max = 307200
            findChild(page, "textsDraft").text = "x"
            compare(mock.attachAsked.length, 2, "made again for the smaller limit")
            made(mock.attachAsked[1].round, [medium(1, "picture")], [], 307200)
            findChild(page, "textsDraft").text = "xy"
            compare(mock.attachAsked.length, 2, "once")
            verify(!findChild(page, "textsSend").enabled, "still over: Send waits")
        }
    }
}
