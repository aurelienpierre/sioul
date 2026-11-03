// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

// The mail client, for when you choose to look: the accounts and their
// folders on the left, the main ones in view and the others folded; a folder's
// last two weeks in the middle, without previews, a small dot for the unread;
// a message on the right, the folders folding away while you read. No counts,
// no badges. Right click on a message for what is not in view. "More…" beside
// the search opens the search by conditions in the folders' place
// (MailSearch.qml): its results in the same list, dragged onto folders as any.
// At the top of the
// column, "All addresses": every address's mail in time order (the unified
// mailbox's parts 4 and 5, `mailbox::page`), chosen when Mail opens: one line
// a message on a computer, two on a phone, its rows coming by pages of sixty.

pragma ComponentBehavior: Bound

import QtQuick
import QtQuick.Controls.Basic
import QtQuick.Layouts

Item {
    id: page

    required property var sioul
    required property var theme
    required property var window

    // Read while shown: a page out of sight keeps what it showed, and reads
    // again when it comes back; results landing meanwhile cost nothing.
    property string mailAccountsText: ""
    readonly property var accounts: page.mailAccountsText ? JSON.parse(page.mailAccountsText) : []

    function takeShown() {
        if (page.visible)
            page.draftsText = page.sioul.drafts
        if (page.visible)
            page.mailFolderText = page.sioul.mailFolder
        if (page.visible)
            page.mailAccountsText = page.sioul.mailAccounts
        if (page.visible)
            page.searchText = page.sioul.mailSearch
    }

    Connections {
        target: page.sioul

        function onDraftsChanged() {
            page.takeShown()
        }
        function onMailFolderChanged() {
            page.takeShown()
        }
        function onMailAccountsChanged() {
            page.takeShown()
        }
        function onMailSearchChanged() {
            page.takeShown()
        }
        function onMailboxPage(text) {
            page.applyBoxPage(text)
        }
        function onMailMoved(moves) {
            page.followMoved(JSON.parse(moves || "[]"))
        }
    }
    // Messages Sioul moved whose files followed them into their new folders
    // ([[before, after], …], the unified mailbox's part 3): one a link or the
    // opener opened stays open on its file now; one of the folder shown
    // leaves it with its file, and the reader closes (`onShownChanged`).
    function followMoved(moves) {
        for (const move of moves) {
            if (page.openKey === move[0])
                page.openKey = move[1]
            if (page.answer !== null && page.answer.file === move[0])
                page.answer = Object.assign({}, page.answer, { file: move[1] })
        }
    }
    // The mailbox is made while Mail shows it, and asked for again when Mail
    // comes back: only the pages that changed meanwhile come.
    onVisibleChanged: {
        page.takeShown()
        if (page.boxShown)
            page.sioul.openMailbox(page.visible ? page.boxAsk(false) : "")
        if (page.boxShown && page.visible)
            page.bringFolders()
    }
    Component.onCompleted: {
        // The chip and the address chosen last, on this device.
        const kept = JSON.parse(page.sioul.mailboxView() || "{}")
        page.boxChip = kept.chip || "all"
        page.boxAccount = kept.account || ""
        page.takeShown()
    }
    property string mailFolderText: ""
    readonly property var shown: page.mailFolderText ? JSON.parse(page.mailFolderText) : null
    property string draftsText: ""
    readonly property var drafts: page.draftsText ? JSON.parse(page.draftsText) : []
    property string account: ""
    property string folder: ""
    // Sioul's own drafts, instead of a folder.
    property bool draftsShown: false
    // "All addresses": every address's mail in time order, instead of a folder
    // (`mailbox::page`): its chip (all, unread, sent, spam), one address or
    // all, its two-week windows, the resting addresses you chose to see.
    property bool boxShown: false
    // Its head as its last page said it (`mail::page_texts`): earlier mail
    // kept, its sentence, the resting lines, the marks; its rows in `boxModel`
    // (one ListView reusing its rows, the day as its section) and, as plain
    // rows for the menu, the selection and the opener, `boxRows`. `boxMore`:
    // the window holds more than the rows come, asked for when the list's end
    // shows; `boxWanted`, how many rows are asked for.
    property var box: null
    property var boxRows: []
    property bool boxMore: false
    property int boxWanted: page.boxPage
    readonly property int boxPage: 60
    property string boxChip: "all"
    property string boxAccount: ""
    property int boxWindows: 1
    property var boxShownResting: []
    // The mailbox's own list, not a search's.
    readonly property bool boxListed: page.boxShown && !page.searchOpen && !page.draftsShown
    property bool all: false
    property string query: ""
    // The message being read, by its file, and its row.
    property string openKey: ""
    // A message the window's one opener asked for (`window.openMail`): its
    // ticket, and what Rust found ({state, file, place, reused}: openmail.rs),
    // the reader saying where things stand until a copy opens; null for a
    // message opened from the list.
    property int askedTicket: 0
    property var answer: null
    // A message shown, or asked for: the reader's place taken.
    readonly property bool reading: page.openKey !== "" || page.answer !== null
    // Whether a message was opened since the page was made: its reader is made then.
    property bool readerMade: false
    onOpenKeyChanged: {
        if (page.openKey !== "")
            page.readerMade = true
    }
    // On a phone, one at a time: the folders (asked for), the list, the
    // message; Back goes the other way (main.qml).
    property bool foldersShown: false
    // Every account resting (quiet time) and none chosen: no list, the line saying so.
    readonly property bool allResting: page.account === "" && !page.draftsShown && !page.boxShown && page.accounts.length > 0 && page.accounts.every(a => a.resting)
    readonly property bool canGoBack: page.reading || page.foldersShown || page.searchOpen
    // The search on a line of its own, under the folder's name: on a phone, and
    // beside an open message, where the list's column is narrow; on one line
    // there, "Plus de critères…" and "Temps réel" pushed the row under the
    // reader, the name and the dates cut.
    readonly property bool searchBelow: page.window.compact || page.reading
    // Back (a phone's button, Android's, Escape on the reader): a message the
    // window's opener brought from another page returns there, one level
    // (main.qml, `mailBack`); else Mail's own way: the message, the folders,
    // the search.
    function back() {
        if (page.reading)
            page.leaveMessage()
        else if (page.foldersShown)
            page.foldersShown = false
        else if (page.searchOpen && !page.searchFormShown)
            page.searchFormShown = true
        else if (page.searchOpen)
            page.closeSearch()
    }
    readonly property var opened: findItem(page.openKey)
    // Accounts unfolded by hand; the first one is unfolded at first.
    property var unfolded: ({})
    property var moreShown: ({})
    // Messages chosen with Ctrl or Shift: acted on together, or dragged onto a folder.
    property var selected: ({})
    property int anchorIndex: -1
    readonly property var selectedKeys: Object.keys(page.selected)
    // Conversations unfolded, by their first message.
    property var threadsOpen: ({})
    // Messages being dragged onto a folder.
    property bool dragging: false
    property var dragKeys: []
    // The search by conditions: open (its column in the folders' place, what
    // it finds in the list), made once asked for; on a phone, its fields or
    // its results, one at a time.
    property bool searchOpen: false
    property bool searchMade: false
    property bool searchFormShown: false
    property string searchText: ""
    readonly property var found: page.searchText ? JSON.parse(page.searchText) : null
    // The list's rows: the folder's, or what the search found.
    // A folder's or a search's rows, and what the list shows: the mailbox's when chosen.
    // Each list takes its own rows, never `listItems`: as the mailbox opens or closes,
    // `listItems` may still hold the other list's rows for a moment.
    readonly property var folderItems: page.searchOpen ? (page.found && page.found.items ? page.found.items : []) : (page.shown ? page.shown.items : [])
    readonly property var listItems: page.boxShown && !page.searchOpen ? page.boxRows : page.folderItems
    // What the rows are in: a folder's purpose ("trash"…); none for a search's or the mailbox's, each row its own.
    readonly property string listRole: page.searchOpen || page.boxShown || !page.shown ? "" : page.shown.role
    // A message being brought from its server, opened once here.
    property string bringing: ""
    // A selection begun from a row's menu (on a touch screen, a long press):
    // a tap then chooses, as Ctrl+click does.
    property bool choosing: false
    // Where "Move to…" leaves out: the folder its messages are in, when they share one.
    property string moveFromAccount: ""
    property string moveFromFolder: ""

    function rowShown(item) {
        return !item.member || page.threadsOpen[item.thread] === true
    }

    // A click: alone, it opens the message; with Ctrl (or once a selection was
    // begun from a row's menu), it adds it to the selection or takes it out;
    // with Shift, everything from the last one clicked. A message the search
    // found on its server only is brought here first.
    function clickRow(index, key, modifiers) {
        if ((modifiers & Qt.ControlModifier) || (page.choosing && !(modifiers & Qt.ShiftModifier))) {
            const next = Object.assign({}, page.selected)
            if (next[key])
                delete next[key]
            else
                next[key] = true
            page.selected = next
            page.anchorIndex = index
            page.choosing = page.choosing && Object.keys(next).length > 0
            return
        }
        if ((modifiers & Qt.ShiftModifier) && page.anchorIndex >= 0) {
            const next = {}
            const items = page.listItems
            for (let i = Math.min(page.anchorIndex, index); i <= Math.max(page.anchorIndex, index) && i < items.length; i++)
                if (page.rowShown(items[i]))
                    next[items[i].key] = true
            page.selected = next
            return
        }
        page.selected = ({})
        page.choosing = false
        page.anchorIndex = index
        page.fromLink = false
        // Chosen in Mail's own list: Mail's, its Back Mail's own.
        page.answer = null
        page.window.mailFrom = null
        const item = page.listItems[index]
        if (item && item.server) {
            page.bringing = key
            page.sioul.bringMessage(key)
            return
        }
        // The mailbox's row: through the window's one opener, by its address,
        // its file a hint; Back closes it, the list where it was.
        if (page.boxListed && item && item.uri) {
            page.window.openMail({ address: item.uri, file: key, account: item.account, folder: item.folder })
            return
        }
        page.openKey = key
    }

    // The mailbox as chosen, asked of Rust (`open_mailbox`): `fresh` when the
    // list holds nothing, every page then new to it.
    function boxAsk(fresh) {
        return JSON.stringify({ chip: page.boxChip, account: page.boxAccount, windows: page.boxWindows, query: page.query, shown_resting: page.boxShownResting, rows: page.boxWanted, fresh: fresh === true })
    }

    // Asked anew (another chip, address, search or resting address): the list
    // emptied, its first page asked for.
    function askBoxAgain() {
        boxModel.clear()
        page.boxRows = []
        page.boxMore = false
        page.boxWanted = page.boxPage
        page.sioul.openMailbox(page.boxAsk(true))
    }

    // A row as it crossed (`mail::RowOut`), whole again: its keys under the
    // page's prefix, its flags and words where it left them out.
    function boxRowOf(row, prefix) {
        return {
            key: prefix + row.key,
            keys: row.keys ? row.keys.split("\n").map(k => prefix + k).join("\n") : "",
            uri: row.uri || "", account: row.account || "", folder: row.folder || "", role: row.role || "",
            who: row.who || "", address: row.address || "", subject: row.subject || "", day: row.day || "", time: row.time || "",
            unread: row.unread === true, read: row.read === true, flagged: row.flagged === true, answered: row.answered === true,
            attachments: row.attachments === true, hostile: row.hostile === true, trust_level: row.trust_level || "",
            trust: row.trust || "", spam: row.spam || "", marks: row.marks || "", server: false
        }
    }

    // A page of the mailbox (`mailbox_page`): its rows in their place from
    // `start`; without `more`, what followed goes. The model's rows change in
    // place, so that the list keeps where it was.
    function applyBoxPage(text) {
        const got = JSON.parse(text)
        page.box = { earlier: got.earlier, sentence: got.sentence, resting: got.resting, addresses: got.addresses, chip: got.chip, account: got.account, windows: got.windows }
        const rows = got.rows.map(r => page.boxRowOf(r, got.prefix || ""))
        const start = Math.min(got.start, page.boxRows.length)
        const end = start + rows.length
        const last = end >= page.boxRows.length || !got.more
        page.boxRows = page.boxRows.slice(0, start).concat(rows, got.more ? page.boxRows.slice(end) : [])
        for (let i = 0; i < rows.length; i++) {
            if (start + i < boxModel.count)
                boxModel.set(start + i, rows[i])
            else
                boxModel.append(rows[i])
        }
        if (!got.more && boxModel.count > end)
            boxModel.remove(end, boxModel.count - end)
        if (last)
            page.boxMore = got.more
    }

    // The list's end in sight while the window holds more: the next page.
    function boxMoreRows() {
        if (!page.boxMore || page.boxWanted > page.boxRows.length)
            return
        page.boxWanted = page.boxRows.length + page.boxPage
        page.sioul.openMailbox(page.boxAsk(false))
    }

    // The mailbox shows on a phone: the other folders' round, at once (decision 19).
    function bringFolders() {
        if (page.window.compact)
            page.sioul.bringFolders()
    }

    // "All addresses": the mailbox instead of a folder; `keepOpen` as for a folder.
    function openBox(keepOpen) {
        if (page.searchOpen)
            page.closeSearch()
        page.clearSelection()
        page.anchorIndex = -1
        if (!keepOpen) {
            page.fromLink = false
            page.openKey = ""
            page.answer = null
        }
        page.account = ""
        page.folder = ""
        page.draftsShown = false
        page.all = false
        page.query = ""
        page.boxWindows = 1
        page.boxShownResting = []
        page.foldersShown = false
        page.boxShown = true
        page.askBoxAgain()
        page.bringFolders()
    }

    // New mail's notification of several messages: the mailbox at its top (decision 3).
    function openBoxTop() {
        page.openBox(false)
        boxList.positionViewAtBeginning()
    }

    // A chip: one at a time; the list starts again from its last two weeks.
    function chooseChip(chip) {
        page.boxChip = chip
        page.boxWindows = 1
        page.clearSelection()
        page.askBoxAgain()
    }

    // An address's mark at the chips' end: that address alone; again, all of them.
    function chooseAddress(id) {
        page.boxAccount = page.boxAccount === id ? "" : id
        page.boxWindows = 1
        page.clearSelection()
        page.askBoxAgain()
    }

    // "Show its mail": a resting address's mail all the same, this time ("" for every one).
    function showResting(id) {
        const ids = id === "" ? page.accounts.filter(a => a.resting).map(a => a.id) : [id]
        page.boxShownResting = page.boxShownResting.concat(ids)
        page.askBoxAgain()
    }

    // "Earlier messages": two more weeks, their first page after the rows held.
    function boxEarlier() {
        page.boxWindows = page.boxWindows + 1
        page.boxWanted = page.boxRows.length + page.boxPage
        page.sioul.openMailbox(page.boxAsk(false))
    }

    // A row's menu, "Select": the selection begins with it; then a tap chooses.
    function beginChoosing(key) {
        const next = Object.assign({}, page.selected)
        next[key] = true
        page.selected = next
        page.choosing = true
    }

    function clearSelection() {
        page.selected = ({})
        page.choosing = false
    }

    function selectAll() {
        const next = {}
        for (const item of page.listItems)
            if (page.rowShown(item))
                next[item.key] = true
        page.selected = next
    }

    // What a drag carries: the selection when the row is in it, else the row alone.
    function startDrag(key) {
        page.dragKeys = page.selected[key] ? page.selectedKeys : [key]
        page.dragging = true
    }

    function endDrag() {
        if (!page.dragging)
            return
        ghost.Drag.drop()
        page.dragging = false
    }

    function dropOn(account, folder) {
        page.sioul.moveMessages(JSON.stringify(page.rowsOf(page.dragKeys)), account, folder)
        page.clearSelection()
    }

    // A mailbox row held by several addresses: each one's copy (`Row::keys`),
    // acted on as one message; null for a row of one copy.
    function rowKeys(item) {
        if (!item || !item.keys)
            return null
        const keys = Array.isArray(item.keys) ? item.keys : item.keys.split("\n")
        return keys.length > 1 ? keys : null
    }

    // Rows by their keys, as the actions take them: each mailbox row's copies together.
    function rowsOf(keys) {
        return keys.map(key => page.rowKeys(page.listItems.find(item => page.sameMessage(item.key, key))) || key)
    }

    // The row's menu: on each address's copy of it, as one message.
    function actOnRow(item, action) {
        const keys = page.rowKeys(item)
        if (keys)
            page.sioul.actMany(JSON.stringify([keys]), action)
        else
            page.sioul.act(item.key, action, "")
    }

    // "Move to…" for `keys`: the folder they are in left out of the choices, when they share one.
    function askMove(keys) {
        page.dragKeys = keys
        const items = page.listItems.filter(item => keys.indexOf(item.key) >= 0)
        const one = items.length > 0 && items.every(item => (item.account || page.account) === (items[0].account || page.account) && (item.folder || page.folder) === (items[0].folder || page.folder))
        page.moveFromAccount = one ? (items[0].account || page.account) : ""
        page.moveFromFolder = one ? (items[0].folder || page.folder) : ""
        moveDialog.now().open()
    }

    // For the window's pictures: the first `n` rows chosen, as a long press,
    // "Select", then a tap would.
    function chooseFirst(n) {
        const items = page.listItems
        if (items.length === 0)
            return
        page.beginChoosing(items[0].key)
        for (let i = 1; i < Math.min(n, items.length); i++)
            page.clickRow(i, items[i].key, 0)
    }

    // For the window's pictures: the first message of two addresses chosen, as Ctrl+click would.
    function chooseAcross() {
        const seen = {}
        const next = {}
        for (const item of page.listItems) {
            if (seen[item.account])
                continue
            seen[item.account] = true
            next[item.key] = true
            if (Object.keys(seen).length === 2)
                break
        }
        page.selected = next
    }

    // For the window's pictures: messages held over the first folder of a
    // purpose ("archive"), as a drag holds them; never let go there.
    function holdOver(keys, role) {
        page.dragKeys = keys
        page.dragging = true
        Qt.callLater(() => {
            const row = page.findNamed(folderColumn, "folder:" + role + ":")
            if (row) {
                const at = row.mapToItem(page, Math.round(row.width / 2), Math.round(row.height / 2))
                ghost.x = at.x
                ghost.y = at.y
            }
        })
    }

    // The drag held for a picture ends without a drop: nothing moves.
    function letGo() {
        page.dragging = false
    }

    function closeMove() {
        moveDialog.close()
    }

    function findNamed(item, prefix) {
        if (item.visible && item.objectName && item.objectName.indexOf(prefix) === 0)
            return item
        for (let i = 0; i < item.children.length; i++) {
            const found = page.findNamed(item.children[i], prefix)
            if (found)
                return found
        }
        return null
    }

    // For the window's tests: messages chosen by words of their subjects, as Ctrl+click would.
    function selectBySubjects(words) {
        const next = {}
        for (const item of page.listItems)
            if (words.some(w => item.subject.indexOf(w) >= 0))
                next[item.key] = true
        page.selected = next
    }

    function moveSelection(account, folder) {
        page.dragKeys = page.selectedKeys
        page.dropOn(account, folder)
    }

    function unfoldConversations() {
        const next = {}
        for (const item of page.listItems)
            if (item.size > 1)
                next[item.thread] = true
        page.threadsOpen = next
    }

    function actOnSelection(action) {
        page.sioul.actMany(JSON.stringify(page.rowsOf(page.selectedKeys)), action)
        page.clearSelection()
    }

    // A message keeps the start of its file name when reading or flagging it
    // renames it; one seen on its server only is known by its whole key.
    function sameMessage(a, b) {
        // A row being made again (another layout) may have no key for a moment.
        if (!a || !b)
            return false
        if (a.indexOf("imap://") === 0 || b.indexOf("imap://") === 0)
            return a === b
        return a.split(/[\/\\]/).pop().split(/[:!]/)[0] === b.split(/[\/\\]/).pop().split(/[:!]/)[0]
    }

    function findItem(key) {
        if (!key)
            return null
        for (const item of page.listItems)
            if (page.sameMessage(item.key, key))
                return item
        return null
    }

    // The search opens in the folders' place, with what the quick search held.
    function openSearch() {
        page.searchMade = true
        page.searchOpen = true
        page.searchFormShown = page.window.compact
        page.clearSelection()
        page.openKey = ""
        page.foldersShown = false
        searchLoader.item.start(page.query)
    }

    // The same, its conditions given (the window's pictures): [{field, test, value, until}].
    function searchWith(conditions, any) {
        page.searchMade = true
        page.searchOpen = true
        page.searchFormShown = page.window.compact
        page.clearSelection()
        page.openKey = ""
        searchLoader.item.startWith(conditions, any)
    }

    // "Clear": the folder again.
    function closeSearch() {
        searchWait.stop()
        page.searchOpen = false
        page.searchFormShown = false
        page.clearSelection()
        page.openKey = ""
        page.bringing = ""
        page.sioul.clearMailSearch()
    }

    // A condition changed: the mail here at once, after a short pause in typing.
    function runSearch(now) {
        if (now) {
            searchWait.stop()
            page.sioul.searchMail(searchLoader.item.json(), true)
        } else {
            searchWait.restart()
        }
    }

    // A folder of an address; `keepOpen`: the message a link opened stays open beside it.
    function openFolder(account, folder, keepOpen) {
        if (page.searchOpen)
            page.closeSearch()
        page.clearSelection()
        page.anchorIndex = -1
        if (!keepOpen) {
            page.fromLink = false
            page.openKey = ""
            page.answer = null
        }
        page.account = account
        page.folder = folder
        page.draftsShown = false
        page.boxShown = false
        page.all = false
        page.query = ""
        page.foldersShown = false
        page.sioul.openFolder(account, folder, false, "")
    }

    // The first account open, unless you chose; in quiet time, the first of yours.
    function isUnfolded(index, id) {
        return page.unfolded[id] === undefined ? index === page.accounts.findIndex(a => !a.resting) : page.unfolded[id]
    }

    // What went wrong, in the status line; nothing when all went well.
    function say(line) {
        if (line !== "")
            page.sioul.status = line
    }

    function toggle(map, id, value) {
        const next = Object.assign({}, map)
        next[id] = value
        return next
    }

    // A message opened from a link: it stays open whatever folder is shown.
    property bool fromLink: false

    function openMessage(key) {
        page.fromLink = true
        page.answer = null
        page.openKey = key
    }

    // The window's one opener (`window.openMail`): the reader says "Opening the
    // message…" at once, never the folder's first message; `answered` brings what
    // Rust found under `ticket`.
    function openFound(ticket) {
        page.askedTicket = ticket
        page.fromLink = true
        page.openKey = ""
        page.answer = { state: "opening" }
        page.readerMade = true
    }

    // What the opener found (`message_found`), if it is still the one asked:
    // the servers being asked, the message opening, or why it does not.
    function answered(ticket, answer) {
        if (ticket !== page.askedTicket || page.answer === null)
            return
        page.answer = answer
        if (answer.state === "here" && answer.file !== "")
            page.openKey = answer.file
    }

    // The message left (Back, Escape, filed away): a message the opener brought
    // from another page returns there; else the list again.
    function leaveMessage() {
        const elsewhere = page.answer !== null && page.window.mailFrom !== null
        page.openKey = ""
        page.answer = null
        page.askedTicket = 0
        if (elsewhere)
            page.window.mailBack()
    }

    // A message by its subject, for the window's tests.
    function openSubject(text) {
        for (const item of page.listItems)
            if (item.subject.indexOf(text) >= 0) {
                page.openKey = item.key
                return
            }
    }

    // The first message of the folder, for the window's images.
    function openFirst() {
        if (page.listItems.length > 0)
            page.openKey = page.listItems[0].key
    }

    // The message open with its attachments unfolded and in view, for the window's images.
    function showAttachments() {
        const shown = readerLoader.item as Reader
        if (shown !== null)
            shown.showAttachments()
    }

    // The page's settings, open or not, scrolled to one of them, and their picture: for the window's images.
    function openSettings(open) {
        mailSettings.show(open)
    }

    function scrollSettings(key) {
        mailSettings.scrollTo(key)
    }

    function grabSettings(path) {
        mailSettings.grab(path)
    }

    // Once the bindings reading the accounts are done: the folder opened
    // changes what they read (a binding loop when done at once). The first
    // address's inbox shows beside a message a link opened, never over it:
    // the page made by the very link that opens it (`openThing`) keeps it.
    // On a computer, "All addresses" (decisions 8 and 9); on a phone the
    // first inbox, as before, until the phone's mailbox (part 5).
    // Mail opens on "All addresses", on a computer and a phone (§4.8, decision 8).
    onAccountsChanged: Qt.callLater(() => {
        if (page.account !== "" || page.boxShown || page.draftsShown || page.accounts.length === 0)
            return
        page.openBox(page.fromLink && page.reading)
    })

    // A message filed away leaves the folder: the reader closes with it.
    onShownChanged: {
        if (!page.searchOpen && !page.boxShown && page.openKey !== "" && !page.fromLink && page.shown !== null && page.findItem(page.openKey) === null)
            page.openKey = ""
    }

    // The search's results: a message brought from its server opens; one moved away closes.
    onFoundChanged: {
        if (!page.searchOpen || page.found === null)
            return
        if (page.found.brought && page.found.brought.from === page.bringing) {
            page.bringing = ""
            page.fromLink = false
            page.openKey = page.found.brought.to
        } else if (page.openKey !== "" && !page.fromLink && page.findItem(page.openKey) === null) {
            page.openKey = ""
        }
    }

    // Searching waits for a pause in typing.
    Timer {
        id: searching

        interval: 300
        onTriggered: {
            if (page.boxShown)
                page.askBoxAgain()
            else
                page.sioul.openFolder(page.account, page.folder, page.all, page.query)
        }
    }

    // The search by conditions, too.
    Timer {
        id: searchWait

        interval: 300
        onTriggered: page.sioul.searchMail(searchLoader.item.json(), false)
    }

    RowLayout {
        id: columns

        anchors.fill: parent
        anchors.margins: page.theme.gap
        spacing: page.theme.gap

        // The accounts and their folders; folded away while a message is open.
        ScrollView {
            id: folderColumn

            visible: page.window.compact ? (page.foldersShown || page.allResting) && !page.reading && !page.searchOpen : (!page.reading && !page.searchOpen) || page.dragging
            Layout.fillHeight: true
            Layout.fillWidth: page.window.compact
            // As wide as the search, whose place it takes during a drag: the list does not move.
            Layout.preferredWidth: page.searchOpen ? 290 : 230
            contentWidth: availableWidth

            ColumnLayout {
                width: folderColumn.availableWidth
                spacing: 2

                // Every address's mail in time order, at the top (decisions 8 and 9).
                ItemDelegate {
                    objectName: "mailboxRow"
                    visible: page.accounts.length > 0
                    Layout.fillWidth: true
                    Layout.bottomMargin: 8
                    highlighted: page.boxShown && !page.draftsShown
                    onClicked: page.openBox(false)

                    contentItem: RowLayout {
                        spacing: 8

                        Icon {
                            iconName: "mail-folder-inbox"
                        }
                        Label {
                            Layout.fillWidth: true
                            text: page.sioul.text("mailbox-title")
                            textFormat: Text.PlainText
                            elide: Text.ElideRight
                            color: page.theme.text
                        }
                    }
                }

                // Drafts written here, kept until sent: only when there are some.
                ItemDelegate {
                    id: draftsRow

                    visible: page.drafts.length > 0
                    Layout.fillWidth: true
                    Layout.bottomMargin: 8
                    highlighted: page.draftsShown
                    onClicked: {
                        page.draftsShown = true
                        page.openKey = ""
                        page.foldersShown = false
                    }

                    contentItem: RowLayout {
                        spacing: 8

                        Icon {
                            iconName: "document-edit"
                        }
                        Label {
                            Layout.fillWidth: true
                            text: page.sioul.text("ui-drafts-here")
                            elide: Text.ElideRight
                            color: page.theme.text
                        }
                    }
                }

                // On a phone, made only while it shows (§6: the mailbox's page
                // within its objects; the folders come with the folders button).
                Repeater {
                    model: !page.window.compact || folderColumn.visible ? page.accounts : []

                    delegate: ColumnLayout {
                        id: accountBlock

                        required property var modelData
                        required property int index
                        readonly property bool open: page.isUnfolded(accountBlock.index, accountBlock.modelData.id)

                        Layout.fillWidth: true
                        spacing: 0

                        Button {
                            id: accountTitle

                            Layout.fillWidth: true
                            flat: true
                            onClicked: page.unfolded = page.toggle(page.unfolded, accountBlock.modelData.id, !accountBlock.open)

                            // Dragging over a folded account unfolds it.
                            DropArea {
                                anchors.fill: parent
                                keys: ["sioul-mail"]
                                onEntered: {
                                    if (!accountBlock.open)
                                        page.unfolded = page.toggle(page.unfolded, accountBlock.modelData.id, true)
                                }
                            }

                            contentItem: RowLayout {
                                spacing: 6

                                Label {
                                    text: accountBlock.open ? "▾" : "▸"
                                    color: page.theme.muted
                                }
                                Label {
                                    Layout.fillWidth: true
                                    text: accountBlock.modelData.title
                                    textFormat: Text.PlainText
                                    font.weight: Font.DemiBold
                                    elide: Text.ElideRight
                                    // Work mail in quiet time: there if you look for it.
                                    color: accountBlock.modelData.resting ? page.theme.muted : page.theme.text
                                }
                                // A new folder on its server: its name asked in a small dialog.
                                ToolButton {
                                    visible: accountBlock.open
                                    implicitWidth: 28
                                    implicitHeight: 28
                                    icon.name: "folder-new"
                                    icon.color: page.theme.text
                                    display: AbstractButton.IconOnly
                                    Accessible.name: page.sioul.text("folder-new-button")
                                    ToolTip.visible: hovered
                                    ToolTip.text: page.sioul.text("folder-new-button")
                                    ToolTip.delay: 600
                                    onClicked: newFolderDialog.now().ask(accountBlock.modelData.id, accountBlock.modelData.title)
                                }
                                // Something unread in its inbox, said without a number.
                                Rectangle {
                                    visible: !accountBlock.open && accountBlock.modelData.unread
                                    Layout.preferredWidth: 7
                                    Layout.preferredHeight: 7
                                    radius: 3.5
                                    color: page.theme.accent
                                }
                            }
                        }

                        Repeater {
                            model: accountBlock.open ? accountBlock.modelData.folders.concat(page.moreShown[accountBlock.modelData.id] ? accountBlock.modelData.more : []) : []

                            delegate: ItemDelegate {
                                id: folderRow

                                required property var modelData

                                // Found by the window's pictures: "folder:<purpose>:<account>:<name>".
                                objectName: "folder:" + folderRow.modelData.role + ":" + accountBlock.modelData.id + ":" + folderRow.modelData.name
                                Layout.fillWidth: true
                                leftPadding: 22
                                highlighted: !page.draftsShown && page.account === accountBlock.modelData.id && page.folder === folderRow.modelData.name
                                Accessible.name: folderRow.modelData.title
                                ToolTip.visible: hovered && !folderRow.modelData.kept
                                ToolTip.text: page.sioul.text("folder-server-only")
                                ToolTip.delay: 600
                                onClicked: page.openFolder(accountBlock.modelData.id, folderRow.modelData.name)

                                // Right click: kept here or on the server only; an empty folder of yours deleted.
                                // On a touch screen, the menu comes at a long press; letting go then opens nothing.
                                onPressAndHold: {
                                    folderRow.Window.window.menuAt = folderRow.mapToItem(null, folderRow.pressX, folderRow.pressY)
                                    folderMenu.now().show(accountBlock.modelData.id, folderRow.modelData)
                                }
                                TapHandler {
                                    acceptedButtons: Qt.RightButton
                                    // A touch has no buttons: on a touch screen, the row's long press.
                                    acceptedDevices: PointerDevice.Mouse | PointerDevice.TouchPad
                                    onTapped: folderMenu.now().show(accountBlock.modelData.id, folderRow.modelData)
                                }

                                background: Rectangle {
                                    color: dropHere.containsDrag ? page.theme.hover : folderRow.highlighted ? page.theme.surface : folderRow.hovered ? page.theme.button : "transparent"
                                    radius: page.theme.radius
                                    border.color: dropHere.containsDrag ? page.theme.accent : folderRow.visualFocus ? page.theme.focus : "transparent"
                                    border.width: dropHere.containsDrag ? 2 : 1
                                }

                                // Messages dropped here move here, from this account or another.
                                DropArea {
                                    id: dropHere

                                    anchors.fill: parent
                                    keys: ["sioul-mail"]
                                    onDropped: drop => {
                                        drop.accept()
                                        page.dropOn(accountBlock.modelData.id, folderRow.modelData.name)
                                    }
                                }

                                contentItem: RowLayout {
                                    spacing: 8

                                    Icon {
                                        iconName: folderRow.modelData.icon
                                        size: 16
                                    }
                                    Label {
                                        Layout.fillWidth: true
                                        text: folderRow.modelData.title
                                        textFormat: Text.PlainText
                                        elide: Text.ElideRight
                                        color: folderRow.modelData.kept ? page.theme.text : page.theme.muted
                                    }
                                    Rectangle {
                                        visible: folderRow.modelData.unread
                                        Layout.preferredWidth: 7
                                        Layout.preferredHeight: 7
                                        radius: 3.5
                                        color: page.theme.accent
                                    }
                                }
                            }
                        }

                        Button {
                            visible: accountBlock.open && accountBlock.modelData.more.length > 0
                            Layout.fillWidth: true
                            leftPadding: 22
                            flat: true
                            font.pixelSize: 13
                            text: (page.moreShown[accountBlock.modelData.id] ? "▾  " : "▸  ") + page.sioul.text("ui-more-folders")
                            onClicked: page.moreShown = page.toggle(page.moreShown, accountBlock.modelData.id, !page.moreShown[accountBlock.modelData.id])
                        }
                    }
                }

                Label {
                    visible: page.accounts.length === 0
                    Layout.fillWidth: true
                    text: page.sioul.text("ui-no-mail-accounts")
                    wrapMode: Text.Wrap
                    color: page.theme.muted
                }
            }
        }

        // The search by conditions, in the folders' place: made when first asked for.
        Loader {
            id: searchLoader

            active: page.searchMade
            visible: page.searchOpen && (page.window.compact ? page.searchFormShown && !page.reading : !page.reading && !page.dragging)
            Layout.fillHeight: true
            Layout.fillWidth: page.window.compact
            Layout.preferredWidth: 290

            sourceComponent: Component {
                MailSearch {
                    sioul: page.sioul
                    theme: page.theme
                    compact: page.window.compact
                    filterOffered: typeof page.window.newFilterFrom === "function"
                    onChanged: now => page.runSearch(now)
                    onClearRequested: page.closeSearch()
                    onShowRequested: page.searchFormShown = false
                    onFilterRequested: conditions => page.window.newFilterFrom(conditions)
                }
            }
        }

        // Quiet time, and only work addresses: nothing opens by itself.
        Label {
            // On a phone, the folders take its place: an account can still be opened.
            visible: page.allResting && !page.window.compact && !page.searchOpen
            Layout.fillWidth: true
            Layout.alignment: Qt.AlignTop
            Layout.topMargin: page.theme.gap
            text: page.sioul.text(page.window.moment.sleep === true ? "mail-rest" : "mail-resting")
            wrapMode: Text.Wrap
            color: page.theme.muted
        }

        // The folder's messages, or the drafts.
        ColumnLayout {
            id: listColumn

            visible: !(page.allResting && !page.searchOpen) && !(page.window.compact && (page.reading || page.foldersShown || (page.searchOpen && page.searchFormShown)))
            Layout.fillHeight: true
            Layout.fillWidth: !page.reading
            Layout.minimumWidth: 0
            // Alone, the room the folders leave (its rows' widths never widen the page); beside a message, two fifths.
            Layout.preferredWidth: !page.reading ? 0 : Math.round((columns.width - columns.spacing) * 0.38)
            spacing: 8

            RowLayout {
                Layout.fillWidth: true
                spacing: 8

                ActionButton {
                    visible: page.reading
                    theme: page.theme
                    compact: true
                    iconName: "go-previous"
                    label: page.sioul.text(page.searchOpen ? "search-back" : "ui-folders")
                    onClicked: page.leaveMessage()
                }
                // A phone shows the folders on their own, when asked; the search's fields, while searching.
                ActionButton {
                    visible: page.window.compact && !page.reading
                    theme: page.theme
                    compact: true
                    iconName: page.searchOpen ? "system-search" : "folder-mail"
                    label: page.sioul.text(page.searchOpen ? "search-change" : "ui-folders")
                    onClicked: {
                        if (page.searchOpen)
                            page.searchFormShown = true
                        else
                            page.foldersShown = true
                    }
                }
                ColumnLayout {
                    Layout.fillWidth: true
                    // On a phone, the room left, its words cut: the title never widens the row.
                    Layout.preferredWidth: page.window.compact ? 0 : -1
                    spacing: 0

                    Label {
                        Layout.fillWidth: true
                        // A search's list: "Results", the column beside it being "Search".
                        text: page.draftsShown ? page.sioul.text("ui-drafts-here") : page.searchOpen ? page.sioul.text("search-results") : page.boxShown ? page.sioul.text("mailbox-title") : page.shown ? page.shown.title : ""
                        textFormat: Text.PlainText
                        font.pixelSize: 19
                        elide: Text.ElideRight
                        color: page.theme.text
                    }
                    Label {
                        visible: !page.draftsShown && !page.searchOpen && !page.boxShown
                        Layout.fillWidth: true
                        text: page.account
                        textFormat: Text.PlainText
                        font.pixelSize: 12
                        elide: Text.ElideRight
                        color: page.theme.muted
                    }
                }
                SearchField {
                    visible: !page.draftsShown && !page.searchBelow && !page.searchOpen
                    Layout.fillWidth: true
                    Layout.preferredWidth: 200
                    Layout.maximumWidth: 200
                    Layout.minimumWidth: 90
                }
                // The search by conditions, when needed: in words, out of the way.
                MoreButton {
                    visible: !page.draftsShown && !page.searchBelow && !page.searchOpen
                }
                CheckBox {
                    id: realtime

                    text: page.sioul.text("ui-realtime")
                    checked: page.sioul.realtime
                    ToolTip.visible: hovered
                    ToolTip.text: page.sioul.text("ui-realtime-help")
                    ToolTip.delay: 400
                    onToggled: {
                        page.sioul.setRealtimeMode(checked)
                        realtime.checked = Qt.binding(() => page.sioul.realtime)
                    }
                }
                SettingsButton {
                    id: mailSettings

                    sioul: page.sioul
                    theme: page.theme
                    view: "mail"
                }
            }

            // On a phone, and beside an open message, the search on a line of its own:
            // the folder's name keeps the first (`searchBelow`).
            RowLayout {
                visible: !page.draftsShown && page.searchBelow && !page.searchOpen
                Layout.fillWidth: true
                spacing: 6

                SearchField {
                    Layout.fillWidth: true
                }
                MoreButton {}
            }
            // The mailbox's chips, one at a time, and the addresses' marks at
            // their end: one address alone, then all again. Nothing counted.
            Flow {
                objectName: "mailboxChips"
                visible: page.boxListed
                Layout.fillWidth: true
                // As wide as the list, its chips on two lines if they must: never wider.
                Layout.preferredWidth: 0
                spacing: 6

                Repeater {
                    model: ["all", "unread", "sent", "spam"]

                    delegate: Button {
                        required property string modelData

                        objectName: "chip:" + modelData
                        flat: page.boxChip !== modelData
                        highlighted: page.boxChip === modelData
                        text: page.sioul.text("mailbox-chip-" + modelData)
                        onClicked: page.chooseChip(modelData)
                    }
                }
                Repeater {
                    model: page.box && page.boxListed ? page.box.addresses : []

                    delegate: ToolButton {
                        id: addressChip

                        required property var modelData

                        objectName: "address:" + addressChip.modelData.account
                        checkable: false
                        flat: page.boxAccount !== addressChip.modelData.account
                        Accessible.name: page.sioul.textWith("mailbox-only", "address", addressChip.modelData.title)
                        ToolTip.visible: hovered
                        ToolTip.text: page.theme.plain(page.sioul.textWith("mailbox-only", "address", addressChip.modelData.title))
                        ToolTip.delay: 400
                        onClicked: page.chooseAddress(addressChip.modelData.account)
                        contentItem: AddressMark {
                            mark: addressChip.modelData
                        }
                    }
                }
            }
            // A resting address's line: its mail out of the list, until you ask for it.
            Repeater {
                model: page.box && page.boxListed ? page.box.resting : []

                delegate: RowLayout {
                    id: restingLine

                    required property var modelData

                    Layout.fillWidth: true
                    spacing: 8

                    Label {
                        objectName: "mailboxResting"
                        Layout.fillWidth: true
                        Layout.preferredWidth: 0
                        text: restingLine.modelData.line
                        textFormat: Text.PlainText
                        wrapMode: Text.Wrap
                        color: page.theme.muted
                    }
                    Button {
                        objectName: "mailboxShowResting"
                        flat: true
                        text: page.sioul.text("mailbox-show-resting")
                        onClicked: page.showResting(restingLine.modelData.account)
                    }
                }
            }
            Label {
                visible: !page.draftsShown && (page.searchOpen ? page.found !== null && page.found.sentence !== "" : page.boxShown ? page.box !== null && page.box.sentence !== "" : page.shown !== null && page.shown.sentence !== "")
                Layout.fillWidth: true
                text: page.searchOpen ? (page.found ? page.found.sentence : "") : page.boxShown ? (page.box ? page.box.sentence : "") : page.shown ? page.shown.sentence : ""
                textFormat: Text.PlainText
                wrapMode: Text.Wrap
                color: page.searchOpen ? page.theme.text : page.theme.muted
            }
            // What the search found, here and on the servers, in words.
            Label {
                visible: page.searchOpen && page.found !== null && page.found.status !== ""
                Layout.fillWidth: true
                text: page.found ? page.found.status : ""
                textFormat: Text.PlainText
                wrapMode: Text.Wrap
                color: page.theme.muted
            }

            // What is chosen, and what can be done to all of it at once; on a phone, over two lines.
            Flow {
                visible: !page.draftsShown && page.selectedKeys.length > 0
                Layout.fillWidth: true
                spacing: 6

                Label {
                    height: archiveChosen.height
                    verticalAlignment: Text.AlignVCenter
                    rightPadding: 6
                    text: page.sioul.textWith("ui-selected", "n", String(page.selectedKeys.length))
                    color: page.theme.text
                }
                Button {
                    text: page.sioul.text("ui-mark-read")
                    onClicked: page.actOnSelection("read")
                }
                Button {
                    id: archiveChosen

                    visible: page.listRole !== "archive"
                    text: page.sioul.text("ui-archive")
                    onClicked: page.actOnSelection("archive")
                }
                Button {
                    text: page.listRole === "trash" ? page.sioul.text("ui-delete-for-good") : page.sioul.text("ui-trash")
                    onClicked: page.actOnSelection("trash")
                }
                Button {
                    text: page.sioul.text("ui-move-to")
                    onClicked: page.askMove(page.selectedKeys)
                }
                ToolButton {
                    text: "×"
                    Accessible.name: page.sioul.text("ui-clear-selection")
                    ToolTip.visible: hovered
                    ToolTip.text: page.sioul.text("ui-clear-selection")
                    onClicked: page.clearSelection()
                }
            }

            ListView {
                id: list

                objectName: "folderList"
                visible: !page.draftsShown && !page.boxListed
                Layout.fillWidth: true
                Layout.fillHeight: true
                clip: true
                spacing: 0
                // The mailbox draws its own rows (`boxList`): none made here meanwhile.
                model: page.boxListed ? [] : page.folderItems
                ScrollBar.vertical: ScrollBar {}
                Keys.onPressed: event => {
                    if (event.matches(StandardKey.SelectAll)) {
                        page.selectAll()
                        event.accepted = true
                    } else if (event.key === Qt.Key_Escape && page.selectedKeys.length > 0) {
                        page.clearSelection()
                        event.accepted = true
                    }
                }

                delegate: ItemDelegate {
                    id: row

                    required property var modelData
                    required property int index
                    readonly property bool chosen: page.selected[row.modelData.key] === true
                    readonly property bool threadOpen: page.threadsOpen[row.modelData.thread] === true

                    // Folded inside its conversation: no room taken.
                    visible: page.rowShown(row.modelData)
                    height: visible ? implicitHeight : 0
                    width: list.width - 12
                    highlighted: page.sameMessage(page.openKey, row.modelData.key) || row.chosen
                    // Its words for screen readers: the row has no text of its own.
                    Accessible.name: [row.modelData.who, row.modelData.subject, row.modelData.date].filter(t => !!t).join(", ")
                    padding: 10
                    leftPadding: row.modelData.member ? 40 : 10
                    topInset: 1
                    bottomInset: 1
                    ToolTip.visible: row.modelData.server === true && row.hovered
                    ToolTip.text: page.sioul.text("search-on-server-tip")
                    ToolTip.delay: 800
                    Keys.onReturnPressed: page.clickRow(row.index, row.modelData.key, 0)
                    Keys.onEnterPressed: page.clickRow(row.index, row.modelData.key, 0)
                    // The keyboard's way to the row's menu ("Move to…" among it): the Menu key, Shift+F10.
                    Keys.onPressed: event => {
                        if (event.key === Qt.Key_Menu || (event.key === Qt.Key_F10 && (event.modifiers & Qt.ShiftModifier))) {
                            row.Window.window.menuAt = row.mapToItem(null, 24, row.height / 2)
                            rowMenu.now().show(row.modelData, row.index)
                            event.accepted = true
                        }
                    }

                    background: Rectangle {
                        color: row.chosen ? page.theme.hover : row.highlighted || row.hovered ? page.theme.surface : "transparent"
                        radius: page.theme.radius
                        border.color: row.visualFocus ? page.theme.focus : row.chosen ? page.theme.accent : row.highlighted ? page.theme.line : "transparent"
                        border.width: row.visualFocus ? 2 : 1
                    }

                    TapHandler {
                        acceptedButtons: Qt.LeftButton
                        onTapped: page.clickRow(row.index, row.modelData.key, point.modifiers)
                    }
                    // On a touch screen, the menu comes at a long press; letting go then opens nothing.
                    onPressAndHold: {
                        row.Window.window.menuAt = row.mapToItem(null, row.pressX, row.pressY)
                        rowMenu.now().show(row.modelData, row.index)
                    }
                    TapHandler {
                        acceptedButtons: Qt.RightButton
                        // A touch has no buttons: on a touch screen, the row's long press.
                        acceptedDevices: PointerDevice.Mouse | PointerDevice.TouchPad
                        onTapped: rowMenu.now().show(row.modelData, row.index)
                    }
                    // Dragged sideways, towards the folders: the row, or the selection it is in.
                    // With a mouse only: on a touch screen a drag scrolls the list.
                    DragHandler {
                        id: dragger

                        target: null
                        acceptedDevices: PointerDevice.Mouse | PointerDevice.TouchPad
                        grabPermissions: PointerHandler.CanTakeOverFromAnything
                        onActiveChanged: {
                            if (dragger.active)
                                page.startDrag(row.modelData.key)
                            else
                                page.endDrag()
                        }
                        onCentroidChanged: {
                            if (dragger.active) {
                                const at = row.mapToItem(page, dragger.centroid.position.x, dragger.centroid.position.y)
                                ghost.x = at.x + 12
                                ghost.y = at.y + 12
                            }
                        }
                    }

                    contentItem: ColumnLayout {
                        spacing: 3

                        RowLayout {
                            spacing: 8

                            // A conversation: its size, and the arrow that unfolds it.
                            ToolButton {
                                visible: row.modelData.size > 1
                                Layout.preferredHeight: 22
                                Layout.preferredWidth: implicitContentWidth + 10
                                padding: 2
                                text: page.theme.plain((row.threadOpen ? "▾ " : "▸ ") + row.modelData.size)
                                font.pixelSize: 12
                                Accessible.name: page.sioul.textWith("ui-conversation", "n", String(row.modelData.size))
                                ToolTip.visible: hovered
                                ToolTip.text: page.theme.plain(page.sioul.textWith("ui-conversation", "n", String(row.modelData.size)))
                                ToolTip.delay: 400
                                onClicked: page.threadsOpen = page.toggle(page.threadsOpen, row.modelData.thread, !row.threadOpen)
                            }
                            // Unread: a small dot, not bold.
                            Rectangle {
                                Layout.preferredWidth: 7
                                Layout.preferredHeight: 7
                                radius: 3.5
                                color: row.modelData.unread ? page.theme.accent : "transparent"
                            }
                            Icon {
                                visible: row.modelData.trust_level !== "own"
                                iconName: row.modelData.trust_level === "verified" ? "security-high" : row.modelData.trust_level === "forged" ? "security-low" : "security-medium"
                                size: 16
                                tip: row.modelData.checks
                            }
                            // A message's own words are plain text: a "<" opens no tag, loads nothing.
                            Label {
                                Layout.fillWidth: true
                                text: row.modelData.who
                                textFormat: Text.PlainText
                                elide: Text.ElideRight
                                color: row.modelData.trust_level === "own" && row.modelData.member ? page.theme.muted : page.theme.text
                            }
                            Icon {
                                visible: row.modelData.flagged
                                iconName: "mail-flag"
                                size: 16
                            }
                            Icon {
                                visible: row.modelData.answered
                                iconName: "mail-replied"
                                size: 16
                            }
                            Icon {
                                visible: row.modelData.attachments
                                iconName: "mail-attachment"
                                size: 16
                            }
                            Label {
                                text: row.modelData.date
                                textFormat: Text.PlainText
                                color: page.theme.muted
                                font.pixelSize: 13
                            }
                        }
                        RowLayout {
                            Layout.fillWidth: true
                            Layout.leftMargin: 15
                            spacing: 8

                            Label {
                                Layout.fillWidth: true
                                // The room left: a long subject is cut, never widens the list.
                                Layout.preferredWidth: 0
                                text: row.modelData.subject
                                textFormat: Text.PlainText
                                elide: Text.ElideRight
                                color: page.theme.text
                            }
                            // A search's row: where it is ("Inbox · noa@example.com", "… · on the server").
                            Label {
                                visible: !!row.modelData.place && !page.window.compact
                                Layout.maximumWidth: Math.round(row.width * 0.45)
                                text: row.modelData.place || ""
                                textFormat: Text.PlainText
                                elide: Text.ElideMiddle
                                font.pixelSize: 12
                                color: page.theme.muted
                            }
                        }
                        // On a phone, where it is goes under the subject, whole.
                        Label {
                            visible: !!row.modelData.place && page.window.compact
                            Layout.fillWidth: true
                            Layout.preferredWidth: 0
                            Layout.leftMargin: 15
                            text: row.modelData.place || ""
                            textFormat: Text.PlainText
                            elide: Text.ElideMiddle
                            font.pixelSize: 12
                            color: page.theme.muted
                        }
                    }
                }

                footer: Button {
                    visible: !page.searchOpen && page.shown !== null && page.shown.earlier
                    height: visible ? implicitHeight + 16 : 0
                    flat: true
                    text: page.sioul.text("ui-earlier")
                    onClicked: {
                        page.all = true
                        page.sioul.openFolder(page.account, page.folder, true, page.query)
                    }
                }
            }

            // The mailbox: its rows under their days, one line each on a
            // computer, two on a phone (§4.4); the rows its own (`mailbox::page`,
            // by pages of sixty: `applyBoxPage`), the selection, the menu and the
            // drag as a folder's. One ListView reusing its rows (§6).
            ListView {
                id: boxList

                objectName: "mailboxList"
                visible: page.boxListed
                Layout.fillWidth: true
                Layout.fillHeight: true
                clip: true
                spacing: 0
                model: page.boxListed ? boxModel : null
                reuseItems: true
                delegate: page.window.compact ? twoLines : oneLine
                ScrollBar.vertical: ScrollBar {}
                // The list's end in sight: its next page, while the window holds more.
                onAtYEndChanged: {
                    if (boxList.atYEnd)
                        page.boxMoreRows()
                }
                onCountChanged: {
                    if (boxList.atYEnd)
                        Qt.callLater(page.boxMoreRows)
                }
                Keys.onPressed: event => {
                    if (event.matches(StandardKey.SelectAll)) {
                        page.selectAll()
                        event.accepted = true
                    } else if (event.key === Qt.Key_Escape && page.selectedKeys.length > 0) {
                        page.clearSelection()
                        event.accepted = true
                    }
                }

                section.property: "day"
                section.delegate: Label {
                    required property string section

                    width: boxList.width - 12
                    topPadding: 10
                    bottomPadding: 4
                    leftPadding: 10
                    text: section
                    textFormat: Text.PlainText
                    font.pixelSize: 13
                    font.weight: Font.DemiBold
                    color: page.theme.muted
                }

                footer: Button {
                    objectName: "mailboxEarlier"
                    visible: page.box !== null && page.box.earlier && !page.boxMore
                    height: visible ? implicitHeight + 16 : 0
                    flat: true
                    text: page.sioul.text("ui-earlier")
                    onClicked: page.boxEarlier()
                }
            }

            // Drafts written here: a click opens one again.
            ListView {
                id: draftList

                visible: page.draftsShown
                Layout.fillWidth: true
                Layout.fillHeight: true
                clip: true
                spacing: 2
                model: page.drafts

                delegate: ItemDelegate {
                    id: draftRow

                    required property var modelData

                    width: draftList.width - 12
                    padding: 10
                    Accessible.name: [draftRow.modelData.to, draftRow.modelData.subject].filter(t => !!t).join(", ")
                    onClicked: page.window.openDraft(draftRow.modelData.id)

                    background: Rectangle {
                        color: draftRow.hovered ? page.theme.surface : "transparent"
                        radius: page.theme.radius
                        border.color: draftRow.visualFocus ? page.theme.focus : "transparent"
                    }

                    contentItem: ColumnLayout {
                        spacing: 3

                        RowLayout {
                            spacing: 8

                            Label {
                                Layout.fillWidth: true
                                text: draftRow.modelData.to
                                textFormat: Text.PlainText
                                elide: Text.ElideRight
                                color: page.theme.text
                            }
                            Label {
                                text: draftRow.modelData.date
                                textFormat: Text.PlainText
                                color: page.theme.muted
                                font.pixelSize: 13
                            }
                            Button {
                                flat: true
                                text: page.sioul.text("ui-discard")
                                onClicked: page.sioul.discard(draftRow.modelData.id)
                            }
                        }
                        Label {
                            Layout.fillWidth: true
                            text: draftRow.modelData.subject
                            textFormat: Text.PlainText
                            elide: Text.ElideRight
                            color: page.theme.text
                        }
                    }
                }
            }
        }

        // The reader is made with the first message opened, then kept; on a
        // phone, while a message is open only: the list's page keeps within
        // its objects (§6), the reader made again at the next.
        Loader {
            id: readerLoader

            active: page.readerMade && (!page.window.compact || page.reading)
            visible: page.reading && readerLoader.item !== null && readerLoader.item.showing
            Layout.fillWidth: true
            Layout.fillHeight: true
            Layout.minimumWidth: 0
            Layout.preferredWidth: Math.round((columns.width - columns.spacing) * 0.62)

            sourceComponent: Component {
                Reader {
                    id: reader

                    objectName: "mailReader"
                    sioul: page.sioul
                    theme: page.theme
                    window: page.window
                    key: page.openKey
                    item: page.opened
                    answer: page.answer
                    onCloseRequested: page.leaveMessage()
                }
            }
        }
    }

    // What is being dragged, under the pointer.
    Rectangle {
        id: ghost

        visible: page.dragging
        z: 10
        width: ghostLabel.implicitWidth + 24
        height: ghostLabel.implicitHeight + 12
        radius: page.theme.radius
        color: page.theme.accent
        Drag.active: page.dragging
        Drag.keys: ["sioul-mail"]
        Drag.hotSpot.x: 0
        Drag.hotSpot.y: 0

        Label {
            id: ghostLabel

            anchors.centerIn: parent
            text: page.sioul.textWith("ui-drag-count", "n", String(page.dragKeys.length))
            color: page.theme.accentText
        }
    }

    Later {
        id: moveDialog

        sourceComponent: Component {
            MoveDialog {
                id: moveDialogForm

                sioul: page.sioul
                theme: page.theme
                fromAccount: page.moveFromAccount
                fromFolder: page.moveFromFolder
                onChosen: (account, folder) => page.dropOn(account, folder)
            }
        }
    }

    // Right click on a message: what is not in view.
    // A folder: kept here or on the server only; an empty folder of yours, deleted.
    Later {
        id: folderMenu

        sourceComponent: Component {
            SioulMenu {
                id: folderMenuForm

                property string account: ""
                property var folder: null

                function show(account, folder) {
                    folderMenuForm.account = account
                    folderMenuForm.folder = folder
                    folderMenuForm.popup()
                }

                MenuItem {
                    id: keepItem

                    enabled: folderMenuForm.folder !== null && folderMenuForm.folder.role !== "inbox"
                    text: page.sioul.text("folder-keep")
                    checkable: true
                    checked: folderMenuForm.folder !== null && folderMenuForm.folder.kept
                    onTriggered: {
                        // What the folder is now decides, not the tick the click left; the tick
                        // then says it again (forgetting it may still be cancelled).
                        if (!folderMenuForm.folder.kept)
                            page.say(page.sioul.keepFolder(folderMenuForm.account, folderMenuForm.folder.name, true))
                        else
                            forgetDialog.now().ask(folderMenuForm.account, folderMenuForm.folder)
                        keepItem.checked = Qt.binding(() => folderMenuForm.folder !== null && folderMenuForm.folder.kept)
                    }
                }
                MenuItem {
                    enabled: folderMenuForm.folder !== null && folderMenuForm.folder.role === "other"
                    text: page.sioul.text("folder-delete")
                    onTriggered: deleteDialog.now().ask(folderMenuForm.account, folderMenuForm.folder)
                }
            }
        }
    }

    // Not kept here: the copy goes, the server keeps everything.
    Later {
        id: forgetDialog

        sourceComponent: Component {
            SafeDialog {
                id: forgetDialogForm

                property string account: ""
                property var folder: null

                function ask(account, folder) {
                    forgetDialogForm.account = account
                    forgetDialogForm.folder = folder
                    forgetDialogForm.open()
                }

                parent: Overlay.overlay
                anchors.centerIn: parent
                modal: true
                width: Math.min(440, (parent ? forgetDialogForm.roomWidth : 440) - 2 * page.theme.gap)
                title: forgetDialogForm.folder ? page.theme.plain(forgetDialogForm.folder.title) : ""

                contentItem: Label {
                    text: page.sioul.text("folder-forget-ask")
                    wrapMode: Text.Wrap
                    color: page.theme.text
                }

                footer: DialogButtonBox {
                    Button {
                        text: page.sioul.text("folder-forget")
                        DialogButtonBox.buttonRole: DialogButtonBox.AcceptRole
                    }
                    Button {
                        text: page.sioul.text("ui-cancel")
                        highlighted: true
                        DialogButtonBox.buttonRole: DialogButtonBox.RejectRole
                    }
                }

                onAccepted: page.say(page.sioul.keepFolder(forgetDialogForm.account, forgetDialogForm.folder.name, false))
            }
        }
    }

    // A new folder on an account's server: its name.
    Later {
        id: newFolderDialog

        sourceComponent: Component {
            SafeDialog {
                id: newFolderDialogForm

                property string account: ""
                property string accountTitle: ""

                function ask(account, title) {
                    newFolderDialogForm.account = account
                    newFolderDialogForm.accountTitle = title
                    newFolderName.text = ""
                    newFolderDialogForm.open()
                    newFolderName.forceActiveFocus()
                }

                parent: Overlay.overlay
                anchors.centerIn: parent
                modal: true
                width: Math.min(440, (parent ? newFolderDialogForm.roomWidth : 440) - 2 * page.theme.gap)
                title: page.theme.plain(page.sioul.textWith("folder-new-title", "account", newFolderDialogForm.accountTitle))

                contentItem: TextField {
                    id: newFolderName

                    placeholderText: page.sioul.text("folder-new")
                    onAccepted: newFolderDialogForm.accept()
                }

                footer: DialogButtonBox {
                    Button {
                        text: page.sioul.text("folder-new-make")
                        highlighted: true
                        enabled: newFolderName.text.trim() !== ""
                        DialogButtonBox.buttonRole: DialogButtonBox.AcceptRole
                    }
                    Button {
                        text: page.sioul.text("ui-cancel")
                        DialogButtonBox.buttonRole: DialogButtonBox.RejectRole
                    }
                }

                onAccepted: {
                    if (newFolderName.text.trim() !== "")
                        page.sioul.createFolder(newFolderDialogForm.account, newFolderName.text.trim())
                }
            }
        }
    }

    // An empty folder of yours, taken off the server.
    Later {
        id: deleteDialog

        sourceComponent: Component {
            SafeDialog {
                id: deleteDialogForm

                property string account: ""
                property var folder: null

                function ask(account, folder) {
                    deleteDialogForm.account = account
                    deleteDialogForm.folder = folder
                    deleteDialogForm.open()
                }

                parent: Overlay.overlay
                anchors.centerIn: parent
                modal: true
                width: Math.min(440, (parent ? deleteDialogForm.roomWidth : 440) - 2 * page.theme.gap)
                title: deleteDialogForm.folder ? page.theme.plain(deleteDialogForm.folder.title) : ""

                contentItem: Label {
                    text: page.sioul.text("folder-delete-ask")
                    wrapMode: Text.Wrap
                    color: page.theme.text
                }

                footer: DialogButtonBox {
                    Button {
                        text: page.sioul.text("folder-delete")
                        DialogButtonBox.buttonRole: DialogButtonBox.AcceptRole
                    }
                    Button {
                        text: page.sioul.text("ui-cancel")
                        highlighted: true
                        DialogButtonBox.buttonRole: DialogButtonBox.RejectRole
                    }
                }

                onAccepted: page.sioul.deleteFolder(deleteDialogForm.account, deleteDialogForm.folder.name)
            }
        }
    }

    Later {
        id: rowMenu

        sourceComponent: Component {
            SioulMenu {
                id: rowMenuForm

                property var target: null
                property var source: null
                property int index: -1
                // Seen on its server only: what needs it here waits until it is opened.
                readonly property bool remote: rowMenuForm.target !== null && rowMenuForm.target.server === true
                // Its folder's purpose: the folder's, or, for a search's row, its own.
                readonly property string role: rowMenuForm.target && rowMenuForm.target.role ? rowMenuForm.target.role : page.listRole
                // Not read yet: a mailbox's row says it apart from its dot (`read`), which waits for its time.
                readonly property bool unread: rowMenuForm.target !== null && (rowMenuForm.target.read !== undefined ? !rowMenuForm.target.read : rowMenuForm.target.unread === true)

                function show(item, index) {
                    rowMenuForm.target = item
                    rowMenuForm.index = index
                    rowMenuForm.source = item.server ? null : { uri: page.sioul.uriOf("mail", item.key), kind: "mail", key: item.key, title: item.subject, name: item.who, address: item.address, known: false }
                    rowMenuForm.popup()
                }

                // A selection begins here: then a tap chooses (on a touch screen, after a long press).
                MenuItem {
                    visible: rowMenuForm.target !== null && page.selected[rowMenuForm.target.key] !== true
                    height: visible ? implicitHeight : 0
                    text: page.sioul.text("ui-select")
                    onTriggered: page.beginChoosing(rowMenuForm.target.key)
                }
                MenuItem {
                    visible: !rowMenuForm.remote
                    height: visible ? implicitHeight : 0
                    text: page.sioul.text("ui-reply")
                    onTriggered: page.window.compose("reply", rowMenuForm.target.key)
                }
                MenuItem {
                    visible: !rowMenuForm.remote
                    height: visible ? implicitHeight : 0
                    text: page.sioul.text("ui-forward")
                    onTriggered: page.window.compose("forward", rowMenuForm.target.key)
                }
                MenuSeparator {}
                MenuItem {
                    text: rowMenuForm.unread ? page.sioul.text("ui-mark-read") : page.sioul.text("ui-mark-unread")
                    onTriggered: page.actOnRow(rowMenuForm.target, rowMenuForm.unread ? "read" : "unread")
                }
                MenuItem {
                    text: rowMenuForm.target && rowMenuForm.target.flagged ? page.sioul.text("ui-unflag") : page.sioul.text("ui-flag")
                    onTriggered: page.actOnRow(rowMenuForm.target, rowMenuForm.target.flagged ? "unflag" : "flag")
                }
                MenuSeparator {}
                MenuItem {
                    visible: rowMenuForm.role !== "archive"
                    height: visible ? implicitHeight : 0
                    text: page.sioul.text("ui-archive")
                    onTriggered: page.actOnRow(rowMenuForm.target, "archive")
                }
                MenuItem {
                    text: rowMenuForm.role === "trash" ? page.sioul.text("ui-delete-for-good") : page.sioul.text("ui-trash")
                    onTriggered: page.actOnRow(rowMenuForm.target, "trash")
                }
                MenuItem {
                    visible: !rowMenuForm.remote
                    height: visible ? implicitHeight : 0
                    text: rowMenuForm.role === "junk" ? page.sioul.text("ui-not-junk") : page.sioul.text("ui-junk")
                    onTriggered: page.actOnRow(rowMenuForm.target, rowMenuForm.role === "junk" ? "not-junk" : "junk")
                }
                MenuItem {
                    text: page.sioul.text("ui-move-to")
                    onTriggered: page.askMove(page.selected[rowMenuForm.target.key] ? page.selectedKeys : [rowMenuForm.target.key])
                }
                MenuSeparator {}
                AddMenu {
                    enabled: !rowMenuForm.remote
                    sioul: page.sioul
                    window: page.window
                    source: rowMenuForm.source
                }
                MenuItem {
                    enabled: rowMenuForm.source !== null && rowMenuForm.source.uri !== ""
                    text: page.sioul.text("ui-link-existing")
                    onTriggered: page.window.linkFrom(rowMenuForm.source)
                }
            }
        }
    }

    // The folder's search: in the title's row, or under it on a phone.
    component SearchField: TextField {
        id: searchField

        objectName: "mailSearchField"
        placeholderText: page.sioul.text("ui-search")
        text: page.query
        onTextEdited: {
            page.query = searchField.text
            searching.restart()
        }
    }

    // The mailbox's rows (§4.4, §6). Both read their row from `boxModel`, act
    // on it as `boxRows` gives it, and draw the marks as text, the dot, who,
    // the subject, the quiet marks and the time.
    ListModel {
        id: boxModel
    }

    // An address's marks as text, no item each (§6): its letter on its tint,
    // the addresses by their places in the page's `addresses` ("0,1").
    function marksText(marks) {
        if (!marks || page.box === null || !page.box.addresses)
            return ""
        return marks.split(",").map(i => page.box.addresses[Number(i)]).filter(m => !!m).map(m => "<span style=\"background-color:" + page.theme.tints[m.tint % page.theme.tints.length] + "; color:" + page.theme.text + ";\">&nbsp;" + page.theme.escaped(m.letter) + "&nbsp;</span>").join("&#8201;")
    }

    // Its words for screen readers: a row has no text of its own.
    function boxRowWords(row) {
        const marks = row.marks && page.box !== null ? row.marks.split(",").map(i => (page.box.addresses[Number(i)] || {}).title).filter(t => !!t) : []
        return marks.concat([row.who, row.subject, row.spam, row.time]).filter(t => !!t).join(", ")
    }

    // Its quiet word: spam's, else "forged".
    function quietWord(row) {
        return row.spam !== "" ? row.spam : row.trust_level === "forged" ? row.trust : ""
    }

    // A computer's row: one line, its marks, the dot, who, the subject, its quiet marks, the time.
    Component {
        id: oneLine

        ItemDelegate {
            id: boxRow

            required property var model
            required property int index
            readonly property bool chosen: page.selected[boxRow.model.key] === true

            objectName: "mailboxRow:" + boxRow.model.subject
            width: boxList.width - 12
            highlighted: page.sameMessage(page.openKey, boxRow.model.key) || boxRow.chosen
            Accessible.name: page.boxRowWords(boxRow.model)
            padding: 6
            leftPadding: 8
            rightPadding: 8
            topInset: 1
            bottomInset: 1
            Keys.onReturnPressed: page.clickRow(boxRow.index, boxRow.model.key, 0)
            Keys.onEnterPressed: page.clickRow(boxRow.index, boxRow.model.key, 0)
            Keys.onPressed: event => {
                if (event.key === Qt.Key_Menu || (event.key === Qt.Key_F10 && (event.modifiers & Qt.ShiftModifier))) {
                    boxRow.Window.window.menuAt = boxRow.mapToItem(null, 24, boxRow.height / 2)
                    rowMenu.now().show(page.boxRows[boxRow.index], boxRow.index)
                    event.accepted = true
                }
            }

            background: Rectangle {
                color: boxRow.chosen ? page.theme.hover : boxRow.highlighted || boxRow.hovered ? page.theme.surface : "transparent"
                radius: page.theme.radius
                border.color: boxRow.visualFocus ? page.theme.focus : boxRow.chosen ? page.theme.accent : boxRow.highlighted ? page.theme.line : "transparent"
                border.width: boxRow.visualFocus ? 2 : 1
            }

            TapHandler {
                acceptedButtons: Qt.LeftButton
                onTapped: page.clickRow(boxRow.index, boxRow.model.key, point.modifiers)
            }
            onPressAndHold: {
                boxRow.Window.window.menuAt = boxRow.mapToItem(null, boxRow.pressX, boxRow.pressY)
                rowMenu.now().show(page.boxRows[boxRow.index], boxRow.index)
            }
            TapHandler {
                acceptedButtons: Qt.RightButton
                acceptedDevices: PointerDevice.Mouse | PointerDevice.TouchPad
                onTapped: rowMenu.now().show(page.boxRows[boxRow.index], boxRow.index)
            }
            DragHandler {
                id: boxDragger

                target: null
                acceptedDevices: PointerDevice.Mouse | PointerDevice.TouchPad
                grabPermissions: PointerHandler.CanTakeOverFromAnything
                onActiveChanged: {
                    if (boxDragger.active)
                        page.startDrag(boxRow.model.key)
                    else
                        page.endDrag()
                }
                onCentroidChanged: {
                    if (boxDragger.active) {
                        const at = boxRow.mapToItem(page, boxDragger.centroid.position.x, boxDragger.centroid.position.y)
                        ghost.x = at.x + 12
                        ghost.y = at.y + 12
                    }
                }
            }
            HoverHandler {
                id: marksHover
            }
            ToolTip.visible: marksHover.hovered && boxRow.model.marks.indexOf(",") > 0
            ToolTip.text: page.theme.plain(page.boxRowWords(boxRow.model))
            ToolTip.delay: 900

            contentItem: RowLayout {
                spacing: 8

                Label {
                    visible: text !== ""
                    // rich on purpose: the marks' letters on their tints, each letter one of your own addresses' (`mailbox::letters`), escaped.
                    text: page.marksText(boxRow.model.marks)
                    textFormat: Text.RichText
                    font.pixelSize: 11
                    font.weight: Font.DemiBold
                }
                // Unread, and its time has come: a small dot, not bold.
                Rectangle {
                    Layout.preferredWidth: 7
                    Layout.preferredHeight: 7
                    radius: 3.5
                    color: boxRow.model.unread ? page.theme.accent : "transparent"
                }
                // A message's own words are plain text: a "<" opens no tag, loads nothing.
                Label {
                    Layout.preferredWidth: Math.round(boxRow.availableWidth * 0.24)
                    text: boxRow.model.who
                    textFormat: Text.PlainText
                    elide: Text.ElideRight
                    color: boxRow.model.hostile ? page.theme.muted : page.theme.text
                }
                Label {
                    Layout.fillWidth: true
                    // The room left: a long subject is cut, never widens the list.
                    Layout.preferredWidth: 0
                    text: boxRow.model.subject
                    textFormat: Text.PlainText
                    elide: Text.ElideRight
                    color: boxRow.model.hostile ? page.theme.muted : page.theme.text
                }
                // Quiet marks: answered, an attachment, flagged; spam's word, "forged".
                Icon {
                    visible: boxRow.model.answered
                    iconName: "mail-replied"
                    size: 14
                }
                Icon {
                    visible: boxRow.model.attachments
                    iconName: "mail-attachment"
                    size: 14
                }
                Icon {
                    visible: boxRow.model.flagged
                    iconName: "mail-flag"
                    size: 14
                }
                Label {
                    visible: text !== ""
                    text: page.quietWord(boxRow.model)
                    textFormat: Text.PlainText
                    font.pixelSize: 12
                    color: boxRow.model.trust_level === "forged" ? page.theme.forged : page.theme.muted
                }
                Label {
                    text: boxRow.model.time
                    textFormat: Text.PlainText
                    font.pixelSize: 13
                    color: page.theme.muted
                }
            }
        }
    }

    // A phone's row (§4.4): two lines, the marks, the dot, who and the time on
    // the first, the subject and its quiet marks on the second; a tap opens
    // it over the page, a long press its menu, Select a selection. Twelve
    // items at most (§6): no layout, no handler of its own.
    Component {
        id: twoLines

        ItemDelegate {
            id: phoneRow

            required property var model
            required property int index
            readonly property bool chosen: page.selected[phoneRow.model.key] === true

            objectName: "mailboxRow:" + phoneRow.model.subject
            width: boxList.width
            highlighted: page.sameMessage(page.openKey, phoneRow.model.key) || phoneRow.chosen
            Accessible.name: page.boxRowWords(phoneRow.model)
            padding: 6
            leftPadding: 8
            rightPadding: 8
            topInset: 1
            bottomInset: 1
            onClicked: page.clickRow(phoneRow.index, phoneRow.model.key, 0)
            onPressAndHold: {
                phoneRow.Window.window.menuAt = phoneRow.mapToItem(null, phoneRow.pressX, phoneRow.pressY)
                rowMenu.now().show(page.boxRows[phoneRow.index], phoneRow.index)
            }

            background: Rectangle {
                color: phoneRow.chosen ? page.theme.hover : phoneRow.highlighted || phoneRow.pressed ? page.theme.surface : "transparent"
                radius: page.theme.radius
                border.color: phoneRow.chosen ? page.theme.accent : "transparent"
            }

            contentItem: Item {
                implicitHeight: phoneWho.implicitHeight + phoneSubject.implicitHeight + 3

                Label {
                    id: phoneMarks

                    anchors.left: parent.left
                    anchors.verticalCenter: phoneWho.verticalCenter
                    width: text === "" ? 0 : implicitWidth
                    // rich on purpose: the marks' letters on their tints, each letter one of your own addresses' (`mailbox::letters`), escaped.
                    text: page.marksText(phoneRow.model.marks)
                    textFormat: Text.RichText
                    font.pixelSize: 11
                    font.weight: Font.DemiBold
                }
                // Unread, and its time has come: a small dot, not bold.
                Rectangle {
                    id: phoneDot

                    anchors.left: phoneMarks.right
                    anchors.leftMargin: phoneMarks.width > 0 ? 6 : 0
                    anchors.verticalCenter: phoneWho.verticalCenter
                    width: 7
                    height: 7
                    radius: 3.5
                    color: phoneRow.model.unread ? page.theme.accent : "transparent"
                }
                // A message's own words are plain text: a "<" opens no tag, loads nothing.
                Label {
                    id: phoneWho

                    anchors.left: phoneDot.right
                    anchors.leftMargin: 6
                    anchors.right: phoneTime.left
                    anchors.rightMargin: 8
                    anchors.top: parent.top
                    text: phoneRow.model.who
                    textFormat: Text.PlainText
                    elide: Text.ElideRight
                    color: phoneRow.model.hostile ? page.theme.muted : page.theme.text
                }
                Label {
                    id: phoneTime

                    anchors.right: parent.right
                    anchors.verticalCenter: phoneWho.verticalCenter
                    text: phoneRow.model.time
                    textFormat: Text.PlainText
                    font.pixelSize: 13
                    color: page.theme.muted
                }
                Label {
                    id: phoneSubject

                    anchors.left: phoneWho.left
                    anchors.right: phoneAnswered.left
                    anchors.rightMargin: 6
                    anchors.top: phoneWho.bottom
                    anchors.topMargin: 3
                    text: phoneRow.model.subject
                    textFormat: Text.PlainText
                    elide: Text.ElideRight
                    font.pixelSize: 14
                    color: page.theme.muted
                }
                // Quiet marks, right to left: spam's word or "forged", flagged, an attachment, answered.
                Icon {
                    id: phoneAnswered

                    anchors.right: phoneAttachment.left
                    anchors.rightMargin: visible ? 4 : 0
                    anchors.verticalCenter: phoneSubject.verticalCenter
                    width: visible ? 14 : 0
                    visible: phoneRow.model.answered
                    iconName: "mail-replied"
                    size: 14
                }
                Icon {
                    id: phoneAttachment

                    anchors.right: phoneFlag.left
                    anchors.rightMargin: visible ? 4 : 0
                    anchors.verticalCenter: phoneSubject.verticalCenter
                    width: visible ? 14 : 0
                    visible: phoneRow.model.attachments
                    iconName: "mail-attachment"
                    size: 14
                }
                Icon {
                    id: phoneFlag

                    anchors.right: phoneWord.left
                    anchors.rightMargin: visible ? 4 : 0
                    anchors.verticalCenter: phoneSubject.verticalCenter
                    width: visible ? 14 : 0
                    visible: phoneRow.model.flagged
                    iconName: "mail-flag"
                    size: 14
                }
                Label {
                    id: phoneWord

                    anchors.right: parent.right
                    anchors.verticalCenter: phoneSubject.verticalCenter
                    width: text === "" ? 0 : Math.min(implicitWidth, parent.width * 0.4)
                    text: page.quietWord(phoneRow.model)
                    textFormat: Text.PlainText
                    elide: Text.ElideRight
                    font.pixelSize: 12
                    color: phoneRow.model.trust_level === "forged" ? page.theme.forged : page.theme.muted
                }
            }
        }
    }

    // An address's mark: its first letter on its tint, the whole address said
    // by its tooltip and to screen readers (the unified mailbox's part 4).
    component AddressMark: Rectangle {
        id: markBox

        property var mark: null

        implicitWidth: 16
        implicitHeight: 16
        radius: 4
        color: markBox.mark ? page.theme.tints[markBox.mark.tint % page.theme.tints.length] : "transparent"
        Accessible.name: markBox.mark ? markBox.mark.title : ""
        ToolTip.visible: markHover.hovered
        ToolTip.text: markBox.mark ? page.theme.plain(markBox.mark.title) : ""
        ToolTip.delay: 600

        HoverHandler {
            id: markHover
        }
        Label {
            anchors.centerIn: parent
            text: markBox.mark ? markBox.mark.letter : ""
            textFormat: Text.PlainText
            font.pixelSize: 10
            font.weight: Font.DemiBold
            color: page.theme.text
        }
    }

    // The search by conditions: a word, not an icon, its tip saying what it offers.
    component MoreButton: Button {
        flat: true
        text: page.sioul.text("ui-search-more")
        ToolTip.visible: hovered
        ToolTip.text: page.sioul.text("ui-search-more-tip")
        ToolTip.delay: 600
        Accessible.description: page.sioul.text("ui-search-more-tip")
        onClicked: page.openSearch()
    }
}
