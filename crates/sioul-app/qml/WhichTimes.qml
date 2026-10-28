// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

// One time of a repeating event, or every time: asked before it is moved or
// deleted, one question for both (the agenda's timeline, its details).
// Answered: `chosen(onlyThis)`; closed without: `dropped`, and nothing changes.

import QtQuick
import QtQuick.Controls.Basic
import QtQuick.Layouts

Dialog {
    id: which

    required property var sioul
    required property var theme
    // What is asked, under the title: "Moved to 09:00 – 10:30", or nothing for a deletion.
    property string question: ""
    // Answered: closed on its way; closed without: nothing moves.
    property bool answered: false

    signal chosen(bool onlyThis)
    signal dropped

    function ask(title, question) {
        which.title = which.theme.plain(title)
        which.question = question
        which.answered = false
        which.open()
    }

    function answer(onlyThis) {
        which.answered = true
        which.close()
        which.chosen(onlyThis)
    }

    parent: Overlay.overlay
    anchors.centerIn: parent
    modal: true
    width: Math.min(440, (parent ? parent.width : 440) - 2 * which.theme.gap)
    onClosed: {
        if (!which.answered)
            which.dropped()
    }

    ColumnLayout {
        width: parent.width
        spacing: 8

        Label {
            visible: which.question !== ""
            Layout.fillWidth: true
            text: which.question
            textFormat: Text.PlainText
            wrapMode: Text.Wrap
            color: which.theme.text
        }
        Button {
            objectName: "whichOnlyThis"
            Layout.fillWidth: true
            text: which.sioul.text("agenda-delete-this")
            onClicked: which.answer(true)
        }
        Button {
            objectName: "whichEveryTime"
            Layout.fillWidth: true
            text: which.sioul.text("agenda-delete-all")
            onClicked: which.answer(false)
        }
        Button {
            Layout.alignment: Qt.AlignRight
            text: which.sioul.text("ui-cancel")
            onClicked: which.close()
        }
    }
}
