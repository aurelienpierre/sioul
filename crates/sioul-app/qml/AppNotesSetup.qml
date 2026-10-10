// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

// Other apps' notifications, on a phone (docs/android.md, "Notifications
// from other apps"), in two parts (`part`). This phone ("phone"): what Sioul
// does with other apps' notifications and what it never does; Android's
// notification access and its two steps for an app installed from a file;
// holding on or off; the gathered times; what rang before Sioul held it,
// each with Android's page to make it silent. What reaches you ▸ Exceptions
// ("lists"): each app and conversation by time, a row of the matrix of what
// reaches you (AttentionGrid.qml: as usual, at once, gathered or held at each
// time), then each app, conversation and site seen, with its choice. On a
// computer, one sentence: this is the phone's; in Exceptions, the rows by
// time set on the phone, by name, changed here as there.

pragma ComponentBehavior: Bound

import QtQuick
import QtQuick.Controls.Basic
import QtQuick.Layouts

ColumnLayout {
    id: setup

    // Sioul's backend (backend.rs): its words in your language and what it does.
    required property var sioul
    // The window's colours, sizes and fonts (Theme.qml).
    required property var theme
    // appnotes.rs's `setup`: {android, access, restricted, contacts, hold, times, heard, rang, apps, conversations, sites, grid, more}.
    property var shown: ({ android: false, access: false, restricted: false, contacts: false, hold: null, times: "", heard: "", rang: [], apps: [], conversations: [], sites: [], grid: { columns: [], marks: [], rows: [] }, more: false })

    // "phone" (this phone's setup), "lists" (each app, conversation, site), or "all".
    property string part: "all"
    // This phone's part shown: on a phone, with `part` "phone" or "all".
    readonly property bool phonePart: setup.shown.android && (setup.part === "all" || setup.part === "phone")
    // The lists' part shown: on a phone, with `part` "lists" or "all".
    readonly property bool listsPart: setup.shown.android && (setup.part === "all" || setup.part === "lists")
    // A computer, in Exceptions: the rows by time set on the phone, when there are some.
    readonly property bool computerPart: !setup.shown.android && setup.part === "lists" && setup.shown.grid !== undefined && setup.shown.grid.rows.length > 0
    // The rows by time shown: the lists' part, or a computer's rows set on the phone.
    readonly property bool byTime: setup.listsPart || setup.computerPart

    // What is shown read again from the backend (`appNotesSetup`).
    function reload() {
        setup.shown = JSON.parse(setup.sioul.appNotesSetup() || "null") || setup.shown
    }

    // A change asked of the backend (`appNotesChange`): `verb` and its arguments; its
    // answer shown.
    function act(verb, args) {
        const answer = JSON.parse(setup.sioul.appNotesChange(verb, JSON.stringify(args || {})) || "null")
        if (answer)
            setup.shown = answer
    }

    spacing: 6
    Component.onCompleted: setup.reload()
    onVisibleChanged: if (visible) setup.reload()

    Label {
        visible: setup.part !== "lists"
        text: setup.sioul.text("appnotes-title")
        Layout.fillWidth: true
        Layout.topMargin: 18
        font.pixelSize: 17
        font.weight: Font.DemiBold
        wrapMode: Text.Wrap
        color: setup.theme.accent
    }
    Label {
        visible: !setup.shown.android && setup.part !== "lists"
        text: setup.sioul.text("appnotes-computer")
        Layout.fillWidth: true
        textFormat: Text.PlainText
        wrapMode: Text.Wrap
        color: setup.theme.text
    }
    Label {
        visible: setup.phonePart
        text: setup.sioul.text("appnotes-does")
        Layout.fillWidth: true
        textFormat: Text.PlainText
        wrapMode: Text.Wrap
        color: setup.theme.text
    }
    Label {
        visible: setup.phonePart
        text: setup.sioul.text("appnotes-never")
        Layout.fillWidth: true
        textFormat: Text.PlainText
        wrapMode: Text.Wrap
        color: setup.theme.muted
    }

    // ---------------------------------------------------------------- the access
    Label {
        visible: setup.phonePart
        Layout.topMargin: 12
        text: setup.sioul.text(setup.shown.access ? "appnotes-access-on" : "appnotes-access-off")
        font.weight: Font.DemiBold
        Layout.fillWidth: true
        textFormat: Text.PlainText
        wrapMode: Text.Wrap
        color: setup.theme.text
    }
    Label {
        visible: setup.phonePart && setup.shown.access && setup.shown.heard !== ""
        text: setup.shown.heard
        Layout.fillWidth: true
        textFormat: Text.PlainText
        wrapMode: Text.Wrap
        color: setup.theme.muted
    }
    Label {
        visible: setup.phonePart && !setup.shown.access
        text: setup.sioul.text("appnotes-steps")
        Layout.fillWidth: true
        textFormat: Text.PlainText
        wrapMode: Text.Wrap
        color: setup.theme.muted
    }
    Label {
        visible: setup.phonePart && !setup.shown.access && setup.shown.restricted
        text: setup.sioul.text("appnotes-step-restricted")
        Layout.fillWidth: true
        textFormat: Text.PlainText
        wrapMode: Text.Wrap
        color: setup.theme.muted
    }
    Label {
        visible: setup.phonePart && !setup.shown.access
        text: setup.sioul.text("appnotes-step-access")
        Layout.fillWidth: true
        textFormat: Text.PlainText
        wrapMode: Text.Wrap
        color: setup.theme.muted
    }
    Label {
        visible: setup.phonePart
        text: setup.sioul.text("appnotes-step-narrow")
        Layout.fillWidth: true
        textFormat: Text.PlainText
        wrapMode: Text.Wrap
        color: setup.theme.muted
    }
    Flow {
        visible: setup.phonePart
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
        visible: setup.phonePart && !setup.shown.contacts
        text: setup.sioul.text("appnotes-contacts-off")
        Layout.fillWidth: true
        textFormat: Text.PlainText
        wrapMode: Text.Wrap
        color: setup.theme.muted
    }
    Button {
        visible: setup.phonePart && !setup.shown.contacts
        text: setup.sioul.text("appnotes-contacts-ask")
        onClicked: setup.act("contacts", {})
    }

    // ---------------------------------------------------------------- holding, and when
    SettingRow {
        visible: setup.phonePart && setup.shown.hold !== null
        Layout.fillWidth: true
        Layout.topMargin: 12
        setting: setup.shown.hold || { key: "hold", kind: "bool", label: "", help: "", value: true, choices: [] }
        sioul: setup.sioul
        theme: setup.theme
        onSave: (key, value) => setup.act("set", { key: key, value: value })
    }
    Label {
        visible: setup.phonePart
        text: setup.shown.times
        Layout.fillWidth: true
        textFormat: Text.PlainText
        wrapMode: Text.Wrap
        color: setup.theme.text
    }
    Flow {
        visible: setup.phonePart
        Layout.fillWidth: true
        spacing: 6

        Button {
            width: Math.min(implicitWidth, parent ? parent.width : implicitWidth)
            text: setup.sioul.text("appnotes-times-change")
            onClicked: setup.sioul.reminderOpened("settings:reminders.gathered", "", "")
        }
    }
    Label {
        visible: setup.phonePart
        text: setup.sioul.text("appnotes-sound")
        Layout.fillWidth: true
        textFormat: Text.PlainText
        wrapMode: Text.Wrap
        color: setup.theme.muted
    }
    Label {
        visible: setup.phonePart && setup.shown.rang.length > 0
        text: setup.sioul.text("appnotes-rang")
        Layout.fillWidth: true
        textFormat: Text.PlainText
        wrapMode: Text.Wrap
        color: setup.theme.text
    }
    Repeater {
        model: setup.phonePart ? setup.shown.rang : []

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

    // ---------------------------------------------------------------- by time
    // Each app and conversation a row of the matrix (attention.rs, its
    // sources' rows): a press on a mark opens its choices in words, saved at
    // once, the row whole ("attention.app.<package>").
    Label {
        visible: setup.byTime
        text: setup.sioul.text("appnotes-by-time")
        Layout.fillWidth: true
        Layout.topMargin: 18
        font.pixelSize: 17
        font.weight: Font.DemiBold
        wrapMode: Text.Wrap
        color: setup.theme.accent
    }
    Label {
        visible: setup.byTime
        text: setup.sioul.text(setup.computerPart ? "appnotes-by-time-computer" : "appnotes-by-time-help")
        Layout.fillWidth: true
        textFormat: Text.PlainText
        wrapMode: Text.Wrap
        color: setup.theme.muted
    }
    AttentionGrid {
        visible: setup.byTime && setup.shown.grid.rows.length > 0
        Layout.fillWidth: true
        // As wide as the page, never wider: on a phone the times' names stand upright.
        Layout.preferredWidth: 0
        setting: ({ grid: setup.shown.grid })
        sioul: setup.sioul
        theme: setup.theme
        onSave: (key, value, shown) => setup.act("row", { key: key, words: value, shown: shown })
    }
    Label {
        visible: setup.listsPart && setup.shown.more === true
        text: setup.sioul.text("appnotes-by-time-more")
        Layout.fillWidth: true
        textFormat: Text.PlainText
        wrapMode: Text.Wrap
        font.pixelSize: 13
        color: setup.theme.muted
    }
    Label {
        visible: setup.listsPart
        text: setup.sioul.text("appnotes-by-time-limits")
        Layout.fillWidth: true
        textFormat: Text.PlainText
        wrapMode: Text.Wrap
        font.pixelSize: 13
        color: setup.theme.muted
    }

    // ---------------------------------------------------------------- apps, conversations, sites
    Label {
        visible: setup.listsPart
        text: setup.sioul.text("appnotes-apps")
        Layout.fillWidth: true
        Layout.topMargin: 18
        font.pixelSize: 17
        font.weight: Font.DemiBold
        wrapMode: Text.Wrap
        color: setup.theme.accent
    }
    Label {
        visible: setup.listsPart
        text: setup.sioul.text(setup.shown.apps.length === 0 ? "appnotes-apps-none" : "appnotes-apps-help")
        Layout.fillWidth: true
        textFormat: Text.PlainText
        wrapMode: Text.Wrap
        color: setup.theme.muted
    }
    Repeater {
        model: setup.listsPart ? setup.shown.apps : []

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
            // Its row by time, in one sentence.
            Label {
                Layout.fillWidth: true
                text: app.modelData.times || ""
                textFormat: Text.PlainText
                wrapMode: Text.Wrap
                font.pixelSize: 13
                color: setup.theme.muted
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
        visible: setup.listsPart && setup.shown.conversations.length > 0
        text: setup.sioul.text("appnotes-conversations")
        Layout.fillWidth: true
        Layout.topMargin: 18
        font.pixelSize: 17
        font.weight: Font.DemiBold
        wrapMode: Text.Wrap
        color: setup.theme.accent
    }
    Label {
        visible: setup.listsPart && setup.shown.conversations.length > 0
        text: setup.sioul.text("appnotes-conversations-help")
        Layout.fillWidth: true
        textFormat: Text.PlainText
        wrapMode: Text.Wrap
        color: setup.theme.muted
    }
    // Each conversation, its choice; Always through, a press to mark it Priority in Android,
    // without which the phone's own do-not-disturb (Sioul's modes) silences it.
    Repeater {
        model: setup.listsPart ? setup.shown.conversations : []

        delegate: ColumnLayout {
            id: talk

            required property var modelData
            readonly property bool always: talk.modelData.value === "always"

            Layout.fillWidth: true
            spacing: 2

            SettingRow {
                Layout.fillWidth: true
                setting: talk.modelData
                sioul: setup.sioul
                theme: setup.theme
                onSave: (key, value) => setup.act("set", { key: key, value: value })
            }
            Label {
                visible: talk.modelData.important === true
                Layout.fillWidth: true
                text: setup.sioul.text(talk.always ? "appnotes-priority-on" : "appnotes-priority-warn")
                textFormat: Text.PlainText
                wrapMode: Text.Wrap
                font.pixelSize: 13
                color: talk.always ? setup.theme.muted : setup.theme.warm
            }
            Button {
                visible: talk.always && talk.modelData.important !== true
                flat: true
                text: setup.sioul.text("appnotes-priority-open")
                onClicked: setup.act("open-conversation", { key: talk.modelData.talk })
            }
        }
    }
    Label {
        visible: setup.listsPart && setup.shown.sites.length > 0
        text: setup.sioul.text("appnotes-sites")
        Layout.fillWidth: true
        Layout.topMargin: 18
        font.pixelSize: 17
        font.weight: Font.DemiBold
        wrapMode: Text.Wrap
        color: setup.theme.accent
    }
    Repeater {
        model: setup.listsPart ? setup.shown.sites : []

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
        visible: setup.phonePart
        Layout.topMargin: 12
        text: setup.sioul.text("appnotes-always")
        Layout.fillWidth: true
        textFormat: Text.PlainText
        wrapMode: Text.Wrap
        color: setup.theme.muted
    }
    Label {
        visible: setup.phonePart
        Layout.bottomMargin: 24
        text: setup.sioul.text("appnotes-privacy")
        Layout.fillWidth: true
        textFormat: Text.PlainText
        wrapMode: Text.Wrap
        color: setup.theme.muted
    }
}
