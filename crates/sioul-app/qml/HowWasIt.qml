// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

// "How was it?", from the status line just after a task is done: its four
// costs and its gain as tiles, what was foreseen a thin mark on each
// (FeltRatings.qml). Over the window, never in its way: not modal, closed by
// Escape or a click elsewhere; nothing is kept unless a tile is tapped.

pragma ComponentBehavior: Bound

import QtQuick
import QtQuick.Controls.Basic
import QtQuick.Layouts

Popup {
    id: popup

    required property var sioul
    required property var theme
    property string uid: ""
    property string title: ""
    property var forecast: ({})
    property var kept: null

    // The task's ratings, read now (it was just written as done).
    function ask(uid) {
        const text = popup.sioul.task(uid)
        if (text === "")
            return
        const task = JSON.parse(text)
        popup.uid = uid
        popup.title = task.card ? task.card.title : task.edit.title
        popup.forecast = task.edit.demands || {}
        // Today's, when some was said today already; an older turn's stays out.
        const d = new Date()
        const pad = n => n < 10 ? "0" + n : String(n)
        const today = d.getFullYear() + "-" + pad(d.getMonth() + 1) + "-" + pad(d.getDate())
        popup.kept = task.edit.felt && task.felt_on === today ? task.edit.felt : null
        problem.text = ""
        popup.open()
    }

    // Kept as felt, and only that: what else the task says is left as it is on disk.
    function keep(values) {
        const said = popup.sioul.setFelt(popup.uid, JSON.stringify(values))
        problem.text = said
        if (said === "")
            popup.kept = values
    }

    parent: Overlay.overlay
    modal: false
    focus: true
    closePolicy: Popup.CloseOnEscape | Popup.CloseOnPressOutside
    // As wide as its tiles side by side, without a cap of its own; never past the
    // window less the usual margins (docs/qt-quick.md, "An editing card's width").
    width: Math.min(ratings.naturalWidth + 2 * popup.padding + 12, (parent ? parent.width : 460) - 2 * popup.theme.gap)
    height: Math.min(implicitHeight, (parent ? parent.height : 800) - 96)
    x: parent ? (parent.width - width) / 2 : 0
    // Above the status line, where "How was it?" was.
    y: parent ? parent.height - height - 48 : 0
    padding: 16

    background: Rectangle {
        color: popup.theme.surface
        radius: popup.theme.radius
        border.color: popup.theme.line
    }

    contentItem: ColumnLayout {
        spacing: 8

        Label {
            Layout.fillWidth: true
            text: popup.sioul.text("felt-ask")
            font.pixelSize: 13
            color: popup.theme.muted
        }
        Label {
            Layout.fillWidth: true
            text: popup.title
            textFormat: Text.PlainText
            wrapMode: Text.Wrap
            font.pixelSize: 17
            color: popup.theme.text
        }
        ScrollView {
            id: scroll

            Layout.fillWidth: true
            Layout.fillHeight: true
            Layout.preferredHeight: ratings.implicitHeight
            contentWidth: availableWidth
            clip: true

            FeltRatings {
                id: ratings

                width: scroll.availableWidth
                sioul: popup.sioul
                theme: popup.theme
                forecast: popup.forecast
                kept: popup.kept
                onGiven: values => popup.keep(values)
            }
        }
        Label {
            id: problem

            visible: text !== ""
            Layout.fillWidth: true
            textFormat: Text.PlainText
            wrapMode: Text.Wrap
            color: popup.theme.warm
        }
        Button {
            Layout.alignment: Qt.AlignRight
            text: popup.sioul.text("ui-close")
            onClicked: popup.close()
        }
    }
}
