// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

// What a task or an event costs, and what it gives back (docs/capacity.md,
// "The form"): four tiles, thinking, feelings, worry, body and senses, two by
// two, then "Gives back" under them. Each tile holds its name, its number, a
// gauge (a small cell for 0, then ten cells for 1 to 10), the word of its
// band on Borg's CR10 scale (moderate, hard…) and a concrete question in
// small print. Feelings are asked about through what stirs them, for whoever
// finds them hard to name.
//
// A tap on a cell gives that value; a tap on the value given clears it:
// unsaid again, which is not 0. With the focus on a tile, a digit gives 0 to
// 9, + or = gives 10, the arrows step by one (given when the key comes up),
// Delete or Backspace clears.
//
// Faint values (`proposed`): what was felt after the same task, else after
// tasks of its kind (`FeltIndex`, sioul-core's capacity.rs). They show on
// unsaid tiles only and are never given by themselves: "Looks right" takes
// them all, a tap sets one tile instead. "How was it?" (`felt`) has none: the
// forecast is a thin mark on each gauge there, never copied into the answer.
//
// Two columns, or one: under `narrow`, and on a touch screen
// (`theme.touch`) wherever two columns would make a cell narrower than a
// fingertip (`fingerCell`). A narrower gauge still works: a tap picks the
// nearest cell.

pragma ComponentBehavior: Bound

import QtQuick
import QtQuick.Controls.Basic
import QtQuick.Layouts

ColumnLayout {
    id: tiles

    required property var sioul
    required property var theme
    // What is said: {cognitive, emotional, anxiety, body, gain}, each 0 to 10, null or missing when unsaid.
    property var values: ({})
    // Faint values from before, the same shape (a task's `proposed`); ({}) for none.
    property var proposed: ({})
    // Where the faint values come from: "item" (this task), "kind" (tasks of its kind), "both".
    property string proposedFrom: ""
    // "How was it?": the forecast as thin marks, the gain asked in the past, no faint values.
    property bool felt: false
    property var forecast: ({})
    // One column under this width, whatever the device.
    property real narrow: 360

    // A cell a fingertip hits among its neighbours (Android asks 48 dp for a
    // lone target; cells side by side, tapped one at a time, do with less).
    readonly property real fingerCell: 30
    // The narrowest cell a mouse hits without care, with the word beside the gauge.
    readonly property real mouseCell: 14
    // A cell at the tiles' natural width.
    readonly property real comfortCell: 16
    readonly property bool touch: tiles.theme.touch === true
    readonly property real gap: 8
    readonly property real pad: 10
    readonly property var costs: ["cognitive", "emotional", "anxiety", "body"]
    readonly property var names: ["cognitive", "emotional", "anxiety", "body", "gain"]

    // The band words, after Borg's CR10: a word at fixed numbers, a number
    // between two takes the word below it (docs/capacity.md, "The form").
    readonly property var costWords: [tiles.sioul.text("tile-cost-0"), tiles.sioul.text("tile-cost-1"), tiles.sioul.text("tile-cost-2"), tiles.sioul.text("tile-cost-3"), tiles.sioul.text("tile-cost-5"), tiles.sioul.text("tile-cost-7"), tiles.sioul.text("tile-cost-10")]
    readonly property var costBands: [0, 1, 2, 3, 3, 4, 4, 5, 5, 5, 6]
    readonly property var gainWords: [tiles.sioul.text("tile-gain-0"), tiles.sioul.text("tile-gain-1"), tiles.sioul.text("tile-gain-3"), tiles.sioul.text("tile-gain-5"), tiles.sioul.text("tile-gain-7"), tiles.sioul.text("tile-gain-10")]
    readonly property var gainBands: [0, 1, 1, 2, 2, 3, 3, 4, 4, 4, 5]

    // The widest word, for the gauges beside it to line up from tile to tile.
    readonly property real wordRoom: Math.ceil(Math.max(...tiles.costWords.concat(tiles.gainWords, [tiles.sioul.text("tile-unsaid")]).map(w => small.advanceWidth(w)))) + 2
    // A tile's inside, in one column or two.
    readonly property real tileInside: (tiles.width - (tiles.columns - 1) * tiles.gap) / tiles.columns - 2 * tiles.pad - 2
    // The word beside the gauge where the cells keep their size, else above it, beside the number.
    readonly property bool beside: tiles.cellOf(tiles.tileInside - tiles.wordRoom - 8) >= (tiles.touch ? tiles.fingerCell : tiles.mouseCell)
    // Every gauge as wide, the gain's too, so that their cells line up.
    readonly property real gaugeWidth: tiles.beside ? tiles.tileInside - tiles.wordRoom - 8 : tiles.tileInside
    readonly property int columns: tiles.width < tiles.narrow || (tiles.touch && tiles.cellOf((tiles.width - tiles.gap) / 2 - 2 * tiles.pad - 2) < tiles.fingerCell) ? 1 : 2
    // Whether a faint value waits on an unsaid tile.
    readonly property bool offered: !tiles.felt && tiles.names.some(n => tiles.said(n) === null && tiles.faint(n) !== null)

    // A value given by hand: 0 to 10, or null (unsaid again).
    signal edited(string name, var value)
    // "Looks right": the faint values of the unsaid tiles, {name: value}.
    signal taken(var values)

    // The width of a cell for 1 to 10 in a gauge `width` wide: the 0 cell is
    // 0.6 of one, and ten gaps of 2 pixels part them.
    function cellOf(width) {
        return (width - 20) / 10.6
    }

    function said(name) {
        const value = tiles.values ? tiles.values[name] : null
        return value === undefined || value === null ? null : value
    }

    // A faint value, rounded (a median may fall between two); null when none or in "How was it?".
    function faint(name) {
        const value = !tiles.felt && tiles.proposed ? tiles.proposed[name] : null
        return value === undefined || value === null ? null : Math.max(0, Math.min(10, Math.round(value)))
    }

    function foreseen(name) {
        const value = tiles.felt && tiles.forecast ? tiles.forecast[name] : null
        return value === undefined || value === null ? null : value
    }

    function word(name, value) {
        if (value === null)
            return tiles.sioul.text("tile-unsaid")
        return name === "gain" ? tiles.gainWords[tiles.gainBands[value]] : tiles.costWords[tiles.costBands[value]]
    }

    function title(name) {
        switch (name) {
        case "cognitive":
            return tiles.sioul.text("tile-cognitive")
        case "emotional":
            return tiles.sioul.text("tile-emotional")
        case "anxiety":
            return tiles.sioul.text("tile-anxiety")
        case "body":
            return tiles.sioul.text("tile-body")
        default:
            return tiles.sioul.text("tile-gain")
        }
    }

    function ask(name) {
        switch (name) {
        case "cognitive":
            return tiles.sioul.text("tile-cognitive-ask")
        case "emotional":
            return tiles.sioul.text("tile-emotional-ask")
        case "anxiety":
            return tiles.sioul.text("tile-anxiety-ask")
        case "body":
            return tiles.sioul.text("tile-body-ask")
        default:
            return tiles.felt ? tiles.sioul.text("tile-gain-felt-ask") : tiles.sioul.text("tile-gain-ask")
        }
    }

    // "Looks right": every faint value of an unsaid tile, given at once.
    function take() {
        const given = {}
        for (const name of tiles.names)
            if (tiles.said(name) === null && tiles.faint(name) !== null)
                given[name] = tiles.faint(name)
        if (Object.keys(given).length > 0)
            tiles.taken(given)
    }

    // A tile by its name, for the tests.
    function tile(name) {
        if (name === "gain")
            return gainTile
        for (let i = 0; i < grid.children.length; ++i)
            if (grid.children[i].name === name)
                return grid.children[i]
        return null
    }

    // Its natural width: two tiles side by side, each gauge's cells at
    // `comfortCell` with the word beside. A card is as wide as this, or as
    // its text at a readable length, whichever is wider (a layout's own
    // implicitWidth follows its children, so this one has a name of its own).
    readonly property real naturalWidth: 2 * (10.6 * tiles.comfortCell + 20 + 8 + tiles.wordRoom + 2 * tiles.pad + 2) + tiles.gap

    spacing: tiles.gap

    FontMetrics {
        id: small

        font.pixelSize: 13
    }

    GridLayout {
        id: grid

        Layout.fillWidth: true
        columns: tiles.columns
        columnSpacing: tiles.gap
        rowSpacing: tiles.gap

        Repeater {
            model: tiles.costs

            delegate: Tile {
                required property string modelData

                owner: tiles
                name: modelData
            }
        }
    }
    Tile {
        id: gainTile

        owner: tiles
        name: "gain"
    }

    // The faint values, said where they come from, and taken in one tap.
    Flow {
        visible: tiles.offered
        Layout.fillWidth: true
        spacing: 8

        Button {
            id: looksRight

            text: tiles.sioul.text("tile-looks-right")
            enabled: tiles.enabled
            onClicked: tiles.take()
        }
        Label {
            width: Math.min(implicitWidth, parent.width)
            height: looksRight.height
            verticalAlignment: Text.AlignVCenter
            text: tiles.proposedFrom === "kind" ? tiles.sioul.text("tile-proposed-kind") : tiles.proposedFrom === "both" ? tiles.sioul.text("tile-proposed-both") : tiles.sioul.text("tile-proposed-item")
            wrapMode: Text.Wrap
            font.pixelSize: 13
            color: tiles.theme.muted
        }
    }

    // One cost, or the gain: its name and number, its gauge and word, its question.
    component Tile: Rectangle {
        id: tile

        required property var owner
        property string name: ""
        readonly property var value: tile.owner.said(tile.name)
        readonly property var hint: tile.owner.faint(tile.name)
        readonly property var mark: tile.owner.foreseen(tile.name)
        // While an arrow is held: the value it will give when the key comes up.
        property var held: null
        // What the tile shows: the value on its way, the value said, else the faint one.
        readonly property var shown: tile.held !== null ? tile.held : tile.value
        readonly property bool faintShown: tile.shown === null && tile.hint !== null
        readonly property var drawn: tile.shown !== null ? tile.shown : tile.hint

        function give(value) {
            tile.held = null
            if (value !== tile.value)
                tile.owner.edited(tile.name, value)
        }

        // A tap on a cell: that value, or unsaid again when it was the value given.
        function tap(cell) {
            tile.forceActiveFocus(Qt.MouseFocusReason)
            tile.give(cell === tile.value ? null : cell)
        }

        // The middle of a cell, in the tile's own coordinates (for the tests).
        function point(index) {
            return gauge.mapToItem(tile, gauge.cellX(index) + (index === 0 ? gauge.zero : gauge.cell) / 2, gauge.height / 2)
        }

        // Where the forecast's mark stands, in the tile's coordinates; null when hidden (for the tests).
        function markAt() {
            return foreseenMark.visible ? foreseenMark.mapToItem(tile, foreseenMark.width / 2, foreseenMark.height / 2) : null
        }

        // The arrows' start, when nothing is said: the faint value, the forecast, else the middle.
        function start() {
            return tile.hint !== null ? tile.hint : tile.mark !== null ? tile.mark : 5
        }

        Layout.fillWidth: true
        Layout.fillHeight: tile.name !== "gain"
        Layout.preferredWidth: 1
        implicitHeight: inside.implicitHeight + 2 * tile.owner.pad
        radius: tile.owner.theme.radius
        color: tile.owner.theme.surface
        border.width: tile.activeFocus ? 2 : 1
        border.color: tile.activeFocus ? tile.owner.theme.focus : tile.owner.theme.line
        activeFocusOnTab: true

        Accessible.role: Accessible.Slider
        Accessible.name: tile.value === null ? tile.owner.sioul.textWith("tile-said-none", "name", tile.owner.title(tile.name)) : tile.owner.sioul.textArgs("tile-said", JSON.stringify({ name: tile.owner.title(tile.name), value: tile.value, word: tile.owner.word(tile.name, tile.value) }))
        Accessible.description: tile.owner.ask(tile.name) + (tile.mark !== null ? " " + tile.owner.sioul.textWith("rating-foreseen", "value", String(tile.mark)) : "") + (tile.faintShown ? " " + tile.owner.sioul.textWith("tile-proposed-value", "value", String(tile.hint)) : "")

        HoverHandler {
            id: hover
        }
        ToolTip.visible: hover.hovered && tile.enabled
        ToolTip.delay: 900
        ToolTip.text: tile.owner.sioul.text("tile-keys")

        Keys.onPressed: event => {
            const now = tile.shown
            let next
            if (event.key >= Qt.Key_0 && event.key <= Qt.Key_9) {
                tile.give(event.key - Qt.Key_0)
                event.accepted = true
                return
            } else if (event.key === Qt.Key_Plus || event.key === Qt.Key_Equal) {
                tile.give(10)
                event.accepted = true
                return
            } else if ((event.key === Qt.Key_Delete || event.key === Qt.Key_Backspace)) {
                tile.give(null)
                event.accepted = true
                return
            } else if (event.key === Qt.Key_Left || event.key === Qt.Key_Down)
                next = now === null ? tile.start() : Math.max(0, now - 1)
            else if (event.key === Qt.Key_Right || event.key === Qt.Key_Up)
                next = now === null ? tile.start() : Math.min(10, now + 1)
            else if (event.key === Qt.Key_Home)
                next = 0
            else if (event.key === Qt.Key_End)
                next = 10
            else
                return
            tile.held = next
            event.accepted = true
        }
        // An arrow's value given when the key comes up, not at each repeat while it is held.
        Keys.onReleased: event => {
            if (!event.isAutoRepeat && tile.held !== null) {
                tile.give(tile.held)
                event.accepted = true
            }
        }
        onActiveFocusChanged: {
            if (!tile.activeFocus && tile.held !== null)
                tile.give(tile.held)
        }

        ColumnLayout {
            id: inside

            anchors.fill: parent
            anchors.margins: tile.owner.pad
            spacing: 4

            RowLayout {
                Layout.fillWidth: true
                spacing: 6

                Label {
                    Layout.fillWidth: true
                    // Its own width, not its wrapped text's (a binding loop otherwise).
                    Layout.preferredWidth: 1
                    text: tile.owner.title(tile.name)
                    textFormat: Text.PlainText
                    wrapMode: Text.Wrap
                    font.weight: Font.DemiBold
                    color: tile.owner.theme.text
                }
                // The word above, beside the number, where the gauge needs the whole width.
                Label {
                    visible: !tile.owner.beside
                    text: tile.owner.word(tile.name, tile.drawn)
                    textFormat: Text.PlainText
                    font.pixelSize: 13
                    color: tile.owner.theme.muted
                    opacity: tile.faintShown ? 0.6 : 1
                }
                Label {
                    Layout.minimumWidth: number.advanceWidth
                    text: tile.drawn === null ? "" : String(tile.drawn)
                    textFormat: Text.PlainText
                    font.weight: Font.DemiBold
                    color: tile.faintShown ? tile.owner.theme.muted : tile.owner.theme.text
                    opacity: tile.faintShown ? 0.6 : 1
                    horizontalAlignment: Text.AlignRight

                    TextMetrics {
                        id: number

                        font.weight: Font.DemiBold
                        text: "10"
                    }
                }
            }
            RowLayout {
                Layout.fillWidth: true
                spacing: 8

                // A small cell for 0, then ten for 1 to 10.
                Item {
                    id: gauge

                    readonly property real zero: Math.max(10, (gauge.width - 20) / 10.6 * 0.6)
                    readonly property real cell: (gauge.width - 20 - gauge.zero) / 10

                    // Where a cell begins.
                    function cellX(index) {
                        return index === 0 ? 0 : gauge.zero + 2 + (index - 1) * (gauge.cell + 2)
                    }

                    // The cell at `x`, or the nearest one.
                    function at(x) {
                        if (x < gauge.zero + 1)
                            return 0
                        return Math.max(1, Math.min(10, Math.floor((x - gauge.zero - 1) / (gauge.cell + 2)) + 1))
                    }

                    Layout.preferredWidth: tile.owner.gaugeWidth
                    Layout.maximumWidth: tile.owner.gaugeWidth
                    implicitHeight: tile.owner.touch ? 28 : 20

                    Repeater {
                        model: 11

                        delegate: Rectangle {
                            required property int index
                            readonly property bool full: tile.drawn !== null && (index === 0 ? tile.drawn === 0 : index <= tile.drawn)

                            x: gauge.cellX(index)
                            width: index === 0 ? gauge.zero : gauge.cell
                            height: index === 0 ? Math.min(gauge.zero, gauge.height) : gauge.height
                            y: (gauge.height - height) / 2
                            radius: index === 0 ? width / 2 : 2
                            color: full ? tile.owner.theme.accent : "transparent"
                            opacity: full && tile.faintShown ? 0.3 : 1
                            border.width: full && !tile.faintShown ? 0 : 1
                            border.color: tile.owner.theme.pressed
                        }
                    }
                    // What was foreseen ("How was it?"): a thin mark across its cell.
                    Rectangle {
                        id: foreseenMark

                        visible: tile.mark !== null
                        x: tile.mark === null ? 0 : gauge.cellX(tile.mark) + (tile.mark === 0 ? gauge.zero : gauge.cell) / 2 - 1
                        y: -4
                        width: 2
                        height: gauge.height + 8
                        radius: 1
                        color: tile.owner.theme.text
                        opacity: 0.7
                    }
                    MouseArea {
                        id: area

                        // A little larger than the cells: a finger lands a little off.
                        // The tile's margin before the 0 cell gives 0, the one after the
                        // last gives 10, where no word stands beside it.
                        anchors.fill: parent
                        anchors.topMargin: -6
                        anchors.bottomMargin: -6
                        anchors.leftMargin: -tile.owner.pad
                        anchors.rightMargin: tile.owner.beside ? 0 : -tile.owner.pad
                        enabled: tile.enabled
                        cursorShape: Qt.PointingHandCursor
                        onClicked: mouse => tile.tap(gauge.at(mouse.x + area.x))
                    }
                }
                Label {
                    visible: tile.owner.beside
                    Layout.preferredWidth: tile.owner.wordRoom
                    text: tile.owner.word(tile.name, tile.drawn)
                    textFormat: Text.PlainText
                    font.pixelSize: 13
                    color: tile.owner.theme.muted
                    opacity: tile.faintShown ? 0.6 : 1
                }
                Item {
                    Layout.fillWidth: true
                }
            }
            Label {
                Layout.fillWidth: true
                Layout.preferredWidth: 1
                text: tile.owner.ask(tile.name)
                textFormat: Text.PlainText
                wrapMode: Text.Wrap
                font.pixelSize: 12
                color: tile.owner.theme.muted
            }
            // Room taken by the taller tile of the row: the question stays under the gauge.
            Item {
                Layout.fillHeight: true
            }
        }
    }
}
