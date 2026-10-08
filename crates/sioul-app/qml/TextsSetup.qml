// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

// Settings ▸ This phone ▸ Texts (« SMS »), on a phone (docs/texts.md): what
// Sioul does with the phone's texts once you allow it, each permission with
// the sentence that says why, Android's question for them, and the
// sharing's part "Texts" (off until you turn it on, here and on each computer
// that should read or write them); what the phone holds and how much has gone
// to your computers, read again while the import runs; the largest media file
// brought there. Sioul never asks to be the SMS app: the phone's messages app
// stays their record.

pragma ComponentBehavior: Bound

import QtQuick
import QtQuick.Controls.Basic
import QtQuick.Layouts

ColumnLayout {
    id: setup

    required property var sioul
    required property var theme

    // texts.rs's `setup`: {android, state: {read, receive, send, phone_state, sms_app, sims}, on, sharing, progress, importing, cap, said}.
    property var shown: ({ android: false, state: null, on: false, sharing: false, progress: "", importing: false, cap: null, said: "" })
    readonly property var allowed: setup.shown.state || ({})
    readonly property bool all: setup.allowed.read === true && setup.allowed.receive === true && setup.allowed.send === true

    function act(verb, args) {
        const answer = JSON.parse(setup.sioul.texts(verb, JSON.stringify(args || {})) || "null")
        if (answer && answer.android !== undefined)
            setup.shown = answer
    }

    visible: setup.shown.android === true
    spacing: 6
    Component.onCompleted: setup.act("setup", {})

    // While Android's question is open, what it gave read again every two seconds, half a minute at most.
    Timer {
        id: asking

        property int left: 0

        interval: 2000
        repeat: true
        onTriggered: {
            setup.act("setup", {})
            asking.left -= 1
            if (asking.left <= 0 || setup.all)
                asking.stop()
        }
    }

    // While the history goes to your computers: its progress read again every ten seconds.
    Timer {
        interval: 10000
        repeat: true
        running: setup.visible && setup.shown.on === true && setup.shown.importing === true
        onTriggered: setup.act("setup", {})
    }

    component Line: Label {
        Layout.fillWidth: true
        textFormat: Text.PlainText
        wrapMode: Text.Wrap
        color: setup.theme.text
    }

    Label {
        Layout.fillWidth: true
        Layout.topMargin: 18
        text: setup.sioul.text("texts-setup-title")
        font.pixelSize: 17
        font.weight: Font.DemiBold
        wrapMode: Text.Wrap
        color: setup.theme.accent
    }
    Line {
        text: setup.sioul.text("texts-setup-help")
    }
    // Each permission and why, then whether Android gave it.
    Repeater {
        model: [["read", "texts-setup-why-read"], ["receive", "texts-setup-why-receive"], ["send", "texts-setup-why-send"], ["phone_state", "texts-setup-why-sims"]]

        delegate: Line {
            required property var modelData

            text: setup.sioul.text(modelData[1]) + " " + setup.sioul.text(setup.allowed[modelData[0]] === true ? "texts-setup-given" : "texts-setup-not-given")
            color: setup.theme.text
        }
    }
    Button {
        visible: !setup.all || setup.allowed.phone_state !== true
        text: setup.sioul.text("texts-setup-ask")
        onClicked: {
            setup.act("ask", {})
            asking.left = 15
            asking.restart()
        }
    }
    Line {
        visible: setup.shown.sharing !== true
        text: setup.sioul.text("phonemsgs-setup-no-sharing")
        color: setup.theme.muted
    }
    SettingRow {
        visible: setup.shown.sharing === true
        Layout.fillWidth: true
        Layout.topMargin: 6
        setting: ({ key: "part", kind: "bool", label: setup.sioul.text("share-part-texts"), help: setup.sioul.text("share-part-texts-carries"), value: setup.shown.on === true, choices: [] })
        sioul: setup.sioul
        theme: setup.theme
        onSave: (key, value) => setup.act("part", { on: value === true })
    }
    Line {
        visible: (setup.shown.progress || "") !== ""
        text: setup.shown.progress || ""
    }
    SettingRow {
        visible: setup.shown.sharing === true && !!setup.shown.cap
        Layout.fillWidth: true
        setting: setup.shown.cap || ({ key: "cap", kind: "note", label: "", help: "", value: "", choices: [] })
        sioul: setup.sioul
        theme: setup.theme
        onSave: (key, value) => setup.act("cap", { value: String(value) })
    }
    Line {
        visible: (setup.shown.said || "") !== ""
        text: setup.shown.said || ""
        color: setup.theme.muted
    }
    Line {
        text: setup.sioul.text("texts-page-never")
        color: setup.theme.muted
    }
}
