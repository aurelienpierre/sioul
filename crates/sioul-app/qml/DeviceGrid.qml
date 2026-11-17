// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

// What comes forward on each device at each time (Settings ▸ Devices;
// docs/areas.md, "What each device is for"): a cell per device and time,
// saying in words the areas that come forward there ("Work", "Admin ·
// Leisure", "Nothing"). A cell opens three boxes, Work, Admin and Leisure,
// and "As its areas say", which puts back what the time brings, kept to what
// the device is for (AreaFilter.qml lays them out); a cell you changed is
// drawn apart, in a frame. In a wide window the cells line up as one matrix,
// devices by times, under the times' names; in a narrow one (a phone's 412
// points) each device says its cells in words, one line per time. A device
// whose Sioul is older says in their place that everything still comes
// there. Rust decides and words every cell (sioul-sync's device_table); this
// lays them out and says what was ticked.

pragma ComponentBehavior: Bound

import QtQuick
import QtQuick.Controls.Basic
import QtQuick.Layouts

ColumnLayout {
    id: grid

    // Sioul's backend (backend.rs): its words in your language.
    required property var sioul
    // The window's colours, sizes and fonts (Theme.qml).
    required property var theme
    // The devices' rows (`share::devices`): each with its name, its id,
    // `follows` (its Sioul follows what devices are for), `cells_older` (said
    // in the cells' place when it does not) and `cells`, five, one per time:
    // {time, title, work, admin, leisure, label, own, name}.
    property var rows: []
    // The five times' names, in the cells' order.
    property var times: []
    // One matrix, devices by times; else each device's cells in words, a line per time.
    property bool wide: false
    // Each device named above its lines (a narrow list of several devices).
    property bool named: false
    // What the "?" of a cell's boxes says.
    property string help: ""
    // The width of the matrix's first column, the devices' names, and of a cell.
    readonly property int nameWidth: Math.floor(grid.width * 0.22)
    readonly property int cellWidth: Math.floor((grid.width - grid.nameWidth - 5 * grid.spacing) / Math.max(1, grid.times.length))

    // A cell changed: the device's id, the time ("work", "meals"), and the
    // areas as written: "work+leisure", "none" for nothing, "" as its areas say.
    signal picked(string device, string time, string areas)

    spacing: 6

    // A cell: its words on a button that unfolds the three boxes.
    component Cell: Rectangle {
        id: cell

        // The grid: an inline component is given what it reads.
        required property var host
        // The device's id.
        required property string deviceId
        // What Rust says of it: {time, title, work, admin, leisure, label, own, name}.
        required property var said

        objectName: "cell-" + cell.deviceId + "-" + (cell.said.time || "")
        implicitWidth: filter.implicitWidth
        implicitHeight: filter.implicitHeight
        radius: cell.host.theme.radius
        color: "transparent"
        // A cell you changed is drawn apart.
        border.width: cell.said.own === true ? 1 : 0
        border.color: cell.host.theme.accent

        AreaFilter {
            id: filter

            anchors.fill: parent
            sioul: cell.host.sioul
            theme: cell.host.theme
            shown: cell.said
            help: cell.host.help
            name: cell.said.name || ""
            asHoursText: cell.host.sioul.text("devices-cell-as-areas")
            // Nothing ticked is your word for nothing, kept as such.
            onTicked: areas => cell.host.picked(cell.deviceId, cell.said.time || "", areas === "" ? "none" : areas)
            onAsHours: {
                filter.fold()
                cell.host.picked(cell.deviceId, cell.said.time || "", "")
            }
        }
    }

    // The matrix's titles: the times.
    RowLayout {
        visible: grid.wide
        Layout.fillWidth: true
        spacing: grid.spacing

        Label {
            Layout.preferredWidth: grid.nameWidth
            Layout.maximumWidth: grid.nameWidth
            text: grid.sioul.text("devices-grid-device")
            font.pixelSize: 12
            font.weight: Font.DemiBold
            elide: Text.ElideRight
            color: grid.theme.muted
        }
        Repeater {
            model: grid.wide ? grid.times : []

            delegate: Label {
                required property string modelData

                Layout.preferredWidth: grid.cellWidth
                Layout.maximumWidth: grid.cellWidth
                text: modelData
                textFormat: Text.PlainText
                font.pixelSize: 12
                font.weight: Font.DemiBold
                elide: Text.ElideRight
                color: grid.theme.muted
            }
        }
    }

    Repeater {
        model: grid.rows

        delegate: ColumnLayout {
            id: device

            required property var modelData
            readonly property var row: device.modelData
            readonly property string deviceId: device.row.id || ""
            readonly property bool follows: device.row.follows !== false
            readonly property var cells: device.follows ? (device.row.cells || []) : []

            objectName: "cells-" + device.deviceId
            Layout.fillWidth: true
            spacing: 2

            // In the matrix: its name, then its five cells under the times' names.
            RowLayout {
                visible: grid.wide
                Layout.fillWidth: true
                spacing: grid.spacing

                Label {
                    Layout.preferredWidth: grid.nameWidth
                    Layout.maximumWidth: grid.nameWidth
                    text: device.row.name || ""
                    textFormat: Text.PlainText
                    wrapMode: Text.Wrap
                    font.pixelSize: 14
                    color: grid.theme.text
                }
                Repeater {
                    model: grid.wide ? device.cells : []

                    delegate: Item {
                        id: slot

                        required property var modelData

                        Layout.preferredWidth: grid.cellWidth
                        Layout.maximumWidth: grid.cellWidth
                        implicitHeight: inMatrix.implicitHeight

                        Cell {
                            id: inMatrix

                            host: grid
                            deviceId: device.deviceId
                            said: slot.modelData
                            width: Math.min(implicitWidth, slot.width)
                        }
                    }
                }
                // An older Sioul: everything still comes there.
                Label {
                    objectName: "cellsOlder"
                    visible: grid.wide && !device.follows
                    Layout.fillWidth: true
                    text: device.row.cells_older || ""
                    textFormat: Text.PlainText
                    wrapMode: Text.Wrap
                    font.pixelSize: 13
                    color: grid.theme.muted
                }
            }

            // In words: its name when several are listed, then a line per time.
            Label {
                visible: !grid.wide && grid.named
                Layout.fillWidth: true
                text: device.row.name || ""
                textFormat: Text.PlainText
                wrapMode: Text.Wrap
                font.pixelSize: 14
                font.weight: Font.DemiBold
                color: grid.theme.text
            }
            Repeater {
                model: grid.wide ? [] : device.cells

                delegate: RowLayout {
                    id: line

                    required property var modelData

                    Layout.fillWidth: true
                    spacing: 6

                    Label {
                        Layout.preferredWidth: Math.floor(grid.width * 0.32)
                        Layout.maximumWidth: Math.floor(grid.width * 0.32)
                        text: line.modelData.title || ""
                        textFormat: Text.PlainText
                        elide: Text.ElideRight
                        font.pixelSize: 13
                        color: grid.theme.muted
                    }
                    Cell {
                        host: grid
                        deviceId: device.deviceId
                        said: line.modelData
                        Layout.maximumWidth: grid.width - Math.floor(grid.width * 0.32) - line.spacing
                    }
                    Item {
                        Layout.fillWidth: true
                    }
                }
            }
            Label {
                objectName: "cellsOlder"
                visible: !grid.wide && !device.follows
                Layout.fillWidth: true
                text: device.row.cells_older || ""
                textFormat: Text.PlainText
                wrapMode: Text.Wrap
                font.pixelSize: 13
                color: grid.theme.muted
            }
        }
    }
}
