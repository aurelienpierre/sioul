// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

// What each kind of notification does at each time (Settings ▸ Reminders and
// notifications; the matrix is sioul_core::notify): the kinds down, in their
// groups, the times across, a mark in each cell. A press on a mark opens its
// choices in words; a fixed mark, greyed, says why. Drawn as the grid of who
// may reach you (SettingRow.qml): on a screen too narrow for a row, its name
// stands above its marks, and the times' names stand upright when even they
// are too wide. Each choice is saved at once, the row whole ("notify.<row>").

pragma ComponentBehavior: Bound

import QtQuick
import QtQuick.Controls.Basic
import QtQuick.Controls.impl
import QtQuick.Layouts

ColumnLayout {
    id: grid

    required property var setting
    required property var sioul
    required property var theme

    // A row's words, nine of them, the one chosen changed.
    signal save(string key, var value)

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
        const words = row.cells.map(c => {
            const v = c.column === column ? value : c.value
            return v === "now" ? c.column : c.column + ":" + v
        })
        grid.save("notify." + row.id, words)
    }

    // A cell's choices opened by its row's and column's ids, and closed: for
    // the documentation's pictures (main.qml's steps "notify").
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

    objectName: "notifyGrid"

    spacing: 0

    // A value's mark: ● at once, ○ later, ◎ at the gathered times, ◐ when its
    // event falls then, – not at all, ★ and ☆ do-not-disturb's list (whatever
    // the grid of who may reach you, or with it). Drawn, not a font's glyph:
    // the same on every system.
    component Mark: Item {
        id: mark

        required property string value
        property color tint: "black"
        // The page's colour, to hollow the star of ☆.
        property color hole: "white"

        implicitWidth: 16
        implicitHeight: 16

        Rectangle {
            visible: mark.value === "now"
            anchors.centerIn: parent
            width: 10
            height: 10
            radius: 5
            color: mark.tint
        }
        Rectangle {
            visible: mark.value === "later" || mark.value === "gathered" || mark.value === "event"
            anchors.centerIn: parent
            width: 12
            height: 12
            radius: 6
            color: "transparent"
            border.color: mark.tint
            border.width: 1.5
        }
        Rectangle {
            visible: mark.value === "gathered"
            anchors.centerIn: parent
            width: 4
            height: 4
            radius: 2
            color: mark.tint
        }
        // The left half of the ring, filled.
        Item {
            visible: mark.value === "event"
            anchors.centerIn: parent
            width: 12
            height: 12

            Item {
                width: 6
                height: 12
                clip: true

                Rectangle {
                    width: 12
                    height: 12
                    radius: 6
                    color: mark.tint
                }
            }
        }
        Rectangle {
            visible: mark.value === "never"
            anchors.centerIn: parent
            width: 10
            height: 2
            radius: 1
            color: mark.tint
        }
        IconImage {
            visible: mark.value === "list" || mark.value === "list-any"
            anchors.centerIn: parent
            name: "emblem-favorite"
            color: mark.tint
            sourceSize: Qt.size(15, 15)
        }
        // ☆: the star hollowed by a smaller one, of the page's colour.
        IconImage {
            visible: mark.value === "list"
            anchors.centerIn: parent
            anchors.verticalCenterOffset: 1
            name: "emblem-favorite"
            color: mark.hole
            sourceSize: Qt.size(7, 7)
        }
    }

    // Measured once each, for the widths above.
    Repeater {
        id: heads

        model: grid.columns

        delegate: Label {
            required property var modelData

            visible: false
            text: modelData.label
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
                    font.pixelSize: 13
                    color: strip.view.theme.muted
                }
                // Read from bottom to top, its end against the marks.
                Label {
                    visible: strip.view.upright
                    x: (head.width - height) / 2
                    y: head.height - 3
                    text: head.modelData.label
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
                elide: Text.ElideRight
                color: grid.theme.text
            }
            RowLayout {
                spacing: 0

                Label {
                    visible: grid.beside
                    Layout.preferredWidth: grid.nameWidth
                    text: row.modelData.label
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
                        ToolTip.text: box.modelData.said
                        ToolTip.delay: 600

                        background: Rectangle {
                            radius: grid.theme.radius
                            color: box.down ? grid.theme.pressed : box.hovered || box.visualFocus ? grid.theme.hover : "transparent"
                        }
                        contentItem: Item {
                            Mark {
                                anchors.centerIn: parent
                                value: box.modelData.value
                                tint: box.fixed ? grid.theme.muted : grid.theme.text
                                hole: grid.theme.background
                                opacity: box.fixed ? 0.6 : 1
                            }
                        }
                        onClicked: grid.open(row.modelData, box.modelData, box)
                    }
                }
            }
        }
    }

    // What each mark says.
    Flow {
        Layout.fillWidth: true
        Layout.topMargin: 10
        Layout.bottomMargin: 6
        spacing: 14

        Repeater {
            model: grid.matrix.marks

            delegate: RowLayout {
                id: legend

                required property var modelData

                spacing: 5

                Mark {
                    value: legend.modelData.id
                    tint: grid.theme.text
                    hole: grid.theme.background
                }
                Label {
                    // A long one wraps under itself on a narrow screen.
                    Layout.maximumWidth: Math.max(120, grid.width - 30)
                    text: legend.modelData.label
                    wrapMode: Text.Wrap
                    font.pixelSize: 12
                    color: grid.theme.muted
                }
            }
        }
        RowLayout {
            spacing: 5

            Mark {
                value: "now"
                tint: grid.theme.muted
                hole: grid.theme.background
                opacity: 0.6
            }
            Label {
                text: grid.sioul.text("notify-legend-fixed")
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

                    Mark {
                        visible: !line.said
                        value: line.said ? "" : line.modelData.id
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
