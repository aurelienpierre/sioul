// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

// An account's password, given on this device: an account come from another
// device arrives without one, since passwords never travel. Typed, or taken
// from your Bitwarden vault (VaultFill.qml), found by the name the account
// logs in with (its address, unless it names another) as the user name;
// tested with the account's server before this device's keyring keeps it.

pragma ComponentBehavior: Bound

import QtQuick
import QtQuick.Controls.Basic
import QtQuick.Layouts

SafeDialog {
    id: dialog

    required property var sioul
    required property var theme
    // The account, as the Accounts page has it: id, address, host, login.
    property var account: null
    property string problem: ""
    property string note: ""
    // Bitwarden: "missing" (no account set), "locked" or "unlocked".
    property string vault: "missing"
    // What asks a security key (VaultUnlock.qml's `keys`): the window's, none on a phone.
    property var keys: null
    // The page's keeper of fields holding a vault's password (VaultHeld.qml): the
    // field emptied if the vault locks while the dialog is open.
    property var held: null
    // A server that takes app passwords only (Google's, Yahoo's): its guide,
    // its field's name and the page where one is made, as Sioul knows them
    // (backend's app_password_for); empty for any other server.
    readonly property var appWords: dialog.account !== null ? JSON.parse(dialog.sioul.appPasswordFor(dialog.account.host || "")) : ({})
    readonly property bool appPassword: !!dialog.appWords.url

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

    // The name the account logs in with: its own, else its address.
    function login() {
        return dialog.account === null ? "" : dialog.account.login || dialog.account.address || ""
    }

    // A login chosen in the vault, its password in the field: tested and kept at once.
    function take() {
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
    width: Math.min(460, (parent ? dialog.roomWidth : 460) - 2 * dialog.theme.gap)
    title: dialog.theme.plain(dialog.account !== null ? (dialog.appPassword ? dialog.sioul.textWith("account-app-password-title", "account", dialog.account.address || dialog.account.id) : dialog.sioul.textWith("account-password-title", "account", dialog.account.address || dialog.account.id)) : "")
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
            text: dialog.appPassword ? dialog.appWords.guide : dialog.sioul.text("account-password-help")
            textFormat: Text.PlainText
            wrapMode: Text.Wrap
            color: dialog.theme.text
        }
        // The page where the provider makes app passwords, in the system's browser.
        Button {
            visible: dialog.appPassword
            text: dialog.sioul.text("ui-app-passwords")
            icon.name: "internet-web-browser"
            icon.color: dialog.theme.text
            onClicked: Qt.openUrlExternally(dialog.appWords.url)
        }
        PasswordField {
            id: password

            sioul: dialog.sioul
            Layout.fillWidth: true
            placeholderText: dialog.theme.plain(dialog.appPassword ? dialog.appWords.field : dialog.sioul.text("ui-password"))
            onAccepted: dialog.keep()
        }
        VaultFill {
            sioul: dialog.sioul
            theme: dialog.theme
            keys: dialog.keys
            vault: dialog.vault
            site: dialog.site()
            user: dialog.login()
            field: password
            held: dialog.held
            // The account's own mail server: the login chosen is proposed first there next time.
            remember: true
            onTaken: dialog.take()
            onProblem: text => dialog.problem = text
            onOpened: dialog.vault = dialog.sioul.bitwardenState()
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

}
