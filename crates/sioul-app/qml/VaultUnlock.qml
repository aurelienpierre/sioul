// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

// Bitwarden, opened once for the session: by the master password (dropped
// once used), with a second step or a new device's code when asked, or by
// your security key alone. A security key is asked from Bitwarden's own page,
// held unseen in this dialog, where one can be (the Sites page gives it, with
// Qt WebEngine): its PIN comes in Sioul's own dialog, above this one. On
// Android, the steps without a key: an app's code, an e-mail's, a YubiKey's,
// a recovery code.

pragma ComponentBehavior: Bound

import QtQuick
import QtQuick.Controls.Basic
import QtQuick.Layouts

Dialog {
    id: unlock

    required property var sioul
    required property var theme
    // What asks a security key, where one can be asked: Bitwarden's page held
    // unseen (the Sites page gives one, with Qt WebEngine); none on Android.
    property Component keyComponent: null
    // The key's page can be made now (the Sites page's web profile is ready).
    property bool keyReady: true
    readonly property bool keyCapable: unlock.keyComponent !== null
    property alias keyView: keyView

    // The vault is open: what asked for it goes on.
    signal unlocked

    // -1: the password only; else the step asked (0 app, 1 e-mail, 3 YubiKey OTP,
    // 7 security key, 8 recovery, 100 new device).
    property int provider: -1
    property var offered: []
    // The security key's step, when the account has one: {page, script}.
    property var key: null
    // The key being asked: {mode: "factor" or "passkey", page, script}; else null.
    property var keyAsk: null
    // The e-mail code was sent for this try.
    property bool sent: false
    // The security key alone opened the vault last time: proposed first.
    property bool passkeyFirst: false
    property string problem: ""
    property string note: ""
    // The steps Sioul can take, in the order Bitwarden's apps propose them:
    // a security key, a YubiKey's code, an app, e-mail; a recovery code only when chosen.
    readonly property var order: [7, 3, 0, 1, 8]

    function begin() {
        unlock.provider = -1
        unlock.offered = []
        unlock.key = null
        unlock.keyAsk = null
        unlock.sent = false
        unlock.problem = ""
        unlock.note = ""
        secret.text = ""
        code.text = ""
        unlock.passkeyFirst = unlock.keyCapable && unlock.sioul.viewFlag("bitwarden-passkey")
        unlock.open()
        if (unlock.passkeyFirst)
            passkeyButton.forceActiveFocus()
        else
            secret.forceActiveFocus()
    }

    // The security key alone: no master password.
    function usePasskey() {
        unlock.problem = ""
        unlock.note = ""
        const begun = JSON.parse(unlock.sioul.bitwardenPasskeyBegin())
        if (begun.error)
            unlock.problem = begun.error
        else
            unlock.askKey("passkey", begun)
    }

    // The key asked: Bitwarden's page loaded unseen below, the key asked there
    // at once. A new page each time: each ask is a fresh one.
    function askKey(mode, ask) {
        unlock.problem = ""
        unlock.note = ""
        unlock.keyAsk = null
        Qt.callLater(() => unlock.keyAsk = { mode: mode, page: ask.page, script: ask.script })
    }

    // What the key answered: the vault opened with it, or why not.
    function keyAnswered(mode, result) {
        unlock.keyAsk = null
        const said = JSON.parse(result)
        if (said.error !== undefined)
            unlock.problem = said.error === "not-allowed" ? unlock.sioul.text("bitwarden-key-not-allowed") : unlock.sioul.textWith("bitwarden-key-failed", "error", said.error)
        else if (mode === "passkey")
            unlock.answer(JSON.parse(unlock.sioul.bitwardenPasskey(result)))
        else
            unlock.answer(JSON.parse(unlock.sioul.bitwardenUnlock(secret.text, 7, said.token)))
    }

    // A step chosen: e-mail sends its code now, once; the security key is asked at once.
    function choose(provider) {
        unlock.provider = provider
        unlock.problem = ""
        unlock.note = ""
        unlock.keyAsk = null
        if (provider === 1 && !unlock.sent)
            unlock.sendCode()
        if (provider === 7)
            unlock.askKey("factor", unlock.key)
        else
            code.forceActiveFocus()
    }

    function sendCode() {
        const problem = unlock.sioul.bitwardenSendCode(secret.text)
        unlock.sent = problem === ""
        unlock.note = problem === "" ? unlock.sioul.text("bitwarden-code-sent") : ""
        unlock.problem = problem
    }

    function tryIt() {
        unlock.keyAsk = null
        unlock.answer(JSON.parse(unlock.sioul.bitwardenUnlock(secret.text, unlock.provider, code.text)))
    }

    // What Bitwarden answered: open, a second step to take, a new device's code, or why not.
    function answer(answer) {
        code.text = ""
        if (answer.ok) {
            secret.text = ""
            unlock.close()
            unlock.unlocked()
        } else if (answer.factor) {
            unlock.key = answer.key || null
            // A security key only where one can be asked (not on Android).
            const can = p => p === 7 ? unlock.key !== null && unlock.keyCapable : [0, 1, 3, 8].indexOf(p) >= 0
            unlock.offered = unlock.order.filter(p => answer.factor.indexOf(p) >= 0 && can(p))
            if (unlock.offered.length === 0) {
                unlock.provider = -1
                unlock.problem = unlock.sioul.text("bitwarden-factor-unsupported")
            } else if (unlock.offered.indexOf(unlock.provider) < 0) {
                unlock.choose(unlock.offered[0])
            } else {
                unlock.problem = unlock.sioul.text("bitwarden-code-refused")
            }
        } else if (answer.new_device) {
            unlock.provider = 100
            unlock.problem = ""
            code.forceActiveFocus()
        } else {
            unlock.problem = answer.error || ""
        }
    }

    parent: Overlay.overlay
    anchors.centerIn: parent
    modal: true
    width: Math.min(460, (parent ? parent.width : 460) - 2 * unlock.theme.gap)
    title: unlock.sioul.text("bitwarden-unlock")
    onClosed: {
        secret.text = ""
        unlock.keyAsk = null
    }

    contentItem: ColumnLayout {
        spacing: 8

        Label {
            Layout.fillWidth: true
            text: unlock.sioul.text(unlock.keyCapable ? "bitwarden-unlock-help" : "bitwarden-unlock-help-password")
            wrapMode: Text.Wrap
            color: unlock.theme.text
        }
        // The security key alone, when Bitwarden knows it as a passkey: no master password.
        Button {
            id: passkeyButton

            visible: unlock.keyCapable && unlock.provider === -1 && unlock.keyAsk === null
            highlighted: unlock.passkeyFirst
            text: unlock.sioul.text("bitwarden-passkey")
            icon.name: "security-high"
            onClicked: unlock.usePasskey()
        }
        Label {
            visible: unlock.keyCapable && unlock.provider === -1 && unlock.keyAsk === null
            Layout.fillWidth: true
            text: unlock.sioul.text("bitwarden-passkey-help")
            wrapMode: Text.Wrap
            font.pixelSize: 13
            color: unlock.theme.muted
        }
        // The key asked: what to do with it, and a way to stop.
        RowLayout {
            visible: unlock.keyAsk !== null
            Layout.fillWidth: true
            spacing: 8

            Icon {
                iconName: "security-high"
            }
            Label {
                Layout.fillWidth: true
                text: unlock.sioul.text(unlock.keyAsk !== null && unlock.keyAsk.mode === "passkey" ? "bitwarden-key-waiting-passkey" : "bitwarden-key-waiting")
                wrapMode: Text.Wrap
                color: unlock.theme.text
            }
            Button {
                flat: true
                text: unlock.sioul.text("bitwarden-key-stop")
                onClicked: unlock.keyAsk = null
            }
        }
        // Bitwarden's page that asks the key, held unseen (one point): the key
        // answers there, its PIN in Sioul's own dialog, above this one. The page
        // must be the vault's: Bitwarden takes keys' signatures for its address only.
        Loader {
            id: keyView

            active: unlock.keyAsk !== null && unlock.keyCapable && unlock.keyReady
            visible: active
            Layout.preferredWidth: 1
            Layout.preferredHeight: 1
            sourceComponent: unlock.keyComponent
            // A page asks a key only while it has the focus.
            onLoaded: keyView.item.forceActiveFocus()
        }
        // The key's answer, taken from the page once given; the first call asks the key.
        Timer {
            interval: 300
            repeat: true
            running: unlock.keyAsk !== null && keyView.item !== null
            onTriggered: {
                const mode = unlock.keyAsk.mode
                keyView.item.runJavaScript("window.sioulKey ? window.sioulKey() : ''", result => {
                    if (result && unlock.keyAsk !== null && unlock.keyAsk.mode === mode)
                        unlock.keyAnswered(mode, result)
                })
            }
        }
        Label {
            visible: unlock.provider === -1
            Layout.fillWidth: true
            Layout.topMargin: 6
            text: unlock.sioul.text("bitwarden-password-or")
            wrapMode: Text.Wrap
            color: unlock.theme.text
        }
        PasswordField {
            id: secret

            sioul: unlock.sioul
            Layout.fillWidth: true
            placeholderText: unlock.sioul.text("bitwarden-password")
            onAccepted: unlock.tryIt()
        }
        // The second step, or a new device's code.
        ComboBox {
            visible: unlock.offered.length > 1
            Layout.fillWidth: true
            model: unlock.offered.map(p => unlock.sioul.text("bitwarden-factor-" + p))
            currentIndex: Math.max(0, unlock.offered.indexOf(unlock.provider))
            onActivated: index => unlock.choose(unlock.offered[index])
        }
        Label {
            visible: unlock.provider >= 0
            Layout.fillWidth: true
            text: unlock.sioul.text(unlock.provider === 100 ? "bitwarden-new-device" : "bitwarden-code-" + unlock.provider)
            wrapMode: Text.Wrap
            color: unlock.theme.muted
        }
        // No key among the steps the account offers: why a YubiKey may be missing.
        Label {
            visible: unlock.offered.length > 0 && unlock.offered.indexOf(7) < 0 && unlock.offered.indexOf(3) < 0 && unlock.provider !== 100
            Layout.fillWidth: true
            text: unlock.sioul.text("bitwarden-no-key")
            wrapMode: Text.Wrap
            font.pixelSize: 13
            color: unlock.theme.muted
        }
        // The security key, asked again (it is asked at once when its step comes).
        Button {
            visible: unlock.provider === 7 && unlock.keyAsk === null
            highlighted: true
            text: unlock.sioul.text("bitwarden-key-use")
            icon.name: "security-high"
            onClicked: unlock.askKey("factor", unlock.key)
        }
        TextField {
            id: code

            visible: unlock.provider >= 0 && unlock.provider !== 7
            Layout.fillWidth: true
            placeholderText: unlock.sioul.text("bitwarden-code")
            inputMethodHints: Qt.ImhNoPredictiveText
            onAccepted: unlock.tryIt()
        }
        Button {
            visible: unlock.provider === 1
            flat: true
            text: unlock.sioul.text("bitwarden-code-again")
            onClicked: unlock.sendCode()
        }
        Label {
            visible: unlock.note !== ""
            Layout.fillWidth: true
            text: unlock.note
            textFormat: Text.PlainText
            wrapMode: Text.Wrap
            color: unlock.theme.muted
        }
        Label {
            visible: unlock.problem !== ""
            Layout.fillWidth: true
            text: unlock.problem
            textFormat: Text.PlainText
            wrapMode: Text.Wrap
            color: unlock.theme.warm
        }
    }

    footer: DialogButtonBox {
        Button {
            visible: unlock.provider !== 7
            text: unlock.sioul.text("bitwarden-open")
            highlighted: !unlock.passkeyFirst || unlock.provider >= 0
            onClicked: unlock.tryIt()
        }
        Button {
            text: unlock.sioul.text("ui-cancel")
            DialogButtonBox.buttonRole: DialogButtonBox.RejectRole
        }
        onRejected: unlock.close()
    }
}
