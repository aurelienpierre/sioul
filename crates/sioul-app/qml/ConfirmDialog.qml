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
    // The sentence holds commands in backticks, Sioul's own (how to install
    // an antivirus): each shown to copy (CommandText.qml). Off unless asked:
    // a name from a message is never taken for a command.
    property bool commands: false

    signal confirmed

    // Asks, then calls `confirmed` if the action is chosen. The heading and the
    // sentence may name what came in a message (an attachment): plain text.
    function ask(heading, sentence, action, commands) {
        dialog.title = dialog.theme.plain(heading)
        dialog.sentence = sentence
        dialog.action = action
        dialog.commands = commands === true
        dialog.open()
    }

    parent: Overlay.overlay
    anchors.centerIn: parent
    modal: true
    width: Math.min(440, (parent ? parent.width : 440) - 2 * dialog.theme.gap)

    // A command in the sentence (how to install an antivirus) to copy.
    contentItem: CommandText {
        sioul: dialog.sioul
        theme: dialog.theme
        text: dialog.sentence
        commands: dialog.commands
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
