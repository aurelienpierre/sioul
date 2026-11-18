// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

// A password taken with "From Bitwarden…" into the forms of Accounts
// (AccountsPage.qml, VaultFill.qml, VaultHeld.qml) is let go with the
// vault: on the real page, on a stand-in for Sioul that holds a made-up
// vault, the fields filled from it are emptied when the card's Lock is
// pressed, when the vault locks after its time without use (the backend's
// `bitwardenLocked`, said by the window's minute), when Add an account is
// left, and when an account is added; never shown through the eye while
// the vault is not open; a password typed by hand is left as it is. A field
// the vault filled stays the keeper's whatever the hand does: emptied by
// hand, typed over, undone or redone, nothing of the vault's comes back by
// Undo or Redo once let go, with the eye on or off, in the forms and in the
// account's password dialog (Qt keeps an edit's history; a text set by
// code twice clears it). The button says which login filled its field.
// Nothing asks Bitwarden. Run by tools/qml-test.sh.

import QtQuick
import QtQuick.Controls.Basic
import QtTest
import "../../qml"

Item {
    id: root

    width: 900
    height: 800

    Theme {
        id: testTheme
    }

    QtObject {
        id: mock

        property string vault: "unlocked"
        property string email: "camille@example.org"
        readonly property string accounts: "[]"
        readonly property string found: ""
        readonly property string scouted: ""
        readonly property bool scouting: false
        readonly property bool formBusy: false
        readonly property string formError: ""
        property string status: ""

        signal bitwardenAnswered(int ticket, string answer)
        signal bitwardenFound(int ticket, string found)
        signal bitwardenLocked
        signal keysChanged
        signal securityKeyChanged(string context, string state)
        signal accountAdded
        signal phoneAccountChosen(string name, string kind)
        signal accountPasswordDone(string id, string problem)

        function text(key) {
            return key
        }
        function textWith(key, name, value) {
            return key + " " + value
        }
        function viewFlag(name) {
            return false
        }
        function googleBuiltIn() {
            return false
        }
        function phoneAccounts() {
            return false
        }
        function historyWeeks() {
            return 2
        }
        function appPasswordFor(host) {
            return "{}"
        }
        function bitwardenState() {
            return mock.vault
        }
        // As the backend does: the vault emptied, then said.
        function bitwardenLock() {
            mock.vault = "locked"
            mock.bitwardenLocked()
        }
        function bitwardenUnlock(password, provider, code) {
            return 1
        }
        function bitwardenSite(url) {
            return JSON.stringify({ domain: "", chosen: "", only: "" })
        }
        function bitwardenSearch(url, site, user) {
            return JSON.stringify({ found: [{ id: "one", name: "Cloud", username: "camille@example.org", site: "cloud.example.org", elsewhere: false }], more: 0 })
        }
        function bitwardenLogin(url, id) {
            return mock.vault === "unlocked" ? JSON.stringify({ name: "Cloud", username: "camille@example.org", password: "made-up-secret", code: "" }) : JSON.stringify({ error: "bitwarden-locked" })
        }
        function settings(view) {
            if (view !== "bitwarden")
                return "[]"
            const row = (key, kind, value) => ({ key: key, kind: kind, label: "set-" + key, help: "", value: value, choices: [], group: "", section: "", min: 0, max: 0, step: 1, unit: "" })
            return JSON.stringify(mock.vault === "missing" ? [] : [row("bitwarden.email", "text", mock.email), row("bitwarden.server", "text", ""), row("bitwarden_vault", "vault", false), row("bitwarden.lock_after", "choice", 0)])
        }
        function setSetting(key, value) {
            return ""
        }
        // The account's password dialog keeps a password taken (tested by its server, not here).
        function setAccountPassword(id, password) {
        }
        function changeSetting(key, shown, value) {
            return ""
        }
    }

    AccountsPage {
        id: page

        anchors.fill: parent
        sioul: mock
        theme: testTheme
    }

    TestCase {
        name: "VaultHold"
        when: windowShown

        function walk(item, test, found) {
            if (!item)
                return found
            if (test(item))
                found.push(item)
            const children = item.children || []
            for (let i = 0; i < children.length; ++i)
                walk(children[i], test, found)
            if (item.contentItem && item.contentItem !== item && children.indexOf(item.contentItem) < 0)
                walk(item.contentItem, test, found)
            return found
        }
        function named(within, name) {
            return walk(within, i => i.objectName === name, [])[0] || null
        }
        function fills() {
            return walk(page, i => i instanceof VaultFill, [])
        }
        function holding() {
            return walk(page, i => i instanceof PasswordField && i.text === "made-up-secret", [])
        }

        // The vault open, Add an account shown, each form's "From Bitwarden…" choosing the
        // chooser's login, as LoginChooser's `chosen` gives it.
        function fillAll() {
            mock.vault = "unlocked"
            page.addVault()
            tryVerify(() => named(page, "vaultForm") !== null)
            page.readVault()
            for (const fill of fills())
                fill.take("one")
            const filled = holding().length
            verify(filled >= 3, filled + " fields hold the vault's password")
            return filled
        }

        function cleanup() {
            for (const field of walk(page, i => i instanceof PasswordField, []))
                field.text = ""
            page.openSettings(true, "")
            mock.vault = "unlocked"
            page.readVault()
        }

        function test_lock_empties_every_field_filled_from_the_vault() {
            fillAll()
            verify(fills().some(f => f.visible && f.text === "account-password-from Cloud"), "the button says which login filled it")
            // The card's Lock (at the top of Your accounts), pressed while Add an account
            // stays shown, so that only the lock can empty them.
            const lock = named(named(page, "vaultCard"), "vaultLock")
            verify(lock !== null)
            compare(page.vault, "unlocked")
            lock.clicked()
            compare(mock.vault, "locked")
            compare(holding().length, 0, "every field emptied")
            verify(fills().every(f => f.text === "account-password-vault"), "no login named any more")
        }

        function test_the_vault_s_time_without_use_empties_them_too() {
            fillAll()
            // The window's minute locks it and says so (backend.rs, `bitwarden_lock_if_idle`).
            mock.vault = "locked"
            mock.bitwardenLocked()
            compare(holding().length, 0, "every field emptied")
            tryCompare(named(page, "vaultCard"), "vault", "locked")
        }

        function test_leaving_the_form_or_adding_the_account_empties_them() {
            fillAll()
            page.openSettings(true, "")
            compare(holding().length, 0, "Add an account left: emptied")
            fillAll()
            mock.accountAdded()
            compare(holding().length, 0, "the account added: emptied")
        }

        function test_a_typed_password_stays_and_a_held_one_is_never_shown_closed() {
            mock.vault = "unlocked"
            page.addVault()
            tryVerify(() => named(page, "vaultForm") !== null)
            page.readVault()
            const typed = walk(page, i => i instanceof PasswordField, [])[0]
            typed.text = "typed-by-hand"
            // Another field from the vault: shown while the vault is open, as asked.
            const fill = fills().find(f => f.field !== typed)
            fill.take("one")
            const held = fill.field
            held.shown = true
            compare(held.echoMode, TextInput.Normal, "the vault open: the eye shows it")
            // Said not open (as an instant before the word that empties it): veiled, the eye gone.
            walk(page, i => i instanceof VaultHeld, [])[0].open = false
            compare(held.echoMode, TextInput.Password, "veiled: nothing shown")
            verify(held.veiled)
            // The vault not open, as the page reads it: emptied; locked: still emptied.
            mock.vault = "locked"
            page.readVault()
            compare(held.text, "", "the vault not open: emptied")
            verify(!held.veiled, "an empty field is no longer the vault's")
            mock.bitwardenLocked()
            compare(typed.text, "typed-by-hand", "a password typed by hand is the person's")
            held.shown = false
            typed.text = ""
        }

        // One field filled from the vault, in view and focused, its eye as asked.
        function oneFilled(shown) {
            mock.vault = "unlocked"
            page.addVault()
            tryVerify(() => named(page, "vaultForm") !== null)
            page.readVault()
            const fill = fills().find(f => f.visible)
            verify(fill, "a From Bitwarden… in view")
            fill.take("one")
            const field = fill.field
            compare(field.text, "made-up-secret")
            field.shown = shown
            compare(field.echoMode, shown ? TextInput.Normal : TextInput.Password)
            field.forceActiveFocus()
            return { fill: fill, field: field }
        }
        // Undo and Redo pressed in the field: whatever they give.
        function undoRedo(field) {
            field.forceActiveFocus()
            keySequence(StandardKey.Undo)
            const undone = field.text
            keySequence(StandardKey.Redo)
            return [undone, field.text]
        }
        function eyes() {
            return [{ tag: "eye off", shown: false }, { tag: "eye on", shown: true }]
        }

        function test_emptied_by_hand_nothing_comes_back_after_the_lock_data() {
            return eyes()
        }
        function test_emptied_by_hand_nothing_comes_back_after_the_lock(data) {
            const one = oneFilled(data.shown)
            one.field.selectAll()
            keyClick(Qt.Key_Delete)
            compare(one.field.text, "", "emptied by hand")
            verify(one.field.canUndo, "Qt keeps the edit to undo")
            mock.bitwardenLock()
            verify(!one.field.canUndo && !one.field.canRedo, "nothing left to undo or redo")
            compare(undoRedo(one.field), ["", ""], "Undo and Redo bring nothing back")
            one.field.shown = false
        }

        function test_typed_over_the_person_s_text_stays_without_the_vault_s_data() {
            return eyes()
        }
        function test_typed_over_the_person_s_text_stays_without_the_vault_s(data) {
            const one = oneFilled(data.shown)
            one.field.selectAll()
            keyClick(Qt.Key_T)
            keyClick(Qt.Key_Y)
            compare(one.field.text, "ty", "typed over, never empty")
            compare(one.fill.text, "account-password-vault", "no login named once typed in")
            mock.bitwardenLock()
            compare(one.field.text, "ty", "the person's text kept")
            verify(!one.field.canUndo && !one.field.canRedo)
            compare(undoRedo(one.field), ["ty", "ty"], "nothing of the vault's by Undo or Redo")
            one.field.shown = false
        }

        // Typed over, then undone back to the vault's password while it is open,
        // the typing kept to redo: at the lock, emptied, and neither comes back.
        function test_undone_and_redone_nothing_comes_back_data() {
            return eyes()
        }
        function test_undone_and_redone_nothing_comes_back(data) {
            const one = oneFilled(data.shown)
            one.field.selectAll()
            keyClick(Qt.Key_T)
            keySequence(StandardKey.Undo)
            compare(one.field.text, "made-up-secret", "undone while the vault is open")
            verify(one.field.canRedo)
            mock.bitwardenLock()
            compare(one.field.text, "", "the vault's password emptied")
            compare(undoRedo(one.field), ["", ""])
            one.field.shown = false
        }

        // The other releases: Add an account left, an account added.
        function test_leaving_the_form_or_adding_an_account_purges_too() {
            let one = oneFilled(true)
            one.field.selectAll()
            keyClick(Qt.Key_Delete)
            page.openSettings(true, "")
            page.addVault()
            compare(undoRedo(one.field), ["", ""], "the form left")
            one = oneFilled(false)
            one.field.selectAll()
            keyClick(Qt.Key_T)
            mock.accountAdded()
            page.addVault()
            const after = undoRedo(one.field)
            verify(after.every(t => t.indexOf("made-up-secret") < 0), "the account added: " + after)
        }

        // The account's password dialog: emptied by hand, closed, opened again: nothing by Undo.
        function test_the_password_dialog_purges_as_it_closes() {
            mock.vault = "unlocked"
            page.openSettings(true, "")
            page.readVault()
            page.askPassword({ id: "a", address: "camille@example.org", host: "imap.example.org", login: "" })
            const dialog = walk(page, i => i.item instanceof AccountPassword, [])[0].item
            tryCompare(dialog, "opened", true)
            const fill = walk(dialog.contentItem, i => i instanceof VaultFill, [])[0]
            const field = fill.field
            fill.take("one")
            compare(field.text, "made-up-secret")
            field.forceActiveFocus()
            field.selectAll()
            keyClick(Qt.Key_Delete)
            dialog.close()
            tryCompare(dialog, "visible", false)
            page.askPassword({ id: "a", address: "camille@example.org", host: "imap.example.org", login: "" })
            tryCompare(dialog, "opened", true)
            compare(undoRedo(field), ["", ""], "nothing of the vault's")
            dialog.close()
            tryCompare(dialog, "visible", false)
        }
    }
}
