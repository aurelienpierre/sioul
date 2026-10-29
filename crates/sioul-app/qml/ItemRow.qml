// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

// One thing in a list of days: What comes, a phone's Month (the unified
// agenda, §4.6): its time, its mark, its title and its line. The mark says
// what it is: a calendar's colour dot for an appointment or a pinned time, a
// diamond for a date asked (filled within seven days), a dose's mark from
// what is known of it (DoseMark.qml: never a medicine's name, never "not
// taken"), a tint for your own times (a meal, a nap, the night). A tap opens
// its card where you are; a right click or a long press its menu. A
// cancelled appointment is struck through, a tentative one in grey.

pragma ComponentBehavior: Bound

import QtQuick
import QtQuick.Controls.Basic
import QtQuick.Layouts

ItemDelegate {
    id: row

    required property var theme
    // "09:00 – 10:30", "All day", "12:30".
    property string when: ""
    property string title: ""
    property string line: ""
    // The item's kind ("event", "block", "step", "dated", "day-off", "meal", "nap", "night", "dose").
    property string kind: ""
    property string tone: "plain"
    property color colour: "transparent"
    // A dose's state as the window knows it ("come", "check", "reading"…), "" when none is drawn.
    property string known: ""
    property bool cancelled: false
    property bool tentative: false
    property bool compact: false
    property bool selected: false
    // The width the times take, the same for every row of a list.
    property real timeWidth: 96

    signal open
    signal menu

    padding: row.compact ? 4 : 6
    highlighted: row.selected
    onClicked: row.open()
    Keys.onReturnPressed: row.open()
    Accessible.name: [row.title, row.when, row.line].filter(t => t !== "").join(", ")

    background: Rectangle {
        color: row.highlighted || row.hovered ? row.theme.surface : "transparent"
        radius: row.theme.radius
        border.color: row.visualFocus ? row.theme.focus : row.highlighted ? row.theme.line : "transparent"
    }

    // On a touch screen, the menu comes at a long press; letting go then opens nothing.
    onPressAndHold: {
        row.Window.window.menuAt = row.mapToItem(null, row.pressX, row.pressY)
        row.menu()
    }
    TapHandler {
        acceptedButtons: Qt.RightButton
        acceptedDevices: PointerDevice.Mouse | PointerDevice.TouchPad
        onTapped: row.menu()
    }

    contentItem: RowLayout {
        spacing: 8

        Label {
            Layout.preferredWidth: row.timeWidth
            Layout.alignment: Qt.AlignTop
            text: row.when
            textFormat: Text.PlainText
            elide: Text.ElideRight
            font.pixelSize: row.compact ? 12 : 14
            font.features: { "tnum": 1 }
            color: row.theme.muted
        }
        // Its mark: one item, a dot, a diamond or a tint; a dose's own mark beside, made for doses alone.
        Item {
            Layout.preferredWidth: 12
            Layout.preferredHeight: 12
            Layout.alignment: Qt.AlignTop | Qt.AlignHCenter
            Layout.topMargin: 3

            Rectangle {
                objectName: "itemRowMark"
                readonly property bool diamond: row.kind === "dated"
                readonly property bool own: row.kind === "meal" || row.kind === "nap" || row.kind === "night" || row.kind === "day-off"

                visible: row.kind !== "dose"
                anchors.centerIn: parent
                width: diamond ? 7 : 8
                height: diamond ? 7 : 8
                rotation: diamond ? 45 : 0
                radius: diamond || own ? 1 : 4
                color: diamond ? (row.tone !== "quiet" ? row.theme.text : "transparent") : own ? row.theme.line : (row.colour.a > 0 ? row.colour : row.theme.accent)
                border.width: diamond ? 1 : 0
                border.color: row.theme.text
            }
            Loader {
                anchors.centerIn: parent
                active: row.kind === "dose" && row.known !== ""
                sourceComponent: DoseMark {
                    theme: row.theme
                    known: row.known
                }
            }
        }
        // What is left of the row: long words cut short, never a row wider than its list.
        ColumnLayout {
            Layout.fillWidth: true
            Layout.preferredWidth: 0
            spacing: 0

            Label {
                Layout.fillWidth: true
                text: row.title
                textFormat: Text.PlainText
                elide: Text.ElideRight
                font.strikeout: row.cancelled
                font.pixelSize: row.compact ? 13 : 15
                color: row.tentative ? row.theme.muted : row.theme.text
            }
            Label {
                visible: row.line !== ""
                Layout.fillWidth: true
                text: row.line
                textFormat: Text.PlainText
                wrapMode: Text.Wrap
                maximumLineCount: 2
                elide: Text.ElideRight
                font.pixelSize: 12
                color: row.theme.muted
            }
        }
    }
}
