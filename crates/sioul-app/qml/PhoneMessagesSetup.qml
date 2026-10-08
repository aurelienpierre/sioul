// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

// Settings ▸ This phone ▸ On your computers (« Sur vos ordinateurs »), on a
// phone (docs/android.md, "Messages on your computers"): what this phone's
// notifications send to your computers' Porch, sealed through your sharing.
// The part's switch (off until you turn it on, here and on each computer
// that should show them), what never leaves the phone, then each app seen
// lately: not on your computers, who and when, or the words too. The SMS app
// sends its words unless you say otherwise; every other app sends nothing
// until you choose it. Nothing here touches what reaches you on the phone.

pragma ComponentBehavior: Bound

import QtQuick
import QtQuick.Controls.Basic
import QtQuick.Layouts

ColumnLayout {
    id: setup

    required property var sioul
    required property var theme

    // phonemsgs.rs's `setup`: {android, sharing, part: a SettingRow's row, apps: [rows]}.
    property var shown: ({ android: false, sharing: false, part: null, apps: [] })
    property string said: ""

    function act(verb, args) {
        const answer = JSON.parse(setup.sioul.phoneMessages(verb, JSON.stringify(args || {})) || "null")
        if (answer && answer.apps !== undefined) {
            setup.shown = answer
            setup.said = answer.said || ""
        }
    }

    visible: setup.shown.android === true
    spacing: 6
    Component.onCompleted: setup.act("setup", {})

    Label {
        Layout.fillWidth: true
        Layout.topMargin: 18
        text: setup.sioul.text("phonemsgs-setup-title")
        font.pixelSize: 17
        font.weight: Font.DemiBold
        wrapMode: Text.Wrap
        color: setup.theme.accent
    }
    Label {
        Layout.fillWidth: true
        text: setup.sioul.text("phonemsgs-setup-help")
        textFormat: Text.PlainText
        wrapMode: Text.Wrap
        color: setup.theme.text
    }
    Label {
        Layout.fillWidth: true
        text: setup.sioul.text("phonemsgs-setup-never")
        textFormat: Text.PlainText
        wrapMode: Text.Wrap
        color: setup.theme.muted
    }
    Label {
        visible: setup.shown.sharing !== true
        Layout.fillWidth: true
        text: setup.sioul.text("phonemsgs-setup-no-sharing")
        textFormat: Text.PlainText
        wrapMode: Text.Wrap
        color: setup.theme.muted
    }
    SettingRow {
        visible: setup.shown.sharing === true && setup.shown.part !== null
        Layout.fillWidth: true
        Layout.topMargin: 6
        setting: setup.shown.part || { key: "part", kind: "bool", label: "", help: "", value: false, choices: [] }
        sioul: setup.sioul
        theme: setup.theme
        onSave: (key, value) => setup.act("part", { on: value === true })
    }
    Label {
        visible: setup.said !== ""
        Layout.fillWidth: true
        text: setup.said
        textFormat: Text.PlainText
        wrapMode: Text.Wrap
        color: setup.theme.muted
    }

    Label {
        Layout.fillWidth: true
        Layout.topMargin: 12
        text: setup.sioul.text("phonemsgs-setup-apps")
        font.weight: Font.DemiBold
        wrapMode: Text.Wrap
        color: setup.theme.text
    }
    Label {
        Layout.fillWidth: true
        text: setup.sioul.text(setup.shown.apps.length > 0 ? "phonemsgs-setup-apps-help" : "phonemsgs-setup-none")
        textFormat: Text.PlainText
        wrapMode: Text.Wrap
        color: setup.theme.muted
    }
    Repeater {
        model: setup.shown.apps

        delegate: SettingRow {
            required property var modelData

            Layout.fillWidth: true
            Layout.topMargin: 4
            setting: modelData
            sioul: setup.sioul
            theme: setup.theme
            onSave: (key, value) => setup.act("set", { key: key, value: value })
        }
    }
}
