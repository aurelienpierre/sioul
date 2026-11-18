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
// account set, one line and a button to add one (Add an account ▸
// Bitwarden, VaultForm.qml). The vault opens as from the Sites page: by the
// security key where the window can ask one (`keys`, VaultUnlock.qml), else
// by the master password and a code, as on a phone.

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

    // The account to add: Add an account ▸ Bitwarden.
    signal addAsked
    // The vault opened, locked, or the account set or taken out: the forms' "From Bitwarden…" follow.
    signal changed

    // Read again: the vault's state and its settings.
    function reload() {
        card.vault = card.sioul.bitwardenState()
        card.rows = card.vault === "missing" ? [] : JSON.parse(card.sioul.settings("bitwarden") || "[]")
    }

    // The vault opened from here (a link by key, Add an account ▸ Bitwarden once added).
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

    // No account set: one line, and a button to add one.
    Label {
        objectName: "vaultCardNone"
        visible: card.vault === "missing"
        Layout.fillWidth: true
        text: card.sioul.text("vault-card-none")
        wrapMode: Text.Wrap
        color: card.theme.text
    }
    Button {
        objectName: "vaultCardAdd"
        visible: card.vault === "missing"
        implicitWidth: implicitContentWidth + leftPadding + rightPadding
        icon.name: "list-add"
        icon.color: card.theme.text
        text: card.sioul.text("vault-card-add")
        onClicked: card.addAsked()
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
