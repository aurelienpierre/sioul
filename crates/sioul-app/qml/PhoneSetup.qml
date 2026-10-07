// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

// Settings ▸ This phone (« Ce téléphone »), on a phone: what sets this phone
// up, nothing that decides when (that is What reaches you). Call screening
// (Sioul as Android's caller ID & spam app, the contacts, where a declined
// call goes, texts; CallsSetup.qml); other apps' notifications (the access
// in two steps, holding, the apps that rang before Sioul held them;
// AppNotesSetup.qml); Do Not Disturb's access and its modes, who of your
// Always through people is starred here, keeping in step in the background,
// the battery (DndSetup.qml); exact alarms and Sioul's own notifications.

pragma ComponentBehavior: Bound

import QtQuick
import QtQuick.Controls.Basic
import QtQuick.Layouts

ColumnLayout {
    id: phone

    required property var sioul
    required property var theme
    // Whether Android lets Sioul set exact alarms (reaches.rs, `view`).
    property bool exact: true

    spacing: 6
    Component.onCompleted: phone.exact = (JSON.parse(phone.sioul.reachesView() || "{}").exact !== false)

    component Heading: Label {
        Layout.fillWidth: true
        Layout.topMargin: 18
        font.pixelSize: 17
        font.weight: Font.DemiBold
        wrapMode: Text.Wrap
        color: phone.theme.accent
    }

    Label {
        Layout.fillWidth: true
        text: phone.sioul.text("attention-phone-intro")
        textFormat: Text.PlainText
        wrapMode: Text.Wrap
        color: phone.theme.muted
    }
    Button {
        flat: true
        text: phone.sioul.text("attention-phone-open-tab")
        icon.name: "go-next"
        icon.color: phone.theme.text
        onClicked: phone.sioul.reminderOpened("settings:attention", "", "")
    }

    CallsSetup {
        Layout.fillWidth: true
        sioul: phone.sioul
        theme: phone.theme
    }
    AppNotesSetup {
        Layout.fillWidth: true
        sioul: phone.sioul
        theme: phone.theme
        part: "phone"
    }
    DndSetup {
        Layout.fillWidth: true
        sioul: phone.sioul
        theme: phone.theme
        part: "phone"
    }

    // ---------------------------------------------------------------- alarms and notifications
    Heading {
        text: phone.sioul.text("attention-phone-alarms-title")
    }
    Label {
        Layout.fillWidth: true
        text: phone.sioul.text("attention-phone-alarms")
        textFormat: Text.PlainText
        wrapMode: Text.Wrap
        color: phone.theme.text
    }
    Label {
        visible: !phone.exact
        Layout.fillWidth: true
        text: phone.sioul.text("set-reminders-before-inexact")
        textFormat: Text.PlainText
        wrapMode: Text.Wrap
        color: phone.theme.warm
    }
    Flow {
        Layout.fillWidth: true
        Layout.bottomMargin: 24
        spacing: 6

        Button {
            width: Math.min(implicitWidth, parent ? parent.width : implicitWidth)
            text: phone.sioul.text("attention-phone-open-exact")
            onClicked: phone.sioul.wakeSettings("exact")
        }
        Button {
            width: Math.min(implicitWidth, parent ? parent.width : implicitWidth)
            text: phone.sioul.text("attention-phone-open-notifications")
            onClicked: phone.sioul.wakeSettings("notifications")
        }
    }
}
