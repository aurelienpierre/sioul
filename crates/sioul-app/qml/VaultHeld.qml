// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

// The password fields that hold a password taken from the Bitwarden vault
// ("From Bitwarden…", VaultFill.qml), one place for a page and its forms:
// each field is told here when it is filled, and emptied here when the
// vault locks (Lock, or its time without use: the backend's
// `bitwardenLocked`; Sioul closing takes the window's memory with it), when
// the page or the form that shows it is left, and when the account it was
// for is added (`release`). While it holds the vault's password, the field
// shows it only while the vault is open, the eye and "Show passwords as you
// type" included (PasswordField.qml's `veiled`). A password typed by hand
// is never the vault's: such a field is left as it is. A field emptied by
// hand is no longer held.

import QtQuick

Item {
    id: held

    // Sioul's backend (backend.rs): the vault's state, and its word when it locks.
    required property var sioul
    // The fields holding a vault's password now.
    property var fields: []
    // The vault is open on this device, as last asked or said.
    property bool open: false

    // A field just filled from the vault, open to have given it.
    function took(field) {
        held.open = held.sioul.bitwardenState() === "unlocked"
        if (held.fields.indexOf(field) < 0) {
            held.fields = held.fields.concat([field])
            field.veiled = Qt.binding(() => !held.open)
            // Emptied by hand: a password typed next is the person's own.
            field.textChanged.connect(function forget() {
                if (field.text === "") {
                    held.forget(field)
                    field.textChanged.disconnect(forget)
                }
            })
        }
    }

    function forget(field) {
        const at = held.fields.indexOf(field)
        if (at < 0)
            return
        field.veiled = false
        held.fields = held.fields.filter(f => f !== field)
    }

    // The vault's state as the page reads it: not open, its passwords go.
    function follow(vault) {
        held.open = vault === "unlocked"
        if (!held.open)
            held.release()
    }

    // Every field holding a vault's password emptied, and no longer held.
    function release() {
        const fields = held.fields
        held.fields = []
        for (const field of fields) {
            field.veiled = false
            field.text = ""
        }
    }

    Connections {
        target: held.sioul

        function onBitwardenLocked() {
            held.open = false
            held.release()
        }
    }
}
