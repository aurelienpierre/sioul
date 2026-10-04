// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

// Asked once, before something that cannot be undone: what happens, in a
// sentence, and the action named by what it does. "Cancel" is the default.

import QtQuick
import QtQuick.Controls.Basic

Dialog {
    id: dialog

    required property var sioul
    required property var theme
    property string sentence: ""
    property string action: ""

    signal confirmed

    // Asks, then calls `confirmed` if the action is chosen. The heading and the
    // sentence may name what came in a message (an attachment): plain text.
    function ask(heading, sentence, action) {
        dialog.title = dialog.theme.plain(heading)
        dialog.sentence = sentence
        dialog.action = action
        dialog.open()
    }

    parent: Overlay.overlay
    anchors.centerIn: parent
    modal: true
    width: Math.min(440, (parent ? parent.width : 440) - 2 * dialog.theme.gap)

    contentItem: Label {
        text: dialog.sentence
        textFormat: Text.PlainText
        wrapMode: Text.Wrap
        color: dialog.theme.text
    }

    footer: DialogButtonBox {
        Button {
            text: dialog.action
            DialogButtonBox.buttonRole: DialogButtonBox.AcceptRole
        }
        Button {
            text: dialog.sioul.text("ui-cancel")
            highlighted: true
            DialogButtonBox.buttonRole: DialogButtonBox.RejectRole
        }
    }

    onAccepted: dialog.confirmed()
}
