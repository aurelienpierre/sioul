// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

// Sioul's own spam filter, under its settings in Settings ▸ Mail
// (crates/sioul-app/src/spam.rs, docs/spam-filter.md): the table in use and
// where it comes from, with what it measured; on a computer, "Train now",
// the training's progress with "Stop", what the last training found, and
// what the corpus holds. A phone never trains: it says where its table
// comes from, another device's, brought sealed by the sharing.

pragma ComponentBehavior: Bound

import QtQuick
import QtQuick.Controls.Basic
import QtQuick.Layouts

ColumnLayout {
    id: filter

    required property var sioul
    required property var theme
    // spam.rs's `Status`: {trains, table, measured, refused, running, stopping, progress, fraction, ended, last, corpus}.
    property var shown: ({ trains: false, table: "", measured: "", refused: "", running: false, stopping: false, progress: "", fraction: -1, ended: "", last: [], corpus: [] })
    // What it is, in a sentence: the setting's own (settings.rs), said first.
    property string about: ""
    // Why "Train now" did not start, else "".
    property string problem: ""

    function read(text) {
        filter.shown = JSON.parse(text || "null") || filter.shown
    }

    spacing: 6
    Component.onCompleted: filter.read(filter.sioul.spamStatus())
    onVisibleChanged: if (visible) filter.read(filter.sioul.spamStatus())

    Connections {
        target: filter.sioul

        function onSpamChanged(status) {
            filter.read(status)
        }
    }

    Label {
        visible: filter.about !== ""
        Layout.fillWidth: true
        text: filter.about
        wrapMode: Text.Wrap
        font.pixelSize: 13
        lineHeight: 1.25
        color: filter.theme.muted
    }

    // The table in use: where it comes from, and what it measured then.
    Label {
        visible: filter.shown.table !== ""
        Layout.fillWidth: true
        text: filter.shown.table
        textFormat: Text.PlainText
        wrapMode: Text.Wrap
        lineHeight: 1.2
        color: filter.theme.text
    }
    Label {
        visible: filter.shown.measured !== ""
        Layout.fillWidth: true
        text: filter.shown.measured
        textFormat: Text.PlainText
        wrapMode: Text.Wrap
        font.pixelSize: 13
        lineHeight: 1.2
        color: filter.theme.muted
    }
    // A newer table this Sioul cannot use: said calmly, the one before still working.
    Label {
        visible: filter.shown.refused !== ""
        Layout.fillWidth: true
        text: filter.shown.refused
        textFormat: Text.PlainText
        wrapMode: Text.Wrap
        lineHeight: 1.2
        color: filter.theme.text
    }

    // On a computer: "Train now", or the training running, with "Stop".
    Button {
        visible: filter.shown.trains
        Layout.topMargin: 4
        implicitWidth: implicitContentWidth + leftPadding + rightPadding
        text: filter.shown.running ? filter.sioul.text("spam-app-stop") : filter.sioul.text("spam-app-train")
        enabled: !filter.shown.stopping
        onClicked: {
            if (filter.shown.running) {
                filter.sioul.spamStop()
            } else {
                filter.problem = filter.sioul.spamTrain()
                filter.read(filter.sioul.spamStatus())
            }
        }
    }
    Label {
        visible: filter.shown.trains && !filter.shown.running
        Layout.fillWidth: true
        text: filter.sioul.text("spam-app-train-about")
        wrapMode: Text.Wrap
        font.pixelSize: 13
        lineHeight: 1.2
        color: filter.theme.muted
    }
    ProgressBar {
        visible: filter.shown.running
        Layout.fillWidth: true
        from: 0
        to: 1
        indeterminate: filter.shown.fraction < 0
        value: Math.max(0, filter.shown.fraction)
        Accessible.name: progressLine.text
    }
    Label {
        id: progressLine

        visible: filter.shown.running
        Layout.fillWidth: true
        text: filter.shown.stopping ? filter.sioul.text("spam-app-stopping") : filter.shown.progress
        textFormat: Text.PlainText
        wrapMode: Text.Wrap
        lineHeight: 1.2
        color: filter.theme.text
    }
    Label {
        visible: filter.problem !== "" && !filter.shown.running
        Layout.fillWidth: true
        text: filter.problem
        textFormat: Text.PlainText
        wrapMode: Text.Wrap
        color: filter.theme.text
    }
    // How the last run ended when it made no table: stopped, too few messages, the disk.
    Label {
        visible: filter.shown.ended !== "" && !filter.shown.running
        Layout.fillWidth: true
        text: filter.shown.ended
        textFormat: Text.PlainText
        wrapMode: Text.Wrap
        lineHeight: 1.2
        color: filter.theme.text
    }

    // The last training here: what it did with the table, then its numbers.
    Label {
        visible: filter.shown.last.length > 0
        Layout.fillWidth: true
        Layout.topMargin: 8
        text: filter.sioul.text("spam-app-last-title")
        font.weight: Font.DemiBold
        wrapMode: Text.Wrap
        color: filter.theme.text
    }
    Repeater {
        model: filter.shown.last

        delegate: Label {
            required property string modelData
            required property int index

            Layout.fillWidth: true
            text: modelData
            textFormat: Text.PlainText
            wrapMode: Text.Wrap
            font.pixelSize: index === 0 ? 14 : 13
            lineHeight: 1.2
            color: index === 0 ? filter.theme.text : filter.theme.muted
        }
    }

    // What it learns from, on this computer only.
    Label {
        visible: filter.shown.trains && filter.shown.corpus.length > 0
        Layout.fillWidth: true
        Layout.topMargin: 8
        text: filter.sioul.text("spam-app-corpus-title")
        font.weight: Font.DemiBold
        wrapMode: Text.Wrap
        color: filter.theme.text
    }
    Repeater {
        model: filter.shown.trains ? filter.shown.corpus : []

        delegate: Label {
            required property string modelData

            Layout.fillWidth: true
            text: modelData
            textFormat: Text.PlainText
            wrapMode: Text.Wrap
            font.pixelSize: 13
            lineHeight: 1.2
            color: filter.theme.muted
        }
    }
}
