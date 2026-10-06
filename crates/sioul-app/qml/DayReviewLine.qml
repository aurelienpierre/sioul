// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

// Under a day on the Health page (docs/reviews.md): how the day went, in the
// words said then, and its notes; today, in the evening and during the
// night, "Close the day", which opens its review (DayReview.qml). Read only
// for the days before. Nothing at all when nothing was said: no gap, no mark.

pragma ComponentBehavior: Bound

import QtQuick
import QtQuick.Controls.Basic
import QtQuick.Layouts

ColumnLayout {
    id: line

    required property var sioul
    required property var theme
    required property var window
    // The day shown ("2026-10-06").
    property string date: ""
    // `reviews::line`: said, words, memos, close, button.
    property var shown: null

    function reload() {
        line.shown = line.date !== "" ? JSON.parse(line.sioul.dayReviewLine(line.date) || "null") : null
    }

    onDateChanged: line.reload()
    Component.onCompleted: line.reload()
    // Each minute, and once a review is given: the evening comes, the words are kept.
    Connections {
        target: line.sioul

        function onModeChanged() {
            line.reload()
        }
    }

    visible: line.shown !== null && (line.shown.words !== "" || line.shown.close)
    spacing: 4

    Label {
        visible: line.shown !== null && line.shown.words !== ""
        Layout.fillWidth: true
        text: line.shown ? line.shown.said + " " + line.shown.words : ""
        textFormat: Text.PlainText
        wrapMode: Text.Wrap
        font.pixelSize: 13
        color: line.theme.muted
    }
    Repeater {
        model: line.shown ? line.shown.memos : []

        delegate: TextEdit {
            required property string modelData

            Layout.fillWidth: true
            text: line.theme.spaced(modelData.replace(/<a href=/g, '<a style="color:' + line.theme.accent + '" href='))
            textFormat: TextEdit.RichText
            readOnly: true
            selectByMouse: true
            wrapMode: TextEdit.Wrap
            color: line.theme.text
            onLinkActivated: link => Qt.openUrlExternally(link)
        }
    }
    Button {
        visible: line.shown !== null && line.shown.close
        flat: true
        text: line.shown ? line.shown.button : ""
        onClicked: line.window.reviewDay("night")
    }
}
