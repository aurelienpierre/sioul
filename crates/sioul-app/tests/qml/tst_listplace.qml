// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

// A list keeps its place (ListPlace.qml; docs/qt-quick.md, "A list keeps its
// place"), in each of its uses: a ListView given whole arrays, a ListView
// whose rows hold rows of their own (a group's), a ScrollView over a
// Repeater, a ListModel changed in place. Each long list scrolled to its
// middle; then a row deleted above the view, below it, the top one itself,
// one added above, below, the rows read again unchanged: the row at the top
// of the view stays there to the pixel, or the next one takes its place
// there when it went. The rows wrap, so that their heights are known only
// once the window has laid them out: the place put back before that is off,
// and the test shows it. Nothing moves while the person drags; another
// filter goes to the top; a row the person makes comes into view by the
// smallest move; a list at its very top stays there. Fiction only. Run by
// tools/qml-test.sh.

import QtQuick
import QtQuick.Controls.Basic
import QtQuick.Layouts
import QtTest
import "../../qml"

Item {
    id: root

    width: 900
    height: 400

    // The rows, `r0` to `r59` but those left out; every third one's words wrap onto several lines.
    function rows(count, without, added) {
        const out = []
        for (let i = 0; i < count; ++i) {
            for (const extra of (added || []).filter(a => a.before === i))
                out.push({ key: extra.key, name: "Added " + extra.key })
            if ((without || []).indexOf("r" + i) < 0)
                out.push({ key: "r" + i, name: "Row " + i + (i % 3 === 0 ? ": a longer line of words, which wraps onto two or three lines in this narrow column" : "") })
        }
        return out
    }
    // The same rows in groups of ten, each group a row of the ListView.
    function groups(flat) {
        const out = []
        for (let i = 0; i < flat.length; i += 10)
            out.push({ title: "Group " + Math.floor(i / 10), rows: flat.slice(i, i + 10) })
        return out
    }

    property var flatRows: root.rows(60)
    property var groupRows: root.groups(root.rows(60))
    property var dayRows: root.rows(60)
    property string flatFilter: ""

    // A ListView given whole arrays, as most of the window's lists are.
    ListPlace {
        id: flatPlace

        view: flatList
        source: root.flatRows
        scope: root.flatFilter
    }
    ListView {
        id: flatList

        width: 200
        height: root.height
        clip: true
        spacing: 2
        cacheBuffer: 300
        model: flatPlace.rows

        delegate: ItemDelegate {
            id: flatRow

            required property var modelData
            readonly property string rowKey: flatRow.modelData.key

            width: flatList.width
            contentItem: RowLayout {
                Label {
                    Layout.fillWidth: true
                    text: flatRow.modelData.name
                    wrapMode: Text.Wrap
                }
            }
        }
    }

    // A ListView of groups, each row's own rows a Repeater's: Tasks ▸ List, the Agenda's list.
    ListPlace {
        id: groupPlace

        view: groupList
        source: root.groupRows
        key: "title"
    }
    ListView {
        id: groupList

        x: 225
        width: 200
        height: root.height
        clip: true
        spacing: 10
        model: groupPlace.rows

        delegate: ColumnLayout {
            id: group

            required property var modelData

            width: groupList.width
            spacing: 2

            Label {
                text: group.modelData.title
                font.bold: true
            }
            Repeater {
                model: group.modelData.rows

                delegate: Label {
                    required property var modelData
                    readonly property string placeKey: modelData.key
                    readonly property string rowKey: modelData.key

                    Layout.fillWidth: true
                    text: modelData.name
                    wrapMode: Text.Wrap
                }
            }
        }
    }

    // A ScrollView over a Repeater, a heading above the rows: the Health page's day, the Porch.
    ScrollView {
        id: dayScroll

        readonly property var day: dayPlace.rows

        x: 450
        width: 200
        height: root.height
        contentWidth: availableWidth
        clip: true

        ListPlace {
            id: dayPlace

            view: dayScroll
            source: root.dayRows
        }

        ColumnLayout {
            width: dayScroll.availableWidth
            spacing: 2

            Label {
                Layout.fillWidth: true
                text: "A heading over the rows, which wraps onto two lines here"
                wrapMode: Text.Wrap
            }
            Repeater {
                model: dayScroll.day

                delegate: ItemDelegate {
                    id: dayRow

                    required property var modelData
                    readonly property string placeKey: dayRow.modelData.key
                    readonly property string rowKey: dayRow.modelData.key

                    Layout.fillWidth: true
                    contentItem: RowLayout {
                        Label {
                            Layout.fillWidth: true
                            text: dayRow.modelData.name
                            wrapMode: Text.Wrap
                        }
                    }
                }
            }
        }
    }

    // A ListModel the page changes in place, row by row from the top (Mail's mailbox, Movements).
    ListModel {
        id: boxModel
    }
    ListPlace {
        id: boxPlace

        view: boxList
        rows: boxModel
    }
    ListView {
        id: boxList

        x: 675
        width: 200
        height: root.height
        clip: true
        spacing: 2
        model: boxModel

        delegate: ItemDelegate {
            id: boxRow

            required property string key
            required property string name
            readonly property string rowKey: boxRow.key

            width: boxList.width
            contentItem: RowLayout {
                Label {
                    Layout.fillWidth: true
                    text: boxRow.name
                    wrapMode: Text.Wrap
                }
            }
        }
    }

    // The rows of `rows` given to `boxModel` by their place, as MailPage.qml's `applyBoxPage` does.
    function fillBox(rows) {
        for (let i = 0; i < rows.length; ++i) {
            if (i < boxModel.count)
                boxModel.set(i, rows[i])
            else
                boxModel.append(rows[i])
        }
        if (boxModel.count > rows.length)
            boxModel.remove(rows.length, boxModel.count - rows.length)
    }

    // Where the row at the top was when the person's movement ended, before the waiting rows came.
    property Item watched: null
    property var stoppedAt: null

    Connections {
        target: root.watched

        function onMovementEnded() {
            root.stoppedAt = test.topRow(root.watched)
        }
    }

    TestCase {
        id: test

        name: "ListPlace"
        when: windowShown

        // Every row shown under `item` (its `rowKey`), with where its top is in `flick`'s view.
        function shown(item, flick, out) {
            out = out || []
            const kids = item.children || []
            for (let i = 0; i < kids.length; ++i) {
                const kid = kids[i]
                if (!kid.visible)
                    continue
                if (kid.rowKey !== undefined)
                    out.push({ key: kid.rowKey, y: kid.mapToItem(flick, 0, 0).y, height: kid.height })
                else
                    shown(kid, flick, out)
            }
            return out
        }

        // The row at the top of the view: its key and how far its top is from the view's top.
        function topRow(flick) {
            const rows = shown(flick.contentItem, flick).filter(r => r.y + r.height > 0 && r.y < flick.height)
            rows.sort((a, b) => a.y - b.y)
            return rows.length > 0 ? { key: rows[0].key, y: rows[0].y } : null
        }

        // A row's place in the view, by its key.
        function placeOf(flick, key) {
            const row = shown(flick.contentItem, flick).find(r => r.key === key)
            return row ? row.y : null
        }

        // The next frame drawn, asked for: a change that draws nothing new would otherwise leave
        // waitForRendering waiting its whole timeout.
        function frame(item) {
            item.Window.window.update()
            waitForRendering(item)
        }

        function flickOf(view) {
            return view.contentItem.contentY !== undefined ? view.contentItem : view
        }

        // The view scrolled to its middle, the top row cut a little.
        function toMiddle(flick) {
            flick.contentY = flick.originY + (flick.contentHeight - flick.height) / 2
            frame(flick)
            const top = topRow(flick)
            flick.contentY += top.y + 7
            frame(flick)
            return topRow(flick)
        }

        function init() {
            root.flatFilter = ""
            root.flatRows = root.rows(60)
            root.groupRows = root.groups(root.rows(60))
            root.dayRows = root.rows(60)
            root.fillBox(root.rows(60))
            wait(0)
            frame(root)
        }

        // Each change, for each kind of list: the row at the top stays at its pixel, else the next one.
        function test_the_top_row_stays_data() {
            const changes = [
                { tag: "deleted above", without: ["r5"], stays: "top" },
                { tag: "deleted below", without: ["r58"], stays: "top" },
                { tag: "the top one deleted", without: ["top"], stays: "next" },
                { tag: "added above", added: [{ key: "n1", before: 3 }], stays: "top" },
                { tag: "added below", added: [{ key: "n2", before: 57 }], stays: "top" },
                { tag: "read again unchanged", stays: "top" },
                { tag: "several at once", without: ["r1", "r2", "r40"], added: [{ key: "n3", before: 0 }, { key: "n4", before: 10 }], stays: "top" }
            ]
            const out = []
            for (const kind of ["flat", "groups", "day", "box"])
                for (const change of changes)
                    out.push(Object.assign({ kind: kind }, change, { tag: kind + ": " + change.tag }))
            return out
        }

        function test_the_top_row_stays(data) {
            const view = { flat: flatList, groups: groupList, day: dayScroll, box: boxList }[data.kind]
            const flick = flickOf(view)
            const before = toMiddle(flick)
            verify(before !== null && before.key !== "r0", "scrolled into the list")
            const index = Number(before.key.slice(1))
            const without = (data.without || []).map(k => k === "top" ? before.key : k)
            const next = "r" + (index + 1)
            const rows = root.rows(60, without, data.added)
            if (data.kind === "flat")
                root.flatRows = rows
            else if (data.kind === "groups")
                root.groupRows = root.groups(rows)
            else if (data.kind === "day")
                root.dayRows = rows
            else
                boxPlace.keep(() => root.fillBox(rows))
            wait(0)
            frame(flick)
            if (data.stays === "top") {
                const after = topRow(flick)
                compare(after.key, before.key, "the same row at the top")
                compare(after.y, before.y, "at the same pixel")
            } else {
                compare(placeOf(flick, before.key), null, "the top row gone")
                compare(topRow(flick).key, next, "the next one at the top")
                compare(topRow(flick).y, before.y, "in the top one's place, to the pixel")
            }
        }

        // Without the putting back after the window has laid the new rows out, the place is off
        // by what the wrapping rows above learned only then: that putting back is what keeps it.
        function test_the_place_is_put_back_after_the_layout() {
            const flick = flickOf(dayScroll)
            const before = toMiddle(flick)
            root.dayRows = root.rows(60, ["r3", "r6", "r9"])
            // Given now, without waiting for the event loop (which may draw a frame meanwhile).
            dayPlace.apply()
            verify(dayPlace.settling !== null, "the putting back waits for the window's layout")
            dayPlace.settling = null
            frame(flick)
            const once = placeOf(flick, before.key)
            verify(Math.abs(once - before.y) > 4, "without it: off by " + (once - before.y))
            // Again, both times.
            toMiddle(flick)
            const again = topRow(flick)
            root.dayRows = root.rows(60, ["r3", "r6", "r9", "r12"])
            wait(0)
            frame(flick)
            compare(placeOf(flick, again.key), again.y, "after the layout: at its pixel")
        }

        // While the person drags the list, nothing moves under the finger: the new rows wait until
        // the movement is over, then come with the place kept.
        function test_nothing_moves_while_dragged_data() {
            return [{ tag: "flat", kind: "flat" }, { tag: "day", kind: "day" }, { tag: "box", kind: "box" }]
        }

        function test_nothing_moves_while_dragged(data) {
            const view = { flat: flatList, day: dayScroll, box: boxList }[data.kind]
            const place = { flat: flatPlace, day: dayPlace, box: boxPlace }[data.kind]
            const flick = flickOf(view)
            toMiddle(flick)
            const x = flick.width / 2
            const y = flick.height / 2
            const touch = touchEvent(flick)
            touch.press(0, flick, x, y).commit()
            for (let step = 1; step <= 6; ++step) {
                touch.move(0, flick, x, y - step * 10).commit()
                wait(16)
            }
            verify(flick.dragging, "dragged")
            const held = topRow(flick)
            const contentY = flick.contentY
            if (data.kind === "flat")
                root.flatRows = root.rows(60, ["r1", "r2", "r3"])
            else if (data.kind === "day")
                root.dayRows = root.rows(60, ["r1", "r2", "r3"])
            else
                boxPlace.keep(() => root.fillBox(root.rows(60, ["r1", "r2", "r3"])))
            wait(0)
            verify(place.waiting, "the new rows wait")
            compare(flick.contentY, contentY, "the view not moved")
            compare(topRow(flick).key, held.key)
            compare(topRow(flick).y, held.y, "nothing moved under the finger")
            root.stoppedAt = null
            root.watched = flick
            touch.release(0, flick, x, y - 60).commit()
            tryCompare(flick, "moving", false, 5000)
            verify(root.stoppedAt !== null, "its end seen")
            wait(0)
            verify(!place.waiting, "given once the movement is over")
            frame(flick)
            root.watched = null
            verify(placeOf(flick, "r1") === null, "the new rows shown")
            compare(topRow(flick).key, root.stoppedAt.key, "the row at the top when it stopped stays")
            compare(topRow(flick).y, root.stoppedAt.y)
        }

        // Another filter is another list: it starts at its top.
        function test_another_filter_goes_to_the_top() {
            toMiddle(flatList)
            root.flatFilter = "only some"
            root.flatRows = root.rows(60, ["r0", "r1", "r2", "r4"])
            wait(0)
            frame(flatList)
            compare(topRow(flatList).key, "r3")
            compare(topRow(flatList).y, 0, "at its top")
        }

        // A row the person made from this list, out of sight, comes into view by the smallest move;
        // one made in sight moves nothing.
        function test_a_row_made_here_comes_into_view_data() {
            return [{ tag: "flat", kind: "flat" }, { tag: "day", kind: "day" }]
        }

        function test_a_row_made_here_comes_into_view(data) {
            const view = data.kind === "flat" ? flatList : dayScroll
            const place = data.kind === "flat" ? flatPlace : dayPlace
            const flick = flickOf(view)
            const before = toMiddle(flick)
            const set = rows => {
                if (data.kind === "flat")
                    root.flatRows = rows
                else
                    root.dayRows = rows
            }
            // Below the view: brought up until its bottom shows, no further.
            place.reveal("made1")
            set(root.rows(60, [], [{ key: "made1", before: 55 }]))
            wait(0)
            frame(flick)
            const at = placeOf(flick, "made1")
            verify(at !== null, "in view")
            const row = shown(flick.contentItem, flick).find(r => r.key === "made1")
            verify(Math.abs(at + row.height - flick.height) < 1, "its bottom at the view's bottom: " + (at + row.height) + " / " + flick.height)
            compare(place.wanted, "", "done")
            // In sight: nothing moves.
            const top = topRow(flick)
            const inSight = shown(flick.contentItem, flick).filter(r => r.y > 50 && r.y + r.height < flick.height - 10)[0]
            place.reveal("made2")
            set(root.rows(60, [], [{ key: "made1", before: 55 }, { key: "made2", before: Number(inSight.key.slice(1)) }]))
            wait(0)
            frame(flick)
            compare(topRow(flick).key, top.key, "nothing moved")
            compare(topRow(flick).y, top.y)
            verify(placeOf(flick, "made2") !== null, "made in sight")
        }

        // At its very top, a list stays there: a row added first shows.
        function test_at_its_top_it_stays_there() {
            flatList.positionViewAtBeginning()
            frame(flatList)
            root.flatRows = root.rows(60, [], [{ key: "first", before: 0 }])
            wait(0)
            frame(flatList)
            compare(topRow(flatList).key, "first")
            compare(topRow(flatList).y, 0)
        }
    }
}
