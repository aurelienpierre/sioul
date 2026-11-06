// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

// The chain of ties as an outline (ChainPage.qml): the same things as the
// drawing, in the same order, each indented by its step, how it is tied to
// the thing above it on its second line, "+N" at its end. The phone's view,
// and the text alternative everywhere: each row is a tree item whose name
// says its kind, its title, how it is tied and what is left out, and whose
// description says whether it is folded (Qt 6.12's `expanded` until then).

pragma ComponentBehavior: Bound

import QtQuick
import QtQuick.Controls.Basic
import QtQuick.Layouts

ListView {
    id: outline

    required property var sioul
    required property var theme
    // The things, in the outline's order (ChainPage.qml's ListModel).
    required property var things
    // The thing the keys are on.
    property int current: 0

    signal chosen(int place)
    signal opened(int place)
    signal plus(int place)
    signal minus(int place)
    signal menu(int place, var at)

    model: outline.things
    currentIndex: outline.current
    reuseItems: true
    clip: true
    boundsBehavior: Flickable.StopAtBounds
    keyNavigationEnabled: false
    highlightFollowsCurrentItem: false
    Accessible.role: Accessible.Tree
    ScrollBar.vertical: ScrollBar {}

    delegate: ItemDelegate {
        id: row

        required property int index
        // Its roles: kept as they were while a removed row's item goes.
        required property string uri
        required property string kind
        required property string family
        required property string kindWords
        required property string howWords
        required property string title
        required property string detail
        required property string date
        required property bool found
        required property int more
        required property string moreWords
        required property int opened
        required property string name
        required property int column
        readonly property var r: ({ uri: row.uri, kind: row.kind, family: row.family, kindWords: row.kindWords, howWords: row.howWords, title: row.title, detail: row.detail, date: row.date, found: row.found, more: row.more, moreWords: row.moreWords, opened: row.opened, name: row.name, column: row.column })

        width: outline.width
        height: Math.max(outline.theme.touch ? 48 : 40, implicitHeight)
        leftPadding: 8 + 20 * row.r.column
        rightPadding: 4
        focusPolicy: Qt.NoFocus
        opacity: row.r.found ? 1 : 0.5
        onClicked: {
            outline.chosen(row.index)
            outline.opened(row.index)
        }
        onPressAndHold: {
            outline.chosen(row.index)
            outline.menu(row.index, row.mapToItem(null, row.pressX, row.pressY))
        }
        Accessible.role: Accessible.TreeItem
        Accessible.name: row.r.name
        Accessible.description: row.r.more > 0 ? outline.sioul.text("chain-folded") : row.r.opened > 0 ? outline.sioul.text("chain-unfolded") : ""

        TapHandler {
            acceptedButtons: Qt.RightButton
            acceptedDevices: PointerDevice.Mouse | PointerDevice.TouchPad
            onTapped: {
                outline.chosen(row.index)
                outline.menu(row.index, null)
            }
        }

        background: Rectangle {
            color: row.hovered ? outline.theme.hover : "transparent"
            radius: outline.theme.radius
            border.width: row.index === outline.current && outline.activeFocus ? 2 : 0
            border.color: outline.theme.focus
        }

        contentItem: RowLayout {
            spacing: 8

            ChainBadge {
                theme: outline.theme
                family: row.r.family
                kind: row.r.kind
            }
            ColumnLayout {
                Layout.fillWidth: true
                Layout.preferredWidth: 0
                spacing: 0

                // A message's subject, a contact's name: plain text.
                Label {
                    Layout.fillWidth: true
                    text: row.r.title
                    textFormat: Text.PlainText
                    elide: Text.ElideRight
                    font.weight: row.index === 0 ? Font.DemiBold : Font.Normal
                    color: outline.theme.text
                }
                Label {
                    Layout.fillWidth: true
                    text: [row.r.howWords, row.r.kindWords, row.r.date !== "" ? row.r.date : row.r.detail].filter(t => t !== "").join(" · ")
                    textFormat: Text.PlainText
                    elide: Text.ElideRight
                    font.pixelSize: 12
                    color: outline.theme.muted
                }
            }
            // "+N" for what is left out under it; "−" folds back what "+" opened.
            ToolButton {
                visible: row.r.more > 0 || row.r.opened > 0
                Layout.preferredWidth: Math.max(outline.theme.touch ? 48 : 40, implicitWidth)
                Layout.preferredHeight: outline.theme.touch ? 48 : 32
                flat: true
                focusPolicy: Qt.NoFocus
                text: outline.theme.plain(row.r.more > 0 ? "+" + row.r.more : "−")
                Accessible.name: row.r.more > 0 ? row.r.moreWords : outline.sioul.text("chain-minus")
                onClicked: row.r.more > 0 ? outline.plus(row.index) : outline.minus(row.index)
            }
        }
    }
}
