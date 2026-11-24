// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

// The password manager's card at the top of Accounts ▸ Your accounts
// (AccountsPage.qml, VaultCard.qml), on a stand-in for Sioul that holds no
// vault, in its three states: no account set, one line, the account's
// e-mail and server and Add, which opens the vault there (Add an account
// lists no Bitwarden: the card is its one place); locked, the account, its
// server, the lock after a time and Unlock, which opens the vault's dialog;
// open, Lock, which locks it. A link by key (SettingDoor.qml) shows the
// card. Made-up account; nothing asks Bitwarden. Run by tools/qml-test.sh.

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

        // "missing", "locked", "unlocked", as the backend says.
        property string vault: "missing"
        property string email: ""
        property int locks: 0
        readonly property string accounts: "[]"
        readonly property string found: ""
        readonly property string scouted: ""
        readonly property bool scouting: false
        readonly property bool formBusy: false
        readonly property string formError: ""
        property string status: ""

        signal bitwardenAnswered(int ticket, string answer)
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
        // The real backend's answer is a call, not a property: nothing tells a binding that
        // it changed. `quiet.vault`, changed in place, stands for that (a property read here
        // would wake every binding that called this, and hide a row that never reads again).
        property var quiet: ({ vault: "" })
        function bitwardenState() {
            return mock.quiet.vault !== "" ? mock.quiet.vault : mock.vault
        }
        // As the backend does: the vault emptied, then said.
        function bitwardenLock() {
            mock.locks += 1
            mock.vault = "locked"
            mock.bitwardenLocked()
        }
        function bitwardenUnlock(password, provider, code) {
            return 1
        }
        // Bitwarden's rows as the core gives them (`settings::for_view("bitwarden")`).
        function settings(view) {
            if (view !== "bitwarden")
                return "[]"
            const row = (key, kind, value) => ({ key: key, kind: kind, label: "set-" + key, help: "", value: value, choices: [], group: "", section: "", min: 0, max: 0, step: 1, unit: "" })
            return JSON.stringify(mock.vault === "missing" ? [] : [
                row("bitwarden.email", "text", mock.email),
                row("bitwarden.server", "text", ""),
                row("bitwarden_vault", "vault", false),
                row("bitwarden.lock_after", "choice", 0)
            ])
        }
        function setSetting(key, value) {
            if (key === "bitwarden.email") {
                mock.email = JSON.parse(value)
                mock.vault = mock.email === "" ? "missing" : "locked"
            }
            return ""
        }
        function changeSetting(key, shown, value) {
            return mock.setSetting(key, value)
        }
    }

    AccountsPage {
        id: page

        anchors.fill: parent
        sioul: mock
        theme: testTheme
    }

    TestCase {
        name: "VaultCard"
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
        function card() {
            return named(page, "vaultCard")
        }
        // Each state as the backend gives it, the card read again as the page shows.
        function set(vault, email) {
            mock.vault = vault
            mock.email = email
            page.openSettings(true, "")
            page.readVault()
            card().reload()
            wait(50)
        }

        function test_no_account_the_card_asks_for_it() {
            // Add an account lists no Bitwarden: the card is its one place.
            page.showTab(1)
            tryVerify(() => named(page, "addAccountTab") !== null, 2000)
            compare(walk(named(page, "addAccountTab"), i => typeof i.objectName === "string" && i.objectName.startsWith("vault"), []).length, 0, "nothing of Bitwarden in Add an account")
            set("missing", "")
            const shown = card()
            verify(shown !== null && shown.visible, "at the top of Your accounts")
            verify(named(shown, "vaultCardNone").visible)
            compare(walk(shown, i => i instanceof SettingRow, []).length, 0, "no row before an account")
            const add = named(shown, "vaultCardAdd")
            verify(add.visible && !add.enabled, "nothing to add before an address")
            verify(named(shown, "vaultCardServer").visible)
            mock.status = ""
            // Added on the card itself: it shows it, locked, and opens the vault at once.
            named(shown, "vaultCardEmail").text = "camille@example.org"
            verify(add.enabled)
            add.clicked()
            compare(mock.email, "camille@example.org")
            tryVerify(() => card().vault === "locked", 2000, "the account set, locked")
            verify(!named(shown, "vaultCardEmail").visible && !add.visible, "the form gone once set")
            compare(walk(shown, i => i instanceof SettingRow, []).map(r => r.setting.key), ["bitwarden.email", "bitwarden.server", "bitwarden_vault", "bitwarden.lock_after"])
            tryCompare(shown.unlockDialog, "opened", true, 2000, "the vault's dialog, at once")
            shown.unlockDialog.close()
            tryCompare(shown.unlockDialog, "visible", false, 2000)
        }

        function test_locked_its_account_and_unlock() {
            set("locked", "camille@example.org")
            const shown = card()
            verify(!named(shown, "vaultCardNone").visible)
            verify(!named(shown, "vaultCardAdd").visible)
            const rows = walk(shown, i => i instanceof SettingRow, [])
            compare(rows.map(r => r.setting.key), ["bitwarden.email", "bitwarden.server", "bitwarden_vault", "bitwarden.lock_after"])
            tryVerify(() => named(shown, "vaultUnlock") !== null && named(shown, "vaultUnlock").visible)
            verify(!named(shown, "vaultLock").visible)
            named(shown, "vaultUnlock").clicked()
            tryCompare(shown.unlockDialog, "opened", true, 2000, "the vault's dialog")
            shown.unlockDialog.close()
            tryCompare(shown.unlockDialog, "visible", false, 2000)
        }

        // Unlocked through the card's own dialog, the row says so at once: open, with Lock,
        // never "locked" still (found on a real session, 19 November 2026: the row read the
        // vault's state once, when it was made, and its rows were not made again).
        function test_unlocked_by_the_dialog_the_row_says_open() {
            mock.quiet.vault = "locked"
            set("locked", "camille@example.org")
            const shown = card()
            tryVerify(() => named(shown, "vaultUnlock") !== null && named(shown, "vaultUnlock").visible)
            compare(named(shown, "vaultState").text, "set-bitwarden-vault-locked")
            // As the dialog does once the vault opened: the backend's state changed, said by no property.
            mock.quiet.vault = "unlocked"
            shown.unlockDialog.unlocked()
            tryCompare(named(shown, "vaultState"), "text", "set-bitwarden-vault-open", 2000, "the row follows the vault")
            verify(named(shown, "vaultLock").visible && !named(shown, "vaultUnlock").visible)
            // And locked again by its time without use, told by the backend.
            mock.quiet.vault = "locked"
            mock.bitwardenLocked()
            tryCompare(named(shown, "vaultState"), "text", "set-bitwarden-vault-locked", 2000)
            verify(named(shown, "vaultUnlock").visible && !named(shown, "vaultLock").visible)
            mock.quiet.vault = ""
        }

        // Every password field of the forms that add an account offers "From Bitwarden…"
        // once an account is set (the mail form's waits for its server to be found).
        function test_the_forms_offer_the_vault_once_it_is_set() {
            set("missing", "")
            page.showTab(1)
            tryVerify(() => named(page, "addAccountTab") !== null)
            const fills = () => walk(page, i => i instanceof VaultFill, [])
            verify(fills().length >= 3, "under the mail's, the calendars' and Google's key's fields")
            compare(fills().filter(f => f.visible).length, 0, "none without an account")
            mock.vault = "locked"
            mock.email = "camille@example.org"
            page.readVault()
            tryVerify(() => fills().filter(f => f.visible).length >= 2, 2000, "the calendars' password and Google's key's secret")
            compare(fills().find(f => f.visible).text, "account-password-vault")
            page.openSettings(true, "")
        }

        function test_open_lock_locks_it() {
            set("unlocked", "camille@example.org")
            const shown = card()
            tryVerify(() => named(shown, "vaultLock") !== null && named(shown, "vaultLock").visible)
            verify(!named(shown, "vaultUnlock").visible)
            mock.locks = 0
            named(shown, "vaultLock").clicked()
            compare(mock.locks, 1)
            tryCompare(card(), "vault", "locked")
            compare(page.vault, "locked", "the forms' From Bitwarden… follow")
        }
    }
}
