// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

// On a phone's Porch: what Android has not allowed Sioul yet that it needs
// here (access.rs; docs/android.md, "What Android withholds"): the texts'
// permissions while the part Texts is on, notification access, the contacts
// where calls are screened or the phone's messages go to your computers.
// Each thing in a line with what Android wants: Allow in Android…, and,
// where Android keeps restricted settings from Sioul installed from a file,
// Sioul's app info first. Said once: Not now puts the card away for those
// things for good (a view flag each), and a thing Android allows leaves it.
// Never counted, never red. Nothing on a computer.

pragma ComponentBehavior: Bound

import QtQuick
import QtQuick.Controls.Basic
import QtQuick.Layouts

Panel {
    id: card

    required property var sioul

    // access.rs's lines: {what, said, restricted}.
    property var lines: []
    // Those not put away: read again with the lines (a view flag is no binding).
    property var shown: []

    function reload() {
        const answer = JSON.parse(card.sioul.texts("access", "{}") || "null") || {}
        card.lines = answer.lines || []
        card.shown = card.lines.filter(l => !card.sioul.viewFlag("porch-access-left-" + l.what))
    }

    function ask(what) {
        card.sioul.texts("access-ask", JSON.stringify({ what: what }))
        asking.left = 15
        asking.restart()
    }

    function info() {
        card.sioul.texts("access-info", "{}")
        asking.left = 15
        asking.restart()
    }

    function notNow() {
        for (const line of card.shown)
            card.sioul.setViewFlag("porch-access-left-" + line.what, true)
        card.reload()
    }

    visible: card.shown.length > 0
    Component.onCompleted: card.reload()

    // Android's question or page answered, the Porch's news: read again.
    Connections {
        target: card.sioul

        function onPorchChanged() {
            card.reload()
        }
    }

    // While Android's question or page is open: what it gave read again every two seconds, half a minute at most.
    Timer {
        id: asking

        property int left: 0

        interval: 2000
        repeat: true
        onTriggered: {
            card.reload()
            asking.left -= 1
            if (asking.left <= 0 || card.shown.length === 0)
                asking.stop()
        }
    }

    ColumnLayout {
        anchors.fill: parent
        spacing: 8

        Label {
            Layout.fillWidth: true
            text: card.sioul.text("access-title")
            textFormat: Text.PlainText
            wrapMode: Text.Wrap
            font.weight: Font.DemiBold
            color: card.theme.text
        }
        Repeater {
            model: card.shown

            delegate: ColumnLayout {
                id: line

                required property var modelData

                Layout.fillWidth: true
                spacing: 4

                Label {
                    Layout.fillWidth: true
                    text: line.modelData.said
                    textFormat: Text.PlainText
                    wrapMode: Text.Wrap
                    color: card.theme.text
                }
                Flow {
                    Layout.fillWidth: true
                    spacing: 6

                    Button {
                        visible: line.modelData.restricted === true
                        width: Math.min(implicitWidth, parent ? parent.width : implicitWidth)
                        text: card.sioul.text("appnotes-open-info")
                        onClicked: card.info()
                    }
                    Button {
                        width: Math.min(implicitWidth, parent ? parent.width : implicitWidth)
                        text: card.sioul.text("texts-setup-ask")
                        onClicked: card.ask(line.modelData.what)
                    }
                }
            }
        }
        Button {
            flat: true
            text: card.sioul.text("access-not-now")
            onClicked: card.notNow()
        }
    }
}
