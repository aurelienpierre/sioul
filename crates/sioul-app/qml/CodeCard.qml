// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

// A code that came by mail, on the Porch (`view::CodeView`): who sent it,
// the code to copy, how long it holds. A click on the card opens its
// message, as the Porch's other cards do; Copy copies, and opens nothing
// (23 October 2026).

import QtQuick
import QtQuick.Controls.Basic
import QtQuick.Layouts

Panel {
    id: codeCard

    required property var sioul
    // {key, title, code, validity, warning}.
    required property var card

    signal copy(string code)
    signal open(string key)

    accent: true
    Accessible.role: Accessible.Button
    Accessible.name: codeCard.card.title || ""

    // Under what it holds, over the whole card, its margins too: Copy,
    // above it, takes its own clicks.
    MouseArea {
        objectName: "codeCardOpen"
        x: -codeCard.leftPadding
        y: -codeCard.topPadding
        width: codeCard.width
        height: codeCard.height
        cursorShape: Qt.PointingHandCursor
        onClicked: codeCard.open(codeCard.card.key || "")
    }

    ColumnLayout {
        anchors.fill: parent
        spacing: 8

        RowLayout {
            spacing: 8

            Icon {
                iconName: "dialog-password"
            }
            // What mail and sites say is plain text: a "<" opens no tag, loads nothing.
            Label {
                Layout.fillWidth: true
                text: codeCard.card.title || ""
                textFormat: Text.PlainText
                wrapMode: Text.Wrap
                color: codeCard.theme.text
            }
        }
        RowLayout {
            visible: !!codeCard.card.code
            spacing: codeCard.theme.gap

            Label {
                text: codeCard.card.code || ""
                textFormat: Text.PlainText
                font.family: codeCard.theme.mono
                font.pixelSize: 30
                font.letterSpacing: 3
                color: codeCard.theme.text
            }
            Button {
                id: copyButton

                property bool done: false

                objectName: "codeCardCopy"
                text: copyButton.done ? codeCard.sioul.text("ui-copied") : codeCard.sioul.text("ui-copy")
                onClicked: {
                    codeCard.copy(codeCard.card.code)
                    copyButton.done = true
                }
            }
        }
        Label {
            visible: !!codeCard.card.validity
            text: codeCard.card.validity || ""
            textFormat: Text.PlainText
            color: codeCard.theme.muted
        }
    }
}
