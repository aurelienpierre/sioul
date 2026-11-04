// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

// The phone's messages on a computer's Porch (PhoneMessagesSection.qml) and
// the phone's tab for them (PhoneMessagesSetup.qml), on a stand-in for
// Sioul. What travels and when a line shows are tested in sioul-core
// (phonemsgs.rs, notices.rs) and sioul-sync (share.rs); here the window's
// side: each line a row (NoticeRow.qml), who, the newest words and the
// earlier under it, Text back only for a text from a number, Dismiss and
// Mark read handed to the Porch's words (their ten seconds to undo are the
// Porch's: tst_porchsections.qml), the part's switch and an app's choice
// handed to Rust; a reply: Reply first, its box and Send (through the
// notification's own Reply, or as a text of Sioul's own), your reply under
// their messages with Take back while it waits, "Reply on your phone" once
// it went, Mark as read in the line's menu after its warning, the phone's
// "Answer from your computers". Run by tools/qml-test.sh.

import QtQuick
import QtQuick.Controls.Basic
import QtTest
import "../../qml"

Item {
    id: root

    width: 900
    height: 1200

    Theme {
        id: testTheme
    }

    QtObject {
        id: mock

        // What the window asked, in order: [verb, args].
        property var asked: []
        property var seen: []
        property bool partOn: false
        property string pharmacy: "off"
        property bool answers: true
        property var flags: ({})
        // Made up: no real person, fiction numbers only.
        property var lines: []
        // Texts not read yet (texts.rs, `notices`): their phone notifications folded into them in Rust.
        property var notices: []
        property string openedTexts: ""
        function textArgs(key, json) {
            const a = JSON.parse(json)
            return key + ": " + a.sent + ", " + a.late
        }
        // A row's notice, as Rust gives it with each line (`notices::Notice`).
        function noticed(line, who, what, when) {
            return Object.assign({ notice: { id: "phone:1b4e:" + line.ids[0], kind: "phone", who: who, what: what, when: when, readable: true, state: { mark: "new", at: 0, device: "" } } }, line)
        }
        readonly property var plain: [
            mock.noticed({ ids: ["1-a"], head: "While you slept, Dr Elena Varga wrote:", said: [{ time: "22:58", from: "", text: "Bonsoir." }, { time: "23:10", from: "", text: "Could you call me back tomorrow morning?" }], note: "", number: "04 65 71 47 70", dial: "+33465714770", known: true, texted: true, code: false },
                         "Dr Elena Varga", "Could you call me back tomorrow morning?", "Tue 3 Nov 23:10"),
            mock.noticed({ ids: ["2-b"], head: "A code came from My Bank at 11:05. It stays on your phone.", said: [], note: "", number: "", dial: "", known: false, texted: false, code: true },
                         "My Bank", "phonemsgs-code-row", "Wed 4 Nov 11:05"),
            mock.noticed({ ids: ["3-c"], head: "In your leisure time, Sam wrote on Signal:", said: [{ time: "13:02", from: "", text: "" }], note: "Their words stay on your phone: Signal sends who and when only.", number: "", dial: "", known: false, texted: false, code: false, app: "Signal" },
                         "Sam", "phonemsgs-no-words", "Wed 4 Nov 13:02")
        ]
        // The same with what each offers a reply from this computer (replies.rs's `Answer`).
        function answer(line, offered, texts, mine) {
            return { phone: "1b4e28ba-2fa1-11d2-883f-0016d3cca427", notice: "0123456789abcdef", app: "org.thoughtcrime.securesms", label: "Signal", line: line, offered: offered, read: offered && !texts, texts: texts,
                     placeholder: "Reply to Alice on Signal…", note: "Replying marks the conversation read in Signal.", read_item: "Mark as read in Signal", read_warning: "Signal may tell them that you read it.",
                     on_phone: "Reply on your phone.", limit: texts ? 1600 : 2000, mine: mine }
        }
        readonly property var answered: [
            mock.noticed({ ids: ["4-d"], head: "Alice wrote on Signal:", said: [{ time: "14:01", from: "", text: "Tu passes ?" }], note: "", number: "", dial: "", known: false, texted: false, code: false, who: "Alice",
              answer: mock.answer("4-d", true, false, [{ key: "k0", time: "14:02", text: "J'arrive.", said: "Waiting for your phone.", waiting: true, again: false, texts: false },
                                                         { key: "k9", time: "14:03", text: "Oui, j'arrive.", said: "Not sent: the same words are already on their way to this conversation.", waiting: false, again: true, texts: false }]) }, "Alice", "Tu passes ?", "14:01"),
            mock.noticed({ ids: ["5-e"], head: "Bob wrote on Signal:", said: [{ time: "14:03", from: "", text: "Merci." }], note: "", number: "", dial: "", known: false, texted: false, code: false, who: "Bob", answer: mock.answer("5-e", false, false, []) }, "Bob", "Merci.", "14:03"),
            mock.noticed({ ids: ["6-f"], head: "Dr Elena Varga wrote:", said: [{ time: "14:04", from: "", text: "Rappelez-moi." }], note: "", number: "04 65 71 47 70", dial: "+33465714770", known: true, texted: true, code: false, who: "Dr Elena Varga", answer: mock.answer("6-f", true, true, []) }, "Dr Elena Varga", "Rappelez-moi.", "14:04")
        ]

        signal porchChanged()

        function text(key) {
            return ({ "notice-dismiss": "Dismiss", "notice-read": "Mark read", "calls-text-back": "Text back", "calls-call-back": "Call back", "ui-undo": "Undo", "phonemsgs-title": "From your phone", "replies-you": "You" })[key] || key
        }
        function viewFlag(flag) {
            return mock.flags[flag] === true
        }
        function setViewFlag(flag, on) {
            const flags = Object.assign({}, mock.flags)
            flags[flag] = on
            mock.flags = flags
        }
        function texts(verb, json) {
            mock.asked = mock.asked.concat([["texts:" + verb, JSON.parse(json)]])
            return JSON.stringify({ said: "", key: "t1", shared: true })
        }
        function textWith(key, name, value) {
            return key + ":" + value
        }
        function phoneMessages(verb, json) {
            const args = JSON.parse(json)
            mock.asked = mock.asked.concat([[verb, args]])
            if (verb === "view")
                return JSON.stringify({ lines: mock.lines, links: { tel: true, sms: true }, notices: mock.notices })
            if (verb === "reply")
                return JSON.stringify(args.words === "refused" ? { said: "Not sent: it has no words." } : { key: "k1", said: "", shared: true })
            if (verb === "take-back") {
                // As Rust says it at once (replies.rs, `state_of`): taking it back, no Take back again.
                mock.lines = mock.lines.map(l => !l.answer ? l : Object.assign({}, l, { answer: Object.assign({}, l.answer, {
                    mine: l.answer.mine.map(m => m.key !== args.key ? m : Object.assign({}, m, { said: "Taking it back: your phone will not send it, unless it has taken it already.", waiting: false })) }) }))
                return JSON.stringify({ said: "", shared: true, at: 0 })
            }
            if (verb === "set" && args.key === "answers.foundation.e.message")
                mock.answers = args.value === true
            if (verb === "part")
                mock.partOn = args.on
            if (verb === "set")
                mock.pharmacy = args.value
            return JSON.stringify({
                android: true,
                sharing: true,
                said: "",
                part: { key: "part", kind: "bool", label: "Messages from your phone", help: "", value: mock.partOn, choices: [] },
                apps: [{ key: "app.foundation.e.message", kind: "choice", label: "Message", help: "The phone's SMS app", value: "words", choices: [{ value: "off", label: "Not on your computers" }, { value: "who", label: "Who and when" }, { value: "words", label: "Who, when and the words" }] },
                       { key: "answers.foundation.e.message", kind: "bool", label: "Answer from your computers", help: "", value: mock.answers, choices: [] }]
            })
        }
    }

    // The Porch, as the section sees it: the rows still new, and its words on them.
    QtObject {
        id: porchStand

        // Rows on one line: a computer's wide Porch.
        property bool narrowRows: false
        property var said: []
        property var gone: ({})

        function newOnes(rows) {
            return (rows || []).filter(r => porchStand.gone[r.notice.id] !== true)
        }
        function say(verb, row) {
            porchStand.said = porchStand.said.concat([[verb, row.ids, row.targets || []]])
            const next = Object.assign({}, porchStand.gone)
            next[row.notice.id] = true
            porchStand.gone = next
        }
    }

    QtObject {
        id: stand

        property var now: 0
        property bool compact: false

        function openTexts(id) {
            mock.openedTexts = id
            mock.notices = []
        }
    }

    Column {
        width: root.width - 32
        x: 16
        spacing: 12

        PhoneMessagesSection {
            id: section

            width: parent.width
            sioul: mock
            theme: testTheme
            window: stand
            porch: porchStand
        }
        PhoneMessagesSetup {
            id: setup

            width: parent.width
            sioul: mock
            theme: testTheme
        }
    }

    TestCase {
        name: "PhoneMessages"
        when: windowShown

        function walk(item, test, found) {
            if (!item)
                return found
            if (test(item))
                found.push(item)
            const children = item.children || []
            for (let i = 0; i < children.length; ++i)
                walk(children[i], test, found)
            if (item.contentItem && item.contentItem !== item && children.indexOf(item.contentItem) < 0)
                walk(item.contentItem, test, found)
            return found
        }
        function labels(under, text) {
            return walk(under, i => i.visible && i.text === text && i.clicked === undefined, [])
        }
        function buttons(under, text) {
            return walk(under, i => i.visible && i.text === text && i.clicked !== undefined, [])
        }

        function init() {
            mock.notices = []
            mock.openedTexts = ""
            mock.asked = []
            porchStand.said = []
            porchStand.gone = {}
            mock.flags = {}
            mock.lines = mock.plain
            section.replyingTo = ""
            section.drafts = {}
            section.reload()
            wait(50)
        }
        function replying(lines) {
            mock.lines = lines
            section.reload()
            wait(50)
        }
        function box() {
            return walk(section, i => i.visible && i.objectName === "replyWords", [])[0]
        }
        function last(verb) {
            const all = mock.asked.filter(a => a[0] === verb)
            return all.length > 0 ? all[all.length - 1][1] : null
        }
        function popups(text) {
            return walk(root.Window.window ? root.Window.window.contentItem : root, i => i.visible && i.text === text && (i.clicked !== undefined || i.triggered !== undefined), [])
        }

        // A text the Texts page holds: one row, opening its conversation there;
        // Mark read hands its conversation to the Porch's words (23 October 2026).
        function test_a_text_is_one_row_opening_texts() {
            mock.lines = []
            mock.notices = [{ conversation: "+33465714770", ids: ["text:p1:7:sms-1@1"], targets: ["+33465714770"], notice: { id: "text:p1:7:sms-1@1", kind: "text", who: "Dr Elena Varga", what: "Rappelez-moi.", when: "14:04", readable: true, state: { mark: "new" } }, count: 1 }]
            section.reload()
            wait(50)
            verify(section.visible, "shown for the text alone")
            const rows = walk(section, i => i.visible && i.objectName === "textNotice", [])
            compare(rows.length, 1)
            compare(labels(rows[0], "Rappelez-moi.").length, 1, "its first words")
            mouseClick(rows[0], 4, 4)
            compare(mock.openedTexts, "+33465714770")
            const read = walk(rows[0], i => i.visible && i.objectName === "readButton", [])
            compare(read.length, 1, "Mark read beside it")
            mouseClick(read[0])
            compare(JSON.stringify(porchStand.said), JSON.stringify([["read", ["text:p1:7:sms-1@1"], ["+33465714770"]]]))
            wait(50)
            compare(walk(section, i => i.visible && i.objectName === "textNotice", []).length, 0, "read: gone")
        }

        // A text that reached the phone 5 minutes or more after it was sent: its
        // row comes on arrival, and says when it was sent (27 October 2026).
        function test_a_late_text_s_row_says_when_it_was_sent() {
            mock.lines = []
            mock.notices = [
                { conversation: "+33465714770", ids: ["t1"], notice: { id: "t1", kind: "text", who: "Dr Elena Varga", what: "À demain, 10 h.", when: "01:55", readable: true, state: { mark: "new" } }, sent: "22:10", late: "3 h 45" },
                { conversation: "+33199001234", ids: ["t2"], notice: { id: "t2", kind: "text", who: "01 99 00 12 34", what: "Bien reçu.", when: "02:05", readable: true, state: { mark: "new" } }, sent: "", late: "" }
            ]
            section.reload()
            wait(50)
            compare(labels(section, "texts-notice-late: 22:10, 3 h 45").length, 1)
            compare(walk(section, i => i.visible && i.objectName === "textNotice", []).length, 2)
        }

        function test_lines_and_their_messages() {
            compare(labels(section, "Dr Elena Varga").length, 1, "who")
            compare(labels(section, "Could you call me back tomorrow morning?").length, 1, "the newest words on the row")
            compare(labels(section, "Bonsoir.").length, 1, "the earlier under it")
            compare(labels(section, "My Bank").length, 1, "a code's row: who sent it")
            compare(labels(section, "phonemsgs-code-row").length, 1, "that a code came, never the code")
            compare(labels(section, "Their words stay on your phone: Signal sends who and when only.").length, 1)
            // A text from a number: Text back its main action, on its row only.
            compare(buttons(section, "Text back").length, 1)
            compare(walk(section, i => i.visible && i.objectName === "dismissButton", []).length, 3, "Dismiss on every row")
        }

        function test_dismiss_and_mark_read_go_to_the_porch_s_words() {
            const dismiss = walk(section, i => i.visible && i.objectName === "dismissButton", [])
            mouseClick(dismiss[0])
            compare(JSON.stringify(porchStand.said[0]), JSON.stringify(["dismiss", ["1-a"], []]))
            wait(50)
            compare(labels(section, "Dr Elena Varga").length, 0, "gone at once")
            const read = walk(section, i => i.visible && i.objectName === "readButton", [])
            verify(read.length > 0)
            mouseClick(read[0])
            compare(porchStand.said[1][0], "read")
            compare(mock.asked.filter(a => a[0] === "seen" || a[0] === "unseen").length, 0, "no Seen of their own any more")
        }

        // Reply first, in place of Text back for a text; through the notification's
        // own Reply: the box, its words and the line answered handed to Rust.
        function test_a_reply_goes_through_the_notification_s_own_reply() {
            replying(mock.answered)
            compare(buttons(section, "replies-reply").length, 2, "offered, and the text")
            compare(buttons(section, "Text back").length, 0, "Reply takes its place")
            verify(box() === undefined, "no box before Reply")
            mouseClick(buttons(section, "replies-reply")[0])
            wait(50)
            const words = box()
            verify(words !== undefined, "the box under the line")
            compare(words.placeholderText, "Reply to Alice on Signal…")
            verify(labels(section, "Replying marks the conversation read in Signal.").length === 1, "said the first time")
            words.text = "J'arrive dans dix minutes."
            mouseClick(buttons(section, "replies-send")[0])
            wait(50)
            const sent = last("reply")
            compare([sent.verb, sent.notice, sent.app, sent.line, sent.words, sent.phone], ["reply", "0123456789abcdef", "org.thoughtcrime.securesms", "4-d", "J'arrive dans dix minutes.", "1b4e28ba-2fa1-11d2-883f-0016d3cca427"])
            verify(mock.viewFlag("replies-note-org.thoughtcrime.securesms"), "said once")
            verify(box() === undefined, "closed once sent")
            // What refused it is said in the box, the words kept.
            mouseClick(buttons(section, "replies-reply")[0])
            wait(50)
            box().text = "refused"
            mouseClick(buttons(section, "replies-send")[0])
            wait(50)
            verify(labels(section, "Not sent: it has no words.").length === 1)
            compare(box().text, "refused")
        }

        // A text from a number, where this computer reads texts: one of Sioul's own texts.
        function test_a_text_goes_as_a_text() {
            replying(mock.answered)
            mouseClick(buttons(section, "replies-reply")[1])
            wait(50)
            box().text = "Je vous rappelle à midi."
            mouseClick(buttons(section, "replies-send")[0])
            wait(50)
            const sent = last("texts:send")
            compare([sent.to, sent.body, sent.phone], ["+33465714770", "Je vous rappelle à midi.", "1b4e28ba-2fa1-11d2-883f-0016d3cca427"], "from the phone it came to")
            compare(last("reply"), null, "not through the notification")
        }

        // Your reply under their messages, its state in words, Take back while it waits;
        // a notification gone: Reply on your phone.
        function test_your_reply_shows_and_waits_and_gone_says_reply_on_your_phone() {
            replying(mock.answered)
            compare(labels(section, "You · J'arrive.").length, 1)
            compare(labels(section, "Waiting for your phone.").length, 1)
            mouseClick(buttons(section, "replies-take-back")[0])
            compare(last("take-back").key, "k0")
            wait(50)
            compare(labels(section, "Taking it back: your phone will not send it, unless it has taken it already.").length, 1, "said at once")
            compare(buttons(section, "replies-take-back").length, 0, "no Take back again")
            compare(labels(section, "Reply on your phone.").length, 1, "Bob's notification gone")
        }

        // Send again goes through the conversation's notification now, the answer's, never the refused reply's
        // own, which its app may have replaced; the phone finds the first, on its way, through any notification
        // of the conversation (review of 18 October 2026, L-2).
        function test_send_again_goes_through_the_notification_now() {
            replying(mock.answered)
            const again = buttons(section, "replies-send-again")
            compare(again.length, 1)
            mouseClick(again[0])
            const sent = last("reply")
            const answer = mock.answered[0].answer
            compare([sent.notice, sent.line, sent.again, sent.words], [answer.notice, answer.line, "k9", "Oui, j'arrive."])
        }

        // Mark as read in the line's menu, its warning first.
        function test_mark_as_read_after_its_warning() {
            replying(mock.answered)
            const more = walk(section, i => i.visible && i.Accessible.name === "ui-more", [])
            verify(more.length > 0)
            mouseClick(more[0])
            wait(100)
            const item = popups("Mark as read in Signal")
            verify(item.length === 1, "in the menu")
            item[0].triggered()
            wait(100)
            verify(walk(root.Window.window.contentItem, i => i.visible && i.text === "Signal may tell them that you read it.", []).length > 0, "its warning")
            const confirm = popups("replies-read-confirm")
            verify(confirm.length > 0)
            mouseClick(confirm[0])
            wait(50)
            const read = last("reply")
            compare([read.verb, read.notice, read.words], ["read", "0123456789abcdef", ""])
        }

        function test_the_phone_s_tab_hands_its_choices_to_rust() {
            const rows = walk(setup, i => i.visible && i.setting !== undefined && i.save !== undefined, [])
            const part = rows.filter(r => r.setting.key === "part")[0]
            const app = rows.filter(r => r.setting.key === "app.foundation.e.message")[0]
            verify(part && app)
            part.save("part", true, part.setting.value)
            verify(mock.partOn)
            compare(mock.asked[mock.asked.length - 1][0], "part")
            app.save("app.foundation.e.message", "off", app.setting.value)
            compare(mock.pharmacy, "off")
            compare(mock.asked[mock.asked.length - 1][1].key, "app.foundation.e.message")
            // "Answer from your computers", under the app.
            const answers = walk(setup, i => i.visible && i.setting !== undefined && i.save !== undefined && i.setting.key === "answers.foundation.e.message", [])[0]
            verify(answers)
            answers.save("answers.foundation.e.message", false, true)
            verify(!mock.answers)
        }
    }
}
