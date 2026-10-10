// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

// A password, a passphrase, a key: hidden by default, shown while you ask
// (the eye at its end, reached with Tab too), for whoever needs to see what
// they type. Paramètres ▸ "Show passwords as you type" shows them from the start.

import QtQuick
import QtQuick.Controls.Basic

TextField {
    id: field

    required property var sioul
    property bool shown: field.sioul.viewFlag("passwords-shown")

    echoMode: field.shown ? TextInput.Normal : TextInput.Password
    rightPadding: eye.width + 6
    inputMethodHints: Qt.ImhNoPredictiveText | Qt.ImhSensitiveData | Qt.ImhNoAutoUppercase

    Button {
        id: eye

        anchors.right: parent.right
        anchors.rightMargin: 2
        anchors.verticalCenter: parent.verticalCenter
        width: height
        height: parent.height - 6
        flat: true
        padding: 4
        icon.name: field.shown ? "view-hidden" : "view-visible"
        display: AbstractButton.IconOnly
        text: field.sioul.text(field.shown ? "password-hide" : "password-show")
        Accessible.name: eye.text
        ToolTip.visible: eye.hovered
        ToolTip.text: field.sioul.text(field.shown ? "password-hide" : "password-show")
        ToolTip.delay: 400
        onClicked: field.shown = !field.shown
    }
}
