// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

// A row of actions (ActionButton.qml, ActionFolds.qml): each with its word.
// On a phone (`folding`) the actions show in their rank while they fit the
// row's room and the others wait in its ⋮ (`folded`), never an icon alone;
// on a computer none folds, what took one press still takes one, and a row
// too narrow wraps its actions onto a second line (`wraps`). Run by
// tools/qml-test.sh.

import QtQuick
import QtQuick.Controls.Basic
import QtTest
import "../../qml"

Item {
    id: root

    width: 800
    height: 400

    Theme {
        id: testTheme
    }

    Component {
        id: rowKind

        Flow {
            id: row

            property alias folds: folds
            property alias reply: reply
            property alias forward: forward
            property alias archive: archive
            property alias trash: trash
            property alias junk: junk

            spacing: 4

            ActionFolds {
                id: folds

                actions: [reply, forward, archive, trash, junk]
                room: row.width
                spacing: row.spacing
            }
            ActionButton {
                id: reply

                theme: testTheme
                folds: folds
                rank: 0
                iconName: "mail-reply-sender"
                label: "Reply"
            }
            ActionButton {
                id: forward

                theme: testTheme
                folds: folds
                rank: 3
                iconName: "mail-forward"
                label: "Forward"
            }
            ActionButton {
                id: archive

                theme: testTheme
                folds: folds
                rank: 1
                iconName: "archive-insert"
                label: "Archive"
            }
            ActionButton {
                id: trash

                theme: testTheme
                folds: folds
                rank: 2
                iconName: "user-trash"
                label: "Delete"
            }
            ActionButton {
                id: junk

                theme: testTheme
                folds: folds
                rank: 4
                offered: false
                iconName: "mail-mark-junk"
                label: "Junk"
            }
        }
    }

    TestCase {
        name: "ActionRow"
        when: windowShown

        // A phone: as many as fit, in their rank; the rest folded, with their words, in their order.
        function test_a_phone_folds_by_rank() {
            const row = createTemporaryObject(rowKind, root, { width: 800 })
            row.folds.folding = true
            const room = row.reply.wideWidth + row.archive.wideWidth + row.spacing + 1
            row.width = room
            tryVerify(() => row.folds.folded.length === 2)
            compare(row.folds.folded.map(a => a.label).join(" "), "Forward Delete", "the rest, in the row's order")
            verify(row.reply.visible && row.archive.visible, "Reply and Archive, the lowest ranks, stay")
            verify(!row.forward.visible && !row.trash.visible, "the others are in ⋮")
            verify(!row.junk.visible, "an action not offered is in neither")
            verify(row.folds.folded.indexOf(row.junk) < 0)
            compare(row.reply.display, AbstractButton.TextBesideIcon, "with its word, never an icon alone")
            // A smaller one never jumps ahead of a lower rank that has no room.
            row.width = row.reply.wideWidth + 2
            tryVerify(() => row.folds.folded.length === 3)
            verify(row.reply.visible && !row.archive.visible)
        }

        // A computer: none folds, every action one press away; too narrow, the row wraps.
        function test_a_computer_wraps_and_never_folds() {
            const row = createTemporaryObject(rowKind, root, { width: 800 })
            row.folds.folding = false
            verify(!row.folds.wraps, "all fit on one line at 800")
            row.width = row.reply.wideWidth + row.archive.wideWidth + row.spacing + 1
            tryVerify(() => row.folds.wraps, 2000, "too narrow: it wraps")
            compare(row.folds.folded.length, 0, "nothing folded")
            verify(row.reply.visible && row.forward.visible && row.archive.visible && row.trash.visible, "each one press away")
            tryVerify(() => row.trash.y > row.reply.y, 2000, "on a second line")
            compare(row.trash.display, AbstractButton.TextBesideIcon, "with its word")
        }
    }
}
