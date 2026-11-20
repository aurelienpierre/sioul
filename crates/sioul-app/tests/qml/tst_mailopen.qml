// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

// Links that open a message (the unified mailbox's part 0), on a stand-in
// for Sioul: Mail made by the very link that opens it keeps the message open
// when its addresses arrive, at a computer's size and a phone's (MailPage.qml);
// a message that does not read here says so in the reader's place (Reader.qml);
// a note's `mid:` link however spelled asks Rust what it leads to
// (NotesPage.qml, `linkTarget`); a money row opening its message is tested in tst_moneymail.qml.
// Run by tools/qml-test.sh.

import QtQuick
import QtQuick.Controls.Basic
import QtTest
import "../../qml"

Item {
    id: root

    width: 1100
    height: 800

    // Where the pictures of the Notes page without its folder go; none when empty
    // (a change of your own, not to commit).
    readonly property string pictures: ""

    Theme {
        id: testTheme
    }

    // Made up: no real address, no real message.
    readonly property string messageKey: "/stand-in/mail/alice/cur/1777000000.U1-1.sioul:2,S"
    readonly property string accountsJson: JSON.stringify([{ id: "alice", label: "alice@example.org", resting: false, folders: [{ name: "INBOX", display: "Inbox", role: "inbox" }] }])

    QtObject {
        id: mock

        // The notes folder as the backend says it (FolderNeeded.qml); "" not known yet.
        property string notesFolder: ""

        property string status: ""
        property string mailAccounts: ""
        property string mailFolder: ""
        property string drafts: "[]"
        property string mailSearch: ""
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
        // The mailbox (part 4): what it was asked.
        signal mailboxPage(string text)
        function bringFolders() {
        }
        property var mailboxAsked: []
        function openMailbox(ask) {
            mock.mailboxAsked = mock.mailboxAsked.concat([ask])
        }
        function mailboxView() {
            return "{}"
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
        // The words above the attachments, as Rust says them (attachments::label).
        function attachmentsScanWords() {
            return "Each attachment is checked by ClamAV before it opens or is saved."
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

    // A screen of a given width, for a page made in it.
    Component {
        id: folderHolder

        Item {}
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
            mock.mailboxAsked = []
            mock.reading = JSON.stringify({ subject: "The plan", from_name: "Ana", from_address: "ana@example.org", to: [], others: false, text: "Hello", attachments: [], uri: "mid:plan@example.org", flagged: false })
            fakeWindow.opened = []
            fakeWindow.compact = false
        }

        // Mail made by the link that opens a message: its addresses arrive a
        // moment later, and the message stays open, the mailbox beside it, on
        // a computer (part 4) and a phone (part 5).
        function linked(compact) {
            fakeWindow.compact = compact
            const page = createTemporaryObject(mailComponent, root)
            page.openMessage(root.messageKey)
            mock.mailAccounts = root.accountsJson
            tryCompare(page, "boxShown", true)
            compare(mock.openedFolders, [], "no folder")
            compare(JSON.parse(mock.mailboxAsked[0]).chip, "all", "the mailbox, asked")
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

        // Without a link, Mail opens on the mailbox, on a computer and on a
        // phone (decisions 8 and 9; the phone's, part 5); nothing open.
        function test_mail_alone_opens_the_mailbox() {
            const page = createTemporaryObject(mailComponent, root)
            mock.mailAccounts = root.accountsJson
            tryCompare(page, "boxShown", true)
            compare(page.openKey, "")
            fakeWindow.compact = true
            const phone = createTemporaryObject(mailComponent, root)
            mock.mailAccounts = ""
            mock.mailAccounts = root.accountsJson
            tryCompare(phone, "boxShown", true)
            compare([phone.account, phone.openKey], ["", ""])
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
            // Nothing to act on, no empty header card (review of 1017fa4, L6).
            verify(!find(shown, "readerActions").visible, "no action bar")
            verify(!find(shown, "readerHeader").visible, "no header card")
            mock.reading = JSON.stringify({ subject: "Back", from_name: "Ana", from_address: "ana@example.org", to: [], others: false, text: "Hi", attachments: [], uri: "mid:b@example.org", flagged: false })
            shown.key = "/stand-in/mail/alice/cur/2"
            tryCompare(shown, "opening", false)
            verify(!find(shown, "readerNotHere").visible, "a message that reads: no sentence")
            verify(find(shown, "readerActions").visible && find(shown, "readerHeader").visible, "its actions and its header")
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

        // Without its notes folder (none set, or one that does not read), at a computer's
        // width and at a phone's: the folder asked for in the page's place (FolderNeeded.qml),
        // what it is or which one does not read, "Choose a folder…" within the screen, and
        // nothing of the page's own; chosen, the page back.
        function test_without_its_folder_it_asks_for_it_data() {
            return [
                { tag: "unset, computer", state: "unset", width: 1400, compact: false },
                { tag: "unset, phone", state: "unset", width: 412, compact: true },
                { tag: "unreadable, computer", state: "unreadable", width: 1400, compact: false },
                { tag: "unreadable, phone", state: "unreadable", width: 412, compact: true }
            ]
        }
        function test_without_its_folder_it_asks_for_it(data) {
            const all = (item, test, found) => {
                if (!item)
                    return found
                if (test(item))
                    found.push(item)
                const kids = item.children || []
                for (let i = 0; i < kids.length; ++i)
                    all(kids[i], test, found)
                if (item.contentItem && item.contentItem !== item && kids.indexOf(item.contentItem) < 0)
                    all(item.contentItem, test, found)
                return found
            }
            const within = (item, outer) => {
                for (let p = item; p; p = p.parent)
                    if (p === outer)
                        return true
                return false
            }
            mock.notesFolder = JSON.stringify(data.state === "unset" ? { state: "unset" } : { state: "unreadable", path: "/media/stand-in/Notes" })
            if (fakeWindow.compact !== undefined)
                fakeWindow.compact = data.compact
            testTheme.compact = data.compact
            const holder = createTemporaryObject(folderHolder, root, { width: data.width, height: 891 })
            const page = createTemporaryObject(notesComponent, holder)
            waitForRendering(page)
            const needed = all(page, i => i instanceof FolderNeeded, [])[0]
            verify(needed && needed.visible, "asked for in the page's place")
            const said = all(needed, i => i.objectName === "folderNeededSaid", [])[0]
            verify(said.visible && said.text !== "")
            if (data.state === "unreadable")
                verify(said.text.indexOf("/media/stand-in/Notes") >= 0, "the folder that does not read, by its path: " + said.text)
            else
                compare(said.text, mock.text("notes-folder-what"))
            const choose = all(needed, i => i.objectName === "folderNeededChoose", [])[0]
            verify(choose.visible, "Choose a folder…")
            const right = choose.mapToItem(holder, choose.width, 0).x
            verify(right <= data.width + 0.5, "within the screen: " + right)
            const shown = all(page, i => i.visible && typeof i.text === "string" && i.text !== "" && i.width > 0 && !within(i, needed), [])
            compare(shown.map(i => i.text), [], "nothing of the page's own")
            if (root.pictures !== "" && data.state === "unset") {
                let saved = false
                holder.grabToImage(result => saved = result.saveToFile(root.pictures + "/notes-no-folder-" + data.width + ".png"))
                tryVerify(() => saved, 3000)
            }
            // Chosen (as the backend says once it reads it): the page is back.
            mock.notesFolder = JSON.stringify({ state: "ready", path: "/stand-in/Notes" })
            verify(!needed.visible, "the page back")
            mock.notesFolder = ""
            if (fakeWindow.compact !== undefined)
                fakeWindow.compact = false
            testTheme.compact = false
        }
    }
}
