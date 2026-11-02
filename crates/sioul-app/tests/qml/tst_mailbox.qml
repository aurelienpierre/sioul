// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

// The mailbox on a computer (the unified mailbox's part 4, MailPage.qml), on
// a stand-in for Sioul and its window: "All addresses" chosen when Mail
// opens; its rows one line each under their days, their addresses' marks;
// the chips, one at a time, and the marks that keep one address, asked of
// Rust and kept on this device; a resting address's line and "Show its
// mail"; "Earlier messages"; a tap opening through the window's one opener,
// Back leaving the list where it was; a selection across two addresses
// acted on and dragged onto a folder; neither list handed the other's rows
// on the way to a folder and back. Run by tools/qml-test.sh.

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
    readonly property string accountsJson: JSON.stringify([
        { id: "work", title: "noa@work.example.org", unread: false, resting: false, folders: [{ name: "INBOX", title: "Inbox", role: "inbox", icon: "mail-folder-inbox", unread: false, kept: true }, { name: "Archive", title: "Archive", role: "archive", icon: "folder", unread: false, kept: true }], more: [] },
        { id: "home", title: "noa@home.example.org", unread: false, resting: true, folders: [{ name: "INBOX", title: "Inbox", role: "inbox", icon: "mail-folder-inbox", unread: false, kept: true }], more: [] }
    ])
    readonly property var marks: ({ work: { account: "work", letter: "N", tint: 0, title: "noa@work.example.org" }, home: { account: "home", letter: "N", tint: 1, title: "noa@home.example.org" } })

    // A row as `mailbox::page` gives it.
    function rowOf(n, day, account, subject, extra) {
        const accounts = Array.isArray(account) ? account : [account]
        return Object.assign({ key: "/stand-in/mail/" + accounts[0] + "/cur/17770000" + n + ".U7-" + n + ".sioul:2,S", uri: "mid:m" + n + "@example.org", account: accounts[0], folder: "INBOX", role: "inbox", who: "Ana", address: "ana@example.org", subject: subject, day: day, time: "09:4" + n, unread: false, read: true, flagged: false, answered: false, attachments: false, trust_level: "verified", trust: "", checks: "", spam: "", hostile: false, marks: accounts.map(a => root.marks[a]), kinds: ["received"], server: false }, extra || {})
    }

    // What the stand-in Rust answers: every row, then those the ask keeps.
    readonly property var allRows: [
        rowOf(1, "Today", ["work", "home"], "On both lists"),
        rowOf(2, "Today", "work", "Quote for the cooperative", { unread: true, read: false }),
        rowOf(3, "Yesterday", "work", "Sent to Iris", { kinds: ["sent"], who: "To Iris" }),
        rowOf(4, "Yesterday", "home", "Dinner on Friday"),
        rowOf(5, "Monday 26 October", "work", "You have won", { kinds: ["spam"], spam: "your provider: spam" })
    ]

    function pageFor(ask) {
        const rows = root.allRows.filter(r => (ask.account === "" || r.marks.some(m => m.account === ask.account))
            && (ask.chip === "spam" ? r.spam !== "" : r.spam === "")
            && (ask.chip !== "sent" || r.kinds.indexOf("sent") >= 0)
            && (ask.chip !== "unread" || !r.read)
            && (ask.account !== "" || ask.shown_resting.indexOf("home") >= 0 || r.marks.some(m => m.account !== "home"))
            && (ask.query === "" || r.subject.toLowerCase().indexOf(ask.query.toLowerCase()) >= 0))
        return { rows: rows, resting: ask.shown_resting.indexOf("home") >= 0 ? [] : [{ account: "home", line: "noa@home.example.org rests until 09:00." }], addresses: [root.marks.work, root.marks.home], earlier: ask.windows < 2, sentence: rows.length === 0 ? "mail-empty" : "", chip: ask.chip, account: ask.account, windows: ask.windows }
    }

    QtObject {
        id: mock

        property string status: ""
        property string mailAccounts: ""
        property string mailFolder: ""
        property string drafts: "[]"
        property string mailSearch: ""
        property bool realtime: false
        property string reading: JSON.stringify({ subject: "Quote for the cooperative", from_name: "Ana", from_address: "ana@example.org", to: [], cc: [], others: false, text: "Hello", html: null, attachments: [], uri: "mid:m2@example.org", flagged: false })
        property string mailbox: ""
        property var mailboxAsked: []
        property string kept: "{}"
        property var acted: []
        property var moved: []
        property var openedFolders: []

        signal messageRead(string key, string reading, string invitation)
        signal messageFound(int ticket, string answer)
        signal mailMoved(string moves)

        function text(key) {
            return key
        }
        function textWith(key, name, value) {
            return key + ":" + value
        }
        // As Rust: the page made for the ask, at once; "" asks nothing.
        function openMailbox(ask) {
            mock.mailboxAsked = mock.mailboxAsked.concat([ask])
            if (ask !== "")
                mock.mailbox = JSON.stringify(root.pageFor(JSON.parse(ask)))
        }
        function mailboxView() {
            return mock.kept
        }
        function openFolder(account, folder, all, query) {
            mock.openedFolders = mock.openedFolders.concat([account + "/" + folder])
        }
        function actMany(keys, action) {
            mock.acted = mock.acted.concat([{ keys: JSON.parse(keys), action: action }])
        }
        function moveMessages(keys, account, folder) {
            mock.moved = mock.moved.concat([{ keys: JSON.parse(keys), account: account, folder: folder }])
        }
        function readMessage(key) {
            Qt.callLater(() => mock.messageRead(key, mock.reading, ""))
        }
        function findMessage(ticket, asked) {
            const ref = JSON.parse(asked)
            Qt.callLater(() => mock.messageFound(ticket, JSON.stringify({ state: "here", file: ref.file, address: ref.address, place: "", reused: false, said: "" })))
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

    QtObject {
        id: host

        property bool open: false
        property var ref: null
        property var leftRef: null

        function leave() {
        }
        function show(ref) {
        }
    }

    Item {
        id: fakeWindow

        anchors.fill: parent

        property int page: 2
        property bool compact: false
        property var cardRef: null
        property var cardHost: host
        property var mailPage: null
        property var moment: ({ sleep: false, quiet: false })
        property var font: ({ family: "" })
        property alias mailFrom: opener.from
        property var asked: []

        function openMail(ref) {
            fakeWindow.asked = fakeWindow.asked.concat([ref])
            opener.open(ref)
        }
        function mailBack() {
            opener.back()
        }
        function openThing(item) {
        }
        function compose(kind, key) {
        }
        function linkFrom(source) {
        }
        function openDraft(id) {
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
        name: "Mailbox"
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
            if (item.contentItem && item.contentItem !== item)
                return find(item.contentItem, name)
            return null
        }

        function all(item, test, into) {
            const found = into || []
            if (!item)
                return found
            if (test(item))
                found.push(item)
            for (let i = 0; i < (item.children || []).length; ++i)
                all(item.children[i], test, found)
            return found
        }

        function lastAsk() {
            return JSON.parse(mock.mailboxAsked[mock.mailboxAsked.length - 1])
        }

        // A Mail page made as the window makes it, its addresses arrived, the mailbox shown.
        function made() {
            mock.mailboxAsked = []
            mock.acted = []
            mock.moved = []
            mock.mailbox = ""
            mock.mailAccounts = ""
            fakeWindow.asked = []
            const page = createTemporaryObject(mailComponent, fakeWindow, { width: 1100, height: 800 })
            fakeWindow.mailPage = page
            mock.mailAccounts = root.accountsJson
            tryCompare(page, "boxShown", true)
            tryVerify(() => page.box !== null && page.box.rows.length > 0)
            return page
        }

        function boxList(page) {
            return find(page, "mailboxList")
        }

        function rowItem(page, subject) {
            return all(page, i => i.objectName === "mailboxRow:" + subject && i.visible)[0] || null
        }

        // "All addresses", chosen when Mail opens on a computer: received and sent
        // in time order, under their days, one line each; a resting address's
        // mail out, its line saying so; the marks of both addresses.
        function test_all_addresses_is_chosen_when_mail_opens() {
            const page = made()
            compare(page.account, "")
            verify(find(page, "mailboxRow").highlighted, "highlighted in the column")
            compare(lastAsk().chip, "all")
            compare(page.listItems.map(r => r.subject), ["On both lists", "Quote for the cooperative", "Sent to Iris"])
            const list = boxList(page)
            verify(list.visible && list.section.property === "day")
            compare(find(page, "folderList").count, 0, "no folder rows made beside it")
            tryVerify(() => rowItem(page, "On both lists") !== null)
            const both = rowItem(page, "On both lists")
            // One line: as tall as one line of text and its paddings.
            verify(both.height < 40, "one line: " + both.height)
            compare(all(both, i => i.mark !== undefined && i.mark !== null).length, 2, "both addresses' marks")
            compare(find(page, "mailboxResting").text, "noa@home.example.org rests until 09:00.")
            // "Show its mail": the resting address's mail too, this time.
            find(page, "mailboxShowResting").clicked()
            compare(lastAsk().shown_resting, ["home"])
            compare(page.listItems.map(r => r.subject), ["On both lists", "Quote for the cooperative", "Sent to Iris", "Dinner on Friday"])
            verify(find(page, "mailboxResting") === null || !find(page, "mailboxResting").visible)
        }

        // The chips, one at a time, nothing counted; the marks keep one
        // address, then all again; what was chosen kept, read back next time.
        function test_the_chips_and_the_marks() {
            const page = made()
            find(page, "chip:spam").clicked()
            compare(lastAsk().chip, "spam")
            compare(page.listItems.map(r => r.subject), ["You have won"])
            verify(find(page, "chip:spam").highlighted && !find(page, "chip:all").highlighted)
            find(page, "chip:sent").clicked()
            compare(page.listItems.map(r => r.subject), ["Sent to Iris"])
            find(page, "chip:all").clicked()
            find(page, "address:home").clicked()
            compare(lastAsk().account, "home")
            compare(page.listItems.map(r => r.subject), ["On both lists", "Dinner on Friday"])
            find(page, "address:home").clicked()
            compare(lastAsk().account, "", "a second tap: every address")
            // No chip says a number.
            for (const chip of ["all", "unread", "sent", "spam"])
                verify(!/[0-9]/.test(find(page, "chip:" + chip).text), chip)
            // Kept on this device: the next page starts from it.
            mock.kept = JSON.stringify({ chip: "unread", account: "work" })
            const next = made()
            compare([next.boxChip, next.boxAccount], ["unread", "work"])
            compare(lastAsk().chip, "unread")
            mock.kept = "{}"
        }

        // "Earlier messages": two more weeks, asked of Rust.
        function test_earlier_adds_two_weeks() {
            const page = made()
            const earlier = find(page, "mailboxEarlier")
            verify(earlier.visible)
            earlier.clicked()
            compare(lastAsk().windows, 2)
            verify(!find(page, "mailboxEarlier").visible)
        }

        // A tap opens through the window's one opener, by its address, its file a
        // hint; Back closes it, the list where it was, Mail's own.
        function test_a_tap_opens_through_the_opener_and_back_keeps_the_place() {
            const page = made()
            const list = boxList(page)
            tryVerify(() => rowItem(page, "Quote for the cooperative") !== null)
            const row = rowItem(page, "Quote for the cooperative")
            list.contentY = 0
            mouseClick(row, 120, row.height / 2)
            compare(fakeWindow.asked.length, 1)
            compare([fakeWindow.asked[0].address, fakeWindow.asked[0].account], ["mid:m2@example.org", "work"])
            tryCompare(page, "openKey", page.listItems[1].key)
            verify(page.reading)
            page.back()
            compare(page.openKey, "")
            compare(fakeWindow.page, 2, "Mail's own Back: the list again")
            verify(page.boxShown && boxList(page).visible)
        }

        // A selection across two addresses, with Ctrl, acted on in one go; then
        // dragged onto a folder, as from a search.
        function test_a_selection_across_addresses_acted_on_and_dragged() {
            const page = made()
            find(page, "mailboxShowResting").clicked()
            const items = page.listItems
            const workRow = items.findIndex(r => r.subject === "Quote for the cooperative")
            const homeRow = items.findIndex(r => r.subject === "Dinner on Friday")
            page.clickRow(workRow, items[workRow].key, Qt.ControlModifier)
            page.clickRow(homeRow, items[homeRow].key, Qt.ControlModifier)
            compare(page.selectedKeys.length, 2)
            verify(items[workRow].account !== items[homeRow].account, "two addresses")
            page.actOnSelection("archive")
            compare(mock.acted.length, 1)
            compare(mock.acted[0].action, "archive")
            compare(mock.acted[0].keys.slice().sort(), [items[workRow].key, items[homeRow].key].sort())
            // Dragged onto a folder: the row with the selection it is in.
            page.clickRow(workRow, items[workRow].key, Qt.ControlModifier)
            page.clickRow(homeRow, items[homeRow].key, Qt.ControlModifier)
            page.startDrag(items[workRow].key)
            verify(page.dragging)
            page.dropOn("work", "Archive")
            compare(mock.moved.length, 1)
            compare([mock.moved[0].account, mock.moved[0].folder, mock.moved[0].keys.length], ["work", "Archive", 2])
            page.letGo()
        }

        // The quick search, typed, asks Rust across every address after a pause.
        function test_the_quick_search_asks_across_addresses() {
            const page = made()
            page.boxShownResting = ["home"]
            const field = all(page, i => i.objectName === "mailSearchField" && i.visible)[0]
            verify(field, "the search field in sight")
            field.text = "dinner"
            field.textEdited()
            tryCompare(page, "query", "dinner")
            tryVerify(() => lastAsk().query === "dinner")
            compare(lastAsk().shown_resting, ["home"])
            compare(page.listItems.map(r => r.subject), ["Dinner on Friday"])
            verify(page.boxShown, "still the mailbox")
        }

        // From the mailbox to a folder and back: neither list is ever handed
        // the other's rows, not even for a moment (a warning would say so).
        function test_neither_list_is_handed_the_other_s_rows() {
            const page = made()
            mock.mailFolder = JSON.stringify({ items: [{ key: "/stand-in/mail/work/cur/17770000.U7-9.sioul:2,S", uri: "mid:f9@example.org", who: "Ben", address: "ben@example.org", subject: "In the inbox", date: "09:00", unread: false, read: true, flagged: false, answered: false, attachments: false, trust_level: "verified", trust: "", checks: "", server: false }], title: "Inbox", sentence: "", role: "inbox", earlier: false })
            failOnWarning(/undefined|TypeError|ReferenceError/)
            page.openFolder("work", "INBOX")
            compare(page.listItems.map(r => r.subject), ["In the inbox"])
            tryVerify(() => find(page, "folderList").count === 1)
            page.openBox()
            compare(page.listItems.map(r => r.subject), ["On both lists", "Quote for the cooperative", "Sent to Iris"])
            tryVerify(() => boxList(page).count === 3)
            compare(find(page, "folderList").count, 0)
            mock.mailFolder = ""
        }

        // Mail out of sight asks nothing; back in sight, it asks again.
        function test_a_hidden_page_asks_nothing() {
            const page = made()
            const before = mock.mailboxAsked.length
            page.visible = false
            compare(mock.mailboxAsked[mock.mailboxAsked.length - 1], "")
            page.visible = true
            compare(mock.mailboxAsked.length, before + 2)
            compare(lastAsk().chip, "all")
        }
    }
}
