// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

// Agenda ⚙'s matrix of conflicts (several agendas): the exceptions to the
// rule its sentence says once above it (a conflict is you in two places).
// Each agenda that takes events is a row and a column, one box per pair, an
// agenda with itself included; unsaid, a box shows the rule (your agendas
// among themselves ticked, the rest not). A ticked pair compares all its
// events, whoever they need; one of yours unticked, none (`agendas::Matrix`).
// On a computer, a triangle; on a phone, the same as a list of pairs grouped
// by agenda.

pragma ComponentBehavior: Bound

import QtQuick
import QtQuick.Controls.Basic
import QtQuick.Layouts

ColumnLayout {
    id: matrix

    required property var sioul
    required property var theme
    // The rule, said once.
    property string about: ""
    property var view: ({ agendas: [], pairs: [] })
    property string problem: ""
    // A phone, or a narrow panel: the list of pairs.
    readonly property bool listed: matrix.width < 480

    readonly property var agendas: (matrix.view.agendas || []).filter(a => a.events)
    // The names' column of the triangle: what the boxes leave of the width, 200 at most.
    readonly property real nameWidth: Math.max(80, Math.min(200, matrix.width - matrix.agendas.length * 30 - 8))

    function read() {
        try {
            matrix.view = JSON.parse(matrix.sioul.agendasSetup())
        } catch (e) {
            matrix.view = { agendas: [], pairs: [] }
        }
    }

    function pair(a, b) {
        return (matrix.view.pairs || []).find(p => (p.a === a && p.b === b) || (p.a === b && p.b === a)) || { ticked: false, usual: false }
    }

    function tick(a, b, on) {
        matrix.problem = matrix.sioul.setAgendasCompared(a, b, on)
        matrix.read()
    }

    spacing: 4
    Component.onCompleted: matrix.read()

    Label {
        visible: matrix.about !== ""
        Layout.fillWidth: true
        text: matrix.about
        textFormat: Text.PlainText
        wrapMode: Text.Wrap
        color: matrix.theme.muted
    }
    Label {
        visible: matrix.problem !== ""
        Layout.fillWidth: true
        text: matrix.problem
        textFormat: Text.PlainText
        wrapMode: Text.Wrap
        color: matrix.theme.warm
    }

    // A computer: the triangle, each row an agenda, its boxes for itself and those before it.
    ColumnLayout {
        visible: !matrix.listed
        Layout.fillWidth: true
        spacing: 2

        RowLayout {
            spacing: 2

            Item {
                Layout.preferredWidth: matrix.nameWidth
                Layout.preferredHeight: 14
            }
            Repeater {
                model: matrix.listed ? [] : matrix.agendas

                delegate: Item {
                    id: head

                    required property var modelData

                    Layout.preferredWidth: 28
                    Layout.preferredHeight: 14

                    Rectangle {
                        anchors.centerIn: parent
                        width: 10
                        height: 10
                        radius: 5
                        color: head.modelData.colour || matrix.theme.accent
                    }
                    HoverHandler {
                        id: headHover
                    }
                    ToolTip.visible: headHover.hovered
                    ToolTip.text: matrix.theme.plain(head.modelData.name)
                }
            }
        }
        Repeater {
            model: matrix.listed ? [] : matrix.agendas

            delegate: RowLayout {
                id: row

                required property var modelData
                required property int index

                spacing: 2

                Label {
                    Layout.preferredWidth: matrix.nameWidth
                    text: row.modelData.name
                    textFormat: Text.PlainText
                    elide: Text.ElideRight
                    color: matrix.theme.text
                }
                Repeater {
                    model: matrix.agendas

                    delegate: Item {
                        id: cell

                        required property var modelData
                        required property int index
                        readonly property var pair: matrix.pair(row.modelData.id, cell.modelData.id)

                        Layout.preferredWidth: 28
                        Layout.preferredHeight: 28

                        CheckBox {
                            objectName: "conflictCell"
                            visible: cell.index <= row.index
                            anchors.centerIn: parent
                            checked: cell.pair.ticked === true
                            Accessible.name: cell.index === row.index ? matrix.sioul.textWith("conflicts-itself", "agenda", row.modelData.name) : matrix.sioul.textArgs("conflicts-pair", JSON.stringify({ first: row.modelData.name, second: cell.modelData.name }))
                            ToolTip.visible: hovered
                            ToolTip.text: matrix.theme.plain(Accessible.name)
                            ToolTip.delay: 300
                            onToggled: {
                                matrix.tick(row.modelData.id, cell.modelData.id, checked)
                                checked = Qt.binding(() => cell.pair.ticked === true)
                            }
                        }
                    }
                }
            }
        }
    }

    // A phone: the pairs, grouped by agenda, each with itself and those after it.
    Repeater {
        model: matrix.listed ? matrix.agendas : []

        delegate: ColumnLayout {
            id: group

            required property var modelData
            required property int index

            Layout.fillWidth: true
            spacing: 0

            Label {
                Layout.fillWidth: true
                Layout.topMargin: 6
                text: group.modelData.name
                textFormat: Text.PlainText
                font.weight: Font.DemiBold
                elide: Text.ElideRight
                color: matrix.theme.text
            }
            Repeater {
                model: matrix.agendas.slice(group.index)

                delegate: CheckBox {
                    id: pairBox

                    required property var modelData
                    readonly property var pair: matrix.pair(group.modelData.id, pairBox.modelData.id)

                    objectName: "conflictPair"
                    Layout.fillWidth: true
                    Layout.leftMargin: 12
                    text: pairBox.modelData.id === group.modelData.id ? matrix.sioul.text("conflicts-with-itself") : matrix.theme.plain(matrix.sioul.textWith("conflicts-with", "agenda", pairBox.modelData.name))
                    checked: pairBox.pair.ticked === true
                    onToggled: {
                        matrix.tick(group.modelData.id, pairBox.modelData.id, checked)
                        checked = Qt.binding(() => pairBox.pair.ticked === true)
                    }
                }
            }
        }
    }
}
