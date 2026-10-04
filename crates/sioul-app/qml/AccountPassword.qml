// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

// An account's password, given on this device: an account come from another
// device arrives without one, since passwords never travel. Typed, or taken
// from your Bitwarden vault, searched by the account's address; tested with
// the account's server before this device's keyring keeps it.

pragma ComponentBehavior: Bound

import QtQuick
import QtQuick.Controls.Basic
import QtQuick.Layouts

Dialog {
    id: dialog

    required property var sioul
    required property var theme
    // The account, as the Accounts page has it: id, address, host.
    property var account: null
    property string problem: ""
    property string note: ""
    // Bitwarden: "missing" (no account set), "locked" or "unlocked".
    property string vault: "missing"

    function ask(account) {
        dialog.account = account
        dialog.problem = ""
        dialog.note = ""
        dialog.vault = dialog.sioul.bitwardenState()
        password.text = ""
        dialog.open()
        password.forceActiveFocus()
    }

    // The account's server, as a site: the vault's logins for it come first.
    function site() {
        return dialog.account !== null && dialog.account.host ? "https://" + dialog.account.host : ""
    }

    // The vault opened first when it is locked; then its logins, searched by the account's address.
    function fromVault() {
        dialog.problem = ""
        dialog.vault = dialog.sioul.bitwardenState()
        if (dialog.vault === "missing")
            dialog.problem = dialog.sioul.text("account-password-vault-missing")
        else if (dialog.vault === "locked")
            unlock.begin()
        else
            chooser.begin(dialog.site(), dialog.account.address || "")
    }

    // A login chosen in the vault: its password tested and kept at once.
    function take(item) {
        const login = JSON.parse(dialog.sioul.bitwardenLogin(dialog.site(), item))
        if (login.error) {
            dialog.problem = login.error
            return
        }
        password.text = login.password
        dialog.note = dialog.sioul.textWith("account-password-from", "name", login.name)
        dialog.keep()
    }

    function keep() {
        if (password.text === "" || dialog.sioul.formBusy)
            return
        dialog.problem = ""
        dialog.sioul.setAccountPassword(dialog.account.id, password.text)
    }

    parent: Overlay.overlay
    anchors.centerIn: parent
    modal: true
    width: Math.min(460, (parent ? parent.width : 460) - 2 * dialog.theme.gap)
    title: dialog.account !== null ? dialog.sioul.textWith("account-password-title", "account", dialog.account.address || dialog.account.id) : ""
    onClosed: password.text = ""

    Connections {
        target: dialog.sioul

        function onAccountPasswordDone(id, problem) {
            if (dialog.account === null || id !== dialog.account.id)
                return
            if (problem === "")
                dialog.close()
            else
                dialog.problem = problem
        }
    }

    contentItem: ColumnLayout {
        spacing: 8

        Label {
            Layout.fillWidth: true
            text: dialog.sioul.text("account-password-help")
            wrapMode: Text.Wrap
            color: dialog.theme.text
        }
        PasswordField {
            id: password

            sioul: dialog.sioul
            Layout.fillWidth: true
            placeholderText: dialog.sioul.text("ui-password")
            onAccepted: dialog.keep()
        }
        Button {
            visible: dialog.vault !== "missing"
            enabled: !dialog.sioul.formBusy
            flat: true
            icon.name: "dialog-password"
            text: dialog.sioul.text("account-password-vault")
            onClicked: dialog.fromVault()
        }
        Label {
            visible: dialog.sioul.formBusy || dialog.note !== ""
            Layout.fillWidth: true
            text: dialog.sioul.formBusy ? dialog.sioul.text("account-password-testing") : dialog.note
            textFormat: Text.PlainText
            wrapMode: Text.Wrap
            color: dialog.theme.muted
        }
        Label {
            visible: dialog.problem !== ""
            Layout.fillWidth: true
            text: dialog.problem
            textFormat: Text.PlainText
            wrapMode: Text.Wrap
            color: dialog.theme.warm
        }
    }

    footer: DialogButtonBox {
        Button {
            text: dialog.sioul.text("account-password-keep")
            highlighted: true
            enabled: password.text !== "" && !dialog.sioul.formBusy
            onClicked: dialog.keep()
        }
        Button {
            text: dialog.sioul.text("ui-cancel")
            DialogButtonBox.buttonRole: DialogButtonBox.RejectRole
        }
        onRejected: dialog.close()
    }

    // Bitwarden, opened here for the session: by its master password and a
    // second step (no security key outside the Sites page's).
    VaultUnlock {
        id: unlock

        sioul: dialog.sioul
        theme: dialog.theme
        onUnlocked: {
            dialog.vault = "unlocked"
            chooser.begin(dialog.site(), dialog.account.address || "")
        }
    }

    LoginChooser {
        id: chooser

        sioul: dialog.sioul
        theme: dialog.theme
        takeText: dialog.sioul.text("account-password-use")
        onChosen: item => dialog.take(item)
    }
}
