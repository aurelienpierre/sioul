// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

// Google's mail (Gmail, Google Workspace) in the mail form: Google refuses
// the account's password there, so this form never asks for it. Two ways:
// an app password made at Google (the default without a key of your own),
// or "Sign in with Google" on Google's page, in the system's browser, with a
// key of your own (Google gives its mail's scope to no key of Sioul's yet).
// Made the first time an address turns out to be Google's (AccountsPage.qml).
// docs/google.md, "Mail".

pragma ComponentBehavior: Bound

import QtQuick
import QtQuick.Controls.Basic
import QtQuick.Layouts

ColumnLayout {
    id: form

    required property var sioul
    required property var theme
    // The address, and what was found for it (backend's FoundView).
    property string address: ""
    property var found: null
    // An account signed in again: its id; empty when adding one.
    property string againId: ""
    property bool stepsShown: false
    property bool ownKey: false
    readonly property bool keyKept: form.found !== null && form.found.google_key === true
    readonly property bool builtIn: form.sioul.googleBuiltIn()
    // "app" or "google": the way ticked (its default set by `reset`).
    readonly property string chosen: signInWay.checked ? "google" : "app"
    readonly property bool phone: Qt.platform.os === "android"

    // An app password given to an account signed in again: what its server said.
    property string problem: ""

    // Something was asked of the backend: the page shows its answer under this form.
    signal started
    // An account signed in again took an app password instead.
    signal done

    // A new address: its fields emptied, and the way ticked by default: Google's
    // page when a key of yours is kept here (or the account signs in again),
    // else an app password.
    function reset() {
        form.stepsShown = false
        form.ownKey = false
        form.problem = ""
        appPassword.text = ""
        clientId.text = ""
        clientSecret.text = ""
        if (form.keyKept || form.againId !== "")
            signInWay.checked = true
        else
            appWay.checked = true
    }

    Component.onCompleted: form.reset()

    // An app password for an account signed in again goes through the
    // account's password (backend's give_password), which answers here.
    Connections {
        target: form.sioul

        function onAccountPasswordDone(id, problem) {
            if (form.againId === "" || id !== form.againId)
                return
            form.problem = problem
            if (problem === "")
                form.done()
        }
    }

    spacing: 10

    // The two ways; on a phone too, both labels are short.
    Flow {
        Layout.fillWidth: true
        spacing: form.theme.gap

        RadioButton {
            id: appWay

            text: form.sioul.text("ui-gmail-choice-app-password")
        }
        RadioButton {
            id: signInWay

            text: form.sioul.text("ui-gmail-choice-sign-in")
        }
    }

    // An app password, made at Google: never the account's own password.
    ColumnLayout {
        visible: form.chosen === "app"
        Layout.fillWidth: true
        spacing: 8

        Label {
            Layout.fillWidth: true
            text: form.sioul.text("ui-gmail-app-password-guide")
            wrapMode: Text.Wrap
            color: form.theme.text
        }
        Button {
            text: form.sioul.text("ui-app-passwords")
            icon.name: "internet-web-browser"
            icon.color: form.theme.text
            onClicked: Qt.openUrlExternally(form.found && form.found.help_url ? form.found.help_url : "https://myaccount.google.com/apppasswords")
        }
        Label {
            text: form.sioul.text("ui-gmail-app-password-field")
            color: form.theme.muted
        }
        PasswordField {
            id: appPassword

            Layout.fillWidth: true
            sioul: form.sioul
            placeholderText: "xxxx xxxx xxxx xxxx"
            onAccepted: if (keep.enabled) keep.clicked()
        }
        Label {
            Layout.fillWidth: true
            text: form.sioul.text("ui-password-note")
            wrapMode: Text.Wrap
            color: form.theme.muted
        }
        Button {
            id: keep

            text: form.sioul.formBusy ? form.sioul.text("ui-connecting") : form.sioul.text("ui-connect")
            enabled: !form.sioul.formBusy && appPassword.text.trim().length > 0 && form.found !== null
            onClicked: {
                form.started()
                form.problem = ""
                if (form.againId !== "")
                    form.sioul.setAccountPassword(form.againId, appPassword.text)
                else
                    form.sioul.addAccount(form.address, form.found.host, form.found.port, form.found.security, form.found.login, appPassword.text)
            }
        }
        Label {
            visible: form.problem !== ""
            Layout.fillWidth: true
            text: form.problem
            textFormat: Text.PlainText
            wrapMode: Text.Wrap
            color: form.theme.forged
        }
    }

    // Google's own page, in the browser, with a key of your own.
    ColumnLayout {
        visible: form.chosen === "google"
        Layout.fillWidth: true
        spacing: 8

        Label {
            Layout.fillWidth: true
            text: form.keyKept ? form.sioul.text("ui-gmail-sign-in-key-kept") : form.builtIn && !form.ownKey ? form.sioul.text("google-mail-built-in") : form.sioul.text("ui-gmail-sign-in-own-key")
            wrapMode: Text.Wrap
            color: form.theme.text
        }
        // Sioul's own key in this build cannot read mail: yours, if you make one.
        WrapCheckBox {
            visible: !form.keyKept && form.builtIn
            Layout.fillWidth: true
            text: form.sioul.text("ui-google-own-key")
            checked: form.ownKey
            onToggled: form.ownKey = checked
        }
        // Its sentence wraps: on one line, it was wider than a phone.
        Button {
            id: stepsButton

            visible: !form.keyKept && (!form.builtIn || form.ownKey)
            Layout.fillWidth: true
            flat: true
            text: (form.stepsShown ? "▾  " : "▸  ") + form.sioul.text("ui-google-steps")
            contentItem: Label {
                text: stepsButton.text
                wrapMode: Text.Wrap
                color: form.theme.text
            }
            onClicked: form.stepsShown = !form.stepsShown
        }
        Label {
            visible: form.stepsShown && !form.keyKept
            Layout.fillWidth: true
            text: form.sioul.text("ui-gmail-steps-text")
            textFormat: Text.MarkdownText
            wrapMode: Text.Wrap
            color: form.theme.text
            onLinkActivated: link => Qt.openUrlExternally(link)
        }
        GridLayout {
            visible: !form.keyKept && (!form.builtIn || form.ownKey)
            Layout.fillWidth: true
            columns: 2
            columnSpacing: form.theme.gap
            rowSpacing: 8

            Label {
                text: form.sioul.text("ui-google-client-id")
                color: form.theme.muted
            }
            TextField {
                id: clientId

                Layout.fillWidth: true
                placeholderText: "….apps.googleusercontent.com"
                inputMethodHints: Qt.ImhNoPredictiveText | Qt.ImhNoAutoUppercase
            }
            Label {
                text: form.sioul.text("ui-google-client-secret")
                color: form.theme.muted
            }
            PasswordField {
                id: clientSecret

                Layout.fillWidth: true
                sioul: form.sioul
            }
        }
        RowLayout {
            spacing: 8

            Button {
                text: form.sioul.text("ui-google-sign-in")
                enabled: !form.sioul.formBusy && form.found !== null && (form.keyKept || (clientId.text.trim().length > 0 && clientSecret.text.trim().length > 0))
                onClicked: {
                    form.started()
                    form.sioul.addGoogleMail(form.address, form.keyKept ? "" : clientId.text, form.keyKept ? "" : clientSecret.text, form.againId)
                }
            }
            Button {
                visible: form.sioul.formBusy
                flat: true
                text: form.sioul.text("ui-cancel")
                onClicked: form.sioul.cancelGoogle()
            }
        }
        Label {
            visible: form.sioul.formBusy
            Layout.fillWidth: true
            text: form.phone ? form.sioul.text("ui-google-waiting-phone") : form.sioul.text("ui-google-waiting")
            wrapMode: Text.Wrap
            color: form.theme.text
        }
    }
}
