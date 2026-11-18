// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

// Bitwarden, as a kind of account in Accounts ▸ Add an account: the
// account's e-mail and its server (bitwarden.com when empty, bitwarden.eu,
// or your own, Vaultwarden too), kept in the configuration as Bitwarden's
// settings (`bitwarden.email`, `bitwarden.server`), shared with the other
// settings. Nothing is asked of Bitwarden here: the vault opens from Your
// accounts' Password manager card, which shows once it is added. One
// account for the whole app: once one is set, the form says which, and it
// is changed on the card.

pragma ComponentBehavior: Bound

import QtQuick
import QtQuick.Controls.Basic
import QtQuick.Layouts

Panel {
    id: form

    // Sioul's backend (backend.rs): its words in your language and what it does.
    required property var sioul
    // "missing" (no account set), "locked" or "unlocked": the page says it.
    property string vault: "missing"
    // The account set, by its e-mail, read again when the vault's state changes.
    readonly property string email: form.vault === "missing" ? "" : String((JSON.parse(form.sioul.settings("bitwarden") || "[]").find(r => r.key === "bitwarden.email") || { value: "" }).value)
    // What went wrong, in words.
    property string problem: ""

    // Added: the page shows it on Your accounts' card, and opens the vault there.
    signal added
    // Already set: the card, on Your accounts.
    signal cardAsked

    function add() {
        form.problem = form.sioul.setSetting("bitwarden.server", JSON.stringify(server.text.trim()))
        if (form.problem === "")
            form.problem = form.sioul.setSetting("bitwarden.email", JSON.stringify(address.text.trim()))
        if (form.problem !== "")
            return
        address.text = ""
        server.text = ""
        form.added()
    }

    objectName: "vaultForm"

    ColumnLayout {
        anchors.fill: parent
        spacing: 10

        Label {
            text: form.sioul.text("vault-form-title")
            font.pixelSize: 18
            color: form.theme.text
        }
        Label {
            Layout.fillWidth: true
            text: form.sioul.text("vault-form-note")
            wrapMode: Text.Wrap
            color: form.theme.muted
        }

        // One account for the whole app: the one set, and where it is changed.
        Label {
            visible: form.vault !== "missing"
            Layout.fillWidth: true
            text: form.sioul.textWith("vault-form-set", "email", form.email)
            textFormat: Text.PlainText
            wrapMode: Text.Wrap
            color: form.theme.text
        }
        Button {
            visible: form.vault !== "missing"
            implicitWidth: implicitContentWidth + leftPadding + rightPadding
            flat: true
            icon.name: "go-next"
            icon.color: form.theme.text
            text: form.sioul.text("vault-form-card")
            onClicked: form.cardAsked()
        }

        GridLayout {
            visible: form.vault === "missing"
            Layout.fillWidth: true
            columns: 2
            columnSpacing: form.theme.gap
            rowSpacing: 8

            Label {
                text: form.sioul.text("ui-address")
                color: form.theme.muted
            }
            TextField {
                id: address

                objectName: "vaultFormEmail"
                Layout.fillWidth: true
                inputMethodHints: Qt.ImhEmailCharactersOnly | Qt.ImhNoAutoUppercase
                onAccepted: if (addButton.enabled) form.add()
            }
            Label {
                text: form.sioul.text("set-bitwarden-server")
                color: form.theme.muted
            }
            TextField {
                id: server

                objectName: "vaultFormServer"
                Layout.fillWidth: true
                placeholderText: "bitwarden.com"
                inputMethodHints: Qt.ImhUrlCharactersOnly | Qt.ImhNoAutoUppercase
                onAccepted: if (addButton.enabled) form.add()
            }
        }
        Label {
            visible: form.vault === "missing"
            Layout.fillWidth: true
            text: form.sioul.text("set-bitwarden-server-help")
            wrapMode: Text.Wrap
            font.pixelSize: 13
            color: form.theme.muted
        }
        Button {
            id: addButton

            objectName: "vaultFormAdd"
            visible: form.vault === "missing"
            enabled: address.text.indexOf("@") > 0
            text: form.sioul.text("vault-form-add")
            onClicked: form.add()
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
}
