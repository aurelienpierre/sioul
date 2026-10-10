// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

// The matrix of what reaches you, or a part of it (Settings ▸ What reaches
// you, ReachesTab.qml; the matrix is sioul_core::attention): the rows down,
// in their groups (a channel's people, or Sioul's own kinds), the seven times
// and the two layers across, a mark in each cell (LevelMark.qml). A press on
// a mark opens its choices in words; a fixed mark, greyed, says why; a cell
// changed from As Sioul does now carries a dot. On a screen too narrow for a
// row, its name stands above its marks, and the times' names stand upright
// when even they are too wide (a phone shows lists instead: ReachesTab.qml).
// Each choice is saved at once, the row whole ("attention.<row>").

pragma ComponentBehavior: Bound

import QtQuick
import QtQuick.Controls.Basic
import QtQuick.Layouts

ColumnLayout {
    id: grid

    required property var setting
    required property var sioul
    required property var theme

    // A row's words, nine of them, the one chosen changed.
    // `shown`: the row's words as it showed them, so that only the cell changed is set.
    signal save(string key, var value, var shown)

    // Where a row is saved: "attention." and its id ("attention.mail.safe").
    property string prefix: "attention."
    // The legend's "fixed" mark: shown only where a row has a fixed cell.
    readonly property bool anyFixed: grid.rows.some(r => r.cells.some(c => (c.locked || "") !== ""))

    readonly property var matrix: grid.setting.grid || ({ columns: [], marks: [], rows: [] })
    readonly property var columns: grid.matrix.columns
    readonly property var rows: grid.matrix.rows
    // A mark's cell, as narrow as a finger allows.
    readonly property real cell: 36
    // Each time's name across, as wide as it is; the kinds' names as wide as the widest.
    readonly property var flatWidths: {
        const out = []
        for (let i = 0; i < heads.count; i++) {
            const head = heads.itemAt(i)
            out.push(head ? Math.max(grid.cell, head.implicitWidth + 10) : grid.cell)
        }
        return out
    }
    readonly property real flatWidth: grid.flatWidths.reduce((sum, w) => sum + w, 0)
    readonly property real nameWidth: {
        let width = 60
        for (let i = 0; i < names.count; i++) {
            const name = names.itemAt(i)
            if (name)
                width = Math.max(width, name.implicitWidth + 12)
        }
        return width
    }
    // The kinds' names beside their marks when both fit; else above them.
    readonly property bool beside: grid.width > 0 && grid.nameWidth + grid.flatWidth <= grid.width
    // The times' names upright when even they do not fit across.
    readonly property bool upright: grid.width > 0 && !grid.beside && grid.flatWidth > grid.width
    readonly property var widths: grid.upright ? grid.columns.map(() => grid.cell) : grid.flatWidths
    readonly property real tallest: {
        let height = 0
        for (let i = 0; i < heads.count; i++) {
            const head = heads.itemAt(i)
            if (head)
                height = Math.max(height, head.implicitWidth)
        }
        return height + 6
    }

    // The page's own scrolling (Settings' ScrollView): the times' names are
    // kept in sight while the rows scroll under them (`kept`).
    readonly property var scroller: {
        let item = grid.parent
        while (item && item.contentY === undefined)
            item = item.parent
        return item
    }
    // How far the page has scrolled past the grid's top, in the grid's own
    // terms; 0 while its first line shows. Read again as the page scrolls.
    readonly property real scrolled: {
        const flick = grid.scroller
        if (!flick || !flick.contentItem)
            return 0
        const top = grid.mapToItem(flick.contentItem, 0, 0).y
        return Math.max(0, Math.min(flick.contentY - top, grid.height - headings.height - 60))
    }

    // The cell whose choices are open.
    property var openRow: null
    property var openCell: null
    readonly property var lines: {
        if (!grid.openRow || !grid.openCell)
            return []
        const column = grid.columns.find(c => c.id === grid.openCell.column)
        const out = [{ head: grid.openRow.label + " · " + (column ? column.label : "") }, { note: grid.openRow.help }]
        if (grid.openCell.locked)
            return out.concat([{ note: grid.openCell.locked }])
        return out.concat(grid.openRow.choices.filter(c => grid.openCell.choices.indexOf(c.id) >= 0))
    }

    function open(row, cell, item) {
        grid.openRow = row
        grid.openCell = cell
        choices.popup(item, 0, item.height)
    }

    // The open cell's row, with this value in that cell: saved whole.
    function choose(value) {
        const row = grid.openRow
        const column = grid.openCell.column
        const word = (c, v) => v === "now" ? c.column : c.column + ":" + v
        const words = row.cells.map(c => word(c, c.column === column ? value : c.value))
        grid.save(grid.prefix + row.id, words, row.cells.map(c => word(c, c.value)))
    }

    // A cell's choices opened by its row's and column's ids, and closed: for
    // the documentation's pictures (main.qml's steps "attention").
    function openAt(rowId, columnId) {
        const row = grid.rows.find(r => r.id === rowId)
        const cell = row ? row.cells.find(c => c.column === columnId) : null
        const name = "cell-" + rowId + "-" + columnId
        const find = item => {
            if (!item)
                return null
            if (item.objectName === name)
                return item
            for (let i = 0; i < item.children.length; i++) {
                const found = find(item.children[i])
                if (found)
                    return found
            }
            return null
        }
        const item = find(grid)
        if (row && cell && item)
            grid.open(row, cell, item)
    }

    function closeChoices() {
        choices.close()
    }

    objectName: "attentionGrid"

    spacing: 0

    // Measured once each, for the widths above.
    Repeater {
        id: heads

        model: grid.columns

        delegate: Label {
            required property var modelData

            visible: false
            text: modelData.label
            textFormat: Text.PlainText
            font.pixelSize: 13
        }
    }
    Repeater {
        id: names

        model: grid.rows

        delegate: Label {
            required property var modelData

            visible: false
            text: modelData.label
            textFormat: Text.PlainText
        }
    }

    // The times across, turned upright on a narrow screen: once above the
    // rows, and again over them while the page scrolls past it.
    component Heads: RowLayout {
        id: strip

        // The grid it heads: an inline component sees no id of the file's.
        required property var view

        spacing: 0

        Item {
            visible: strip.view.beside
            Layout.preferredWidth: strip.view.nameWidth
            Layout.preferredHeight: 1
        }
        Repeater {
            model: strip.view.columns

            delegate: Item {
                id: head

                required property var modelData
                required property int index

                Layout.preferredWidth: strip.view.widths[head.index] || strip.view.cell
                Layout.preferredHeight: strip.view.upright ? strip.view.tallest : flat.implicitHeight + 4

                Label {
                    id: flat

                    visible: !strip.view.upright
                    anchors.horizontalCenter: parent.horizontalCenter
                    anchors.bottom: parent.bottom
                    text: head.modelData.label
                    textFormat: Text.PlainText
                    font.pixelSize: 13
                    color: strip.view.theme.muted
                }
                // Read from bottom to top, its end against the marks.
                Label {
                    visible: strip.view.upright
                    x: (head.width - height) / 2
                    y: head.height - 3
                    text: head.modelData.label
                    textFormat: Text.PlainText
                    font.pixelSize: 13
                    color: strip.view.theme.muted
                    transform: Rotation { angle: -90 }
                }
            }
        }
    }


    Heads {
        id: headings

        view: grid
    }

    Repeater {
        model: grid.rows

        delegate: ColumnLayout {
            id: row

            required property var modelData
            required property int index
            readonly property bool newGroup: row.index === 0 || grid.rows[row.index - 1].group !== row.modelData.group

            spacing: 0

            Label {
                visible: row.newGroup
                Layout.fillWidth: true
                Layout.topMargin: row.index === 0 ? 4 : 12
                Layout.bottomMargin: 2
                text: row.modelData.group
                textFormat: Text.PlainText
                font.pixelSize: 13
                font.weight: Font.DemiBold
                wrapMode: Text.Wrap
                color: grid.theme.accent
            }
            // On a narrow screen: the name on its own line.
            Label {
                visible: !grid.beside
                Layout.preferredWidth: Math.max(grid.widths.reduce((sum, w) => sum + w, 0), 1)
                Layout.topMargin: 4
                text: row.modelData.label
                textFormat: Text.PlainText
                elide: Text.ElideRight
                color: grid.theme.text
            }
            RowLayout {
                spacing: 0

                Label {
                    visible: grid.beside
                    Layout.preferredWidth: grid.nameWidth
                    text: row.modelData.label
                    textFormat: Text.PlainText
                    elide: Text.ElideRight
                    color: grid.theme.text
                }
                Repeater {
                    model: row.modelData.cells

                    delegate: AbstractButton {
                        id: box

                        required property var modelData
                        required property int index
                        readonly property bool fixed: (box.modelData.locked || "") !== ""

                        objectName: "cell-" + row.modelData.id + "-" + box.modelData.column
                        Layout.preferredWidth: grid.widths[box.index] || grid.cell
                        implicitHeight: 30
                        hoverEnabled: true
                        Accessible.role: Accessible.Button
                        Accessible.name: box.modelData.said
                        ToolTip.visible: box.hovered
                        ToolTip.text: grid.theme.plain(box.modelData.said)
                        ToolTip.delay: 600

                        background: Rectangle {
                            radius: grid.theme.radius
                            color: box.down ? grid.theme.pressed : box.hovered || box.visualFocus ? grid.theme.hover : "transparent"
                        }
                        contentItem: Item {
                            LevelMark {
                                anchors.centerIn: parent
                                value: box.modelData.value
                                always: row.modelData.id.endsWith(".always")
                                tint: box.fixed ? grid.theme.muted : grid.theme.text
                                hole: grid.theme.background
                                opacity: box.fixed ? 0.6 : 1
                            }
                            // Changed from As Sioul does now: a dot in its corner.
                            Rectangle {
                                visible: box.modelData.changed === true
                                x: parent.width / 2 + 8
                                y: 4
                                width: 5
                                height: 5
                                radius: 2.5
                                color: grid.theme.accent
                            }
                        }
                        onClicked: grid.open(row.modelData, box.modelData, box)
                    }
                }
            }
        }
    }

    // What each mark shown in these rows says, and no other: at once on the
    // Always through row is ★, said apart.
    readonly property var legendMarks: {
        const starred = r => r.id.endsWith(".always")
        const shown = id => grid.rows.some(r => r.cells.some(c => c.value === id && !(id === "now" && starred(r))))
        const used = grid.matrix.marks.filter(m => shown(m.id))
        const always = grid.rows.find(starred)
        const star = always && always.cells.some(c => c.value === "now") ? always.choices.find(c => c.id === "now") : null
        return star ? used.concat([{ id: "now", label: star.label, always: true }]) : used
    }

    Flow {
        Layout.fillWidth: true
        Layout.topMargin: 10
        Layout.bottomMargin: 6
        spacing: 14

        Repeater {
            model: grid.legendMarks

            delegate: RowLayout {
                id: legend

                required property var modelData

                spacing: 5

                LevelMark {
                    value: legend.modelData.id
                    always: legend.modelData.always === true
                    tint: grid.theme.text
                    hole: grid.theme.background
                }
                Label {
                    // A long one wraps under itself on a narrow screen.
                    Layout.maximumWidth: Math.max(120, grid.width - 30)
                    text: legend.modelData.label
                    textFormat: Text.PlainText
                    wrapMode: Text.Wrap
                    font.pixelSize: 12
                    color: grid.theme.muted
                }
            }
        }
        RowLayout {
            visible: grid.anyFixed
            spacing: 5

            LevelMark {
                value: "now"
                tint: grid.theme.muted
                hole: grid.theme.background
                opacity: 0.6
            }
            Label {
                text: grid.sioul.text("attention-legend-fixed")
                font.pixelSize: 12
                color: grid.theme.muted
            }
        }
    }

    // The times' names kept in sight while the rows scroll under them.
    Item {
        id: holder

        Layout.preferredHeight: 0
        z: 2

        Rectangle {
            visible: grid.scrolled > 0
            y: grid.scrolled - holder.y
            width: headings.width
            height: headings.height + 4
            color: grid.theme.background

            Heads {
                y: 2
                view: grid
            }
        }
    }

    // A cell's choices, in words: its row and time, what the row is, then the
    // values it may take, the one it has ticked; fixed, why.
    SioulMenu {
        id: choices

        Instantiator {
            model: grid.lines

            delegate: MenuItem {
                id: line

                required property var modelData
                readonly property bool said: line.modelData.head !== undefined || line.modelData.note !== undefined
                // The value the cell has: said in bold and in the accent, beside its own mark.
                readonly property bool current: !line.said && grid.openCell !== null && grid.openCell.value === line.modelData.id

                enabled: !line.said
                Accessible.checkable: !line.said
                Accessible.checked: line.current
                contentItem: RowLayout {
                    spacing: 8

                    LevelMark {
                        visible: !line.said
                        value: line.said ? "" : line.modelData.id
                        always: grid.openRow !== null && grid.openRow.id.endsWith(".always")
                        tint: line.current ? grid.theme.accent : grid.theme.text
                        hole: grid.theme.surface
                    }
                    Label {
                        // A line of reading length, wrapped; a phone's narrower.
                        Layout.fillWidth: true
                        Layout.maximumWidth: Math.min(420, Math.max(160, (grid.Window.window ? grid.Window.window.width : 400) - 110))
                        text: line.modelData.head !== undefined ? line.modelData.head : line.modelData.note !== undefined ? line.modelData.note : line.modelData.label
                        textFormat: Text.PlainText
                        wrapMode: Text.Wrap
                        font.pixelSize: line.modelData.note !== undefined ? 12 : 14
                        font.weight: line.modelData.head !== undefined || line.current ? Font.DemiBold : Font.Normal
                        color: line.modelData.note !== undefined ? grid.theme.muted : line.current ? grid.theme.accent : grid.theme.text
                    }
                }
                onTriggered: grid.choose(line.modelData.id)
            }
            onObjectAdded: (index, object) => choices.insertItem(index, object)
            onObjectRemoved: (index, object) => choices.removeItem(object)
        }
    }
}
