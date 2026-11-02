// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

// The window's one opener for a message (the unified mailbox's part 2,
// MailOpener.qml), on a stand-in for Sioul and its window: every path of
// §3.4 reaches its one message, as each gives it (new mail's notification
// of one message, a code's notification and card, the phone's home card by
// address and an older card's file, "Tied to it", a project's line of time,
// a task's source, a note's link, a money row, an event's invitation, the
// card table), with Mail made before and not, at a computer's size and a
// phone's; never a folder over it; Back returns to the page it came from,
// its card shown again, one level; a message chosen in Mail's own list keeps
// Mail's own Back; each answer says its sentence, never a blank page. Run
// by tools/qml-test.sh.

import QtQuick
import QtQuick.Controls.Basic
import QtTest
import "../../qml"

Item {
    id: root

    width: 1100
    height: 800

    Theme {
        id: testTheme
    }

    // Made up: no real address, no real message.
    readonly property string messageKey: "/stand-in/mail/alice/.Archive/cur/1777000000.U1-1.sioul:2,S"
    readonly property string accountsJson: JSON.stringify([{ id: "alice", label: "alice@example.org", resting: false, folders: [{ name: "INBOX", display: "Inbox", role: "inbox" }], more: [] }])
    readonly property var here: ({ state: "here", file: root.messageKey, address: "mid:plan@example.org", place: "In Archive · alice@example.org", reused: false })

    QtObject {
        id: mock

        property string status: ""
        property string mailAccounts: ""
        property string mailFolder: ""
        property string drafts: "[]"
        property string mailSearch: ""
        property bool realtime: false
        property string reading: JSON.stringify({ subject: "The plan", from_name: "Ana", from_address: "ana@example.org", to: [], cc: [], others: false, text: "Hello", html: null, attachments: [], uri: "mid:plan@example.org", flagged: false })
        property var openedFolders: []
        // What `findMessage` was asked, and what it answers, in order (one answer each turn).
        property var asked: []
        property var plan: [root.here]

        signal messageRead(string key, string reading, string invitation)
        signal messageFound(int ticket, string answer)

        function text(key) {
            return key
        }
        function textWith(key, name, value) {
            return key + ":" + value
        }
        function openFolder(account, folder, all, query) {
            mock.openedFolders = mock.openedFolders.concat([account + "/" + folder])
        }
        function readMessage(key) {
            Qt.callLater(() => mock.messageRead(key, mock.reading, ""))
        }
        function findMessage(ticket, asked) {
            mock.asked = mock.asked.concat([JSON.parse(asked)])
            const answers = mock.plan.slice()
            const next = () => {
                const answer = answers.shift()
                if (answer === undefined)
                    return
                mock.messageFound(ticket, JSON.stringify(answer))
                if (answers.length > 0)
                    Qt.callLater(next)
            }
            Qt.callLater(next)
        }
        function clearMailSearch() {
        }
        function viewFlag(name) {
            return false
        }
        function siteForSender(address) {
            return "null"
        }
        function contactFor(address) {
            return ""
        }
        function opened(key) {
        }
        function attachmentsUnscanned() {
            return false
        }
        function uriOf(kind, id) {
            return ""
        }
        function related(uri) {
            return "[]"
        }
    }

    // The card host's part the opener reads: the card open, the one a place's opening just closed.
    QtObject {
        id: host

        property bool open: false
        property var ref: null
        property var leftRef: null
        property var shown: []

        function leave() {
            if (fakeWindow.compact && host.open) {
                host.leftRef = host.ref
                Qt.callLater(() => host.leftRef = null)
                host.open = false
                host.ref = null
            }
        }
        function show(ref) {
            host.ref = ref
            host.open = true
            host.shown = host.shown.concat([ref])
        }
    }

    // The window, as the pages and the opener use it: its pages by number, Mail made when first shown.
    Item {
        id: fakeWindow

        anchors.fill: parent

        property int page: 0
        property bool compact: false
        property var cardRef: host.ref
        property var cardHost: host
        property var mailPage: null
        // What the window's own pages read of it.
        property var moment: ({ sleep: false, quiet: false })
        property var font: ({ family: "" })
        property alias mailFrom: opener.from
        property var opened: []

        onPageChanged: {
            if (fakeWindow.page === 2 && fakeWindow.mailPage === null)
                fakeWindow.mailPage = mailComponent.createObject(fakeWindow, { width: fakeWindow.width, height: fakeWindow.height })
        }

        function openMail(ref) {
            opener.open(ref)
        }
        function mailBack() {
            opener.back()
        }
        // As main.qml's `openThing` takes a message: a place, so the card over the page left
        // first (on a phone it closes), then the opener, by its reference.
        function openThing(item) {
            fakeWindow.opened = fakeWindow.opened.concat([item])
            if (["task", "event", "dose", "need", "stopped"].indexOf(item.kind) < 0)
                host.leave()
            if (item.kind === "mail")
                opener.open(opener.refOf(item))
        }
        function compose(kind, key) {
        }
        function linkFrom(source) {
        }
        function openDraft(id) {
        }
        function openTask(key) {
        }
    }

    MailOpener {
        id: opener

        window: fakeWindow
        sioul: mock
    }

    Component {
        id: mailComponent

        MailPage {
            sioul: mock
            theme: testTheme
            window: fakeWindow
        }
    }

    TestCase {
        name: "OpenMail"
        when: windowShown

        function find(item, name) {
            if (!item)
                return null
            if (item.objectName === name)
                return item
            const kids = item.children || []
            for (let i = 0; i < kids.length; ++i) {
                const found = find(kids[i], name)
                if (found)
                    return found
            }
            if (item.contentItem && item.contentItem !== item) {
                const inside = find(item.contentItem, name)
                if (inside)
                    return inside
            }
            return null
        }

        // The Reader shown in a page.
        function reader(item) {
            if (!item)
                return null
            if (item.showing !== undefined && item.key !== undefined && item.reading !== undefined)
                return item
            const kids = item.children || []
            for (let i = 0; i < kids.length; ++i) {
                const found = reader(kids[i])
                if (found)
                    return found
            }
            return null
        }

        function init() {
            if (fakeWindow.mailPage !== null) {
                fakeWindow.mailPage.destroy()
                fakeWindow.mailPage = null
            }
            fakeWindow.page = 0
            fakeWindow.compact = false
            fakeWindow.opened = []
            opener.from = null
            host.open = false
            host.ref = null
            host.shown = []
            mock.mailAccounts = root.accountsJson
            mock.mailFolder = ""
            mock.openedFolders = []
            mock.asked = []
            mock.plan = [root.here]
        }

        // Every path of §3.4 as it reaches the opener: what it carries, and what the opener asks Rust.
        function paths() {
            return [
                { tag: "new mail's notification of one message", thing: { kind: "mail", uri: "", key: "mid:plan@example.org" }, address: "mid:plan@example.org", file: "" },
                { tag: "a code's notification", thing: { kind: "mail", uri: "mid:code-1@example.org", key: "/stand-in/mail/alice/cur/1777000001.U1-2.sioul:2,S" }, address: "mid:code-1@example.org", file: "/stand-in/mail/alice/cur/1777000001.U1-2.sioul:2,S" },
                { tag: "a code card on the Porch", ref: { address: "mid:code-1@example.org", file: "/stand-in/mail/alice/cur/1777000001.U1-2.sioul:2,S" }, address: "mid:code-1@example.org", file: "/stand-in/mail/alice/cur/1777000001.U1-2.sioul:2,S" },
                { tag: "the home card, by its address", ref: { address: "mid:plan@example.org", file: "" }, address: "mid:plan@example.org", file: "" },
                { tag: "an older home card, by its file", ref: { address: "", file: "/stand-in/mail/alice/cur/1777000000.U1-1.sioul:2,S" }, address: "", file: "/stand-in/mail/alice/cur/1777000000.U1-1.sioul:2,S" },
                { tag: "Tied to it", thing: { kind: "mail", uri: "mid:plan@example.org", key: "/stand-in/mail/alice/cur/1777000000.U1-1.sioul:2,S", title: "The plan" }, address: "mid:plan@example.org", file: "/stand-in/mail/alice/cur/1777000000.U1-1.sioul:2,S" },
                { tag: "a project's line of time", thing: { uri: "mid:plan@example.org", kind: "mail", key: "/stand-in/mail/alice/cur/1777000000.U1-1.sioul:2,S" }, address: "mid:plan@example.org", file: "/stand-in/mail/alice/cur/1777000000.U1-1.sioul:2,S" },
                { tag: "a note's link", thing: { uri: "mid:plan@example.org", kind: "mail", key: "" }, address: "mid:plan@example.org", file: "" },
                { tag: "a money row", thing: { uri: "mid:invoice-1@example.org", kind: "mail", key: "" }, address: "mid:invoice-1@example.org", file: "" },
                { tag: "a message without a Message-ID", thing: { kind: "mail", uri: "", key: "sioul:mail/alice/0123456789abcdef" }, address: "sioul:mail/alice/0123456789abcdef", file: "" },
                { tag: "an event's invitation", ref: { invitation: "event-1", organizer: "ana@example.org", file: "/stand-in/mail/alice/cur/1777000003.U1-4.sioul:2,S" }, address: "", file: "/stand-in/mail/alice/cur/1777000003.U1-4.sioul:2,S", invitation: "event-1" },
                { tag: "the card table", ref: { kind: "mail", address: "mid:plan@example.org" }, address: "mid:plan@example.org", file: "" }
            ]
        }

        // One path from the Tasks page (1), a card open there, Mail made before or not: the
        // message opens, never a folder over it; Back returns to Tasks, the card shown again.
        function opensFrom(path, compact, madeBefore) {
            init()
            fakeWindow.compact = compact
            if (madeBefore) {
                fakeWindow.page = 2
                tryCompare(fakeWindow.mailPage, "account", "alice")
            }
            fakeWindow.page = 1
            host.show({ kind: "task", owner: "t1" })
            host.shown = []
            if (path.thing)
                fakeWindow.openThing(path.thing)
            else
                fakeWindow.openMail(path.ref)
            const label = path.tag + (compact ? ", a phone" : ", a computer") + (madeBefore ? ", Mail made before" : ", Mail made by it")
            compare(fakeWindow.page, 2, label + ": Mail")
            const page = fakeWindow.mailPage
            const asked = mock.asked[mock.asked.length - 1]
            compare([asked.address, asked.file, asked.invitation].join(" | "), [path.address, path.file, path.invitation || ""].join(" | "), label + ": what Rust is asked")
            tryCompare(page, "openKey", root.messageKey, 5000, label)
            tryVerify(() => reader(page) !== null && reader(page).reading !== null, 5000, label + ": read")
            verify(reader(page).visible, label + ": its reader shows")
            tryCompare(page, "account", "alice", 5000, label + ": the first inbox beside it")
            compare(page.openKey, root.messageKey, label + ": never closed by the folder")
            compare(find(page, "readerPlace").text, root.here.place, label + ": where it was found")
            // Back: Tasks again, its card shown again (on a phone the opening closed it).
            page.back()
            compare(fakeWindow.page, 1, label + ": Back to where it came from")
            compare(page.openKey, "", label + ": the reader closed")
            verify(host.open && host.ref.owner === "t1", label + ": the card again")
            compare(opener.from, null, label + ": one level")
        }

        function test_every_path_opens_its_message_data() {
            return paths()
        }

        function test_every_path_opens_its_message(path) {
            for (const compact of [false, true])
                for (const madeBefore of [false, true])
                    opensFrom(path, compact, madeBefore)
        }

        // Each answer says its sentence in the reader's place, never a blank page: the
        // servers asked, then the message; gone; unreachable; not here; no invitation.
        function test_each_answer_says_its_sentence_data() {
            return [
                { tag: "asked, then brought", plan: [{ state: "asking" }, root.here], first: "reader-looking", last: "" },
                { tag: "gone", plan: [{ state: "asking" }, { state: "gone" }], first: "reader-looking", last: "mail-find-gone" },
                { tag: "unreachable", plan: [{ state: "asking" }, { state: "unreachable" }], first: "reader-looking", last: "mail-find-unreachable" },
                { tag: "not here", plan: [{ state: "not-here" }], first: "reader-not-here", last: "reader-not-here" },
                { tag: "no invitation", plan: [{ state: "no-invitation" }], first: "reader-no-invitation", last: "reader-no-invitation" }
            ]
        }

        function test_each_answer_says_its_sentence(data) {
            for (const compact of [false, true]) {
                init()
                fakeWindow.compact = compact
                mock.plan = data.plan
                fakeWindow.page = 0
                fakeWindow.openMail({ address: "mid:away@example.org" })
                const page = fakeWindow.mailPage
                const shown = reader(page)
                verify(shown !== null && shown.visible && shown.showing, "something shows at once")
                compare(find(page, "readerAnswer").text, "reader-opening", "Opening the message…")
                tryCompare(find(page, "readerAnswer"), "text", data.first, 5000, data.tag)
                verify(find(page, "readerAnswer").visible && !find(page, "readerActions").visible && !find(page, "readerHeader").visible, data.tag + ": the sentence alone")
                if (data.last === "") {
                    tryCompare(page, "openKey", root.messageKey, 5000)
                    tryVerify(() => reader(page).reading !== null)
                    verify(!find(page, "readerAnswer").visible, "the message, no sentence")
                } else {
                    tryCompare(find(page, "readerAnswer"), "text", data.last, 5000, data.tag)
                    wait(50)
                    compare(find(page, "readerAnswer").text, data.last, data.tag + ": it stays")
                    verify(reader(page).visible, data.tag + ": never a blank page")
                    compare(page.openKey, "", data.tag + ": no file opened")
                }
                // Mail's first inbox beside it, never over it.
                tryCompare(page, "account", "alice")
                verify(page.reading, data.tag + ": still the reader's place")
            }
        }

        // A server that could not be reached: asked again when mail comes in, then opened.
        function test_an_unreachable_server_is_asked_again() {
            mock.plan = [{ state: "asking" }, { state: "unreachable" }]
            fakeWindow.openMail({ address: "mid:away@example.org" })
            const page = fakeWindow.mailPage
            tryCompare(find(page, "readerAnswer"), "text", "mail-find-unreachable")
            compare(mock.asked.length, 1)
            mock.plan = [root.here]
            mock.mailFolder = JSON.stringify({ items: [] })
            compare(mock.asked.length, 2, "asked again")
            compare(mock.asked[1].address, "mid:away@example.org")
            tryCompare(page, "openKey", root.messageKey)
            // Once open, mail coming in asks nothing more.
            mock.mailFolder = JSON.stringify({ items: [], again: true })
            compare(mock.asked.length, 2)
        }

        // Escape on the reader's sentence, as on a message: back where it was opened from.
        function test_escape_returns_from_the_reader_s_sentence() {
            fakeWindow.page = 1
            mock.plan = [{ state: "gone" }]
            fakeWindow.openMail({ address: "mid:gone@example.org" })
            tryCompare(find(fakeWindow.mailPage, "readerAnswer"), "text", "mail-find-gone")
            fakeWindow.forceActiveFocus()
            keyClick(Qt.Key_Escape)
            tryCompare(fakeWindow, "page", 1)
            verify(!fakeWindow.mailPage.reading, "the reader closed")
        }

        // A message chosen in Mail's own list is Mail's: its Back stays in Mail.
        function test_a_message_chosen_in_mail_keeps_mail_s_back() {
            fakeWindow.page = 1
            fakeWindow.openMail({ address: "mid:plan@example.org" })
            const page = fakeWindow.mailPage
            tryCompare(page, "openKey", root.messageKey)
            compare(opener.from.page, 1)
            page.clickRow(0, root.messageKey, Qt.NoModifier)
            compare(opener.from, null, "Mail's own now")
            page.back()
            compare(fakeWindow.page, 2, "Back stays in Mail")
            compare(page.openKey, "")
        }

        // Asked again while the first answer is on its way: the last asked alone opens.
        function test_only_the_last_message_asked_opens() {
            fakeWindow.page = 1
            const keys = []
            mock.plan = [{ state: "here", file: "/stand-in/mail/alice/cur/old", address: "mid:old@example.org", place: "", reused: false }]
            fakeWindow.openMail({ address: "mid:old@example.org" })
            fakeWindow.mailPage.openKeyChanged.connect(() => keys.push(fakeWindow.mailPage.openKey))
            mock.plan = [root.here]
            fakeWindow.openMail({ address: "mid:plan@example.org" })
            tryCompare(fakeWindow.mailPage, "openKey", root.messageKey)
            wait(50)
            compare(fakeWindow.mailPage.openKey, root.messageKey)
            verify(keys.indexOf("/stand-in/mail/alice/cur/old") < 0, "the older answer never opened: " + keys)
            compare(opener.from.page, 1, "still from Tasks")
        }

        // Another message carries the same id: said in the reader.
        function test_a_reused_id_is_said() {
            mock.plan = [Object.assign({}, root.here, { reused: true })]
            fakeWindow.openMail({ address: "mid:plan@example.org" })
            tryVerify(() => find(fakeWindow.mailPage, "readerReused") !== null && find(fakeWindow.mailPage, "readerReused").visible)
        }
    }
}
