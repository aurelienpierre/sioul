// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

// A security key, asked for by a site (WebAuthn): which of its accounts, its
// PIN (entered, set, or changed when the key asks), touching it again, and
// in plain words why it failed, with Retry. After Qt's own example
// (Quick Nano Browser's WebAuthDialog). A plain touch shows nothing here: the
// key blinks and the page waits for it (Qt 6.11). On Windows, Windows' own
// dialog takes all of it.

pragma ComponentBehavior: Bound

import QtQuick
import QtQuick.Controls.Basic
import QtQuick.Layouts
import QtWebEngine

Dialog {
    id: dialog

    required property var sioul
    required property var theme
    property var request: null
    readonly property int step: dialog.request ? dialog.request.state : WebEngineWebAuthUxRequest.WebAuthUxState.NotStarted
    readonly property var pin: dialog.request ? dialog.request.pinRequest : null
    readonly property var failures: ["timeout", "not-registered", "already-registered", "soft-block", "hard-block", "removed", "no-resident", "no-uv", "no-blob", "no-algorithm", "full", "denied", "cancelled"]
    readonly property var pinErrors: ["", "uv-locked", "wrong", "too-short", "invalid", "same"]
    property string account: ""

    function show(request) {
        dialog.request = request
        dialog.account = request.userNames.length > 0 ? request.userNames[0] : ""
        pinField.text = ""
        confirmField.text = ""
        dialog.open()
    }

    function go() {
        if (!dialog.request)
            return
        if (dialog.step === WebEngineWebAuthUxRequest.WebAuthUxState.SelectAccount) {
            dialog.request.setSelectedAccount(dialog.account)
        } else if (dialog.step === WebEngineWebAuthUxRequest.WebAuthUxState.CollectPin) {
            const setting = dialog.pin.reason !== WebEngineWebAuthUxRequest.PinEntryReason.Challenge
            if (setting && pinField.text !== confirmField.text)
                return
            dialog.request.setPin(pinField.text)
            pinField.text = ""
            confirmField.text = ""
        } else if (dialog.step === WebEngineWebAuthUxRequest.WebAuthUxState.RequestFailed) {
            dialog.request.retry()
        }
    }

    parent: Overlay.overlay
    anchors.centerIn: parent
    modal: true
    closePolicy: Popup.NoAutoClose
    width: Math.min(440, (parent ? parent.width : 440) - 2 * dialog.theme.gap)
    title: dialog.request ? dialog.sioul.textWith("webauth-title", "site", dialog.request.relyingPartyId) : ""

    Connections {
        target: dialog.request

        function onStateChanged(state) {
            if (state === WebEngineWebAuthUxRequest.WebAuthUxState.Completed || state === WebEngineWebAuthUxRequest.WebAuthUxState.Cancelled) {
                dialog.close()
                dialog.request = null
            }
        }
    }

    contentItem: ColumnLayout {
        spacing: 8

        // Which account of the key.
        Label {
            visible: dialog.step === WebEngineWebAuthUxRequest.WebAuthUxState.SelectAccount
            Layout.fillWidth: true
            text: dialog.sioul.text("webauth-account")
            wrapMode: Text.Wrap
            color: dialog.theme.text
        }
        Repeater {
            model: dialog.step === WebEngineWebAuthUxRequest.WebAuthUxState.SelectAccount && dialog.request ? dialog.request.userNames : []

            delegate: RadioButton {
                required property string modelData

                // An account's name is what a site stored on the key: plain text.
                text: dialog.theme.plain(modelData)
                checked: dialog.account === modelData
                onClicked: dialog.account = modelData
            }
        }

        // The key's PIN: entered, or set or changed when the key asks.
        Label {
            visible: dialog.step === WebEngineWebAuthUxRequest.WebAuthUxState.CollectPin
            Layout.fillWidth: true
            text: !dialog.pin ? "" : dialog.pin.reason === WebEngineWebAuthUxRequest.PinEntryReason.Challenge ? dialog.sioul.textWith("webauth-pin", "left", String(dialog.pin.remainingAttempts)) : dialog.sioul.textWith(dialog.pin.reason === WebEngineWebAuthUxRequest.PinEntryReason.Set ? "webauth-pin-set" : "webauth-pin-change", "length", String(dialog.pin.minPinLength))
            wrapMode: Text.Wrap
            color: dialog.theme.text
        }
        PasswordField {
            id: pinField

            visible: dialog.step === WebEngineWebAuthUxRequest.WebAuthUxState.CollectPin
            Layout.fillWidth: true
            sioul: dialog.sioul
            placeholderText: dialog.sioul.text("webauth-pin-field")
            onAccepted: dialog.go()
        }
        PasswordField {
            id: confirmField

            visible: dialog.step === WebEngineWebAuthUxRequest.WebAuthUxState.CollectPin && dialog.pin !== null && dialog.pin.reason !== WebEngineWebAuthUxRequest.PinEntryReason.Challenge
            Layout.fillWidth: true
            sioul: dialog.sioul
            placeholderText: dialog.sioul.text("webauth-pin-again")
            onAccepted: dialog.go()
        }
        Label {
            visible: dialog.step === WebEngineWebAuthUxRequest.WebAuthUxState.CollectPin && dialog.pin !== null && dialog.pin.error !== WebEngineWebAuthUxRequest.PinEntryError.NoError
            Layout.fillWidth: true
            text: dialog.pin && dialog.pin.error > 0 ? dialog.sioul.text("webauth-pin-" + dialog.pinErrors[dialog.pin.error]) : ""
            wrapMode: Text.Wrap
            color: dialog.theme.warm
        }

        // Touch it again.
        Label {
            visible: dialog.step === WebEngineWebAuthUxRequest.WebAuthUxState.FinishTokenCollection
            Layout.fillWidth: true
            text: dialog.sioul.text("webauth-touch")
            wrapMode: Text.Wrap
            font.pixelSize: 16
            color: dialog.theme.text
        }

        // Why it failed, in plain words.
        Label {
            visible: dialog.step === WebEngineWebAuthUxRequest.WebAuthUxState.RequestFailed
            Layout.fillWidth: true
            text: dialog.request && dialog.step === WebEngineWebAuthUxRequest.WebAuthUxState.RequestFailed ? dialog.sioul.text("webauth-failed-" + dialog.failures[dialog.request.requestFailureReason]) : ""
            wrapMode: Text.Wrap
            color: dialog.theme.text
        }
    }

    footer: DialogButtonBox {
        Button {
            visible: dialog.step === WebEngineWebAuthUxRequest.WebAuthUxState.SelectAccount || dialog.step === WebEngineWebAuthUxRequest.WebAuthUxState.CollectPin || dialog.step === WebEngineWebAuthUxRequest.WebAuthUxState.RequestFailed
            text: dialog.step === WebEngineWebAuthUxRequest.WebAuthUxState.RequestFailed ? dialog.sioul.text("webauth-retry") : dialog.sioul.text("webauth-go")
            highlighted: true
            onClicked: dialog.go()
        }
        Button {
            text: dialog.sioul.text("ui-cancel")
            onClicked: {
                if (dialog.request)
                    dialog.request.cancel()
                dialog.close()
            }
        }
    }
}
