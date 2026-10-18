// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

// The phone's messages on a computer's Porch (PhoneMessagesSection.qml) and
// the phone's tab for them (PhoneMessagesSetup.qml), on a stand-in for
// Sioul. What travels and when a line shows are tested in sioul-core
// (phonemsgs.rs) and sioul-sync (share.rs); here the window's side: the
// lines and their messages, Text back only for a text from a number, Seen
// and its ten seconds to undo, the part's switch and an app's choice handed
// to Rust; a reply: Reply first, its box and Send (through the
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
        readonly property var plain: [
            { ids: ["1-a"], head: "While you slept, Dr Elena Varga wrote:", said: [{ time: "23:10", from: "", text: "Could you call me back tomorrow morning?" }], note: "", number: "04 65 71 47 70", dial: "+33465714770", known: true, texted: true, code: false },
            { ids: ["2-b"], head: "A code came from My Bank at 11:05. It stays on your phone.", said: [], note: "", number: "", dial: "", known: false, texted: false, code: true },
            { ids: ["3-c"], head: "In your leisure time, Sam wrote on Signal:", said: [{ time: "13:02", from: "", text: "" }], note: "Their words stay on your phone: Signal sends who and when only.", number: "", dial: "", known: false, texted: false, code: false }
        ]
        // The same with what each offers a reply from this computer (replies.rs's `Answer`).
        function answer(line, offered, texts, mine) {
            return { phone: "1b4e28ba-2fa1-11d2-883f-0016d3cca427", notice: "0123456789abcdef", app: "org.thoughtcrime.securesms", label: "Signal", line: line, offered: offered, read: offered && !texts, texts: texts,
                     placeholder: "Reply to Alice on Signal…", note: "Replying marks the conversation read in Signal.", read_item: "Mark as read in Signal", read_warning: "Signal may tell them that you read it.",
                     on_phone: "Reply on your phone.", limit: texts ? 1600 : 2000, mine: mine }
        }
        readonly property var answered: [
            { ids: ["4-d"], head: "Alice wrote on Signal:", said: [{ time: "14:01", from: "", text: "Tu passes ?" }], note: "", number: "", dial: "", known: false, texted: false, code: false, who: "Alice",
              answer: mock.answer("4-d", true, false, [{ key: "k0", time: "14:02", text: "J'arrive.", said: "Waiting for your phone.", waiting: true, again: false, texts: false }]) },
            { ids: ["5-e"], head: "Bob wrote on Signal:", said: [{ time: "14:03", from: "", text: "Merci." }], note: "", number: "", dial: "", known: false, texted: false, code: false, who: "Bob", answer: mock.answer("5-e", false, false, []) },
            { ids: ["6-f"], head: "Dr Elena Varga wrote:", said: [{ time: "14:04", from: "", text: "Rappelez-moi." }], note: "", number: "04 65 71 47 70", dial: "+33465714770", known: true, texted: true, code: false, who: "Dr Elena Varga", answer: mock.answer("6-f", true, true, []) }
        ]

        signal porchChanged()

        function text(key) {
            return ({ "calls-seen": "Seen", "calls-text-back": "Text back", "calls-call-back": "Call back", "ui-undo": "Undo", "phonemsgs-title": "From your phone", "replies-you": "You" })[key] || key
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
                return JSON.stringify({ lines: mock.lines.filter(l => mock.seen.indexOf(l.ids[0]) < 0), links: { tel: true, sms: true } })
            if (verb === "seen") {
                mock.seen = mock.seen.concat(args.ids)
                return JSON.stringify({ said: "Marked seen.", shared: true, at: 1791446400000 })
            }
            if (verb === "unseen") {
                mock.seen = []
                return JSON.stringify({ said: "", shared: true, at: 0 })
            }
            if (verb === "reply")
                return JSON.stringify(args.words === "refused" ? { said: "Not sent: it has no words." } : { key: "k1", said: "", shared: true })
            if (verb === "take-back")
                return JSON.stringify({ said: "", shared: true, at: 0 })
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

    QtObject {
        id: stand

        property var now: 0
        property bool compact: false
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
            mock.asked = []
            mock.seen = []
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

        function test_lines_and_their_messages() {
            compare(labels(section, "While you slept, Dr Elena Varga wrote:").length, 1)
            compare(labels(section, "Could you call me back tomorrow morning?").length, 1)
            compare(labels(section, "A code came from My Bank at 11:05. It stays on your phone.").length, 1)
            compare(labels(section, "Their words stay on your phone: Signal sends who and when only.").length, 1)
            // A text from a number: Text back and Call back on its line only.
            compare(buttons(section, "Text back").length, 1)
            compare(buttons(section, "Call back").length, 1)
            compare(buttons(section, "Seen").length, 3)
        }

        function test_seen_then_undone() {
            mouseClick(buttons(section, "Seen")[0])
            const last = mock.asked.filter(a => a[0] === "seen")
            compare(last.length, 1)
            compare(last[0][1].ids, ["1-a"])
            wait(50)
            compare(labels(section, "While you slept, Dr Elena Varga wrote:").length, 0, "gone at once")
            compare(labels(section, "Marked seen.").length, 1)
            const undo = buttons(section, "Undo")
            compare(undo.length, 1, "ten seconds to undo")
            mouseClick(undo[0])
            const back = mock.asked.filter(a => a[0] === "unseen")
            compare(back.length, 1)
            compare(back[0][1].at, 1791446400000)
            wait(50)
            compare(labels(section, "While you slept, Dr Elena Varga wrote:").length, 1, "back")
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
            compare([sent.to, sent.body], ["+33465714770", "Je vous rappelle à midi."])
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
            compare(labels(section, "Reply on your phone.").length, 1, "Bob's notification gone")
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
