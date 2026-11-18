// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

// The password manager's card in Accounts (VaultCard.qml): the Bitwarden
// vault as it is now on this device (SettingRow.qml's "vault" kind, decided
// 3 November 2026), on a stand-in for Sioul that holds no vault: open, it
// says so with Lock, which locks it and tells the card; locked, Unlock,
// which asks the card to open it, and no Lock. Nothing asks Bitwarden. Run
// by tools/qml-test.sh.

import QtQuick
import QtQuick.Controls.Basic
import QtTest
import "../../qml"

Item {
    id: root

    width: 396
    height: 300

    Theme {
        id: testTheme
    }

    QtObject {
        id: mock

        property string vault: "unlocked"
        property int locks: 0

        function text(key) {
            return key
        }
        function bitwardenState() {
            return mock.vault
        }
        function bitwardenLock() {
            mock.locks += 1
            mock.vault = "locked"
        }
    }

    SignalSpy {
        id: unlockSpy

        signalName: "unlockAsked"
    }
    SignalSpy {
        id: changedSpy

        signalName: "changedElsewhere"
    }

    Component {
        id: rowComponent

        SettingRow {
            width: root.width
            setting: ({ key: "bitwarden_vault", kind: "vault", label: "set-bitwarden-vault", help: "set-bitwarden-vault-help", value: false, choices: [] })
            sioul: mock
            theme: testTheme
        }
    }

    TestCase {
        name: "VaultRow"
        when: windowShown

        function find(item, name) {
            if (!item)
                return null
            if (item.objectName === name)
                return item
            const kids = item.children || []
            for (let i = 0; i < kids.length; ++i) {
                const found = find(kids[i], name)
                if (found)
                    return found
            }
            return null
        }

        function test_an_open_vault_is_locked_by_lock() {
            mock.vault = "unlocked"
            mock.locks = 0
            const row = createTemporaryObject(rowComponent, root)
            changedSpy.target = row
            changedSpy.clear()
            tryVerify(() => find(row, "vaultState") !== null)
            compare(find(row, "vaultState").text, "set-bitwarden-vault-open")
            verify(!find(row, "vaultUnlock").visible, "open: no Unlock")
            const lock = find(row, "vaultLock")
            verify(lock.visible, "Lock while it is open")
            mouseClick(lock)
            compare(mock.locks, 1)
            compare(find(row, "vaultState").text, "set-bitwarden-vault-locked")
            verify(!lock.visible, "locked: no Lock")
            compare(changedSpy.count, 1, "the card told")
        }

        function test_a_locked_vault_offers_unlock_and_no_lock() {
            mock.vault = "locked"
            const row = createTemporaryObject(rowComponent, root)
            unlockSpy.target = row
            unlockSpy.clear()
            tryVerify(() => find(row, "vaultState") !== null)
            compare(find(row, "vaultState").text, "set-bitwarden-vault-locked")
            verify(!find(row, "vaultLock").visible)
            const unlock = find(row, "vaultUnlock")
            verify(unlock.visible, "Unlock while it is locked")
            mouseClick(unlock)
            compare(unlockSpy.count, 1, "the card opens its dialog")
        }
    }
}
