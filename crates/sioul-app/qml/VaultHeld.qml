// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

// The password fields that a password taken from the Bitwarden vault ever
// filled ("From Bitwarden…", VaultFill.qml), one place for a page and its
// forms. A field told here once stays here until it is destroyed, whatever
// the hand does to it after. At each release (the vault locks, by Lock or
// its time without use: the backend's `bitwardenLocked`; the vault read as
// not open; the page or the form left; the account added) every such field
// is purged: emptied when it still holds what the vault gave, else left
// with the person's own text; either way set by code twice, which clears
// the field's undo and redo (QQuickTextInput::setText, Qt 6.9 to 6.11:
// any change of the text by code empties its history; a text set to what
// it holds already is no change, so the first set always differs). The
// vault's value is kept here only from the fill to the release, as the vault
// itself keeps it while open. Until the release, the field shows it only
// while the vault is open, the eye and "Show passwords as you type"
// included (PasswordField.qml's `veiled`). A field the vault never filled
// is the person's alone: nothing here touches it. Sioul closing takes the
// window's memory with it.

import QtQuick

Item {
    id: held

    // Sioul's backend (backend.rs): the vault's state, and its word when it locks.
    required property var sioul
    // Every field the vault filled, with what it gave it while not yet released:
    // [{field, value}].
    property var fields: []
    // The vault is open on this device, as last asked or said.
    property bool open: false

    // A field let go (its password purged): its "From Bitwarden…" no longer names a login.
    signal purged(var field)

    // A field just filled from the vault, open to have given it.
    function took(field, value) {
        held.open = held.sioul.bitwardenState() === "unlocked"
        const entry = held.fields.find(e => e.field === field)
        if (entry)
            entry.value = value
        else
            held.fields = held.fields.concat([{ field: field, value: value }])
        field.veiled = Qt.binding(() => !held.open)
    }

    // The vault's state as the page reads it: not open, its passwords go.
    function follow(vault) {
        held.open = vault === "unlocked"
        if (!held.open)
            held.release()
    }

    // One field purged: its text set by code twice, which leaves nothing in its
    // undo and redo; what the vault gave dropped from here, the field kept.
    function purge(entry) {
        const field = entry.field
        // Still holding the vault's password, even edited around: emptied; else the person's own text.
        const keep = entry.value !== "" && field.text.indexOf(entry.value) >= 0 ? "" : field.text
        field.text = field.text === "x" ? "y" : "x"
        field.text = keep
        field.veiled = false
        entry.value = ""
        held.purged(field)
    }

    // A field let go alone (a dialog closed), if the vault ever filled it.
    function letGo(field) {
        const entry = held.fields.find(e => e.field === field)
        if (entry)
            held.purge(entry)
    }

    // Every field the vault ever filled purged; those destroyed meanwhile forgotten.
    function release() {
        held.fields = held.fields.filter(e => e.field && typeof e.field.text === "string")
        for (const entry of held.fields)
            held.purge(entry)
    }

    Connections {
        target: held.sioul

        function onBitwardenLocked() {
            held.open = false
            held.release()
        }
    }
}
