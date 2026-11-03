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
// on the way to a folder and back; a row held by two addresses acted on as
// one message, each copy at once; the rows by pages of sixty; on a phone
// (part 5), two lines a row within their objects, a long press, Select,
// Back, the other folders asked for. Run by tools/qml-test.sh.

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
    readonly property var marks: ({ work: { account: "work", letter: "N", tint: 0, title: "noa@work.example.org" }, home: { account: "home", letter: "H", tint: 1, title: "noa@home.example.org" } })
    readonly property var places: ["work", "home"]
    readonly property string prefix: "/stand-in/mail/"

    // A row as `mailbox::page` holds it (its addresses in `accounts`, for the
    // stand-in's filters), and as it crosses (`outOf`: `mail::RowOut`).
    function keyOf(n, account) {
        return root.prefix + account + "/cur/17770000" + n + ".U7-" + n + ".sioul:2,S"
    }
    function rowOf(n, day, account, subject, extra) {
        const accounts = Array.isArray(account) ? account : [account]
        return Object.assign({ n: n, accounts: accounts, uri: "mid:m" + n + "@example.org", folder: "INBOX", role: "inbox", who: "Ana", address: "ana@example.org", subject: subject, day: day, time: "09:4" + (n % 10), read: true, trust_level: "verified", trust: "Signed by example.org", kinds: ["received"] }, extra || {})
    }
    function keysOf(row) {
        return row.accounts.map(a => root.keyOf(row.n, a))
    }
    function outOf(row) {
        const out = { key: root.keyOf(row.n, row.accounts[0]).slice(root.prefix.length), uri: row.uri, account: row.accounts[0], folder: row.folder, role: row.role, who: row.who, address: row.address, subject: row.subject, day: row.day, time: row.time, trust_level: row.trust_level, trust: row.trust, marks: row.accounts.map(a => String(root.places.indexOf(a))).join(",") }
        if (row.accounts.length > 1)
            out.keys = root.keysOf(row).map(k => k.slice(root.prefix.length)).join("\n")
        for (const flag of ["unread", "read", "flagged", "answered", "attachments", "hostile"])
            if (row[flag] === true)
                out[flag] = true
        if (row.spam)
            out.spam = row.spam
        return out
    }
    // A crowded fortnight, for the pages and the objects: `n` more rows of one address.
    function bulkRows(n) {
        const rows = []
        for (let i = 0; i < n; i++)
            rows.push(root.rowOf(100 + i, i < 20 ? "Today" : i < 60 ? "Yesterday" : "Friday 30 October", "work", "Note " + i, { who: "Sender " + i, unread: i % 3 === 0, read: i % 3 !== 0 }))
        return rows
    }

    // What the stand-in Rust answers: every row, then those the ask keeps.
    readonly property var allRows: [
        rowOf(1, "Today", ["work", "home"], "On both lists", { unread: true, read: false }),
        rowOf(2, "Today", "work", "Quote for the cooperative", { unread: true, read: false }),
        rowOf(3, "Yesterday", "work", "Sent to Iris", { kinds: ["sent"], who: "To Iris" }),
        rowOf(4, "Yesterday", "home", "Dinner on Friday"),
        rowOf(5, "Monday 26 October", "work", "You have won", { kinds: ["spam"], spam: "your provider: spam" })
    ]

    // What the stand-in Rust answers to an ask: the rows it keeps, cut into
    // the pages it publishes (sixty rows each, as many as asked for).
    function pagesFor(ask) {
        const every = root.allRows.concat(root.bulkRows(mock.bulk))
        const rows = every.filter(r => !root.keysOf(r).every(k => mock.gone.indexOf(k) >= 0) && (ask.account === "" || r.accounts.indexOf(ask.account) >= 0)
            && (ask.chip === "spam" ? !!r.spam : !r.spam)
            && (ask.chip !== "sent" || r.kinds.indexOf("sent") >= 0)
            && (ask.chip !== "unread" || !r.read)
            && (ask.account !== "" || ask.shown_resting.indexOf("home") >= 0 || r.accounts.some(a => a !== "home"))
            && (ask.query === "" || r.subject.toLowerCase().indexOf(ask.query.toLowerCase()) >= 0))
        const shown = rows.slice(0, ask.rows || 60)
        const head = { resting: ask.shown_resting.indexOf("home") >= 0 ? [] : [{ account: "home", line: "noa@home.example.org rests until 09:00." }], addresses: [root.marks.work, root.marks.home], earlier: ask.windows < 2, sentence: rows.length === 0 ? "mail-empty" : "", chip: ask.chip, account: ask.account, windows: ask.windows, prefix: root.prefix }
        const pages = []
        for (let start = 0; start === 0 || start < shown.length; start += 60) {
            const part = shown.slice(start, start + 60)
            pages.push(Object.assign({ start: start, rows: part.map(root.outOf), more: start + 60 < shown.length || rows.length > shown.length }, head))
        }
        return pages
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
        property var mailboxAsked: []
        // Rows added to the stand-in's, for the pages and the objects.
        property int bulk: 0
        property int broughtFolders: 0
        property string kept: "{}"
        property var acted: []
        // Keys deleted: their rows leave the next page, as Rust leaves what waits for its Undo.
        property var gone: []
        property var moved: []
        property var openedFolders: []

        signal messageRead(string key, string reading, string invitation)
        signal mailboxPage(string text)
        signal messageFound(int ticket, string answer)
        signal mailMoved(string moves)

        function text(key) {
            return key
        }
        function textWith(key, name, value) {
            return key + ":" + value
        }
        // As Rust: the pages made for the ask, at once; "" asks nothing.
        function openMailbox(ask) {
            mock.mailboxAsked = mock.mailboxAsked.concat([ask])
            if (ask !== "")
                mock.publish(JSON.parse(ask))
        }
        function publish(ask) {
            for (const one of root.pagesFor(ask))
                mock.mailboxPage(JSON.stringify(one))
        }
        function bringFolders() {
            mock.broughtFolders += 1
        }
        function mailboxView() {
            return mock.kept
        }
        function openFolder(account, folder, all, query) {
            mock.openedFolders = mock.openedFolders.concat([account + "/" + folder])
        }
        function actMany(keys, action) {
            mock.acted = mock.acted.concat([{ keys: JSON.parse(keys), action: action }])
            if (action === "trash")
                mock.drop(JSON.parse(keys).reduce((all, k) => all.concat(k), []))
        }
        function act(key, action, target) {
            mock.acted = mock.acted.concat([{ keys: [key], action: action, one: true }])
            if (action === "trash")
                mock.drop([key])
        }
        function drop(keys) {
            mock.gone = mock.gone.concat(keys)
            const asked = mock.mailboxAsked.filter(a => a !== "")
            if (asked.length > 0)
                mock.publish(JSON.parse(asked[asked.length - 1]))
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

        // Each test on a computer's size and the stand-in's own rows, whatever the one before left.
        function init() {
            fakeWindow.compact = false
            mock.bulk = 0
        }

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
            mock.gone = []
            mock.moved = []
            mock.broughtFolders = 0
            mock.mailAccounts = ""
            fakeWindow.asked = []
            const page = createTemporaryObject(mailComponent, fakeWindow, { width: 1100, height: 800 })
            fakeWindow.mailPage = page
            mock.mailAccounts = root.accountsJson
            tryCompare(page, "boxShown", true)
            tryVerify(() => page.box !== null && page.boxRows.length > 0)
            return page
        }

        function boxList(page) {
            return find(page, "mailboxList")
        }

        function boxModel(page) {
            return boxList(page).model
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
            compare((page.marksText(page.listItems[0].marks).match(/<span/g) || []).length, 2, "both addresses' marks, as text")
            verify(page.marksText(page.listItems[0].marks).indexOf("H") > 0, "each its own letter")
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

        function menuItem(page, text) {
            return all(page.Overlay.overlay, i => i.text === text && i.visible && i.triggered !== undefined)[0] || null
        }

        function openMenu(page, subject) {
            tryVerify(() => rowItem(page, subject) !== null)
            const row = rowItem(page, subject)
            mouseClick(row, 120, row.height / 2, Qt.RightButton)
        }

        // A row held by two addresses is one message: its menu's Mark read and
        // Delete, the selection, a drag and the reader act on each address's
        // copy at once; deleted, the row leaves. A row of one copy (Gmail's
        // inbox and All Mail give one key) acts once (review of 863e4c4, M1).
        function test_a_row_s_actions_act_on_each_address() {
            const page = made()
            const both = root.keysOf(root.allRows[0])
            compare(page.rowKeys(page.listItems[0]), both)
            openMenu(page, "On both lists")
            tryVerify(() => menuItem(page, "ui-mark-read") !== null)
            menuItem(page, "ui-mark-read").triggered()
            compare(mock.acted[mock.acted.length - 1], { keys: [both], action: "read" })
            // The selection: the row's copies together, beside a row of one.
            const quote = page.listItems.find(r => r.subject === "Quote for the cooperative")
            page.clickRow(0, page.listItems[0].key, Qt.ControlModifier)
            page.clickRow(1, quote.key, Qt.ControlModifier)
            page.actOnSelection("archive")
            compare(mock.acted[mock.acted.length - 1], { keys: [both, quote.key], action: "archive" })
            // Dragged onto a folder: the row as one, Rust moving one copy of it.
            page.clickRow(0, page.listItems[0].key, Qt.ControlModifier)
            page.startDrag(page.listItems[0].key)
            page.dropOn("work", "Archive")
            compare(mock.moved[mock.moved.length - 1].keys, [both])
            page.letGo()
            // The reader, opened on it: each copy too.
            page.clickRow(0, page.listItems[0].key, 0)
            tryVerify(() => find(page, "mailReader") !== null && find(page, "mailReader").item !== null)
            find(page, "mailReader").act("flag")
            compare(mock.acted[mock.acted.length - 1], { keys: [both], action: "flag" })
            page.back()
            // Deleted from its menu: both copies, and the row leaves.
            openMenu(page, "On both lists")
            tryVerify(() => menuItem(page, "ui-trash") !== null)
            menuItem(page, "ui-trash").triggered()
            compare(mock.acted[mock.acted.length - 1], { keys: [both], action: "trash" })
            tryVerify(() => page.listItems.every(r => r.subject !== "On both lists"))
            // One copy: acted on once, by its key.
            page.showResting("home")
            openMenu(page, "Dinner on Friday")
            tryVerify(() => menuItem(page, "ui-trash") !== null)
            const before = mock.acted.length
            menuItem(page, "ui-trash").triggered()
            compare(mock.acted.length, before + 1)
            compare(mock.acted[before], { keys: [root.keyOf(4, "home")], action: "trash", one: true })
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

        // A phone held sideways (891 by 412, a computer's layout, the page 835 wide beside its rail): the search on a
        // line of its own, the title keeping its words (LB, item 4); wide, on the title's line.
        function test_sideways_the_title_keeps_its_words() {
            const page = made()
            // The window's 891 points less its rail of places.
            page.width = 835
            page.height = 412
            const title = find(page, "mailTitle")
            tryVerify(() => title.width > 0)
            wait(50)
            verify(!title.truncated, "the title whole: " + title.text + " in " + title.width)
            verify(page.searchBelow, "the search under it")
            page.width = 1400
            tryVerify(() => !page.searchBelow)
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

        // "Flag only" in a folder (decided 3 November 2026): a message your filter
        // flagged where it lies says its quiet word beside its subject; one not flagged, nothing.
        function test_a_flagged_message_says_so_in_its_folder() {
            const page = made()
            const row = (n, subject, spam) => ({ key: "/stand-in/mail/work/cur/1777000" + n + ".U7-" + n + ".sioul:2,S", uri: "mid:f" + n + "@example.org", who: "Ben", address: "ben@example.org", subject: subject, date: "09:00", unread: false, read: true, flagged: false, answered: false, attachments: false, trust_level: "verified", trust: "", checks: "", server: false, spam: spam })
            mock.mailFolder = JSON.stringify({ items: [row(1, "A doubt", "maybe spam"), row(2, "A friend", "")], title: "Inbox", sentence: "", role: "inbox", earlier: false })
            page.openFolder("work", "INBOX")
            tryVerify(() => find(page, "folderList").count === 2)
            wait(50)
            const quiet = all(page, c => c.objectName === "folderRowQuiet")
            compare(quiet.map(c => c.visible ? c.text : "").sort(), ["", "maybe spam"])
            mock.mailFolder = ""
        }

        // The list comes by pages of sixty (§6): the first at once, the next as
        // the list's end comes into view, never the list again; "Earlier
        // messages" past the window's end; a page published again changes its
        // rows in place, the list where it was.
        function test_the_list_comes_by_pages() {
            mock.bulk = 130
            const page = made()
            compare(lastAsk().rows, 60)
            verify(lastAsk().fresh, "the first page: new to the list")
            compare([page.boxRows.length, page.boxMore], [60, true])
            verify(!find(page, "mailboxEarlier").visible, "Earlier waits for the window's end")
            const list = boxList(page)
            list.positionViewAtEnd()
            tryCompare(page.boxRows, "length", 120)
            compare([lastAsk().rows, lastAsk().fresh], [120, false])
            list.positionViewAtEnd()
            // Three of the stand-in's rows (the resting address's own left out) and the 130.
            tryVerify(() => page.boxRows.length === 133 && !page.boxMore)
            tryVerify(() => find(page, "mailboxEarlier").visible)
            compare(boxModel(page).count, 133)
            // Page two again, one subject changed: in place, nothing else moves.
            list.positionViewAtIndex(70, ListView.Beginning)
            const y = list.contentY
            const again = root.pagesFor(lastAsk())[1]
            again.rows[0].subject = "Changed in place"
            mock.mailboxPage(JSON.stringify(again))
            compare(page.boxRows[60].subject, "Changed in place")
            compare([boxModel(page).count, list.contentY], [133, y])
            // A page fewer (mail deleted elsewhere): the last page left says no more follow; the list ends there.
            const first = root.pagesFor(Object.assign({}, lastAsk(), { rows: 60 }))[0]
            first.more = false
            mock.mailboxPage(JSON.stringify(first))
            compare([boxModel(page).count, page.boxRows.length, page.boxMore], [60, 60, false])
            mock.bulk = 0
        }

        // On a phone (§4.4, §4.5, §4.8, §6): Mail opens on "All addresses" and asks
        // for the other folders' round once (decision 19); the rows take two
        // lines; at most 12 objects a row and 400 for the whole page; a long press
        // opens the row's menu, Select a selection the bar acts on across
        // addresses; a tap opens the message over the page, Back the list where it was.
        function test_a_phone_s_mailbox() {
            fakeWindow.compact = true
            mock.bulk = 130
            const page = made()
            page.width = 412
            page.height = 891
            compare(mock.broughtFolders, 1, "the other folders asked for, once")
            tryVerify(() => rowItem(page, "On both lists") !== null)
            const row = rowItem(page, "On both lists")
            verify(row.height > 40, "two lines: " + row.height)
            const count = item => {
                let n = 1
                for (let i = 0; i < item.children.length; ++i)
                    n += count(item.children[i])
                return n
            }
            const rows = all(page, i => i.objectName.indexOf("mailboxRow:") === 0)
            verify(rows.length > 5)
            for (const one of rows)
                verify(count(one) <= 12, one.objectName + ": " + count(one))
            verify(count(page) <= 400, "the whole page: " + count(page))
            console.info("sioul-objects: a phone's mailbox page " + count(page) + ", its rows " + rows.length + ", a row at most " + Math.max(...rows.map(count)))
            // A long press: the row's menu, then Select, then a tap chooses another.
            row.pressAndHold()
            tryVerify(() => menuItem(page, "ui-select") !== null)
            menuItem(page, "ui-select").triggered()
            verify(page.choosing)
            const quote = rowItem(page, "Quote for the cooperative")
            quote.clicked()
            compare(page.selectedKeys.length, 2)
            page.actOnSelection("read")
            compare(mock.acted[mock.acted.length - 1], { keys: [root.keysOf(root.allRows[0]), root.keyOf(2, "work")], action: "read" })
            // A tap opens it over the page; Back, the list where it was.
            const list = boxList(page)
            list.positionViewAtIndex(40, ListView.Beginning)
            const y = list.contentY
            page.clickRow(40, page.boxRows[40].key, 0)
            tryVerify(() => page.reading && !list.visible)
            page.back()
            tryVerify(() => list.visible)
            compare(list.contentY, y)
            // The reader goes with Back: the page within its objects again.
            tryVerify(() => count(page) <= 400, 5000, "after Back: " + count(page))
            // Shown again: asked again within the minute, once more (Rust keeps the minute).
            page.visible = false
            page.visible = true
            compare(mock.broughtFolders, 2)
            fakeWindow.compact = false
            mock.bulk = 0
        }

        // On a computer, the other folders are not asked for: its watcher brings them.
        function test_a_computer_asks_no_round() {
            const page = made()
            compare(mock.broughtFolders, 0)
        }

        // On a phone, the folders' column makes its addresses only while it shows (§6).
        function test_the_folders_come_with_their_button() {
            fakeWindow.compact = true
            const page = made()
            compare(all(page, i => i.objectName && i.objectName.indexOf("folder:inbox:") === 0).length, 0)
            page.foldersShown = true
            tryVerify(() => all(page, i => i.objectName && i.objectName.indexOf("folder:inbox:") === 0).length >= 1)
            fakeWindow.compact = false
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
