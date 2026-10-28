// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

// Every card's three parts (the unified agenda, part 3): the head, kept in
// view while the card scrolls (its mark, its title as plain text with Edit
// beside it, when, where it comes from, one quiet line of its state, and the
// reason in Edit's place where nothing can be changed here); the body, the
// kind's own; the foot, the kind's own actions, then Close. A card opened
// from another says so at its top, "‹ Doctor", which goes back to it.

import QtQuick
import QtQuick.Controls.Basic
import QtQuick.Layouts

Panel {
    id: frame

    required property var sioul
    required property var host

    // The kind's mark: an icon's name, or a colour dot when there is none.
    property string iconName: ""
    property color dot: "transparent"
    property string title: ""
    property bool struck: false
    // Edit offered (its kind's own form); else `why` says why not, in its place.
    property bool editable: false
    property string why: ""
    property string when: ""
    property string from: ""
    property string stateLine: ""
    // The kind's body, under the head; the kind's actions, in the foot before Close.
    default property alias body: bodyColumn.data
    property alias actions: actionsRow.data

    signal editAsked

    ColumnLayout {
        anchors.fill: parent
        spacing: 8

        // The card you came from, one level deep.
        Label {
            objectName: "cardBack"
            visible: frame.host.previous !== null && !frame.host.window.compact
            Layout.fillWidth: true
            text: "‹  " + (frame.host.previous ? frame.host.previous.title || "" : "")
            textFormat: Text.PlainText
            elide: Text.ElideRight
            color: frame.theme.accent
            Accessible.role: Accessible.Button
            Accessible.name: frame.sioul.text("ui-back")

            TapHandler {
                cursorShape: Qt.PointingHandCursor
                onTapped: frame.host.back()
            }
        }
        RowLayout {
            Layout.fillWidth: true
            spacing: 8

            Icon {
                visible: frame.iconName !== ""
                Layout.alignment: Qt.AlignTop
                iconName: frame.iconName || "view-calendar"
                size: 18
            }
            Rectangle {
                visible: frame.iconName === ""
                Layout.alignment: Qt.AlignTop
                Layout.topMargin: 6
                Layout.preferredWidth: 10
                Layout.preferredHeight: 10
                radius: 5
                color: frame.dot
            }
            Label {
                objectName: "cardTitle"
                Layout.fillWidth: true
                text: frame.title
                textFormat: Text.PlainText
                font.pixelSize: 20
                font.strikeout: frame.struck
                wrapMode: Text.Wrap
                color: frame.theme.text
            }
            Button {
                objectName: "cardEdit"
                visible: frame.editable
                Layout.alignment: Qt.AlignTop
                text: frame.sioul.text("ui-edit")
                icon.name: "document-edit"
                icon.color: frame.theme.text
                onClicked: frame.editAsked()
            }
        }
        Label {
            visible: frame.when !== ""
            Layout.fillWidth: true
            text: frame.when
            textFormat: Text.PlainText
            wrapMode: Text.Wrap
            color: frame.theme.text
        }
        Label {
            visible: frame.from !== ""
            Layout.fillWidth: true
            text: frame.from
            textFormat: Text.PlainText
            wrapMode: Text.Wrap
            color: frame.theme.muted
        }
        Label {
            objectName: "cardState"
            visible: frame.stateLine !== ""
            Layout.fillWidth: true
            text: frame.stateLine
            textFormat: Text.PlainText
            wrapMode: Text.Wrap
            color: frame.theme.muted
        }
        // Where nothing can be changed here, why, in Edit's place.
        Label {
            objectName: "cardWhy"
            visible: !frame.editable && frame.why !== ""
            Layout.fillWidth: true
            text: frame.why
            textFormat: Text.PlainText
            wrapMode: Text.Wrap
            color: frame.theme.muted
        }
        ScrollView {
            id: scroll

            Layout.fillWidth: true
            Layout.fillHeight: true
            contentWidth: availableWidth
            clip: true

            ColumnLayout {
                id: bodyColumn

                width: scroll.availableWidth
                spacing: 8
            }
        }
        RowLayout {
            Layout.fillWidth: true
            spacing: 6

            Flow {
                id: actionsRow

                Layout.fillWidth: true
                spacing: 6
            }
            Button {
                objectName: "cardClose"
                Layout.alignment: Qt.AlignBottom
                text: frame.sioul.text("ui-close")
                onClicked: frame.host.close()
            }
        }
    }
}
