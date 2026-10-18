// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

// Settings ▸ Texts (« SMS »), on a computer (docs/texts.md, "Multimedia
// messages sent from a computer"): how much a multimedia message written
// here may weigh, up to your carrier's limit or 300 kB (for older phones and
// some networks abroad), saved at once; and what your phone said its
// carrier takes, read only, a line for each SIM. The files attached on the
// Texts page are made for the smaller of the two.

pragma ComponentBehavior: Bound

import QtQuick
import QtQuick.Controls.Basic
import QtQuick.Layouts

ColumnLayout {
    id: sending

    required property var sioul
    required property var theme

    // texts.rs's `sending`: {limit: a setting's row, phone: [lines], said (a problem saving)}.
    property var shown: ({ limit: null, phone: [], said: "" })

    function act(verb, args) {
        const answer = JSON.parse(sending.sioul.texts(verb, JSON.stringify(args || {})) || "null")
        if (answer && answer.limit !== undefined)
            sending.shown = answer
    }

    spacing: 6
    Component.onCompleted: sending.act("sending", {})
    onVisibleChanged: {
        if (sending.visible)
            sending.act("sending", {})
    }

    // Your phone's word on its carrier, come through the sharing: read again.
    Connections {
        target: sending.sioul

        function onSharedIn(stores) {
            if (sending.visible && stores.indexOf("data/texts/") >= 0)
                sending.act("sending", {})
        }
    }

    component Line: Label {
        Layout.fillWidth: true
        textFormat: Text.PlainText
        wrapMode: Text.Wrap
        color: sending.theme.text
    }

    Label {
        Layout.fillWidth: true
        Layout.topMargin: 18
        text: sending.sioul.text("texts-sending-title")
        font.pixelSize: 17
        font.weight: Font.DemiBold
        wrapMode: Text.Wrap
        color: sending.theme.accent
    }
    Line {
        text: sending.sioul.text("texts-sending-help")
    }
    SettingRow {
        objectName: "textsSendingLimit"
        visible: !!sending.shown.limit
        Layout.fillWidth: true
        Layout.topMargin: 6
        setting: sending.shown.limit || ({ key: "limit", kind: "note", label: "", help: "", value: "", choices: [] })
        sioul: sending.sioul
        theme: sending.theme
        onSave: (key, value) => sending.act("limit", { value: String(value) })
    }
    // What your phone said its carrier takes: read only.
    Repeater {
        model: sending.shown.phone || []

        delegate: Line {
            required property string modelData

            text: modelData
            color: sending.theme.muted
        }
    }
    Line {
        visible: (sending.shown.said || "") !== ""
        text: sending.shown.said || ""
        color: sending.theme.warm
    }
}
