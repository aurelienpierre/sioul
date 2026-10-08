// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

// A line Sioul says that may hold commands to run by hand, each written in
// backticks in its words (`sudo apt install pcscd`): its words as text, and
// each command on a line of its own, in a read-only field that selects
// (keyboard and mouse) with Copy beside it. Words after a command lose the
// separators that joined them to it (", ", "; "). A line without a command
// reads as a plain label.

pragma ComponentBehavior: Bound

import QtQuick
import QtQuick.Controls.Basic
import QtQuick.Layouts

ColumnLayout {
    id: said

    required property var sioul
    required property var theme
    property string text: ""
    property color color: said.theme.text
    // The words' size; 0 for a label's own.
    property int pixelSize: 0
    // Backticks read as commands; off, the line is words alone, as written.
    property bool commands: true

    // Its parts in order: {command, text}.
    readonly property var parts: {
        const out = []
        const pieces = said.commands ? String(said.text || "").split("`") : [String(said.text || "")]
        // A backtick without its pair is words, as written.
        if (pieces.length % 2 === 0) {
            const last = pieces.pop()
            pieces[pieces.length - 1] += "`" + last
        }
        for (let i = 0; i < pieces.length; i++) {
            const command = i % 2 === 1
            let text = command ? pieces[i].trim() : pieces[i]
            if (!command && i > 0)
                text = text.replace(/^[\s,;]+/, "")
            text = text.trim()
            if (text !== "")
                out.push({ command: command, text: text })
        }
        return out
    }

    spacing: 4

    Repeater {
        model: said.parts

        delegate: RowLayout {
            id: part

            required property var modelData

            Layout.fillWidth: true
            spacing: 6

            // As wide as what is left, however long the words: never wider than the line.
            Label {
                visible: !part.modelData.command
                Layout.fillWidth: true
                Layout.preferredWidth: 1
                text: part.modelData.command ? "" : part.modelData.text
                textFormat: Text.PlainText
                wrapMode: Text.Wrap
                color: said.color

                Binding on font.pixelSize {
                    when: said.pixelSize > 0
                    value: said.pixelSize
                }
            }
            TextField {
                id: command

                visible: part.modelData.command
                Layout.fillWidth: true
                Layout.preferredWidth: 1
                Layout.minimumWidth: 80
                readOnly: true
                selectByMouse: true
                text: part.modelData.command ? part.modelData.text : ""
                font.family: said.theme.mono
                font.pixelSize: 13
                color: said.theme.text
                Accessible.name: part.modelData.text
                // Read from its start: the command's name first.
                onActiveFocusChanged: {
                    if (!command.activeFocus)
                        command.cursorPosition = 0
                }
                Component.onCompleted: command.cursorPosition = 0
            }
            Button {
                id: copy

                visible: part.modelData.command
                flat: true
                implicitWidth: implicitContentWidth + leftPadding + rightPadding
                icon.name: "edit-copy"
                icon.color: said.theme.text
                text: copied.running ? said.sioul.text("ui-copied") : said.sioul.text("ui-copy")
                Accessible.description: part.modelData.text
                onClicked: {
                    command.selectAll()
                    command.copy()
                    command.deselect()
                    copied.restart()
                }

                Timer {
                    id: copied

                    interval: 2000
                }
            }
        }
    }
}
