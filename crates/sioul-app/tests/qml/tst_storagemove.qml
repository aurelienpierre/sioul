// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

// The window of the move of Sioul's lists out of the notes folder
// (StorageMove.qml), on a stand-in for Sioul, at a phone's width (412): what
// moves, the two folders, what stays, the copy kept; "Move now" asks the move
// with the folders chosen and the part "projects" when it is off here;
// while a device runs an older Sioul it says which and waits; without
// sharing it says the lists will no longer travel by the notes folder's
// sync; "Later" keeps it from showing by itself again. The system's folder
// chooser itself is not driven here. Made-up paths. Run by
// tools/qml-test.sh.

import QtQuick
import QtQuick.Controls.Basic
import QtTest
import "../../qml"

Item {
    id: root

    width: 412
    height: 800

    Theme {
        id: testTheme

        compact: true
    }

    QtObject {
        id: mock

        property string storageMove: ""
        property var flags: ({})
        property var asked: []

        function text(key) {
            return key
        }
        function textWith(key, name, value) {
            return key + " " + value
        }
        function textArgs(key, args) {
            return key + " " + args
        }
        function viewFlag(name) {
            return mock.flags[name] === true
        }
        function setViewFlag(name, value) {
            const flags = Object.assign({}, mock.flags)
            flags[name] = value
            mock.flags = flags
        }
        function folderCarrier(folder) {
            return folder.indexOf("/Nextcloud/") >= 0 ? "storage-move-carried " + folder : ""
        }
        function moveLists(papers, letters, carry) {
            mock.asked = mock.asked.concat([[papers, letters, carry]])
        }
    }

    function view(more) {
        return JSON.stringify(Object.assign({ moved: false, cleaned: false, moving: false, said: "", open: true, alone: false, sharing: true, waiting: "", unmoved: "", carries: true, notes_carried: false, notes: "/stand-in/Notes", notes_state: "ready", lost: "", held: "", papers: "/stand-in/Documents/Papers", letters: "/stand-in/Documents/Letters", lists: 9, papers_files: 3, letters_files: 2, bytes: 4096, offer: true }, more))
    }

    StorageMove {
        id: move

        sioul: mock
        theme: testTheme
    }

    TestCase {
        name: "StorageMove"
        when: windowShown

        function walk(item, test, found) {
            if (!item)
                return found
            if (test(item))
                found.push(item)
            const kids = item.children || []
            for (let i = 0; i < kids.length; ++i)
                walk(kids[i], test, found)
            if (item.contentItem && item.contentItem !== item && kids.indexOf(item.contentItem) < 0)
                walk(item.contentItem, test, found)
            return found
        }
        function named(name) {
            return walk(move.contentItem, i => i.objectName === name, []).concat(walk(move.footer, i => i.objectName === name, []))[0]
        }

        function init() {
            mock.asked = []
            mock.flags = {}
            move.close()
            tryVerify(() => !move.visible)
        }

        function test_what_moves_where_and_what_stays() {
            mock.storageMove = root.view({})
            move.show()
            tryVerify(() => move.opened)
            verify(move.width <= root.width - 16, "at a phone's width: " + move.width)
            verify(named("storageMoveWhat").visible)
            compare(named("storageMoveWhat").text, "storage-move-what " + JSON.stringify({ lists: 9 }))
            compare(named("storageMoveFolder:papers").text, "/stand-in/Documents/Papers")
            compare(named("storageMoveFolder:letters").text, "/stand-in/Documents/Letters")
            compare(named("storageMoveNotes").text, "storage-move-notes /stand-in/Notes")
            verify(!named("storageMoveWaiting").visible && !named("storageMoveNoSharing").visible && !named("storageMoveCarry").visible)
            verify(named("storageMoveNow").visible && named("storageMoveNow").enabled)
            verify(!named("storageMoveLeft").visible, "nothing left aside")
            mock.storageMove = root.view({ left: ["letters/2026/reply.md", "papers/Mine.pdf"] })
            compare(named("storageMoveLeft").text, "storage-move-left letters/2026/reply.md, papers/Mine.pdf")
        }

        function test_move_now_asks_the_move_with_the_folders_chosen() {
            mock.storageMove = root.view({ carries: false })
            move.show()
            tryVerify(() => move.opened)
            verify(named("storageMoveCarry").visible && named("storageMoveCarry").checked, "the part on, unless unticked")
            const papers = named("storageMoveFolder:papers")
            papers.text = "/stand-in/Elsewhere/Papers"
            papers.editingFinished()
            named("storageMoveNow").clicked()
            compare(mock.asked, [["/stand-in/Elsewhere/Papers", "", true]])
            verify(mock.flags["storage-move-said"] === true, "never shown by itself again")
            verify(move.opened, "it stays to say how the move went")
            // Moving, then done: the window says so; Move now gone.
            mock.storageMove = root.view({ carries: false, moving: true })
            verify(named("storageMoveMoving").visible && !named("storageMoveNow").enabled && !named("storageMoveLater").enabled)
            mock.storageMove = root.view({ moved: true, offer: false })
            verify(named("storageMoveDone").visible && !named("storageMoveNow").visible && !named("storageMoveWhat").visible)
            compare(named("storageMoveLater").text, "ui-close")
        }

        function test_a_folder_a_sync_program_carries_is_said() {
            mock.storageMove = root.view({})
            move.show()
            tryVerify(() => move.opened)
            verify(!named("storageMoveCarried:papers").visible && !named("storageMoveCarried:letters").visible)
            const papers = named("storageMoveFolder:papers")
            papers.text = "/stand-in/Nextcloud/Papers"
            papers.editingFinished()
            verify(named("storageMoveCarried:papers").visible)
            compare(named("storageMoveCarried:papers").text, "storage-move-carried /stand-in/Nextcloud/Papers")
            verify(!named("storageMoveCarried:letters").visible)
        }

        function test_the_scans_changing_part_is_said() {
            mock.storageMove = root.view({})
            move.show()
            tryVerify(() => move.opened)
            verify(!named("storageMoveScans").visible)
            mock.storageMove = root.view({ scans_part: "stop" })
            verify(named("storageMoveScans").visible)
            compare(named("storageMoveScans").text, "storage-move-scans-stop")
            mock.storageMove = root.view({ scans_part: "start" })
            compare(named("storageMoveScans").text, "storage-move-scans-start " + JSON.stringify({ count: 2 }))
        }

        function test_unticked_the_part_stays_as_it_is() {
            mock.storageMove = root.view({ carries: false })
            move.show()
            tryVerify(() => move.opened)
            named("storageMoveCarry").toggle()
            named("storageMoveCarry").toggled()
            named("storageMoveNow").clicked()
            compare(mock.asked, [["", "", false]])
        }

        function test_a_failed_move_is_said_and_may_be_tried_again() {
            mock.storageMove = root.view({})
            move.show()
            tryVerify(() => move.opened)
            named("storageMoveNow").clicked()
            mock.storageMove = root.view({ said: "storage-copy" })
            verify(named("storageMoveSaid").visible)
            compare(named("storageMoveSaid").text, "storage-copy")
            verify(named("storageMoveNow").enabled, "tried again")
        }

        function test_a_notes_folder_not_read_now_holds_the_move() {
            mock.storageMove = root.view({ notes_state: "unreadable", notes: "/stand-in/Unplugged/Notes", offer: false })
            move.show()
            tryVerify(() => move.opened)
            compare(named("storageMoveNotes").text, "storage-notes-unreadable /stand-in/Unplugged/Notes")
            verify(!named("storageMoveNow").enabled, "no move while it does not read")
            mock.storageMove = root.view({ notes_state: "unset", notes: "" })
            compare(named("storageMoveNotes").text, "storage-move-no-notes")
            verify(named("storageMoveNow").enabled, "none set: nothing to move, the switch alone")
        }

        function test_where_the_lists_are_lost_holds_everything() {
            mock.storageMove = root.view({ lost: "storage-state-missing", offer: false })
            move.show()
            tryVerify(() => move.opened)
            verify(named("storageMoveLost").visible)
            compare(named("storageMoveLost").text, "storage-state-missing")
            verify(!named("storageMoveNow").enabled)
        }

        function test_an_older_sioul_holds_the_move_and_is_named() {
            mock.storageMove = root.view({ open: false, offer: false, waiting: "Old laptop" })
            move.show()
            tryVerify(() => move.opened)
            verify(named("storageMoveWaiting").visible)
            compare(named("storageMoveWaiting").text, "storage-waiting Old laptop")
            verify(!named("storageMoveNow").enabled)
        }

        function test_a_sharing_that_cannot_be_read_holds_the_move() {
            mock.storageMove = root.view({ open: false, offer: false, held: "storage-sharing-unreadable" })
            move.show()
            tryVerify(() => move.opened)
            verify(named("storageMoveWaiting").visible)
            compare(named("storageMoveWaiting").text, "storage-sharing-unreadable")
            verify(!named("storageMoveNow").enabled)
        }

        function test_without_sharing_it_says_so_before_the_press() {
            mock.storageMove = root.view({ sharing: false, alone: true, carries: false })
            move.show()
            tryVerify(() => move.opened)
            verify(named("storageMoveNoSharing").visible && !named("storageMoveCarry").visible)
            named("storageMoveNow").clicked()
            compare(mock.asked, [["", "", false]], "no part to switch without sharing")
        }

        function test_later_closes_and_never_offers_again() {
            mock.storageMove = root.view({ unmoved: "Phone" })
            move.show()
            tryVerify(() => move.opened)
            compare(named("storageMoveOthers").text, "storage-move-others Phone")
            named("storageMoveLater").clicked()
            tryVerify(() => !move.opened)
            verify(mock.flags["storage-move-said"] === true)
            compare(mock.asked, [])
        }
    }
}
