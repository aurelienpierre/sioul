// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

// Sioul's own spam filter, under its settings in Settings ▸ Mail
// (crates/sioul-app/src/spam.rs, docs/spam-filter.md): the table in use and
// what it measured; "Train now", the training's progress with "Stop", what
// the last training found, and what the corpus and the outside material
// hold; and this device's own switches, never shared: whether it trains by
// itself, once a week while it charges and is idle, and on which networks
// its download may go. Every device trains its own, phones included.

pragma ComponentBehavior: Bound

import QtQuick
import QtQuick.Controls.Basic
import QtQuick.Layouts

ColumnLayout {
    id: filter

    required property var sioul
    required property var theme
    // spam.rs's `Status`: {trains, train_about, train_named, table, measured, refused, running, stopping, progress, fraction, ended, by_itself, by_itself_here, by_itself_line, unmetered, last, corpus, outside}.
    property var shown: ({ trains: false, train_about: "", train_named: [], table: "", measured: "", refused: "", running: false, stopping: false, progress: "", fraction: -1, ended: "", by_itself: true, by_itself_here: true, by_itself_line: "", unmetered: false, last: [], corpus: [], outside: [] })
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
        textFormat: Text.PlainText
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

    // "Train now", or the training running, with "Stop".
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
                // The addresses its sentence names: what the press consents to.
                filter.problem = filter.sioul.spamTrain(JSON.stringify(filter.shown.train_named || []))
                filter.read(filter.sioul.spamStatus())
            }
        }
    }
    Label {
        visible: filter.shown.trains && !filter.shown.running
        Layout.fillWidth: true
        // It names the addresses it learns from: pressing Train now consents to that.
        text: filter.shown.train_about
        textFormat: Text.PlainText
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

    // Training here by itself, once a week, charging and idle: this device's
    // own switch; under it, what its last run did, or that the guard turned it off.
    RowLayout {
        visible: filter.shown.trains
        Layout.fillWidth: true
        Layout.topMargin: 4
        spacing: 8

        Switch {
            id: byItself

            checked: filter.shown.by_itself
            enabled: filter.shown.by_itself_here
            Accessible.name: byItselfLabel.text
            onToggled: {
                filter.problem = filter.sioul.setSetting("spam.train_here", JSON.stringify(checked))
                filter.read(filter.sioul.spamStatus())
                byItself.checked = Qt.binding(() => filter.shown.by_itself)
            }
        }
        Label {
            id: byItselfLabel

            Layout.fillWidth: true
            text: filter.sioul.text("spam-app-by-itself")
            wrapMode: Text.Wrap
            lineHeight: 1.2
            color: byItself.enabled ? filter.theme.text : filter.theme.muted
        }
    }
    // The networks its download may use, this device's own: any (unsaid), or Wi-Fi and other unmetered ones.
    ColumnLayout {
        visible: filter.shown.trains
        Layout.fillWidth: true
        spacing: 0

        Repeater {
            model: [{ key: "any", label: "spam-app-network-any" }, { key: "unmetered", label: "spam-app-network-unmetered" }]

            delegate: RadioButton {
                id: choice

                required property var modelData

                objectName: "spamNetwork-" + choice.modelData.key
                Layout.fillWidth: true
                text: filter.sioul.text(choice.modelData.label)
                checked: filter.shown.unmetered === (choice.modelData.key === "unmetered")
                onToggled: {
                    if (!choice.checked)
                        return
                    filter.problem = filter.sioul.setSetting("spam.train_network", JSON.stringify(choice.modelData.key))
                    filter.read(filter.sioul.spamStatus())
                }

                // Wrapped on a phone's width.
                contentItem: Label {
                    leftPadding: choice.indicator.width + choice.spacing
                    text: choice.text
                    textFormat: Text.PlainText
                    wrapMode: Text.Wrap
                    lineHeight: 1.2
                    color: filter.theme.text
                }
            }
        }
    }
    Label {
        visible: filter.shown.trains && filter.shown.by_itself_line !== ""
        Layout.fillWidth: true
        text: filter.shown.by_itself_line
        textFormat: Text.PlainText
        wrapMode: Text.Wrap
        font.pixelSize: 13
        lineHeight: 1.2
        color: filter.theme.muted
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

    // What it learns from, on this device only: your mail, and outside material (`sioul spam import`).
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
        model: filter.shown.trains ? filter.shown.corpus.concat(filter.shown.outside || []) : []

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
