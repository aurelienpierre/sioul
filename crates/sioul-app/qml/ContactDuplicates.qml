// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

// Duplicates in the contacts, looked for when this view opens: the cards that
// hold a number or an address twice, each ticked, cleaned with one click;
// then the pairs of cards that may be one person, one at a time, merged, kept
// apart or left for later; and what was done lately, each with Undo. How many
// is said in words; nothing changes without a click.

pragma ComponentBehavior: Bound

import QtQuick
import QtQuick.Controls.Basic
import QtQuick.Layouts

Panel {
    id: view

    required property var sioul
    // One column, on a phone: the two cards of a pair one under the other.
    property bool compact: false
    property string answer: ""
    readonly property var found: view.answer ? JSON.parse(view.answer) : null
    property bool looking: false
    property string problem: ""
    // Cards left unticked, by file: kept as they are.
    property var unticked: ({})
    // Pairs passed over this time ("Later"), by their two files.
    property var passed: ({})
    readonly property var pairs: view.found ? view.found.pairs.filter(p => !view.passed[p.first.key + "\n" + p.second.key]) : []
    readonly property var pair: view.pairs.length > 0 ? view.pairs[0] : null
    // Whose name is kept: "first" or "second".
    property string kept: "first"

    signal openContact(string key)
    signal closeAsked

    function look() {
        view.looking = true
        view.sioul.findDuplicates()
    }

    function ticked() {
        return view.found ? view.found.cleanings.filter(c => !view.unticked[c.key]).map(c => c.key) : []
    }

    function clean() {
        view.problem = view.sioul.cleanContacts(JSON.stringify(view.ticked()))
    }

    function merge() {
        const lead = view.kept === "second" ? view.pair.second : view.pair.first
        const other = view.kept === "second" ? view.pair.first : view.pair.second
        const answer = JSON.parse(view.sioul.mergeContacts(lead.key, other.key))
        view.problem = answer.error || ""
    }

    function notSame() {
        view.problem = view.sioul.notSameContacts(view.pair.first.key, view.pair.second.key)
    }

    function later() {
        const passed = Object.assign({}, view.passed)
        passed[view.pair.first.key + "\n" + view.pair.second.key] = true
        view.passed = passed
    }

    // The newest of what was done, undone (the window's tests).
    function undoLast() {
        if (view.found && view.found.done.length > 0)
            view.problem = view.sioul.undoContacts(view.found.done[0].id)
    }

    function setTicked(key, on) {
        const unticked = Object.assign({}, view.unticked)
        if (on)
            delete unticked[key]
        else
            unticked[key] = true
        view.unticked = unticked
    }

    onPairChanged: view.kept = view.pair ? view.pair.lead : "first"
    onVisibleChanged: {
        if (view.visible)
            view.look()
    }
    Component.onCompleted: view.look()

    Connections {
        target: view.sioul

        function onDuplicatesChanged() {
            view.answer = view.sioul.duplicates
            view.looking = false
        }
    }

    ColumnLayout {
        anchors.fill: parent
        spacing: 10

        RowLayout {
            Layout.fillWidth: true

            Label {
                Layout.fillWidth: true
                text: view.sioul.text("dup-title")
                font.pixelSize: 20
                elide: Text.ElideRight
                color: view.theme.text
            }
            ToolButton {
                text: "×"
                Accessible.name: view.sioul.text("ui-close")
                ToolTip.visible: hovered
                ToolTip.text: view.sioul.text("ui-close")
                onClicked: view.closeAsked()
            }
        }

        ScrollView {
            id: scroll

            Layout.fillWidth: true
            Layout.fillHeight: true
            contentWidth: availableWidth

            ColumnLayout {
                width: scroll.availableWidth
                spacing: 8

                Label {
                    Layout.fillWidth: true
                    text: view.found === null ? view.sioul.text("dup-looking") : view.found.region
                    textFormat: Text.PlainText
                    wrapMode: Text.Wrap
                    color: view.theme.muted
                }
                Label {
                    visible: view.problem !== ""
                    Layout.fillWidth: true
                    text: view.problem
                    textFormat: Text.PlainText
                    wrapMode: Text.Wrap
                    color: view.theme.warm
                }

                // On one card: what goes, card by card, each ticked.
                Label {
                    Layout.fillWidth: true
                    Layout.topMargin: 6
                    text: view.sioul.text("dup-within-title")
                    font.pixelSize: 16
                    font.weight: Font.DemiBold
                    wrapMode: Text.Wrap
                    color: view.theme.accent
                }
                Label {
                    visible: view.found !== null
                    Layout.fillWidth: true
                    text: view.found ? view.found.within : ""
                    textFormat: Text.PlainText
                    wrapMode: Text.Wrap
                    color: view.theme.text
                }
                Repeater {
                    model: view.found ? view.found.cleanings : []

                    delegate: RowLayout {
                        id: cleaning

                        required property var modelData

                        Layout.fillWidth: true
                        spacing: 6

                        CheckBox {
                            Layout.alignment: Qt.AlignTop
                            checked: !view.unticked[cleaning.modelData.key]
                            Accessible.name: cleaning.modelData.name
                            onToggled: view.setTicked(cleaning.modelData.key, checked)
                        }
                        ColumnLayout {
                            Layout.fillWidth: true
                            spacing: 2

                            Label {
                                Layout.fillWidth: true
                                Layout.topMargin: 8
                                text: cleaning.modelData.name
                                textFormat: Text.PlainText
                                wrapMode: Text.Wrap
                                color: view.theme.text

                                TapHandler {
                                    onTapped: view.openContact(cleaning.modelData.key)
                                }
                            }
                            Repeater {
                                model: cleaning.modelData.lines

                                delegate: Label {
                                    required property string modelData

                                    Layout.fillWidth: true
                                    text: modelData
                                    textFormat: Text.PlainText
                                    wrapMode: Text.Wrap
                                    font.pixelSize: 13
                                    color: view.theme.muted
                                }
                            }
                        }
                    }
                }
                Button {
                    visible: view.found !== null && view.found.cleanings.length > 0
                    enabled: view.ticked().length > 0
                    text: view.sioul.text("dup-clean")
                    onClicked: view.clean()
                }

                // Two cards, one person? One pair at a time.
                Label {
                    Layout.fillWidth: true
                    Layout.topMargin: 14
                    text: view.sioul.text("dup-pairs-title")
                    font.pixelSize: 16
                    font.weight: Font.DemiBold
                    wrapMode: Text.Wrap
                    color: view.theme.accent
                }
                Label {
                    visible: view.found !== null
                    Layout.fillWidth: true
                    text: view.found ? view.found.between : ""
                    textFormat: Text.PlainText
                    wrapMode: Text.Wrap
                    color: view.theme.text
                }
                GridLayout {
                    visible: view.pair !== null
                    Layout.fillWidth: true
                    columns: view.compact ? 1 : 2
                    columnSpacing: 10
                    rowSpacing: 10

                    Repeater {
                        model: view.pair ? [view.pair.first, view.pair.second] : []

                        delegate: Rectangle {
                            id: side

                            required property var modelData
                            required property int index
                            readonly property bool chosen: view.kept === (side.index === 0 ? "first" : "second")

                            Layout.fillWidth: true
                            Layout.preferredWidth: 1
                            Layout.alignment: Qt.AlignTop
                            implicitHeight: sideColumn.implicitHeight + 20
                            radius: view.theme.radius
                            color: "transparent"
                            border.color: side.chosen ? view.theme.accent : view.theme.line
                            border.width: side.chosen ? 2 : 1

                            ColumnLayout {
                                id: sideColumn

                                anchors.left: parent.left
                                anchors.right: parent.right
                                anchors.top: parent.top
                                anchors.margins: 10
                                spacing: 4

                                RowLayout {
                                    Layout.fillWidth: true
                                    spacing: 10

                                    Avatar {
                                        Layout.alignment: Qt.AlignTop
                                        theme: view.theme
                                        source: side.modelData.photo
                                        name: side.modelData.name
                                        size: 40
                                    }
                                    ColumnLayout {
                                        Layout.fillWidth: true
                                        spacing: 2

                                        Label {
                                            Layout.fillWidth: true
                                            text: side.modelData.name
                                            textFormat: Text.PlainText
                                            wrapMode: Text.Wrap
                                            font.weight: Font.DemiBold
                                            color: view.theme.text
                                        }
                                        Label {
                                            Layout.fillWidth: true
                                            text: side.modelData.book
                                            textFormat: Text.PlainText
                                            elide: Text.ElideRight
                                            font.pixelSize: 12
                                            color: view.theme.muted
                                        }
                                    }
                                }
                                Label {
                                    visible: side.modelData.work !== ""
                                    Layout.fillWidth: true
                                    text: side.modelData.work
                                    textFormat: Text.PlainText
                                    wrapMode: Text.Wrap
                                    color: view.theme.muted
                                }
                                Repeater {
                                    model: side.modelData.phones.map(p => ["call-start", p]).concat(side.modelData.emails.map(e => ["mail-message", e])).concat(side.modelData.addresses.map(a => ["mark-location", a]))

                                    delegate: RowLayout {
                                        id: valueRow

                                        required property var modelData

                                        Layout.fillWidth: true
                                        spacing: 6

                                        Icon {
                                            Layout.alignment: Qt.AlignTop
                                            iconName: valueRow.modelData[0]
                                            size: 16
                                        }
                                        Label {
                                            Layout.fillWidth: true
                                            text: valueRow.modelData[1]
                                            textFormat: Text.PlainText
                                            wrapMode: Text.WrapAnywhere
                                            color: view.theme.text
                                        }
                                    }
                                }
                                Label {
                                    visible: side.modelData.birthday !== ""
                                    Layout.fillWidth: true
                                    text: view.sioul.text("contact-birthday") + "  " + side.modelData.birthday
                                    textFormat: Text.PlainText
                                    wrapMode: Text.Wrap
                                    color: view.theme.muted
                                }
                                Label {
                                    visible: side.modelData.categories.length > 0
                                    Layout.fillWidth: true
                                    text: side.modelData.categories.join(" · ")
                                    textFormat: Text.PlainText
                                    wrapMode: Text.Wrap
                                    font.pixelSize: 13
                                    color: view.theme.muted
                                }
                                Label {
                                    visible: side.modelData.notes !== ""
                                    Layout.fillWidth: true
                                    text: side.modelData.notes
                                    textFormat: Text.PlainText
                                    wrapMode: Text.Wrap
                                    maximumLineCount: 3
                                    elide: Text.ElideRight
                                    font.pixelSize: 13
                                    color: view.theme.muted
                                }
                                Button {
                                    flat: true
                                    text: view.sioul.text("ui-open-contact")
                                    onClicked: view.openContact(side.modelData.key)
                                }
                            }
                        }
                    }
                }
                Label {
                    visible: view.pair !== null
                    Layout.fillWidth: true
                    text: view.pair ? view.pair.share : ""
                    textFormat: Text.PlainText
                    wrapMode: Text.Wrap
                    color: view.theme.text
                }
                // Whose name the card keeps, when they differ.
                ColumnLayout {
                    visible: view.pair !== null && view.pair.first.name !== view.pair.second.name
                    Layout.fillWidth: true
                    spacing: 0

                    Label {
                        text: view.sioul.text("dup-keep-name")
                        color: view.theme.muted
                    }
                    RadioButton {
                        Layout.fillWidth: true
                        text: view.pair ? view.pair.first.name : ""
                        checked: view.kept === "first"
                        onClicked: view.kept = "first"
                    }
                    RadioButton {
                        Layout.fillWidth: true
                        text: view.pair ? view.pair.second.name : ""
                        checked: view.kept === "second"
                        onClicked: view.kept = "second"
                    }
                }
                Label {
                    visible: view.pair !== null
                    Layout.fillWidth: true
                    text: view.sioul.text("dup-merge-what")
                    textFormat: Text.PlainText
                    wrapMode: Text.Wrap
                    font.pixelSize: 13
                    color: view.theme.muted
                }
                Flow {
                    visible: view.pair !== null
                    Layout.fillWidth: true
                    spacing: 8

                    Button {
                        text: view.sioul.text("dup-merge")
                        highlighted: true
                        onClicked: view.merge()
                    }
                    Button {
                        text: view.sioul.text("dup-not-same")
                        onClicked: view.notSame()
                    }
                    Button {
                        text: view.sioul.text("dup-later")
                        onClicked: view.later()
                    }
                }

                // What was done lately, each with Undo.
                Label {
                    visible: view.found !== null && view.found.done.length > 0
                    Layout.fillWidth: true
                    Layout.topMargin: 14
                    text: view.sioul.text("dup-done-title")
                    font.pixelSize: 16
                    font.weight: Font.DemiBold
                    wrapMode: Text.Wrap
                    color: view.theme.accent
                }
                Label {
                    visible: view.found !== null && view.found.done.length > 0
                    Layout.fillWidth: true
                    text: view.found ? view.found.kept : ""
                    textFormat: Text.PlainText
                    wrapMode: Text.Wrap
                    font.pixelSize: 13
                    color: view.theme.muted
                }
                Repeater {
                    model: view.found ? view.found.done.slice(0, 10) : []

                    delegate: RowLayout {
                        id: done

                        required property var modelData

                        Layout.fillWidth: true
                        spacing: 8

                        Label {
                            Layout.fillWidth: true
                            text: done.modelData.said + "  ·  " + done.modelData.when
                            textFormat: Text.PlainText
                            wrapMode: Text.Wrap
                            color: view.theme.text
                        }
                        Button {
                            text: view.sioul.text("ui-undo")
                            onClicked: view.problem = view.sioul.undoContacts(done.modelData.id)
                        }
                    }
                }
            }
        }
    }
}
