// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

// One row of the Porch (docs/porch.md, "Notices"): a notice, or several of
// one conversation, said the same way whatever it is: who, what and when on
// one line on a computer, on two on a phone; an amount, right-aligned in
// tabular figures; the kind's main action (Open, Reply, Call back, See the
// scan, Open in Budgets); and two quiet actions, Mark read where its item
// has a read state, and Dismiss, which act at once, the page offering ten
// seconds to undo them. On a phone those two are in the row's menu, at a
// long press; on a computer the menu (⋮, a right click, the Menu key or
// Shift+F10) holds the kind's other actions. What a kind shows under its
// row (a voicemail, a reply's box) is the row's own content.

pragma ComponentBehavior: Bound

import QtQml.Models
import QtQuick
import QtQuick.Controls.Basic
import QtQuick.Layouts

ItemDelegate {
    id: row

    required property var sioul
    required property var theme
    // The row's notice (`notices::Notice`): {id, kind, who, what, when, readable, state}.
    required property var notice
    // Two lines, the quiet actions in the menu: a phone, or a window as narrow.
    property bool compact: false
    // Its icon, beside who ("" none), and what resting on it says.
    property string iconName: ""
    property string iconTip: ""
    // An amount, as Budgets writes it ("€62.00"): right-aligned, tabular.
    property string amount: ""
    // A few quiet words beside what (an account ranked above, "maybe spam").
    property var chips: []
    // Its words kept from view (hostile mail): what in italics, muted.
    property bool veiled: false
    // A quiet line under it, when its kind has one to say (a code's warning).
    property string note: ""
    // The kind's main action, beside it ("" none: a click is its Open).
    property string mainText: ""
    property string mainIcon: ""
    // The kind's other actions, in the row's menu: [{id, text, icon}].
    property var more: []
    // What it is, for something new tied to it (AddMenu.qml) and a tie to
    // something that exists: {uri, kind, key, title, name, address, known};
    // null: neither offered. `window` opens what is made.
    property var source: null
    property var window: null
    // What a kind shows under the row: its own items.
    default property alias details: under.data

    signal opened
    signal mainPressed
    signal readPressed
    signal dismissPressed
    signal chosen(string id)

    readonly property bool readable: !!row.notice && row.notice.readable === true

    // The row's menu, made the first time it opens: at its ⋮, else where the pointer or the finger is.
    function showMenu(at) {
        rowMenu.active = true
        const menu = rowMenu.item as SioulMenu
        if (menu === null)
            return
        if (at) {
            const below = at.mapToItem(menu.parent, 0, at.height)
            menu.popup(below.x, below.y)
        } else {
            menu.popup()
        }
    }

    Layout.fillWidth: true
    leftPadding: 8
    rightPadding: 6
    topPadding: row.compact ? 6 : 4
    bottomPadding: row.compact ? 6 : 4
    Accessible.name: [row.notice.who, row.notice.what, row.notice.when, row.amount].filter(t => !!t).join(", ")
    onClicked: row.opened()
    Keys.onReturnPressed: row.opened()
    Keys.onEnterPressed: row.opened()
    Keys.onPressed: event => {
        if (event.key === Qt.Key_Menu || (event.key === Qt.Key_F10 && (event.modifiers & Qt.ShiftModifier))) {
            row.showMenu(null)
            event.accepted = true
        }
    }
    // On a touch screen, the menu comes at a long press, where the finger is; letting go then opens nothing.
    onPressAndHold: {
        row.Window.window.menuAt = row.mapToItem(null, row.pressX, row.pressY)
        row.showMenu(null)
    }
    TapHandler {
        acceptedButtons: Qt.RightButton
        // A touch has no buttons: on a touch screen, the row's long press.
        acceptedDevices: PointerDevice.Mouse | PointerDevice.TouchPad
        onTapped: row.showMenu(null)
    }

    background: Rectangle {
        color: row.highlighted || row.hovered ? row.theme.surface : "transparent"
        radius: row.theme.radius
        border.color: row.visualFocus ? row.theme.focus : row.highlighted ? row.theme.line : "transparent"
        border.width: row.visualFocus ? 2 : 1
    }

    contentItem: ColumnLayout {
        spacing: 2

        // Who, what and when; on a phone what goes under.
        RowLayout {
            Layout.fillWidth: true
            spacing: 8

            Icon {
                visible: row.iconName !== ""
                iconName: row.iconName
                size: 16
                tip: row.iconTip
            }
            Label {
                id: who

                Layout.fillWidth: row.compact
                // Filling a phone's line, it never widens the row past the screen: elided.
                Layout.preferredWidth: row.compact ? 0 : -1
                Layout.maximumWidth: row.compact ? Number.POSITIVE_INFINITY : Math.max(120, row.availableWidth * 0.32)
                text: row.notice.who || ""
                textFormat: Text.PlainText
                font.weight: Font.DemiBold
                elide: Text.ElideRight
                color: row.theme.text
            }
            Label {
                visible: !row.compact
                Layout.fillWidth: true
                // The rest of the line, elided: never what widens the row.
                Layout.preferredWidth: 0
                text: row.notice.what || ""
                textFormat: Text.PlainText
                elide: Text.ElideRight
                font.italic: row.veiled
                color: row.veiled ? row.theme.muted : row.theme.text
            }
            Repeater {
                model: row.compact ? [] : row.chips

                // In words, quiet: never a coloured mark.
                delegate: Rectangle {
                    id: chip

                    required property string modelData

                    implicitWidth: chipText.implicitWidth + 12
                    implicitHeight: chipText.implicitHeight + 4
                    radius: height / 2
                    color: "transparent"
                    border.color: row.theme.line

                    Label {
                        id: chipText

                        anchors.centerIn: parent
                        text: chip.modelData
                        textFormat: Text.PlainText
                        font.pixelSize: 11
                        color: row.theme.muted
                    }
                }
            }
            Label {
                visible: text !== ""
                text: row.notice.when || ""
                textFormat: Text.PlainText
                font.pixelSize: 13
                font.features: { "tnum": 1 }
                color: row.theme.muted
            }
            // An amount, right-aligned before the row's actions: amounts line up.
            Label {
                visible: row.amount !== ""
                Layout.minimumWidth: implicitWidth
                horizontalAlignment: Text.AlignRight
                text: row.amount
                textFormat: Text.PlainText
                font.features: { "tnum": 1 }
                color: row.theme.text
            }
            Button {
                id: main

                visible: row.mainText !== "" && !row.compact
                flat: true
                font.pixelSize: 13
                text: row.theme.plain(row.mainText)
                icon.name: row.mainIcon
                icon.color: row.theme.text
                Accessible.name: (row.notice.who || "") + ", " + text
                onClicked: row.mainPressed()
            }
            // A phone's row with nothing more to say: its main action on its one line.
            ToolButton {
                visible: row.compact && row.mainText !== "" && (row.notice.what || "") === ""
                icon.name: row.mainIcon !== "" ? row.mainIcon : "go-next"
                icon.color: row.theme.text
                Accessible.name: row.mainText
                onClicked: row.mainPressed()
            }
            ToolButton {
                objectName: "readButton"
                visible: row.readable && !row.compact
                icon.name: "dialog-ok-apply"
                icon.color: row.theme.muted
                Accessible.name: row.sioul.text("notice-read")
                ToolTip.visible: hovered
                ToolTip.text: row.sioul.text("notice-read")
                ToolTip.delay: 400
                onClicked: row.readPressed()
            }
            ToolButton {
                objectName: "dismissButton"
                visible: !row.compact
                icon.name: "window-close"
                icon.color: row.theme.muted
                Accessible.name: row.sioul.text("notice-dismiss")
                ToolTip.visible: hovered
                ToolTip.text: row.sioul.text("notice-dismiss")
                ToolTip.delay: 400
                onClicked: row.dismissPressed()
            }
            ToolButton {
                id: moreButton

                visible: !row.compact && row.more.length > 0
                icon.name: "overflow-menu"
                icon.color: row.theme.text
                Accessible.name: row.sioul.text("ui-more")
                ToolTip.visible: hovered
                ToolTip.text: row.sioul.text("ui-more")
                ToolTip.delay: 400
                onClicked: row.showMenu(moreButton)
            }
        }
        // A phone's second line: what, its main action as its icon.
        RowLayout {
            visible: row.compact && (row.notice.what || "") !== ""
            Layout.fillWidth: true
            Layout.leftMargin: row.iconName !== "" ? 16 + 8 : 0
            spacing: 6

            Label {
                Layout.fillWidth: true
                Layout.preferredWidth: 0
                text: row.notice.what || ""
                textFormat: Text.PlainText
                elide: Text.ElideRight
                font.italic: row.veiled
                color: row.veiled ? row.theme.muted : row.theme.text
            }
            ToolButton {
                visible: row.mainText !== ""
                icon.name: row.mainIcon !== "" ? row.mainIcon : "go-next"
                icon.color: row.theme.text
                Accessible.name: row.mainText
                onClicked: row.mainPressed()
            }
        }
        Label {
            visible: row.note !== ""
            Layout.fillWidth: true
            Layout.leftMargin: row.iconName !== "" ? 16 + 8 : 0
            text: row.note
            textFormat: Text.PlainText
            wrapMode: Text.Wrap
            font.pixelSize: 13
            color: row.theme.muted
        }
        // What its kind shows under it.
        ColumnLayout {
            id: under

            visible: children.length > 0
            Layout.fillWidth: true
            Layout.leftMargin: row.iconName !== "" ? 16 + 8 : 0
            spacing: 4
        }
    }

    // Mark read and Dismiss, then the kind's own; on a phone its main action
    // first; then something new tied to it, and a tie to what exists.
    Loader {
        id: rowMenu

        active: false
        sourceComponent: SioulMenu {
            id: menu

            Component.onCompleted: {
                if (row.source === null || row.window === null)
                    return
                menu.addMenu(adding.createObject(menu))
                menu.addItem(linking.createObject(menu))
            }

            MenuItem {
                visible: row.compact && row.mainText !== ""
                height: visible ? implicitHeight : 0
                text: row.theme.plain(row.mainText)
                icon.name: row.mainIcon
                icon.color: row.theme.text
                onTriggered: row.mainPressed()
            }
            MenuItem {
                visible: row.readable
                height: visible ? implicitHeight : 0
                text: row.sioul.text("notice-read")
                icon.name: "dialog-ok-apply"
                icon.color: row.theme.text
                onTriggered: row.readPressed()
            }
            MenuItem {
                text: row.sioul.text("notice-dismiss")
                icon.name: "window-close"
                icon.color: row.theme.text
                onTriggered: row.dismissPressed()
            }
            Instantiator {
                model: row.more

                delegate: MenuItem {
                    required property var modelData

                    text: row.theme.plain(modelData.text)
                    icon.name: modelData.icon || ""
                    icon.color: row.theme.text
                    onTriggered: row.chosen(modelData.id)
                }
                onObjectAdded: (index, object) => menu.insertItem(3 + index, object)
                onObjectRemoved: (index, object) => menu.removeItem(object)
            }
        }
    }
    Component {
        id: adding

        AddMenu {
            sioul: row.sioul
            window: row.window
            source: row.source
        }
    }
    Component {
        id: linking

        MenuItem {
            enabled: row.source !== null && row.source.uri !== ""
            text: row.sioul.text("ui-link-existing")
            onTriggered: row.window.linkFrom(row.source)
        }
    }
}
