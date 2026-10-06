// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

// Settings ▸ Calls, on a phone (docs/android.md, "Calls"): what Sioul does
// with calls, what it never does, what always rings; then this phone: Sioul
// as Android's "Caller ID & spam app" (Android 10 and later, asked in
// Android's own dialog), reading the contacts, where a declined call goes
// (the operator's voicemail, read with the codes the phone app dials), and
// what Sioul leaves to Android (texts, its own blocked numbers).

pragma ComponentBehavior: Bound

import QtQuick
import QtQuick.Controls.Basic
import QtQuick.Layouts

ColumnLayout {
    id: setup

    required property var sioul
    required property var theme

    // calls.rs's `setup`: {android, state: {api, available, held, contacts, table, emergency_at}, moment, people, dnd_people}.
    property var shown: ({ android: false, state: null, moment: {}, people: 0, dnd_people: true })
    readonly property var phone: setup.shown.state || ({})
    readonly property bool held: setup.phone.held === true
    readonly property bool available: setup.phone.available === true
    readonly property bool tooOld: (setup.phone.api || 0) > 0 && setup.phone.api < 29

    function reload() {
        setup.shown = JSON.parse(setup.sioul.callsSetup() || "null") || setup.shown
    }

    function act(verb, args) {
        const answer = JSON.parse(setup.sioul.callsSetupChange(verb, JSON.stringify(args || {})) || "null")
        if (answer)
            setup.shown = answer
    }

    spacing: 6
    Component.onCompleted: setup.reload()
    onVisibleChanged: if (visible) setup.reload()

    // Back from Android's question or its settings: what it allows, read again.
    Connections {
        target: Qt.application

        function onStateChanged() {
            if (Qt.application.state === Qt.ApplicationActive && setup.visible)
                setup.reload()
        }
    }

    component Heading: Label {
        Layout.fillWidth: true
        Layout.topMargin: 18
        font.pixelSize: 17
        font.weight: Font.DemiBold
        wrapMode: Text.Wrap
        color: setup.theme.accent
    }
    component Said: Label {
        Layout.fillWidth: true
        textFormat: Text.PlainText
        wrapMode: Text.Wrap
        color: setup.theme.text
    }
    component Note: Label {
        Layout.fillWidth: true
        textFormat: Text.PlainText
        wrapMode: Text.Wrap
        font.pixelSize: 13
        color: setup.theme.muted
    }

    // ---------------------------------------------------------------- what Sioul does
    Heading {
        text: setup.sioul.text("calls-setup-what-title")
    }
    Said {
        text: setup.sioul.text("calls-setup-what")
    }
    Said {
        text: setup.sioul.text("calls-setup-never")
    }
    Heading {
        text: setup.sioul.text("calls-setup-always-title")
    }
    Said {
        text: setup.sioul.text("calls-setup-always")
    }
    Note {
        text: setup.sioul.text("calls-setup-emergency-doubt")
    }
    Note {
        text: setup.sioul.text("calls-setup-dnd")
    }

    // ---------------------------------------------------------------- this phone
    Heading {
        text: setup.sioul.text("calls-setup-here")
    }
    Said {
        text: setup.sioul.text(setup.held ? "calls-setup-on" : setup.tooOld ? "calls-setup-too-old" : !setup.available ? "calls-setup-unavailable" : "calls-setup-off")
    }
    Note {
        visible: setup.held && setup.phone.table !== true
        text: setup.sioul.text("calls-setup-no-table")
    }
    Button {
        visible: !setup.held && setup.available
        text: setup.sioul.text("calls-setup-ask")
        icon.name: "call-start"
        icon.color: setup.theme.text
        onClicked: setup.act("ask-role", {})
    }
    Note {
        visible: !setup.held && setup.available
        text: setup.sioul.text("calls-setup-ask-help")
    }
    Note {
        visible: !setup.held && setup.available
        text: setup.sioul.text("calls-setup-replaces")
    }
    // Without reading the contacts, Android does not ask Sioul about their calls.
    Said {
        visible: setup.held && setup.phone.contacts !== true
        text: setup.sioul.text("calls-setup-contacts-off")
    }
    Button {
        visible: setup.held && setup.phone.contacts !== true
        text: setup.sioul.text("calls-setup-contacts-allow")
        onClicked: setup.act("allow-contacts", {})
    }
    Note {
        visible: setup.held
        text: setup.sioul.text("calls-setup-android-block")
    }
    Button {
        visible: setup.held || (!setup.available && !setup.tooOld)
        flat: true
        text: setup.sioul.text("calls-setup-change")
        onClicked: setup.act("open-roles", {})
    }
    Note {
        visible: setup.held
        text: setup.sioul.text("calls-setup-change-help")
    }

    // ---------------------------------------------------------------- who may call when
    Heading {
        text: setup.sioul.text("calls-setup-when-title")
    }
    Said {
        text: setup.sioul.text("calls-setup-when")
    }
    // Accounts ▸ Who may reach you ▸ Calls (AccountsPage.qml, the reach matrix).
    Button {
        flat: true
        text: setup.sioul.text("calls-setup-when-open")
        onClicked: setup.sioul.reminderOpened("reach-calls", "", "")
    }
    Said {
        text: setup.sioul.text(!setup.shown.dnd_people ? "calls-setup-people-off" : setup.shown.people > 0 ? "calls-setup-people" : "calls-setup-people-none")
    }
    Button {
        flat: true
        text: setup.sioul.text("calls-setup-people-open")
        onClicked: {
            const window = setup.Window.window
            if (window && window.showParameters)
                window.showParameters("dnd.people")
        }
    }

    // ---------------------------------------------------------------- where a declined call goes
    Heading {
        text: setup.sioul.text("calls-setup-voicemail-title")
    }
    Said {
        text: setup.sioul.text("calls-setup-voicemail")
    }
    Flow {
        Layout.fillWidth: true
        spacing: 6

        Repeater {
            model: ["*#67#", "*#61#"]

            delegate: Button {
                id: code

                required property string modelData

                width: Math.min(implicitWidth, parent ? parent.width : implicitWidth)
                text: setup.sioul.textWith("calls-setup-dial", "code", code.modelData)
                onClicked: setup.act("dial", { number: code.modelData })
            }
        }
    }
    Note {
        text: setup.sioul.text("calls-setup-greeting")
    }
    Note {
        text: setup.sioul.text("calls-setup-free")
    }

    // ---------------------------------------------------------------- texts, and Android's own list
    Heading {
        text: setup.sioul.text("calls-setup-texts-title")
    }
    Said {
        text: setup.sioul.text("calls-setup-texts")
    }
    Button {
        flat: true
        text: setup.sioul.text("calls-open-blocked")
        onClicked: setup.act("open-blocked", {})
    }
    Note {
        Layout.bottomMargin: 24
        text: setup.sioul.text("calls-setup-try")
    }
}
