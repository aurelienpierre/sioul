// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

// Links that open a message (the unified mailbox's part 0), on a stand-in
// for Sioul: Mail made by the very link that opens it keeps the message open
// when its addresses arrive, at a computer's size and a phone's (MailPage.qml);
// a message that does not read here says so in the reader's place (Reader.qml);
// a note's `mid:` link however spelled asks Rust what it leads to
// (NotesPage.qml, `linkTarget`); a money row opens its message (BudgetsPage.qml).
// Run by tools/qml-test.sh.

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
    readonly property string messageKey: "/stand-in/mail/alice/cur/1777000000.U1-1.sioul:2,S"
    readonly property string accountsJson: JSON.stringify([{ id: "alice", label: "alice@example.org", resting: false, folders: [{ name: "INBOX", display: "Inbox", role: "inbox" }] }])

    QtObject {
        id: mock

        property string status: ""
        property string mailAccounts: ""
        property string mailFolder: ""
        property string drafts: "[]"
        property string mailSearch: ""
        property string budgets: ""
        property bool realtime: false
        // What a message reads as: "" for one that is not here.
        property string reading: JSON.stringify({ subject: "The plan", from_name: "Ana", from_address: "ana@example.org", to: [], others: false, text: "Hello", attachments: [], uri: "mid:plan@example.org", flagged: false })
        property var openedFolders: []

        signal messageRead(string key, string reading, string invitation)

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
        function clearMailSearch() {
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
            return JSON.stringify([{ uri: "mid:abc@example.org", kind: "mail", key: root.messageKey, title: "The plan" }])
        }
        // As Rust answers it (`links::World::link_target`): the address made canonical, then the tie or the thing.
        function linkTarget(source, href) {
            const bare = decodeURIComponent(href.trim().replace(/^mid:/, "")).replace(/^</, "").replace(/>$/, "")
            return bare === "abc@example.org" ? JSON.stringify({ uri: "mid:abc@example.org", kind: "mail", key: root.messageKey, title: "The plan" }) : "null"
        }
    }

    QtObject {
        id: fakeWindow

        property bool compact: false
        property var cardRef: null
        property var opened: []

        function openThing(item) {
            fakeWindow.opened = fakeWindow.opened.concat([item])
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

    Component {
        id: mailComponent

        MailPage {
            anchors.fill: parent
            sioul: mock
            theme: testTheme
            window: fakeWindow
        }
    }

    Component {
        id: readerComponent

        Reader {
            width: 600
            height: 600
            sioul: mock
            theme: testTheme
            window: fakeWindow
        }
    }

    Component {
        id: notesComponent

        NotesPage {
            anchors.fill: parent
            sioul: mock
            theme: testTheme
            window: fakeWindow
        }
    }

    Component {
        id: budgetsComponent

        BudgetsPage {
            anchors.fill: parent
            sioul: mock
            theme: testTheme
            window: fakeWindow
        }
    }

    TestCase {
        name: "MailOpen"
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

        // The Reader shown in a page: the item with the reader's own `showing`.
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
            mock.mailAccounts = ""
            mock.mailFolder = ""
            mock.openedFolders = []
            mock.reading = JSON.stringify({ subject: "The plan", from_name: "Ana", from_address: "ana@example.org", to: [], others: false, text: "Hello", attachments: [], uri: "mid:plan@example.org", flagged: false })
            fakeWindow.opened = []
            fakeWindow.compact = false
        }

        // Mail made by the link that opens a message: its addresses arrive a
        // moment later, and the message stays open, the first inbox beside it.
        function linked(compact) {
            fakeWindow.compact = compact
            const page = createTemporaryObject(mailComponent, root)
            page.openMessage(root.messageKey)
            mock.mailAccounts = root.accountsJson
            tryCompare(page, "account", "alice")
            compare(mock.openedFolders, ["alice/INBOX"], "the first inbox, listed")
            compare(page.openKey, root.messageKey, "the message asked stays open")
            tryVerify(() => reader(page) !== null && reader(page).reading !== null)
            verify(reader(page).visible, "its reader shows")
        }

        function test_mail_made_by_a_link_keeps_the_message_on_a_computer() {
            linked(false)
        }

        function test_mail_made_by_a_link_keeps_the_message_on_a_phone() {
            linked(true)
        }

        // Without a link, Mail opens on the first inbox, nothing open.
        function test_mail_alone_opens_the_first_inbox() {
            const page = createTemporaryObject(mailComponent, root)
            mock.mailAccounts = root.accountsJson
            tryCompare(page, "account", "alice")
            compare(page.openKey, "")
        }

        // A message that does not read here: the reader says so in its place, never blank.
        function test_a_message_not_here_says_so() {
            mock.reading = ""
            const shown = createTemporaryObject(readerComponent, root)
            shown.key = root.messageKey
            tryCompare(shown, "opening", false)
            verify(shown.showing, "something shows")
            const said = find(shown, "readerNotHere")
            verify(said !== null && said.visible, "the sentence")
            compare(said.text, "reader-not-here")
            mock.reading = JSON.stringify({ subject: "Back", from_name: "Ana", from_address: "ana@example.org", to: [], others: false, text: "Hi", attachments: [], uri: "mid:b@example.org", flagged: false })
            shown.key = "/stand-in/mail/alice/cur/2"
            tryCompare(shown, "opening", false)
            verify(!find(shown, "readerNotHere").visible, "a message that reads: no sentence")
        }

        // A note's link to a message, however spelled, opens the message's file.
        function test_a_note_s_mail_link_opens_its_message_however_spelled() {
            const page = createTemporaryObject(notesComponent, root)
            for (const href of ["mid:<abc@example.org>", "mid:abc@example.org", "mid:abc%40example.org"]) {
                fakeWindow.opened = []
                page.follow(href)
                compare(fakeWindow.opened.length, 1, href)
                compare(fakeWindow.opened[0].key, root.messageKey, href)
            }
        }

        // A money row opens the message it comes from.
        function test_a_money_row_opens_its_message() {
            mock.budgets = JSON.stringify({ budgets: [], choices: [], mail_title: "Mail about money", mail: [
                { key: "mid:abc@example.org", kind: "invoice", kind_label: "Invoice", party: "Shop", amount: "12.00", credit: false, date: "30 Oct", subject: "Your invoice", note: "", state: "proposed", doubtful: false, can_add: false, budget: "" },
                { key: "file:/stand-in/mail/alice/cur/9", kind: "invoice", kind_label: "Invoice", party: "Shop", amount: "3.00", credit: false, date: "29 Oct", subject: "No id", note: "", state: "proposed", doubtful: false, can_add: false, budget: "" }
            ] })
            const page = createTemporaryObject(budgetsComponent, root)
            const buttons = () => {
                const out = []
                const collect = item => {
                    if (item.objectName === "budgetOpenMessage")
                        out.push(item)
                    for (let i = 0; i < (item.children || []).length; ++i)
                        collect(item.children[i])
                }
                collect(page)
                return out
            }
            tryVerify(() => buttons().length === 2, 5000, "both rows, each with its button")
            buttons()[0].clicked()
            buttons()[1].clicked()
            compare(fakeWindow.opened.map(o => o.key), [root.messageKey, "/stand-in/mail/alice/cur/9"])
            compare(fakeWindow.opened[0].kind, "mail")
        }
    }
}
