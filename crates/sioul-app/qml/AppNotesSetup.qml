// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

// Settings ▸ Other apps, on a phone (docs/android.md, "Notifications from
// other apps"): what Sioul does with other apps' notifications and what it
// never does; Android's notification access and its two steps for an app
// installed from a file; holding on or off; the gathered times; what rang
// before Sioul held it, each with Android's page to make it silent; then
// each app, conversation and site seen, with its choice. On a computer, one
// sentence: this is the phone's.

pragma ComponentBehavior: Bound

import QtQuick
import QtQuick.Controls.Basic
import QtQuick.Layouts

ColumnLayout {
    id: setup

    required property var sioul
    required property var theme
    // appnotes.rs's `setup`: {android, access, restricted, contacts, hold, times, heard, rang, apps, conversations, sites}.
    property var shown: ({ android: false, access: false, restricted: false, contacts: false, hold: null, times: "", heard: "", rang: [], apps: [], conversations: [], sites: [] })

    // Settings ▸ Reminders and notifications, at the gathered times (ParametersPage.qml).
    signal showSetting(string key)

    function reload() {
        setup.shown = JSON.parse(setup.sioul.appNotesSetup() || "null") || setup.shown
    }

    function act(verb, args) {
        const answer = JSON.parse(setup.sioul.appNotesChange(verb, JSON.stringify(args || {})) || "null")
        if (answer)
            setup.shown = answer
    }

    spacing: 6
    Component.onCompleted: setup.reload()
    onVisibleChanged: if (visible) setup.reload()

    Label {
        text: setup.sioul.text("appnotes-title")
        Layout.fillWidth: true
        Layout.topMargin: 18
        font.pixelSize: 17
        font.weight: Font.DemiBold
        wrapMode: Text.Wrap
        color: setup.theme.accent
    }
    Label {
        visible: !setup.shown.android
        text: setup.sioul.text("appnotes-computer")
        Layout.fillWidth: true
        textFormat: Text.PlainText
        wrapMode: Text.Wrap
        color: setup.theme.text
    }
    Label {
        visible: setup.shown.android
        text: setup.sioul.text("appnotes-does")
        Layout.fillWidth: true
        textFormat: Text.PlainText
        wrapMode: Text.Wrap
        color: setup.theme.text
    }
    Label {
        visible: setup.shown.android
        text: setup.sioul.text("appnotes-never")
        Layout.fillWidth: true
        textFormat: Text.PlainText
        wrapMode: Text.Wrap
        color: setup.theme.muted
    }

    // ---------------------------------------------------------------- the access
    Label {
        visible: setup.shown.android
        Layout.topMargin: 12
        text: setup.sioul.text(setup.shown.access ? "appnotes-access-on" : "appnotes-access-off")
        font.weight: Font.DemiBold
        Layout.fillWidth: true
        textFormat: Text.PlainText
        wrapMode: Text.Wrap
        color: setup.theme.text
    }
    Label {
        visible: setup.shown.android && setup.shown.access && setup.shown.heard !== ""
        text: setup.shown.heard
        Layout.fillWidth: true
        textFormat: Text.PlainText
        wrapMode: Text.Wrap
        color: setup.theme.muted
    }
    Label {
        visible: setup.shown.android && !setup.shown.access
        text: setup.sioul.text("appnotes-steps")
        Layout.fillWidth: true
        textFormat: Text.PlainText
        wrapMode: Text.Wrap
        color: setup.theme.muted
    }
    Label {
        visible: setup.shown.android && !setup.shown.access && setup.shown.restricted
        text: setup.sioul.text("appnotes-step-restricted")
        Layout.fillWidth: true
        textFormat: Text.PlainText
        wrapMode: Text.Wrap
        color: setup.theme.muted
    }
    Label {
        visible: setup.shown.android && !setup.shown.access
        text: setup.sioul.text("appnotes-step-access")
        Layout.fillWidth: true
        textFormat: Text.PlainText
        wrapMode: Text.Wrap
        color: setup.theme.muted
    }
    Label {
        visible: setup.shown.android
        text: setup.sioul.text("appnotes-step-narrow")
        Layout.fillWidth: true
        textFormat: Text.PlainText
        wrapMode: Text.Wrap
        color: setup.theme.muted
    }
    Flow {
        visible: setup.shown.android
        Layout.fillWidth: true
        spacing: 6

        Button {
            visible: !setup.shown.access && setup.shown.restricted
            width: Math.min(implicitWidth, parent ? parent.width : implicitWidth)
            text: setup.sioul.text("appnotes-open-info")
            onClicked: setup.act("open-info", {})
        }
        Button {
            width: Math.min(implicitWidth, parent ? parent.width : implicitWidth)
            text: setup.sioul.text("appnotes-open-access")
            onClicked: setup.act("open-access", {})
        }
    }
    Label {
        visible: setup.shown.android && !setup.shown.contacts
        text: setup.sioul.text("appnotes-contacts-off")
        Layout.fillWidth: true
        textFormat: Text.PlainText
        wrapMode: Text.Wrap
        color: setup.theme.muted
    }
    Button {
        visible: setup.shown.android && !setup.shown.contacts
        text: setup.sioul.text("appnotes-contacts-ask")
        onClicked: setup.act("contacts", {})
    }

    // ---------------------------------------------------------------- holding, and when
    SettingRow {
        visible: setup.shown.android && setup.shown.hold !== null
        Layout.fillWidth: true
        Layout.topMargin: 12
        setting: setup.shown.hold || { key: "hold", kind: "bool", label: "", help: "", value: true, choices: [] }
        sioul: setup.sioul
        theme: setup.theme
        onSave: (key, value) => setup.act("set", { key: key, value: value })
    }
    Label {
        visible: setup.shown.android
        text: setup.shown.times
        Layout.fillWidth: true
        textFormat: Text.PlainText
        wrapMode: Text.Wrap
        color: setup.theme.text
    }
    Flow {
        visible: setup.shown.android
        Layout.fillWidth: true
        spacing: 6

        Button {
            width: Math.min(implicitWidth, parent ? parent.width : implicitWidth)
            text: setup.sioul.text("appnotes-times-change")
            onClicked: setup.showSetting("reminders.gathered")
        }
    }
    Label {
        visible: setup.shown.android
        text: setup.sioul.text("appnotes-sound")
        Layout.fillWidth: true
        textFormat: Text.PlainText
        wrapMode: Text.Wrap
        color: setup.theme.muted
    }
    Label {
        visible: setup.shown.android && setup.shown.rang.length > 0
        text: setup.sioul.text("appnotes-rang")
        Layout.fillWidth: true
        textFormat: Text.PlainText
        wrapMode: Text.Wrap
        color: setup.theme.text
    }
    Repeater {
        model: setup.shown.android ? setup.shown.rang : []

        delegate: RowLayout {
            id: rang

            required property var modelData

            Layout.fillWidth: true
            spacing: 6

            Label {
                Layout.fillWidth: true
                text: rang.modelData.line
                textFormat: Text.PlainText
                wrapMode: Text.Wrap
                color: setup.theme.text
            }
            Button {
                text: setup.sioul.text("appnotes-make-silent")
                onClicked: setup.act("open-channel", { package: rang.modelData.package, channel: rang.modelData.channel })
            }
        }
    }

    // ---------------------------------------------------------------- apps, conversations, sites
    Label {
        visible: setup.shown.android
        text: setup.sioul.text("appnotes-apps")
        Layout.fillWidth: true
        Layout.topMargin: 18
        font.pixelSize: 17
        font.weight: Font.DemiBold
        wrapMode: Text.Wrap
        color: setup.theme.accent
    }
    Label {
        visible: setup.shown.android
        text: setup.sioul.text(setup.shown.apps.length === 0 ? "appnotes-apps-none" : "appnotes-apps-help")
        Layout.fillWidth: true
        textFormat: Text.PlainText
        wrapMode: Text.Wrap
        color: setup.theme.muted
    }
    Repeater {
        model: setup.shown.android ? setup.shown.apps : []

        delegate: ColumnLayout {
            id: app

            required property var modelData

            Layout.fillWidth: true
            Layout.bottomMargin: 6
            spacing: 2

            SettingRow {
                Layout.fillWidth: true
                setting: app.modelData.kind
                sioul: setup.sioul
                theme: setup.theme
                onSave: (key, value) => setup.act("set", { key: key, value: value })
            }
            SettingRow {
                Layout.fillWidth: true
                setting: app.modelData.area
                sioul: setup.sioul
                theme: setup.theme
                onSave: (key, value) => setup.act("set", { key: key, value: value })
            }
            Button {
                text: setup.sioul.text("appnotes-open-app")
                onClicked: setup.act("open-app", { package: app.modelData.package })
            }
        }
    }
    Label {
        visible: setup.shown.android && setup.shown.conversations.length > 0
        text: setup.sioul.text("appnotes-conversations")
        Layout.fillWidth: true
        Layout.topMargin: 18
        font.pixelSize: 17
        font.weight: Font.DemiBold
        wrapMode: Text.Wrap
        color: setup.theme.accent
    }
    Label {
        visible: setup.shown.android && setup.shown.conversations.length > 0
        text: setup.sioul.text("appnotes-conversations-help")
        Layout.fillWidth: true
        textFormat: Text.PlainText
        wrapMode: Text.Wrap
        color: setup.theme.muted
    }
    Repeater {
        model: setup.shown.android ? setup.shown.conversations : []

        delegate: SettingRow {
            required property var modelData

            Layout.fillWidth: true
            setting: modelData
            sioul: setup.sioul
            theme: setup.theme
            onSave: (key, value) => setup.act("set", { key: key, value: value })
        }
    }
    Label {
        visible: setup.shown.android && setup.shown.sites.length > 0
        text: setup.sioul.text("appnotes-sites")
        Layout.fillWidth: true
        Layout.topMargin: 18
        font.pixelSize: 17
        font.weight: Font.DemiBold
        wrapMode: Text.Wrap
        color: setup.theme.accent
    }
    Repeater {
        model: setup.shown.android ? setup.shown.sites : []

        delegate: SettingRow {
            required property var modelData

            Layout.fillWidth: true
            setting: modelData
            sioul: setup.sioul
            theme: setup.theme
            onSave: (key, value) => setup.act("set", { key: key, value: value })
        }
    }

    // ---------------------------------------------------------------- what always comes, what is kept
    Label {
        visible: setup.shown.android
        Layout.topMargin: 12
        text: setup.sioul.text("appnotes-always")
        Layout.fillWidth: true
        textFormat: Text.PlainText
        wrapMode: Text.Wrap
        color: setup.theme.muted
    }
    Label {
        visible: setup.shown.android
        Layout.bottomMargin: 24
        text: setup.sioul.text("appnotes-privacy")
        Layout.fillWidth: true
        textFormat: Text.PlainText
        wrapMode: Text.Wrap
        color: setup.theme.muted
    }
}
