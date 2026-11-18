// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

// One phone's calls, in its card in Settings ▸ Devices (docs/android.md,
// "Calls, per phone"), set from any device: the profile to start from
// (Professional, Personal, Both, As Android does; "Your own" once changed by
// hand), with what it does in a line; one small table, Rings or Voicemail by
// row (Always through, fixed; Safe; Your contacts; the groups given a row;
// Restricted; numbers nobody knows; hidden numbers) and by moment (work
// hours, the rest of the day, the night, while busy); three switches; a
// group given its row, or taken off; what always rings; the SIMs; and, on a
// phone that does not screen yet, what to do. At a computer's width the
// table is a grid of buttons; at a phone's, a list of rows, each opening its
// four choices. The words are the core's (`calls_line::view`, through
// `share::devices`); each change is written as a field of the phone's line
// (`deviceSay(id, "calls.…", value)`), the same on every device.

pragma ComponentBehavior: Bound

import QtQuick
import QtQuick.Controls.Basic
import QtQuick.Layouts

ColumnLayout {
    id: line

    // Sioul's backend (backend.rs): its words in your language.
    required property var sioul
    // The window's colours, sizes and fonts (Theme.qml).
    required property var theme
    // What the core says of this phone's calls (`calls_line::view`).
    property var view: ({ rows: [], columns: [], profiles: [], switches: [], groups: [] })
    // This phone's own card, on the phone itself: Android's SIM page offered.
    property bool here: false
    // A phone's width: a list of rows rather than a grid.
    property bool narrow: false
    // The row open, at a phone's width (its id).
    property string openRow: ""

    // A field of the line to write ("calls.profile", "calls.cell.safe.night"…) and its value.
    signal say(string field, string value)
    // Android's SIM page asked for (on the phone itself).
    signal simsAsked

    readonly property var chosen: (line.view.profiles || []).find(p => p.chosen === true) || null
    // The profile chosen; the one offered while the table shows neither today's nor yours.
    readonly property var shownProfile: line.chosen || (line.view.today === true ? null : (line.view.profiles || []).find(p => p.id === line.view.offered) || null)
    readonly property bool android: line.view.profile === "as-android"
    // The rows' names' column, at a computer's width.
    readonly property int labelWidth: Math.min(220, Math.round(line.width * 0.28))

    spacing: 8

    component Said: Label {
        Layout.fillWidth: true
        textFormat: Text.PlainText
        wrapMode: Text.Wrap
        color: line.theme.text
    }
    component Note: Label {
        Layout.fillWidth: true
        textFormat: Text.PlainText
        wrapMode: Text.Wrap
        font.pixelSize: 13
        color: line.theme.muted
    }
    // One cell: Rings (the accent's fill) or Voicemail; a press turns it to the
    // other. Today's cell that varies within its column (Varies, As usual):
    // neither, and a press makes it ring.
    component Cell: Button {
        id: cell

        required property string rowId
        required property var said
        required property string rowLabel
        required property string columnLabel
        readonly property bool varies: cell.said.varies === true

        objectName: "callsCell-" + cell.rowId + "-" + (cell.said.when || "")
        text: line.theme.plain(cell.said.word || "")
        highlighted: cell.said.rings === true && !cell.varies
        flat: cell.said.rings !== true || cell.varies
        font.italic: cell.varies
        Accessible.name: cell.rowLabel + ", " + cell.columnLabel + ": " + (cell.said.word || "")
        onClicked: line.say("calls.cell." + cell.rowId + "." + cell.said.when, cell.said.rings === true && !cell.varies ? "voicemail" : "rings")
    }

    // ---------------------------------------------------------------- the profile
    Said {
        objectName: "callsSaid"
        visible: (line.view.said || "") !== ""
        text: line.view.said || ""
    }
    Label {
        Layout.fillWidth: true
        text: line.sioul.text("phone-calls-profile")
        font.pixelSize: 13
        font.weight: Font.DemiBold
        color: line.theme.muted
    }
    Flow {
        Layout.fillWidth: true
        spacing: 6

        Repeater {
            model: line.view.profiles || []

            delegate: Button {
                id: profile

                required property var modelData

                objectName: "callsProfile-" + profile.modelData.id
                width: Math.min(implicitWidth, parent ? parent.width : implicitWidth)
                text: line.theme.plain(profile.modelData.label)
                checkable: true
                checked: profile.modelData.chosen === true
                flat: profile.modelData.chosen !== true
                // "Your own" is no choice: a cell or a switch makes it.
                enabled: profile.modelData.id !== "none"
                onClicked: {
                    line.say("calls.profile", profile.modelData.id)
                    profile.checked = Qt.binding(() => profile.modelData.chosen === true)
                }
            }
        }
    }
    // What the profile chosen (or offered) does, in its own line.
    Said {
        objectName: "callsProfileSaid"
        visible: line.shownProfile !== null
        text: line.shownProfile ? line.shownProfile.said || "" : ""
    }

    // ---------------------------------------------------------------- the table, at a computer's width
    ColumnLayout {
        objectName: "callsGrid"
        visible: !line.narrow && !line.android
        Layout.fillWidth: true
        Layout.topMargin: 4
        spacing: 4

        // The moments, above their columns.
        RowLayout {
            Layout.fillWidth: true
            spacing: 6

            Item {
                Layout.preferredWidth: line.labelWidth
                Layout.maximumWidth: line.labelWidth
            }
            Repeater {
                model: line.narrow ? [] : line.view.columns || []

                delegate: Label {
                    required property var modelData

                    Layout.fillWidth: true
                    Layout.preferredWidth: 10
                    text: modelData.label
                    textFormat: Text.PlainText
                    wrapMode: Text.Wrap
                    horizontalAlignment: Text.AlignHCenter
                    font.pixelSize: 12
                    font.weight: Font.DemiBold
                    color: line.theme.muted
                }
            }
        }
        Repeater {
            model: line.narrow || line.android ? [] : line.view.rows || []

            delegate: RowLayout {
                id: gridRow

                required property var modelData

                Layout.fillWidth: true
                spacing: 6

                RowLayout {
                    Layout.preferredWidth: line.labelWidth
                    Layout.maximumWidth: line.labelWidth
                    spacing: 2

                    Label {
                        objectName: "callsRow-" + gridRow.modelData.id
                        Layout.fillWidth: true
                        text: gridRow.modelData.label
                        textFormat: Text.PlainText
                        wrapMode: Text.Wrap
                        color: line.theme.text
                    }
                    Button {
                        objectName: "callsRemove-" + gridRow.modelData.id
                        visible: gridRow.modelData.group === true
                        implicitWidth: implicitHeight
                        flat: true
                        text: "×"
                        Accessible.name: line.sioul.text("phone-calls-remove-group") + ": " + gridRow.modelData.label
                        onClicked: line.say("calls.group.remove", gridRow.modelData.label)
                    }
                }
                Repeater {
                    model: gridRow.modelData.cells || []

                    delegate: Item {
                        id: place

                        required property var modelData
                        required property int index

                        Layout.fillWidth: true
                        Layout.preferredWidth: 10
                        implicitHeight: Math.max(fixedWord.implicitHeight, cellButton.implicitHeight)

                        // Always through rings at every moment: words, no button.
                        Label {
                            id: fixedWord

                            visible: gridRow.modelData.fixed === true
                            anchors.centerIn: parent
                            text: place.modelData.word || ""
                            textFormat: Text.PlainText
                            color: line.theme.muted
                        }
                        Cell {
                            id: cellButton

                            visible: gridRow.modelData.fixed !== true
                            anchors.horizontalCenter: parent.horizontalCenter
                            width: Math.min(implicitWidth, place.width)
                            rowId: gridRow.modelData.id
                            rowLabel: gridRow.modelData.label
                            columnLabel: (line.view.columns || [])[place.index] ? line.view.columns[place.index].label : ""
                            said: place.modelData
                        }
                    }
                }
            }
        }
    }

    // ---------------------------------------------------------------- the table, at a phone's width: a list of rows
    Repeater {
        model: line.narrow && !line.android ? line.view.rows || [] : []

        delegate: ColumnLayout {
            id: listRow

            required property var modelData
            readonly property bool opened: line.openRow === listRow.modelData.id

            Layout.fillWidth: true
            spacing: 2

            RowLayout {
                Layout.fillWidth: true
                spacing: 4

                BareButton {
                    id: rowLine

                    objectName: "callsRow-" + listRow.modelData.id
                    Layout.fillWidth: true
                    enabled: listRow.modelData.fixed !== true
                    text: (listRow.modelData.fixed === true ? "" : listRow.opened ? "▾  " : "▸  ") + listRow.modelData.label
                    Accessible.name: listRow.modelData.label
                    onClicked: line.openRow = listRow.opened ? "" : listRow.modelData.id

                    contentItem: Label {
                        text: rowLine.text
                        textFormat: Text.PlainText
                        wrapMode: Text.Wrap
                        color: line.theme.text
                    }
                }
                Button {
                    visible: listRow.modelData.group === true
                    implicitWidth: implicitHeight
                    flat: true
                    text: "×"
                    Accessible.name: line.sioul.text("phone-calls-remove-group") + ": " + listRow.modelData.label
                    onClicked: line.say("calls.group.remove", listRow.modelData.label)
                }
            }
            Note {
                visible: listRow.modelData.fixed === true
                Layout.leftMargin: 8
                text: line.sioul.text("phone-calls-always-rings")
            }
            // Its four choices, each moment on a line of its own.
            Repeater {
                model: listRow.opened ? listRow.modelData.cells || [] : []

                delegate: RowLayout {
                    id: choice

                    required property var modelData
                    required property int index

                    Layout.fillWidth: true
                    Layout.leftMargin: 16
                    spacing: 6

                    Label {
                        Layout.fillWidth: true
                        text: (line.view.columns || [])[choice.index] ? line.view.columns[choice.index].label : ""
                        textFormat: Text.PlainText
                        wrapMode: Text.Wrap
                        color: line.theme.text
                    }
                    Cell {
                        rowId: listRow.modelData.id
                        rowLabel: listRow.modelData.label
                        columnLabel: (line.view.columns || [])[choice.index] ? line.view.columns[choice.index].label : ""
                        said: choice.modelData
                    }
                }
            }
        }
    }

    // ---------------------------------------------------------------- the groups, the switches
    Flow {
        visible: !line.android && (line.view.groups || []).length > 0
        Layout.fillWidth: true
        spacing: 6

        Button {
            id: addGroup

            objectName: "callsAddGroup"
            width: Math.min(implicitWidth, parent ? parent.width : implicitWidth)
            flat: true
            text: line.sioul.text("phone-calls-add-group")
            onClicked: groupMenu.popup(addGroup, 0, addGroup.height)

            Menu {
                id: groupMenu

                Repeater {
                    model: line.view.groups || []

                    delegate: MenuItem {
                        required property string modelData

                        text: line.theme.plain(modelData)
                        onTriggered: line.say("calls.group.add", modelData)
                    }
                }
            }
        }
    }
    Repeater {
        model: line.android ? [] : line.view.switches || []

        delegate: Switch {
            id: toggle

            required property var modelData

            objectName: "callsSwitch-" + toggle.modelData.key
            Layout.fillWidth: true
            text: line.theme.plain(toggle.modelData.label)
            checked: toggle.modelData.on === true
            onToggled: {
                line.say("calls.switch." + toggle.modelData.key, toggle.checked ? "on" : "off")
                toggle.checked = Qt.binding(() => toggle.modelData.on === true)
            }

            contentItem: Label {
                leftPadding: toggle.indicator.width + toggle.spacing
                text: toggle.text
                textFormat: Text.PlainText
                wrapMode: Text.Wrap
                verticalAlignment: Text.AlignVCenter
                color: line.theme.text
            }
        }
    }

    // Today's cells that vary: when each rings.
    Repeater {
        model: line.android ? [] : line.view.details || []

        delegate: Note {
            required property string modelData

            objectName: "callsDetail"
            text: modelData
        }
    }

    // ---------------------------------------------------------------- what always rings, the SIMs, the role
    Note {
        objectName: "callsAlways"
        text: line.view.always || ""
    }
    // What reaches you's presets leave calls as they are.
    Note {
        objectName: "callsHere"
        text: line.view.here || ""
    }
    Said {
        objectName: "callsNotScreening"
        visible: (line.view.not_screening || "") !== ""
        color: line.theme.warm
        text: line.view.not_screening || ""
    }
    Note {
        objectName: "callsSims"
        text: line.view.sims || ""
    }
    Flow {
        Layout.fillWidth: true
        spacing: 6

        Button {
            objectName: "callsSimsOpen"
            visible: line.here
            width: Math.min(implicitWidth, parent ? parent.width : implicitWidth)
            flat: true
            text: line.sioul.text("phone-calls-sims-open")
            onClicked: line.simsAsked()
        }
        // The line taken out: this phone screens as every phone does again.
        Button {
            objectName: "callsBack"
            visible: line.view.set === true
            width: Math.min(implicitWidth, parent ? parent.width : implicitWidth)
            flat: true
            text: line.sioul.text("phone-calls-back")
            onClicked: line.say("calls.clear", "")
        }
    }
}
