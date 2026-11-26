// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

// An extension's Details… (docs/extensions.md, "Its log"): the last lines of
// its log on this device, the newest first (`extensions::log_json`), to read
// and to select, with Copy, on every device, a phone included. Sioul takes
// out of each line what it recognises as a password, a token, a cookie, a
// header's value or a web address's query, when the line is written and again
// when it is read here (`state::scrub`); the card asks for a look before sharing.

pragma ComponentBehavior: Bound

import QtQuick
import QtQuick.Controls.Basic
import QtQuick.Layouts

SafeDialog {
    id: card

    // Sioul's backend (backend.rs): its words in your language and what it does.
    required property var sioul
    // The window's colours, sizes and fonts (Theme.qml).
    required property var theme

    // The extension shown, and what the backend gave for it: {title, lines}.
    property string extension: ""
    property var view: ({ title: "", lines: [] })
    readonly property var lines: card.view.lines || []
    // The lines as one text, the newest first: what is shown and what Copy copies.
    readonly property string text: card.lines.join("\n")

    // Its log read now, then shown.
    function show(id) {
        card.extension = id
        card.view = JSON.parse(card.sioul.extensionLog(id) || "{}")
        card.open()
    }

    objectName: "extensionLog"
    title: card.theme.plain(card.view.title || "")
    parent: Overlay.overlay
    anchors.centerIn: parent
    modal: true
    width: Math.min(720, card.roomWidth - 2 * card.theme.gap)
    height: Math.min(640, card.roomHeight - 2 * card.theme.gap)

    // The title wraps: an extension's name is long, a phone narrow.
    header: Label {
        text: card.title
        visible: card.title !== ""
        wrapMode: Text.Wrap
        textFormat: Text.PlainText
        font.bold: true
        padding: 12
        background: Rectangle {
            x: 1
            y: 1
            width: parent.width - 2
            height: parent.height - 1
            color: card.palette.window
        }
    }

    contentItem: ColumnLayout {
        spacing: 8

        Label {
            Layout.fillWidth: true
            text: card.sioul.text("extension-log-about")
            textFormat: Text.PlainText
            wrapMode: Text.Wrap
            color: card.theme.muted
        }
        Label {
            objectName: "extensionLogEmpty"
            Layout.fillWidth: true
            visible: card.lines.length === 0
            text: card.sioul.text("extension-log-empty")
            textFormat: Text.PlainText
            wrapMode: Text.Wrap
            color: card.theme.text
        }
        // Read only, selectable by mouse, keyboard and touch; long lines wrap, never pass the card's edge.
        ScrollView {
            Layout.fillWidth: true
            Layout.fillHeight: true
            visible: card.lines.length > 0
            clip: true
            ScrollBar.horizontal.policy: ScrollBar.AlwaysOff

            TextArea {
                id: shown

                objectName: "extensionLogLines"
                readOnly: true
                selectByMouse: true
                persistentSelection: true
                wrapMode: TextEdit.WrapAtWordBoundaryOrAnywhere
                textFormat: TextEdit.PlainText
                text: card.text
                font.family: card.theme.mono
                font.pixelSize: 12
                color: card.theme.text
                Accessible.name: card.view.title || ""
            }
        }
    }

    footer: DialogButtonBox {
        Button {
            objectName: "extensionLogCopy"
            enabled: card.text !== ""
            icon.name: "edit-copy"
            text: card.sioul.text(copied.running ? "ui-copied" : "ui-copy")
            // On the clipboard; where the system refuses it, the text selected, to copy by hand.
            onClicked: {
                if (card.sioul.copyText(card.text)) {
                    copied.restart()
                } else {
                    shown.forceActiveFocus()
                    shown.selectAll()
                }
            }
        }
        Button {
            objectName: "extensionLogClose"
            text: card.sioul.text("ui-close")
            highlighted: true
            DialogButtonBox.buttonRole: DialogButtonBox.RejectRole
        }
    }

    // "Copied." said on the button for two seconds.
    Timer {
        id: copied

        interval: 2000
    }
}
