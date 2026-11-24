// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

// Your password manager, at the top of Accounts ▸ Your accounts
// (docs/sites.md, "Logins from Bitwarden"): Bitwarden's account, its
// server, whether the vault is open on this device, with Unlock and Lock,
// and the lock after a time without use. One vault for the whole app: the
// Sites page fills logins from it, the account forms take passwords from it
// (VaultFill.qml). Its settings are the core's (`settings::for_view
// ("bitwarden")`), kept in the configuration and shared with the other
// settings; the vault itself never leaves this device's memory. Without an
// account set, the card itself asks for it: one line, its e-mail and its
// server (bitwarden.com when empty, bitwarden.eu, or your own, Vaultwarden
// too), and Add, which opens the vault at once. Its one place: Add an
// account lists no Bitwarden, the Sites page's ⚙ no line about it (one
// canonical place per setting, 24 November 2026). The vault opens as from
// the Sites page: by the security key where the window can ask one (`keys`,
// VaultUnlock.qml), else by the master password and a code, as on a phone.

pragma ComponentBehavior: Bound

import QtQuick
import QtQuick.Controls.Basic
import QtQuick.Layouts

Section {
    id: card

    // Sioul's backend (backend.rs): its words in your language and what it does.
    required property var sioul
    // What asks a security key (VaultUnlock.qml's `keys`): the window's on a computer, none on a phone.
    property var keys: null
    // "missing" (no account set), "locked" or "unlocked", as the backend says (`bitwardenState`).
    property string vault: "missing"
    // Bitwarden's settings, as the core gives them, once an account is set.
    property var rows: []
    // What went wrong saving one of them, in words.
    property string problem: ""
    // The vault's dialog, for the window's pictures and tests.
    property alias unlockDialog: unlockDialog

    // The vault opened, locked, or the account set or taken out: the forms' "From Bitwarden…" follow.
    signal changed

    // Read again: the vault's state and its settings.
    function reload() {
        card.vault = card.sioul.bitwardenState()
        card.rows = card.vault === "missing" ? [] : JSON.parse(card.sioul.settings("bitwarden") || "[]")
    }

    // The vault opened from here (a link by key, the account just added).
    function unlock() {
        card.reload()
        if (card.vault === "locked")
            unlockDialog.begin()
    }

    function save(key, value, shown) {
        card.problem = card.sioul.changeSetting(key, JSON.stringify(shown), JSON.stringify(value))
        card.reload()
        card.changed()
    }

    // The account added here: Bitwarden's settings written, then the vault opened.
    function add() {
        card.problem = card.sioul.setSetting("bitwarden.server", JSON.stringify(server.text.trim()))
        if (card.problem === "")
            card.problem = card.sioul.setSetting("bitwarden.email", JSON.stringify(address.text.trim()))
        if (card.problem !== "")
            return
        address.text = ""
        server.text = ""
        card.reload()
        card.changed()
        card.unlock()
    }

    Component.onCompleted: card.reload()

    // Locked, by Lock or after its time without use: said at once, the page told.
    Connections {
        target: card.sioul

        function onBitwardenLocked() {
            card.reload()
            card.changed()
        }
    }
    // Read again each time it shows: the vault locks by itself after its time without use.
    onVisibleChanged: if (card.visible) card.reload()

    objectName: "vaultCard"
    title: card.sioul.text("vault-card-title")
    iconName: "dialog-password"
    body.spacing: 8

    // No account set: one line, then its e-mail and its server, and Add.
    Label {
        objectName: "vaultCardNone"
        visible: card.vault === "missing"
        Layout.fillWidth: true
        text: card.sioul.text("vault-card-none")
        wrapMode: Text.Wrap
        color: card.theme.text
    }
    GridLayout {
        visible: card.vault === "missing"
        Layout.fillWidth: true
        columns: card.theme.compact ? 1 : 2
        columnSpacing: card.theme.gap
        rowSpacing: 6

        Label {
            text: card.sioul.text("ui-address")
            color: card.theme.muted
        }
        TextField {
            id: address

            objectName: "vaultCardEmail"
            Layout.fillWidth: true
            inputMethodHints: Qt.ImhEmailCharactersOnly | Qt.ImhNoAutoUppercase
            Accessible.name: card.sioul.text("ui-address")
            onAccepted: if (addButton.enabled) card.add()
        }
        Label {
            text: card.sioul.text("set-bitwarden-server")
            color: card.theme.muted
        }
        TextField {
            id: server

            objectName: "vaultCardServer"
            Layout.fillWidth: true
            placeholderText: "bitwarden.com"
            inputMethodHints: Qt.ImhUrlCharactersOnly | Qt.ImhNoAutoUppercase
            Accessible.name: card.sioul.text("set-bitwarden-server")
            onAccepted: if (addButton.enabled) card.add()
        }
    }
    Label {
        visible: card.vault === "missing"
        Layout.fillWidth: true
        text: card.sioul.text("set-bitwarden-server-help")
        wrapMode: Text.Wrap
        font.pixelSize: 13
        color: card.theme.muted
    }
    Button {
        id: addButton

        objectName: "vaultCardAdd"
        visible: card.vault === "missing"
        enabled: address.text.indexOf("@") > 0
        implicitWidth: implicitContentWidth + leftPadding + rightPadding
        icon.name: "list-add"
        icon.color: card.theme.text
        text: card.sioul.text("vault-card-add")
        onClicked: card.add()
    }

    // Its account, its server, open or locked with Unlock and Lock, the lock after a time.
    Repeater {
        model: card.rows

        delegate: SettingRow {
            required property var modelData

            Layout.fillWidth: true
            Layout.topMargin: 4
            setting: modelData
            sioul: card.sioul
            theme: card.theme
            // The row follows the card's own reading, made again at each change (`reload`).
            vaultState: card.vault
            onSave: (key, value, shown) => card.save(key, value, shown)
            onUnlockAsked: card.unlock()
            onChangedElsewhere: {
                card.reload()
                card.changed()
            }
        }
    }
    Label {
        visible: card.problem !== ""
        Layout.fillWidth: true
        text: card.problem
        textFormat: Text.PlainText
        wrapMode: Text.Wrap
        color: card.theme.warm
    }

    // Bitwarden, opened here for the session, as from the Sites page.
    VaultUnlock {
        id: unlockDialog

        sioul: card.sioul
        theme: card.theme
        keys: card.keys
        onUnlocked: {
            card.reload()
            card.changed()
        }
    }
}
